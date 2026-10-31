//! Import / Export ("takeout") for Collections - the same takeout and import
//! as a RepoView or WebView (`repoview_ie`), for a Collection.
//!
//! A Collection is not a list with an index of its own: it is a membership
//! file (`storage/collections/{name}.sqlite`, just the endpoint ids) plus
//! bookmark rows, and every endpoint's data lives in the central tables and
//! in `globalEQPData`. So a takeout has to describe the endpoints from the
//! central tables, and an import has to create everything a Collection is
//! made of:
//!
//! - **Takeout** (`POST /collections/:name/takeout`) copies the EQP data of
//!   every member out of globalEQPData into `storage/takeout/{name}/` and
//!   writes `repoview-manifest.json` (the same file and format as a RepoView's,
//!   with `kind: "collection"`): the collection's name, annotation, tags and,
//!   per endpoint, its URL, method, annotation, tags and QP status/timings. No
//!   SQLite file is ever in a takeout.
//! - **Import** (`POST /collections/import`) gives every endpoint a *fresh*
//!   EID, copies its EQP files renamed to match, writes the endpoint, QP and
//!   tag rows, and creates the Collection: its bookmark rows, membership file,
//!   catalog row and collection tags.
//!
//! Both are jobs (`GET /jobs/:id`, `/jobs/ws`) and use the same compute tasks
//! as RepoView/WebView (`view_takeout`, `view_unzip`, `view_import_copy`).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use compute::view_io::EqpEntry;
use edms::ops::{collection_tag_ops::CollectionTagMembershipOps, tag_ops::TagOps, view_ops::ViewKind};
use serde_json::{json, Value};

use crate::{
    db,
    events::ServerEvent,
    handlers::{
        repoview_ie::{
            begin_copy_job, load_manifest, plan_new_eids, respond, rollback_endpoint, takeout_root,
            ImportContext, ImportRequest, ImportedEndpoints, IeError, Manifest, ManifestEndpoint, ManifestQp,
            ManifestRepoview, TakeoutRequest, UnzipContext, MANIFEST_FILE, MANIFEST_FORMAT, MANIFEST_VERSION,
        },
        view_catalog::{collection_file_path, open_catalog, open_existing_collection_membership, open_membership, validate_folder_name},
    },
    state::AppState,
};

/// What a Collection's manifest and import job call themselves
/// (`manifest.repoview.kind`, `ImportContext.flavor`).
pub(crate) const KIND: &str = "collection";
/// The job's `view` (the same word the merge job uses).
const VIEW: &str = "collections";

/// Fixed routes under `/collections/` that a collection's own name would
/// shadow (`GET /collections/:name` could never reach it).
const RESERVED: [&str; 5] = ["list", "create", "import", "tags", "by-tag"];

/// A collection's name becomes a file name (`storage/collections/{name}.sqlite`)
/// and a route segment, so it follows the same rules as a RepoView's.
pub(crate) fn validate_collection_name(name: &str) -> Result<(), String> {
    validate_folder_name(name, "Collection name")?;
    if RESERVED.iter().any(|word| name.eq_ignore_ascii_case(word)) {
        return Err(format!(
            "'{name}' is reserved (it's a fixed /collections route) - pick another Collection name"
        ));
    }
    Ok(())
}

fn imports_dir(state: &AppState) -> std::path::PathBuf {
    state.storage_root.join("storage").join("imports").join("uncompressed").join("collections")
}

// ── The manifest, from the central tables ────────────────────────────────

/// The manifest for a Collection: the same shape as a RepoView's, `source`
/// left empty, endpoints in EID order so two takeouts of the same thing are
/// the same file.
pub(crate) fn assemble_manifest(
    name: &str,
    annotation: Option<String>,
    created_at: String,
    mut collection_tags: Vec<String>,
    mut endpoints: Vec<ManifestEndpoint>,
) -> Manifest {
    collection_tags.sort();
    collection_tags.dedup();
    endpoints.sort_by(|a, b| a.eid.cmp(&b.eid));
    Manifest {
        format: MANIFEST_FORMAT.to_string(),
        version: MANIFEST_VERSION,
        repoview: ManifestRepoview {
            name: name.to_string(),
            annotation,
            source: None,
            tags: collection_tags,
            created_at,
            kind: KIND.to_string(),
        },
        endpoints,
    }
}

