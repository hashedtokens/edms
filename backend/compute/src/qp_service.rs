use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Volatile transport headers stripped during duplicate comparison.
pub const VOLATILE_HEADERS: &[&str] = &[
    "date",
    "age",
    "cf-ray",
    "cf-cache-status",
    "x-request-id",
    "request-id",
    "x-runtime",
    "x-response-time",
    "server-timing",
    "nel",
    "report-to",
    "x-ratelimit-reset",
    "x-ratelimit-remaining",
    "set-cookie",
    "alt-svc",
    "expires",
    "last-modified",
    "etag",
];

/// Recursively sorts all JSON object keys alphabetically to produce canonical JSON.
/// Does not alter primitives, arrays, or string values.
pub fn normalize(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted: BTreeMap<String, Value> = map
                .iter()
                .map(|(k, v)| (k.clone(), normalize(v)))
                .collect();
            let mut result = Map::new();
            for (k, v) in sorted {
                result.insert(k, v);
            }
            Value::Object(result)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(normalize).collect()),
        other => other.clone(),
    }
}

/// Parses and canonicalizes a JSON string into a compact, normalized representation.
/// If parsing fails or the input is empty, returns the trimmed string.
pub fn canonical(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(v) => serde_json::to_string(&normalize(&v)).unwrap_or_else(|_| trimmed.to_string()),
        Err(_) => trimmed.to_string(),
    }
}

/// Strips volatile transport headers from header JSON recursively.
pub fn sanitize_header_value(val: &mut Value) {
    match val {
        Value::Object(map) => {
            if map.contains_key("response_headers") || map.contains_key("request_headers") {
                if let Some(req_h) = map.get_mut("request_headers") {
                    sanitize_header_value(req_h);
                }
                if let Some(res_h) = map.get_mut("response_headers") {
                    sanitize_header_value(res_h);
                }
            } else {
                map.retain(|k, _| !VOLATILE_HEADERS.contains(&k.to_ascii_lowercase().as_str()));
            }
        }
        _ => {}
    }
}

/// Canonicalizes headers JSON and strips dynamic transport headers.
pub fn canonical_headers(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(mut v) => {
            sanitize_header_value(&mut v);
            serde_json::to_string(&normalize(&v)).unwrap_or_else(|_| trimmed.to_string())
        }
        Err(_) => trimmed.to_string(),
    }
}

/// Extracts and alphabetically sorts query parameter data string.
pub fn extract_qp_data(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
        if let Some(data) = val.get("data").and_then(|d| d.as_str()) {
            return extract_qp_data(data);
        }
    }

    let query_str = if let Some(pos) = trimmed.find('?') {
        &trimmed[pos + 1..]
    } else if trimmed.starts_with("http://") || trimmed.starts_with("https://") || trimmed.starts_with('/') {
        return String::new();
    } else if trimmed.contains('=') {
        trimmed
    } else {
        return String::new();
    };

    let mut pairs: Vec<&str> = query_str.split('&').filter(|s| !s.is_empty()).collect();
    pairs.sort();
    pairs.join("&")
}

/// Generates a canonical SHA-256 hash key for a request snapshot.
pub fn make_canonical_key(
    method: &str,
    endpoint: &str,
    q: &str,
    p: &str,
    h: &str,
    qp_data: &str,
) -> String {
    let cq = canonical(q);
    let cp = canonical(p);
    let ch = canonical_headers(h);

    let mut hasher = Sha256::new();
    for part in &[method, endpoint, &cq, &cp, &ch, qp_data] {
        let prefix = format!("{}:{}|", part.len(), part);
        hasher.update(prefix.as_bytes());
    }

    hex::encode(hasher.finalize())
}

/// Thread-safe in-memory store for tracking seen request snapshot hash keys.
#[derive(Clone, Default)]
pub struct DuplicateStore {
    seen: Arc<Mutex<HashSet<String>>>,
}

