//! edms-child — IPC child process for endpoint_mng_sys
//!
//! Reads one IpcRequest JSON line from stdin.
//! Dispatches to the appropriate handler.
//! POSTs IpcCallback result back to parent's /internal/callback.
//! Exits.

use serde_json::Value;
use std::io::{self};
use std::time::Instant;

// Re-use all the handler inner functions and request types
use compute::api::handlers::{
    BookmarkRequest, CrudOperationsRequest, EndpointWriteRequest, ExportCollectionRequest,
    ExportMergeRequest, HeadersDoc, ImportZipRequest, MarkActiveFolderRequest, MarkdownRequest, MetaRequest,
    RequestDoc, ResponseDoc, RunTestRequest, StaticCreateRequest, StaticExportRequest,
    compute_crud_operations_inner, create_static_inner, export_bookmarks_inner,
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
    let root = compute::folder_manager::default_root_path();
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

    let (success, mut result, error) = dispatch(&task, request.payload).await;

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
