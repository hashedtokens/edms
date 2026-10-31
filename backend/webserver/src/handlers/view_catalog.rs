//! Catalog registration for collections/webview/repoview — which instances
//! exist, per Ravi's schema (2026-08-25). `file_path` (the independent
//! SQLite file each one gets) isn't created for webview/repoview by this
//! pass yet — those catalog rows still get `file_path: null`.
//!
//! Collections are further along (per Ravi, 2026-09-03): each one gets its
//! own real SQLite file under `storage/collections/{name}.sqlite`, holding
//! just endpoint_id + when it was added — never a copy of the endpoint's
//! actual data, which always stays in the central `endpoints` table.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use edms::ops::collection_membership_ops::CollectionMembershipOps;
use edms::ops::tag_ops::TagOps;
use edms::ops::view_ops::{ViewCatalogOps, ViewKind};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;

use crate::{db, handlers::view_flavor::Flavor, state::AppState};

#[derive(Debug, Deserialize)]
pub struct RegisterViewRequest {
    pub name: String,
    #[serde(default)]
    pub annotation: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EndpointIdRequest {
    pub endpoint_id: String,
}

pub(crate) fn open_catalog(state: &AppState) -> Result<ViewCatalogOps, String> {
    let ops = ViewCatalogOps::new(&state.db_path.display().to_string());
    ops.initialize().map_err(|e| format!("{e:?}"))?;
    Ok(ops)
}

/// Where a collection's own file lives on disk — matches the Sept-1 schema
/// path, and the directory is guaranteed to already exist by the folder
/// init work (`storage/collections`) that runs on every launch.
pub(crate) fn collection_file_path(state: &AppState, name: &str) -> String {
    state
        .storage_root
        .join("storage")
        .join("collections")
        .join(format!("{name}.sqlite"))
        .display()
        .to_string()
}

pub(crate) fn open_membership(file_path: &str) -> Result<CollectionMembershipOps, String> {
    let ops = CollectionMembershipOps::new(file_path);
    ops.initialize().map_err(|e| format!("{e:?}"))?;
    Ok(ops)
}

/// A RepoView gets its own directory, unlike a collection's single flat file:
/// the SQLite index (`repoview.sqlite`) plus the generated `Tables-*.md`.
/// No EQP data lives here (v1.0) - that is only copied at Import/Export.
/// (WebView's is the same shape: `Flavor::dir`.)
pub(crate) fn repoview_dir(state: &AppState, name: &str) -> std::path::PathBuf {
    Flavor::Repo.dir(state, name)
}

/// A RepoView's name becomes a real folder name (`storage/repoview/{name}`)
/// and `delete` removes that whole folder, so it must never be able to point
/// anywhere else: `..` would resolve to `storage/` itself. See
/// `Flavor::validate_name`, which this and the WebView equivalent share.
pub(crate) fn validate_repoview_name(name: &str) -> Result<(), String> {
    Flavor::Repo.validate_name(name)
}

/// The same rules for any name that becomes a file or folder name - used for
/// the Collection that "Convert to Collection" creates too (its file is
/// `storage/collections/{name}.sqlite`). `what` is only for the message.
pub(crate) fn validate_folder_name(name: &str, what: &str) -> Result<(), String> {
    const FORBIDDEN: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    if name.trim().is_empty() {
        return Err(format!("{what} can't be empty"));
    }
    if name == "." || name == ".." {
        return Err(format!("'{name}' isn't a valid {what}"));
    }
    if name != name.trim() || name.ends_with('.') {
        return Err(format!("{what} can't start or end with a space, or end with a dot"));
    }
    if name.chars().count() > 100 {
        return Err(format!("{what} can't be longer than 100 characters"));
    }
    if let Some(bad) = name.chars().find(|c| FORBIDDEN.contains(c) || c.is_control()) {
        return Err(format!("{what} can't contain {bad:?} (it becomes a file or folder name)"));
    }
    Ok(())
}

/// Recursively copies `src` into `dst`, creating directories as needed.
/// Blocking — callers run this inside `spawn_blocking`. Best-effort at the
/// call site: a source endpoint directory that doesn't exist yet (no QPs
/// recorded) isn't an error, it's just an EID with nothing to copy.
pub(crate) fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Looks up an existing catalog entry by kind + name and opens its
/// membership file. Shared by every handler that operates on a specific,
/// already-created collection/repoview (add/remove/list, and load/save/
/// unsave in bookmarks.rs for collections) — one place that defines "does
/// this entry exist and where's its file."
pub(crate) fn open_existing_membership(
    state: &AppState,
    kind: ViewKind,
    name: &str,
) -> Result<CollectionMembershipOps, String> {
    let catalog = open_catalog(state)?;
    let row = catalog
        .get(kind, name)
        .map_err(|e| format!("{e:?}"))?
        .ok_or_else(|| format!("'{name}' does not exist"))?;
    let path = row.1.ok_or_else(|| format!("'{name}' has no file yet"))?;
    open_membership(&path)
}

/// Collections' own lookup, kept exactly as it was before RepoView existed
/// (including its "Collection '...'" error wording) — RepoView uses the
/// generalized `open_existing_membership` above instead, so nothing about
/// Collections' behavior changed.
pub(crate) fn open_existing_collection_membership(
    state: &AppState,
    name: &str,
) -> Result<CollectionMembershipOps, String> {
    let catalog = open_catalog(state)?;
    let row = catalog
        .get(ViewKind::Collections, name)
        .map_err(|e| format!("{e:?}"))?
        .ok_or_else(|| format!("Collection '{name}' does not exist"))?;
    let path = row.1.ok_or_else(|| format!("Collection '{name}' has no file yet"))?;
    open_membership(&path)
}

/// POST /collections/create — registers the catalog row AND creates the
/// collection's own SQLite file, storing its path in `file_path`.
pub async fn create_collection_entry(
    State(state): State<AppState>,
    Json(payload): Json<RegisterViewRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        let name = payload.name.clone();
        let annotation = payload.annotation.clone();
        move || -> Result<(usize, String), String> {
            let path = collection_file_path(&state, &name);

            // Create the file + its schema first — if this fails, we don't
            // want a catalog row pointing at a file that doesn't exist.
            open_membership(&path)?;

            let catalog = open_catalog(&state)?;
            let inserted = catalog
                .register(ViewKind::Collections, &name, Some(&path), annotation.as_deref())
                .map_err(|e| format!("{e:?}"))?;
            Ok((inserted, path))
        }
    })
    .await;

    match res {
        Ok(Ok((inserted, path))) => {
            state.refresh_dashboard_snapshot();
            (
                StatusCode::OK,
                Json(json!({ "ok": true, "inserted": inserted, "file_path": path })),
            )
        }
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

/// GET /collections/:name — one collection's catalog row, not the full list.
pub async fn get_collection_entry(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        let name = name.clone();
        move || -> Result<Option<(String, Option<String>, String, Option<String>, Option<i64>)>, String> {
            let catalog = open_catalog(&state)?;
            let row = catalog.get(ViewKind::Collections, &name).map_err(|e| format!("{e:?}"))?;
            Ok(row.map(|(name, file_path, created_at, annotation)| {
                let count = file_path
                    .as_deref()
                    .and_then(|p| open_membership(p).ok())
                    .and_then(|m| m.count().ok());
                (name, file_path, created_at, annotation, count)
            }))
        }
    })
    .await;

    match res {
        Ok(Ok(Some((name, file_path, created_at, annotation, endpoint_count)))) => (
            StatusCode::OK,
            Json(json!({
                "ok": true, "name": name, "file_path": file_path,
                "created_at": created_at, "annotation": annotation,
                "endpoint_count": endpoint_count
            })),
        ),
        Ok(Ok(None)) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "ok": false, "error": format!("Collection '{name}' does not exist") })),
        ),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct AnnotateViewRequest {
    pub annotation: String,
}

