//! Import / Export ("takeout") for RepoViews - v1.0 (Ravi, 2026-10-05/06) -
//! and, the same way, WebViews (`Flavor` says which; a WebView's folder also
//! holds `front-page.json`, which travels with it).
//!
//! A RepoView is only a list, so this is the one place its EQP data is
//! touched:
//!
//! - **Takeout** copies the EQP data of every listed endpoint out of
//!   globalEQPData into `storage/takeout/{name}/`, next to the generated
//!   Tables files, with the SQLite index *removed* - "friendly for git or
//!   web". The existing `compute::table_view::takeout_item` does the copy of
//!   the RepoView folder, the SQLite stripping and the name-collision check.
//! - Because a takeout has no SQLite, it also carries
//!   `repoview-manifest.json`: the RepoView's own details plus each
//!   endpoint's URL, method, tags and QP status codes/timings. A real EQP
//!   folder holds only request/response/header files, so without this an
//!   import would have no way to rebuild the index.
//! - **Import** reads a takeout folder, gives every endpoint a *fresh* EID
//!   ("IE always have their own unique EIDs even if the same endpoint exists
//!   in a different EID"), renames the EQP files to match, writes the central
//!   rows, and builds the RepoView's index.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use edms::ops::collection_membership_ops::CollectionMembershipOps;
use edms::ops::tag_ops::TagOps;
use serde::{Deserialize, Serialize};
use compute::view_io::EqpEntry;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::{
    db,
    ipc::IpcCallback,
    handlers::{
        repoview_index::{build_index, catalog_rows, init_snapshot_tables},
        repoview_tables::{generate_tables_blocking, parse_approach_line, Approach},
        view_catalog::{open_catalog, open_existing_membership, open_membership, validate_folder_name},
        view_flavor::Flavor,
    },
    state::AppState,
};

pub(crate) const FRONT_PAGE_FILE: &str = "front-page.json";
pub(crate) const MANIFEST_FILE: &str = "repoview-manifest.json";
pub(crate) const MANIFEST_FORMAT: &str = "edms-repoview";
pub(crate) const MANIFEST_VERSION: u32 = 1;
const EQP_DIR: &str = "globalEQPData";

// ── The manifest ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Manifest {
    pub format: String,
    pub version: u32,
    pub repoview: ManifestRepoview,
    pub endpoints: Vec<ManifestEndpoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ManifestRepoview {
    pub name: String,
    pub annotation: Option<String>,
    pub source: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    /// `"repoview"` or `"webview"`. Takeouts made before WebView existed
    /// have no such field, and are RepoViews.
    #[serde(default = "default_kind")]
    pub kind: String,
}

