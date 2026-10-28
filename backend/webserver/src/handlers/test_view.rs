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
    test_runs::{spawn_run_timer, PendingRun, TakeError},
    timer::TimerConfig,
};
use tokio_util::sync::CancellationToken;

use edms::error::EdmsError;

// ── WS handlers ──────────────────────────────────────────────────────

pub async fn ws_load_endpoints(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_ws_subscribe_endpoints(socket, state).await;
    })
}

/// GET /test-view/:collection/bookmarks/load — collection-scoped, unlike
/// the old parameter-less version that read `state.active_collection`.
pub async fn ws_load_bookmarks(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(collection): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_ws_subscribe_bookmarks(socket, state, collection).await;
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
    /// The run's `run_id`.
    pub endpoint_id: String,
    /// Not used any more; still accepted so an existing caller's body parses.
    #[serde(default)]
    #[allow(dead_code)]
    pub request_number: i32,
}

/// POST /test-view/stop
///
/// Stops waiting for a run: pass its `run_id` as `endpoint_id` (and 0 as
/// `request_number`). The run is dropped and `RunCancelled` is sent.
///
/// Honest limitation: this does NOT kill the actual in-flight HTTP call.
/// That's already running in a separate, detached edms-child process by
/// the time this is called, and nothing tracks a handle to it - so a slow
/// or hanging request keeps running in the background regardless; its
/// answer is simply ignored.
pub async fn stop(
    State(state): State<AppState>,
    Json(payload): Json<StopRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let stopped = crate::test_runs::is_run_id(&payload.endpoint_id)
        && state.pending_runs.discard(&payload.endpoint_id);
    if stopped {
        let _ = state.events_tx.send(ServerEvent::RunCancelled {
            run_id: payload.endpoint_id.clone(),
        });
    }
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "timer_cancelled": stopped,
            "message": if stopped {
                "Run stopped and dropped. Note: the underlying HTTP call may still complete in the background."
            } else {
                "No running test found for that run id - it may have already finished."
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

/// POST /test-view/save/bookmark — body now requires `collection` too,
/// since there's no more implicit "the loaded one" to fall back to.
pub async fn save_bookmark(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    let collection = payload["collection"].as_str().unwrap_or("").to_string();
    if collection.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": "collection is required" })),
        );
    }
    let endpoint_id = payload["endpoint_id"].as_str().unwrap_or("").to_string();
    let notes = payload.get("notes").and_then(|v| v.as_str()).map(|s| s.to_string());

    let res = tokio::task::spawn_blocking({
        let st = state.clone();
        let collection = collection.clone();
        move || db::insert_bookmark(&st.core, &st.queries, &endpoint_id, &collection, notes.as_deref())
    })
    .await;

    match res {
        Ok(Ok(_)) => {
            let count = tokio::task::spawn_blocking({
                let st = state.clone();
                let collection = collection.clone();
                move || db::bookmarks_count(&st.core, &st.queries, &collection)
            })
            .await
            .unwrap_or(Ok(0))
            .unwrap_or(0);

            state.refresh_dashboard_snapshot();
            state.emit(ServerEvent::BookmarksUpdated { collection, count }).await;
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

/// POST /test-view/:collection/bookmark/clearall
pub async fn clear_bookmarks(
    State(state): State<AppState>,
    Path(collection): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let st = state.clone();
        let collection = collection.clone();
        move || db::delete_bookmarks_for_folder(&st.core, &st.queries, &collection)
    })
    .await;

    match res {
        Ok(Ok(_)) => {
            state.refresh_dashboard_snapshot();
            state.emit(ServerEvent::BookmarksUpdated { collection, count: 0 }).await;
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
                                let resp = match start_run(&state, wrapper.payload) {
                                    Ok(run_id) => json!({"type":"run_accepted","run_id": run_id}),
                                    Err(e) => json!({"type":"error","message": e}),
                                };
                                let _ = socket.send(Message::Text(resp.to_string())).await;
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
//  Run without saving
// ═════════════════════════════════════════════════════════════════════

/// Starts a run that saves nothing: no endpoint, no request number, no
/// files, no History row. The same `run_test` child makes the HTTP call; it
/// is handed the run id where an endpoint id would go and echoes it back,
/// which is how the callback knows to keep the result in memory (see
/// `test_runs.rs`) instead of writing it.
fn start_run(state: &AppState, m: RunMessage) -> Result<String, String> {
    if m.endpoint_str.trim().is_empty() {
        return Err("endpoint_str is required".to_string());
    }
    if m.method.trim().is_empty() {
        return Err("method is required".to_string());
    }
    let cfg = m.timer_config();
    let method = m.method.to_uppercase();
    let token = CancellationToken::new();
    let run_id = state.pending_runs.begin(
        m.endpoint_id.clone().filter(|id| !id.trim().is_empty()),
        m.endpoint_str.clone(),
        method.clone(),
        m.annotation.clone(),
        m.body.clone(),
        token.clone(),
    );

    spawn_run_timer(
        run_id.clone(),
        token,
        cfg.clone(),
        state.events_tx.clone(),
        state.pending_runs.clone(),
    );
    let _ = state.events_tx.send(ServerEvent::RunStarted {
        run_id: run_id.clone(),
        endpoint_str: m.endpoint_str.clone(),
        method: method.clone(),
    });

    let started = ipc::spawn_child(
        "run_test",
        json!({
            "endpoint_id":    run_id,
            "url":            m.endpoint_str,
            "method":         method,
            "body":           m.body,
            "request_number": 0,
            "timeout_ms":     cfg.limit_ms,
            "headers":        m.headers,
        }),
        3000,
    );
    if !started {
        state.pending_runs.discard(&run_id);
        return Err("could not start the test process".to_string());
    }
    Ok(run_id)
}

#[derive(Debug, Deserialize)]
pub struct SaveTestRequest {
    pub run_id: String,
    /// Annotation for the endpoint if saving creates it; ignored when the
    /// endpoint already exists.
    #[serde(default)]
    pub annotation: Option<String>,
}

/// POST /test-view/save/test - saves a run made with `save: false`.
///
/// Does what a saving Run Test does, from the kept result: creates the
/// endpoint if that URL and method aren't there already, adds the QP (request,
/// response and headers files plus their rows) and writes the History row.
/// An identical QP already saved is not added again: the answer says
/// `saved: false` and which QP it matches, and no History row is written.
///
/// 404 if the run is unknown, timed out, was stopped, or is older than 30
/// minutes; 409 if its HTTP call hasn't come back yet. A run saves once.
pub async fn save_test(
    State(state): State<AppState>,
    Json(payload): Json<SaveTestRequest>,
) -> (StatusCode, Json<Value>) {
    let run = match state.pending_runs.take_finished(&payload.run_id) {
        Ok(run) => run,
        Err(TakeError::NotFound) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "ok": false,
                    "error": format!("no finished run '{}' (it may have timed out, been stopped or expired)", payload.run_id)
                })),
            )
        }
        Err(TakeError::StillRunning) => {
            return (
                StatusCode::CONFLICT,
                Json(json!({ "ok": false, "error": "the run hasn't finished yet" })),
            )
        }
    };

    match persist_run(&state, &run, payload.annotation).await {
        Ok(body) => (StatusCode::OK, Json(body)),
        Err((status, msg)) => {
            // nothing was kept: the run can be saved again
            state.pending_runs.restore(run);
            (status, Json(json!({ "ok": false, "error": msg })))
        }
    }
}

