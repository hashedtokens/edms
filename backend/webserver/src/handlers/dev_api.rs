//! The dev API: backend developers push one QP for one endpoint into EDMS
//! instead of using the UI.
//!
//! - `POST /hashedtokens/edms/api/history/`     `ops: "test"`
//! - `POST /hashedtokens/edms/api/collections/` `ops: "test_and_insert" | "insert"`
//!
//! The route is write-only. It checks the body, hands the work to a compute
//! op in `edms-child` and answers `202 {transaction_id}` at once. The final
//! code (see `compute::api_status`, `pkg/status_codes.md`) is delivered over
//! `GET /hashedtokens/edms/api/ws` as a `DevApiResult` event, and the open UI
//! views refresh from the events the ops already send. A call whose child
//! never reports back is closed with `COMPUTE_STALLED`.
//!
//! The compute ops are Hemang's (history) and Manorath's (collections). They
//! receive the task, the validated body and `job_id` (the transaction id)
//! as the payload, and report back to `/internal/callback` with
//! `{job_id, status, ...}` - an `ApiResult` plus the echoed `job_id`.
//!
//! Which params are mandatory is provisional until the Notion schema is
//! confirmed; `validate` is the one place to change.

use axum::{
    body::Bytes,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use compute::api_status::{ApiResult, ApiStatus};
use serde_json::{json, Map, Value};
use std::time::Duration;
use tokio::sync::broadcast;

use crate::{
    events::ServerEvent,
    handlers::jobs::job_id_of,
    ipc::IpcCallback,
    jobs::{Job, JobStatus, JobTable},
    state::AppState,
};

/// A transaction whose child hasn't reported back after this long is closed
/// with `COMPUTE_STALLED`. Longer than a caller's own wait (about 15 s), so a
/// slow but live op still delivers its result.
pub const STALL_AFTER: Duration = Duration::from_secs(30);

const METHODS: [&str; 7] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];
const MAX_TIMEOUT_MS: u64 = 120_000;
/// Job ops for this API all start with this, which is how a failed child's
/// callback is told apart from an Import/Export job's.
const OP_PREFIX: &str = "api_";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    History,
    Collections,
}

impl Route {
    fn name(self) -> &'static str {
        match self {
            Route::History => "history",
            Route::Collections => "collections",
        }
    }

    fn ops(self) -> &'static [&'static str] {
        match self {
            Route::History => &["test"],
            Route::Collections => &["test_and_insert", "insert"],
        }
    }
}

/// The compute task for a route and op, e.g. `api_collections_insert`.
pub fn task_name(route: Route, op: &str) -> String {
    format!("{OP_PREFIX}{}_{op}", route.name())
}

#[derive(Debug, PartialEq)]
pub struct ApiError {
    pub status: ApiStatus,
    pub detail: String,
}

fn err(status: ApiStatus, detail: impl Into<String>) -> ApiError {
    ApiError { status, detail: detail.into() }
}

/// A body that passed `validate`: the op and the body to hand to compute,
/// with the method upper-cased and the strings trimmed.
#[derive(Debug)]
pub struct Validated {
    pub op: String,
    pub payload: Map<String, Value>,
}

/// An optional string param: absent is fine, present must be a string.
fn opt_str<'a>(obj: &'a Map<String, Value>, key: &str) -> Result<Option<&'a str>, ApiError> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.as_str())),
        Some(_) => Err(err(ApiStatus::InvalidParam, format!("'{key}' must be a string"))),
    }
}

fn require_str<'a>(obj: &'a Map<String, Value>, key: &str) -> Result<&'a str, ApiError> {
    match opt_str(obj, key)?.map(str::trim) {
        Some(s) if !s.is_empty() => Ok(s),
        _ => Err(err(ApiStatus::MissingParam, format!("'{key}' is required"))),
    }
}

/// Headers are an object of string values.
fn check_headers(obj: &Map<String, Value>, key: &str) -> Result<(), ApiError> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(()),
        Some(Value::Object(m)) if m.values().all(Value::is_string) => Ok(()),
        Some(_) => Err(err(ApiStatus::InvalidParam, format!("'{key}' must be an object of strings"))),
    }
}