/// POST /collections/:name/annotation — sets a collection's annotation.
pub async fn annotate_collection_entry(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(payload): Json<AnnotateViewRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        let name = name.clone();
        let annotation = payload.annotation.clone();
        move || -> Result<usize, String> {
            let catalog = open_catalog(&state)?;
            if catalog
                .get(ViewKind::Collections, &name)
                .map_err(|e| format!("{e:?}"))?
                .is_none()
            {
                return Err(format!("Collection '{name}' does not exist"));
            }
            catalog
                .annotate(ViewKind::Collections, &name, &annotation)
                .map_err(|e| format!("{e:?}"))
        }
    })
    .await;

    match res {
        Ok(Ok(rows)) => (StatusCode::OK, Json(json!({ "ok": true, "updated_rows": rows }))),
        Ok(Err(e)) => (StatusCode::NOT_FOUND, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct RenameViewRequest {
    pub new_name: String,
}

/// POST /collections/:name/rename — renames the catalog entry AND moves
/// the collection's own file on disk to match, so name and file basename
/// never drift apart.
pub async fn rename_collection_entry(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(payload): Json<RenameViewRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        let old_name = name.clone();
        let new_name = payload.new_name.clone();
        move || -> Result<usize, String> {
            let catalog = open_catalog(&state)?;
            let row = catalog
                .get(ViewKind::Collections, &old_name)
                .map_err(|e| format!("{e:?}"))?
                .ok_or_else(|| format!("Collection '{old_name}' does not exist"))?;
            let old_path = row.1.ok_or_else(|| format!("Collection '{old_name}' has no file yet"))?;

            // Reject a name collision before touching anything — a plain
            // file rename onto an existing path would silently overwrite
            // it, which would destroy that other collection's data.
            if catalog
                .get(ViewKind::Collections, &new_name)
                .map_err(|e| format!("{e:?}"))?
                .is_some()
            {
                return Err(format!("Collection '{new_name}' already exists"));
            }

            let new_path = collection_file_path(&state, &new_name);

            // Update the catalog row first — its UNIQUE constraint is the
            // real safety net against a race (two renames to the same new
            // name at once), and if it fails, the file is never touched.
            let rows = catalog
                .rename(ViewKind::Collections, &old_name, &new_name, Some(&new_path))
                .map_err(|e| {
                    if db::is_unique_violation(&e) {
                        format!("Collection '{new_name}' already exists")
                    } else {
                        format!("{e:?}")
                    }
                })?;

            // Now move the file to match. If this fails, roll back the
            // catalog row so it doesn't point at a path that doesn't
            // actually hold the renamed file.
            if let Err(e) = std::fs::rename(&old_path, &new_path) {
                let _ = catalog.rename(ViewKind::Collections, &new_name, &old_name, Some(&old_path));
                return Err(format!("failed to rename collection file, rolled back: {e}"));
            }

            // Bookmarks live in the central table keyed by folder = collection
            // name (2026-09-22) — unlike membership/tags, which moved with the
            // file automatically, these need an explicit cascade or they'd be
            // stranded under the old name.
            let _ = db::rename_bookmarks_folder(&state.core, &state.queries, &old_name, &new_name);

            Ok(rows)
        }
    })
    .await;

    match res {
        Ok(Ok(rows)) => {
            state.refresh_dashboard_snapshot();
            (StatusCode::OK, Json(json!({ "ok": true, "renamed_rows": rows })))
        }
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

/// POST /collections/:name/delete — removes the catalog row and deletes
/// the collection's own file from disk.
pub async fn delete_collection_entry(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        let name = name.clone();
        move || -> Result<(usize, bool), String> {
            let catalog = open_catalog(&state)?;
            let existing = catalog
                .get(ViewKind::Collections, &name)
                .map_err(|e| format!("{e:?}"))?;

            let deleted_rows = catalog
                .remove(ViewKind::Collections, &name)
                .map_err(|e| format!("{e:?}"))?;

            let mut file_deleted = false;
            if let Some((_, Some(file_path), _, _)) = existing {
                if std::path::Path::new(&file_path).exists() {
                    std::fs::remove_file(&file_path).map_err(|e| e.to_string())?;
                    file_deleted = true;
                }
            }

            // Same cascade need as rename above: bookmarks for this
            // collection live in the central table (folder = name), not in
            // the file that was just deleted, so they'd be orphaned
            // otherwise (2026-09-22).
            let _ = db::delete_bookmarks_for_folder(&state.core, &state.queries, &name);

            Ok((deleted_rows, file_deleted))
        }
    })
    .await;

    match res {
        Ok(Ok((deleted_rows, file_deleted))) => {
            state.refresh_dashboard_snapshot();
            (
                StatusCode::OK,
                Json(json!({ "ok": true, "deleted_rows": deleted_rows, "file_deleted": file_deleted })),
            )
        }
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

/// POST /collections/:name/endpoints/remove
///
/// Direct removal stays available (removal doesn't carry the same "must be
/// deliberately curated via testing" risk as addition — see the merged
/// bookmark/collection design, Mathew 2026-09-08). Addition, by contrast,
/// no longer has a direct route here: the only way an endpoint enters a
/// collection now is bookmarks.rs's save-to-collection action, which
/// requires it to already be a bookmarked member of the loaded collection's
/// active workspace first.
pub async fn remove_endpoint_from_collection(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(payload): Json<EndpointIdRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        let name = name.clone();
        let endpoint_id = payload.endpoint_id.clone();
        move || -> Result<usize, String> {
            let membership = open_existing_collection_membership(&state, &name)?;
            membership.remove(&endpoint_id).map_err(|e| format!("{e:?}"))
        }
    })
    .await;

    match res {
        Ok(Ok(deleted)) => (StatusCode::OK, Json(json!({ "ok": true, "deleted": deleted }))),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

/// GET /collections/:name/endpoints — list this collection's members.
pub async fn list_collection_endpoints(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        let name = name.clone();
        move || -> Result<Vec<(String, String)>, String> {
            let membership = open_existing_collection_membership(&state, &name)?;
            let entries = membership.list().map_err(|e| format!("{e:?}"))?;
            Ok(entries.into_iter().map(|e| (e.endpoint_id, e.added_at)).collect())
        }
    })
    .await;

    match res {
        Ok(Ok(entries)) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "endpoints": entries.into_iter().map(|(endpoint_id, added_at)| json!({
                    "endpoint_id": endpoint_id, "added_at": added_at
                })).collect::<Vec<_>>()
            })),
        ),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

