//! edms-child — IPC child process for endpoint_mng_sys
//!
//! Reads one IpcRequest JSON line from stdin.
//! Dispatches to the appropriate handler.
//! POSTs IpcCallback result back to parent's /internal/callback.
//! Exits.

use serde_json::{json, Value};
use std::io::{self};
use std::time::Instant;

// Re-use all the handler inner functions and request types
use compute::api::handlers::{
    BookmarkRequest, CrudOperationsRequest, EndpointWriteRequest, ExportCollectionRequest,
    ExportMergeRequest, FileConversionRequest, HeadersDoc, ImportZipRequest,
    MarkActiveFolderRequest, MarkdownRequest, MetaRequest, RequestDoc, ResponseDoc, RunTestRequest,
    StaticCreateRequest, StaticExportRequest, compute_crud_operations_inner, convert_to_html_inner,
    convert_to_markdown_inner, convert_to_pdf_inner, create_static_inner, export_bookmarks_inner,
    export_collection_inner, export_merge_inner, export_static_inner, generate_markdown_inner,
    generate_meta_inner, import_zip_inner, mark_active_folder_inner, run_test_inner,
    write_endpoint_inner, write_headers_inner, write_request_inner, write_response_inner,
    tagops_merge_inner, tagops_create_inner, tagops_bulk_add_inner,
    tagops_bulk_remove_inner, tagops_rename_inner,
    remove_url_prefix_inner, RemoveUrlRequest,
    table_view_scan_inner, TableViewScanRequest,
    table_view_format_check_inner, TableViewFormatCheckRequest,
    table_view_move_inner, TableViewMoveRequest,
    table_view_takeout_inner, TableViewTakeoutRequest,
    audit_orphaned_eids_inner, AuditRequest,
    purge_orphaned_eids_inner, PurgeRequest,
};
use compute::tagops::{
    MergeRequest as TagMergeRequest, CreateFromTagsRequest,
    BulkTagRequest, RenameTagRequest,
};

// IpcRequest/IpcCallback are defined in Tara's ipc.rs.
// We redefine them here — they just need to match the JSON shape.
#[derive(serde::Deserialize)]
struct IpcRequest {
    task: String,
    payload: Value,
    callback_port: u16,
}

#[derive(serde::Serialize)]
struct IpcCallback {
    task: String,
    result: Value,
    elapsed_ms: u128,
    success: bool,
    error: Option<String>,
}

