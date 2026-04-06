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