fn internal<E: std::fmt::Debug>(e: E) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}"))
}

async fn persist_run(
    state: &AppState,
    run: &PendingRun,
    annotation: Option<String>,
) -> Result<Value, (StatusCode, String)> {
    let result = run
        .result
        .as_ref()
        .ok_or((StatusCode::CONFLICT, "the run hasn't finished yet".to_string()))?;
    let run_method = run.method.to_uppercase();

    // 1) the endpoint: reused when this URL + method is already saved
    let (endpoint, endpoint_created) = crate::handlers::endpoints::get_or_create_endpoint(
        state,
        run.endpoint_id.as_deref(),
        &run.endpoint_str,
        Some(run_method.clone()),
        annotation.or_else(|| run.annotation.clone()),
    )
    .await?;
    let endpoint_id = endpoint.endpoint_id.clone();
    if endpoint_created {
        state.refresh_dashboard_snapshot();
        let _ = state.events_tx.send(ServerEvent::CrudOperationsUpdated {
            computed_at: chrono::Utc::now().to_rfc3339(),
        });
    }

    // 2) the QP's three files' content, exactly as a saving run writes them
    let req_content = serde_json::to_string(&run.request_body).unwrap_or_default();
    let res_content = serde_json::to_string(&result.response_body).unwrap_or_default();
    let hdr_content = json!({
        "request_headers":  result.request_headers,
        "response_headers": result.response_headers,
    })
    .to_string();

    let qp_method = endpoint.method.clone().unwrap_or_else(|| run_method.clone());
    let dir = state.endpoint_storage_dir(&endpoint_id);

    let request_number = tokio::task::spawn_blocking({
        let st = state.clone();
        let id = endpoint_id.clone();
        move || db::get_next_request_number(&st.core, &st.queries, &id)
    })
    .await
    .map_err(internal)?
    .map_err(internal)?;

    // 3) an identical QP is saved already: don't add it again
    if let Some(dup) = crate::qp_service::find_duplicate_qp(
        &dir,
        &endpoint_id,
        &qp_method,
        &endpoint.endpoint_str,
        &req_content,
        &res_content,
        &hdr_content,
        None,
    )
    .filter(|n| *n > 0)
    {
        let _ = state.events_tx.send(ServerEvent::DuplicateQpBlocked {
            endpoint_id: endpoint_id.clone(),
            request_number,
            duplicate_of: dup,
        });
        return Ok(json!({
            "ok": true,
            "saved": false,
            "duplicate_of": dup,
            "endpoint_id": endpoint_id,
            "endpoint_created": endpoint_created,
            "request_number": dup,
        }));
    }
    let qp_data = crate::qp_service::extract_qp_data(&endpoint.endpoint_str);
    state.duplicate_store.add_if_new(&crate::qp_service::make_canonical_key(
        &qp_method,
        &endpoint_id,
        &req_content,
        &res_content,
        &hdr_content,
        &qp_data,
    ));

    // 4) files, then their rows
    let request_path = dir.join(format!("{endpoint_id}-request-{request_number}.json"));
    let response_path = dir.join(format!("{endpoint_id}-response-{request_number}.json"));
    let headers_path = dir.join(format!("{endpoint_id}-headers-{request_number}.json"));
    let written = async {
        tokio::fs::create_dir_all(&dir).await?;
        tokio::fs::write(&request_path, &req_content).await?;
        tokio::fs::write(&response_path, &res_content).await?;
        tokio::fs::write(&headers_path, &hdr_content).await
    }
    .await;
    if let Err(e) = written {
        remove_qp_files(&[&request_path, &response_path, &headers_path]).await;
        return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("failed to write the QP files: {e}")));
    }

    let status_code = result.status_code;
    let response_time_ms = result.response_time_ms;
    let rows = tokio::task::spawn_blocking({
        let st = state.clone();
        let eid = endpoint_id.clone();
        let rf = request_path.display().to_string();
        let resf = response_path.display().to_string();
        let m = run_method.clone();
        move || -> edms::error::EdmsResult<usize> {
            db::insert_request_metadata(&st.core, &st.queries, &eid, request_number, &rf, &m)?;
            db::insert_response_metadata(
                &st.core,
                &st.queries,
                &eid,
                request_number,
                &resf,
                Some(status_code),
                Some(response_time_ms),
            )?;
            let details = format!("{status_code} in {response_time_ms}ms");
            db::insert_history(&st.core, &st.queries, &eid, "test", Some(&details))?;
            db::history_count(&st.core, &st.queries)
        }
    })
    .await
    .map_err(internal)?;
    let history_count = match rows {
        Ok(count) => count,
        Err(e) => {
            remove_qp_files(&[&request_path, &response_path, &headers_path]).await;
            return Err(internal(e));
        }
    };

    // 5) tell the open views
    let _ = state.events_tx.send(ServerEvent::QpCreated {
        endpoint_id: endpoint_id.clone(),
        request_number,
    });
    state
        .emit(ServerEvent::HistoryUpdated { count: history_count })
        .await;

    Ok(json!({
        "ok": true,
        "saved": true,
        "endpoint_id": endpoint_id,
        "endpoint_created": endpoint_created,
        "request_number": request_number,
        "status_code": status_code,
        "response_time_ms": response_time_ms,
        "history_count": history_count,
    }))
}