fn default_kind() -> String {
    "repoview".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ManifestEndpoint {
    pub eid: String,
    pub endpoint_str: String,
    pub method: Option<String>,
    pub annotation: Option<String>,
    pub tags: Vec<String>,
    pub qps: Vec<ManifestQp>,
    /// When the endpoint was created, and last changed, where the takeout
    /// came from. Optional: a takeout made before these were recorded has
    /// none, and an import gives its endpoints the dates of the import.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ManifestQp {
    pub request_number: i32,
    pub method: Option<String>,
    pub status_code: Option<i32>,
    pub response_time_ms: Option<i32>,
}

/// Everything an import needs, read straight from the RepoView's own index.
pub(crate) fn build_manifest(
    membership: &CollectionMembershipOps,
    repoview: ManifestRepoview,
) -> Result<Manifest, String> {
    init_snapshot_tables(membership)?;

    let rows: Vec<(String, String, Option<String>, Option<String>, Option<String>, Option<String>)> = membership
        .core
        .cproc(
            "SELECT s.endpoint_id, s.endpoint_str, s.method, s.annotation, s.created_at, s.updated_at
             FROM endpoint_snapshot s JOIN membership m ON m.endpoint_id = s.endpoint_id
             ORDER BY s.endpoint_id",
            &[],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .map_err(|e| format!("{e:?}"))?;

    let tag_rows: Vec<(String, String)> = membership
        .core
        .cproc(
            "SELECT endpoint_id, tag FROM endpoint_tags
             WHERE endpoint_id IN (SELECT endpoint_id FROM membership) ORDER BY endpoint_id, tag",
            &[],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| format!("{e:?}"))?;
    let mut tags: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (eid, tag) in tag_rows {
        tags.entry(eid).or_default().push(tag);
    }

    let qp_rows: Vec<(String, ManifestQp)> = membership
        .core
        .cproc(
            "SELECT endpoint_id, request_number, method, status_code, response_time_ms FROM qp_snapshot
             WHERE endpoint_id IN (SELECT endpoint_id FROM membership) ORDER BY endpoint_id, request_number",
            &[],
            |r| {
                Ok((
                    r.get(0)?,
                    ManifestQp {
                        request_number: r.get(1)?,
                        method: r.get(2)?,
                        status_code: r.get(3)?,
                        response_time_ms: r.get(4)?,
                    },
                ))
            },
        )
        .map_err(|e| format!("{e:?}"))?;
    let mut qps: BTreeMap<String, Vec<ManifestQp>> = BTreeMap::new();
    for (eid, qp) in qp_rows {
        qps.entry(eid).or_default().push(qp);
    }

    let endpoints = rows
        .into_iter()
        .map(|(eid, endpoint_str, method, annotation, created_at, updated_at)| ManifestEndpoint {
            tags: tags.remove(&eid).unwrap_or_default(),
            qps: qps.remove(&eid).unwrap_or_default(),
            eid,
            endpoint_str,
            method,
            annotation,
            created_at,
            updated_at,
        })
        .collect();

    Ok(Manifest {
        format: MANIFEST_FORMAT.to_string(),
        version: MANIFEST_VERSION,
        repoview,
        endpoints,
    })
}

/// Rejects a manifest that isn't ours, or that would make an import unsafe
/// (an EID is used to build folder and file names, so it must be a real one).
pub(crate) fn validate_manifest(m: &Manifest) -> Result<(), String> {
    if m.format != MANIFEST_FORMAT {
        return Err(format!(
            "not a RepoView takeout: manifest format is '{}', expected '{MANIFEST_FORMAT}'",
            m.format
        ));
    }
    if m.version != MANIFEST_VERSION {
        return Err(format!(
            "unsupported manifest version {} (this build reads version {MANIFEST_VERSION})",
            m.version
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for e in &m.endpoints {
        if !compute::eid::is_valid_eid(&e.eid) {
            return Err(format!("'{}' in the manifest is not a valid EID", e.eid));
        }
        if !seen.insert(e.eid.as_str()) {
            return Err(format!("EID '{}' appears twice in the manifest", e.eid));
        }
        if e.endpoint_str.trim().is_empty() {
            return Err(format!("endpoint '{}' has no URL in the manifest", e.eid));
        }
    }
    Ok(())
}

// ── EID-aware file copying ───────────────────────────────────────────────

pub(crate) fn takeout_root(state: &AppState) -> PathBuf {
    state.storage_root.join("storage")
}

fn imports_dir(flavor: Flavor, state: &AppState) -> PathBuf {
    state
        .storage_root
        .join("storage")
        .join("imports")
        .join("uncompressed")
        .join(flavor.import_folder())
}

// ── Takeout (export) ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TakeoutRequest {
    /// Folder name under `storage/takeout/`. Defaults to the RepoView's name.
    #[serde(default)]
    pub dest_name: Option<String>,
    /// Replace an existing takeout of that name instead of refusing.
    #[serde(default)]
    pub overwrite: bool,
}

pub(crate) enum IeError {
    Conflict { message: String, options: Vec<&'static str> },
    Bad(String),
}

impl From<String> for IeError {
    fn from(e: String) -> Self {
        IeError::Bad(e)
    }
}

pub(crate) fn respond(
    state: &AppState,
    ok_status: StatusCode,
    res: Result<Result<Value, IeError>, tokio::task::JoinError>,
) -> (StatusCode, Json<Value>) {
    match res {
        Ok(Ok(body)) => {
            state.refresh_dashboard_snapshot();
            (ok_status, Json(body))
        }
        Ok(Err(IeError::Conflict { message, options })) => (
            StatusCode::CONFLICT,
            Json(json!({ "ok": false, "conflict": true, "error": message, "options": options })),
        ),
        Ok(Err(IeError::Bad(e))) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

/// POST /repoview/:name/takeout
///
/// Copies the EQP data of every listed endpoint out of globalEQPData into
/// `storage/takeout/{dest_name}/globalEQPData/{eid}/`, alongside the Tables
/// files and `repoview-manifest.json`. The SQLite index is never included.
/// If that takeout folder already exists: **409** with `options:
/// ["overwrite", "rename"]` and nothing changes.
pub async fn takeout_repoview(
    State(state): State<AppState>,
    Extension(flavor): Extension<Flavor>,
    Path(name): Path<String>,
    Json(payload): Json<TakeoutRequest>,
) -> (StatusCode, Json<Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        let name = name.clone();
        move || takeout_blocking(flavor, &state, &name, payload.dest_name.as_deref(), payload.overwrite)
    })
    .await;
    respond(&state, StatusCode::ACCEPTED, res)
}

fn takeout_blocking(flavor: Flavor, state: &AppState, name: &str, dest_name: Option<&str>, overwrite: bool) -> Result<Value, IeError> {
    flavor.validate_name(name)?;
    let dest_name = dest_name.map(str::trim).filter(|d| !d.is_empty()).unwrap_or(name);
    validate_folder_name(dest_name, "Takeout name")?;

    let membership = open_existing_membership(state, flavor.kind(), name)
        .map_err(|_| IeError::Bad(flavor.not_found(name)))?;
    let dir = flavor.dir(state, name);
    if !dir.is_dir() {
        return Err(IeError::Bad(format!("{} '{name}' has no folder yet", flavor.label())));
    }

    let row = catalog_rows(state, &flavor.catalog_query("GET"), &[&name])?
        .into_iter()
        .next()
        .ok_or_else(|| IeError::Bad(flavor.not_found(name)))?;
    let row_tags = flavor.row_tag_ops(state)?.list(name).map_err(|e| format!("{e:?}"))?;
    let manifest = build_manifest(
        &membership,
        ManifestRepoview {
            name: name.to_string(),
            annotation: row.3,
            source: row.4,
            tags: row_tags,
            created_at: row.2,
            kind: flavor.route().to_string(),
        },
    )?;

    // Collisions are answered now (an immediate 409), not by the job later.
    let dest_dir = takeout_root(state).join("takeout").join(dest_name);
    if dest_dir.exists() && !overwrite {
        return Err(IeError::Conflict {
            message: format!("A takeout named '{dest_name}' already exists"),
            options: vec!["overwrite", "rename"],
        });
    }

    // The copying itself - the view folder with its index stripped, every
    // endpoint's EQP data, the manifest - runs in edms-child. The request
    // returns now; the result arrives as a `ViewIoDone` event (and on
    // `GET /jobs/:id`).
    let endpoints: Vec<EqpEntry> = manifest
        .endpoints
        .iter()
        .map(|e| EqpEntry { eid: e.eid.clone(), has_qps: !e.qps.is_empty() })
        .collect();
    let job_id = state.jobs.create("takeout", Some(flavor.route()), Some(name));
    let started = crate::ipc::spawn_child(
        "view_takeout",
        json!({
            "job_id": job_id,
            "view_dir": dir,
            "dest_name": dest_name,
            "storage_dir": takeout_root(state),
            "overwrite": overwrite,
            "eqp_root": state.storage_root.join("storage").join("globalEQPData"),
            "endpoints": endpoints,
            "manifest": manifest,
            "manifest_file": MANIFEST_FILE,
            // A RepoView's takeout always gets a Tables-meta.md with the
            // takeout's own facts; a WebView has no Tables to describe.
            "create_meta_if_missing": flavor.has_tables(),
        }),
        3000,
    );
    if !started {
        let error = "couldn't start the compute process (is edms-child built?)".to_string();
        if let Some(job) = state.jobs.fail(&job_id, error.clone()) {
            crate::handlers::jobs::emit_done(state, &job);
        }
        return Err(IeError::Bad(error));
    }

    Ok(json!({
        "ok": true,
        "status": "running",
        "job_id": job_id,
        "destination": dest_dir,
        "endpoints": manifest.endpoints.len()
    }))
}

// ── Import ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ImportRequest {
    /// Folder name under `storage/imports/uncompressed/repo/` (or `/webview/`)
    /// - a takeout copied back from git (or straight from `storage/takeout/`).
    /// Give this or `zip`.
    #[serde(default)]
    pub folder: Option<String>,
    /// A zipped takeout in `storage/imports/compressed/repo/` (or
    /// `/webview/`): unzipped to `uncompressed/{name without .zip}/` first,
    /// then imported as above.
    #[serde(default)]
    pub zip: Option<String>,
    /// With `zip`: replace that unzipped folder if it is already there.
    #[serde(default)]
    pub replace_extracted: bool,
    /// Name for the new RepoView. Defaults to the name in the manifest.
    #[serde(default)]
    pub name: Option<String>,
}

/// POST /repoview/import
///
/// Reads `repoview-manifest.json` from the folder and builds a RepoView:
/// every endpoint gets a **fresh EID** (even if the same URL already exists
/// under another one), its EQP files are copied into globalEQPData under the
/// new name, its central rows and tags are written, and the RepoView's index
/// is built from them. If the RepoView name is taken: **409** with
/// `options: ["rename"]` - retry with `name`.
///
/// It runs as a job in three parts, so the request returns at once:
/// 1. here: validate, check the name, allocate the new EIDs, start the child
///    -> `202 {job_id}`;
/// 2. `edms-child` (`view_import_copy`): copy and rename the EQP files;
/// 3. `finish_import`, when the child calls back: write the central rows,
///    build the index and catalog row, then finish the job (`ViewIoDone`).
/// Endpoints that can't be imported are reported in the result's `skipped`;
/// if none can, nothing is created.
pub async fn import_repoview(
    State(state): State<AppState>,
    Extension(flavor): Extension<Flavor>,
    Json(payload): Json<ImportRequest>,
) -> (StatusCode, Json<Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        move || match (payload.folder.as_deref(), payload.zip.as_deref()) {
            (Some(folder), None) => import_blocking(flavor, &state, folder, payload.name.as_deref(), None),
            (None, Some(zip)) => import_zip_blocking(flavor, &state, zip, payload.name.as_deref(), payload.replace_extracted),
            _ => Err(IeError::Bad("give exactly one of `folder` (an unzipped takeout) or `zip` (a zipped one)".to_string())),
        }
    })
    .await;
    respond(&state, StatusCode::ACCEPTED, res)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PlanItem {
    pub(crate) old: String,
    pub(crate) new: String,
}

/// What part 3 needs, kept in the job while the child copies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ImportContext {
    /// `repoview`, `webview` or `collection`.
    pub(crate) flavor: String,
    pub(crate) name: String,
    pub(crate) src: String,
    pub(crate) manifest: Manifest,
    pub(crate) plan: Vec<PlanItem>,
    pub(crate) skipped: Vec<Value>,
}

fn flavor_from_route(route: &str) -> Option<Flavor> {
    [Flavor::Repo, Flavor::Web].into_iter().find(|f| f.route() == route)
}

/// Undoes one endpoint's partial import so a failure never leaves a
/// half-written endpoint behind.
pub(crate) fn rollback_endpoint(state: &AppState, new_eid: &str) {
    let _ = db::delete_endpoint(&state.core, &state.queries, new_eid);
    let _ = std::fs::remove_dir_all(state.endpoint_storage_dir(new_eid));
    let _ = state.eid_allocator.release(new_eid);
}

/// Gives back an EID that never became an endpoint: its files (if any were
/// copied) and the allocation.
fn release_eid(state: &AppState, new_eid: &str) {
    let _ = std::fs::remove_dir_all(state.endpoint_storage_dir(new_eid));
    let _ = state.eid_allocator.release(new_eid);
}

/// Where to send someone who picked the wrong kind of import.
fn import_route(kind: &str) -> String {
    match kind {
        "collection" => "/collections/import".to_string(),
        other => format!("/{other}/import"),
    }
}

/// Reads and checks a takeout folder's manifest. `expected_kind` is
/// `repoview`, `webview` or `collection`: a takeout of the other kind is
/// refused, with a pointer to the route that does take it.
pub(crate) fn load_manifest(folder: &str, src: &std::path::Path, expected_kind: &str) -> Result<Manifest, IeError> {
    let manifest_text = std::fs::read_to_string(src.join(MANIFEST_FILE))
        .map_err(|e| IeError::Bad(format!("can't read {MANIFEST_FILE} in '{folder}': {e}")))?;
    let manifest: Manifest = serde_json::from_str(&manifest_text)
        .map_err(|e| IeError::Bad(format!("{MANIFEST_FILE} in '{folder}' is not valid: {e}")))?;
    validate_manifest(&manifest)?;
    if manifest.repoview.kind != expected_kind {
        return Err(IeError::Bad(format!(
            "'{folder}' is a {} takeout, not a {expected_kind} - import it through {}",
            manifest.repoview.kind,
            import_route(&manifest.repoview.kind)
        )));
    }
    Ok(manifest)
}

/// Every endpoint gets a fresh EID, allocated now so the child can name the
/// copied files. The central rows wait until the files are in place. Returns
/// the plan and the endpoints that couldn't get an EID; refuses if none did.
pub(crate) fn plan_new_eids(state: &AppState, manifest: &Manifest) -> Result<(Vec<PlanItem>, Vec<Value>), IeError> {
    let mut plan: Vec<PlanItem> = Vec::new();
    let mut skipped: Vec<Value> = Vec::new();
    for endpoint in &manifest.endpoints {
        let skip = |why: String| json!({ "eid": endpoint.eid, "reason": why });
        match state.eid_allocator.allocate() {
            Ok(new) => match db::get_endpoint(&state.core, &state.queries, &new) {
                Ok(None) => plan.push(PlanItem { old: endpoint.eid.clone(), new }),
                // The allocator and the table disagree: leave that endpoint
                // (and its EID) alone rather than write over it.
                Ok(Some(_)) => skipped.push(skip(format!("the new EID {new} is already in use"))),
                Err(e) => {
                    let _ = state.eid_allocator.release(&new);
                    skipped.push(skip(format!("couldn't check the new EID: {e:?}")));
                }
            },
            Err(e) => skipped.push(skip(format!("couldn't allocate a new EID: {e:?}"))),
        }
    }
    if plan.is_empty() {
        return Err(IeError::Bad("none of the endpoints in this takeout could be imported".to_string()));
    }
    Ok((plan, skipped))
}

/// Creates (or continues) the import's job, keeps what part 3 needs in it and
/// starts the child that copies the EQP files. If the child can't start, the
/// EIDs are given back and the job fails. Returns the job's id.
pub(crate) fn begin_copy_job(
    state: &AppState,
    view: &str,
    kind: &str,
    name: &str,
    // A zip import has a job already (it started with the unzip).
    existing_job: Option<&str>,
    src: &std::path::Path,
    manifest: &Manifest,
    plan: &[PlanItem],
    skipped: &[Value],
) -> Result<String, IeError> {
    let job_id = match existing_job {
        Some(id) => id.to_string(),
        None => state.jobs.create("import", Some(view), Some(name)),
    };
    let context = ImportContext {
        flavor: kind.to_string(),
        name: name.to_string(),
        src: src.display().to_string(),
        manifest: manifest.clone(),
        plan: plan.to_vec(),
        skipped: skipped.to_vec(),
    };
    state.jobs.set_context(&job_id, serde_json::to_value(&context).map_err(|e| e.to_string())?);

    let started = crate::ipc::spawn_child(
        "view_import_copy",
        json!({
            "job_id": job_id,
            "src_dir": src,
            "eqp_root": state.storage_root.join("storage").join("globalEQPData"),
            "items": plan.iter().map(|p| json!({ "old_eid": p.old, "new_eid": p.new })).collect::<Vec<_>>(),
        }),
        3000,
    );
    if !started {
        for item in plan {
            release_eid(state, &item.new);
        }
        state.jobs.take_context(&job_id);
        let error = "couldn't start the compute process (is edms-child built?)".to_string();
        if let Some(job) = state.jobs.fail(&job_id, error.clone()) {
            crate::handlers::jobs::emit_done(state, &job);
        }
        return Err(IeError::Bad(error));
    }
    Ok(job_id)
}

/// Part 1.
fn import_blocking(
    flavor: Flavor,
    state: &AppState,
    folder: &str,
    name_override: Option<&str>,
    // A zip import has a job already (it started with the unzip).
    existing_job: Option<&str>,
) -> Result<Value, IeError> {
    validate_folder_name(folder, "Folder name")?;
    let src = imports_dir(flavor, state).join(folder);
    if !src.is_dir() {
        return Err(IeError::Bad(format!(
            "'{folder}' was not found under storage/imports/uncompressed/{}/",
            flavor.import_folder()
        )));
    }

    let manifest = load_manifest(folder, &src, flavor.route())?;

    let name = name_override
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or(&manifest.repoview.name)
        .to_string();
    flavor.validate_name(&name)?;
    let catalog = open_catalog(state)?;
    if catalog.get(flavor.kind(), &name).map_err(|e| format!("{e:?}"))?.is_some() {
        return Err(IeError::Conflict {
            message: format!("{} '{name}' already exists", flavor.label()),
            options: vec!["rename"],
        });
    }

    let (plan, skipped) = plan_new_eids(state, &manifest)?;
    let job_id = begin_copy_job(state, flavor.route(), flavor.route(), &name, existing_job, &src, &manifest, &plan, &skipped)?;

    Ok(json!({
        "ok": true,
        "status": "running",
        "job_id": job_id,
        "name": name,
        "endpoints": plan.len(),
        "skipped": skipped
    }))
}

/// What a zip import keeps while the child unzips.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UnzipContext {
    pub(crate) kind: String,
    /// `repoview`, `webview` or `collection`.
    pub(crate) flavor: String,
    pub(crate) folder: String,
    pub(crate) name: Option<String>,
}

/// `import` with a `zip`: the same job, with an unzip in front. The child
/// unzips into `storage/imports/uncompressed/{repo|webview}/{zip stem}/`
/// (strictly - see `compute::view_io::run_unzip`), then `continue_after_unzip`
/// carries on exactly like an import of that folder.
fn import_zip_blocking(
    flavor: Flavor,
    state: &AppState,
    zip: &str,
    name_override: Option<&str>,
    replace: bool,
) -> Result<Value, IeError> {
    validate_folder_name(zip, "Zip name")?;
    let Some(stem) = zip.strip_suffix(".zip").or_else(|| zip.strip_suffix(".ZIP")) else {
        return Err(IeError::Bad(format!("'{zip}' is not a .zip file")));
    };
    validate_folder_name(stem, "Zip name")?;
    let zip_path = state
        .storage_root
        .join("storage")
        .join("imports")
        .join("compressed")
        .join(flavor.import_folder())
        .join(zip);
    if !zip_path.is_file() {
        return Err(IeError::Bad(format!(
            "'{zip}' was not found under storage/imports/compressed/{}/",
            flavor.import_folder()
        )));
    }

    let dest = imports_dir(flavor, state).join(stem);
    if dest.exists() && !replace {
        return Err(IeError::Conflict {
            message: format!("'{stem}' is already unzipped under storage/imports/uncompressed/{}/", flavor.import_folder()),
            options: vec!["replace"],
        });
    }
    // A name given up front can be checked now; one taken from the manifest
    // can only be checked once the zip is open.
    if let Some(name) = name_override.map(str::trim).filter(|n| !n.is_empty()) {
        flavor.validate_name(name)?;
        if open_catalog(state)?.get(flavor.kind(), name).map_err(|e| format!("{e:?}"))?.is_some() {
            return Err(IeError::Conflict {
                message: format!("{} '{name}' already exists", flavor.label()),
                options: vec!["rename"],
            });
        }
    }

    let job_id = state.jobs.create("import", Some(flavor.route()), Some(stem));
    let context = UnzipContext {
        kind: "unzip".to_string(),
        flavor: flavor.route().to_string(),
        folder: stem.to_string(),
        name: name_override.map(str::trim).filter(|n| !n.is_empty()).map(str::to_string),
    };
    state.jobs.set_context(&job_id, serde_json::to_value(&context).map_err(|e| e.to_string())?);
    let started = crate::ipc::spawn_child(
        "view_unzip",
        json!({
            "job_id": job_id,
            "zip_path": zip_path,
            "dest_dir": dest,
            "replace": replace,
            "manifest_file": MANIFEST_FILE,
        }),
        3000,
    );
    if !started {
        state.jobs.take_context(&job_id);
        let error = "couldn't start the compute process (is edms-child built?)".to_string();
        if let Some(job) = state.jobs.fail(&job_id, error.clone()) {
            crate::handlers::jobs::emit_done(state, &job);
        }
        return Err(IeError::Bad(error));
    }

    Ok(json!({ "ok": true, "status": "running", "job_id": job_id, "step": "unzipping", "folder": stem }))
}

/// The child has unzipped: carry on with the import of that folder, in the
/// same job. A problem found now (not a takeout, bad manifest, the name is
/// taken) fails the job, since there is no request left to answer.
pub async fn continue_after_unzip(state: &AppState, callback: &IpcCallback) {
    let state = state.clone();
    let callback = callback.clone();
    let _ = tokio::task::spawn_blocking(move || {
        let Some(job_id) = crate::handlers::jobs::job_id_of(&callback) else { return };
        let Some(context) = state.jobs.take_context(&job_id) else { return };
        let outcome = serde_json::from_value::<UnzipContext>(context)
            .map_err(|e| format!("lost the import's plan: {e}"))
            .and_then(|context| {
                let to_text = |e: IeError| match e {
                    IeError::Bad(why) => why,
                    IeError::Conflict { message, .. } => format!("{message} - import again with another `name`"),
                };
                if context.flavor == crate::handlers::collection_ie::KIND {
                    return crate::handlers::collection_ie::import_blocking(&state, &context.folder, context.name.as_deref(), Some(&job_id))
                        .map_err(to_text);
                }
                let flavor = flavor_from_route(&context.flavor).ok_or_else(|| format!("unknown view kind '{}'", context.flavor))?;
                import_blocking(flavor, &state, &context.folder, context.name.as_deref(), Some(&job_id)).map_err(to_text)
            });
        if let Err(error) = outcome {
            if let Some(job) = state.jobs.fail(&job_id, error) {
                crate::handlers::jobs::emit_done(&state, &job);
            }
        }
    })
    .await;
}

/// The child failed (or couldn't start properly): give back every EID part 1
/// allocated and any files it got to copy. The job itself is failed by the
/// generic callback code afterwards.
pub fn abort_import(state: &AppState, callback: &IpcCallback) {
    let Some(job_id) = crate::handlers::jobs::job_id_of(callback) else { return };
    let Some(context) = state.jobs.take_context(&job_id) else { return };
    if let Ok(context) = serde_json::from_value::<ImportContext>(context) {
        for item in &context.plan {
            release_eid(state, &item.new);
        }
    }
}

/// Part 3: the child has copied the files.
pub async fn finish_import(state: &AppState, callback: &IpcCallback) {
    let state = state.clone();
    let callback = callback.clone();
    let _ = tokio::task::spawn_blocking(move || {
        let Some(job_id) = crate::handlers::jobs::job_id_of(&callback) else { return };
        // Taken once: a duplicate callback finds nothing and does nothing.
        let Some(context) = state.jobs.take_context(&job_id) else { return };
        let outcome = serde_json::from_value::<ImportContext>(context)
            .map_err(|e| format!("lost the import's plan: {e}"))
            .and_then(|context| finish_import_blocking(&state, &context, &callback.result));
        let job = match outcome {
            Ok(result) => {
                state.refresh_dashboard_snapshot();
                state.jobs.complete(&job_id, result)
            }
            Err(error) => state.jobs.fail(&job_id, error),
        };
        if let Some(job) = job {
            crate::handlers::jobs::emit_done(&state, &job);
        }
    })
    .await;
}

/// What part 3 produced for the endpoints themselves.
pub(crate) struct ImportedEndpoints {
    pub(crate) mapping: Vec<Value>,
    pub(crate) new_eids: Vec<String>,
    pub(crate) skipped: Vec<Value>,
}

/// Part 3, the half every kind of import shares: for each endpoint the child
/// copied, write its central row, QP rows and tags. An endpoint that fails is
/// rolled back and reported in `skipped`; if none is left, the import fails.
pub(crate) fn write_imported_endpoints(
    state: &AppState,
    context: &ImportContext,
    copy_result: &Value,
) -> Result<ImportedEndpoints, String> {
    let manifest = &context.manifest;
    // What the child says about each endpoint's files, by new EID.
    let copied: BTreeMap<String, (usize, Option<String>)> = copy_result["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some((
                        item["new_eid"].as_str()?.to_string(),
                        (
                            item["files"].as_u64().unwrap_or(0) as usize,
                            item["error"].as_str().map(str::to_string),
                        ),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();

    let tag_ops = TagOps::new(&state.db_path.display().to_string());
    tag_ops.initialize().map_err(|e| format!("{e:?}"))?;
    let insert_request = state.queries.get_request_query("R1").ok_or_else(|| "query R1 is missing".to_string())?;
    let insert_response = state.queries.get_response_query("RES1").ok_or_else(|| "query RES1 is missing".to_string())?;

    let by_old: BTreeMap<&str, &ManifestEndpoint> = manifest.endpoints.iter().map(|e| (e.eid.as_str(), e)).collect();
    let mut mapping: Vec<Value> = Vec::new();
    let mut new_eids: Vec<String> = Vec::new();
    let mut skipped: Vec<Value> = context.skipped.clone();

    for item in &context.plan {
        let skip = |why: String| json!({ "eid": item.old, "reason": why });
        let Some(endpoint) = by_old.get(item.old.as_str()) else {
            release_eid(state, &item.new);
            skipped.push(skip("not in the manifest".to_string()));
            continue;
        };
        let new_eid = &item.new;

        match copied.get(new_eid) {
            Some((_, None)) => {}
            Some((_, Some(why))) => {
                release_eid(state, new_eid);
                skipped.push(skip(format!("couldn't copy its EQP data: {why}")));
                continue;
            }
            None => {
                release_eid(state, new_eid);
                skipped.push(skip("the compute process didn't report on its EQP data".to_string()));
                continue;
            }
        }

        let dto = db::EndpointDto {
            endpoint_id: new_eid.clone(),
            endpoint_str: endpoint.endpoint_str.clone(),
            annotation: endpoint.annotation.clone(),
            method: Some(endpoint.method.clone().unwrap_or_else(|| "GET".to_string())),
        };
        if let Err(e) = db::insert_endpoint(&state.core, &state.queries, &dto) {
            release_eid(state, new_eid);
            skipped.push(skip(format!("couldn't create the endpoint: {e:?}")));
            continue;
        }

        let eqp_dst = state.endpoint_storage_dir(new_eid);
        let mut qps_written = 0usize;
        let mut failed: Option<String> = None;
        for qp in &endpoint.qps {
            let request_path = eqp_dst.join(format!("{new_eid}-request-{}.json", qp.request_number));
            if !request_path.is_file() {
                continue; // no saved request for this QP in the takeout
            }
            let response_path = eqp_dst.join(format!("{new_eid}-response-{}.json", qp.request_number));
            let (req, res) = (request_path.display().to_string(), response_path.display().to_string());
            let written = state
                .core
                .proc(insert_request, &[new_eid, &qp.request_number, &req, &qp.method])
                .and_then(|_| {
                    state.core.proc(
                        insert_response,
                        &[new_eid, &qp.request_number, &res, &qp.status_code, &qp.response_time_ms],
                    )
                });
            match written {
                Ok(_) => qps_written += 1,
                Err(e) => {
                    failed = Some(format!("couldn't record QP {}: {e:?}", qp.request_number));
                    break;
                }
            }
        }
        if let Some(why) = failed {
            rollback_endpoint(state, new_eid);
            skipped.push(skip(why));
            continue;
        }

        for tag in &endpoint.tags {
            let _ = tag_ops.add(new_eid, tag);
        }

        mapping.push(json!({ "old_eid": item.old, "new_eid": new_eid, "qps": qps_written }));
        new_eids.push(new_eid.clone());
    }

    if new_eids.is_empty() {
        return Err("none of the endpoints in this takeout could be imported".to_string());
    }

    Ok(ImportedEndpoints { mapping, new_eids, skipped })
}

fn finish_import_blocking(state: &AppState, context: &ImportContext, copy_result: &Value) -> Result<Value, String> {
    let is_collection = context.flavor == crate::handlers::collection_ie::KIND;
    // Refuse an unknown kind before anything is written.
    let flavor = if is_collection {
        None
    } else {
        Some(flavor_from_route(&context.flavor).ok_or_else(|| format!("unknown view kind '{}'", context.flavor))?)
    };
    let imported = write_imported_endpoints(state, context, copy_result)?;
    let Some(flavor) = flavor else {
        return crate::handlers::collection_ie::finish_blocking(state, context, imported);
    };
    let name = &context.name;
    let src = PathBuf::from(&context.src);
    let manifest = &context.manifest;
    let ImportedEndpoints { mapping, new_eids, skipped } = imported;

    // Build the view's index from what was just written - the same code
    // Create uses, so an imported view is indistinguishable from a created
    // one.
    let catalog = open_catalog(state)?;
    if catalog.get(flavor.kind(), name).map_err(|e| format!("{e:?}"))?.is_some() {
        // Someone created that name while the child was copying.
        for eid in &new_eids {
            rollback_endpoint(state, eid);
        }
        return Err(format!("{} '{name}' was created by someone else meanwhile - import again with another name", flavor.label()));
    }
    let dir = flavor.dir(state, name);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file_path = flavor.file_path(state, name);
    let created = (|| -> Result<(), String> {
        let membership = open_membership(&file_path)?;
        build_index(state, &membership, &new_eids)?;
        let query = state
            .queries
            .get_catalog_query(&flavor.catalog_query("CREATE"))
            .ok_or_else(|| format!("query {} is missing", flavor.catalog_query("CREATE")))?;
        catalog
            .core
            .proc(query, &[name, &file_path, &manifest.repoview.annotation, &manifest.repoview.source])
            .map_err(|e| format!("{e:?}"))?;
        let row_tags = flavor.row_tag_ops(state)?;
        for tag in &manifest.repoview.tags {
            let _ = row_tags.add(name, tag);
        }
        Ok(())
    })();
    if let Err(e) = created {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(format!(
            "the endpoints were imported but the {} couldn't be created: {e}",
            flavor.label()
        ));
    }

    // A WebView's front page comes back with it (it names no EIDs, so it is
    // copied as is - but only if it really is JSON).
    let front_page_restored = flavor == Flavor::Web
        && std::fs::read_to_string(src.join(FRONT_PAGE_FILE))
            .ok()
            .filter(|text| serde_json::from_str::<Value>(text).is_ok())
            .map(|text| std::fs::write(dir.join(FRONT_PAGE_FILE), text).is_ok())
            .unwrap_or(false);

    // A takeout without a (valid) front page still gets the default one.
    if flavor == Flavor::Web && !front_page_restored {
        let _ = crate::handlers::webview_front_page::ensure_default(&dir);
    }

    // The takeout's Tables name EIDs that no longer exist here, so rebuild
    // them (same approach) rather than copying stale files.
    let tables_regenerated = flavor.has_tables()
        && std::fs::read_to_string(src.join("Tables-meta.md"))
            .ok()
            .and_then(|meta| parse_approach_line(&meta))
            .and_then(|api_name| Approach::parse(Some(api_name)).ok())
            .map(|approach| generate_tables_blocking(state, name, approach, 100).is_ok())
            .unwrap_or(false);

    Ok(json!({
        "ok": skipped.is_empty(),
        "name": name,
        "endpoints_imported": new_eids.len(),
        "mapping": mapping,
        "skipped": skipped,
        "tables_regenerated": tables_regenerated,
        "front_page_restored": front_page_restored
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("edms-ie-{label}-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_manifest() -> Manifest {
        Manifest {
            format: MANIFEST_FORMAT.to_string(),
            version: MANIFEST_VERSION,
            repoview: ManifestRepoview {
                name: "rv".into(),
                annotation: Some("note".into()),
                source: Some("src".into()),
                tags: vec!["production".into()],
                created_at: "2026-10-06 10:00:00".into(),
                kind: "repoview".into(),
            },
            endpoints: vec![ManifestEndpoint {
                eid: "E0001-AAA".into(),
                endpoint_str: "https://x.com/users".into(),
                method: Some("GET".into()),
                annotation: None,
                tags: vec!["t1".into()],
                qps: vec![ManifestQp {
                    request_number: 1,
                    method: Some("GET".into()),
                    status_code: Some(200),
                    response_time_ms: Some(12),
                }],
                created_at: Some("2026-10-01 09:00:00".into()),
                updated_at: Some("2026-10-05 12:30:00".into()),
            }],
        }
    }

    #[test]
    fn a_manifest_survives_a_json_round_trip() {
        let m = sample_manifest();
        let text = serde_json::to_string_pretty(&m).unwrap();
        let back: Manifest = serde_json::from_str(&text).unwrap();
        assert_eq!(m, back);
        assert!(validate_manifest(&back).is_ok());
    }

    #[test]
    fn a_bad_manifest_is_refused() {
        let mut wrong_format = sample_manifest();
        wrong_format.format = "something-else".into();
        assert!(validate_manifest(&wrong_format).unwrap_err().contains("not a RepoView takeout"));

        let mut wrong_version = sample_manifest();
        wrong_version.version = 99;
        assert!(validate_manifest(&wrong_version).unwrap_err().contains("unsupported manifest version"));

        for bad_eid in ["../E0001-AAA", "E1", "", "E0001-AAA/../x"] {
            let mut m = sample_manifest();
            m.endpoints[0].eid = bad_eid.into();
            assert!(validate_manifest(&m).is_err(), "{bad_eid:?} must be rejected");
        }

        let mut dup = sample_manifest();
        dup.endpoints.push(dup.endpoints[0].clone());
        assert!(validate_manifest(&dup).unwrap_err().contains("twice"));

        let mut no_url = sample_manifest();
        no_url.endpoints[0].endpoint_str = "  ".into();
        assert!(validate_manifest(&no_url).is_err());
    }

    #[test]
    fn the_manifest_is_built_from_the_index() {
        let dir = temp_dir("manifest");
        let m = CollectionMembershipOps::new(&dir.join("repoview.sqlite").display().to_string());
        m.initialize().unwrap();
        init_snapshot_tables(&m).unwrap();
        for (eid, url, method) in [("E0002-AAA", "https://x.com/b", "POST"), ("E0001-AAA", "https://x.com/a", "GET")] {
            m.add(eid).unwrap();
            m.core
                .proc(
                    "INSERT INTO endpoint_snapshot (endpoint_id, endpoint_str, method, annotation, data_size_bytes) VALUES (?, ?, ?, NULL, 5)",
                    &[&eid, &url, &method],
                )
                .unwrap();
        }
        m.add_tag("E0001-AAA", "t1").unwrap();
        m.add_tag("E0001-AAA", "t2").unwrap();
        m.core
            .proc(
                "INSERT INTO qp_snapshot (endpoint_id, request_number, method, status_code, response_time_ms) VALUES ('E0001-AAA', 1, 'GET', 200, 7)",
                &[],
            )
            .unwrap();
        // a stray snapshot row for something that isn't a member must not leak in
        m.core
            .proc(
                "INSERT INTO endpoint_snapshot (endpoint_id, endpoint_str, method, annotation, data_size_bytes) VALUES ('E0099-AAA', 'https://x.com/ghost', 'GET', NULL, 0)",
                &[],
            )
            .unwrap();

        let manifest = build_manifest(&m, sample_manifest().repoview).unwrap();

        let eids: Vec<&str> = manifest.endpoints.iter().map(|e| e.eid.as_str()).collect();
        assert_eq!(eids, vec!["E0001-AAA", "E0002-AAA"]);
        assert_eq!(manifest.endpoints[0].tags, vec!["t1", "t2"]);
        assert_eq!(manifest.endpoints[0].qps.len(), 1);
        assert_eq!(manifest.endpoints[0].qps[0].status_code, Some(200));
        assert!(manifest.endpoints[1].tags.is_empty() && manifest.endpoints[1].qps.is_empty());
        assert!(validate_manifest(&manifest).is_ok());

        drop(m);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