pub fn validate(route: Route, raw: &[u8]) -> Result<Validated, ApiError> {
    let body: Value = serde_json::from_slice(raw)
        .map_err(|e| err(ApiStatus::InvalidRequest, format!("the body is not valid JSON: {e}")))?;
    let Value::Object(mut obj) = body else {
        return Err(err(ApiStatus::InvalidRequest, "the body must be a JSON object"));
    };

    let op = require_str(&obj, "ops")?.to_string();
    if !route.ops().contains(&op.as_str()) {
        return Err(err(
            ApiStatus::UnknownOp,
            format!("'{op}' is not an op of /{}/ (use {})", route.name(), route.ops().join(" or ")),
        ));
    }
    let tests = op != "insert";

    let endpoint_str = require_str(&obj, "endpoint_str")?.to_string();
    if tests && !(endpoint_str.starts_with("http://") || endpoint_str.starts_with("https://")) {
        return Err(err(ApiStatus::InvalidParam, "'endpoint_str' must be an http(s) URL to test it"));
    }

    let method = match (opt_str(&obj, "method")?.map(str::trim), tests) {
        (Some(m), _) if !m.is_empty() => Some(m.to_uppercase()),
        (_, true) => return Err(err(ApiStatus::MissingParam, "'method' is required to test an endpoint")),
        _ => None,
    };
    if let Some(m) = &method {
        if !METHODS.contains(&m.as_str()) {
            return Err(err(ApiStatus::InvalidParam, format!("'{m}' is not a method ({})", METHODS.join(", "))));
        }
    }

    let collection = if route == Route::Collections {
        let name = require_str(&obj, "collection")?.to_string();
        crate::handlers::collection_ie::validate_collection_name(&name)
            .map_err(|e| err(ApiStatus::InvalidParam, e))?;
        Some(name)
    } else {
        None
    };

    if !tests {
        for key in ["request_body", "response_body"] {
            if !obj.contains_key(key) {
                return Err(err(ApiStatus::MissingParam, format!("'{key}' is required to insert")));
            }
        }
    }
    for key in ["headers", "request_headers", "response_headers"] {
        check_headers(&obj, key)?;
    }
    if let Some(v) = obj.get("timeout_ms").filter(|v| !v.is_null()) {
        match v.as_u64() {
            Some(ms) if (1..=MAX_TIMEOUT_MS).contains(&ms) => {}
            _ => {
                return Err(err(
                    ApiStatus::InvalidParam,
                    format!("'timeout_ms' must be a whole number from 1 to {MAX_TIMEOUT_MS}"),
                ))
            }
        }
    }
    for key in ["annotation", "collection_annotation", "notes", "endpoint_id"] {
        opt_str(&obj, key)?;
    }

    obj.insert("ops".into(), json!(op));
    obj.insert("endpoint_str".into(), json!(endpoint_str));
    if let Some(m) = method {
        obj.insert("method".into(), json!(m));
    }
    if let Some(c) = collection {
        obj.insert("collection".into(), json!(c));
    }
    Ok(Validated { op, payload: obj })
}

// ── Routes ───────────────────────────────────────────────────────────────

/// POST /hashedtokens/edms/api/history/
pub async fn api_history(State(state): State<AppState>, body: Bytes) -> (StatusCode, Json<Value>) {
    handle(&state, Route::History, &body)
}

/// POST /hashedtokens/edms/api/collections/
pub async fn api_collections(State(state): State<AppState>, body: Bytes) -> (StatusCode, Json<Value>) {
    handle(&state, Route::Collections, &body)
}

fn rejection(e: ApiError) -> (StatusCode, Json<Value>) {
    let http = e.status.http_status().and_then(|c| StatusCode::from_u16(c).ok());
    (
        http.unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        Json(json!({ "ok": false, "status": e.status, "detail": e.detail })),
    )
}

fn handle(state: &AppState, route: Route, raw: &[u8]) -> (StatusCode, Json<Value>) {
    let Validated { op, mut payload } = match validate(route, raw) {
        Ok(v) => v,
        Err(e) => return rejection(e),
    };

    let task = task_name(route, &op);
    let endpoint_str = payload.get("endpoint_str").and_then(Value::as_str).map(str::to_string);
    let job_id = state.jobs.create(&task, None, endpoint_str.as_deref());
    payload.insert("job_id".into(), json!(job_id));

    if !crate::ipc::spawn_child(&task, Value::Object(payload), 3000) {
        let _ = state.jobs.fail(&job_id, "couldn't start the compute process".to_string());
        return rejection(err(
            ApiStatus::ComputeUnavailable,
            "couldn't start the compute process (is edms-child built?)",
        ));
    }

    let jobs = state.jobs.clone();
    let events = state.events_tx.clone();
    let id = job_id.clone();
    tokio::spawn(async move {
        close_if_stalled(&jobs, &events, &id, STALL_AFTER).await;
    });

    (
        StatusCode::ACCEPTED,
        Json(json!({ "ok": true, "transaction_id": job_id, "ops": op })),
    )
}

