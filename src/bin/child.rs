

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead};
use std::time::Instant;

// ── Protocol types (mirrored from parent's ipc.rs) ──────────────────

#[derive(Debug, Deserialize)]
struct IpcRequest {
    task: String,
    payload: Value,
    callback_port: u16,
}

#[derive(Debug, Serialize)]
struct IpcCallback {
    task: String,
    result: Value,
    elapsed_ms: u128,
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

// ── Main ─────────────────────────────────────────────────────────────

fn main() {
    let stdin = io::stdin();
    let line = match stdin.lock().lines().next() {
        Some(Ok(l)) => l,
        Some(Err(e)) => {
            eprintln!("[child] Failed to read stdin: {e}");
            std::process::exit(1);
        }
        None => {
            eprintln!("[child] Empty stdin, nothing to do");
            std::process::exit(0);
        }
    };

    let request: IpcRequest = match serde_json::from_str(&line) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[child] Failed to parse IpcRequest: {e}");
            std::process::exit(1);
        }
    };

    eprintln!(
        "[child] Received task='{}' callback_port={}",
        request.task, request.callback_port
    );

    let start = Instant::now();

    // Dispatch to the right handler
    let result = match request.task.as_str() {
        "run_test" => handle_run_test(&request.payload),
        "export" => handle_export(&request.payload),
        "delete_folder" => handle_delete_folder(&request.payload),
        "merge_folder" => handle_merge_folder(&request.payload),
        other => Err(format!("Unknown task: {other}")),
    };

    let elapsed_ms = start.elapsed().as_millis();

    // Build the callback
    let callback = match result {
        Ok(result_value) => IpcCallback {
            task: request.task.clone(),
            result: result_value,
            elapsed_ms,
            success: true,
            error: None,
        },
        Err(err_msg) => IpcCallback {
            task: request.task.clone(),
            result: json!({}),
            elapsed_ms,
            success: false,
            error: Some(err_msg),
        },
    };

    // POST back to the parent
    let url = format!(
        "http://127.0.0.1:{}/internal/callback",
        request.callback_port
    );

    eprintln!(
        "[child] task='{}' success={} elapsed={}ms, posting to {}",
        callback.task, callback.success, elapsed_ms, url
    );

    let client = reqwest::blocking::Client::new();
    match client.post(&url).json(&callback).send() {
        Ok(resp) => {
            eprintln!("[child] Callback sent, status={}", resp.status());
        }
        Err(e) => {
            eprintln!("[child] Failed to send callback: {e}");
        }
    }
}

// ── Task handlers ────────────────────────────────────────────────────

/// Executes an HTTP request against the target endpoint and returns the result.
fn handle_run_test(payload: &Value) -> Result<Value, String> {
    let endpoint_id = payload["endpoint_id"]
        .as_str()
        .ok_or("missing endpoint_id")?
        .to_string();
    let url = payload["url"]
        .as_str()
        .ok_or("missing url")?
        .to_string();
    let method = payload["method"]
        .as_str()
        .unwrap_or("GET")
        .to_uppercase();
    let request_number = payload["request_number"]
        .as_i64()
        .ok_or("missing request_number")? as i32;
    let body = payload.get("body").cloned().unwrap_or(json!({}));
    let timeout_ms = payload["timeout_ms"].as_u64().unwrap_or(30_000);

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_millis(timeout_ms))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let start = Instant::now();

    let req = match method.as_str() {
        "POST" => client.post(&url).json(&body),
        "PUT" => client.put(&url).json(&body),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url).json(&body),
        "HEAD" => client.head(&url),
        _ => client.get(&url),
    };

    match req.send() {
        Ok(resp) => {
            let elapsed = start.elapsed().as_millis() as i64;
            let status_code = resp.status().as_u16() as i64;
            let response_body = resp.text().unwrap_or_default();

            // Write response to file
            // The edms_data directory structure: edms_data/{endpoint_id}/response-{NNN}.json
            let dir = format!("edms_data/{}", endpoint_id);
            let _ = std::fs::create_dir_all(&dir);
            let response_file = format!("{}/response-{:03}.json", dir, request_number);
            let _ = std::fs::write(&response_file, &response_body);

            Ok(json!({
                "endpoint_id": endpoint_id,
                "request_number": request_number,
                "status_code": status_code,
                "response_time_ms": elapsed,
                "response_file": response_file,
                "timed_out": false,
            }))
        }
        Err(e) => {
            if e.is_timeout() {
                Ok(json!({
                    "endpoint_id": endpoint_id,
                    "request_number": request_number,
                    "timed_out": true,
                }))
            } else {
                Err(format!("HTTP request failed: {e}"))
            }
        }
    }
}


fn handle_export(payload: &Value) -> Result<Value, String> {
    let collection = payload["collection"]
        .as_str()
        .unwrap_or("unknown");

    eprintln!("[child] Export task for collection='{}' — placeholder", collection);

    
    Ok(json!({
        "collection": collection,
        "status": "export_placeholder",
    }))
}

/// Placeholder for folder deletion.
fn handle_delete_folder(payload: &Value) -> Result<Value, String> {
    let folder = payload["folder"]
        .as_str()
        .ok_or("missing folder")?
        .to_string();

    eprintln!("[child] Delete folder task for '{}' — placeholder", folder);

    
    Ok(json!({
        "folder": folder,
        "status": "delete_placeholder",
    }))
}

/// Placeholder for folder merge.
fn handle_merge_folder(payload: &Value) -> Result<Value, String> {
    let folder = payload["folder"]
        .as_str()
        .ok_or("missing folder")?
        .to_string();

    eprintln!("[child] Merge folder task for '{}' — placeholder", folder);

    Ok(json!({
        "folder": folder,
        "status": "merge_placeholder",
    }))
}