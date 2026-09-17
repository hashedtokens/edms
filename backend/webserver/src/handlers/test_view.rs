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

pub async fn ws_load_endpoints(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_ws_subscribe_endpoints(socket, state).await;
    })
}

pub async fn ws_load_bookmarks(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_ws_subscribe_bookmarks(socket, state).await;
    })
}

/// GET /test-view/history/load
///
/// Was missing entirely — db::list_history() existed but nothing called it,
/// so there was no way (REST or WS) to actually read history back, only
/// POST /test-view/save/history to write it. Mirrors ws_load_endpoints /
/// ws_load_bookmarks: sends an initial snapshot, then streams events.
pub async fn ws_load_history(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_ws_subscribe_history(socket, state).await;
    })
}

pub async fn ws_run(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_ws_run(socket, state).await;
    })
}

#[derive(Debug, Deserialize)]
pub struct StopRequest {
    pub endpoint_id: String,
    pub request_number: i32,
}

/// POST /test-view/stop
///
/// Cancels the app's own countdown timer for this test — the UI stops
/// getting TimerTick/expects-a-result state for it, and a TimerCancelled
/// event fires (same path the timer's own natural completion uses).
///
/// Honest limitation: this does NOT kill the actual in-flight HTTP call.
/// That's already running in a separate, detached edms-child process by
/// the time this is called, and nothing tracks a handle to it — so a slow
/// or hanging request keeps running in the background regardless. This
/// only stops the app from waiting on / reporting about it.
pub async fn stop(
    State(state): State<AppState>,
    Json(payload): Json<StopRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let cancelled = state.cancel_timer(&payload.endpoint_id, payload.request_number);
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "timer_cancelled": cancelled,
            "message": if cancelled {
                "Timer stopped. Note: the underlying HTTP call may still complete in the background."
            } else {
                "No running timer found for that endpoint/request — it may have already finished."
            }
        })),
    )
}