/// GET /collections/list — like the generic `list_for`, but also folds in
/// each collection's `endpoint_count` (from its own membership file —
/// `CollectionMembershipOps::count()` already existed, just unused here).
/// `null` if the collection has no file yet.
pub async fn list_collections(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        move || -> Result<Vec<(String, Option<String>, String, Option<String>, Option<i64>)>, String> {
            let catalog = open_catalog(&state)?;
            let rows = catalog.list(ViewKind::Collections).map_err(|e| format!("{e:?}"))?;
            Ok(rows
                .into_iter()
                .map(|(name, file_path, created_at, annotation)| {
                    let count = file_path
                        .as_deref()
                        .and_then(|p| open_membership(p).ok())
                        .and_then(|m| m.count().ok());
                    (name, file_path, created_at, annotation, count)
                })
                .collect())
        }
    })
    .await;

    match res {
        Ok(Ok(rows)) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "items": rows.into_iter().map(|(name, file_path, created_at, annotation, endpoint_count)| json!({
                    "name": name, "file_path": file_path, "created_at": created_at,
                    "annotation": annotation, "endpoint_count": endpoint_count
                })).collect::<Vec<_>>()
            })),
        ),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

// ── RepoView / WebView ───────────────────────────────────────────────────────────
//
// v1.0 (Ravi, 2026-10-05/06): a RepoView is a LIST - a SQLite index (members
// plus a snapshot of their endpoint rows, tags and QP metadata) and the
// Tables-*.md files. It holds no EQP data; that is copied out of
// globalEQPData only at Import/Export (takeout) time, which keeps Create
// instant (the spec's "zero time" op). It's created FROM a Collection;
// "Bookmark Tag" / Source records which one.

