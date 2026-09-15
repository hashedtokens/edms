use axum::Json;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::{folder_manager, zipops};
use crate::markdown_generator::{create_markdown, EndpointRecord};
use crate::markdown_meta::create_markdown_meta;
use crate::endpoint_writer;
// ── HEALTH ────────────────────────────────────────────────────────────────────

pub async fn health() -> &'static str {
    "Ok"
}

// ── EXPORT COLLECTION ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ExportCollectionRequest {
    pub source: String,
    pub output: String,
}

pub async fn export_collection_inner(payload: ExportCollectionRequest) -> Result<String, String> {
    let source = PathBuf::from(payload.source);
    let output = PathBuf::from(payload.output);
    tokio::task::spawn_blocking(move || zipops::zip_collection(&source, &output))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    Ok("collection exported".into())
}

pub async fn export_collection(
    Json(payload): Json<ExportCollectionRequest>,
) -> Result<String, String> {
    export_collection_inner(payload).await
}

// ── EXPORT MERGE ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ExportMergeRequest {
    pub inputs:  Vec<String>,
    pub folders: Vec<String>,
    pub output:  String,
}

pub async fn export_merge_inner(payload: ExportMergeRequest) -> Result<String, String> {
    let input_refs:  Vec<PathBuf> = payload.inputs.iter().map(PathBuf::from).collect();
    let folder_refs: Vec<PathBuf> = payload.folders.iter().map(PathBuf::from).collect();
    let input_paths:  Vec<&Path> = input_refs.iter().map(|p| p.as_path()).collect();
    let folder_paths: Vec<&Path> = folder_refs.iter().map(|p| p.as_path()).collect();
    let output = PathBuf::from(payload.output);
    zipops::export_merge(&input_paths, &folder_paths, &output)
        .map_err(|e| e.to_string())?;
    Ok("merge export done".into())
}

pub async fn export_merge(
    Json(payload): Json<ExportMergeRequest>,
) -> Result<String, String> {
    export_merge_inner(payload).await
}

// ── IMPORT ZIP ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ImportZipRequest {
    pub zip:         String,
    pub destination: String,
}

pub async fn import_zip_inner(payload: ImportZipRequest) -> Result<String, String> {
    let zip  = PathBuf::from(payload.zip);
    let dest = PathBuf::from(payload.destination);
    zipops::import_zip_impl(&zip, &dest).map_err(|e| e.to_string())?;
    Ok("zip imported".into())
}

pub async fn import_zip(
    Json(payload): Json<ImportZipRequest>,
) -> Result<String, String> {
    import_zip_inner(payload).await
}

// ── EXPORT BOOKMARKS ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct BookmarkRequest {
    pub repo:   String,
    pub eids:   Vec<String>,
    pub output: String,
}

pub async fn export_bookmarks_inner(payload: BookmarkRequest) -> Result<String, String> {
    let repo   = PathBuf::from(payload.repo);
    let output = PathBuf::from(payload.output);
    zipops::create_zip_from_bookmarks(&repo, &payload.eids, &output)
        .map_err(|e| e.to_string())?;
    Ok("bookmark zip created".into())
}

pub async fn export_bookmarks(
    Json(payload): Json<BookmarkRequest>,
) -> Result<String, String> {
    export_bookmarks_inner(payload).await
}

// ── CREATE STATIC SITE ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct StaticCreateRequest {
    pub source: String,
    pub output: String,
}

pub async fn create_static_inner(payload: StaticCreateRequest) -> Result<String, String> {
    let source = PathBuf::from(payload.source);
    let output = PathBuf::from(payload.output);
    zipops::create_static_website(&source, &output).map_err(|e| e.to_string())?;
    Ok("static site created".into())
}

pub async fn create_static(
    Json(payload): Json<StaticCreateRequest>,
) -> Result<String, String> {
    create_static_inner(payload).await
}

// ── EXPORT STATIC ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct StaticExportRequest {
    pub source: String,
    pub output: String,
}

pub async fn export_static_inner(payload: StaticExportRequest) -> Result<String, String> {
    let source = PathBuf::from(payload.source);
    let output = PathBuf::from(payload.output);
    zipops::export_static_website(&source, &output).map_err(|e| e.to_string())?;
    Ok("static site exported".into())
}

pub async fn export_static(
    Json(payload): Json<StaticExportRequest>,
) -> Result<String, String> {
    export_static_inner(payload).await
}