async fn remove_qp_files(paths: &[&std::path::Path]) {
    for p in paths {
        let _ = tokio::fs::remove_file(p).await;
    }
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

/// Collection-scoped now, unlike the old version that read
/// `state.active_collection` — `collection` is always known (it's the
/// path param), so there's no more `Option`/`None` branch to handle.
async fn handle_ws_subscribe_bookmarks(mut socket: WebSocket, state: AppState, collection: String) {
    // Per Mathew (2026-09-08): bookmark view resolves full endpoint data,
    // not just EIDs — endpoints_for_ids already does that join. What's
    // here is also marking, per endpoint, whether it's currently a *saved
    // member* of this collection (vs. just bookmarked-but-not-yet-saved) —
    // computed by cross-referencing against the collection's own
    // membership set, never stored redundantly in the bookmarks table.
    let bookmarks = tokio::task::spawn_blocking({
        let st = state.clone();
        let collection = collection.clone();
        move || {
            let ids_with_timestamps =
                db::list_bookmarked_endpoints_with_timestamps(&st.core, &st.queries, &collection)?;
            let ids: Vec<String> = ids_with_timestamps.iter().map(|(id, _)| id.clone()).collect();
            let endpoints = db::endpoints_for_ids(&st.core, &st.queries, &ids)?;
            let updated_at: std::collections::HashMap<String, String> =
                ids_with_timestamps.into_iter().collect();

            let member_set = match crate::handlers::view_catalog::open_existing_collection_membership(&st, &collection)
                .and_then(|m| m.list_set().map_err(|e| format!("{e:?}")))
            {
                Ok(set) => set,
                Err(_) => std::collections::HashSet::new(),
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
            "collection": collection,
            "bookmarks": enriched
        });
        let _ = socket.send(Message::Text(resp.to_string())).await;
        let _ = state.events_tx.send(ServerEvent::ActiveWorkspaceBookmarksLoaded { count });
    }

    // Forward every event on the shared channel — same pattern every other
    // subscribe-style WS route in this codebase uses. BookmarksUpdated and
    // CollectionMembershipUpdated both carry `collection`, so the client
    // filters to events about this one and ignores the rest.
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

/// `bookmark` is now always a real collection name, taken directly from
/// the path — no more "active" alias to resolve, and no more gate to
/// check, since bookmarking into a named collection never requires
/// anything to be "loaded" first (2026-09-22).
async fn handle_ws_add_from_history(mut socket: WebSocket, state: AppState, bookmark: String) {
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
                state
                    .emit(ServerEvent::BookmarksUpdated { collection: bookmark.clone(), count })
                    .await;
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
                state
                    .emit(ServerEvent::BookmarksUpdated { collection: bookmark.clone(), count })
                    .await;
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
// A saved QP is read back through these routes: the client uses
// endpoint_id + request_number (returned by POST /test-view/save/test) to
// pull the real body. Deliberately NOT taking a raw path from the client —
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
// persisted in request_metadata/response_metadata + the three saved JSON
// files when a run is saved (see persist_run above). These two routes are the missing "access all of them" / "get
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

    let dir = state.endpoint_storage_dir(&endpoint_id);

    // Remove deleted QP's key from in-memory duplicate store so re-testing works immediately
    if let Some((q, p, h, qp_d)) = crate::qp_service::read_qp_quadruple(&dir, &endpoint_id, request_number) {
        let (method, endpoint_str) = {
            let st = state.clone();
            let eid = endpoint_id.clone();
            tokio::task::spawn_blocking(move || {
                crate::db::get_endpoint(&st.core, &st.queries, &eid)
                    .ok()
                    .flatten()
                    .map(|ep| (ep.method.unwrap_or_else(|| "GET".to_string()), ep.endpoint_str))
            })
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| ("GET".to_string(), String::new()))
        };
        let qp_data = if !qp_d.is_empty() { qp_d } else { crate::qp_service::extract_qp_data(&endpoint_str) };
        let key = crate::qp_service::make_canonical_key(&method, &endpoint_id, &q, &p, &h, &qp_data);
        state.duplicate_store.remove(&key);
    }

    // Delete QP files from disk
    for kind in ["request", "response", "headers"] {
        let path = dir.join(format!("{endpoint_id}-{kind}-{request_number}.json"));
        let _ = tokio::fs::remove_file(path).await;
    }

    // Await synchronous file renames on disk, then await DB renumbering
    let eid_clone = endpoint_id.clone();
    let dir_clone = dir.clone();
    let _ = tokio::task::spawn_blocking(move || {
        reassign_qp_files_inline(&dir_clone, &eid_clone);
    }).await;

    let st2 = state.clone();
    let eid2 = endpoint_id.clone();
    let storage_dir = dir.to_string_lossy().to_string();
    let _ = tokio::task::spawn_blocking(move || {
        crate::db::reassign_qp_numbers(&st2.core, &eid2, &storage_dir)
    }).await;

    state
        .emit(ServerEvent::QpDeleted {
            endpoint_id,
            request_number,
        })
        .await;

    (StatusCode::OK, Json(json!({ "ok": true })))
}

// ── QP manual create / update (no real HTTP test) ───────────────────
//
// Per Shivanshu's spec (2026-09-29): the frontend needs to create and edit
// QP pairs by hand, not just by running a real test. Both routes write the
// saved JSON files directly and skip ipc::spawn_child/run_test entirely —
// there's no real request to send and no real response to wait for.

/// Parses a QP body string as JSON, falling back to a plain JSON string if
/// it isn't valid JSON — matches how read_saved_file already renders a
/// saved file back to the caller, so a round-trip through update/create
/// reads back the same shape either way.
fn parse_qp_body(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_string()))
}