// ── Delivering the result ────────────────────────────────────────────────

/// Closes a transaction with its final result and tells the WebSocket. A
/// transaction that already finished is left alone (a late or duplicate
/// callback can't rewrite it) and nothing is sent.
pub fn deliver(jobs: &JobTable, events: &broadcast::Sender<ServerEvent>, job_id: &str, result: ApiResult) {
    let value = serde_json::to_value(&result).unwrap_or(Value::Null);
    if let Some(job) = jobs.complete(job_id, value.clone()) {
        let _ = events.send(ServerEvent::DevApiResult {
            transaction_id: job.id,
            op: job.op,
            result: value,
        });
    }
}

fn failure(status: ApiStatus, detail: impl Into<String>) -> ApiResult {
    let mut r = ApiResult::new(status);
    r.detail = Some(detail.into());
    r
}

/// The compute op finished: its result is an `ApiResult` (plus `job_id`).
pub fn finish_from_callback(jobs: &JobTable, events: &broadcast::Sender<ServerEvent>, callback: &IpcCallback) {
    let Some(id) = job_id_of(callback) else { return };
    let result = serde_json::from_value::<ApiResult>(callback.result.clone())
        .unwrap_or_else(|e| failure(ApiStatus::Unknown, format!("compute returned a result that isn't an ApiResult: {e}")));
    deliver(jobs, events, &id, result);
}

/// The child reported a failure. Returns whether it belonged to a dev API
/// transaction (and was closed with `INTERNAL_ERROR`), so the caller can skip
/// the Import/Export job handling.
pub fn fail_from_callback(jobs: &JobTable, events: &broadcast::Sender<ServerEvent>, callback: &IpcCallback) -> bool {
    let Some(id) = job_id_of(callback) else { return false };
    match jobs.get(&id) {
        Some(job) if job.op.starts_with(OP_PREFIX) => {
            let detail = callback.error.clone().unwrap_or_else(|| "unknown error".to_string());
            deliver(jobs, events, &id, failure(ApiStatus::InternalError, detail));
            true
        }
        _ => false,
    }
}

/// Waits, then closes the transaction with `COMPUTE_STALLED` if its child
/// still hasn't reported back.
pub async fn close_if_stalled(
    jobs: &JobTable,
    events: &broadcast::Sender<ServerEvent>,
    job_id: &str,
    after: Duration,
) {
    tokio::time::sleep(after).await;
    deliver(
        jobs,
        events,
        job_id,
        failure(ApiStatus::ComputeStalled, "the compute process never reported back"),
    );
}

// ── WebSocket ────────────────────────────────────────────────────────────

/// GET /hashedtokens/edms/api/ws - the caller keeps this open to receive the
/// final code. On connect, a `{"type":"transactions","transactions":[...]}`
/// snapshot of the dev API transactions still running, then only the
/// `DevApiResult` events as `{"type":"event","event":...}`. A transaction
/// that finished before the caller connected is read with `GET /jobs/:id`.
pub async fn ws_api(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_api(socket, state))
}