pub async fn save_history(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    let endpoint_id = payload["endpoint_id"].as_str().unwrap_or("").to_string();
    let action = payload["action"].as_str().unwrap_or("test").to_string();
    let details = payload.get("details").and_then(|v| v.as_str()).map(|s| s.to_string());

    let res = tokio::task::spawn_blocking({
        let st = state.clone();
        move || db::insert_history(&st.core, &st.queries, &endpoint_id, &action, details.as_deref())
    })
    .await;

    match res {
        Ok(Ok(_)) => {
            let count = tokio::task::spawn_blocking({
                let st = state.clone();
                move || db::history_count(&st.core, &st.queries)
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

pub async fn save_bookmark(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Per Mathew (2026-09-08): bookmarking requires a collection to
    // already be loaded — "active" is the draft state of whichever
    // collection that is, not a free-floating staging area.
    if let Err((status, body)) = crate::handlers::bookmarks::require_active_collection(&state).await {
        return (status, body);
    }

    let endpoint_id = payload["endpoint_id"].as_str().unwrap_or("").to_string();
    let notes = payload.get("notes").and_then(|v| v.as_str()).map(|s| s.to_string());

    let res = tokio::task::spawn_blocking({
        let st = state.clone();
        move || db::insert_bookmark_active(&st.core, &st.queries, &endpoint_id, notes.as_deref())
    })
    .await;

    match res {
        Ok(Ok(_)) => {
            let count = tokio::task::spawn_blocking({
                let st = state.clone();
                move || db::bookmarks_count_active(&st.core, &st.queries)
            })
            .await
            .unwrap_or(Ok(0))
            .unwrap_or(0);

            state.refresh_dashboard_snapshot();
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

pub async fn ws_add_from_history_to_bookmark(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(bookmark): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_ws_add_from_history(socket, state, bookmark).await;
    })
}

pub async fn ws_delete_from_bookmark(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(bookmark): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_ws_delete_from_bookmark(socket, state, bookmark).await;
    })
}

pub async fn clear_history(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let st = state.clone();
        move || db::clear_history(&st.core, &st.queries)
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

pub async fn clear_bookmarks(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let st = state.clone();
        move || db::clear_bookmarks_active(&st.core, &st.queries)
    })
    .await;

    match res {
        Ok(Ok(_)) => {
            state.refresh_dashboard_snapshot();
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

// ── Internal WS implementations ──────────────────────────────────────

#[derive(Debug, Deserialize)]
struct RunWrapper {
    #[serde(rename = "type")]
    _msg_type: String,
    payload: RunMessage,
}

#[derive(Debug, Deserialize)]
struct RunMessage {
    /// Optional — per Mathew (2026-09-08), an endpoint is only created
    /// the moment it's tested, not via a separate manual step. Omit this
    /// to have a fresh canonical EID auto-allocated; pass an existing
    /// one to re-test it (the common case); pass a not-yet-existing one
    /// to create it with that exact id.
    #[serde(default)]
    endpoint_id: Option<String>,
    /// The URL to test. Required even when re-testing an existing
    /// endpoint_id — used only to create the row when it doesn't exist
    /// yet; ignored (the stored endpoint_str wins) when it does.
    endpoint_str: String,
    method: String,
    #[serde(default, alias = "request_json")]
    body: Value,
    #[serde(default)]
    timeout_ms: u64,
    #[serde(default)]
    tick_interval_ms: u64,
    /// Headers to send with this request — per Ravi (2026-09-04), the
    /// third piece to capture alongside request/response.
    #[serde(default)]
    headers: std::collections::HashMap<String, String>,
    /// Annotation to save if this run ends up creating a brand-new
    /// endpoint. Ignored when re-testing an existing one.
    #[serde(default)]
    annotation: Option<String>,
}

impl RunMessage {
    fn timer_config(&self) -> TimerConfig {
        TimerConfig {
            limit_ms: if self.timeout_ms > 0 { self.timeout_ms } else { 30_000 },
            tick_interval_ms: if self.tick_interval_ms > 0 { self.tick_interval_ms } else { 500 },
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
                                    let timer_cfg = run_msg.timer_config();
                                    if let Err(e) = run_test_impl(
                                        &st,
                                        run_msg.endpoint_id.as_deref(),
                                        &run_msg.endpoint_str,
                                        &run_msg.method,
                                        run_msg.body.clone(),
                                        run_msg.headers.clone(),
                                        run_msg.annotation.clone(),
                                        timer_cfg,
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
//  run_test_impl
// ═════════════════════════════════════════════════════════════════════

async fn run_test_impl(
    state: &AppState,
    endpoint_id: Option<&str>,
    endpoint_str: &str,
    method: &str,
    request_json: Value,
    headers: std::collections::HashMap<String, String>,
    annotation: Option<String>,
    timer_cfg: TimerConfig,
) -> Result<(), EdmsError> {
    // 1) Get-or-create the endpoint — per Mathew (2026-09-08), an endpoint
    //    is only created the moment it's actually tested. Re-testing an
    //    existing endpoint_id is the common case (returned as-is); a new
    //    or omitted id creates a fresh row here.
    let (endpoint, was_created) = crate::handlers::endpoints::get_or_create_endpoint(
        state,
        endpoint_id,
        endpoint_str,
        Some(method.to_uppercase()),
        annotation,
    )
    .await
    .map_err(|(_, msg)| {
        tracing::warn!("[run_test_impl] get_or_create_endpoint failed: {msg}");
        EdmsError::UnknownError
    })?;
    let endpoint_id = endpoint.endpoint_id.clone();
    if was_created {
        state.refresh_dashboard_snapshot();
    }

    // 2) Allocate request number
    let request_number = tokio::task::spawn_blocking({
        let st = state.clone();
        let id = endpoint_id.to_string();
        move || db::get_next_request_number(&st.core, &st.queries, &id)
    })
    .await
    .map_err(|_| EdmsError::UnknownError)?
    .map_err(|_| EdmsError::UnknownError)?;

    // 3) Insert request metadata into DB
    let method_upper = method.to_uppercase();
    // Must match the {eid}-request-{N}.json filename write_request_file
    // actually produces (compute::endpoint_writer) — these two were out of
    // sync before, meaning the file this points at didn't exist.
    let request_file = state
        .endpoint_storage_dir(&endpoint_id)
        .join(format!("{endpoint_id}-request-{request_number}.json"))
        .display()
        .to_string();

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

    // 4) Spawn edms-child to write the request file to disk
    //    Result comes back via /internal/callback
    ipc::spawn_child(
        "write_request",
        json!({
            "repo_path":  state.endpoint_storage_dir(&endpoint_id).display().to_string(),
            "eid":        endpoint_id,
            "req_index":  request_number,
            "content":    serde_json::to_string(&request_json).unwrap_or_default(),
        }),
        3000,
    );

    // 5) Emit TestStarted
    let _ = state.events_tx.send(ServerEvent::TestStarted {
        endpoint_id: endpoint_id.to_string(),
        request_number,
    });

    // 6) Start timer — keep the handle so callback.rs can cancel it once
    //    the real outcome (success or timeout) is known, instead of letting
    //    it tick on its own independent schedule.
    let timer_handle = timer::spawn_timer(
        endpoint_id.to_string(),
        request_number,
        timer_cfg.clone(),
        state.events_tx.clone(),
    );
    state
        .active_timers
        .lock()
        .unwrap()
        .insert((endpoint_id.to_string(), request_number), timer_handle);

    // 7) Spawn edms-child for the actual HTTP test call
    //    This is the original run_test task — unchanged
    ipc::spawn_child(
        "run_test",
        json!({
            "endpoint_id":    endpoint_id,
            "url":            endpoint.endpoint_str,
            "method":         method_upper,
            "body":           request_json,
            "request_number": request_number,
            "timeout_ms":     timer_cfg.limit_ms,
            "headers":        headers,
        }),
        3000,
    );

    Ok(())
}

// ── Subscription handlers ─────────────────────────────────────────────

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
    let active_collection = state.active_collection.read().await.clone();

    // Per Mathew (2026-09-08): loading a collection copies/resolves full
    // endpoint data into bookmark view, not just EIDs — endpoints_for_ids
    // already does that join. What's new here is also marking, per
    // endpoint, whether it's currently a *saved member* of the loaded
    // collection (vs. just bookmarked-but-not-yet-saved) — computed by
    // cross-referencing against the collection's own membership set, never
    // stored redundantly in the bookmarks table itself.
    let bookmarks = tokio::task::spawn_blocking({
        let st = state.clone();
        let active_collection = active_collection.clone();
        move || {
            let ids_with_timestamps =
                db::list_bookmarked_endpoints_active_with_timestamps(&st.core, &st.queries)?;
            let ids: Vec<String> = ids_with_timestamps.iter().map(|(id, _)| id.clone()).collect();
            let endpoints = db::endpoints_for_ids(&st.core, &st.queries, &ids)?;
            let updated_at: std::collections::HashMap<String, String> =
                ids_with_timestamps.into_iter().collect();

            let member_set = match &active_collection {
                Some(name) => {
                    match crate::handlers::view_catalog::open_existing_collection_membership(&st, name)
                        .and_then(|m| m.list_set().map_err(|e| format!("{e:?}")))
                    {
                        Ok(set) => set,
                        Err(_) => std::collections::HashSet::new(),
                    }
                }
                None => std::collections::HashSet::new(),
            };

            Ok::<_, EdmsError>((endpoints, member_set, updated_at))
        }
    })
    .await;

    if let Ok(Ok((bks, member_set, updated_at))) = bookmarks {
        let enriched: Vec<Value> = bks
            .iter()
            .map(|ep| {
                let mut v = serde_json::to_value(ep).unwrap_or(Value::Null);
                if let Some(obj) = v.as_object_mut() {
                    obj.insert(
                        "in_collection".to_string(),
                        Value::Bool(member_set.contains(&ep.endpoint_id)),
                    );
                    obj.insert(
                        "updated".to_string(),
                        updated_at
                            .get(&ep.endpoint_id)
                            .cloned()
                            .map(Value::String)
                            .unwrap_or(Value::Null),
                    );
                }
                v
            })
            .collect();
        let count = enriched.len();
        let resp = json!({
            "type": "snapshot",
            "active_collection": active_collection,
            "bookmarks": enriched
        });
        let _ = socket.send(Message::Text(resp.to_string())).await;
        let _ = state.events_tx.send(ServerEvent::ActiveWorkspaceBookmarksLoaded { count });
    }

    let mut rx = state.events_tx.subscribe();
    while let Ok(evt) = rx.recv().await {
        let msg = json!({ "type": "event", "event": evt });
        if socket.send(Message::Text(msg.to_string())).await.is_err() {
            break;
        }
    }
}

async fn handle_ws_subscribe_history(mut socket: WebSocket, state: AppState) {
    let history = tokio::task::spawn_blocking({
        let st = state.clone();
        move || db::list_history(&st.core, &st.queries)
    })
    .await;

    if let Ok(Ok(entries)) = history {
        let resp = json!({ "type": "snapshot", "history": entries });
        let _ = socket.send(Message::Text(resp.to_string())).await;
        let _ = state.events_tx.send(ServerEvent::ActiveWorkspaceHistoryLoaded {
            count: entries.len(),
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

/// Maps the public-facing "active" alias used in `:bookmark` path params
/// to the internal ACTIVE_FOLDER constant ("__active__"). Without this,
/// `/test-view/active/add|delete` silently wrote to/deleted from a
/// literal folder named "active" — a completely different, nonexistent
/// row from what save_bookmark/bookmarks/load actually use — so deletes
/// always matched 0 rows and reported success while doing nothing.
/// Confirmed 2026-09-08 during end-to-end testing; pre-existing, not
/// introduced by the bookmark/collection merge work.
fn resolve_folder_alias(bookmark: &str) -> String {
    if bookmark == "active" {
        db::ACTIVE_FOLDER.to_string()
    } else {
        bookmark.to_string()
    }
}

async fn handle_ws_add_from_history(mut socket: WebSocket, state: AppState, bookmark: String) {
    let bookmark = resolve_folder_alias(&bookmark);
    while let Some(Ok(msg)) = socket.recv().await {
        let text = match msg {
            Message::Text(t) => t,
            _ => continue,
        };

        let endpoint_id = match serde_json::from_str::<Value>(&text) {
            Ok(v) => v["endpoint_id"].as_str().unwrap_or("").to_string(),
            Err(_) => text.trim().to_string(),
        };

        // Same gate as save_bookmark, only for the "active" workspace —
        // other named folders are legacy/unused surface this pass doesn't
        // otherwise touch.
        if bookmark == db::ACTIVE_FOLDER {
            if let Err((_, body)) = crate::handlers::bookmarks::require_active_collection(&state).await {
                let _ = socket.send(Message::Text(body.0.to_string())).await;
                continue;
            }
        }

        let res = tokio::task::spawn_blocking({
            let st = state.clone();
            let eid = endpoint_id.clone();
            let folder = bookmark.clone();
            move || db::insert_bookmark(&st.core, &st.queries, &eid, &folder, None)
        })
        .await;

        match res {
            Ok(Ok(_)) => {
                let count = tokio::task::spawn_blocking({
                    let st = state.clone();
                    let folder = bookmark.clone();
                    move || db::bookmarks_count(&st.core, &st.queries, &folder)
                })
                .await
                .unwrap_or(Ok(0))
                .unwrap_or(0);

                state.refresh_dashboard_snapshot();
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

async fn handle_ws_delete_from_bookmark(mut socket: WebSocket, state: AppState, bookmark: String) {
    let bookmark = resolve_folder_alias(&bookmark);
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
            let folder = bookmark.clone();
            move || db::delete_bookmark(&st.core, &st.queries, &eid, &folder)
        })
        .await;

        match res {
            Ok(Ok(_)) => {
                let count = tokio::task::spawn_blocking({
                    let st = state.clone();
                    let folder = bookmark.clone();
                    move || db::bookmarks_count(&st.core, &st.queries, &folder)
                })
                .await
                .unwrap_or(Ok(0))
                .unwrap_or(0);

                state.refresh_dashboard_snapshot();
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

// ── Fetch a saved request/response body ─────────────────────────────────
//
// TestFinished only carries status_code/response_time_ms/response_file — a
// server-side path, not the actual content. These routes are the missing
// second half of the WS-trigger-then-REST-fetch pattern: the client uses
// endpoint_id + request_number (already known from TestFinished) to pull
// the real body. Deliberately NOT taking a raw path from the client —
// that would be a path-traversal risk. The server builds the path itself,
// the same way test_view.rs/callback.rs already do when writing it.

fn safe_id(id: &str) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    if id.is_empty() || id.contains("..") || id.contains('/') || id.contains('\\') {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": "invalid endpoint_id" })),
        ));
    }
    Ok(())
}

async fn read_saved_file(
    state: &AppState,
    endpoint_id: &str,
    request_number: i64,
    kind: &str,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Err(e) = safe_id(endpoint_id) {
        return e;
    }
    let path = state
        .endpoint_storage_dir(endpoint_id)
        .join(format!("{endpoint_id}-{kind}-{request_number}.json"));
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => {
            let body: serde_json::Value =
                serde_json::from_str(&content).unwrap_or(serde_json::Value::String(content));
            (StatusCode::OK, Json(json!({ "ok": true, "body": body })))
        }
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "ok": false, "error": format!("no saved {kind} for {endpoint_id}#{request_number}: {e}") })),
        ),
    }
}

/// GET /test-view/{endpoint_id}/request/{request_number}
pub async fn get_saved_request(
    State(state): State<AppState>,
    Path((endpoint_id, request_number)): Path<(String, i64)>,
) -> (StatusCode, Json<serde_json::Value>) {
    read_saved_file(&state, &endpoint_id, request_number, "request").await
}

/// GET /test-view/{endpoint_id}/response/{request_number}
pub async fn get_saved_response(
    State(state): State<AppState>,
    Path((endpoint_id, request_number)): Path<(String, i64)>,
) -> (StatusCode, Json<serde_json::Value>) {
    read_saved_file(&state, &endpoint_id, request_number, "response").await
}

/// GET /test-view/{endpoint_id}/headers/{request_number}
/// Body is {"request_headers": {...}, "response_headers": {...}}.
pub async fn get_saved_headers(
    State(state): State<AppState>,
    Path((endpoint_id, request_number)): Path<(String, i64)>,
) -> (StatusCode, Json<serde_json::Value>) {
    read_saved_file(&state, &endpoint_id, request_number, "headers").await
}

// ── QP list / delete ─────────────────────────────────────────────────
//
// A "QP" (request/response pair) is one test run against an endpoint —
// already persisted automatically in request_metadata/response_metadata +
// the three saved JSON files whenever a test runs (see run_test_impl
// above). These two routes are the missing "access all of them" / "get
// rid of one" pieces — everything else about a QP already existed.

/// GET /test-view/{endpoint_id}/qps
/// Lists every QP pair saved for this endpoint, oldest first.
pub async fn list_qps(
    State(state): State<AppState>,
    Path(endpoint_id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Err(e) = safe_id(&endpoint_id) {
        return e;
    }

    let res = tokio::task::spawn_blocking({
        let st = state.clone();
        let eid = endpoint_id.clone();
        move || db::list_qps_for_endpoint(&st.core, &st.queries, &eid)
    })
    .await;

    match res {
        Ok(Ok(qps)) => (StatusCode::OK, Json(json!({ "ok": true, "qps": qps }))),
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

/// POST /test-view/{endpoint_id}/qps/{request_number}/delete
/// Deletes one QP pair's metadata rows and its three saved JSON files
/// (request/response/headers). 404 if no such pair exists.
pub async fn delete_qp(
    State(state): State<AppState>,
    Path((endpoint_id, request_number)): Path<(String, i32)>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Err(e) = safe_id(&endpoint_id) {
        return e;
    }

    let res = tokio::task::spawn_blocking({
        let st = state.clone();
        let eid = endpoint_id.clone();
        move || db::delete_qp(&st.core, &st.queries, &eid, request_number)
    })
    .await;

    let rows_affected = match res {
        Ok(Ok(n)) => n,
        Ok(Err(e)) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": format!("{e:?}") })),
            )
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": format!("{e:?}") })),
            )
        }
    };

    if rows_affected == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "ok": false, "error": format!("no QP pair {endpoint_id}#{request_number}") })),
        );
    }

    // Best-effort: the metadata rows are the source of truth this route
    // deletes by, so a file that's already missing isn't an error.
    let dir = state.endpoint_storage_dir(&endpoint_id);
    for kind in ["request", "response", "headers"] {
        let path = dir.join(format!("{endpoint_id}-{kind}-{request_number}.json"));
        let _ = tokio::fs::remove_file(path).await;
    }

    state
        .emit(ServerEvent::QpDeleted {
            endpoint_id,
            request_number,
        })
        .await;

    (StatusCode::OK, Json(json!({ "ok": true })))
}