#[tokio::main]
async fn main() {
    use std::io::Read;
    // Log process ID for debugging
    eprintln!("[child] PID: {}", std::process::id());

    // Log current working directory
    eprintln!(
        "[child] Current dir: {:?}",
        std::env::current_dir().unwrap_or_default()
    );
    let mut buffer = String::new();
    io::stdin()
        .read_to_string(&mut buffer)
        .expect("failed to read stdin");

    let request: IpcRequest = serde_json::from_str(&buffer).expect("JSON parse error");

    // --- FIX STARTS HERE ---
    // 1. Get the absolute path to ensure we are in the right spot
    // Match the parent webserver's Docker storage configuration. Falling
    // back keeps the child usable in local, non-Docker development.
    let root = std::env::var_os("EDMS_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(compute::folder_manager::default_root_path);
    eprintln!("[DEBUG] Using root path: {:?}", root);
    // 2. Force create the base structure
    compute::folder_manager::verify_and_init(&root).expect("Init failed");

    // 3. If the task is mark_active_folder, we MUST ensure the subfolder exists
    if request.task == "mark_active_folder" {
        let folder_name = request.payload["folder_name"].as_str().unwrap_or("");
        // We force create the source folder so mark_active_folder doesn't crash
        let source_path = root.join("session-backup").join(folder_name);
        if !source_path.exists() {
            std::fs::create_dir_all(&source_path).expect("Failed to create source");
            // Create a dummy file so the folder isn't totally empty if needed
            std::fs::write(source_path.join(".gitkeep"), "").ok();
        }
    }
    let task = request.task.clone();
    let port = request.callback_port;
    let started_at = Instant::now();

    // 4. Dispatch to the right inner handler
    let view_type = request
        .payload
        .get("view_type")
        .or_else(|| request.payload.get("ViewType"))
        .cloned();

    // RepoView/WebView Import-Export jobs report progress while they run, so
    // they need the callback port; everything else just returns a result.
    let (success, mut result, error) = if is_view_io_task(&task) {
        dispatch_view_io(&task, request.payload, port).await
    } else {
        dispatch(&task, request.payload).await
    };

    // Preserve ViewType in callback result if specified in request payload
    if let Some(vt) = view_type {
        if let Some(obj) = result.as_object_mut() {
            obj.insert("view_type".to_string(), vt);
        }
    }

    let elapsed_ms = started_at.elapsed().as_millis();

    // 5. POST result back to parent
    let callback = IpcCallback {
        task: task.clone(),
        result,
        elapsed_ms,
        success,
        error,
    };

    let url = format!("http://127.0.0.1:{}/internal/callback", port);
    let client = reqwest::Client::new();

    if let Err(e) = client.post(&url).json(&callback).send().await {
        eprintln!("[edms-child] Failed to POST callback to {url}: {e}");
        std::process::exit(1);
    }

    eprintln!(
        "[edms-child] task='{}' done in {}ms, callback sent",
        task, elapsed_ms
    );
}
// ── RepoView / WebView Import-Export jobs ─────────────────────────────────
//
// The webserver gives each a `job_id` and expects it back in `result.job_id`
// - on failure too, which is why a failure here still carries a result.
// Progress goes out as extra callbacks with task `view_io_progress`.

fn is_view_io_task(task: &str) -> bool {
    matches!(
        task,
        "view_takeout" | "view_import_copy" | "view_unzip" | "view_table_check" | "collection_merge"
    )
}

async fn dispatch_view_io(task: &str, payload: Value, port: u16) -> (bool, Value, Option<String>) {
    let job_id = payload
        .get("job_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let fail = |e: String| (false, json!({ "job_id": job_id }), Some(e));

    match task {
        "view_takeout" => {
            let req: compute::view_io::ViewTakeoutRequest = match serde_json::from_value(payload) {
                Ok(r) => r,
                Err(e) => return fail(format!("payload deserialise error: {e}")),
            };
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
            let poster = tokio::spawn(post_progress(port, job_id.clone(), rx));
            let worked = tokio::task::spawn_blocking(move || {
                let mut report = |p: Value| {
                    let _ = tx.send(p);
                };
                compute::view_io::run_takeout(&req, &mut report)
            })
            .await;
            // the sender died with the worker, so the poster drains and ends
            let _ = poster.await;
            match worked {
                Ok(Ok(mut result)) => {
                    result["job_id"] = json!(job_id);
                    (true, result, None)
                }
                Ok(Err(e)) => fail(e),
                Err(e) => fail(e.to_string()),
            }
        }
        "view_import_copy" => {
            let req: compute::view_io::ViewImportCopyRequest = match serde_json::from_value(payload) {
                Ok(r) => r,
                Err(e) => return fail(format!("payload deserialise error: {e}")),
            };
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
            let poster = tokio::spawn(post_progress(port, job_id.clone(), rx));
            let worked = tokio::task::spawn_blocking(move || {
                let mut report = |p: Value| {
                    let _ = tx.send(p);
                };
                compute::view_io::run_import_copy(&req, &mut report)
            })
            .await;
            let _ = poster.await;
            match worked {
                Ok(Ok(mut result)) => {
                    result["job_id"] = json!(job_id);
                    (true, result, None)
                }
                Ok(Err(e)) => fail(e),
                Err(e) => fail(e.to_string()),
            }
        }
        "view_unzip" => {
            let req: compute::view_io::ViewUnzipRequest = match serde_json::from_value(payload) {
                Ok(r) => r,
                Err(e) => return fail(format!("payload deserialise error: {e}")),
            };
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
            let poster = tokio::spawn(post_progress(port, job_id.clone(), rx));
            let worked = tokio::task::spawn_blocking(move || {
                let mut report = |p: Value| {
                    let _ = tx.send(p);
                };
                compute::view_io::run_unzip(&req, &mut report)
            })
            .await;
            let _ = poster.await;
            match worked {
                Ok(Ok(mut result)) => {
                    result["job_id"] = json!(job_id);
                    (true, result, None)
                }
                Ok(Err(e)) => fail(e),
                Err(e) => fail(e.to_string()),
            }
        }
        "view_table_check" => {
            let req: compute::view_io::ViewCheckRequest = match serde_json::from_value(payload) {
                Ok(r) => r,
                Err(e) => return fail(format!("payload deserialise error: {e}")),
            };
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
            let poster = tokio::spawn(post_progress(port, job_id.clone(), rx));
            let worked = tokio::task::spawn_blocking(move || {
                let mut report = |p: Value| {
                    let _ = tx.send(p);
                };
                compute::view_io::run_check(&req, &mut report)
            })
            .await;
            let _ = poster.await;
            match worked {
                Ok(Ok(mut result)) => {
                    result["job_id"] = json!(job_id);
                    (true, result, None)
                }
                Ok(Err(e)) => fail(e),
                Err(e) => fail(e.to_string()),
            }
        }
        // Merge collections (whole, or only the endpoints carrying chosen
        // tags) into a new or existing collection. No progress reports: it
        // is quick, and the result says what happened.
        "collection_merge" => {
            let req: compute::tagops::collection_merge::CollectionMergeRequest = match serde_json::from_value(payload) {
                Ok(r) => r,
                Err(e) => return fail(format!("payload deserialise error: {e}")),
            };
            let worked = tokio::task::spawn_blocking(move || compute::tagops::collection_merge::run_collection_merge(&req)).await;
            match worked {
                Ok(Ok(mut result)) => {
                    result["job_id"] = json!(job_id);
                    (true, result, None)
                }
                Ok(Err(e)) => fail(e),
                Err(e) => fail(e.to_string()),
            }
        }
        other => fail(format!("unknown task: '{other}'")),
    }
}

async fn post_progress(port: u16, job_id: String, mut rx: tokio::sync::mpsc::UnboundedReceiver<Value>) {
    let url = format!("http://127.0.0.1:{port}/internal/callback");
    let client = reqwest::Client::new();
    while let Some(progress) = rx.recv().await {
        let callback = IpcCallback {
            task: "view_io_progress".to_string(),
            result: json!({ "job_id": job_id, "progress": progress }),
            elapsed_ms: 0,
            success: true,
            error: None,
        };
        // Progress is best effort: a lost one must never fail the job.
        let _ = client.post(&url).json(&callback).send().await;
    }
}

async fn dispatch(task: &str, payload: Value) -> (bool, Value, Option<String>) {
    match task {
        "export_collection" => {
            run(payload, |p: ExportCollectionRequest| {
                export_collection_inner(p)
            })
            .await
        }

        "export_merge" => run(payload, |p: ExportMergeRequest| export_merge_inner(p)).await,

        "import_zip" => run(payload, |p: ImportZipRequest| import_zip_inner(p)).await,

        "export_bookmarks" => run(payload, |p: BookmarkRequest| export_bookmarks_inner(p)).await,

        "create_static" => run(payload, |p: StaticCreateRequest| create_static_inner(p)).await,

        "export_static" => run(payload, |p: StaticExportRequest| export_static_inner(p)).await,

        // File-conversion recipe: deserialize the three-field payload, call the
        // matching compute function, then send its output_path in the callback.
        "convert_to_pdf" => run(payload, |p: FileConversionRequest| convert_to_pdf_inner(p)).await,

        "convert_to_html" => {
            run(payload, |p: FileConversionRequest| convert_to_html_inner(p)).await
        }

        "convert_to_markdown" => {
            run(payload, |p: FileConversionRequest| {
                convert_to_markdown_inner(p)
            })
            .await
        }

        "generate_markdown" => run(payload, |p: MarkdownRequest| generate_markdown_inner(p)).await,

        "generate_meta" => run(payload, |p: MetaRequest| generate_meta_inner(p)).await,

        "write_endpoint" => run(payload, |p: EndpointWriteRequest| write_endpoint_inner(p)).await,

        "write_request" => run(payload, |p: RequestDoc| write_request_inner(p)).await,

        "write_response" => run(payload, |p: ResponseDoc| write_response_inner(p)).await,

        "write_headers" => run(payload, |p: HeadersDoc| write_headers_inner(p)).await,

        "compute_crud_operations" => {
            run(payload, |p: CrudOperationsRequest| {
                compute_crud_operations_inner(p)
            })
            .await
        }

        "mark_active_folder" => {
            run(payload, |p: MarkActiveFolderRequest| async move {
                mark_active_folder_inner(p)
                    .await
                    .and_then(|r| serde_json::to_string(&r).map_err(|e| e.to_string()))
            })
            .await
        }

        "run_test" => run(payload, |p: RunTestRequest| run_test_inner(p)).await,

        // ── Tag operations ──────────────────────────────────────────────────
        // Each task receives a typed payload that includes db_path.
        // The inner fn runs on a blocking thread and returns the Activity Log
        // serialised as a JSON string, which becomes the IPC result value.

        "tagops_merge" => {
            run(payload, |p: TagMergeRequest| tagops_merge_inner(p)).await
        }

        "tagops_create_from_tags" => {
            run(payload, |p: CreateFromTagsRequest| tagops_create_inner(p)).await
        }

        "tagops_bulk_add" => {
            run(payload, |p: BulkTagRequest| tagops_bulk_add_inner(p)).await
        }

        "tagops_bulk_remove" => {
            run(payload, |p: BulkTagRequest| tagops_bulk_remove_inner(p)).await
        }

        "tagops_rename" => {
            run(payload, |p: RenameTagRequest| tagops_rename_inner(p)).await
        }

        "remove_url_prefix" => {
            run(payload, |p: RemoveUrlRequest| remove_url_prefix_inner(p)).await
        }

        "table_view_scan" => {
            run(payload, |p: TableViewScanRequest| table_view_scan_inner(p)).await
        }

        "table_view_format_check" => {
            run(payload, |p: TableViewFormatCheckRequest| table_view_format_check_inner(p)).await
        }

        "table_view_move" => {
            run(payload, |p: TableViewMoveRequest| table_view_move_inner(p)).await
        }

        "table_view_takeout" => {
            run(payload, |p: TableViewTakeoutRequest| table_view_takeout_inner(p)).await
        }

        "audit_orphaned_eids" => {
            run(payload, |p: AuditRequest| audit_orphaned_eids_inner(p)).await
        }

        "purge_orphaned_eids" => {
            run(payload, |p: PurgeRequest| purge_orphaned_eids_inner(p)).await
        }

        unknown => {
            let msg = format!("unknown task: '{}'", unknown);
            eprintln!("[edms-child] {msg}");
            (false, Value::Null, Some(msg))
        }
    }
}

// Generic helper — deserialises payload, runs the future, returns IPC triple
async fn run<T, F, Fut>(payload: Value, f: F) -> (bool, Value, Option<String>)
where
    T: serde::de::DeserializeOwned,
    F: FnOnce(T) -> Fut,
    Fut: std::future::Future<Output = Result<String, String>>,
{
    let typed: T = match serde_json::from_value(payload) {
        Ok(v) => v,
        Err(e) => {
            return (
                false,
                Value::Null,
                Some(format!("payload deserialise error: {e}")),
            );
        }
    };

    match f(typed).await {
        Ok(msg) => {
            let val: serde_json::Value =
                serde_json::from_str(&msg).unwrap_or(serde_json::json!({ "message": msg }));
            (true, val, None)
        }
        Err(e) => (false, Value::Null, Some(e)),
    }
}