#[derive(Debug, Deserialize)]
pub struct UpdateQpRequest {
    pub request_body: String,
    pub response_body: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateQpRequest {
    pub method: String,
    pub request_body: String,
    pub response_body: String,
}

/// POST /test-view/{endpoint_id}/qps/{request_number}/update
///
/// Overwrites an existing QP's request/response files in place. Leaves
/// status_code/response_time_ms and the headers file untouched — the user
/// only edited the bodies, not the outcome of a test that already ran.
pub async fn update_qp(
    State(state): State<AppState>,
    Path((endpoint_id, request_number)): Path<(String, i32)>,
    Json(payload): Json<UpdateQpRequest>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = safe_id(&endpoint_id) {
        return e;
    }

    let exists = tokio::task::spawn_blocking({
        let st = state.clone();
        let eid = endpoint_id.clone();
        move || db::qp_exists(&st.core, &st.queries, &eid, request_number)
    })
    .await;

    match exists {
        Ok(Ok(true)) => {}
        Ok(Ok(false)) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "ok": false, "error": format!("no QP pair {endpoint_id}#{request_number}") })),
            )
        }
        Ok(Err(e)) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": format!("{e:?}") })),
            )
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": e.to_string() })),
            )
        }
    }

    let dir = state.endpoint_storage_dir(&endpoint_id);

    // Remove old snapshot key from duplicate store if it existed
    if let Some((q, p, h, qp_d)) = crate::qp_service::read_qp_quadruple(&dir, &endpoint_id, request_number) {
        let (method, endpoint_str) = {
            let st = state.clone();
            let eid = endpoint_id.clone();
            tokio::task::spawn_blocking(move || {
                crate::db::get_endpoint(&st.core, &st.queries, &eid)
                    .ok()
                    .flatten()
                    .map(|ep| (ep.method.unwrap_or_else(|| "GET".to_string()), ep.endpoint_str))
            })
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| ("GET".to_string(), String::new()))
        };
        let qp_data = if !qp_d.is_empty() { qp_d } else { crate::qp_service::extract_qp_data(&endpoint_str) };
        let old_key = crate::qp_service::make_canonical_key(&method, &endpoint_id, &q, &p, &h, &qp_data);
        state.duplicate_store.remove(&old_key);
    }

    let request_path = dir.join(format!("{endpoint_id}-request-{request_number}.json"));
    let response_path = dir.join(format!("{endpoint_id}-response-{request_number}.json"));

    let new_req_str = parse_qp_body(&payload.request_body).to_string();
    let new_res_str = parse_qp_body(&payload.response_body).to_string();

    if let Err(e) = tokio::fs::write(&request_path, &new_req_str).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": format!("failed to write request file: {e}") })),
        );
    }
    if let Err(e) = tokio::fs::write(&response_path, &new_res_str).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": format!("failed to write response file: {e}") })),
        );
    }

    // Register new snapshot key in duplicate store
    let (method, endpoint_str) = {
        let st = state.clone();
        let eid = endpoint_id.clone();
        tokio::task::spawn_blocking(move || {
            crate::db::get_endpoint(&st.core, &st.queries, &eid)
                .ok()
                .flatten()
                .map(|ep| (ep.method.unwrap_or_else(|| "GET".to_string()), ep.endpoint_str))
        })
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| ("GET".to_string(), String::new()))
    };
    let qp_data = crate::qp_service::extract_qp_data(&endpoint_str);
    let hdr_path = dir.join(format!("{endpoint_id}-headers-{request_number}.json"));
    let hdr_content = tokio::fs::read_to_string(&hdr_path).await.unwrap_or_else(|_| "{}".to_string());
    let new_key = crate::qp_service::make_canonical_key(&method, &endpoint_id, &new_req_str, &new_res_str, &hdr_content, &qp_data);
    state.duplicate_store.add_if_new(&new_key);

    state
        .emit(ServerEvent::QpUpdated {
            endpoint_id,
            request_number,
        })
        .await;

    (StatusCode::OK, Json(json!({ "ok": true })))
}

