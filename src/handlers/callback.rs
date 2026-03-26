use axum::{extract::State, Json};
use serde_json::json;
use tracing::{error, info};

use crate::{
    events::ServerEvent,
    ipc::IpcCallback,
    state::AppState,
};

/// POST /internal/callback
///
/// Called by child processes to report their result.
pub async fn ipc_callback(
    State(state): State<AppState>,
    Json(callback): Json<IpcCallback>,
) -> Json<serde_json::Value> {
    info!(
        "[callback] Received: task='{}' success={} elapsed={}ms",
        callback.task, callback.success, callback.elapsed_ms
    );

    if !callback.success {
        if let Some(ref err) = callback.error {
            error!("[callback] Task '{}' failed: {}", callback.task, err);
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
        "run_test" => handle_run_test_callback(&state, &callback).await,
        _ => {
            info!(
                "[callback] Task '{}' completed, no specific handler — result: {}",
                callback.task, callback.result
            );
        }
    }

    Json(json!({ "status": "received" }))
}

async fn handle_run_test_callback(state: &AppState, callback: &IpcCallback) {
    let r = &callback.result;

    let endpoint_id = r["endpoint_id"].as_str().unwrap_or("").to_string();
    let request_number = r["request_number"].as_i64().unwrap_or(0) as i32;

    if r.get("timed_out").and_then(|v| v.as_bool()).unwrap_or(false) {
        let _ = state.events_tx.send(ServerEvent::TestTimeout {
            endpoint_id,
            request_number,
        });
        return;
    }

    let status_code = r["status_code"].as_i64().unwrap_or(0) as i32;
    let response_time_ms = r["response_time_ms"].as_i64().unwrap_or(0) as i32;
    let response_file = r["response_file"].as_str().unwrap_or("").to_string();

    let _ = state.events_tx.send(ServerEvent::TestFinished {
        endpoint_id,
        request_number,
        status_code,
        response_time_ms,
        response_file,
    });
}