// ── MARKDOWN GENERATION ───────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct MarkdownRequest {
    pub repo_path: String,
    pub per_file:  usize,
    pub endpoints: Vec<EndpointInput>,
}

#[derive(Deserialize)]
pub struct EndpointInput {
    pub eid:           u64,
    pub eid_string:    String,
    pub endpoint_type: String,
    pub request_type:  String,
    pub annotation:    String,
    pub tags:          String,
    pub req_count:     u32,
    pub res_count:     u32,
}

pub async fn generate_markdown_inner(payload: MarkdownRequest) -> Result<String, String> {
    let repo = PathBuf::from(payload.repo_path);
    let records = payload.endpoints.into_iter().map(|e| EndpointRecord {
        eid:           e.eid,
        eid_string:    e.eid_string,
        endpoint_type: e.endpoint_type,
        request_type:  e.request_type,
        annotation:    e.annotation,
        tags:          e.tags,
        req_count:     e.req_count,
        res_count:     e.res_count,
    });
    create_markdown(&repo, payload.per_file, records).map_err(|e| e.to_string())?;
    Ok("Markdown generated".into())
}

pub async fn generate_markdown(
    Json(payload): Json<MarkdownRequest>,
) -> Result<String, String> {
    generate_markdown_inner(payload).await
}

// ── MARKDOWN META ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct MetaRequest {
    pub repo_path: String,
    pub eids:      Vec<String>,
}

pub async fn generate_meta_inner(payload: MetaRequest) -> Result<String, String> {
    let repo = PathBuf::from(payload.repo_path);
    create_markdown_meta(repo, payload.eids.into_iter()).map_err(|e| e.to_string())?;
    Ok("Metadata generated".into())
}

pub async fn generate_meta(
    Json(payload): Json<MetaRequest>,
) -> Result<String, String> {
    generate_meta_inner(payload).await
}

// ── ENDPOINT WRITER ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct EndpointWriteRequest {
    pub repo_path:  String,
    pub eid:        String,
    pub page_index: usize,
    pub content:    String,
}

pub async fn write_endpoint_inner(payload: EndpointWriteRequest) -> Result<String, String> {
    endpoint_writer::write_endpoint_file(
        payload.repo_path,
        &payload.eid,
        payload.page_index,
        &payload.content,
    )
    .map_err(|e| e.to_string())?;
    Ok("Endpoint page created".into())
}

pub async fn write_endpoint(
    Json(payload): Json<EndpointWriteRequest>,
) -> Result<String, String> {
    write_endpoint_inner(payload).await
}

// ── REQUEST DOC ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RequestDoc {
    pub repo_path:  String,
    pub eid:        String,
    pub req_index:  usize,
    pub content:    String,
}

pub async fn write_request_inner(payload: RequestDoc) -> Result<String, String> {
    endpoint_writer::write_request_file(
        payload.repo_path,
        &payload.eid,
        payload.req_index,
        &payload.content,
    )
    .map_err(|e| e.to_string())?;
    Ok("Request doc written".into())
}

pub async fn write_request(
    Json(payload): Json<RequestDoc>,
) -> Result<String, String> {
    write_request_inner(payload).await
}

// ── RESPONSE DOC ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ResponseDoc {
    pub repo_path:  String,
    pub eid:        String,
    pub res_index:  usize,
    pub content:    String,
}

pub async fn write_response_inner(payload: ResponseDoc) -> Result<String, String> {
    endpoint_writer::write_response_file(
        payload.repo_path,
        &payload.eid,
        payload.res_index,
        &payload.content,
    )
    .map_err(|e| e.to_string())?;
    Ok("Response doc written".into())
}

pub async fn write_response(
    Json(payload): Json<ResponseDoc>,
) -> Result<String, String> {
    write_response_inner(payload).await
}

// ── HEADERS DOC ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct HeadersDoc {
    pub repo_path: String,
    pub eid:       String,
    pub index:     usize,
    pub content:   String,
}

pub async fn write_headers_inner(payload: HeadersDoc) -> Result<String, String> {
    endpoint_writer::write_headers_file(
        payload.repo_path,
        &payload.eid,
        payload.index,
        &payload.content,
    )
    .map_err(|e| e.to_string())?;
    Ok("Headers doc written".into())
}

pub async fn write_headers(
    Json(payload): Json<HeadersDoc>,
) -> Result<String, String> {
    write_headers_inner(payload).await
}

// ── MARK ACTIVE FOLDER ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct MarkActiveFolderRequest {
    pub session_backup:   String,
    pub active_folder:    String,
    pub folder_name:      String,
    pub yaml_config_path: String,
}