#[derive(Debug, Deserialize)]
pub struct CreateRepoviewRequest {
    pub name: String,
    #[serde(default)]
    pub annotation: Option<String>,
    pub source_collection: String,
    /// Which members of `source_collection` to list. Omitted or empty
    /// means "all of them".
    #[serde(default)]
    pub endpoint_ids: Option<Vec<String>>,
}

/// POST /repoview/create - builds the list: registers the catalog row (with
/// `source`) and indexes the chosen members of `source_collection` into the
/// RepoView's own SQLite. No EQP data is copied.
pub async fn create_repoview_entry(
    State(state): State<AppState>,
    Extension(flavor): Extension<Flavor>,
    Json(payload): Json<CreateRepoviewRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        let name = payload.name.clone();
        let annotation = payload.annotation.clone();
        let source_collection = payload.source_collection.clone();
        let requested_ids = payload.endpoint_ids.clone();
        move || -> Result<serde_json::Value, String> {
            flavor.validate_name(&name)?;
            let catalog = open_catalog(&state)?;
            if catalog
                .get(flavor.kind(), &name)
                .map_err(|e| format!("{e:?}"))?
                .is_some()
            {
                return Err(format!("{} \'{name}\' already exists", flavor.label()));
            }

            let source_membership =
                open_existing_membership(&state, ViewKind::Collections, &source_collection)
                    .map_err(|_| format!("Collection '{source_collection}' does not exist"))?;
            let source_members = source_membership.list_set().map_err(|e| format!("{e:?}"))?;

            let endpoint_ids: Vec<String> = match requested_ids {
                Some(ids) if !ids.is_empty() => {
                    let missing: Vec<&String> =
                        ids.iter().filter(|id| !source_members.contains(*id)).collect();
                    if !missing.is_empty() {
                        return Err(format!(
                            "not members of collection '{source_collection}': {missing:?}"
                        ));
                    }
                    ids
                }
                _ => source_members.into_iter().collect(),
            };

            let dir = flavor.dir(&state, &name);
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

            let file_path = flavor.file_path(&state, &name);
            let membership = open_membership(&file_path)?;

            let stats = crate::handlers::repoview_index::build_index(&state, &membership, &endpoint_ids)
                .map_err(|e| {
                    // Same rollback as a failed catalog insert below: don't
                    // leave a half-built folder behind.
                    let _ = std::fs::remove_dir_all(&dir);
                    e
                })?;
            if flavor == Flavor::Web {
                crate::handlers::webview_front_page::ensure_default(&dir).map_err(|e| {
                    let _ = std::fs::remove_dir_all(&dir);
                    e
                })?;
            }

            let query = state
                .queries
                .get_catalog_query(&flavor.catalog_query("CREATE"))
                .ok_or(edms::error::EdmsError::UnknownError)
                .map_err(|e| format!("{e:?}"))?;
            catalog
                .core
                .proc(query, &[&name, &file_path, &annotation, &source_collection])
                .map_err(|e| {
                    // Roll back the directory we just created so a failed
                    // create doesn't leave an orphaned folder behind.
                    let _ = std::fs::remove_dir_all(&dir);
                    format!("{e:?}")
                })?;

            Ok(json!({
                "ok": true,
                "name": name,
                "file_path": file_path,
                "source": source_collection,
                "endpoints_added": stats.added,
                "tags_copied": stats.tags_copied,
                "qps_snapshotted": stats.qps_snapshotted,
                "data_size_bytes": stats.data_size_bytes,
                "endpoints_requested": endpoint_ids.len()
            }))
        }
    })
    .await;

    match res {
        Ok(Ok(body)) => {
            state.refresh_dashboard_snapshot();
            (StatusCode::OK, Json(body))
        }
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

/// POST /repoview/:name/rename — renames the catalog entry AND moves the
/// whole RepoView directory (not just one file, unlike a collection) to
/// match.
pub async fn rename_repoview_entry(
    State(state): State<AppState>,
    Extension(flavor): Extension<Flavor>,
    Path(name): Path<String>,
    Json(payload): Json<RenameViewRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        let old_name = name.clone();
        let new_name = payload.new_name.clone();
        move || -> Result<usize, String> {
            flavor.validate_name(&new_name)?;
            let catalog = open_catalog(&state)?;
            if catalog
                .get(flavor.kind(), &old_name)
                .map_err(|e| format!("{e:?}"))?
                .is_none()
            {
                return Err(format!("{} \'{old_name}\' does not exist", flavor.label()));
            }
            if catalog
                .get(flavor.kind(), &new_name)
                .map_err(|e| format!("{e:?}"))?
                .is_some()
            {
                return Err(format!("{} \'{new_name}\' already exists", flavor.label()));
            }

            let old_dir = flavor.dir(&state, &old_name);
            let new_dir = flavor.dir(&state, &new_name);
            let new_path = flavor.file_path(&state, &new_name);

            let rows = catalog
                .rename(flavor.kind(), &old_name, &new_name, Some(&new_path))
                .map_err(|e| {
                    if db::is_unique_violation(&e) {
                        format!("{} \'{new_name}\' already exists", flavor.label())
                    } else {
                        format!("{e:?}")
                    }
                })?;

            if let Err(e) = std::fs::rename(&old_dir, &new_dir) {
                let old_path = flavor.file_path(&state, &old_name);
                let _ = catalog.rename(flavor.kind(), &new_name, &old_name, Some(&old_path));
                return Err(format!("failed to rename {} folder, rolled back: {e}", flavor.label()));
            }
            // Carry its row-level tags to the new name (they're keyed by name).
            let (tag_table, name_col) = flavor.tag_table();
            let _ = catalog.core.proc(
                &format!("UPDATE {tag_table} SET {name_col} = ? WHERE {name_col} = ?"),
                &[&new_name, &old_name],
            );

            Ok(rows)
        }
    })
    .await;

    match res {
        Ok(Ok(rows)) => {
            state.refresh_dashboard_snapshot();
            (StatusCode::OK, Json(json!({ "ok": true, "renamed_rows": rows })))
        }
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

/// POST /repoview/:name/annotation — sets a RepoView's annotation. The
/// generic `annotate()` op already handles this correctly; `source`
/// doesn't change here.
pub async fn annotate_repoview_entry(
    State(state): State<AppState>,
    Extension(flavor): Extension<Flavor>,
    Path(name): Path<String>,
    Json(payload): Json<AnnotateViewRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        let name = name.clone();
        let annotation = payload.annotation.clone();
        move || -> Result<usize, String> {
            let catalog = open_catalog(&state)?;
            if catalog
                .get(flavor.kind(), &name)
                .map_err(|e| format!("{e:?}"))?
                .is_none()
            {
                return Err(format!("{} \'{name}\' does not exist", flavor.label()));
            }
            catalog
                .annotate(flavor.kind(), &name, &annotation)
                .map_err(|e| format!("{e:?}"))
        }
    })
    .await;

    match res {
        Ok(Ok(rows)) => (StatusCode::OK, Json(json!({ "ok": true, "updated_rows": rows }))),
        Ok(Err(e)) => (StatusCode::NOT_FOUND, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

/// POST /repoview/:name/delete — removes the catalog row and deletes the
/// whole view directory from disk.
/// Shared by the single-name route below and the bulk-delete route — one
/// place that defines "delete this one RepoView's catalog row + folder."
fn delete_view_sync(flavor: Flavor, state: &AppState, name: &str) -> Result<(usize, bool), String> {
    let catalog = open_catalog(state)?;
    if catalog
        .get(flavor.kind(), name)
        .map_err(|e| format!("{e:?}"))?
        .is_none()
    {
        return Err(format!("{} \'{name}\' does not exist", flavor.label()));
    }

    let deleted_rows = catalog
        .remove(flavor.kind(), name)
        .map_err(|e| format!("{e:?}"))?;
    // Its row-level tags live in a central table keyed by name; leave them
    // and `GET /repoview/by-tag/:tag` keeps returning a RepoView that's gone.
    let (tag_table, name_col) = flavor.tag_table();
    let _ = catalog.core.proc(&format!("DELETE FROM {tag_table} WHERE {name_col} = ?"), &[&name]);

    // Re-check the name before touching the disk: `remove_dir_all` on a path
    // built from an unvalidated name is the most destructive call in here. A
    // row with an unsafe name (shouldn't exist, but) still gets its catalog
    // entry removed; its folder is left alone.
    let dir = flavor.dir(&state, name);
    let mut dir_deleted = false;
    if flavor.validate_name(name).is_ok() && dir.is_dir() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        dir_deleted = true;
    }

    Ok((deleted_rows, dir_deleted))
}

pub async fn delete_repoview_entry(
    State(state): State<AppState>,
    Extension(flavor): Extension<Flavor>,
    Path(name): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        let name = name.clone();
        move || delete_view_sync(flavor, &state, &name)
    })
    .await;

    match res {
        Ok(Ok((deleted_rows, dir_deleted))) => {
            state.refresh_dashboard_snapshot();
            (
                StatusCode::OK,
                Json(json!({ "ok": true, "deleted_rows": deleted_rows, "dir_deleted": dir_deleted })),
            )
        }
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct NamesRequest {
    pub names: Vec<String>,
}

/// POST /repoview/delete — multi-select delete, per the RepoView spec's
/// "select multiple entries in the table and delete them" control. Each
/// name is deleted independently with its own result, so one bad name in
/// the batch doesn't block the rest — same philosophy as
/// ViewTagCountOps::delete_many.
pub async fn delete_repoviews_bulk(
    State(state): State<AppState>,
    Extension(flavor): Extension<Flavor>,
    Json(payload): Json<NamesRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let names = payload.names;
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        move || -> Vec<serde_json::Value> {
            names
                .into_iter()
                .map(|name| match delete_view_sync(flavor, &state, &name) {
                    Ok((deleted_rows, dir_deleted)) => json!({
                        "name": name, "ok": true,
                        "deleted_rows": deleted_rows, "dir_deleted": dir_deleted
                    }),
                    Err(e) => json!({ "name": name, "ok": false, "error": e }),
                })
                .collect()
        }
    })
    .await;

    match res {
        Ok(results) => {
            state.refresh_dashboard_snapshot();
            let all_ok = results.iter().all(|r| r["ok"].as_bool().unwrap_or(false));
            (StatusCode::OK, Json(json!({ "ok": all_ok, "results": results })))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

/// POST /repoview/:name/duplicate — clones the whole RepoView folder
/// (membership file, copied EQP data, and any generated Tables-*.md) as-is
/// under a new name, per the proposal sent to Ravi: the underlying data
/// doesn't change on duplicate, so the batched tables are still correct
/// the instant they're copied — no need to regenerate them. Checked: the
/// generated Tables-meta.md doesn't embed the RepoView's own name/
/// created_at/source anywhere, so there's nothing to patch post-copy
/// either — a straight directory copy is the whole operation.
///
/// Also copies this RepoView's own row-level membership-tags (the
/// modifiable "Tags" field) — those are part of the row's identity, not
/// the data, so they come along with the clone same as annotation does.
pub async fn duplicate_repoview_entry(
    State(state): State<AppState>,
    Extension(flavor): Extension<Flavor>,
    Path(name): Path<String>,
    Json(payload): Json<RenameViewRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let new_name = payload.new_name;
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        let old_name = name.clone();
        let new_name = new_name.clone();
        move || -> Result<serde_json::Value, String> {
            flavor.validate_name(&new_name)?;
            let query = state
                .queries
                .get_catalog_query(&flavor.catalog_query("GET"))
                .ok_or(edms::error::EdmsError::UnknownError)
                .map_err(|e| format!("{e:?}"))?;
            let catalog = open_catalog(&state)?;
            let row: Option<(String, Option<String>, String, Option<String>, Option<String>)> =
                catalog
                    .core
                    .cproc(query, &[&old_name], |r| {
                        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
                    })
                    .map_err(|e| format!("{e:?}"))?
                    .into_iter()
                    .next();

            let Some((_, _, _, annotation, source)) = row else {
                return Err(format!("{} \'{old_name}\' does not exist", flavor.label()));
            };

            if catalog
                .get(flavor.kind(), &new_name)
                .map_err(|e| format!("{e:?}"))?
                .is_some()
            {
                return Err(format!("{} \'{new_name}\' already exists", flavor.label()));
            }

            let old_dir = flavor.dir(&state, &old_name);
            let new_dir = flavor.dir(&state, &new_name);
            if !old_dir.is_dir() {
                return Err(format!("{} \'{old_name}\' has no folder yet", flavor.label()));
            }
            copy_dir_recursive(&old_dir, &new_dir).map_err(|e| e.to_string())?;

            let new_file_path = flavor.file_path(&state, &new_name);
            let create_query = state
                .queries
                .get_catalog_query(&flavor.catalog_query("CREATE"))
                .ok_or(edms::error::EdmsError::UnknownError)
                .map_err(|e| format!("{e:?}"))?;
            if let Err(e) = catalog
                .core
                .proc(create_query, &[&new_name, &new_file_path, &annotation, &source])
            {
                // Roll back the directory copy so a failed catalog insert
                // doesn't leave an orphaned duplicate folder behind.
                let _ = std::fs::remove_dir_all(&new_dir);
                return Err(format!("{e:?}"));
            }

            // Copy row-level membership-tags across too — same table used
            // by add_repoview_tag/list_repoview_tags_for_name.
            let (tag_table, name_col) = flavor.tag_table();
            let _ = catalog.core.proc(
                &format!(
                    "INSERT OR IGNORE INTO {tag_table} ({name_col}, tagname)                      SELECT ?, tagname FROM {tag_table} WHERE {name_col} = ?"
                ),
                &[&new_name, &old_name],
            );

            Ok(json!({
                "ok": true,
                "name": new_name,
                "file_path": new_file_path,
                "annotation": annotation,
                "source": source
            }))
        }
    })
    .await;

    match res {
        Ok(Ok(body)) => {
            state.refresh_dashboard_snapshot();
            (StatusCode::OK, Json(body))
        }
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

// ── Tag → Collection transfer ("Add to Collection") ─────────────────────
//
// The central `tags` table (TagOps) stays the single source of truth for
// an endpoint's own tags, untouched by this — it's read-only here. This
// only writes to the destination collection's own file: adds the matched
// endpoints as members, and — if requested — copies each one's current
// tags into that collection's endpoint_tags table (a separate, per-
// collection record, not a move out of the central table).

const MAX_TAGS_PER_ENDPOINT: i64 = 25;

#[derive(Debug, Deserialize)]
pub struct ImportTagsRequest {
    pub tags: Vec<String>,
    #[serde(default)]
    pub export_existing_tags: bool,
}

/// POST /collections/:name/tags/import
pub async fn import_tags_into_collection(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(payload): Json<ImportTagsRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        let name = name.clone();
        move || -> Result<serde_json::Value, String> {
            let membership = open_existing_collection_membership(&state, &name)?;

            let tag_ops = TagOps::new(&state.db_path.display().to_string());
            tag_ops.initialize().map_err(|e| format!("{e:?}"))?;

            let mut endpoint_ids: HashSet<String> = HashSet::new();
            for tag in &payload.tags {
                let ids = tag_ops.get_endpoints_by_tag(tag).map_err(|e| format!("{e:?}"))?;
                endpoint_ids.extend(ids);
            }
            let endpoint_ids: Vec<String> = endpoint_ids.into_iter().collect();

            let added_members = membership.add_batch(&endpoint_ids).map_err(|e| format!("{e:?}"))?;

            let mut tags_exported = 0usize;
            let mut tags_skipped_cap = 0usize;
            if payload.export_existing_tags {
                for eid in &endpoint_ids {
                    let existing_tags = tag_ops.get_by_endpoint(eid).map_err(|e| format!("{e:?}"))?;
                    let mut current_count = membership.count_tags_for_endpoint(eid).map_err(|e| format!("{e:?}"))?;
                    for tag in existing_tags {
                        if current_count >= MAX_TAGS_PER_ENDPOINT {
                            tags_skipped_cap += 1;
                            continue;
                        }
                        let inserted = membership.add_tag(eid, &tag).map_err(|e| format!("{e:?}"))?;
                        if inserted > 0 {
                            tags_exported += 1;
                            current_count += 1;
                        }
                    }
                }
            }

            Ok(json!({
                "ok": true,
                "endpoints_matched": endpoint_ids.len(),
                "endpoints_added": added_members,
                "tags_exported": tags_exported,
                "tags_skipped_cap": tags_skipped_cap
            }))
        }
    })
    .await;

    match res {
        Ok(Ok(body)) => (StatusCode::OK, Json(body)),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

/// GET /collections/:name/tags/endpoints — list every (endpoint_id, tag)
/// pair this collection carries (from the import route above — separate
/// from the central tags table).
pub async fn list_collection_endpoint_tags(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        let name = name.clone();
        move || -> Result<Vec<(String, String)>, String> {
            let membership = open_existing_collection_membership(&state, &name)?;
            membership.list_all_endpoint_tags().map_err(|e| format!("{e:?}"))
        }
    })
    .await;

    match res {
        Ok(Ok(pairs)) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "tags": pairs.into_iter().map(|(endpoint_id, tag)| json!({
                    "endpoint_id": endpoint_id, "tag": tag
                })).collect::<Vec<_>>()
            })),
        ),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::validate_repoview_name;

    #[test]
    fn accepts_ordinary_names() {
        for ok in ["repo-one", "product catalog", "v2.0-final", "Café", "a"] {
            assert!(validate_repoview_name(ok).is_ok(), "{ok} should be accepted");
        }
    }

    #[test]
    fn rejects_names_that_would_be_shadowed_by_a_fixed_route() {
        for word in ["list", "create", "delete", "import", "combine", "tags", "by-tag", "LIST", "Import"] {
            assert!(validate_repoview_name(word).is_err(), "{word} is a route word");
        }
        // only whole words are reserved
        assert!(validate_repoview_name("my-list").is_ok());
        assert!(validate_repoview_name("imports").is_ok());
    }

    #[test]
    fn rejects_names_that_could_escape_or_break_the_folder() {
        for bad in [
            "", "   ", ".", "..", "../x", "a/b", "a\\b", "C:evil", "a*b", "a?b", "a\"b", "a<b", "a>b",
            "a|b", " leading", "trailing ", "dot.", "tab\there", "nul\0byte",
        ] {
            assert!(validate_repoview_name(bad).is_err(), "{bad:?} should be rejected");
        }
        assert!(validate_repoview_name(&"x".repeat(101)).is_err());
        assert!(validate_repoview_name(&"x".repeat(100)).is_ok());
    }
}
