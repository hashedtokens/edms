//! Collections Developer API operations (compute side).
//!
//! The compute logic behind `POST /hashedtokens/edms/api/collections/`: the
//! `insert` and `test_and_insert` ops. The webserver validates the body and
//! starts the `api_collections_insert` / `api_collections_test_and_insert`
//! task in `edms-child`; this module does the work and returns an
//! [`ApiResult`] (see `pkg/status_codes.md`).
//!
//! What a successful call leaves behind is what a Run in Test View plus a
//! Bookmark and a Save leave behind:
//!
//! - the collection, created empty (file, catalog row, annotation) if it is new;
//! - the endpoint, found by URL + method or created with a fresh EID;
//! - one QP (request, response, headers files and metadata rows), unless an
//!   identical one already exists (`DUPLICATE_QP`, nothing is stored);
//! - for `test_and_insert`, a History row, like any completed Run;
//! - the endpoint's bookmark and its membership in the collection, and the
//!   collection's stored size.
//!
//! A response of any status counts: a 404 from the tested API is a completed
//! test, stored like a 200. Only "no answer" (timeout, unreachable) is a
//! failure, and then nothing is created.
//!
//! Where things are stored is behind [`StorageBackend`], so the flow can be
//! tested without a database; `collections_store::CollectionStore` is the
//! real one.

use crate::api_status::{ApiResult, ApiStatus};
use crate::qp_store::Stored;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Default)]
pub struct CollectionOpRequest {
    /// Operation to execute: "insert" or "test_and_insert"
    pub ops: String,
    /// Target endpoint URL string
    pub endpoint_str: String,
    /// Target collection name (a missing collection is created)
    pub collection: String,
    /// HTTP method (mandatory for "test_and_insert"; an "insert" without one
    /// uses GET, the database default)
    #[serde(default)]
    pub method: Option<String>,
    /// Request payload / body
    #[serde(default)]
    pub request_body: Option<Value>,
    /// Pre-recorded response payload / body (mandatory for "insert")
    #[serde(default)]
    pub response_body: Option<Value>,
    /// Headers to send with the test request
    #[serde(default)]
    pub headers: Option<Value>,
    /// Headers of the recorded request (insert)
    #[serde(default)]
    pub request_headers: Option<Value>,
    /// Headers of the recorded response (insert)
    #[serde(default)]
    pub response_headers: Option<Value>,
    /// Optional timeout in milliseconds
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Optional annotation for the endpoint (used only when it is created)
    #[serde(default)]
    pub annotation: Option<String>,
    /// Parsed from the nested spec schema; not stored yet.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// Optional annotation for the collection (used only when it is created)
    #[serde(default)]
    pub collection_annotation: Option<String>,
    /// Parsed from the nested spec schema; not stored yet.
    #[serde(default)]
    pub collection_tags: Option<Vec<String>>,
    /// Asks for the tested API's full response in the result. Parsed, but the
    /// webserver does not deliver it yet (open question).
    #[serde(default)]
    pub return_response: Option<bool>,
    /// Notes on the bookmark
    #[serde(default)]
    pub notes: Option<String>,
    /// An existing endpoint to use, or a canonical EID to create it with
    #[serde(default)]
    pub endpoint_id: Option<String>,
}