async fn handle_ws_api(mut socket: WebSocket, state: AppState) {
    // Subscribe before taking the snapshot, so an event between the two
    // isn't lost.
    let mut rx = state.events_tx.subscribe();

    let running: Vec<Value> = state
        .jobs
        .list()
        .iter()
        .filter(|j| j.status == JobStatus::Running && j.op.starts_with(OP_PREFIX))
        .map(Job::to_json)
        .collect();
    let snapshot = json!({ "type": "transactions", "transactions": running });
    if socket.send(Message::Text(snapshot.to_string())).await.is_err() {
        return;
    }

    loop {
        match rx.recv().await {
            Ok(evt @ ServerEvent::DevApiResult { .. }) => {
                let msg = json!({ "type": "event", "event": evt });
                if socket.send(Message::Text(msg.to_string())).await.is_err() {
                    break;
                }
            }
            Ok(_) => {}
            Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(r: Result<Validated, ApiError>) -> i16 {
        r.err().expect("expected a rejection").status.code()
    }

    fn history(body: Value) -> Result<Validated, ApiError> {
        validate(Route::History, body.to_string().as_bytes())
    }

    fn collections(body: Value) -> Result<Validated, ApiError> {
        validate(Route::Collections, body.to_string().as_bytes())
    }

    #[test]
    fn a_history_test_needs_ops_url_and_method() {
        let ok = history(json!({"ops": "test", "endpoint_str": " https://a.io/x ", "method": "get"})).unwrap();
        assert_eq!(ok.op, "test");
        assert_eq!(ok.payload["method"], "GET");
        assert_eq!(ok.payload["endpoint_str"], "https://a.io/x");

        assert_eq!(code(history(json!({"endpoint_str": "https://a.io", "method": "GET"}))), 101);
        assert_eq!(code(history(json!({"ops": "test", "method": "GET"}))), 101);
        assert_eq!(code(history(json!({"ops": "test", "endpoint_str": "https://a.io"}))), 101);
    }

    #[test]
    fn a_route_only_takes_its_own_ops() {
        assert_eq!(code(history(json!({"ops": "insert", "endpoint_str": "https://a.io", "method": "GET"}))), 102);
        assert_eq!(code(history(json!({"ops": "nope", "endpoint_str": "https://a.io", "method": "GET"}))), 102);
        let body = json!({"ops": "test", "endpoint_str": "https://a.io", "method": "GET", "collection": "c"});
        assert_eq!(code(collections(body)), 102);
    }

    #[test]
    fn a_bad_body_is_an_invalid_request() {
        assert_eq!(code(validate(Route::History, b"not json")), 100);
        assert_eq!(code(validate(Route::History, b"[1,2]")), 100);
        assert_eq!(code(validate(Route::History, b"")), 100);
    }

    #[test]
    fn values_that_are_not_allowed_are_invalid_params() {
        let base = |extra: Value| {
            let mut b = json!({"ops": "test", "endpoint_str": "https://a.io", "method": "GET"});
            b.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            history(b)
        };
        assert_eq!(code(history(json!({"ops": "test", "endpoint_str": "ftp://a.io", "method": "GET"}))), 103);
        assert_eq!(code(history(json!({"ops": "test", "endpoint_str": "https://a.io", "method": "FETCH"}))), 103);
        assert_eq!(code(history(json!({"ops": "test", "endpoint_str": 5, "method": "GET"}))), 103);
        assert_eq!(code(base(json!({"headers": {"a": 1}}))), 103);
        assert_eq!(code(base(json!({"headers": "x"}))), 103);
        assert_eq!(code(base(json!({"timeout_ms": 0}))), 103);
        assert_eq!(code(base(json!({"timeout_ms": 999_999}))), 103);
        assert_eq!(code(base(json!({"timeout_ms": "5"}))), 103);
        assert!(base(json!({"timeout_ms": 5000, "headers": {"a": "b"}, "annotation": "hi"})).is_ok());
    }

    #[test]
    fn collections_need_a_valid_collection_name() {
        let test_and_insert = |c: Value| {
            collections(json!({"ops": "test_and_insert", "endpoint_str": "https://a.io", "method": "GET", "collection": c}))
        };
        assert!(test_and_insert(json!("my-api")).is_ok());
        assert_eq!(code(collections(json!({"ops": "test_and_insert", "endpoint_str": "https://a.io", "method": "GET"}))), 101);
        assert_eq!(code(test_and_insert(json!(""))), 101);
        assert_eq!(code(test_and_insert(json!("list"))), 103);
        assert_eq!(code(test_and_insert(json!("a/b"))), 103);
    }

    #[test]
    fn an_insert_carries_the_data_and_needs_no_method() {
        let ok = collections(json!({
            "ops": "insert", "endpoint_str": "/v1/users", "collection": "c",
            "request_body": {"id": 1}, "response_body": {"ok": true},
            "request_headers": {"a": "b"}, "response_headers": {"c": "d"}
        }))
        .unwrap();
        assert_eq!(ok.op, "insert");
        assert!(ok.payload.get("method").is_none());

        let no_response = json!({"ops": "insert", "endpoint_str": "/v1/users", "collection": "c", "request_body": {}});
        assert_eq!(code(collections(no_response)), 101);
        let bad_headers = json!({
            "ops": "insert", "endpoint_str": "/v1/users", "collection": "c",
            "request_body": {}, "response_body": {}, "response_headers": [1]
        });
        assert_eq!(code(collections(bad_headers)), 103);
    }

    #[test]
    fn task_names_follow_the_route_and_op() {
        assert_eq!(task_name(Route::History, "test"), "api_history_test");
        assert_eq!(task_name(Route::Collections, "insert"), "api_collections_insert");
    }

    #[test]
    fn a_rejection_uses_the_http_status_of_its_code() {
        let (status, Json(body)) = rejection(err(ApiStatus::MissingParam, "x"));
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["status"]["code"], 101);
        assert_eq!(body["ok"], false);
        let (status, _) = rejection(err(ApiStatus::ComputeUnavailable, "x"));
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    fn setup() -> (JobTable, broadcast::Sender<ServerEvent>, broadcast::Receiver<ServerEvent>, String) {
        let jobs = JobTable::new();
        let (tx, rx) = broadcast::channel(16);
        let id = jobs.create("api_history_test", None, Some("https://a.io"));
        (jobs, tx, rx, id)
    }

    fn callback(result: Value, success: bool, error: Option<&str>) -> IpcCallback {
        IpcCallback {
            task: "api_history_test".into(),
            result,
            elapsed_ms: 1,
            success,
            error: error.map(str::to_string),
        }
    }

    fn event_result(rx: &mut broadcast::Receiver<ServerEvent>) -> Option<(String, Value)> {
        match rx.try_recv() {
            Ok(ServerEvent::DevApiResult { transaction_id, result, .. }) => Some((transaction_id, result)),
            _ => None,
        }
    }

    #[test]
    fn a_compute_result_closes_the_transaction_and_is_sent_once() {
        let (jobs, tx, mut rx, id) = setup();
        let cb = callback(json!({"job_id": id, "status": 0, "endpoint_id": "EID-1", "request_number": 2}), true, None);
        finish_from_callback(&jobs, &tx, &cb);

        let (txn, result) = event_result(&mut rx).expect("an event");
        assert_eq!(txn, id);
        assert_eq!(result["status"]["code"], 0);
        assert_eq!(result["request_number"], 2);
        assert_eq!(jobs.get(&id).unwrap().status, JobStatus::Done);

        // a duplicate callback changes nothing and sends nothing
        let again = callback(json!({"job_id": id, "status": 105}), true, None);
        finish_from_callback(&jobs, &tx, &again);
        assert!(event_result(&mut rx).is_none());
        assert_eq!(jobs.get(&id).unwrap().result.unwrap()["status"]["code"], 0);
    }

    #[test]
    fn a_result_that_is_not_an_api_result_is_unknown() {
        let (jobs, tx, mut rx, id) = setup();
        finish_from_callback(&jobs, &tx, &callback(json!({"job_id": id, "hello": 1}), true, None));
        let (_, result) = event_result(&mut rx).unwrap();
        assert_eq!(result["status"]["code"], -1);
    }

    #[test]
    fn a_failed_child_closes_an_api_transaction_with_internal_error() {
        let (jobs, tx, mut rx, id) = setup();
        let cb = callback(json!({"job_id": id}), false, Some("disk full"));
        assert!(fail_from_callback(&jobs, &tx, &cb));
        let (_, result) = event_result(&mut rx).unwrap();
        assert_eq!(result["status"]["code"], 109);
        assert_eq!(result["detail"], "disk full");
    }

    #[test]
    fn a_failed_import_export_job_is_left_to_its_own_handler() {
        let jobs = JobTable::new();
        let (tx, mut rx) = broadcast::channel(16);
        let id = jobs.create("takeout", Some("repoview"), Some("rv"));
        let cb = callback(json!({"job_id": id}), false, Some("boom"));
        assert!(!fail_from_callback(&jobs, &tx, &cb));
        assert!(event_result(&mut rx).is_none());
        assert_eq!(jobs.get(&id).unwrap().status, JobStatus::Running);
        // no job id at all
        assert!(!fail_from_callback(&jobs, &tx, &callback(Value::Null, false, Some("x"))));
    }

    #[tokio::test]
    async fn a_silent_child_is_closed_as_stalled() {
        let (jobs, tx, mut rx, id) = setup();
        close_if_stalled(&jobs, &tx, &id, Duration::from_millis(20)).await;
        let (_, result) = event_result(&mut rx).unwrap();
        assert_eq!(result["status"]["code"], 108);
    }

    #[tokio::test]
    async fn the_stall_timer_does_not_overwrite_a_finished_transaction() {
        let (jobs, tx, mut rx, id) = setup();
        deliver(&jobs, &tx, &id, ApiResult::new(ApiStatus::Ok));
        let _ = event_result(&mut rx);
        close_if_stalled(&jobs, &tx, &id, Duration::from_millis(10)).await;
        assert!(event_result(&mut rx).is_none());
        assert_eq!(jobs.get(&id).unwrap().result.unwrap()["status"]["code"], 0);
    }
}
