//! Task api_history_test (ops "test"): run the endpoint, then store the
//! endpoint, QP and history row the same way Test View's Run does. Sends back
//! one code from crate::api_status, plus the job_id it was given.
//! Mandatory: ops, endpoint_str, method. Optional: body, headers, timeout_ms,
//! annotation, notes, endpoint_id.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

// explicit list: a glob would pull in ApiStatus::Ok over Result::Ok
use crate::api_status::ApiStatus::{
    self, DuplicateQp, InternalError, InvalidParam, InvalidRequest, MissingParam, TestFailed,
    TestTimedOut, UnknownOp,
};
use crate::api_status::ApiResult;
use crate::qp_store::{NewQp, QpStore, Stored};

const METHODS: [&str; 7] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];
// the webserver closes the call with 108 after 30s; stay under it so a slow
// target reports 106 and there is time left to store
const MAX_TIMEOUT_MS: u64 = 25_000;

type Rejection = (ApiStatus, String);

#[derive(Debug, Clone)]
pub struct HistoryTestRequest {
    pub endpoint_str: String,
    pub method: String,
    pub body: Value,
    pub headers: HashMap<String, String>,
    pub timeout_ms: u64,
    pub annotation: Option<String>,
    pub notes: Option<String>,
    pub endpoint_id: Option<String>,
}

/// What the target answered.
#[derive(Debug, Clone)]
pub struct Tested {
    pub status_code: i64,
    pub response_time_ms: i64,
    pub headers: HashMap<String, String>,
    pub body: Value,
}

// IPC task "api_history_test"; always Ok, the outcome is in `status`.
// job_id goes back on every path: the webserver finds the call by it
pub async fn history_ops_inner(payload: Value) -> Result<String, String> {
    let result = match parse_history_request(&payload) {
        Err((status, detail)) => failed(status, detail),
        Ok(req) => test_op(&req, &payload).await,
    };
    let mut out = json!(result);
    out["job_id"] = payload.get("job_id").cloned().unwrap_or(Value::Null);
    // a new QP: the callback turns this into a ViewRefresh for the open views
    if result.request_number.is_some() {
        out["view_type"] = json!("History");
    }
    Ok(out.to_string())
}

pub fn parse_history_request(payload: &Value) -> Result<HistoryTestRequest, Rejection> {
    let obj = payload
        .as_object()
        .ok_or((InvalidRequest, "payload must be a JSON object".into()))?;

    // mandatory
    let ops = required(obj, "ops")?;
    if ops != "test" {
        return Err((UnknownOp, format!("ops '{ops}' not supported (use: test)")));
    }
    let endpoint_str = required(obj, "endpoint_str")?;
    match url::Url::parse(endpoint_str) {
        Ok(u) if matches!(u.scheme(), "http" | "https") => {}
        _ => return Err((InvalidParam, format!("endpoint_str '{endpoint_str}' must be an http(s) URL"))),
    }
    let method = required(obj, "method")?.to_uppercase();
    if !METHODS.contains(&method.as_str()) {
        return Err((InvalidParam, format!("method '{method}' not supported")));
    }

    // optional
    let mut headers = HashMap::new();
    match obj.get("headers") {
        None | Some(Value::Null) => {}
        Some(Value::Object(map)) => {
            for (k, v) in map {
                let v = v.as_str().ok_or_else(|| wrong_type(&format!("headers.{k}")))?;
                headers.insert(k.clone(), v.to_string());
            }
        }
        Some(_) => return Err(wrong_type("headers")),
    }

    let timeout_ms = match obj.get("timeout_ms") {
        None | Some(Value::Null) => MAX_TIMEOUT_MS,
        Some(v) => v
            .as_u64()
            .filter(|ms| *ms > 0)
            .ok_or_else(|| wrong_type("timeout_ms"))?
            .min(MAX_TIMEOUT_MS),
    };

    Ok(HistoryTestRequest {
        endpoint_str: endpoint_str.into(),
        method,
        body: obj.get("body").cloned().unwrap_or(Value::Null),
        headers,
        timeout_ms,
        annotation: optional(obj, "annotation")?,
        notes: optional(obj, "notes")?,
        endpoint_id: optional(obj, "endpoint_id")?,
    })
}

fn required<'a>(obj: &'a Map<String, Value>, key: &str) -> Result<&'a str, Rejection> {
    match obj.get(key) {
        None | Some(Value::Null) => Err((MissingParam, format!("'{key}' is required"))),
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(s.trim()),
        Some(Value::String(_)) => Err((MissingParam, format!("'{key}' is empty"))),
        Some(_) => Err(wrong_type(key)),
    }
}

fn optional(obj: &Map<String, Value>, key: &str) -> Result<Option<String>, Rejection> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(wrong_type(key)),
    }
}

fn wrong_type(key: &str) -> Rejection {
    (InvalidParam, format!("'{key}' has the wrong type"))
}

