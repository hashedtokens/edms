//! Routes and callback plumbing for long-running Import/Export jobs (see
//! `jobs.rs`): look a job up, list recent ones, and a WebSocket that streams
//! only job events.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::{json, Value};

use crate::{
    events::ServerEvent,
    ipc::IpcCallback,
    jobs::{Job, JobStatus},
    state::AppState,
};

/// GET /jobs - recent jobs, newest first (running ones included).
pub async fn list_jobs(State(state): State<AppState>) -> Json<Value> {
    let jobs: Vec<Value> = state.jobs.list().iter().map(Job::to_json).collect();
    Json(json!({ "ok": true, "jobs": jobs }))
}

/// GET /jobs/:id - one job: `status` is `running`, `done`, `failed` or
/// `stalled` (running far too long, so the child most likely died), with its
/// `result` / `error` and latest `progress`.
pub async fn get_job(State(state): State<AppState>, Path(id): Path<String>) -> (StatusCode, Json<Value>) {
    match state.jobs.get(&id) {
        Some(job) => {
            let mut body = job.to_json();
            body["ok"] = json!(true);
            (StatusCode::OK, Json(body))
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "ok": false, "error": format!("job '{id}' not found (the job list is kept in memory and resets on restart)") })),
        ),
    }
}

/// GET /jobs/ws - on connect, a `{"type":"jobs","jobs":[...running...]}`
/// snapshot, then only the job events (`ViewIoProgress`, `ViewIoDone`) as
/// they happen, in the same `{"type":"event","event":...}` shape as the other
/// sockets. A page that opens this after starting a job still learns about
/// it from the snapshot, then from `GET /jobs/:id` if it already finished.
pub async fn ws_jobs(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_jobs(socket, state))
}

async fn handle_ws_jobs(mut socket: WebSocket, state: AppState) {
    // Subscribe before taking the snapshot, so an event between the two
    // isn't lost.
    let mut rx = state.events_tx.subscribe();

    let running: Vec<Value> = state
        .jobs
        .list()
        .iter()
        .filter(|j| j.status == JobStatus::Running)
        .map(Job::to_json)
        .collect();
    let snapshot = json!({ "type": "jobs", "jobs": running });
    if socket.send(Message::Text(snapshot.to_string())).await.is_err() {
        return;
    }

    loop {
        match rx.recv().await {
            Ok(evt @ (ServerEvent::ViewIoProgress { .. } | ServerEvent::ViewIoDone { .. })) => {
                let msg = json!({ "type": "event", "event": evt });
                if socket.send(Message::Text(msg.to_string())).await.is_err() {
                    break;
                }
            }
            Ok(_) => {}
            // Fell behind the broadcast buffer: keep going, the client can
            // GET /jobs/:id for anything it missed.
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
}

// ── Callback plumbing ────────────────────────────────────────────────────

/// A child echoes the job id it was given in `result.job_id` (also on
/// failure - see `edms-child`), which is how its callback finds the job.
pub fn job_id_of(callback: &IpcCallback) -> Option<String> {
    callback.result.get("job_id").and_then(Value::as_str).map(str::to_string)
}

pub fn emit_done(state: &AppState, job: &Job) {
    let _ = state.events_tx.send(ServerEvent::ViewIoDone {
        job_id: job.id.clone(),
        op: job.op.clone(),
        view: job.view.clone(),
        name: job.name.clone(),
        ok: job.status == JobStatus::Done,
        result: job.result.clone(),
        error: job.error.clone(),
    });
}

/// The child reported a failure: fail its job (if it has one) and tell the
/// UI. A callback with no job id is some other task and is left alone.
pub fn fail_from_callback(state: &AppState, callback: &IpcCallback) {
    let Some(id) = job_id_of(callback) else { return };
    let error = callback.error.clone().unwrap_or_else(|| "unknown error".to_string());
    if let Some(job) = state.jobs.fail(&id, error) {
        emit_done(state, &job);
    }
}

/// A child's mid-task progress report: `result = {job_id, progress}`.
pub fn progress_from_callback(state: &AppState, callback: &IpcCallback) {
    let Some(id) = job_id_of(callback) else { return };
    let progress = callback.result.get("progress").cloned().unwrap_or(Value::Null);
    if let Some(job) = state.jobs.set_progress(&id, progress.clone()) {
        let _ = state.events_tx.send(ServerEvent::ViewIoProgress {
            job_id: job.id,
            op: job.op,
            view: job.view,
            name: job.name,
            progress,
        });
    }
}

/// A child-only job (takeout, unzip, ...) whose whole result is the child's
/// result: complete it and tell the UI. Jobs with more steps on the
/// webserver side (import) finish themselves instead.
pub fn complete_from_callback(state: &AppState, callback: &IpcCallback) {
    let Some(id) = job_id_of(callback) else { return };
    if let Some(job) = state.jobs.complete(&id, callback.result.clone()) {
        emit_done(state, &job);
    }
}
