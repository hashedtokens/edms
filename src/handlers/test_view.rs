use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    db,
    events::ServerEvent,
    ipc,
    state::AppState,
    timer::{self, TimerConfig},
};

use edms::error::EdmsError;

// ── WS handlers ──────────────────────────────────────────────────────

/// WS: /test-view/endpoints/load
pub async fn ws_load_endpoints(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_ws_subscribe_endpoints(socket, state).await;
    })
}

/// WS: /test-view/bookmarks/load
pub async fn ws_load_bookmarks(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_ws_subscribe_bookmarks(socket, state).await;
    })
}

/// WS: /test-view/run
pub async fn ws_run(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_ws_run(socket, state).await;
    })
}

/// REST: /test-view/stop
pub async fn stop() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(json!({ "ok": true, "message": "stop requested" })),
    )
}

/// REST: /test-view/save/history
pub async fn save_history(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Extract fields from the JSON payload
    let endpoint_id = payload["endpoint_id"].as_str().unwrap_or("").to_string();
    let action = payload["action"].as_str().unwrap_or("test").to_string();
    let details = payload.get("details").and_then(|v| v.as_str()).map(|s| s.to_string());

    let res = tokio::task::spawn_blocking({
        let st = state.clone();
        move || db::insert_history(&st.core, &endpoint_id, &action, details.as_deref())
    })
    .await;

    match res {
        Ok(Ok(_)) => {
            // Get updated count
            let count = tokio::task::spawn_blocking({
                let st = state.clone();
                move || db::history_count(&st.core)
            })
            .await
            .unwrap_or(Ok(0))
            .unwrap_or(0);

            state.emit(ServerEvent::HistoryUpdated { count }).await;
            (StatusCode::OK, Json(json!({ "ok": true, "history_count": count })))
        }
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

/// REST: /test-view/save/bookmark
pub async fn save_bookmark(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    let endpoint_id = payload["endpoint_id"].as_str().unwrap_or("").to_string();
    let notes = payload.get("notes").and_then(|v| v.as_str()).map(|s| s.to_string());

    let res = tokio::task::spawn_blocking({
        let st = state.clone();
        move || db::insert_bookmark_active(&st.core, &endpoint_id, notes.as_deref())
    })
    .await;

    match res {
        Ok(Ok(_)) => {
            let count = tokio::task::spawn_blocking({
                let st = state.clone();
                move || db::bookmarks_count_active(&st.core)
            })
            .await
            .unwrap_or(Ok(0))
            .unwrap_or(0);

            state.emit(ServerEvent::BookmarksUpdated { count }).await;
            (StatusCode::OK, Json(json!({ "ok": true, "bookmark_count": count })))
        }
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

/// WS: /test-view/:bookmark/add
pub async fn ws_add_from_history_to_bookmark(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(bookmark): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_ws_add_from_history(socket, state, bookmark).await;
    })
}

/// WS: /test-view/:bookmark/delete
pub async fn ws_delete_from_bookmark(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(bookmark): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_ws_delete_from_bookmark(socket, state, bookmark).await;
    })
}

