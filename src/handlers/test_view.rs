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
use std::time::Instant;

use crate::{
    db,
    events::ServerEvent,
    state::AppState,
};

use edms::{error::EdmsError, file_io};

/// WS: /test-view/endpoints/load  (notification: yes)
pub async fn ws_load_endpoints(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_ws_subscribe_endpoints(socket, state).await;
    })
}

/// WS: /test-view/bookmarks/load  (notification: yes)
pub async fn ws_load_bookmarks(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_ws_subscribe_bookmarks(socket, state).await;
    })
}

/// WS: /test-view/run  (run the test)
pub async fn ws_run(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_ws_run(socket, state).await;
    })
}

/// REST: /test-view/stop  (placeholder; cancel is non-trivial without tracking tasks)
pub async fn stop() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "message": "stop requested (not implemented: no task tracking yet)"
        })),
    )
}

/// REST: /test-view/save/history  (notification: yes)
#[derive(Debug, Deserialize)]
pub struct SaveHistoryRequest {
    pub endpoint_id: String,
    pub status: String, // "success"/"failed"/...
    pub details: Option<Value>,
}

pub async fn save_history(
    State(state): State<AppState>,
    Json(payload): Json<SaveHistoryRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let details_str = payload.details.as_ref().map(|v| v.to_string());
    let res = tokio::task::spawn_blocking({
        let st = state.clone();
        let endpoint_id = payload.endpoint_id.clone();
        let status = payload.status.clone();
        move || db::insert_history(&st.core, &endpoint_id, &status, details_str.as_deref())
    })
    .await;

    match res {
        Ok(Ok(_)) => {
            let count = tokio::task::spawn_blocking({
                let st = state.clone();
                move || db::history_count(&st.core)
            })
            .await
            .ok()
            .and_then(|x| x.ok())
            .unwrap_or(0);

            state.emit(ServerEvent::HistoryUpdated { count }).await;

            (StatusCode::OK, Json(json!({ "ok": true, "history_count": count })))
        }
        // FIX #4: Separate error handling for different error types
        Ok(Err(db_err)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": format!("database error: {db_err:?}") })),
        ),
        Err(join_err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": format!("task error: {join_err:?}") })),
        ),
    }
}

/// REST: /test-view/save/bookmark  (notification: yes)
#[derive(Debug, Deserialize)]
pub struct SaveBookmarkRequest {
    pub endpoint_id: String,
    pub notes: Option<String>,
}

pub async fn save_bookmark(
    State(state): State<AppState>,
    Json(payload): Json<SaveBookmarkRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let st = state.clone();
        let endpoint_id = payload.endpoint_id.clone();
        let notes = payload.notes.clone();
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
            .ok()
            .and_then(|x| x.ok())
            .unwrap_or(0);

            state.emit(ServerEvent::BookmarksUpdated { count }).await;

            (StatusCode::OK, Json(json!({ "ok": true, "bookmark_count": count })))
        }
        Ok(Err(db_err)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": format!("database error: {db_err:?}") })),
        ),
        Err(join_err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": format!("task error: {join_err:?}") })),
        ),
    }
}

/// WS: /test-view/:bookmark/add  (adds entries from history into active bookmarks)
pub async fn ws_add_from_history_to_bookmark(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(bookmark): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_ws_add_from_history(socket, state, bookmark).await;
    })
}

/// WS: /test-view/:bookmark/delete  (removes entries from active bookmarks)
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
        Ok(Err(db_err)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": format!("database error: {db_err:?}") })),
        ),
        Err(join_err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": format!("task error: {join_err:?}") })),
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
        Ok(Err(db_err)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": format!("database error: {db_err:?}") })),
        ),
        Err(join_err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": format!("task error: {join_err:?}") })),
        ),
    }
}

/* ---------------- WS internals ---------------- */

async fn handle_ws_subscribe_endpoints(mut socket: WebSocket, state: AppState) {
    // Snapshot: list endpoints from DB
    let endpoints = tokio::task::spawn_blocking({
        let st = state.clone();
        move || db::list_endpoints(&st.core, &st.queries)
    })
    .await
    .ok()
    .and_then(|x| x.ok())
    .unwrap_or_default();

    let snapshot = json!({
        "type": "snapshot",
        "topic": "endpoints",
        "endpoints": endpoints
    });
    let _ = socket.send(Message::Text(snapshot.to_string())).await;

    // notification yes
    state
        .emit(ServerEvent::ActiveWorkspaceEndpointsLoaded {
            count: endpoints.len(),
        })
        .await;

    // stream events
    stream_events(&mut socket, &state).await;
}

