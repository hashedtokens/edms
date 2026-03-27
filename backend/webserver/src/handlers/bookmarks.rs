use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use crate::{db, events::ServerEvent, state::AppState};

pub async fn create_collection(
    State(state): State<AppState>,
    Path(collection): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let st = state.clone();
        let c = collection.clone();
        move || db::create_collection_from_active(&st.core, &c)
    })
    .await;

    match res {
        Ok(Ok(inserted)) => (
            StatusCode::OK,
            Json(json!({ "ok": true, "collection": collection, "inserted": inserted })),
        ),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": format!("{e:?}") })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": format!("{e:?}") })),
        ),
    }
}

pub async fn ws_load_collection(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(collection): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_ws_load_collection(socket, state, collection).await;
    })
}

async fn handle_ws_load_collection(mut socket: WebSocket, state: AppState, collection: String) {
    let res = tokio::task::spawn_blocking({
        let st = state.clone();
        let c = collection.clone();
        move || db::load_collection_into_active(&st.core, &c)
    })
    .await;

    match res {
        Ok(Ok((moved_to_backup, loaded))) => {
            state
                .emit(ServerEvent::CollectionLoaded {
                    collection: collection.clone(),
                    moved_to_backup,
                })
                .await;
            // also emit bookmark count updated
            let count = tokio::task::spawn_blocking({
                let st = state.clone();
                move || db::bookmarks_count_active(&st.core)
            })
            .await
            .ok()
            .and_then(|x| x.ok())
            .unwrap_or(0);
            state.emit(ServerEvent::BookmarksUpdated { count }).await;
            let resp = json!({
                "type": "collection_loaded",
                "collection": collection,
                "moved_to_backup": moved_to_backup,
                "loaded_into_active": loaded
            });
            let _ = socket.send(Message::Text(resp.to_string())).await;
        }
        Ok(Err(_)) | Err(_) => {
            let resp = json!({"type":"error","message":"failed to load collection"});
            let _ = socket.send(Message::Text(resp.to_string())).await;
            return;
        }
    }

    // stream events after load
    let mut rx = state.events_tx.subscribe();
    while let Ok(evt) = rx.recv().await {
        let msg = json!({"type":"event","event": evt});
        if socket.send(Message::Text(msg.to_string())).await.is_err() {
            break;
        }
    }
}