/// Each member's endpoint row, tags and QP rows from the central tables.
/// Returns the endpoints it could describe, and the member ids that have no
/// endpoint row any more (a member can outlive its endpoint: deleting an
/// endpoint doesn't remove it from every Collection's file).
fn describe_members(state: &AppState, member_ids: &[String]) -> Result<(Vec<ManifestEndpoint>, Vec<String>), String> {
    let tag_ops = TagOps::new(&state.db_path.display().to_string());
    tag_ops.initialize().map_err(|e| format!("{e:?}"))?;

    let mut endpoints = Vec::new();
    let mut without_endpoint = Vec::new();
    for eid in member_ids {
        let Some(ep) = db::get_endpoint(&state.core, &state.queries, eid).map_err(|e| format!("{e:?}"))? else {
            without_endpoint.push(eid.clone());
            continue;
        };
        let mut tags = tag_ops.get_by_endpoint(eid).map_err(|e| format!("{e:?}"))?;
        tags.sort();
        tags.dedup();
        let (created_at, updated_at) = db::get_endpoint_dates(&state.core, eid)
            .map_err(|e| format!("{e:?}"))?
            .unwrap_or((None, None));
        let qps = db::list_qps_for_endpoint(&state.core, &state.queries, eid)
            .map_err(|e| format!("{e:?}"))?
            .into_iter()
            .map(|qp| ManifestQp {
                request_number: qp.request_number,
                method: qp.method,
                status_code: qp.status_code,
                response_time_ms: qp.response_time_ms,
            })
            .collect();
        endpoints.push(ManifestEndpoint {
            eid: ep.endpoint_id,
            endpoint_str: ep.endpoint_str,
            method: ep.method,
            annotation: ep.annotation,
            tags,
            qps,
            created_at,
            updated_at,
        });
    }
    Ok((endpoints, without_endpoint))
}

// ── Takeout (export) ─────────────────────────────────────────────────────

/// POST /collections/:name/takeout `{dest_name?, overwrite?}`
///
/// Copies the EQP data of every member into
/// `storage/takeout/{dest_name}/globalEQPData/{eid}/`, next to
/// `repoview-manifest.json`. A job: **202** `{job_id, ...}`; the result arrives
/// as a `ViewIoDone` event (op `takeout`, view `collections`). If that takeout
/// folder exists: **409** with `options: ["overwrite", "rename"]`, nothing
/// changes.
pub async fn takeout_collection(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(payload): Json<TakeoutRequest>,
) -> (StatusCode, Json<Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        let name = name.clone();
        move || takeout_blocking(&state, &name, payload.dest_name.as_deref(), payload.overwrite)
    })
    .await;
    respond(&state, StatusCode::ACCEPTED, res)
}

fn takeout_blocking(state: &AppState, name: &str, dest_name: Option<&str>, overwrite: bool) -> Result<Value, IeError> {
    let dest_name = dest_name.map(str::trim).filter(|d| !d.is_empty()).unwrap_or(name);
    validate_folder_name(dest_name, "Takeout name")?;

    let catalog = open_catalog(state)?;
    let row = catalog
        .get(ViewKind::Collections, name)
        .map_err(|e| format!("{e:?}"))?
        .ok_or_else(|| IeError::Bad(format!("Collection '{name}' does not exist")))?;
    let membership = open_existing_collection_membership(state, name).map_err(IeError::Bad)?;
    let member_ids = membership.list_ids().map_err(|e| format!("{e:?}"))?;

    let (endpoints, without_endpoint) = describe_members(state, &member_ids)?;
    if endpoints.is_empty() {
        return Err(IeError::Bad(format!("Collection '{name}' has no endpoints to take out")));
    }

    let collection_tags = {
        let ops = CollectionTagMembershipOps::new(&state.db_path.display().to_string());
        ops.initialize().map_err(|e| format!("{e:?}"))?;
        ops.list(name).map_err(|e| format!("{e:?}"))?
    };
    // (name, file_path, created_at, annotation)
    let manifest = assemble_manifest(name, row.3, row.2, collection_tags, endpoints);

    // Collisions are answered now (an immediate 409), not by the job later.
    let dest_dir = takeout_root(state).join("takeout").join(dest_name);
    if dest_dir.exists() && !overwrite {
        return Err(IeError::Conflict {
            message: format!("A takeout named '{dest_name}' already exists"),
            options: vec!["overwrite", "rename"],
        });
    }

    let eqp: Vec<EqpEntry> = manifest
        .endpoints
        .iter()
        .map(|e| EqpEntry { eid: e.eid.clone(), has_qps: !e.qps.is_empty() })
        .collect();
    let job_id = state.jobs.create("takeout", Some(VIEW), Some(name));
    let started = crate::ipc::spawn_child(
        "view_takeout",
        json!({
            "job_id": job_id,
            // no "view_dir": a Collection has no folder of its own
            "dest_name": dest_name,
            "storage_dir": takeout_root(state),
            "overwrite": overwrite,
            "eqp_root": state.storage_root.join("storage").join("globalEQPData"),
            "endpoints": eqp,
            "manifest": manifest,
            "manifest_file": MANIFEST_FILE,
            "create_meta_if_missing": false,
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
        "endpoints": manifest.endpoints.len(),
        // members whose endpoint no longer exists can't be described or copied
        "members_without_endpoint": without_endpoint,
    }))
}

