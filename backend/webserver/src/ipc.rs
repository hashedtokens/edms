//! IPC passthrough infrastructure.
//!
//! Any handler can call `spawn_child(task, payload, callback_port)` to offload
//! compute to a detached child process. The child reads its assignment from
//! stdin, does the work, and POSTs the result back to `/internal/callback`.
//!
//! The parent never waits — this is fire-and-forget.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::{Command, Stdio};
use tracing::{error, info};

// ── Protocol types (shared between parent & child) ───────────────────

/// What the parent sends to the child via stdin (one JSON line).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcRequest {
   
    pub task: String,
    /// Arbitrary JSON payload — each task defines its own shape.
    pub payload: serde_json::Value,
    /// Port the parent is listening on (child POSTs result here).
    pub callback_port: u16,
}

/// What the child POSTs back to `/internal/callback`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcCallback {
    /// Which task produced this result.
    pub task: String,
    /// Arbitrary JSON result — each task defines its own shape.
    pub result: serde_json::Value,
    /// How long the child spent on the task (ms).
    pub elapsed_ms: u128,
    /// Whether the task succeeded.
    pub success: bool,
    /// Error message if `success` is false.
    #[serde(default)]
    pub error: Option<String>,
}

// ── Spawn helper ─────────────────────────────────────────────────────

pub fn spawn_child(task: &str, payload: serde_json::Value, callback_port: u16) {
    let request = IpcRequest {
        task: task.to_string(),
        payload,
        callback_port,
    };

    let request_json = match serde_json::to_string(&request) {
        Ok(j) => j,
        Err(e) => {
            error!("[ipc] Failed to serialize IpcRequest: {e}");
            return;
        }
    };

    info!(
        "[ipc] Spawning child for task='{}', callback_port={}",
        task, callback_port
    );

    // Try the release binary first, fall back to debug
    let child_bin = if std::path::Path::new("./target/release/edms-child").exists() {
        "./target/release/edms-child"
    } else {
        "./target/debug/edms-child"
    };

    let spawn_result = Command::new(child_bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::null()) // detached — we don't read stdout
        .stderr(Stdio::inherit()) // keep stderr so child logs are visible
        .spawn();

    match spawn_result {
        Ok(mut child) => {
            // Write the request JSON to the child's stdin, then drop it
            if let Some(mut stdin) = child.stdin.take() {
                let _ = writeln!(stdin, "{}", request_json);
                let _ = stdin.flush();
                // stdin drops here — child is on its own
            }

           
            std::mem::forget(child);

            info!("[ipc] Child spawned and detached for task='{}'", task);
        }
        Err(e) => {
            error!(
                "[ipc] Failed to spawn child binary '{}': {e}. \
                 Make sure you've run `cargo build` so the binary exists.",
                child_bin
            );
        }
    }
}