// Flexible deserializer: supports both the nested spec schema and the flat
// schema the webserver validates.
impl<'de> Deserialize<'de> for CollectionOpRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = Value::deserialize(deserializer)?;

        let s = |x: Option<&Value>| x.and_then(|y| y.as_str()).map(str::to_string);

        // 1. Resolve ops ("insert", "test_and_insert")
        let ops = if let Some(ops_obj) = v.get("ops").and_then(|o| o.as_object()) {
            ops_obj.get("type").and_then(|t| t.as_str()).unwrap_or("").to_string()
        } else {
            v.get("ops").and_then(|o| o.as_str()).unwrap_or("").to_string()
        };

        // 2. Resolve collection, collection_annotation, collection_tags
        let (collection, collection_annotation, collection_tags) =
            if let Some(col_obj) = v.get("collection").and_then(|c| c.as_object()) {
                let name = col_obj.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                let annot = s(col_obj.get("annotation"));
                let tags = col_obj.get("tags").and_then(|t| serde_json::from_value(t.clone()).ok());
                (name, annot, tags)
            } else {
                let name = v.get("collection").and_then(|c| c.as_str()).unwrap_or("").to_string();
                let annot = s(v.get("collection_annotation"));
                let tags = v.get("collection_tags").and_then(|t| serde_json::from_value(t.clone()).ok());
                (name, annot, tags)
            };

        // 3. Resolve endpoint, method/crud, headers, request, response, annotation, tags
        let src = v.get("endpoint").and_then(|e| e.as_object()).map(|o| Value::Object(o.clone()));
        let nested = src.is_some();
        let ep = src.as_ref().unwrap_or(&v);
        let endpoint_str = if nested {
            s(ep.get("name").or_else(|| ep.get("endpoint_str")))
        } else {
            s(ep.get("endpoint_str").or_else(|| ep.get("endpoint")))
        }
        .unwrap_or_default();
        let method = if nested {
            s(ep.get("crud").or_else(|| ep.get("method")))
        } else {
            s(ep.get("method").or_else(|| ep.get("crud")))
        };
        let headers = ep.get("headers").cloned();
        let request_body = ep.get("request").or_else(|| ep.get("request_body")).cloned();
        let response_body = ep.get("response").or_else(|| ep.get("response_body")).cloned();
        let request_headers = ep.get("request_headers").cloned();
        let response_headers = ep.get("response_headers").cloned();
        let annotation = s(ep.get("annotation"));
        let tags = ep.get("tags").and_then(|t| serde_json::from_value(t.clone()).ok());
        let notes = s(ep.get("notes").or_else(|| v.get("notes")));
        let endpoint_id = s(ep.get("endpoint_id").or_else(|| v.get("endpoint_id")));

        // 4. Timeout and return_response flags
        let timeout_ms = v.get("timeout_ms").and_then(|t| t.as_u64());
        let return_response = v.get("return_response").and_then(|r| r.as_bool());

        Ok(CollectionOpRequest {
            ops,
            endpoint_str,
            collection,
            method,
            request_body,
            response_body,
            headers,
            request_headers,
            response_headers,
            timeout_ms,
            annotation,
            tags,
            collection_annotation,
            collection_tags,
            return_response,
            notes,
            endpoint_id,
        })
    }
}

/// The tested API's answer, whatever its status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EndpointResponse {
    pub status: u16,
    /// The response's own headers (an object of strings)
    pub headers: Value,
    /// The body: parsed JSON when it is JSON, otherwise a string
    pub body: Value,
    pub time_ms: i64,
}

/// What the test request needs.
#[derive(Debug, Clone)]
pub struct HttpCall {
    pub url: String,
    pub method: String,
    pub body: Option<Value>,
    pub headers: Option<Value>,
    pub timeout_ms: Option<u64>,
}

/// The tested API gave no answer.
#[derive(Debug, Clone, PartialEq)]
pub enum HttpFailure {
    TimedOut(String),
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CollectionOpResponse {
    pub result: ApiResult,
    /// The collection's name, once it exists
    pub collection_id: Option<String>,
    /// Full endpoint response when `return_response` is enabled
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<EndpointResponse>,
}

/// A QP, as the storage stores it.
pub struct QpToStore<'a> {
    pub endpoint_id: &'a str,
    /// The endpoint's URL, for the duplicate check's fingerprint
    pub endpoint_str: &'a str,
    pub method: &'a str,
    pub request: &'a str,
    pub response: &'a str,
    pub headers: &'a str,
    pub status_code: Option<i32>,
    pub response_time_ms: Option<i32>,
}

/// Where the op keeps things. Every method is one step of the flow above.
pub trait StorageBackend {
    /// Makes sure the collection exists, creating it empty if it doesn't.
    /// `true` if this call created it (of several calls creating the same
    /// collection at once, exactly one gets `true`).
    fn ensure_collection(&mut self, name: &str, annotation: Option<&str>) -> Result<bool, String>;
    /// `(endpoint_id, created)`
    fn find_or_create_endpoint(
        &mut self,
        endpoint_id: Option<&str>,
        endpoint_str: &str,
        method: &str,
        annotation: Option<&str>,
    ) -> Result<(String, bool), String>;
    /// Takes back an endpoint this call created, after a later step failed.
    fn discard_endpoint(&mut self, endpoint_id: &str) -> Result<(), String>;
    /// Stores the QP, unless the endpoint already has an identical one (the
    /// check and the store are one step, so two parallel calls can't both
    /// store it).
    fn store_qp(&mut self, qp: &QpToStore) -> Result<Stored, String>;
    fn record_history(&mut self, endpoint_id: &str, details: &str) -> Result<(), String>;
    /// Bookmark + membership in the collection (and its size)
    fn add_to_collection(&mut self, collection: &str, endpoint_id: &str, notes: Option<&str>) -> Result<(), String>;
}

// ── helpers ──────────────────────────────────────────────────────────────

/// A body given as a string is the text of a body: JSON if it parses, else a
/// JSON string, the same as `parse_qp_body` for the QP routes of Test View.
fn parse_body(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_string()))
}

/// The text of a request/response file for a body the caller gave.
fn body_text(val: Option<&Value>) -> String {
    match val {
        None => "{}".to_string(),
        Some(Value::String(s)) => parse_body(s).to_string(),
        Some(v) => v.to_string(),
    }
}

