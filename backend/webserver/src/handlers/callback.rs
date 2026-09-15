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
        "write_request"            => handle_write_request(&state, &callback).await,
        "export_collection"        => handle_export_collection(&state, &callback).await,
        "import_zip"                => handle_import_zip(&state, &callback).await,
        "generate_markdown"        => handle_generate_markdown(&state, &callback).await,
        "export_merge"             => handle_export_merge(&state, &callback).await,
        "mark_active_folder"       => handle_mark_active_folder(&state, &callback).await,
        "compute_crud_operations"  => handle_compute_crud_operations(&state, &callback).await,
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

// ── run_test ─────────────────────────────────────────────────────────────────
//
// The HTTP test call completed. Spawn edms-child to persist the response file,
// then broadcast TestFinished so WebSocket clients update.

async fn handle_run_test(state: &AppState, callback: &IpcCallback) {
    let r = &callback.result;

    let endpoint_id    = r["endpoint_id"].as_str().unwrap_or("").to_string();
    let request_number = r["request_number"].as_i64().unwrap_or(0) as i32;

    // Timeout path
    if r.get("timed_out").and_then(|v| v.as_bool()).unwrap_or(false) {
        // Cancel the app's own independent timer — it's on the same
        // timeout_ms but a separate schedule, so without this it can still
        // fire its own TestTimeout a tick later (duplicate event).
        state.cancel_timer(&endpoint_id, request_number);
        let _ = state.events_tx.send(ServerEvent::TestTimeout {
            endpoint_id,
            request_number,
        });
        return;
    }

    let status_code      = r["status_code"].as_i64().unwrap_or(0) as i32;
    let response_time_ms = r["response_time_ms"].as_i64().unwrap_or(0) as i32;
    let response_body    = r.get("response_body").cloned().unwrap_or(serde_json::Value::Null);
    // Must match the {eid}-response-{N}.json filename write_response_file
    // actually produces (compute::endpoint_writer) — see the matching note
    // on request_file in test_view.rs.
    let response_file    = state
        .endpoint_storage_dir(&endpoint_id)
        .join(format!("{endpoint_id}-response-{request_number}.json"))
        .display()
        .to_string();

    // Spawn edms-child to write the response file to disk
    crate::ipc::spawn_child(
        "write_response",
        serde_json::json!({
            "repo_path":  state.endpoint_storage_dir(&endpoint_id).display().to_string(),
            "eid":        endpoint_id,
            "res_index":  request_number,
            "content":    serde_json::to_string(&response_body).unwrap_or_default(),
        }),
        3000,
    );

    // Headers — per Ravi (2026-09-04). compute echoes back the request
    // headers it actually sent alongside the response's own headers, so
    // both land in one combined file without webserver needing to
    // remember what it asked for earlier.
    let request_headers  = r.get("request_headers").cloned().unwrap_or(serde_json::json!({}));
    let response_headers = r.get("response_headers").cloned().unwrap_or(serde_json::json!({}));
    let headers_content = serde_json::json!({
        "request_headers":  request_headers,
        "response_headers": response_headers,
    });
    crate::ipc::spawn_child(
        "write_headers",
        serde_json::json!({
            "repo_path": state.endpoint_storage_dir(&endpoint_id).display().to_string(),
            "eid":       endpoint_id,
            "index":     request_number,
            "content":   headers_content.to_string(),
        }),
        3000,
    );

    // Record response metadata in the DB — without this, response_metadata
    // stays permanently empty and QP Pairs (request+response joins) in the
    // CRUD Operations table can never show anything but zero.
    {
        let st = state.clone();
        let eid = endpoint_id.clone();
        let rf = response_file.clone();
        let res = tokio::task::spawn_blocking(move || {
            crate::db::insert_response_metadata(
                &st.core,
                &st.queries,
                &eid,
                request_number,
                &rf,
                status_code,
                Some(response_time_ms),
            )
        })
        .await;
        match res {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => warn!("[callback] failed to insert response_metadata: {e:?}"),
            Err(e) => warn!("[callback] failed to join insert_response_metadata task: {e}"),
        }
    }

    // Record this QP pair in History automatically — per Ravi's email
    // (2026-09-03), every completed test is auto-linked into History with
    // no manual step; only promoting one into Bookmarks is a deliberate
    // user action. POST /test-view/save/history remains available for
    // anything that wants to log a history entry without a real test run.
    let history_count = {
        let st = state.clone();
        let eid = endpoint_id.clone();
        let details = format!("{status_code} in {response_time_ms}ms");
        let res = tokio::task::spawn_blocking(move || {
            crate::db::insert_history(&st.core, &st.queries, &eid, "test", Some(&details))?;
            crate::db::history_count(&st.core, &st.queries)
        })
        .await;
        match res {
            Ok(Ok(count)) => Some(count),
            Ok(Err(e)) => {
                warn!("[callback] failed to auto-record history: {e:?}");
                None
            }
            Err(e) => {
                warn!("[callback] failed to join auto-record-history task: {e}");
                None
            }
        }
    };
    if let Some(count) = history_count {
        state.emit(ServerEvent::HistoryUpdated { count }).await;
    }

    // Test finished for real — stop the app's own countdown so it doesn't
    // keep emitting TimerTick after the fact.
    state.cancel_timer(&endpoint_id, request_number);

    // Broadcast TestFinished — WS clients update immediately
    let _ = state.events_tx.send(ServerEvent::TestFinished {
        endpoint_id,
        request_number,
        status_code,
        response_time_ms,
        response_file,
    });
}

// ── write_request ─────────────────────────────────────────────────────────────
//
// edms-child finished writing the request file to disk.
// Nothing to broadcast — this is a background persistence operation.

async fn handle_write_request(_state: &AppState, callback: &IpcCallback) {
    info!(
        "[callback] request file written: {}",
        callback.result["message"].as_str().unwrap_or("ok")
    );
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