/// POST /test-view/{endpoint_id}/qps/create
///
/// Creates a brand-new QP without running a real HTTP test — the request
/// and response bodies come straight from the caller. status_code and
/// response_time_ms are left NULL (no real response happened), and a real
/// but empty headers file is written so GET .../headers/{n} doesn't 404
/// for a manually created pair.
pub async fn create_qp(
    State(state): State<AppState>,
    Path(endpoint_id): Path<String>,
    Json(payload): Json<CreateQpRequest>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = safe_id(&endpoint_id) {
        return e;
    }

    let endpoint = tokio::task::spawn_blocking({
        let st = state.clone();
        let eid = endpoint_id.clone();
        move || db::get_endpoint(&st.core, &st.queries, &eid)
    })
    .await;

    match endpoint {
        Ok(Ok(Some(_))) => {}
        Ok(Ok(None)) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "ok": false, "error": format!("no endpoint {endpoint_id}") })),
            )
        }
        Ok(Err(e)) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": format!("{e:?}") })),
            )
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": e.to_string() })),
            )
        }
    }

    let method = payload.method.to_uppercase();

    let request_number = tokio::task::spawn_blocking({
        let st = state.clone();
        let eid = endpoint_id.clone();
        move || db::get_next_request_number(&st.core, &st.queries, &eid)
    })
    .await;
    let request_number = match request_number {
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
                Json(json!({ "ok": false, "error": e.to_string() })),
            )
        }
    };

    let dir = state.endpoint_storage_dir(&endpoint_id);
    let req_content = parse_qp_body(&payload.request_body).to_string();
    let res_content = parse_qp_body(&payload.response_body).to_string();
    let headers_json = json!({ "request_headers": {}, "response_headers": {} });
    let hdr_content = headers_json.to_string();

    let (m, u) = crate::qp_service::extract_method_and_endpoint(&req_content);
    let method_to_use = if m.is_empty() { &method } else { &m };
    let qp_data = crate::qp_service::extract_qp_data(&u);
    let snapshot_key = crate::qp_service::make_canonical_key(
        method_to_use,
        &endpoint_id,
        &req_content,
        &res_content,
        &hdr_content,
        &qp_data,
    );

    let dup_opt = crate::qp_service::find_duplicate_qp(
        &dir,
        &endpoint_id,
        method_to_use,
        &u,
        &req_content,
        &res_content,
        &hdr_content,
        None,
    );

    if let Some(dup_num) = dup_opt {
        if dup_num > 0 {
            return (
                StatusCode::CONFLICT,
                Json(json!({
                    "ok": false,
                    "error": format!("Duplicate QP detected: an identical (Q, P, H) already exists as QP #{dup_num} for endpoint {endpoint_id}"),
                    "duplicate_of": dup_num,
                })),
            );
        }
    }
    state.duplicate_store.add_if_new(&snapshot_key);
    let request_path = dir.join(format!("{endpoint_id}-request-{request_number}.json"));
    let response_path = dir.join(format!("{endpoint_id}-response-{request_number}.json"));
    let headers_path = dir.join(format!("{endpoint_id}-headers-{request_number}.json"));

    if let Err(e) = tokio::fs::create_dir_all(&dir).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": format!("failed to create storage dir: {e}") })),
        );
    }
    if let Err(e) = tokio::fs::write(&request_path, parse_qp_body(&payload.request_body).to_string()).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": format!("failed to write request file: {e}") })),
        );
    }
    if let Err(e) = tokio::fs::write(&response_path, parse_qp_body(&payload.response_body).to_string()).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": format!("failed to write response file: {e}") })),
        );
    }
    let headers_json = json!({ "request_headers": {}, "response_headers": {} });
    if let Err(e) = tokio::fs::write(&headers_path, headers_json.to_string()).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": format!("failed to write headers file: {e}") })),
        );
    }

    let insert_result = tokio::task::spawn_blocking({
        let st = state.clone();
        let eid = endpoint_id.clone();
        let rf = request_path.display().to_string();
        let resf = response_path.display().to_string();
        let m = method.clone();
        move || -> edms::error::EdmsResult<()> {
            db::insert_request_metadata(&st.core, &st.queries, &eid, request_number, &rf, &m)?;
            db::insert_response_metadata(&st.core, &st.queries, &eid, request_number, &resf, None, None)?;
            Ok(())
        }
    })
    .await;

    match insert_result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": format!("{e:?}") })),
            )
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": e.to_string() })),
            )
        }
    }

    state
        .emit(ServerEvent::QpCreated {
            endpoint_id,
            request_number,
        })
        .await;

    (StatusCode::OK, Json(json!({ "ok": true, "request_number": request_number })))
}
fn reassign_qp_files_inline(folder: &std::path::Path, eid: &str) {
    let prefix = format!("{}-request-", eid);
    let suffix = ".json";
    let mut numbers: Vec<usize> = Vec::new();

    let entries = match std::fs::read_dir(folder) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(prefix.as_str()) && name.ends_with(suffix) {
            let mid = &name[prefix.len()..name.len() - suffix.len()];
            if let Ok(n) = mid.parse::<usize>() {
                numbers.push(n);
            }
        }
    }

    numbers.sort_unstable();

    for (zero_idx, old_n) in numbers.into_iter().enumerate() {
        let new_n = zero_idx + 1;
        if old_n == new_n {
            continue;
        }
        for kind in ["request", "response", "headers"] {
            let old_path = folder.join(format!("{}-{}-{}.json", eid, kind, old_n));
            let new_path = folder.join(format!("{}-{}-{}.json", eid, kind, new_n));
            if old_path.exists() {
                let _ = std::fs::rename(&old_path, &new_path);
            }
        }
    }
}
