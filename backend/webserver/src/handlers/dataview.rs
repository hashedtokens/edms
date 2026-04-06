use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use std::path::Path as StdPath;

use crate::{db, events::ServerEvent, ipc, state::AppState};

const EDMS_DATA_DIR: &str = "edms_data";

pub async fn delete_folder(
    State(_state): State<AppState>,
    Path(folder): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let dir = format!("{EDMS_DATA_DIR}/{folder}");
    let res = tokio::task::spawn_blocking(move || {
        if StdPath::new(&dir).exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        Ok::<(), std::io::Error>(())
    })
    .await;

    match res {
        Ok(Ok(())) => (
            StatusCode::OK,
            Json(json!({ "ok": true, "deleted_folder": folder })),
        ),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": format!("{e:?}") })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": format!("join error: {e:?}") })),
        ),
    }
}

pub async fn merge_folder(
    State(_state): State<AppState>,
    Path(folder): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Spawn edms-child to handle the merge — result comes back via /internal/callback
    ipc::spawn_child(
        "export_merge",
        json!({
            "inputs":  [],
            "folders": [folder],
            "output":  format!("edms_root/exports/{folder}-merged.zip"),
        }),
        3000,
    );

    // Return immediately — this is fire and forget, result arrives via callback
    (
        StatusCode::ACCEPTED,
        Json(json!({
            "ok": true,
            "status": "merge queued",
            "folder": folder,
        })),
    )
}

pub async fn ws_make_folder_active(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(folder): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_ws_make_active(socket, state, folder).await;
    })
}

async fn handle_ws_make_active(mut socket: WebSocket, state: AppState, folder: String) {
    // 1. Update local state
    {
        let mut guard = state.active_folder.write().await;
        *guard = Some(folder.clone());
    }

    // 2. Spawn edms-child to sync the active folder on disk
    //    Result comes back via /internal/callback — non-blocking
    ipc::spawn_child(
        "mark_active_folder",
        json!({
            "session_backup":   "edms_root/session-backup",
            "active_folder":    "edms_root/active",
            "folder_name":      folder,
            "yaml_config_path": "src/config.yml",
        }),
        3000,
    );

    // 3. Broadcast event locally — don't wait for child
    state
        .emit(ServerEvent::FolderBecameActive {
            folder: folder.clone(),
        })
        .await;

    let resp = json!({ "type": "active_folder_set", "folder": folder });
    let _ = socket.send(Message::Text(resp.to_string())).await;

    // 4. Stream events over WS
    let mut rx = state.events_tx.subscribe();
    while let Ok(evt) = rx.recv().await {
        let msg = json!({ "type": "event", "event": evt });
        if socket.send(Message::Text(msg.to_string())).await.is_err() {
            break;
        }
    }
}

pub async fn dashboard(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    let active_folder = state.active_folder.read().await.clone();

    let counts = tokio::task::spawn_blocking({
        let st = state.clone();
        move || {
            let endpoints = db::list_endpoints(&st.core, &st.queries)
                .map(|v| v.len())
                .unwrap_or(0);
            let bookmarks = db::bookmarks_count_active(&st.core).unwrap_or(0);
            let history = db::history_count(&st.core).unwrap_or(0);
            (endpoints, bookmarks, history)
        }
    })
    .await
    .ok()
    .unwrap_or((0, 0, 0));

    (
        StatusCode::OK,
        Json(json!({
            "active_folder": active_folder,
            "endpoints":     counts.0,
            "bookmarks":     counts.1,
            "history":       counts.2,
        })),
    )
}