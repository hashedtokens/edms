use axum::{extract::State, Json};
use serde_json::json;
use tracing::{error, info, warn};

use crate::{
    events::ServerEvent,
    ipc::IpcCallback,
    state::AppState,
};

/// POST /internal/callback
///
/// Single entry point for ALL edms-child processes to report back.
/// Each task has its own handler below.
pub async fn ipc_callback(
    State(state): State<AppState>,
    Json(callback): Json<IpcCallback>,
) -> Json<serde_json::Value> {
    info!(
        "[callback] task='{}' success={} elapsed={}ms",
        callback.task, callback.success, callback.elapsed_ms
    );

    if !callback.success {
        if let Some(ref err) = callback.error {
            error!("[callback] task='{}' failed: {}", callback.task, err);
        }
        // A job's child reports its failure here: fail the job too.
        crate::handlers::repoview_ie::abort_import(&state, &callback);
        crate::handlers::jobs::fail_from_callback(&state, &callback);
        let _ = state.events_tx.send(ServerEvent::Error {
            message: format!(
                "IPC task '{}' failed: {}",
                callback.task,
                callback.error.as_deref().unwrap_or("unknown error")
            ),
        });
        return Json(json!({ "status": "error_noted" }));
    }

    match callback.task.as_str() {
        "run_test"                 => handle_run_test(&state, &callback).await,
        "export_collection"        => handle_export_collection(&state, &callback).await,
        "import_zip"                => handle_import_zip(&state, &callback).await,
        "generate_markdown"        => handle_generate_markdown(&state, &callback).await,
        "export_merge"             => handle_export_merge(&state, &callback).await,
        "mark_active_folder"       => handle_mark_active_folder(&state, &callback).await,
        "compute_crud_operations"  => handle_compute_crud_operations(&state, &callback).await,
        "view_io_progress"         => crate::handlers::jobs::progress_from_callback(&state, &callback),
        "view_takeout"             => crate::handlers::jobs::complete_from_callback(&state, &callback),
        "view_import_copy"         => crate::handlers::repoview_ie::finish_import(&state, &callback).await,
        "view_table_check"         => crate::handlers::jobs::complete_from_callback(&state, &callback),
        "collection_merge"         => crate::handlers::collection_merge::finish_merge(&state, &callback),
        "view_unzip"               => crate::handlers::repoview_ie::continue_after_unzip(&state, &callback).await,
        "convert_to_pdf" | "convert_to_html" | "convert_to_markdown" => {
            handle_file_conversion(&state, &callback).await
        }
        _ => {
            info!(
                "[callback] task='{}' completed — no specific handler",
                callback.task
            );
        }
    }

    // Check if the result requests a ViewType refresh
    if let Some(vt) = callback
        .result
        .get("view_type")
        .or_else(|| callback.result.get("ViewType"))
        .and_then(|v| v.as_str())
    {
        let count = callback
            .result
            .get("count")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;
        info!("[callback] broadcasting ViewRefresh for view_type='{vt}', count={count}");
        let _ = state.events_tx.send(ServerEvent::ViewRefresh {
            view_type: vt.to_string(),
            count,
        });
    }

    Json(json!({ "status": "received" }))
}

async fn handle_file_conversion(state: &AppState, callback: &IpcCallback) {
    let output_path = callback.result["output_path"]
        .as_str()
        .unwrap_or("unknown output path");
    info!(
        "[callback] {} wrote report to {}",
        callback.task, output_path
    );
    let _ = state.events_tx.send(ServerEvent::ExportReady {
        message: format!("EQP report ready: {output_path}"),
    });
}

// ── run_test ─────────────────────────────────────────────────────────────────
//
// The HTTP call of a Run Test came back (or timed out). A run saves nothing:
// the result is kept in memory under its run id and sent to the page as
// RunFinished (or RunTimeout). Save Test, POST /test-view/save/test, is what
// writes it.