// ── Import ───────────────────────────────────────────────────────────────

/// POST /collections/import `{folder? | zip?, name?, replace_extracted?}`
///
/// Reads `repoview-manifest.json` from a takeout under
/// `storage/imports/uncompressed/collections/` (or unzips a `.zip` from
/// `storage/imports/compressed/collections/` first) and builds a Collection:
/// every endpoint gets a **fresh EID**, its EQP files are copied under the new
/// name, its central rows and tags are written, and the Collection is created
/// with those members. A job (**202**); a taken name is **409**
/// `options: ["rename"]`.
pub async fn import_collection_takeout(
    State(state): State<AppState>,
    Json(payload): Json<ImportRequest>,
) -> (StatusCode, Json<Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        move || match (payload.folder.as_deref(), payload.zip.as_deref()) {
            (Some(folder), None) => import_blocking(&state, folder, payload.name.as_deref(), None),
            (None, Some(zip)) => import_zip_blocking(&state, zip, payload.name.as_deref(), payload.replace_extracted),
            _ => Err(IeError::Bad("give exactly one of `folder` (an unzipped takeout) or `zip` (a zipped one)".to_string())),
        }
    })
    .await;
    respond(&state, StatusCode::ACCEPTED, res)
}

fn name_is_taken(state: &AppState, name: &str) -> Result<bool, IeError> {
    Ok(open_catalog(state)?.get(ViewKind::Collections, name).map_err(|e| format!("{e:?}"))?.is_some())
}

/// Part 1 (see `repoview_ie`): validate, check the name, allocate the new
/// EIDs, start the child. Also where a zip import carries on once unzipped.
pub(crate) fn import_blocking(
    state: &AppState,
    folder: &str,
    name_override: Option<&str>,
    // A zip import has a job already (it started with the unzip).
    existing_job: Option<&str>,
) -> Result<Value, IeError> {
    validate_folder_name(folder, "Folder name")?;
    let src = imports_dir(state).join(folder);
    if !src.is_dir() {
        return Err(IeError::Bad(format!("'{folder}' was not found under storage/imports/uncompressed/collections/")));
    }

    let manifest = load_manifest(folder, &src, KIND)?;

    let name = name_override
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or(&manifest.repoview.name)
        .to_string();
    validate_collection_name(&name)?;
    if name_is_taken(state, &name)? {
        return Err(IeError::Conflict { message: format!("Collection '{name}' already exists"), options: vec!["rename"] });
    }

    let (plan, skipped) = plan_new_eids(state, &manifest)?;
    let job_id = begin_copy_job(state, VIEW, KIND, &name, existing_job, &src, &manifest, &plan, &skipped)?;

    Ok(json!({
        "ok": true,
        "status": "running",
        "job_id": job_id,
        "name": name,
        "endpoints": plan.len(),
        "skipped": skipped
    }))
}