/// REST: /test-view/history/clearall
pub async fn clear_history(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let st = state.clone();
        move || db::clear_history(&st.core)
    })
    .await;

    match res {
        Ok(Ok(_)) => {
            state.emit(ServerEvent::HistoryUpdated { count: 0 }).await;
            (StatusCode::OK, Json(json!({ "ok": true })))
        }
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

/// REST: /test-view/bookmark/clearall
pub async fn clear_bookmarks(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let st = state.clone();
        move || db::clear_bookmarks_active(&st.core)
    })
    .await;

    match res {
        Ok(Ok(_)) => {
            state.emit(ServerEvent::BookmarksUpdated { count: 0 }).await;
            (StatusCode::OK, Json(json!({ "ok": true })))
        }
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

// ── Internal WS implementations ─────────────────────────────────────

#[derive(Debug, Deserialize)]
struct RunWrapper {
    #[serde(rename = "type")]
    _msg_type: String,
    payload: RunMessage,
}

#[derive(Debug, Deserialize)]
struct RunMessage {
    endpoint_id: String,
    method: String,
    #[serde(default, alias = "request_json")]
    body: Value,
    #[serde(default)]
    timeout_ms: u64,
    #[serde(default)]
    tick_interval_ms: u64,
}

impl RunMessage {
    fn timer_config(&self) -> TimerConfig {
        TimerConfig {
            limit_ms: if self.timeout_ms > 0 { self.timeout_ms } else { 30_000 },
            tick_interval_ms: if self.tick_interval_ms > 0 {
                self.tick_interval_ms
            } else {
                500
            },
        }
    }
}

async fn handle_ws_run(mut socket: WebSocket, state: AppState) {
    let mut rx = state.events_tx.subscribe();

    loop {
        tokio::select! {
            Ok(evt) = rx.recv() => {
                let msg = json!({ "type": "event", "event": evt });
                if socket.send(Message::Text(msg.to_string())).await.is_err() {
                    break;
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<RunWrapper>(&text) {
                            Ok(wrapper) => {
                                let run_msg = wrapper.payload;
                                let st = state.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = run_test_impl(
                                        &st,
                                        &run_msg.endpoint_id,
                                        &run_msg.method,
                                        run_msg.body.clone(),
                                        run_msg.timer_config(),
                                    )
                                    .await
                                    {
                                        let _ = st.events_tx.send(ServerEvent::Error {
                                            message: format!("{e:?}"),
                                        });
                                    }
                                });
                            }
                            Err(e) => {
                                let resp = json!({"type":"error","message": format!("bad message: {e}")});
                                let _ = socket.send(Message::Text(resp.to_string())).await;
                            }
                        }
                    }
                    Some(Ok(_)) => continue,
                    _ => break,
                }
            }
        }
    }
}

// ═════════════════════════════════════════════════════════════════════
//  run_test_impl — IPC PASSTHROUGH
// ═════════════════════════════════════════════════════════════════════

async fn run_test_impl(
    state: &AppState,
    endpoint_id: &str,
    method: &str,
    request_json: Value,
    timer_cfg: TimerConfig,
) -> Result<(), EdmsError> {
    // 1) Look up endpoint (lightweight DB read)
    let endpoint = tokio::task::spawn_blocking({
        let st = state.clone();
        let id = endpoint_id.to_string();
        move || db::get_endpoint(&st.core, &st.queries, &id)
    })
    .await
    .map_err(|_| EdmsError::UnknownError)?
    .map_err(|_| EdmsError::UnknownError)?
    .ok_or(EdmsError::UnknownError)?;

    // 2) Allocate request number (lightweight DB write)
    let request_number = tokio::task::spawn_blocking({
        let st = state.clone();
        let id = endpoint_id.to_string();
        move || db::get_next_request_number(&st.core, &st.queries, &id)
    })
    .await
    .map_err(|_| EdmsError::UnknownError)?
    .map_err(|_| EdmsError::UnknownError)?;

    // 3) Insert request metadata
    let method_upper = method.to_uppercase();
    let request_file = format!("edms_data/{}/request-{:03}.json", endpoint_id, request_number);

    tokio::task::spawn_blocking({
        let st = state.clone();
        let eid = endpoint_id.to_string();
        let rf = request_file.clone();
        let m = method_upper.clone();
        move || db::insert_request_metadata(&st.core, &st.queries, &eid, request_number, &rf, &m)
    })
    .await
    .map_err(|_| EdmsError::UnknownError)?
    .map_err(|_| EdmsError::UnknownError)?;

    // 4) Emit TestStarted
    let _ = state.events_tx.send(ServerEvent::TestStarted {
        endpoint_id: endpoint_id.to_string(),
        request_number,
    });

    // 5) Start timer task
   let _timer_handle = timer::spawn_timer(
        endpoint_id.to_string(),
        request_number,
        timer_cfg.clone(),
        state.events_tx.clone(),
    );

    // 6) FIRE AND FORGET — spawn child process for the HTTP call
    let child_payload = json!({
        "endpoint_id": endpoint_id,
        "url": endpoint.endpoint_str,
        "method": method_upper,
        "body": request_json,
        "request_number": request_number,
        "timeout_ms": timer_cfg.limit_ms,
    });

    ipc::spawn_child("run_test", child_payload, 3000);

    Ok(())
}

// ── Subscription handlers ────────────────────────────────────────────