fn headers_text(request: Option<&Value>, response: Option<&Value>) -> String {
    let obj = |v: Option<&Value>| match v {
        Some(Value::Object(m)) => Value::Object(m.clone()),
        _ => json!({}),
    };
    json!({ "request_headers": obj(request), "response_headers": obj(response) }).to_string()
}

/// A collection name becomes a file name (`storage/collections/{name}.sqlite`),
/// so it must not be able to point anywhere else. The webserver has already
/// applied the full naming rules; this is the part that protects the disk.
fn collection_name_problem(name: &str) -> Option<String> {
    const FORBIDDEN: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    if name == "." || name == ".." || name != name.trim() || name.ends_with('.') {
        return Some(format!("'{name}' is not a valid collection name"));
    }
    if name.chars().count() > 100 {
        return Some("a collection name can't be longer than 100 characters".to_string());
    }
    name.chars()
        .find(|c| FORBIDDEN.contains(c) || c.is_control())
        .map(|bad| format!("a collection name can't contain {bad:?} (it becomes a file name)"))
}

fn finish(status: ApiStatus, detail: impl Into<String>) -> CollectionOpResponse {
    let mut result = ApiResult::new(status);
    result.detail = Some(detail.into());
    CollectionOpResponse { result, collection_id: None, response: None }
}

/// What the flow knows by the time it stores: shared by both ops.
struct Plan<'a> {
    method: &'a str,
    request: String,
    response: String,
    headers: String,
    status_code: Option<i32>,
    response_time_ms: Option<i32>,
    /// History details (`"404 in 120ms"`), for a real test only
    history: Option<String>,
    returned: Option<EndpointResponse>,
}

/// Dispatches and processes developer collection operations.
pub async fn process_collection_op<S, F, Fut>(
    req: CollectionOpRequest,
    storage: &mut S,
    http_client: F,
) -> CollectionOpResponse
where
    S: StorageBackend,
    F: FnOnce(HttpCall) -> Fut,
    Fut: std::future::Future<Output = Result<EndpointResponse, HttpFailure>>,
{
    // 1. Mandatory parameters: collection & endpoint_str
    if req.collection.trim().is_empty() || req.endpoint_str.trim().is_empty() {
        return finish(ApiStatus::MissingParam, "Mandatory parameter 'collection' or 'endpoint_str' is missing");
    }

    // 2. The name becomes a file name: never let it point elsewhere
    if let Some(problem) = collection_name_problem(&req.collection) {
        return finish(ApiStatus::InvalidParam, problem);
    }

    let plan = match req.ops.as_str() {
        "insert" => {
            let Some(response_body) = req.response_body.as_ref() else {
                return finish(ApiStatus::MissingParam, "Mandatory parameter 'response_body' is missing for insert op");
            };
            Plan {
                method: req.method.as_deref().map(str::trim).filter(|m| !m.is_empty()).unwrap_or("GET"),
                request: body_text(req.request_body.as_ref()),
                response: body_text(Some(response_body)),
                headers: headers_text(
                    req.request_headers.as_ref().or(req.headers.as_ref()),
                    req.response_headers.as_ref(),
                ),
                // Nothing was really sent, so there is no status or timing
                status_code: None,
                response_time_ms: None,
                history: None,
                returned: None,
            }
        }

        "test_and_insert" => {
            let method = match req.method.as_deref().map(str::trim) {
                Some(m) if !m.is_empty() => m,
                _ => return finish(ApiStatus::MissingParam, "Mandatory parameter 'method' is missing for test_and_insert"),
            };

            // Test first: if the API gives no answer, nothing is created
            let answer = http_client(HttpCall {
                url: req.endpoint_str.clone(),
                method: method.to_string(),
                body: req.request_body.clone(),
                headers: req.headers.clone(),
                timeout_ms: req.timeout_ms,
            })
            .await;
            let answer = match answer {
                Ok(a) => a,
                Err(HttpFailure::TimedOut(msg)) => return finish(ApiStatus::TestTimedOut, msg),
                Err(HttpFailure::Failed(msg)) => return finish(ApiStatus::TestFailed, msg),
            };

            Plan {
                method,
                request: body_text(req.request_body.as_ref()),
                response: answer.body.to_string(),
                headers: headers_text(req.headers.as_ref(), Some(&answer.headers)),
                status_code: Some(i32::from(answer.status)),
                response_time_ms: Some(answer.time_ms.clamp(0, i64::from(i32::MAX)) as i32),
                history: Some(format!("{} in {}ms", answer.status, answer.time_ms)),
                returned: (req.return_response == Some(true)).then(|| answer.clone()),
            }
        }

        unknown => {
            return finish(
                ApiStatus::UnknownOp,
                format!("Unknown op '{unknown}', expected 'insert' or 'test_and_insert'"),
            );
        }
    };

    store_and_join(&req, storage, plan)
}