impl DuplicateStore {
    pub fn new() -> Self {
        Self {
            seen: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Checks if the key is stored, inserting it if new.
    pub fn add_if_new(&self, key: &str) -> bool {
        let mut set = self.seen.lock().unwrap();
        set.insert(key.to_string())
    }

    /// Removes a key from the set.
    pub fn remove(&self, key: &str) {
        let mut set = self.seen.lock().unwrap();
        set.remove(key);
    }

    /// Checks if a key exists in the store without inserting it.
    pub fn contains(&self, key: &str) -> bool {
        let set = self.seen.lock().unwrap();
        set.contains(key)
    }

    /// Returns the total count of keys currently in the store.
    pub fn count(&self) -> usize {
        let set = self.seen.lock().unwrap();
        set.len()
    }

    /// Clears all keys in the store.
    pub fn clear(&self) {
        let mut set = self.seen.lock().unwrap();
        set.clear();
    }

    /// Rebuilds the store on startup from storage files using the endpoint database metadata.
    pub fn rebuild_from_storage(
        &self,
        storage_root: &Path,
        endpoint_map: &HashMap<String, (String, String)>,
    ) -> usize {
        let candidates = [
            storage_root.join("storage").join("globalEQPData"),
            storage_root.join("globalEQPData"),
            std::path::PathBuf::from("/app/storage/globalEQPData"),
            std::path::PathBuf::from("/app/edms_root/storage/globalEQPData"),
            std::path::PathBuf::from("/app/edms_root/globalEQPData"),
        ];

        let mut loaded = 0;
        let mut set = self.seen.lock().unwrap();

        for eqp_root in &candidates {
            if !eqp_root.exists() {
                continue;
            }
            if let Ok(entries) = fs::read_dir(eqp_root) {
                for entry in entries.flatten() {
                    if !entry.path().is_dir() {
                        continue;
                    }
                    let eid = entry.file_name().to_string_lossy().to_string();
                    let folder = entry.path();
                    let numbers = list_existing_qp_numbers(&folder, &eid);

                    let (method, endpoint_str) = endpoint_map
                        .get(&eid)
                        .cloned()
                        .unwrap_or_else(|| ("GET".to_string(), String::new()));
                    let qp_data = extract_qp_data(&endpoint_str);

                    for num in numbers {
                        if let Some((q, p, h, _)) = read_qp_quadruple(&folder, &eid, num) {
                            let key = make_canonical_key(&method, &eid, &q, &p, &h, &qp_data);
                            set.insert(key);
                            loaded += 1;
                        }
                    }
                }
            }
        }

        loaded
    }
}

/// Helper to parse method and endpoint URL out of request JSON if available.
pub fn extract_method_and_endpoint(q_json: &str) -> (String, String) {
    if let Ok(v) = serde_json::from_str::<Value>(q_json) {
        let m = v.get("method").and_then(|s| s.as_str()).unwrap_or("GET").to_uppercase();
        let u = v.get("url").or_else(|| v.get("endpoint_str")).and_then(|s| s.as_str()).unwrap_or("").to_string();
        (m, u)
    } else {
        ("GET".to_string(), String::new())
    }
}

/// Reads the Q, P, H, and QP-data file contents for a specific QP number from disk.
pub fn read_qp_quadruple(folder: &Path, eid: &str, num: i32) -> Option<(String, String, String, String)> {
    let req_path = folder.join(format!("{eid}-request-{num}.json"));
    let res_path = folder.join(format!("{eid}-response-{num}.json"));
    let hdr_path = folder.join(format!("{eid}-headers-{num}.json"));

    let q = fs::read_to_string(&req_path).ok()?;
    let p = fs::read_to_string(&res_path).unwrap_or_else(|_| "{}".to_string());
    let h = fs::read_to_string(&hdr_path).unwrap_or_else(|_| "{}".to_string());

    let qp_file_1 = folder.join(format!("query-param-{num}.json"));
    let qp_file_2 = folder.join(format!("{eid}-query-param-{num}.json"));
    let raw_qp = fs::read_to_string(&qp_file_1)
        .or_else(|_| fs::read_to_string(&qp_file_2))
        .unwrap_or_default();
    let qp_data = extract_qp_data(&raw_qp);

    Some((q, p, h, qp_data))
}

/// Scans the endpoint storage directory and returns all existing QP request numbers.
pub fn list_existing_qp_numbers(folder: &Path, eid: &str) -> Vec<i32> {
    let mut numbers = Vec::new();
    let prefix = format!("{eid}-request-");
    let suffix = ".json";

    if let Ok(entries) = fs::read_dir(folder) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(&prefix) && name_str.ends_with(suffix) {
                let num_str = &name_str[prefix.len()..name_str.len() - suffix.len()];
                if let Ok(n) = num_str.parse::<i32>() {
                    numbers.push(n);
                }
            }
        }
    }
    numbers.sort_unstable();
    numbers
}