async fn handle_ws_subscribe_endpoints(mut socket: WebSocket, state: AppState) {
    let endpoints = tokio::task::spawn_blocking({
        let st = state.clone();
        move || db::list_endpoints(&st.core, &st.queries)
    })
    .await;

    if let Ok(Ok(eps)) = endpoints {
        let resp = json!({ "type": "snapshot", "endpoints": eps });
        let _ = socket.send(Message::Text(resp.to_string())).await;

        let _ = state.events_tx.send(ServerEvent::ActiveWorkspaceEndpointsLoaded {
            count: eps.len(),
        });
    }

    let mut rx = state.events_tx.subscribe();
    while let Ok(evt) = rx.recv().await {
        let msg = json!({ "type": "event", "event": evt });
        if socket.send(Message::Text(msg.to_string())).await.is_err() {
            break;
        }
    }
}

async fn handle_ws_subscribe_bookmarks(mut socket: WebSocket, state: AppState) {
    // Use list_bookmarked_endpoints_active to get endpoint IDs,
    // then resolve them to full EndpointDto objects for the frontend
    let bookmarks = tokio::task::spawn_blocking({
        let st = state.clone();
        move || {
            let ids = db::list_bookmarked_endpoints_active(&st.core)?;
            db::endpoints_for_ids(&st.core, &st.queries, &ids)
        }
    })
    .await;

    if let Ok(Ok(bks)) = bookmarks {
        let resp = json!({ "type": "snapshot", "bookmarks": bks });
        let _ = socket.send(Message::Text(resp.to_string())).await;

        let _ = state.events_tx.send(ServerEvent::ActiveWorkspaceBookmarksLoaded {
            count: bks.len(),
        });
    }

    let mut rx = state.events_tx.subscribe();
    while let Ok(evt) = rx.recv().await {
        let msg = json!({ "type": "event", "event": evt });
        if socket.send(Message::Text(msg.to_string())).await.is_err() {
            break;
        }
    }
}

async fn handle_ws_add_from_history(mut socket: WebSocket, state: AppState, _bookmark: String) {
    while let Some(Ok(msg)) = socket.recv().await {
        let text = match msg {
            Message::Text(t) => t,
            _ => continue,
        };

        // Parse the endpoint_id from the incoming message
        let endpoint_id = match serde_json::from_str::<Value>(&text) {
            Ok(v) => v["endpoint_id"].as_str().unwrap_or("").to_string(),
            Err(_) => text.trim().to_string(),
        };

        let res = tokio::task::spawn_blocking({
            let st = state.clone();
            let eid = endpoint_id.clone();
            move || db::insert_bookmark_active(&st.core, &eid, None)
        })
        .await;

        match res {
            Ok(Ok(_)) => {
                let count = tokio::task::spawn_blocking({
                    let st = state.clone();
                    move || db::bookmarks_count_active(&st.core)
                })
                .await
                .unwrap_or(Ok(0))
                .unwrap_or(0);

                state.emit(ServerEvent::BookmarksUpdated { count }).await;
                let resp = json!({ "type": "ok", "bookmark_count": count });
                let _ = socket.send(Message::Text(resp.to_string())).await;
            }
            _ => {
                let resp = json!({ "type": "error", "message": "failed to add to bookmark" });
                let _ = socket.send(Message::Text(resp.to_string())).await;
            }
        }
    }
}

async fn handle_ws_delete_from_bookmark(mut socket: WebSocket, state: AppState, _bookmark: String) {
    while let Some(Ok(msg)) = socket.recv().await {
        let text = match msg {
            Message::Text(t) => t,
            _ => continue,
        };

        let endpoint_id = match serde_json::from_str::<Value>(&text) {
            Ok(v) => v["endpoint_id"].as_str().unwrap_or("").to_string(),
            Err(_) => text.trim().to_string(),
        };

        let res = tokio::task::spawn_blocking({
            let st = state.clone();
            let eid = endpoint_id.clone();
            move || db::delete_bookmark_active(&st.core, &eid)
        })
        .await;

        match res {
            Ok(Ok(_)) => {
                let count = tokio::task::spawn_blocking({
                    let st = state.clone();
                    move || db::bookmarks_count_active(&st.core)
                })
                .await
                .unwrap_or(Ok(0))
                .unwrap_or(0);

                state.emit(ServerEvent::BookmarksUpdated { count }).await;
                let resp = json!({ "type": "ok", "bookmark_count": count });
                let _ = socket.send(Message::Text(resp.to_string())).await;
            }
            _ => {
                let resp = json!({ "type": "error", "message": "failed to delete from bookmark" });
                let _ = socket.send(Message::Text(resp.to_string())).await;
            }
        }
    }
}