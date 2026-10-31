//! Collections Developer API Operations (Compute Module)
//!
//! Provides the compute logic for `/hashedtokens/edms/api/collections/`,
//! executing `insert` and `test_and_insert` operations against collections,
//! and returning standard `ApiStatus` codes and `ApiResult` matching origin/tmp.

use crate::api_status::{ApiResult, ApiStatus};
use crate::endpoint_writer::{write_headers_file, write_request_file, write_response_file};
use crate::qp_service;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Default)]
pub struct CollectionOpRequest {
    /// Operation to execute: "insert" or "test_and_insert"
    pub ops: String,
    /// Target endpoint URL string
    pub endpoint_str: String,
    /// Target collection name (missing collection will be created)
    pub collection: String,
    /// HTTP method (mandatory for "test_and_insert", e.g. "GET", "POST")
    #[serde(default)]
    pub method: Option<String>,
    /// Pre-recorded request payload / body
    #[serde(default)]
    pub request_body: Option<serde_json::Value>,
    /// Pre-recorded response payload / body (mandatory for "insert")
    #[serde(default)]
    pub response_body: Option<serde_json::Value>,
    /// Optional HTTP request headers
    #[serde(default)]
    pub headers: Option<serde_json::Value>,
    /// Optional timeout in milliseconds
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Optional annotation for the endpoint
    #[serde(default)]
    pub annotation: Option<String>,
    /// Optional tags for the endpoint
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// Optional annotation for the collection
    #[serde(default)]
    pub collection_annotation: Option<String>,
    /// Optional tags for the collection
    #[serde(default)]
    pub collection_tags: Option<Vec<String>>,
    /// Optional response mode flag: returns full endpoint response in result
    #[serde(default)]
    pub return_response: Option<bool>,
}

