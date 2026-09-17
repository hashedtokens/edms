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
    Json,
};
use edms::ops::collection_membership_ops::CollectionMembershipOps;
use edms::ops::tag_ops::TagOps;
use edms::ops::view_ops::{ViewCatalogOps, ViewKind};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;

use crate::{db, state::AppState};

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

/// Looks up an existing collection by name in the catalog and opens its
/// membership file. Shared by every handler that operates on a specific,
/// already-created collection (add/remove/list here, and load/save/unsave
/// in bookmarks.rs) — one place that defines "does this collection exist
/// and where's its file."
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

async fn register_for(
    kind: ViewKind,
    state: AppState,
    payload: RegisterViewRequest,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        move || -> Result<usize, String> {
            let ops = open_catalog(&state)?;
            ops.register(kind, &payload.name, None, payload.annotation.as_deref())
                .map_err(|e| format!("{e:?}"))
        }
    })
    .await;

    match res {
        Ok(Ok(inserted)) => {
            state.refresh_dashboard_snapshot();
            (StatusCode::OK, Json(json!({ "ok": true, "inserted": inserted })))
        }
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

async fn list_for(kind: ViewKind, state: AppState) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        move || -> Result<Vec<(String, Option<String>, String, Option<String>)>, String> {
            let ops = open_catalog(&state)?;
            ops.list(kind).map_err(|e| format!("{e:?}"))
        }
    })
    .await;

    match res {
        Ok(Ok(rows)) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "items": rows.into_iter().map(|(name, file_path, created_at, annotation)| json!({
                    "name": name, "file_path": file_path, "created_at": created_at, "annotation": annotation
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

pub async fn create_webview_entry(
    State(state): State<AppState>,
    Json(payload): Json<RegisterViewRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    register_for(ViewKind::Webview, state, payload).await
}

pub async fn list_webviews(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    list_for(ViewKind::Webview, state).await
}

pub async fn create_repoview_entry(
    State(state): State<AppState>,
    Json(payload): Json<RegisterViewRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    register_for(ViewKind::Repoview, state, payload).await
}

pub async fn list_repoviews(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    list_for(ViewKind::Repoview, state).await
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