async fn handle_ws_subscribe_bookmarks(mut socket: WebSocket, state: AppState) {
    // Snapshot: list active bookmarks from DB and hydrate to EndpointDto
    let endpoints = tokio::task::spawn_blocking({
        let st = state.clone();
        move || {
            let ids = db::list_bookmarked_endpoints_active(&st.core)?;
            db::endpoints_for_ids(&st.core, &st.queries, &ids)
        }
    })
    .await
    .ok()
    .and_then(|x| x.ok())
    .unwrap_or_default();

    let snapshot = json!({
        "type": "snapshot",
        "topic": "bookmarks",
        "bookmarks": endpoints
    });
    let _ = socket.send(Message::Text(snapshot.to_string())).await;

    // notification yes
    state
        .emit(ServerEvent::ActiveWorkspaceBookmarksLoaded {
            count: endpoints.len(),
        })
        .await;

    // stream events
    stream_events(&mut socket, &state).await;
}

async fn stream_events(socket: &mut WebSocket, state: &AppState) {
    let mut rx = state.events_tx.subscribe();

    loop {
        tokio::select! {
            evt = rx.recv() => {
                match evt {
                    Ok(e) => {
                        let msg = json!({"type":"event","event": e});
                        if socket.send(Message::Text(msg.to_string())).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => { /* ignore */ }
                    Some(Err(_)) => break,
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", content = "payload")]
enum ClientMsg {
    Run {
        endpoint_id: String,
        method: String,
        request_json: Value,
        timeout_ms: Option<u64>,   // default 60_000
        notify_only: Option<bool>, // if true, WS returns queued; event delivers result
    },
}

async fn handle_ws_run(mut socket: WebSocket, state: AppState) {
    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(txt) = msg {
            let parsed: Result<ClientMsg, _> = serde_json::from_str(&txt);
            match parsed {
                Ok(ClientMsg::Run { endpoint_id, method, request_json, timeout_ms, notify_only }) => {
                    let timeout_ms = timeout_ms.unwrap_or(60_000);
                    let _notify_only = notify_only.unwrap_or(false); // FIX #14: Prefix with _ to silence warning

                    // FIX #1 & #2: Properly clone state and remove dead socket_clone code
                    let ack = json!({"type":"ack","ok":true,"queued":true,"endpoint_id": &endpoint_id});
                    let _ = socket.send(Message::Text(ack.to_string())).await;

                    // Spawn the test with properly cloned state
                    tokio::spawn({
                        let st = state.clone();
                        let endpoint_id = endpoint_id.clone();
                        async move {
                            if let Err(e) = run_test_impl(&st, &endpoint_id, &method, request_json, timeout_ms).await {
                                let _ = st.events_tx.send(ServerEvent::Error { message: format!("{e:?}") });
                            }
                        }
                    });
                }
                Err(e) => {
                    let resp = json!({"type":"error","message": format!("bad message: {e}")});
                    let _ = socket.send(Message::Text(resp.to_string())).await;
                }
            }
        }
    }
}

// FIX #7: Changed signature to take references to avoid unnecessary cloning
// and wrapped file I/O in spawn_blocking
async fn run_test_impl(
    state: &AppState,
    endpoint_id: &str,
    method: &str,
    request_json: Value,
    timeout_ms: u64,
) -> Result<(), EdmsError> {
    // 1) load endpoint url from DB
    let endpoint = tokio::task::spawn_blocking({
        let st = state.clone();
        let id = endpoint_id.to_string();
        move || db::get_endpoint(&st.core, &st.queries, &id)
    })
    .await
    .map_err(|_| EdmsError::UnknownError)?
    .map_err(|_| EdmsError::UnknownError)?
    .ok_or(EdmsError::UnknownError)?;

    // 2) next request number
    let request_number = tokio::task::spawn_blocking({
        let st = state.clone();
        let id = endpoint_id.to_string();
        move || db::get_next_request_number(&st.core, &st.queries, &id)
    })
    .await
    .map_err(|_| EdmsError::UnknownError)?
    .map_err(|_| EdmsError::UnknownError)?;

    // FIX #7: Wrap file I/O in spawn_blocking
    let request_file = tokio::task::spawn_blocking({
        let endpoint_id = endpoint_id.to_string();
        let request_json = request_json.clone();
        move || file_io::write_request_json(&endpoint_id, request_number, &request_json)
    })
    .await
    .map_err(|_| EdmsError::UnknownError)?
    .map_err(|_| EdmsError::UnknownError)?;

    let method_upper = method.to_uppercase();

    tokio::task::spawn_blocking({
        let st = state.clone();
        let endpoint_id = endpoint_id.to_string();
        let request_file = request_file.clone();
        let method = method_upper.clone();
        move || db::insert_request_metadata(&st.core, &st.queries, &endpoint_id, request_number, &request_file, &method)
    })
    .await
    .map_err(|_| EdmsError::UnknownError)?
    .map_err(|_| EdmsError::UnknownError)?;

    let _ = state.events_tx.send(ServerEvent::TestStarted { 
        endpoint_id: endpoint_id.to_string(), 
        request_number 
    });

    // 4) do HTTP call with timeout
    let url = endpoint.endpoint_str.clone();
    let method_parsed = method_upper.parse::<axum::http::Method>().unwrap_or(axum::http::Method::POST);

    // FIX #12: Use shared HTTP client from AppState for connection pooling
    let client = state.http_client.clone();
    
    let events_tx = state.events_tx.clone();
    let endpoint_id_owned = endpoint_id.to_string();
    
    let future = async {
        let start = Instant::now();

        let resp = match method_parsed {
            axum::http::Method::GET => client.get(&url).send().await,
            axum::http::Method::POST => client.post(&url).json(&request_json).send().await,
            axum::http::Method::PUT => client.put(&url).json(&request_json).send().await,
            axum::http::Method::DELETE => client.delete(&url).json(&request_json).send().await,
            _ => client.request(method_parsed.clone(), &url).json(&request_json).send().await,
        };

        // FIX #13: Use saturating conversion to avoid overflow
        let elapsed_ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;

        let (status_code, response_value) = match resp {
            Ok(r) => {
                let status = r.status().as_u16() as i32;
                let text = r.text().await.unwrap_or_else(|_| "".to_string());
                let v: Value = serde_json::from_str(&text).unwrap_or(json!({ "raw": text }));
                (status, v)
            }
            Err(e) => {
                let _ = events_tx.send(ServerEvent::Error { message: e.to_string() });
                (599, json!({ "error": e.to_string() }))
            }
        };

        Ok::<(i32, i32, Value), EdmsError>((status_code, elapsed_ms, response_value))
    };

    let timed = tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), future).await;

    match timed {
        Ok(Ok((status, elapsed_ms, response_value))) => {
            // FIX #7: Wrap file I/O in spawn_blocking
            let response_file = tokio::task::spawn_blocking({
                let endpoint_id = endpoint_id.to_string();
                move || file_io::write_response_json(&endpoint_id, request_number, &response_value)
            })
            .await
            .map_err(|_| EdmsError::UnknownError)?
            .map_err(|_| EdmsError::UnknownError)?;

            tokio::task::spawn_blocking({
                let st = state.clone();
                let endpoint_id = endpoint_id.to_string();
                let response_file = response_file.clone();
                move || db::insert_response_metadata(&st.core, &st.queries, &endpoint_id, request_number, &response_file, status, Some(elapsed_ms))
            })
            .await
            .map_err(|_| EdmsError::UnknownError)?
            .map_err(|_| EdmsError::UnknownError)?;

            // Save a history row too (so your "history tab" is real)
            let details = json!({
                "request_number": request_number,
                "status_code": status,
                "response_time_ms": elapsed_ms,
                "request_file": request_file,
                "response_file": response_file,
            })
            .to_string();

            let _ = tokio::task::spawn_blocking({
                let st = state.clone();
                let endpoint_id = endpoint_id.to_string();
                move || db::insert_history(&st.core, &endpoint_id, "test_finished", Some(&details))
            }).await;

            // notify
            let _ = state.events_tx.send(ServerEvent::TestFinished {
                endpoint_id: endpoint_id_owned,
                request_number,
                status_code: status,
                response_time_ms: elapsed_ms,
                response_file: response_file.clone(),
            });

            // emit history count update
            if let Ok(Ok(count)) = tokio::task::spawn_blocking({
                let st = state.clone();
                move || db::history_count(&st.core)
            }).await {
                let _ = state.events_tx.send(ServerEvent::HistoryUpdated { count });
            }

            Ok(())
        }
        Ok(Err(_e)) => {
            let _ = state.events_tx.send(ServerEvent::Error { message: "test failed".into() });
            Ok(())
        }
        Err(_) => {
            let _ = state.events_tx.send(ServerEvent::TestTimeout { 
                endpoint_id: endpoint_id_owned, 
                request_number 
            });
            Ok(())
        }
    }
}

// FIX #6: Changed from indices to endpoint_ids to avoid race condition
#[derive(Debug, Deserialize)]
struct AddFromHistoryCmd {
    // Changed: Now accepts endpoint_ids directly instead of indices
    endpoint_ids: Vec<String>,
}

async fn handle_ws_add_from_history(mut socket: WebSocket, state: AppState, _bookmark: String) {
    // FIX #15: Acknowledged that bookmark param exists but we're using __active__
    let hello = json!({"type":"info","message": "connected: add to active bookmarks"});
    let _ = socket.send(Message::Text(hello.to_string())).await;

    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(txt) = msg {
            let parsed: Result<AddFromHistoryCmd, _> = serde_json::from_str(&txt);
            match parsed {
                Ok(cmd) => {
                    // FIX #6: Use endpoint_ids directly instead of indices
                    let res = tokio::task::spawn_blocking({
                        let st = state.clone();
                        let endpoint_ids = cmd.endpoint_ids;
                        move || {
                            let mut added = 0usize;
                            for eid in &endpoint_ids {
                                // This now uses the fixed insert_bookmark_active with IGNORE
                                added += db::insert_bookmark_active(&st.core, eid, None)?;
                            }
                            Ok::<usize, EdmsError>(added)
                        }
                    }).await;

                    match res {
                        Ok(Ok(_added)) => {
                            let count = tokio::task::spawn_blocking({
                                let st = state.clone();
                                move || db::bookmarks_count_active(&st.core)
                            })
                            .await
                            .ok()
                            .and_then(|x| x.ok())
                            .unwrap_or(0);

                            state.emit(ServerEvent::BookmarksUpdated { count }).await;

                            let resp = json!({"type":"ok","bookmark_count": count});
                            let _ = socket.send(Message::Text(resp.to_string())).await;
                        }
                        _ => {
                            let resp = json!({"type":"error","message":"failed to add"});
                            let _ = socket.send(Message::Text(resp.to_string())).await;
                        }
                    }
                }
                Err(e) => {
                    let resp = json!({"type":"error","message": format!("bad add cmd: {e}")});
                    let _ = socket.send(Message::Text(resp.to_string())).await;
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct DeleteFromBookmarkCmd {
    endpoint_ids: Vec<String>,
}

async fn handle_ws_delete_from_bookmark(mut socket: WebSocket, state: AppState, _bookmark: String) {
    let hello = json!({"type":"info","message": "connected: delete from active bookmarks"});
    let _ = socket.send(Message::Text(hello.to_string())).await;

    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(txt) = msg {
            let parsed: Result<DeleteFromBookmarkCmd, _> = serde_json::from_str(&txt);
            match parsed {
                Ok(cmd) => {
                    let res = tokio::task::spawn_blocking({
                        let st = state.clone();
                        move || {
                            let mut removed = 0usize;
                            for eid in &cmd.endpoint_ids {
                                removed += db::delete_bookmark_active(&st.core, eid)?;
                            }
                            Ok::<usize, EdmsError>(removed)
                        }
                    }).await;

                    match res {
                        Ok(Ok(_)) => {
                            let count = tokio::task::spawn_blocking({
                                let st = state.clone();
                                move || db::bookmarks_count_active(&st.core)
                            })
                            .await
                            .ok()
                            .and_then(|x| x.ok())
                            .unwrap_or(0);

                            state.emit(ServerEvent::BookmarksUpdated { count }).await;

                            let resp = json!({"type":"ok","bookmark_count": count});
                            let _ = socket.send(Message::Text(resp.to_string())).await;
                        }
                        _ => {
                            let resp = json!({"type":"error","message":"failed to delete"});
                            let _ = socket.send(Message::Text(resp.to_string())).await;
                        }
                    }
                }
                Err(e) => {
                    let resp = json!({"type":"error","message": format!("bad delete cmd: {e}")});
                    let _ = socket.send(Message::Text(resp.to_string())).await;
                }
            }
        }
    }
}

