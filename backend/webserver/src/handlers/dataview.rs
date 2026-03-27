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

use crate::{db, events::ServerEvent, state::AppState};

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
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "merged_folder": folder,
            "note": "merge logic not implemented yet"
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
    {
        let mut guard = state.active_folder.write().await;
        *guard = Some(folder.clone());
    }

    state
        .emit(ServerEvent::FolderBecameActive {
            folder: folder.clone(),
        })
        .await;

    let resp = json!({ "type": "active_folder_set", "folder": folder });
    let _ = socket.send(Message::Text(resp.to_string())).await;

    // stream events
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
            "endpoints": counts.0,
            "bookmarks": counts.1,
            "history": counts.2,
        })),
    )
}
