// compute/tests/ipc_child_test.rs

use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;
use tempfile::tempdir;
use tokio::time::timeout;

type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct IpcCallback {
    task: String,
    result: Value,
    elapsed_ms: u128,
    success: bool,
    error: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct IpcRequest {
    task: String,
    payload: Value,
    callback_port: u16,
}

struct MockCallbackServer {
    port: u16,
    callback_receiver: tokio::sync::mpsc::UnboundedReceiver<IpcCallback>,
    callback_sender: tokio::sync::mpsc::UnboundedSender<IpcCallback>,
}

impl MockCallbackServer {
    async fn start() -> Self {
        use warp::Filter;

        let port = portpicker::pick_unused_port().expect("No free port");
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<IpcCallback>();

        // Clone tx BEFORE moving it into the closure.
        // The closure captures tx_for_route and is called once per request.
        // Inside each call, we clone again for the async tokio::spawn.
        let tx_for_route = tx.clone();

        let callback_route = warp::path("internal")
            .and(warp::path("callback"))
            .and(warp::post())
            .and(warp::body::json())
            .map(move |callback: IpcCallback| {
                // Clone inside the closure so each spawned task gets its own handle.
                // The closure itself retains tx_for_route for the next request.
                let tx = tx_for_route.clone();
                tokio::spawn(async move {
                    let _ = tx.send(callback);
                });
                warp::reply::with_status("OK", warp::http::StatusCode::OK)
            });

        tokio::spawn(async move {
            warp::serve(callback_route)
                .run(([127, 0, 0, 1], port))
                .await;
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        Self {
            port,
            callback_receiver: rx,
            // tx is still valid here since we only cloned it above, never moved it
            callback_sender: tx,
        }
    }

    async fn wait_for_callback(&mut self, timeout_secs: u64) -> Option<IpcCallback> {
        timeout(
            Duration::from_secs(timeout_secs),
            self.callback_receiver.recv(),
        )
        .await
        .ok()
        .flatten()
    }
}

async fn spawn_child_and_send_request(
    task: &str,
    payload: Value,
    callback_port: u16,
) -> TestResult {
    // Get the path to the child binary
    let child_bin = if std::path::Path::new("./target/debug/edms-child").exists() {
        "./target/debug/edms-child"
    } else if std::path::Path::new("./target/release/edms-child").exists() {
        "./target/release/edms-child"
    } else {
        // Try to build it first
        let status = Command::new("cargo")
            .args(&["build", "--bin", "edms-child"])
            .status()?;
        if !status.success() {
            return Err("Failed to build child binary".into());
        }
        "./target/debug/edms-child"
    };

    let request = IpcRequest {
        task: task.to_string(),
        payload,
        callback_port,
    };

    let request_json = serde_json::to_string(&request)?;

    let mut child = Command::new(child_bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    // Send request to child's stdin
    if let Some(mut stdin) = child.stdin.take() {
        writeln!(stdin, "{}", request_json)?;
        stdin.flush()?;
    }

    // Don't wait for child - it will detach
    tokio::spawn(async move {
        let _ = child.wait();
    });

    Ok(())
}

#[tokio::test]
async fn test_child_export_collection() -> TestResult {
    let temp_dir = tempdir()?;
    let source = temp_dir.path().join("source");
    let output = temp_dir.path().join("output.zip");

    // Create test files
    std::fs::create_dir_all(&source)?;
    std::fs::write(source.join("test.txt"), "test content")?;

    // Start mock callback server
    let mut callback_server = MockCallbackServer::start().await;

    // Prepare payload
    let payload = json!({
        "source": source.to_string_lossy(),
        "output": output.to_string_lossy()
    });

    // Spawn child with request
    spawn_child_and_send_request("export_collection", payload, callback_server.port).await?;

    // Wait for callback
    let callback = callback_server.wait_for_callback(5).await;

    // Verify callback was received
    assert!(callback.is_some(), "Should receive callback from child");
    let callback = callback.unwrap();
    assert!(
        callback.success,
        "Task should succeed: {:?}",
        callback.error
    );

    // Verify output was created
    assert!(output.exists(), "Output zip file should exist");
    assert!(
        output.metadata()?.len() > 0,
        "Output zip should not be empty"
    );

    Ok(())
}

#[tokio::test]
async fn test_child_mark_active_folder() -> TestResult {
    let temp_dir = tempdir()?;
    let session_backup = temp_dir.path().join("session-backup");
    let active_folder = temp_dir.path().join("active");
    let config_path = temp_dir.path().join("config.yaml");
    let folder_name = "test_folder";

    // Create source folder with content
    let source = session_backup.join(folder_name);
    std::fs::create_dir_all(&source)?;
    std::fs::write(source.join("data.txt"), "test data")?;

    // Start mock callback server
    let mut callback_server = MockCallbackServer::start().await;

    // Prepare payload
    let payload = json!({
        "session_backup": session_backup.to_string_lossy(),
        "active_folder": active_folder.to_string_lossy(),
        "folder_name": folder_name,
        "yaml_config_path": config_path.to_string_lossy()
    });

    // Spawn child
    spawn_child_and_send_request("mark_active_folder", payload, callback_server.port).await?;

    // Wait for callback
    let callback = callback_server.wait_for_callback(5).await;
    assert!(callback.is_some(), "Should receive callback");
    let callback = callback.unwrap();
    assert!(callback.success, "Task failed: {:?}", callback.error);

    // Verify operation succeeded
    assert!(
        active_folder.join(folder_name).exists(),
        "Active folder should exist"
    );
    assert!(
        active_folder.join(folder_name).join("data.txt").exists(),
        "Data file should be copied"
    );
    assert!(config_path.exists(), "Config file should be created");

    Ok(())
}

#[tokio::test]
async fn test_child_import_zip() -> TestResult {
    let temp_dir = tempdir()?;
    let source_dir = temp_dir.path().join("source");
    let dest_dir = temp_dir.path().join("dest");
    let zip_path = temp_dir.path().join("test.zip");

    // Create test zip
    std::fs::create_dir_all(&source_dir)?;
    std::fs::write(source_dir.join("test.txt"), "content")?;

    // Use zipops to create zip
    compute::zipops::zip_folder(&source_dir, &zip_path)?;

    // Start mock callback server
    let mut callback_server = MockCallbackServer::start().await;

    // Prepare payload
    let payload = json!({
        "zip": zip_path.to_string_lossy(),
        "destination": dest_dir.to_string_lossy()
    });

    // Spawn child
    spawn_child_and_send_request("import_zip", payload, callback_server.port).await?;

    // Wait for callback
    let callback = callback_server.wait_for_callback(5).await;
    assert!(callback.is_some());
    let callback = callback.unwrap();
    assert!(callback.success, "Import failed: {:?}", callback.error);

    // Verify import was successful
    assert!(
        dest_dir.join("test.txt").exists(),
        "Imported file should exist"
    );

    Ok(())
}

#[tokio::test]
async fn test_child_unknown_task() -> TestResult {
    let temp_dir = tempdir()?;

    // Start mock callback server
    let mut callback_server = MockCallbackServer::start().await;

    // Prepare payload with unknown task
    let payload = json!({});

    // Spawn child with unknown task
    spawn_child_and_send_request("unknown_task_xyz", payload, callback_server.port).await?;

    // Wait for callback (should still receive something)
    let callback = callback_server.wait_for_callback(5).await;

    // Child should send a callback indicating failure
    if let Some(callback) = callback {
        assert!(!callback.success, "Unknown task should fail");
        assert!(callback.error.is_some(), "Should have error message");
        assert!(callback.error.unwrap().contains("unknown task"));
    }

    Ok(())
}