// nothing stored: just the code and what was wrong
fn failed(status: ApiStatus, detail: String) -> ApiResult {
    ApiResult { detail: Some(detail), ..ApiResult::new(status) }
}

// run, then store
async fn test_op(req: &HistoryTestRequest, payload: &Value) -> ApiResult {
    let tested = match call_endpoint(req).await {
        Ok(t) => t,
        Err((status, detail)) => return failed(status, detail),
    };
    store(req, &tested, &Paths::from(payload))
        .unwrap_or_else(|e| failed(InternalError, format!("store failed: {e}")))
}

// fresh copy of api::handlers::run_test_inner
pub async fn call_endpoint(req: &HistoryTestRequest) -> Result<Tested, Rejection> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(req.timeout_ms))
        .build()
        .map_err(|e| (InternalError, format!("http client: {e}")))?;
    let url = &req.endpoint_str;
    let mut http = match req.method.as_str() {
        "POST" => client.post(url).json(&req.body),
        "PUT" => client.put(url).json(&req.body),
        "PATCH" => client.patch(url).json(&req.body),
        "DELETE" => client.delete(url),
        "HEAD" => client.head(url),
        "OPTIONS" => client.request(reqwest::Method::OPTIONS, url),
        _ => client.get(url),
    };
    for (k, v) in &req.headers {
        http = http.header(k, v);
    }

    let test_error = |e: reqwest::Error| {
        if e.is_timeout() {
            (TestTimedOut, format!("no response in {}ms", req.timeout_ms))
        } else {
            (TestFailed, format!("unreachable: {e}"))
        }
    };

    let start = Instant::now();
    let resp = http.send().await.map_err(test_error)?;
    let response_time_ms = start.elapsed().as_millis() as i64;
    let status_code = resp.status().as_u16() as i64;
    let headers = resp
        .headers()
        .iter()
        .filter_map(|(k, v)| Some((k.to_string(), v.to_str().ok()?.to_string())))
        .collect();
    // a body cut off part way is a failed test, not an empty response to store
    let text = resp.text().await.map_err(test_error)?;
    let body = serde_json::from_str(&text).unwrap_or(Value::String(text));
    Ok(Tested { status_code, response_time_ms, headers, body })
}

/// Where the DB and EQP files live. The webserver passes both; the env
/// fallbacks match what the webserver and edms-child use.
pub struct Paths {
    pub db_path: String,
    pub storage_root: PathBuf,
}

impl Paths {
    pub fn from(payload: &Value) -> Self {
        let db_path = payload["db_path"].as_str().map(String::from)
            .or_else(|| std::env::var("EDMS_DB_PATH").ok())
            .unwrap_or_else(|| "edms.db".into());
        let storage_root = payload["storage_root"].as_str().map(PathBuf::from)
            .or_else(|| std::env::var_os("EDMS_ROOT").map(PathBuf::from))
            .unwrap_or_else(crate::folder_manager::default_root_path);
        Paths { db_path, storage_root }
    }
}

// same steps as a Run, through the QpStore the collections ops share: endpoint
// (reuse or create), then the QP and duplicate check as one locked section,
// then the history row
pub fn store(req: &HistoryTestRequest, t: &Tested, paths: &Paths) -> Result<ApiResult, String> {
    let db = QpStore::open(&paths.db_path, &paths.storage_root)?;
    let request = req.body.to_string();
    let response = t.body.to_string();
    let headers = json!({ "request_headers": req.headers, "response_headers": t.headers }).to_string();

    let (eid, created) = db.find_or_create_endpoint(
        req.endpoint_id.as_deref(),
        &req.endpoint_str,
        &req.method,
        req.annotation.as_deref(),
    )?;
    let stored = db.store_qp(&NewQp {
        endpoint_id: &eid,
        endpoint_str: &req.endpoint_str,
        method: &req.method,
        request: &request,
        response: &response,
        headers: &headers,
        status_code: Some(t.status_code as i32),
        response_time_ms: Some(t.response_time_ms as i32),
    });
    let stored = match stored {
        Ok(stored) => stored,
        Err(e) => {
            // don't leave a half-made endpoint behind
            if created {
                let _ = db.discard_endpoint(&eid);
            }
            return Err(e);
        }
    };

    let mut result = ApiResult {
        endpoint_id: Some(eid.clone()),
        endpoint_created: Some(created),
        target_status: Some(t.status_code),
        ..ApiResult::new(ApiStatus::Ok)
    };
    match stored {
        Stored::Duplicate(n) => {
            result.status = DuplicateQp;
            result.duplicate_of = Some(n);
        }
        Stored::New(n) => {
            let details = format!("{} in {}ms", t.status_code, t.response_time_ms);
            // the QP is stored: a missing history row must not report it as failed
            result.detail = Some(match db.record_history(&eid, "test", Some(&details)) {
                Ok(()) => details,
                Err(e) => format!("{details}; the History row could not be written: {e}"),
            });
            result.request_number = Some(n);
        }
    }
    Ok(result)
}