/// Finds matching duplicate QP number if an identical snapshot already exists on disk.
pub fn find_duplicate_qp(
    folder: &Path,
    eid: &str,
    method: &str,
    endpoint_str: &str,
    new_q: &str,
    new_p: &str,
    new_h: &str,
    exclude_request_number: Option<i32>,
) -> Option<i32> {
    if !folder.exists() {
        return None;
    }

    let qp_data = extract_qp_data(endpoint_str);
    let candidate_key = make_canonical_key(method, eid, new_q, new_p, new_h, &qp_data);
    let existing_numbers = list_existing_qp_numbers(folder, eid);

    for num in existing_numbers {
        if Some(num) == exclude_request_number {
            continue;
        }

        if let Some((q, p, h, qp_d)) = read_qp_quadruple(folder, eid, num) {
            let qp_use = if !qp_d.is_empty() { qp_d } else { qp_data.clone() };
            let existing_key = make_canonical_key(method, eid, &q, &p, &h, &qp_use);
            if candidate_key == existing_key {
                return Some(num);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_key_same_despite_key_order() {
        let q1 = r#"{"url":"https://api.test/users","method":"GET","body":{"b":2,"a":1}}"#;
        let q2 = r#"{"method":"GET","body":{"a":1,"b":2},"url":"https://api.test/users"}"#;
        let p = r#"{"status":200,"body":{"total":10}}"#;
        let h = r#"{"content-type":"application/json"}"#;
        let qp = "sort=listPrice,displayName";

        let key1 = make_canonical_key("GET", "/users", q1, p, h, qp);
        let key2 = make_canonical_key("GET", "/users", q2, p, h, qp);

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_canonical_key_differs_on_single_character() {
        let q = r#"{"method":"GET","url":"/users"}"#;
        let p1 = r#"{"status":200,"body":{"total":10}}"#;
        let p2 = r#"{"status":200,"body":{"total":11}}"#;
        let h = r#"{}"#;
        let qp = "sort=price";

        let key1 = make_canonical_key("GET", "/users", q, p1, h, qp);
        let key2 = make_canonical_key("GET", "/users", q, p2, h, qp);

        assert_ne!(key1, key2);
    }

    #[test]
    fn test_canonical_headers_ignores_timestamp_and_volatile_headers() {
        let h1 = r#"{
            "request_headers": {"Authorization": "Bearer 123"},
            "response_headers": {
                "Content-Type": "application/json",
                "Date": "Sun, 19 Oct 2026 05:22:15 GMT",
                "CF-RAY": "8d3e9102-DFW",
                "Age": "0",
                "nel": "report-to-nel",
                "x-ratelimit-reset": "12345"
            }
        }"#;

        let h2 = r#"{
            "request_headers": {"Authorization": "Bearer 123"},
            "response_headers": {
                "Content-Type": "application/json",
                "Date": "Sun, 19 Oct 2026 05:22:30 GMT",
                "CF-RAY": "8d3e9999-DFW",
                "Age": "15",
                "nel": "report-to-diff",
                "x-ratelimit-reset": "67890"
            }
        }"#;

        let key1 = make_canonical_key("GET", "/users", r#"{"q":1}"#, r#"{"status":200}"#, h1, "");
        let key2 = make_canonical_key("GET", "/users", r#"{"q":1}"#, r#"{"status":200}"#, h2, "");

        assert_eq!(key1, key2, "Volatile headers must not cause duplicate detection failure");
    }

    #[test]
    fn test_canonical_headers_still_differs_on_meaningful_header() {
        let h1 = r#"{"response_headers":{"Content-Type":"application/json","Date":"Sun, 19 Oct 2026 05:22:15 GMT"}}"#;
        let h2 = r#"{"response_headers":{"Content-Type":"text/html","Date":"Sun, 19 Oct 2026 05:22:15 GMT"}}"#;

        let key1 = make_canonical_key("GET", "/users", r#"{"q":1}"#, r#"{"status":200}"#, h1, "");
        let key2 = make_canonical_key("GET", "/users", r#"{"q":1}"#, r#"{"status":200}"#, h2, "");

        assert_ne!(key1, key2);
    }

    #[test]
    fn test_extract_qp_data_url_and_metadata() {
        assert_eq!(extract_qp_data("https://dummyjson.com/users"), "");
        assert_eq!(extract_qp_data("https://dummyjson.com/users?limit=30&skip=0"), "limit=30&skip=0");
        assert_eq!(extract_qp_data("https://dummyjson.com/users?skip=0&limit=30"), "limit=30&skip=0");
        assert_eq!(extract_qp_data(r#"{"data":"sort=price&category=electronics"}"#), "category=electronics&sort=price");
    }

    #[test]
    fn test_delete_removes_from_duplicate_store_and_allows_retest() {
        let store = DuplicateStore::new();
        let key = make_canonical_key("GET", "E001", "{}", "{}", "{}", "");
        assert!(store.add_if_new(&key));
        assert!(!store.add_if_new(&key)); // duplicate

        // User deletes QP
        store.remove(&key);
        // User re-runs same request -> now accepted!
        assert!(store.add_if_new(&key));
    }

    #[test]
    fn test_restart_rebuilds_keys_identically_for_get_with_query_and_post_with_body() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();
        let eqp_dir = root.join("storage").join("globalEQPData");
        let eid = "E0003-AAA";
        let folder = eqp_dir.join(eid);
        fs::create_dir_all(&folder).unwrap();

        // 1. Create a GET QP with query parameter ?x=1
        let get_req = "{}";
        let get_res = r#"{"data":"ok"}"#;
        let get_hdr = r#"{"request_headers":{},"response_headers":{}}"#;
        fs::write(folder.join(format!("{eid}-request-1.json")), get_req).unwrap();
        fs::write(folder.join(format!("{eid}-response-1.json")), get_res).unwrap();
        fs::write(folder.join(format!("{eid}-headers-1.json")), get_hdr).unwrap();

        // 2. Create a POST QP with body {"username":"admin"}
        let post_eid = "E0004-AAA";
        let post_folder = eqp_dir.join(post_eid);
        fs::create_dir_all(&post_folder).unwrap();
        let post_req = r#"{"username":"admin"}"#;
        let post_res = r#"{"token":"xyz"}"#;
        let post_hdr = r#"{"request_headers":{},"response_headers":{}}"#;
        fs::write(post_folder.join(format!("{post_eid}-request-1.json")), post_req).unwrap();
        fs::write(post_folder.join(format!("{post_eid}-response-1.json")), post_res).unwrap();
        fs::write(post_folder.join(format!("{post_eid}-headers-1.json")), post_hdr).unwrap();

        let mut ep_map = HashMap::new();
        ep_map.insert(eid.to_string(), ("GET".to_string(), "https://api.test/items?x=1".to_string()));
        ep_map.insert(post_eid.to_string(), ("POST".to_string(), "https://api.test/login".to_string()));

        // Run-time keys
        let runtime_get_key = make_canonical_key("GET", eid, get_req, get_res, get_hdr, &extract_qp_data("https://api.test/items?x=1"));
        let runtime_post_key = make_canonical_key("POST", post_eid, post_req, post_res, post_hdr, &extract_qp_data("https://api.test/login"));

        // Simulate server restart: rebuild store from storage
        let store = DuplicateStore::new();
        let count = store.rebuild_from_storage(root, &ep_map);
        assert_eq!(count, 2);

        // After restart: identical GET with ?x=1 MUST be detected!
        assert!(!store.add_if_new(&runtime_get_key));
        // After restart: identical POST with body MUST be detected!
        assert!(!store.add_if_new(&runtime_post_key));
    }
}