#[derive(Debug, Serialize)]
pub struct GenericResponse {
    pub success: bool,
    pub message: String,
}

pub async fn mark_active_folder_inner(
    req: MarkActiveFolderRequest,
) -> Result<GenericResponse, String> {
    zipops::mark_active_folder(
        Path::new(&req.session_backup),
        Path::new(&req.active_folder),
        &req.folder_name,
        Path::new(&req.yaml_config_path),
    )
    .map(|_| GenericResponse {
        success: true,
        message: "Active folder updated".to_string(),
    })
    .map_err(|e| e.to_string())
}

pub async fn mark_active_folder_handler(
    Json(req): Json<MarkActiveFolderRequest>,
) -> Result<Json<GenericResponse>, (StatusCode, String)> {
    mark_active_folder_inner(req)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

// ── CRUD OPERATIONS (dashboard) ────────────────────────────────────────────────
//
// Ravi's [1] approach: global, one-time processing spun up via a child
// process, triggered by app launch or the dashboard's Refresh button — not
// a per-transaction update. Scans a single edms.db for now; extending to
// multiple per-folder sqlite files (once that data model exists) just means
// looping this same logic over more db_paths and summing the counts.

#[derive(Deserialize)]
pub struct CrudOperationsRequest {
    pub db_path: String,
}

#[derive(Serialize)]
pub struct CrudOperationsRowOut {
    pub entity_type: String,
    pub method: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct CrudOperationsResult {
    pub rows: Vec<CrudOperationsRowOut>,
}

const CRUD_QUERIES: &[(&str, &str)] = &[
    (
        "endpoint_segments",
        "SELECT COALESCE(method, 'UNCLASSIFIED') AS m, COUNT(*) AS c \
         FROM endpoints GROUP BY m",
    ),
    (
        "tags",
        "SELECT COALESCE(e.method, 'UNCLASSIFIED') AS m, COUNT(*) AS c \
         FROM tags t JOIN endpoints e ON t.endpoint_id = e.endpoint_id GROUP BY m",
    ),
    (
        "bookmarks",
        "SELECT COALESCE(e.method, 'UNCLASSIFIED') AS m, COUNT(*) AS c \
         FROM bookmarks b JOIN endpoints e ON b.endpoint_id = e.endpoint_id GROUP BY m",
    ),
    (
        "history",
        "SELECT COALESCE(e.method, 'UNCLASSIFIED') AS m, COUNT(*) AS c \
         FROM history h JOIN endpoints e ON h.endpoint_id = e.endpoint_id GROUP BY m",
    ),
    (
        "qp_pairs",
        "SELECT COALESCE(e.method, 'UNCLASSIFIED') AS m, COUNT(*) AS c \
         FROM request_metadata rm \
         JOIN response_metadata resp \
           ON rm.endpoint_id = resp.endpoint_id AND rm.request_number = resp.request_number \
         JOIN endpoints e ON rm.endpoint_id = e.endpoint_id \
         GROUP BY m",
    ),
];

fn compute_crud_operations_sync(db_path: &str) -> Result<CrudOperationsResult, String> {
    let conn = rusqlite::Connection::open(db_path).map_err(|e| e.to_string())?;
    let mut rows = Vec::new();

    for (entity_type, query) in CRUD_QUERIES {
        let mut stmt = conn.prepare(query).map_err(|e| e.to_string())?;
        let mapped = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(|e| e.to_string())?;

        for entry in mapped {
            let (method, count) = entry.map_err(|e| e.to_string())?;
            rows.push(CrudOperationsRowOut {
                entity_type: entity_type.to_string(),
                method,
                count,
            });
        }
    }

    Ok(CrudOperationsResult { rows })
}

pub async fn compute_crud_operations_inner(payload: CrudOperationsRequest) -> Result<String, String> {
    let result = tokio::task::spawn_blocking(move || compute_crud_operations_sync(&payload.db_path))
        .await
        .map_err(|e| e.to_string())??;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub async fn compute_crud_operations(
    Json(payload): Json<CrudOperationsRequest>,
) -> Result<String, String> {
    compute_crud_operations_inner(payload).await
}

// ── SYSTEM STATUS / INIT ──────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SystemInitRequest {
    pub root_path: Option<String>,
}

pub async fn system_status(
) -> Result<Json<folder_manager::SystemInitReport>, (StatusCode, String)> {
    let root = folder_manager::default_root_path();
    let report = tokio::task::spawn_blocking(move || folder_manager::verify_and_init(&root))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(report))
}

// ── RUN TEST ───────────────────────────────────────────────────────────────────
// Ported from the dead webserver/src/bin/child.rs, which was never actually
// spawned (ipc.rs only ever spawns edms-child, this binary). Output shape
// adapted to match what webserver/src/handlers/callback.rs::handle_run_test
// actually reads today (response_body, not the old response_file) — the two
// had drifted apart while this sat unused.

#[derive(Deserialize)]
pub struct RunTestRequest {
    pub endpoint_id: String,
    pub url: String,
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default)]
    pub body: serde_json::Value,
    pub request_number: i64,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    /// Headers to send with the request. Echoed back in the result
    /// alongside the response's own headers, so the caller (webserver)
    /// can save both together without having to remember what it sent.
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
}