async fn handle_run_test(state: &AppState, callback: &IpcCallback) {
    let r = &callback.result;
    let run_id = r["endpoint_id"].as_str().unwrap_or("").to_string();

    if !crate::test_runs::is_run_id(&run_id) {
        warn!("[callback] run_test answer for '{run_id}', which is not a run id - dropped");
        return;
    }

    if r.get("timed_out").and_then(|v| v.as_bool()).unwrap_or(false) {
        if state.pending_runs.discard(&run_id) {
            let _ = state.events_tx.send(ServerEvent::RunTimeout { run_id });
        }
        return;
    }

    let result = crate::test_runs::RunResult {
        status_code: r["status_code"].as_i64().unwrap_or(0) as i32,
        response_time_ms: r["response_time_ms"].as_i64().unwrap_or(0) as i32,
        response_body: r.get("response_body").cloned().unwrap_or(serde_json::Value::Null),
        request_headers: r.get("request_headers").cloned().unwrap_or(serde_json::json!({})),
        response_headers: r.get("response_headers").cloned().unwrap_or(serde_json::json!({})),
    };
    match state.pending_runs.finish(&run_id, result.clone()) {
        Some(run) => {
            let _ = state.events_tx.send(ServerEvent::RunFinished {
                run_id,
                endpoint_str: run.endpoint_str,
                method: run.method,
                status_code: result.status_code,
                response_time_ms: result.response_time_ms,
                request_body: run.request_body,
                response_body: result.response_body,
                request_headers: result.request_headers,
                response_headers: result.response_headers,
            });
        }
        None => info!("[callback] result for run '{run_id}' which is gone (timed out or stopped) - dropped"),
    }
}

// ── export_collection ─────────────────────────────────────────────────────────
//
// edms-child finished packaging the zip. Broadcast so the UI can
// show a download-ready notification.

async fn handle_export_collection(state: &AppState, callback: &IpcCallback) {
    info!(
        "[callback] export_collection done: {}",
        callback.result["message"].as_str().unwrap_or("ok")
    );
    let _ = state.events_tx.send(ServerEvent::ExportReady {
        message: "Collection export complete".to_string(),
    });
}

// ── import_zip ───────────────────────────────────────────────────────────────
//
// edms-child finished unzipping the collection archive to disk. Broadcast
// so the UI can refresh (e.g. reload the collection's endpoint list).

async fn handle_import_zip(state: &AppState, callback: &IpcCallback) {
    info!(
        "[callback] import_zip done: {}",
        callback.result["message"].as_str().unwrap_or("ok")
    );
    let _ = state.events_tx.send(ServerEvent::ImportReady {
        message: "Collection import complete".to_string(),
    });
}

// ── generate_markdown ─────────────────────────────────────────────────────────

async fn handle_generate_markdown(state: &AppState, callback: &IpcCallback) {
    info!(
        "[callback] generate_markdown done: {}",
        callback.result["message"].as_str().unwrap_or("ok")
    );
    let _ = state.events_tx.send(ServerEvent::ExportReady {
        message: "Markdown generation complete".to_string(),
    });
}

// ── export_merge ──────────────────────────────────────────────────────────────

async fn handle_export_merge(state: &AppState, callback: &IpcCallback) {
    info!(
        "[callback] export_merge done: {}",
        callback.result["message"].as_str().unwrap_or("ok")
    );
    let _ = state.events_tx.send(ServerEvent::ExportReady {
        message: "Merge export complete".to_string(),
    });
}

// ── mark_active_folder ────────────────────────────────────────────────────────

async fn handle_mark_active_folder(state: &AppState, callback: &IpcCallback) {
    info!(
        "[callback] mark_active_folder done: {}",
        callback.result["message"].as_str().unwrap_or("ok")
    );
    // active_folder is already updated in AppState by handle_ws_make_active
    // before the child was even spawned — nothing more to do here
}

// ── compute_crud_operations ─────────────────────────────────────────────────────
//
// The child process finished scanning edms.db and grouping every entity
// type by HTTP method. Write it into dashboard.db and broadcast so the
// dashboard's Refresh button can flip out of its loading state.

async fn handle_compute_crud_operations(state: &AppState, callback: &IpcCallback) {
    match crate::handlers::dashboard::store_crud_operations_result(state, &callback.result) {
        Ok(computed_at) => {
            info!("[callback] compute_crud_operations stored, computed_at={computed_at}");
            let _ = state
                .events_tx
                .send(ServerEvent::CrudOperationsUpdated { computed_at });
        }
        Err(e) => {
            warn!("[callback] failed to store crud operations result: {e}");
            let _ = state.events_tx.send(ServerEvent::Error {
                message: format!("Failed to store CRUD operations result: {e}"),
            });
        }
    }
}