/// The part both ops share: collection, endpoint, duplicate check, store,
/// History, membership.
fn store_and_join<S: StorageBackend>(req: &CollectionOpRequest, storage: &mut S, plan: Plan) -> CollectionOpResponse {
    let mut result = ApiResult::new(ApiStatus::Ok);
    let col = req.collection.as_str();

    // Collection: created empty if new
    let created_collection = match storage.ensure_collection(col, req.collection_annotation.as_deref()) {
        Ok(created) => created,
        Err(e) => return finish(ApiStatus::InternalError, format!("Failed to create collection: {e}")),
    };
    result.collection_created = Some(created_collection);
    let collection_id = Some(col.to_string());
    let respond = |result: ApiResult| CollectionOpResponse {
        result,
        collection_id: collection_id.clone(),
        response: plan.returned.clone(),
    };

    // Endpoint: found by URL + method, or created with a real EID
    let (eid, created) = match storage.find_or_create_endpoint(
        req.endpoint_id.as_deref(),
        &req.endpoint_str,
        plan.method,
        req.annotation.as_deref(),
    ) {
        Ok(pair) => pair,
        Err(e) => {
            result.status = ApiStatus::InternalError;
            result.detail = Some(format!("Failed to create endpoint: {e}"));
            return respond(result);
        }
    };
    result.endpoint_id = Some(eid.clone());
    result.endpoint_created = Some(created);

    // The QP, unless an identical one is already stored
    let stored = match storage.store_qp(&QpToStore {
        endpoint_id: &eid,
        endpoint_str: &req.endpoint_str,
        method: plan.method,
        request: &plan.request,
        response: &plan.response,
        headers: &plan.headers,
        status_code: plan.status_code,
        response_time_ms: plan.response_time_ms,
    }) {
        Ok(n) => n,
        Err(e) => {
            if created {
                let _ = storage.discard_endpoint(&eid);
                result.endpoint_created = Some(false);
            }
            result.status = ApiStatus::InternalError;
            result.detail = Some(format!("Failed to store QP: {e}"));
            return respond(result);
        }
    };
    let number = match stored {
        Stored::New(n) => n,
        // Nothing new is stored, but the endpoint still joins the collection:
        // an identical QP is already there, so the endpoint is not new to
        // EDMS, only to this collection.
        Stored::Duplicate(dup) => {
            result.status = ApiStatus::DuplicateQp;
            result.duplicate_of = Some(dup);
            result.detail = Some("Duplicate QP detected; redundant write skipped".to_string());
            if let Err(e) = storage.add_to_collection(col, &eid, req.notes.as_deref()) {
                result.detail = Some(format!("Duplicate QP detected, and the endpoint could not join the collection: {e}"));
            }
            return respond(result);
        }
    };
    result.request_number = Some(number);

    // History, like any completed Run. A QP that is stored must not be
    // reported as failed because its History row was not.
    let mut notes: Vec<String> = Vec::new();
    if let Some(details) = &plan.history {
        if let Err(e) = storage.record_history(&eid, details) {
            notes.push(format!("the History row could not be written: {e}"));
        }
    }

    // Bookmark + membership
    if let Err(e) = storage.add_to_collection(col, &eid, req.notes.as_deref()) {
        result.status = ApiStatus::InternalError;
        notes.push(format!("the QP was stored but the endpoint could not join the collection: {e}"));
    }
    if !notes.is_empty() {
        result.detail = Some(notes.join("; "));
    }
    respond(result)
}

// ── the task `edms-child` runs ───────────────────────────────────────────

/// The real test request: the same call a Run makes.
pub async fn http_call(call: HttpCall) -> Result<EndpointResponse, HttpFailure> {
    use crate::api::handlers::{run_test_inner, RunTestRequest};

    let headers: HashMap<String, String> = match &call.headers {
        Some(Value::Object(m)) => m
            .iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
            .collect(),
        _ => HashMap::new(),
    };
    let sent = run_test_inner(RunTestRequest {
        endpoint_id: String::new(),
        url: call.url,
        method: call.method,
        body: call.body.unwrap_or(Value::Null),
        request_number: 0,
        timeout_ms: call.timeout_ms.unwrap_or(30_000),
        headers,
    })
    .await
    .map_err(HttpFailure::Failed)?;

    let v: Value = serde_json::from_str(&sent).map_err(|e| HttpFailure::Failed(format!("unreadable test result: {e}")))?;
    if v.get("timed_out").and_then(Value::as_bool).unwrap_or(false) {
        return Err(HttpFailure::TimedOut("the test request timed out".to_string()));
    }
    Ok(EndpointResponse {
        status: v.get("status_code").and_then(Value::as_u64).unwrap_or(0) as u16,
        headers: v.get("response_headers").cloned().unwrap_or_else(|| json!({})),
        body: v.get("response_body").cloned().unwrap_or(Value::Null),
        time_ms: v.get("response_time_ms").and_then(Value::as_i64).unwrap_or(0),
    })
}