fn default_method() -> String {
    "GET".to_string()
}

fn default_timeout_ms() -> u64 {
    30_000
}

/// Converts a reqwest HeaderMap into a plain string map for JSON. A header
/// value that isn't valid UTF-8 is skipped rather than failing the whole
/// capture — rare in practice, and losing one odd header is better than
/// losing the test result over it.
fn headers_to_map(headers: &reqwest::header::HeaderMap) -> std::collections::HashMap<String, String> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            value.to_str().ok().map(|v| (name.to_string(), v.to_string()))
        })
        .collect()
}

pub async fn run_test_inner(payload: RunTestRequest) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(payload.timeout_ms))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let method = payload.method.to_uppercase();
    let mut req = match method.as_str() {
        "POST" => client.post(&payload.url).json(&payload.body),
        "PUT" => client.put(&payload.url).json(&payload.body),
        "DELETE" => client.delete(&payload.url),
        "PATCH" => client.patch(&payload.url).json(&payload.body),
        "HEAD" => client.head(&payload.url),
        _ => client.get(&payload.url),
    };
    for (name, value) in &payload.headers {
        req = req.header(name, value);
    }

    let start = std::time::Instant::now();

    match req.send().await {
        Ok(resp) => {
            let elapsed_ms = start.elapsed().as_millis() as i64;
            let status_code = resp.status().as_u16() as i64;
            let response_headers = headers_to_map(resp.headers());
            let text = resp.text().await.unwrap_or_default();
            // Best-effort parse as JSON; fall back to the raw text as a
            // string value if the endpoint didn't return JSON.
            let response_body: serde_json::Value =
                serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text));

            Ok(serde_json::json!({
                "endpoint_id": payload.endpoint_id,
                "request_number": payload.request_number,
                "status_code": status_code,
                "response_time_ms": elapsed_ms,
                "response_body": response_body,
                "request_headers": payload.headers,
                "response_headers": response_headers,
                "timed_out": false,
            })
            .to_string())
        }
        Err(e) if e.is_timeout() => Ok(serde_json::json!({
            "endpoint_id": payload.endpoint_id,
            "request_number": payload.request_number,
            "timed_out": true,
        })
        .to_string()),
        Err(e) => Err(format!("HTTP request failed: {e}")),
    }
}

pub async fn system_init(
    payload: Option<Json<SystemInitRequest>>,
) -> Result<Json<folder_manager::SystemInitReport>, (StatusCode, String)> {
    let root: PathBuf = match payload.and_then(|Json(p)| p.root_path) {
        Some(custom) => {
            let p = PathBuf::from(&custom);
            if !p.is_absolute() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!("root_path must be absolute, got: '{}'", custom),
                ));
            }
            p
        }
        None => folder_manager::default_root_path(),
    };
    let report = tokio::task::spawn_blocking(move || folder_manager::verify_and_init(&root))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(report))
}

// Tag operation handlers — each runs the synchronous tagops fn on spawn_blocking.

use crate::tagops::{
    ActivityEntry, BulkTagRequest, CreateFromTagsRequest, MergeRequest, RenameTagRequest,
    bulk_add_tags, bulk_remove_tags, create_from_tags, merge_by_tags, rename_tag,
};

