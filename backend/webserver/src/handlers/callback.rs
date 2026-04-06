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
        "run_test"           => handle_run_test(&state, &callback).await,
        "write_request"      => handle_write_request(&state, &callback).await,
        "export_collection"  => handle_export_collection(&state, &callback).await,
        "generate_markdown"  => handle_generate_markdown(&state, &callback).await,
        "export_merge"       => handle_export_merge(&state, &callback).await,
        "mark_active_folder" => handle_mark_active_folder(&state, &callback).await,
        _ => {
            info!(
                "[callback] task='{}' completed — no specific handler",
                callback.task
            );
        }
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
        let _ = state.events_tx.send(ServerEvent::TestTimeout {
            endpoint_id,
            request_number,
        });
        return;
    }

    let status_code      = r["status_code"].as_i64().unwrap_or(0) as i32;
    let response_time_ms = r["response_time_ms"].as_i64().unwrap_or(0) as i32;
    let response_body    = r.get("response_body").cloned().unwrap_or(serde_json::Value::Null);
    let response_file    = format!("edms_data/{}/response-{:03}.json", endpoint_id, request_number);

    // Spawn edms-child to write the response file to disk
    crate::ipc::spawn_child(
        "write_response",
        serde_json::json!({
            "repo_path":  format!("edms_data/{}", endpoint_id),
            "eid":        endpoint_id,
            "res_index":  request_number,
            "content":    serde_json::to_string(&response_body).unwrap_or_default(),
        }),
        3000,
    );

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