/// `import` with a `zip`: the same job with an unzip in front (see
/// `repoview_ie::import_zip_blocking`). Unzips into
/// `storage/imports/uncompressed/collections/{zip name}/`.
fn import_zip_blocking(state: &AppState, zip: &str, name_override: Option<&str>, replace: bool) -> Result<Value, IeError> {
    validate_folder_name(zip, "Zip name")?;
    let Some(stem) = zip.strip_suffix(".zip").or_else(|| zip.strip_suffix(".ZIP")) else {
        return Err(IeError::Bad(format!("'{zip}' is not a .zip file")));
    };
    validate_folder_name(stem, "Zip name")?;
    let zip_path = state.storage_root.join("storage").join("imports").join("compressed").join("collections").join(zip);
    if !zip_path.is_file() {
        return Err(IeError::Bad(format!("'{zip}' was not found under storage/imports/compressed/collections/")));
    }

    let dest = imports_dir(state).join(stem);
    if dest.exists() && !replace {
        return Err(IeError::Conflict {
            message: format!("'{stem}' is already unzipped under storage/imports/uncompressed/collections/"),
            options: vec!["replace"],
        });
    }
    // A name given up front can be checked now; one taken from the manifest
    // can only be checked once the zip is open.
    let name_override = name_override.map(str::trim).filter(|n| !n.is_empty());
    if let Some(name) = name_override {
        validate_collection_name(name)?;
        if name_is_taken(state, name)? {
            return Err(IeError::Conflict { message: format!("Collection '{name}' already exists"), options: vec!["rename"] });
        }
    }

    let job_id = state.jobs.create("import", Some(VIEW), Some(stem));
    let context = UnzipContext {
        kind: "unzip".to_string(),
        flavor: KIND.to_string(),
        folder: stem.to_string(),
        name: name_override.map(str::to_string),
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

/// Part 3, once the endpoints are written (`repoview_ie::write_imported_endpoints`):
/// create the Collection those endpoints belong to - a bookmark row for each
/// (a member has to be bookmarked into its Collection first), the membership
/// file, the catalog row and the collection's own tags.
pub(crate) fn finish_blocking(state: &AppState, context: &ImportContext, imported: ImportedEndpoints) -> Result<Value, String> {
    let name = &context.name;
    let manifest = &context.manifest;
    let ImportedEndpoints { mapping, new_eids, skipped } = imported;

    let catalog = open_catalog(state)?;
    if catalog.get(ViewKind::Collections, name).map_err(|e| format!("{e:?}"))?.is_some() {
        // Someone created that name while the child was copying.
        for eid in &new_eids {
            rollback_endpoint(state, eid);
        }
        return Err(format!("Collection '{name}' was created by someone else meanwhile - import again with another name"));
    }

    let file_path = collection_file_path(state, name);
    let created = (|| -> Result<usize, String> {
        let membership = open_membership(&file_path)?;
        for eid in &new_eids {
            db::insert_bookmark(&state.core, &state.queries, eid, name, None).map_err(|e| format!("{e:?}"))?;
        }
        membership.add_batch(&new_eids).map_err(|e| format!("{e:?}"))?;
        catalog
            .register(ViewKind::Collections, name, Some(&file_path), manifest.repoview.annotation.as_deref())
            .map_err(|e| format!("{e:?}"))?;

        let tag_ops = CollectionTagMembershipOps::new(&state.db_path.display().to_string());
        tag_ops.initialize().map_err(|e| format!("{e:?}"))?;
        let mut restored = 0;
        for tag in &manifest.repoview.tags {
            if tag_ops.add(name, tag).map(|n| n > 0).unwrap_or(false) {
                restored += 1;
            }
        }
        Ok(restored)
    })();
    let collection_tags_restored = match created {
        Ok(n) => n,
        Err(e) => {
            // Leave no half-made Collection behind. The endpoints themselves
            // were written and stay, as they do for a RepoView.
            let _ = catalog.remove(ViewKind::Collections, name);
            let _ = db::delete_bookmarks_for_folder(&state.core, &state.queries, name);
            let _ = std::fs::remove_file(&file_path);
            return Err(format!("the endpoints were imported but the Collection couldn't be created: {e}"));
        }
    };

    let _ = state.events_tx.send(ServerEvent::CollectionMembershipUpdated { collection: name.clone() });

    Ok(json!({
        "ok": skipped.is_empty(),
        "name": name,
        "endpoints_imported": new_eids.len(),
        "mapping": mapping,
        "skipped": skipped,
        "collection_tags_restored": collection_tags_restored,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint(eid: &str, tags: &[&str]) -> ManifestEndpoint {
        ManifestEndpoint {
            eid: eid.into(),
            endpoint_str: format!("https://x.com/{eid}"),
            method: Some("GET".into()),
            annotation: None,
            tags: tags.iter().map(|t| t.to_string()).collect(),
            qps: vec![ManifestQp { request_number: 1, method: Some("GET".into()), status_code: Some(200), response_time_ms: Some(5) }],
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn collection_names_follow_the_file_name_rules_and_avoid_fixed_routes() {
        assert!(validate_collection_name("release-candidate").is_ok());
        for bad in ["", "..", "a/b", "a:b", "  x", "x.", "list", "IMPORT", "create", "tags", "by-tag"] {
            assert!(validate_collection_name(bad).is_err(), "{bad:?}");
        }
        assert!(validate_collection_name("import").unwrap_err().contains("/collections route"));
    }

    #[test]
    fn the_manifest_is_a_collections_one_in_a_stable_order() {
        let m = assemble_manifest(
            "auth-suite",
            Some("note".into()),
            "2026-10-01 10:00:00".into(),
            vec!["team-b".into(), "team-a".into(), "team-a".into()],
            vec![endpoint("E0003-AAA", &["x"]), endpoint("E0001-AAA", &[])],
        );
        assert_eq!(m.format, MANIFEST_FORMAT);
        assert_eq!(m.version, MANIFEST_VERSION);
        assert_eq!(m.repoview.kind, "collection");
        assert_eq!(m.repoview.source, None, "a Collection has no source");
        assert_eq!(m.repoview.tags, vec!["team-a".to_string(), "team-b".to_string()], "sorted, no repeats");
        assert_eq!(m.endpoints.iter().map(|e| e.eid.as_str()).collect::<Vec<_>>(), ["E0001-AAA", "E0003-AAA"]);
        // it is the same file a RepoView's import already validates
        assert!(crate::handlers::repoview_ie::validate_manifest(&m).is_ok());
        // and it survives the trip through JSON
        let back: Manifest = serde_json::from_str(&serde_json::to_string_pretty(&m).unwrap()).unwrap();
        assert_eq!(m, back);
    }

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("edms-colie-{label}-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_takeout_of_another_kind_is_refused_with_the_route_that_takes_it() {
        let dir = temp_dir("kinds");
        let mut m = assemble_manifest("c", None, "t".into(), vec![], vec![endpoint("E0001-AAA", &[])]);
        std::fs::write(dir.join(MANIFEST_FILE), serde_json::to_string(&m).unwrap()).unwrap();
        // a Collection takeout is fine for the Collection import...
        assert!(load_manifest("tk", &dir, KIND).is_ok());
        // ...but not for a RepoView or WebView import, which is told where it goes
        for other in ["repoview", "webview"] {
            let err = match load_manifest("tk", &dir, other) {
                Err(IeError::Bad(why)) => why,
                _ => panic!("should be refused"),
            };
            assert!(err.contains("is a collection takeout") && err.contains("/collections/import"), "{err}");
        }
        // and a RepoView takeout sent to the Collection import is pointed at /repoview/import
        m.repoview.kind = "repoview".into();
        std::fs::write(dir.join(MANIFEST_FILE), serde_json::to_string(&m).unwrap()).unwrap();
        let err = match load_manifest("tk", &dir, KIND) {
            Err(IeError::Bad(why)) => why,
            _ => panic!("should be refused"),
        };
        assert!(err.contains("/repoview/import"), "{err}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_missing_or_broken_manifest_is_a_clear_error() {
        let dir = temp_dir("broken");
        let err = match load_manifest("tk", &dir, KIND) {
            Err(IeError::Bad(why)) => why,
            _ => panic!("no manifest"),
        };
        assert!(err.contains("can't read"), "{err}");
        std::fs::write(dir.join(MANIFEST_FILE), "{ not json").unwrap();
        let err = match load_manifest("tk", &dir, KIND) {
            Err(IeError::Bad(why)) => why,
            _ => panic!("bad json"),
        };
        assert!(err.contains("is not valid"), "{err}");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