pub async fn tagops_merge_inner(req: MergeRequest) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let mut log: Vec<ActivityEntry> = Vec::new();
        merge_by_tags(req, &mut log).map_err(|e| e.to_string())?;
        serde_json::to_string(&log).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn tagops_create_inner(req: CreateFromTagsRequest) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let mut log: Vec<ActivityEntry> = Vec::new();
        create_from_tags(req, &mut log).map_err(|e| e.to_string())?;
        serde_json::to_string(&log).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn tagops_bulk_add_inner(req: BulkTagRequest) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let mut log: Vec<ActivityEntry> = Vec::new();
        bulk_add_tags(req, &mut log).map_err(|e| e.to_string())?;
        serde_json::to_string(&log).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn tagops_bulk_remove_inner(req: BulkTagRequest) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let mut log: Vec<ActivityEntry> = Vec::new();
        bulk_remove_tags(req, &mut log).map_err(|e| e.to_string())?;
        serde_json::to_string(&log).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn tagops_rename_inner(req: RenameTagRequest) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let mut log: Vec<ActivityEntry> = Vec::new();
        rename_tag(req, &mut log).map_err(|e| e.to_string())?;
        serde_json::to_string(&log).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── REMOVE URL PREFIX ─────────────────────────────────────────────────────────

pub use crate::remove_url::{
    apply_remove_url_prefix, strip_url_prefix, RemoveUrlRequest, RemoveUrlSummary, TargetSelection,
};

pub async fn remove_url_prefix_inner(req: RemoveUrlRequest) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let summary = apply_remove_url_prefix(req).map_err(|e| e.to_string())?;
        serde_json::to_string(&summary).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── TABLE VIEW & VALIDATION ──────────────────────────────────────────────────

pub use crate::table_view::{
    check_item_format, move_item_to_view, scan_imports_table, takeout_item, MoveResult, TableItem,
    TableViewResponse, TakeoutResult, ViewPurpose,
};
pub use crate::validate::{
    validate_bookmark_format, validate_webview_format, ComponentStatus, ValidationReport,
};

#[derive(Deserialize)]
pub struct TableViewScanRequest {
    pub imports_dir: String,
}

#[derive(Deserialize)]
pub struct TableViewFormatCheckRequest {
    pub item_path: String,
    pub purpose: ViewPurpose,
}

#[derive(Deserialize)]
pub struct TableViewMoveRequest {
    pub storage_root: String,
    pub item_path: String,
    pub purpose: ViewPurpose,
}

#[derive(Deserialize)]
pub struct TableViewTakeoutRequest {
    pub source_path: String,
    pub dest_name: String,
    pub storage_root: String,
    pub overwrite: bool,
}

pub async fn table_view_scan_inner(req: TableViewScanRequest) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let res = scan_imports_table(Path::new(&req.imports_dir));
        serde_json::to_string(&res).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn table_view_format_check_inner(
    req: TableViewFormatCheckRequest,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let report = check_item_format(Path::new(&req.item_path), req.purpose);
        serde_json::to_string(&report).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn table_view_move_inner(req: TableViewMoveRequest) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let res = move_item_to_view(
            Path::new(&req.storage_root),
            Path::new(&req.item_path),
            req.purpose,
        )
        .map_err(|e| e.to_string())?;
        serde_json::to_string(&res).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn table_view_takeout_inner(req: TableViewTakeoutRequest) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let res = takeout_item(
            Path::new(&req.source_path),
            &req.dest_name,
            Path::new(&req.storage_root),
            req.overwrite,
        )
        .map_err(|e| e.to_string())?;
        serde_json::to_string(&res).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── ORPHANED EID AUDIT & PURGE ───────────────────────────────────────────────

pub use crate::audit_orphanedEIDs::{
    execute_purge, generate_audit_report, AuditReport, AuditRequest, PurgeRequest, PurgeResult,
};

pub async fn audit_orphaned_eids_inner(req: AuditRequest) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let report = generate_audit_report(
            Path::new(&req.db_path),
            Path::new(&req.eqp_dir),
            Path::new(&req.output_path),
        )
        .map_err(|e| e.to_string())?;
        serde_json::to_string(&report).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn purge_orphaned_eids_inner(req: PurgeRequest) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let report = if let Some(ref path_str) = req.audit_report_path {
            let f = std::fs::File::open(path_str).map_err(|e| e.to_string())?;
            serde_json::from_reader(f).map_err(|e| e.to_string())?
        } else {
            generate_audit_report(
                Path::new(&req.db_path),
                Path::new(&req.eqp_dir),
                Path::new("temp/audit.json"),
            )
            .map_err(|e| e.to_string())?
        };

        let result = execute_purge(
            Path::new(&req.db_path),
            Path::new(&req.eqp_dir),
            &report,
        )
        .map_err(|e| e.to_string())?;
        serde_json::to_string(&result).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}