/// Runs one `api_collections_*` task. `payload` is the validated Dev API
/// body plus what the webserver adds: `job_id`, `db_path`, `storage_root`.
/// The returned JSON is an `ApiResult` (plus `job_id`, `collection_id`, and
/// `response` when asked for), which the webserver delivers over the Dev API
/// WebSocket.
pub async fn run_task(payload: Value) -> Value {
    let job_id = payload.get("job_id").cloned().unwrap_or(Value::Null);
    let mut out = match run_task_inner(payload).await {
        Ok(v) => v,
        Err(msg) => serde_json::to_value(finish(ApiStatus::InternalError, msg).result).unwrap_or(Value::Null),
    };
    out["job_id"] = job_id;
    out
}

async fn run_task_inner(payload: Value) -> Result<Value, String> {
    let db_path = payload
        .get("db_path")
        .and_then(Value::as_str)
        .ok_or("the payload has no db_path")?
        .to_string();
    let root = payload
        .get("storage_root")
        .and_then(Value::as_str)
        .ok_or("the payload has no storage_root")?
        .to_string();
    let req: CollectionOpRequest =
        serde_json::from_value(payload).map_err(|e| format!("payload deserialise error: {e}"))?;

    let store = crate::qp_store::QpStore::open(&db_path, std::path::Path::new(&root))?;
    let mut storage = crate::collections_store::CollectionStore::new(store);
    let done = process_collection_op(req, &mut storage, http_call).await;

    let mut out = serde_json::to_value(&done.result).map_err(|e| e.to_string())?;
    if let Some(id) = done.collection_id {
        out["collection_id"] = json!(id);
    }
    if let Some(r) = done.response {
        out["response"] = serde_json::to_value(r).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A storage that keeps everything in memory and records what was asked.
    #[derive(Default)]
    struct Mem {
        collections: Vec<(String, Option<String>)>,
        endpoints: Vec<(String, String, String)>, // (eid, url, method)
        qps: Vec<(String, i32, String, Option<i32>)>, // (eid, number, request, status)
        history: Vec<(String, String)>,
        joined: Vec<(String, String, Option<String>)>,
        discarded: Vec<String>,
        fail_store: bool,
        fail_join: bool,
        fail_history: bool,
    }

    impl StorageBackend for Mem {
        fn ensure_collection(&mut self, name: &str, annotation: Option<&str>) -> Result<bool, String> {
            if self.collections.iter().any(|(n, _)| n == name) {
                return Ok(false);
            }
            self.collections.push((name.to_string(), annotation.map(str::to_string)));
            Ok(true)
        }
        fn find_or_create_endpoint(
            &mut self,
            _endpoint_id: Option<&str>,
            endpoint_str: &str,
            method: &str,
            _annotation: Option<&str>,
        ) -> Result<(String, bool), String> {
            if let Some((eid, ..)) = self.endpoints.iter().find(|(_, u, m)| u == endpoint_str && m == method) {
                return Ok((eid.clone(), false));
            }
            let eid = format!("E{:04}-AAA", self.endpoints.len() + 1);
            self.endpoints.push((eid.clone(), endpoint_str.to_string(), method.to_string()));
            Ok((eid, true))
        }
        fn discard_endpoint(&mut self, endpoint_id: &str) -> Result<(), String> {
            self.discarded.push(endpoint_id.to_string());
            self.endpoints.retain(|(e, ..)| e != endpoint_id);
            Ok(())
        }
        fn store_qp(&mut self, qp: &QpToStore) -> Result<Stored, String> {
            if self.fail_store {
                return Err("disk full".to_string());
            }
            if let Some((_, n, ..)) = self.qps.iter().find(|(e, _, r, _)| e == qp.endpoint_id && r == qp.request) {
                return Ok(Stored::Duplicate(*n));
            }
            let n = self.qps.iter().filter(|(e, ..)| e == qp.endpoint_id).count() as i32 + 1;
            self.qps.push((qp.endpoint_id.to_string(), n, qp.request.to_string(), qp.status_code));
            Ok(Stored::New(n))
        }
        fn record_history(&mut self, eid: &str, details: &str) -> Result<(), String> {
            if self.fail_history {
                return Err("locked".to_string());
            }
            self.history.push((eid.to_string(), details.to_string()));
            Ok(())
        }
        fn add_to_collection(&mut self, collection: &str, eid: &str, notes: Option<&str>) -> Result<(), String> {
            if self.fail_join {
                return Err("locked".to_string());
            }
            self.joined.push((collection.to_string(), eid.to_string(), notes.map(str::to_string)));
            Ok(())
        }
    }

    fn insert_req() -> CollectionOpRequest {
        CollectionOpRequest {
            ops: "insert".to_string(),
            endpoint_str: "/api/v1/login".to_string(),
            collection: "auth_suite".to_string(),
            method: Some("POST".to_string()),
            request_body: Some(json!({"user": "alice"})),
            response_body: Some(json!({"token": "xyz"})),
            ..Default::default()
        }
    }

    fn test_req() -> CollectionOpRequest {
        CollectionOpRequest {
            ops: "test_and_insert".to_string(),
            endpoint_str: "https://httpbin.org/get".to_string(),
            collection: "httpbin_suite".to_string(),
            method: Some("GET".to_string()),
            timeout_ms: Some(5000),
            ..Default::default()
        }
    }

    fn answer(status: u16) -> EndpointResponse {
        EndpointResponse {
            status,
            headers: json!({"content-type": "application/json"}),
            body: json!({"origin": "127.0.0.1"}),
            time_ms: 120,
        }
    }

    async fn never(_: HttpCall) -> Result<EndpointResponse, HttpFailure> {
        unreachable!("an insert makes no HTTP call")
    }

    #[tokio::test]
    async fn test_missing_mandatory_params() {
        let mut db = Mem::default();
        let req = CollectionOpRequest { endpoint_str: "".to_string(), ..insert_req() };
        let res = process_collection_op(req, &mut db, never).await;
        assert_eq!(res.result.status, ApiStatus::MissingParam);
        assert!(db.collections.is_empty(), "nothing is created for a bad request");
    }

    #[tokio::test]
    async fn test_path_traversal_in_collection_rejected() {
        for bad in ["../malicious_dir", "a/b", "a\\b", "..", "x:y", "what?"] {
            let mut db = Mem::default();
            let req = CollectionOpRequest { collection: bad.to_string(), ..insert_req() };
            let res = process_collection_op(req, &mut db, never).await;
            assert_eq!(res.result.status, ApiStatus::InvalidParam, "{bad}");
            assert!(db.collections.is_empty());
        }
    }

    #[tokio::test]
    async fn a_collection_name_with_a_space_is_fine() {
        let mut db = Mem::default();
        let req = CollectionOpRequest { collection: "my api".to_string(), ..insert_req() };
        let res = process_collection_op(req, &mut db, never).await;
        assert_eq!(res.result.status, ApiStatus::Ok);
    }

    #[tokio::test]
    async fn test_unknown_op_rejected() {
        let mut db = Mem::default();
        let req = CollectionOpRequest { ops: "delete_everything".to_string(), ..insert_req() };
        let res = process_collection_op(req, &mut db, never).await;
        assert_eq!(res.result.status, ApiStatus::UnknownOp);
    }

    #[tokio::test]
    async fn an_insert_needs_a_response_body() {
        let mut db = Mem::default();
        let req = CollectionOpRequest { response_body: None, ..insert_req() };
        let res = process_collection_op(req, &mut db, never).await;
        assert_eq!(res.result.status, ApiStatus::MissingParam);
        // a response that is JSON null is still a response
        let mut db = Mem::default();
        let req = CollectionOpRequest { response_body: Some(Value::Null), ..insert_req() };
        assert_eq!(process_collection_op(req, &mut db, never).await.result.status, ApiStatus::Ok);
    }

    #[tokio::test]
    async fn test_insert_creates_collection_and_endpoint() {
        let mut db = Mem::default();
        let mut req = insert_req();
        req.collection_annotation = Some("login flows".to_string());
        req.notes = Some("from the dev api".to_string());
        let res = process_collection_op(req, &mut db, never).await;

        assert_eq!(res.result.status, ApiStatus::Ok);
        assert_eq!(res.result.collection_created, Some(true));
        assert_eq!(res.result.endpoint_created, Some(true));
        assert_eq!(res.result.request_number, Some(1));
        assert_eq!(res.result.endpoint_id.as_deref(), Some("E0001-AAA"));
        assert_eq!(res.collection_id.as_deref(), Some("auth_suite"));
        assert_eq!(db.collections, vec![("auth_suite".to_string(), Some("login flows".to_string()))]);
        assert_eq!(
            db.joined,
            vec![("auth_suite".to_string(), "E0001-AAA".to_string(), Some("from the dev api".to_string()))]
        );
        // an insert makes no History row and records no status
        assert!(db.history.is_empty());
        assert_eq!(db.qps[0].3, None);
    }

    #[tokio::test]
    async fn an_insert_without_a_method_uses_get() {
        let mut db = Mem::default();
        let req = CollectionOpRequest { method: None, ..insert_req() };
        process_collection_op(req, &mut db, never).await;
        assert_eq!(db.endpoints[0].2, "GET");
    }

    #[tokio::test]
    async fn test_insert_duplicate_blocked() {
        let mut db = Mem::default();
        let res1 = process_collection_op(insert_req(), &mut db, never).await;
        assert_eq!(res1.result.status, ApiStatus::Ok);

        let res2 = process_collection_op(insert_req(), &mut db, never).await;
        assert_eq!(res2.result.status, ApiStatus::DuplicateQp);
        assert_eq!(res2.result.collection_created, Some(false));
        assert_eq!(res2.result.endpoint_created, Some(false));
        assert_eq!(res2.result.duplicate_of, Some(1));
        assert_eq!(db.qps.len(), 1, "the duplicate is not stored");
    }

    #[tokio::test]
    async fn a_duplicate_still_joins_the_collection() {
        let mut db = Mem::default();
        process_collection_op(insert_req(), &mut db, never).await;
        let other = CollectionOpRequest { collection: "second".to_string(), ..insert_req() };
        let res = process_collection_op(other, &mut db, never).await;
        assert_eq!(res.result.status, ApiStatus::DuplicateQp);
        assert!(db.joined.iter().any(|(c, e, _)| c == "second" && e == "E0001-AAA"));
    }

    #[tokio::test]
    async fn test_and_insert_stores_a_404_like_any_answer() {
        let mut db = Mem::default();
        let res = process_collection_op(test_req(), &mut db, |_| async { Ok(answer(404)) }).await;

        assert_eq!(res.result.status, ApiStatus::Ok);
        assert_eq!(res.result.request_number, Some(1));
        assert_eq!(db.qps[0].3, Some(404));
        assert_eq!(db.history, vec![("E0001-AAA".to_string(), "404 in 120ms".to_string())]);
        assert_eq!(db.joined.len(), 1);
    }

    #[tokio::test]
    async fn test_and_insert_sends_what_it_was_given() {
        let mut db = Mem::default();
        let mut req = test_req();
        req.method = Some("POST".to_string());
        req.request_body = Some(json!({"a": 1}));
        req.headers = Some(json!({"X-Key": "k"}));
        let res = process_collection_op(req, &mut db, |call| async move {
            assert_eq!(call.url, "https://httpbin.org/get");
            assert_eq!(call.method, "POST");
            assert_eq!(call.body, Some(json!({"a": 1})));
            assert_eq!(call.headers, Some(json!({"X-Key": "k"})));
            assert_eq!(call.timeout_ms, Some(5000));
            Ok(answer(200))
        })
        .await;
        assert_eq!(res.result.status, ApiStatus::Ok);
    }

    #[tokio::test]
    async fn test_and_insert_needs_a_method() {
        let mut db = Mem::default();
        let req = CollectionOpRequest { method: None, ..test_req() };
        let res = process_collection_op(req, &mut db, |_| async { unreachable!() }).await;
        assert_eq!(res.result.status, ApiStatus::MissingParam);
    }

    #[tokio::test]
    async fn test_test_and_insert_with_response_mode() {
        let mut db = Mem::default();
        let req = CollectionOpRequest { return_response: Some(true), ..test_req() };
        let res = process_collection_op(req, &mut db, |_| async { Ok(answer(200)) }).await;
        assert_eq!(res.result.status, ApiStatus::Ok);
        assert_eq!(res.response.as_ref().unwrap().status, 200);
        assert_eq!(res.response.as_ref().unwrap().body, json!({"origin": "127.0.0.1"}));

        let mut db = Mem::default();
        let res = process_collection_op(test_req(), &mut db, |_| async { Ok(answer(200)) }).await;
        assert!(res.response.is_none(), "only when asked for");
    }

    #[tokio::test]
    async fn a_test_with_no_answer_creates_nothing() {
        let mut db = Mem::default();
        let res = process_collection_op(test_req(), &mut db, |_| async {
            Err(HttpFailure::TimedOut("timed out after 5000ms".to_string()))
        })
        .await;
        assert_eq!(res.result.status, ApiStatus::TestTimedOut);

        let res = process_collection_op(test_req(), &mut db, |_| async {
            Err(HttpFailure::Failed("dns error".to_string()))
        })
        .await;
        assert_eq!(res.result.status, ApiStatus::TestFailed);
        assert!(db.collections.is_empty() && db.endpoints.is_empty() && db.qps.is_empty() && db.history.is_empty());
    }

    #[tokio::test]
    async fn a_failed_store_takes_back_an_endpoint_it_created() {
        let mut db = Mem { fail_store: true, ..Default::default() };
        let res = process_collection_op(insert_req(), &mut db, never).await;
        assert_eq!(res.result.status, ApiStatus::InternalError);
        assert_eq!(db.discarded, vec!["E0001-AAA".to_string()]);
        assert!(db.endpoints.is_empty());
        assert_eq!(res.result.endpoint_created, Some(false));

        // an endpoint that was already there is left alone
        let mut db = Mem::default();
        process_collection_op(insert_req(), &mut db, never).await;
        db.fail_store = true;
        let other = CollectionOpRequest { request_body: Some(json!({"user": "bob"})), ..insert_req() };
        let res = process_collection_op(other, &mut db, never).await;
        assert_eq!(res.result.status, ApiStatus::InternalError);
        assert!(db.discarded.is_empty());
        assert_eq!(db.endpoints.len(), 1);
    }

    #[tokio::test]
    async fn a_history_failure_does_not_fail_a_stored_qp() {
        let mut db = Mem { fail_history: true, ..Default::default() };
        let res = process_collection_op(test_req(), &mut db, |_| async { Ok(answer(200)) }).await;
        assert_eq!(res.result.status, ApiStatus::Ok);
        assert!(res.result.detail.as_deref().unwrap().contains("History row"));
    }

    #[tokio::test]
    async fn a_failed_join_is_reported() {
        let mut db = Mem { fail_join: true, ..Default::default() };
        let res = process_collection_op(insert_req(), &mut db, never).await;
        assert_eq!(res.result.status, ApiStatus::InternalError);
        assert_eq!(res.result.request_number, Some(1), "the QP was stored, and the result says so");
    }

    #[test]
    fn bodies_and_headers_become_the_text_of_the_files() {
        assert_eq!(body_text(None), "{}");
        assert_eq!(body_text(Some(&json!({"b": 2, "a": 1}))), r#"{"a":1,"b":2}"#);
        // a string is the text of a body: JSON if it parses, else a JSON string
        assert_eq!(body_text(Some(&json!("{\"a\": 1}"))), r#"{"a":1}"#);
        assert_eq!(body_text(Some(&json!("plain text"))), r#""plain text""#);
        assert_eq!(
            headers_text(Some(&json!({"A": "b"})), None),
            r#"{"request_headers":{"A":"b"},"response_headers":{}}"#
        );
    }

    #[tokio::test]
    async fn test_nested_spec_schema_deserialization() {
        let nested_json = json!({
            "collection": {
                "name": "payments_v1",
                "tags": ["prod", "finance"],
                "annotation": "Payment processing endpoints"
            },
            "endpoint": {
                "name": "https://api.example.com/charge",
                "crud": "POST",
                "headers": {"Authorization": "Bearer token123"},
                "request": {"amount": 500, "currency": "USD"},
                "response": {"id": "ch_123", "status": "succeeded"},
                "annotation": "Charge credit card",
                "tags": ["payments"]
            },
            "ops": {
                "type": "insert"
            }
        });

        let req: CollectionOpRequest = serde_json::from_value(nested_json).unwrap();
        assert_eq!(req.ops, "insert");
        assert_eq!(req.collection, "payments_v1");
        assert_eq!(req.collection_annotation.as_deref(), Some("Payment processing endpoints"));
        assert_eq!(req.collection_tags, Some(vec!["prod".to_string(), "finance".to_string()]));
        assert_eq!(req.endpoint_str, "https://api.example.com/charge");
        assert_eq!(req.method.as_deref(), Some("POST"));
        assert_eq!(req.tags, Some(vec!["payments".to_string()]));

        let mut db = Mem::default();
        let res = process_collection_op(req, &mut db, never).await;
        assert_eq!(res.result.status, ApiStatus::Ok);
        assert_eq!(res.result.collection_created, Some(true));
        assert_eq!(res.result.endpoint_created, Some(true));
        assert_eq!(res.result.request_number, Some(1));
    }

    #[test]
    fn the_flat_body_the_webserver_validates_is_read() {
        let flat = json!({
            "ops": "insert", "endpoint_str": "/v1/users", "collection": "c",
            "request_body": {"id": 1}, "response_body": {"ok": true},
            "request_headers": {"a": "b"}, "response_headers": {"c": "d"},
            "annotation": "users", "collection_annotation": "all users", "notes": "n",
            "endpoint_id": "E0007-AAA", "job_id": "job-1", "db_path": "/x/edms.db", "storage_root": "/x"
        });
        let req: CollectionOpRequest = serde_json::from_value(flat).unwrap();
        assert_eq!(req.ops, "insert");
        assert_eq!(req.collection, "c");
        assert_eq!(req.endpoint_str, "/v1/users");
        assert_eq!(req.request_headers, Some(json!({"a": "b"})));
        assert_eq!(req.response_headers, Some(json!({"c": "d"})));
        assert_eq!(req.notes.as_deref(), Some("n"));
        assert_eq!(req.endpoint_id.as_deref(), Some("E0007-AAA"));
        assert_eq!(req.method, None);
    }
}