// Flexible deserializer: supports both the nested spec schema and flat schema
impl<'de> Deserialize<'de> for CollectionOpRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = serde_json::Value::deserialize(deserializer)?;
        
        // 1. Resolve ops ("insert", "test_and_insert")
        let ops = if let Some(ops_obj) = v.get("ops").and_then(|o| o.as_object()) {
            ops_obj.get("type").and_then(|t| t.as_str()).unwrap_or("").to_string()
        } else {
            v.get("ops").and_then(|o| o.as_str()).unwrap_or("").to_string()
        };

        // 2. Resolve collection, collection_annotation, collection_tags
        let (collection, collection_annotation, collection_tags) = if let Some(col_obj) = v.get("collection").and_then(|c| c.as_object()) {
            let name = col_obj.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
            let annot = col_obj.get("annotation").and_then(|a| a.as_str()).map(|s| s.to_string());
            let tags = col_obj.get("tags").and_then(|t| serde_json::from_value(t.clone()).ok());
            (name, annot, tags)
        } else {
            let name = v.get("collection").and_then(|c| c.as_str()).unwrap_or("").to_string();
            let annot = v.get("collection_annotation").and_then(|a| a.as_str()).map(|s| s.to_string());
            let tags = v.get("collection_tags").and_then(|t| serde_json::from_value(t.clone()).ok());
            (name, annot, tags)
        };

        // 3. Resolve endpoint, method/crud, headers, request, response, annotation, tags
        let (endpoint_str, method, headers, request_body, response_body, annotation, tags) =
            if let Some(ep_obj) = v.get("endpoint").and_then(|e| e.as_object()) {
                let ep_name = ep_obj.get("name").or_else(|| ep_obj.get("endpoint_str"))
                    .and_then(|n| n.as_str()).unwrap_or("").to_string();
                let m = ep_obj.get("crud").or_else(|| ep_obj.get("method"))
                    .and_then(|m| m.as_str()).map(|s| s.to_string());
                let h = ep_obj.get("headers").cloned();
                let req_b = ep_obj.get("request").or_else(|| ep_obj.get("request_body")).cloned();
                let resp_b = ep_obj.get("response").or_else(|| ep_obj.get("response_body")).cloned();
                let annot = ep_obj.get("annotation").and_then(|a| a.as_str()).map(|s| s.to_string());
                let ep_tags = ep_obj.get("tags").and_then(|t| serde_json::from_value(t.clone()).ok());
                (ep_name, m, h, req_b, resp_b, annot, ep_tags)
            } else {
                let ep_name = v.get("endpoint_str").or_else(|| v.get("endpoint"))
                    .and_then(|n| n.as_str()).unwrap_or("").to_string();
                let m = v.get("method").or_else(|| v.get("crud"))
                    .and_then(|m| m.as_str()).map(|s| s.to_string());
                let h = v.get("headers").cloned();
                let req_b = v.get("request_body").or_else(|| v.get("request")).cloned();
                let resp_b = v.get("response_body").or_else(|| v.get("response")).cloned();
                let annot = v.get("annotation").and_then(|a| a.as_str()).map(|s| s.to_string());
                let ep_tags = v.get("tags").and_then(|t| serde_json::from_value(t.clone()).ok());
                (ep_name, m, h, req_b, resp_b, annot, ep_tags)
            };

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
            timeout_ms,
            annotation,
            tags,
            collection_annotation,
            collection_tags,
            return_response,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EndpointResponse {
    pub status: u16,
    pub headers: serde_json::Value,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CollectionOpResponse {
    pub result: ApiResult,
    /// Target collection ID / handle
    pub collection_id: Option<String>,
    /// Full endpoint response when `return_response` is enabled
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<EndpointResponse>,
}

/// Normalizes JSON values (whether pre-stringified or raw JSON objects) to prevent double-escaping.
fn to_json_str(val: Option<&serde_json::Value>) -> String {
    match val {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(v) if !v.is_null() => v.to_string(),
        _ => String::new(),
    }
}

/// Sanitizes an endpoint identifier so it can be safely used as a filename prefix on all filesystems.
fn sanitize_eid(raw: &str) -> String {
    let clean: String = raw.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
    if clean.is_empty() {
        "EID".to_string()
    } else {
        clean
    }
}

pub trait StorageBackend {
    fn collection_exists(&self, name: &str) -> bool;
    fn create_collection(&mut self, name: &str, annotation: Option<&str>) -> Result<String, String>;
    fn endpoint_exists(&self, endpoint_str: &str) -> bool;
    fn create_endpoint(&mut self, endpoint_str: &str, method: &str, annotation: Option<&str>) -> Result<String, String>;
    fn is_duplicate(&self, col_id: &str, qp_data: &serde_json::Value) -> Option<i32>;
    fn insert_qp(&mut self, col_id: &str, endpoint_id: &str, qp_data: &serde_json::Value) -> Result<i32, String>;
}

/// Production disk-backed storage engine implementing Ravi's file layout and canonical deduplication.
pub struct EdmsStorage {
    pub storage_root: PathBuf,
}

impl EdmsStorage {
    pub fn new<P: Into<PathBuf>>(storage_root: P) -> Self {
        Self {
            storage_root: storage_root.into(),
        }
    }

    pub fn collections_dir(&self) -> PathBuf {
        self.storage_root.join("collections")
    }

    pub fn collection_path(&self, name: &str) -> PathBuf {
        self.collections_dir().join(name)
    }
}

impl StorageBackend for EdmsStorage {
    fn collection_exists(&self, name: &str) -> bool {
        self.collection_path(name).exists()
    }

    fn create_collection(&mut self, name: &str, _annotation: Option<&str>) -> Result<String, String> {
        let path = self.collection_path(name);
        fs::create_dir_all(&path).map_err(|e| format!("Failed to create collection dir: {}", e))?;
        Ok(format!("col_{}", name))
    }

    fn endpoint_exists(&self, _endpoint_str: &str) -> bool {
        true
    }

    fn create_endpoint(&mut self, endpoint_str: &str, _method: &str, _annotation: Option<&str>) -> Result<String, String> {
        let clean = sanitize_eid(endpoint_str);
        Ok(format!("ep_{}", clean))
    }

    fn is_duplicate(&self, col_id: &str, qp_data: &serde_json::Value) -> Option<i32> {
        let col_name = col_id.strip_prefix("col_").unwrap_or(col_id);
        let folder = self.collection_path(col_name);
        if !folder.exists() {
            return None;
        }

        let method = qp_data.get("method").and_then(|m| m.as_str()).unwrap_or("POST");
        let endpoint = qp_data.get("endpoint").and_then(|e| e.as_str()).unwrap_or("");
        let req_body = to_json_str(qp_data.get("request_body"));
        let resp_body = to_json_str(qp_data.get("response_body"));
        let headers = to_json_str(qp_data.get("headers"));

        let eid_raw = qp_data.get("endpoint_id").and_then(|e| e.as_str()).unwrap_or("EID");
        let eid = sanitize_eid(eid_raw);

        qp_service::find_duplicate_qp(&folder, &eid, method, endpoint, &req_body, &resp_body, &headers, None)
    }

    fn insert_qp(&mut self, col_id: &str, endpoint_id: &str, qp_data: &serde_json::Value) -> Result<i32, String> {
        let col_name = col_id.strip_prefix("col_").unwrap_or(col_id);
        let folder = self.collection_path(col_name);
        fs::create_dir_all(&folder).map_err(|e| format!("Failed to create collection folder: {}", e))?;

        let eid = sanitize_eid(endpoint_id);
        let existing = qp_service::list_existing_qp_numbers(&folder, &eid);
        let next_num = existing.iter().max().copied().unwrap_or(0) + 1;

        let req_body = to_json_str(qp_data.get("request_body"));
        let resp_body = to_json_str(qp_data.get("response_body"));
        let headers = to_json_str(qp_data.get("headers"));

        // Write disk files matching Ravi's file layout specification:
        // {eid}-request-{N}.json, {eid}-response-{N}.json, {eid}-headers-{N}.json
        write_request_file(&folder, &eid, next_num as usize, &req_body)
            .map_err(|e| format!("Failed to write request file: {}", e))?;
        write_response_file(&folder, &eid, next_num as usize, &resp_body)
            .map_err(|e| format!("Failed to write response file: {}", e))?;
        if !headers.is_empty() {
            write_headers_file(&folder, &eid, next_num as usize, &headers)
                .map_err(|e| format!("Failed to write headers file: {}", e))?;
        }

        Ok(next_num)
    }
}

/// Dispatches and processes developer collection operations in the compute engine.
pub async fn process_collection_op<S, F, Fut>(
    req: CollectionOpRequest,
    storage: &mut S,
    http_client: F,
) -> CollectionOpResponse
where
    S: StorageBackend,
    F: FnOnce(String, String, Option<serde_json::Value>, Option<u64>) -> Fut,
    Fut: std::future::Future<Output = Result<EndpointResponse, String>>,
{
    // 1. Mandatory parameter validation: collection & endpoint_str
    if req.collection.trim().is_empty() || req.endpoint_str.trim().is_empty() {
        let mut result = ApiResult::new(ApiStatus::MissingParam);
        result.detail = Some("Mandatory parameter 'collection' or 'endpoint_str' is missing".to_string());
        return CollectionOpResponse {
            result,
            collection_id: None,
            response: None,
        };
    }

    // 2. Security validation: prevent path traversal in collection name
    let valid_col_name = req.collection.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid_col_name {
        let mut result = ApiResult::new(ApiStatus::InvalidParam);
        result.detail = Some("Collection name contains invalid characters; must be alphanumeric, '-' or '_'".to_string());
        return CollectionOpResponse {
            result,
            collection_id: None,
            response: None,
        };
    }

    match req.ops.as_str() {
        "insert" => {
            let response_body = match req.response_body {
                Some(ref b) if !b.is_null() => b,
                _ => {
                    let mut result = ApiResult::new(ApiStatus::MissingParam);
                    result.detail = Some("Mandatory parameter 'response_body' is missing for insert op".to_string());
                    return CollectionOpResponse {
                        result,
                        collection_id: None,
                        response: None,
                    };
                }
            };

            // Resolve or create collection
            let col_already_exists = storage.collection_exists(&req.collection);
            let col_id = if col_already_exists {
                format!("col_{}", req.collection)
            } else {
                match storage.create_collection(&req.collection, req.collection_annotation.as_deref()) {
                    Ok(id) => id,
                    Err(e) => {
                        let mut result = ApiResult::new(ApiStatus::InternalError);
                        result.detail = Some(format!("Failed to create collection: {}", e));
                        return CollectionOpResponse {
                            result,
                            collection_id: None,
                            response: None,
                        };
                    }
                }
            };
            let collection_created = !col_already_exists;

            // Resolve or create endpoint
            let method = req.method.as_deref().unwrap_or("POST");
            let ep_already_exists = storage.endpoint_exists(&req.endpoint_str);
            let ep_id = if ep_already_exists {
                let clean = sanitize_eid(&req.endpoint_str);
                format!("ep_{}", clean)
            } else {
                match storage.create_endpoint(&req.endpoint_str, method, req.annotation.as_deref()) {
                    Ok(id) => id,
                    Err(e) => {
                        let mut result = ApiResult::new(ApiStatus::InternalError);
                        result.collection_created = Some(collection_created);
                        result.detail = Some(format!("Failed to create endpoint: {}", e));
                        return CollectionOpResponse {
                            result,
                            collection_id: Some(col_id),
                            response: None,
                        };
                    }
                }
            };
            let endpoint_created = !ep_already_exists;

            let qp_payload = serde_json::json!({
                "endpoint": req.endpoint_str,
                "endpoint_id": ep_id,
                "method": method,
                "request_body": req.request_body,
                "response_body": response_body,
                "headers": req.headers,
            });

            // Duplicate detection check
            if let Some(dup_num) = storage.is_duplicate(&col_id, &qp_payload) {
                let mut result = ApiResult::new(ApiStatus::DuplicateQp);
                result.endpoint_id = Some(ep_id);
                result.duplicate_of = Some(dup_num);
                result.endpoint_created = Some(endpoint_created);
                result.collection_created = Some(collection_created);
                result.detail = Some("Duplicate QP detected; redundant write skipped".to_string());
                return CollectionOpResponse {
                    result,
                    collection_id: Some(col_id),
                    response: None,
                };
            }

            // Insert QP record
            match storage.insert_qp(&col_id, &ep_id, &qp_payload) {
                Ok(req_num) => {
                    let mut result = ApiResult::new(ApiStatus::Ok);
                    result.endpoint_id = Some(ep_id);
                    result.request_number = Some(req_num);
                    result.endpoint_created = Some(endpoint_created);
                    result.collection_created = Some(collection_created);
                    CollectionOpResponse {
                        result,
                        collection_id: Some(col_id),
                        response: None,
                    }
                }
                Err(e) => {
                    let mut result = ApiResult::new(ApiStatus::InternalError);
                    result.endpoint_id = Some(ep_id);
                    result.endpoint_created = Some(endpoint_created);
                    result.collection_created = Some(collection_created);
                    result.detail = Some(format!("Failed to store QP: {}", e));
                    CollectionOpResponse {
                        result,
                        collection_id: Some(col_id),
                        response: None,
                    }
                }
            }
        }

        "test_and_insert" => {
            let method = match req.method {
                Some(ref m) if !m.trim().is_empty() => m.clone(),
                _ => {
                    let mut result = ApiResult::new(ApiStatus::MissingParam);
                    result.detail = Some("Mandatory parameter 'method' is missing for test_and_insert".to_string());
                    return CollectionOpResponse {
                        result,
                        collection_id: None,
                        response: None,
                    };
                }
            };

            // Execute HTTP test call
            let test_result = http_client(
                req.endpoint_str.clone(),
                method.clone(),
                req.request_body.clone(),
                req.timeout_ms,
            )
            .await;

            let ep_res = match test_result {
                Ok(resp) => resp,
                Err(err_msg) => {
                    let lower = err_msg.to_lowercase();
                    let status = if lower.contains("timeout") || lower.contains("timed out") {
                        ApiStatus::TestTimedOut
                    } else {
                        ApiStatus::TestFailed
                    };
                    let mut result = ApiResult::new(status);
                    result.detail = Some(err_msg);
                    return CollectionOpResponse {
                        result,
                        collection_id: None,
                        response: None,
                    };
                }
            };

            // Resolve or create collection
            let col_already_exists = storage.collection_exists(&req.collection);
            let col_id = if col_already_exists {
                format!("col_{}", req.collection)
            } else {
                match storage.create_collection(&req.collection, req.collection_annotation.as_deref()) {
                    Ok(id) => id,
                    Err(e) => {
                        let mut result = ApiResult::new(ApiStatus::InternalError);
                        result.detail = Some(format!("Failed to create collection: {}", e));
                        return CollectionOpResponse {
                            result,
                            collection_id: None,
                            response: None,
                        };
                    }
                }
            };
            let collection_created = !col_already_exists;

            // Resolve or create endpoint
            let ep_already_exists = storage.endpoint_exists(&req.endpoint_str);
            let ep_id = if ep_already_exists {
                let clean = sanitize_eid(&req.endpoint_str);
                format!("ep_{}", clean)
            } else {
                match storage.create_endpoint(&req.endpoint_str, &method, req.annotation.as_deref()) {
                    Ok(id) => id,
                    Err(e) => {
                        let mut result = ApiResult::new(ApiStatus::InternalError);
                        result.collection_created = Some(collection_created);
                        result.detail = Some(format!("Failed to create endpoint: {}", e));
                        return CollectionOpResponse {
                            result,
                            collection_id: Some(col_id),
                            response: None,
                        };
                    }
                }
            };
            let endpoint_created = !ep_already_exists;

            let qp_payload = serde_json::json!({
                "endpoint": req.endpoint_str,
                "endpoint_id": ep_id,
                "method": method,
                "request_body": req.request_body,
                "response_body": ep_res.body,
                "response_status": ep_res.status,
                "headers": ep_res.headers,
            });

            let returned_resp = if req.return_response == Some(true) {
                Some(ep_res.clone())
            } else {
                None
            };

            // Insert QP record
            match storage.insert_qp(&col_id, &ep_id, &qp_payload) {
                Ok(req_num) => {
                    let mut result = ApiResult::new(ApiStatus::Ok);
                    result.endpoint_id = Some(ep_id);
                    result.request_number = Some(req_num);
                    result.endpoint_created = Some(endpoint_created);
                    result.collection_created = Some(collection_created);
                    CollectionOpResponse {
                        result,
                        collection_id: Some(col_id),
                        response: returned_resp,
                    }
                }
                Err(e) => {
                    let mut result = ApiResult::new(ApiStatus::InternalError);
                    result.endpoint_id = Some(ep_id);
                    result.endpoint_created = Some(endpoint_created);
                    result.collection_created = Some(collection_created);
                    result.detail = Some(format!("Failed to store QP: {}", e));
                    CollectionOpResponse {
                        result,
                        collection_id: Some(col_id),
                        response: returned_resp,
                    }
                }
            }
        }

        unknown => {
            let mut result = ApiResult::new(ApiStatus::UnknownOp);
            result.detail = Some(format!("Unknown op '{}', expected 'insert' or 'test_and_insert'", unknown));
            CollectionOpResponse {
                result,
                collection_id: None,
                response: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct TestDb {
        collections: HashMap<String, String>,
        endpoints: HashMap<String, String>,
        data: Vec<(String, String, serde_json::Value)>,
        next_request_number: i32,
    }

    impl TestDb {
        fn new() -> Self {
            Self {
                collections: HashMap::new(),
                endpoints: HashMap::new(),
                data: Vec::new(),
                next_request_number: 1,
            }
        }
    }

    impl StorageBackend for TestDb {
        fn collection_exists(&self, name: &str) -> bool {
            self.collections.contains_key(name)
        }
        fn create_collection(&mut self, name: &str, _annotation: Option<&str>) -> Result<String, String> {
            let id = format!("col_{}", name);
            self.collections.insert(name.to_string(), id.clone());
            Ok(id)
        }
        fn endpoint_exists(&self, endpoint_str: &str) -> bool {
            self.endpoints.contains_key(endpoint_str)
        }
        fn create_endpoint(&mut self, endpoint_str: &str, _method: &str, _annotation: Option<&str>) -> Result<String, String> {
            let id = format!("ep_{}", sanitize_eid(endpoint_str));
            self.endpoints.insert(endpoint_str.to_string(), id.clone());
            Ok(id)
        }
        fn is_duplicate(&self, col_id: &str, qp_data: &serde_json::Value) -> Option<i32> {
            for (cid, _, val) in &self.data {
                if cid == col_id && val == qp_data {
                    return Some(1);
                }
            }
            None
        }
        fn insert_qp(&mut self, col_id: &str, endpoint_id: &str, qp_data: &serde_json::Value) -> Result<i32, String> {
            self.data.push((col_id.to_string(), endpoint_id.to_string(), qp_data.clone()));
            let num = self.next_request_number;
            self.next_request_number += 1;
            Ok(num)
        }
    }

    #[tokio::test]
    async fn test_missing_mandatory_params() {
        let mut db = TestDb::new();
        let req = CollectionOpRequest {
            ops: "insert".to_string(),
            endpoint_str: "".to_string(),
            collection: "auth".to_string(),
            method: None,
            request_body: None,
            response_body: Some(serde_json::json!({"status": 200})),
            headers: None,
            timeout_ms: None,
            annotation: None,
            tags: None,
            collection_annotation: None,
            collection_tags: None,
            return_response: None,
        };
        let res = process_collection_op(req, &mut db, |_, _, _, _| async {
            unreachable!()
        })
        .await;
        assert_eq!(res.result.status, ApiStatus::MissingParam);
    }

    #[tokio::test]
    async fn test_path_traversal_in_collection_rejected() {
        let mut db = TestDb::new();
        let req = CollectionOpRequest {
            ops: "insert".to_string(),
            endpoint_str: "/api/v1/test".to_string(),
            collection: "../malicious_dir".to_string(),
            method: None,
            request_body: None,
            response_body: Some(serde_json::json!({"status": 200})),
            headers: None,
            timeout_ms: None,
            annotation: None,
            tags: None,
            collection_annotation: None,
            collection_tags: None,
            return_response: None,
        };
        let res = process_collection_op(req, &mut db, |_, _, _, _| async {
            unreachable!()
        })
        .await;
        assert_eq!(res.result.status, ApiStatus::InvalidParam);
    }

    #[tokio::test]
    async fn test_unknown_op_rejected() {
        let mut db = TestDb::new();
        let req = CollectionOpRequest {
            ops: "delete_everything".to_string(),
            endpoint_str: "/api/v1/test".to_string(),
            collection: "my_collection".to_string(),
            method: None,
            request_body: None,
            response_body: Some(serde_json::json!({"status": 200})),
            headers: None,
            timeout_ms: None,
            annotation: None,
            tags: None,
            collection_annotation: None,
            collection_tags: None,
            return_response: None,
        };
        let res = process_collection_op(req, &mut db, |_, _, _, _| async {
            unreachable!()
        })
        .await;
        assert_eq!(res.result.status, ApiStatus::UnknownOp);
    }

    #[tokio::test]
    async fn test_insert_creates_collection_and_endpoint() {
        let mut db = TestDb::new();
        let req = CollectionOpRequest {
            ops: "insert".to_string(),
            endpoint_str: "/api/v1/login".to_string(),
            collection: "auth_suite".to_string(),
            method: Some("POST".to_string()),
            request_body: Some(serde_json::json!({"user": "alice"})),
            response_body: Some(serde_json::json!({"token": "xyz"})),
            headers: None,
            timeout_ms: None,
            annotation: None,
            tags: None,
            collection_annotation: None,
            collection_tags: None,
            return_response: None,
        };
        let res = process_collection_op(req, &mut db, |_, _, _, _| async {
            unreachable!()
        })
        .await;
        assert_eq!(res.result.status, ApiStatus::Ok);
        assert_eq!(res.result.collection_created, Some(true));
        assert_eq!(res.result.endpoint_created, Some(true));
        assert_eq!(res.result.request_number, Some(1));
    }

    #[tokio::test]
    async fn test_insert_duplicate_blocked() {
        let mut db = TestDb::new();
        let req = CollectionOpRequest {
            ops: "insert".to_string(),
            endpoint_str: "/api/v1/login".to_string(),
            collection: "auth_suite".to_string(),
            method: Some("POST".to_string()),
            request_body: Some(serde_json::json!({"user": "alice"})),
            response_body: Some(serde_json::json!({"token": "xyz"})),
            headers: None,
            timeout_ms: None,
            annotation: None,
            tags: None,
            collection_annotation: None,
            collection_tags: None,
            return_response: None,
        };
        let res1 = process_collection_op(req.clone(), &mut db, |_, _, _, _| async { unreachable!() }).await;
        assert_eq!(res1.result.status, ApiStatus::Ok);

        // Second identical insert
        let res2 = process_collection_op(req, &mut db, |_, _, _, _| async { unreachable!() }).await;
        assert_eq!(res2.result.status, ApiStatus::DuplicateQp);
        assert_eq!(res2.result.collection_created, Some(false));
        assert_eq!(res2.result.endpoint_created, Some(false));
        assert_eq!(res2.result.duplicate_of, Some(1));
    }

    #[tokio::test]
    async fn test_test_and_insert_with_response_mode() {
        let mut db = TestDb::new();
        let req = CollectionOpRequest {
            ops: "test_and_insert".to_string(),
            endpoint_str: "https://httpbin.org/get".to_string(),
            collection: "httpbin_suite".to_string(),
            method: Some("GET".to_string()),
            request_body: None,
            response_body: None,
            headers: None,
            timeout_ms: Some(5000),
            annotation: None,
            tags: None,
            collection_annotation: None,
            collection_tags: None,
            return_response: Some(true),
        };

        let res = process_collection_op(req, &mut db, |url, method, _, _| async move {
            assert_eq!(url, "https://httpbin.org/get");
            assert_eq!(method, "GET");
            Ok(EndpointResponse {
                status: 200,
                headers: serde_json::json!({"content-type": "application/json"}),
                body: "{\"origin\": \"127.0.0.1\"}".to_string(),
            })
        })
        .await;

        assert_eq!(res.result.status, ApiStatus::Ok);
        assert_eq!(res.result.request_number, Some(1));
        assert!(res.response.is_some());
        assert_eq!(res.response.as_ref().unwrap().status, 200);
        assert_eq!(res.response.as_ref().unwrap().body, "{\"origin\": \"127.0.0.1\"}");
    }

    #[tokio::test]
    async fn test_test_and_insert_timeout_error() {
        let mut db = TestDb::new();
        let req = CollectionOpRequest {
            ops: "test_and_insert".to_string(),
            endpoint_str: "https://unreachable.test".to_string(),
            collection: "timeout_suite".to_string(),
            method: Some("GET".to_string()),
            request_body: None,
            response_body: None,
            headers: None,
            timeout_ms: Some(1000),
            annotation: None,
            tags: None,
            collection_annotation: None,
            collection_tags: None,
            return_response: None,
        };

        let res = process_collection_op(req, &mut db, |_, _, _, _| async move {
            Err("Connection timed out after 1000ms".to_string())
        })
        .await;

        assert_eq!(res.result.status, ApiStatus::TestTimedOut);
    }

    #[tokio::test]
    async fn test_real_disk_persistence_and_file_layout() {
        let temp_dir = std::env::temp_dir().join("edms_disk_test");
        let _ = fs::remove_dir_all(&temp_dir);
        let mut disk_storage = EdmsStorage::new(&temp_dir);

        let req = CollectionOpRequest {
            ops: "insert".to_string(),
            endpoint_str: "/api/disk/test".to_string(),
            collection: "disk_collection".to_string(),
            method: Some("POST".to_string()),
            request_body: Some(serde_json::json!({"input": 42})),
            response_body: Some(serde_json::json!({"output": 84})),
            headers: Some(serde_json::json!({"content-type": "application/json"})),
            timeout_ms: None,
            annotation: None,
            tags: None,
            collection_annotation: None,
            collection_tags: None,
            return_response: None,
        };

        let res = process_collection_op(req, &mut disk_storage, |_, _, _, _| async { unreachable!() }).await;
        assert_eq!(res.result.status, ApiStatus::Ok);
        assert_eq!(res.result.request_number, Some(1));

        // Verify that the files were actually physically created on disk!
        let col_folder = temp_dir.join("collections").join("disk_collection");
        assert!(col_folder.exists(), "Collection directory must exist on disk");
        assert!(col_folder.join("ep_apidisktest-request-1.json").exists(), "Request file must be written to disk");
        assert!(col_folder.join("ep_apidisktest-response-1.json").exists(), "Response file must be written to disk");
        assert!(col_folder.join("ep_apidisktest-headers-1.json").exists(), "Headers file must be written to disk");

        // Verify file contents
        let req_content = fs::read_to_string(col_folder.join("ep_apidisktest-request-1.json")).unwrap();
        assert!(req_content.contains("42"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_nested_spec_schema_deserialization() {
        let nested_json = serde_json::json!({
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

        let mut db = TestDb::new();
        let res = process_collection_op(req, &mut db, |_, _, _, _| async { unreachable!() }).await;
        assert_eq!(res.result.status, ApiStatus::Ok);
        assert_eq!(res.result.collection_created, Some(true));
        assert_eq!(res.result.endpoint_created, Some(true));
        assert_eq!(res.result.request_number, Some(1));
    }
}
