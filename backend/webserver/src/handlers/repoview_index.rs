//! A RepoView's (and, the same way, a WebView's) own SQLite index - v1.0
//! (Ravi, 2026-10-05/06). The route handlers here serve both: `Flavor` says
//! which.
//!
//! A RepoView is a list, not a copy of the data: `repoview.sqlite` holds the
//! members, a snapshot of each member's endpoint row (URL, method,
//! annotation, size of its EQP data), its tags and its QP metadata. That is
//! everything the list view shows - counts, methods, tags, endpoint segments
//! - and everything Import/Export needs to know what to copy later. No EQP
//! data is stored here; it's copied from globalEQPData only at takeout.
//!
//! Also here: the routes that read that index back (`list`, `get`) and
//! "Convert to Collection", the RepoView's escape hatch into a Collection.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use edms::ops::collection_membership_ops::CollectionMembershipOps;
use edms::ops::repoview_tag_ops::RepoviewTagMembershipOps;
use edms::ops::tag_ops::TagOps;
use edms::ops::view_ops::ViewKind;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path as FsPath;

use crate::{
    db,
    handlers::{
        repoview_tables::{is_table_file_name, path_segments},
        view_flavor::Flavor,
        view_catalog::{
            collection_file_path, open_catalog, open_existing_membership, open_membership,
            validate_folder_name,
        },
    },
    state::AppState,
};

// ── The index tables ─────────────────────────────────────────────────────

const SNAPSHOT_DDL: [&str; 2] = [
    "CREATE TABLE IF NOT EXISTS endpoint_snapshot (
        endpoint_id      TEXT PRIMARY KEY,
        endpoint_str     TEXT NOT NULL,
        method           TEXT,
        annotation       TEXT,
        data_size_bytes  INTEGER
    )",
    "CREATE TABLE IF NOT EXISTS qp_snapshot (
        endpoint_id      TEXT NOT NULL,
        request_number   INTEGER NOT NULL,
        method           TEXT,
        status_code      INTEGER,
        response_time_ms INTEGER,
        PRIMARY KEY (endpoint_id, request_number)
    )",
];

/// Idempotent. Also upgrades an index created before `data_size_bytes`
/// existed (the 2026-10-04 snapshot tables) by adding the column.
pub(crate) fn init_snapshot_tables(membership: &CollectionMembershipOps) -> Result<(), String> {
    for ddl in SNAPSHOT_DDL {
        membership.core.proc(ddl, &[]).map_err(|e| format!("{e:?}"))?;
    }
    let has_size = scalar(
        membership,
        "SELECT COUNT(*) FROM pragma_table_info('endpoint_snapshot') WHERE name = 'data_size_bytes'",
    )?;
    if has_size == 0 {
        membership
            .core
            .proc("ALTER TABLE endpoint_snapshot ADD COLUMN data_size_bytes INTEGER", &[])
            .map_err(|e| format!("{e:?}"))?;
    }
    Ok(())
}

pub(crate) fn scalar(membership: &CollectionMembershipOps, sql: &str) -> Result<i64, String> {
    let rows: Vec<i64> = membership
        .core
        .cproc(sql, &[], |r| r.get::<_, i64>(0))
        .map_err(|e| format!("{e:?}"))?;
    Ok(rows.first().copied().unwrap_or(0))
}

#[derive(Debug, Default)]
pub(crate) struct IndexStats {
    pub added: usize,
    pub tags_copied: usize,
    pub qps_snapshotted: usize,
    pub data_size_bytes: u64,
}

/// Lists `endpoint_ids` in this RepoView's index: membership, a copy of each
/// endpoint's current central tags, a snapshot of its endpoint row and QP
/// metadata, and the size of its EQP data (measured, not copied). Nothing is
/// written to disk beyond the SQLite - Create stays instant.
pub(crate) fn build_index(
    state: &AppState,
    membership: &CollectionMembershipOps,
    endpoint_ids: &[String],
) -> Result<IndexStats, String> {
    init_snapshot_tables(membership)?;

    let mut stats = IndexStats::default();
    stats.added = membership.add_batch(endpoint_ids).map_err(|e| format!("{e:?}"))?;

    let tag_ops = TagOps::new(&state.db_path.display().to_string());
    tag_ops.initialize().map_err(|e| format!("{e:?}"))?;

    for eid in endpoint_ids {
        for tag in tag_ops.get_by_endpoint(eid).map_err(|e| format!("{e:?}"))? {
            if membership.add_tag(eid, &tag).map_err(|e| format!("{e:?}"))? > 0 {
                stats.tags_copied += 1;
            }
        }

        // 0 for an endpoint with no saved QPs yet (no folder on disk).
        let size = compute::table_view::compute_size(&state.endpoint_storage_dir(eid));
        stats.data_size_bytes += size;

        if let Some(ep) = db::get_endpoint(&state.core, &state.queries, eid).map_err(|e| format!("{e:?}"))? {
            membership
                .core
                .proc(
                    "INSERT OR REPLACE INTO endpoint_snapshot (endpoint_id, endpoint_str, method, annotation, data_size_bytes) VALUES (?, ?, ?, ?, ?)",
                    &[&ep.endpoint_id, &ep.endpoint_str, &ep.method, &ep.annotation, &(size as i64)],
                )
                .map_err(|e| format!("{e:?}"))?;
        }

        for qp in db::list_qps_for_endpoint(&state.core, &state.queries, eid).map_err(|e| format!("{e:?}"))? {
            membership
                .core
                .proc(
                    "INSERT OR REPLACE INTO qp_snapshot (endpoint_id, request_number, method, status_code, response_time_ms) VALUES (?, ?, ?, ?, ?)",
                    &[&eid, &qp.request_number, &qp.method, &qp.status_code, &qp.response_time_ms],
                )
                .map_err(|e| format!("{e:?}"))?;
            stats.qps_snapshotted += 1;
        }
    }

    Ok(stats)
}

/// `(endpoint_id, endpoint_str, method)` for every listed member, straight
/// from the index - this is what the Tables-*.md are generated from, so a
/// RepoView never needs the central tables to describe itself.
pub(crate) fn snapshot_endpoints(
    membership: &CollectionMembershipOps,
) -> Result<Vec<(String, String, String)>, String> {
    init_snapshot_tables(membership)?;
    membership
        .core
        .cproc(
            "SELECT s.endpoint_id, s.endpoint_str, COALESCE(s.method, 'UNCLASSIFIED')
             FROM endpoint_snapshot s JOIN membership m ON m.endpoint_id = s.endpoint_id
             ORDER BY s.endpoint_id",
            &[],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| format!("{e:?}"))
}

// ── Stats: everything the list row shows, computed from the index ────────

#[derive(Debug, Default)]
pub(crate) struct RepoviewStats {
    pub eid_count: usize,
    pub qp_count: usize,
    pub data_size_bytes: u64,
    pub crud: BTreeMap<String, usize>,
    pub data_tags: Vec<String>,
    /// The distinct path pieces, sorted (the keys of `segment_frequency`).
    pub segments: Vec<String>,
    /// Each path piece and how many times it occurs across the list's
    /// endpoints' URLs (Ravi, 2026-10-07: `/a/b/c` and `/z/a/c` give
    /// a:2, b:1, c:2, z:1). Every occurrence counts, so `/a/a` is two.
    pub segment_frequency: BTreeMap<String, usize>,
    pub index_lists: usize,
}

/// Reads only the RepoView's own SQLite and folder - never the central
/// tables. `dir` is only used to count the generated Tables-NNN.md files (and
/// as a size fallback for an index that predates per-endpoint sizes).
pub(crate) fn compute_stats(membership: &CollectionMembershipOps, dir: &FsPath) -> Result<RepoviewStats, String> {
    init_snapshot_tables(membership)?;

    let eid_count = scalar(membership, "SELECT COUNT(*) FROM membership")? as usize;
    let qp_count = scalar(
        membership,
        "SELECT COUNT(*) FROM qp_snapshot WHERE endpoint_id IN (SELECT endpoint_id FROM membership)",
    )? as usize;

    let rows: Vec<(String, Option<String>, Option<i64>)> = membership
        .core
        .cproc(
            "SELECT s.endpoint_str, s.method, s.data_size_bytes
             FROM endpoint_snapshot s JOIN membership m ON m.endpoint_id = s.endpoint_id",
            &[],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| format!("{e:?}"))?;

    let mut crud: BTreeMap<String, usize> = BTreeMap::new();
    let mut segment_frequency: BTreeMap<String, usize> = BTreeMap::new();
    let mut sized = false;
    let mut data_size_bytes: u64 = 0;
    for (endpoint_str, method, size) in &rows {
        let method = method.as_deref().unwrap_or("UNCLASSIFIED").to_uppercase();
        *crud.entry(method).or_insert(0) += 1;
        for piece in path_segments(endpoint_str) {
            *segment_frequency.entry(piece).or_insert(0) += 1;
        }
        if let Some(size) = size {
            sized = true;
            data_size_bytes += (*size).max(0) as u64;
        }
    }
    if !sized && eid_count > 0 {
        data_size_bytes = compute::table_view::compute_size(dir);
    }

    let data_tags: Vec<String> = membership
        .core
        .cproc(
            "SELECT DISTINCT tag FROM endpoint_tags WHERE endpoint_id IN (SELECT endpoint_id FROM membership) ORDER BY tag",
            &[],
            |r| r.get(0),
        )
        .map_err(|e| format!("{e:?}"))?;

    let mut index_lists = 0usize;
    if dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if is_table_file_name(&file_name) && file_name != "Tables-meta.md" {
                    index_lists += 1;
                }
            }
        }
    }

    Ok(RepoviewStats {
        eid_count,
        qp_count,
        data_size_bytes,
        crud,
        data_tags,
        segments: segment_frequency.keys().cloned().collect(),
        segment_frequency,
        index_lists,
    })
}

// ── One RepoView as the API shows it ─────────────────────────────────────

struct RepoviewRow {
    name: String,
    file_path: Option<String>,
    created_at: String,
    annotation: Option<String>,
    source: Option<String>,
    tags: Vec<String>,
    stats: RepoviewStats,
    /// Set when this one RepoView's index couldn't be read, so one broken
    /// folder shows up as a flagged row instead of failing the whole list.
    error: Option<String>,
}

impl RepoviewRow {
    /// The list-view row: the fields the UI table already reads
    /// (`repoview.js`: dataTags, crud, segments, segmentCount, eidCount,
    /// qpCount, indexLists, dateCreated, dataSizeBytes, ...) plus the
    /// snake_case fields `/repoview/list` returned before, so nothing that
    /// read it breaks.
    fn list_json(&self) -> Value {
        json!({
            "id": self.name,
            "name": self.name,
            "file_path": self.file_path,
            "created_at": self.created_at,
            "annotation": self.annotation,
            "source": self.source,
            "tags": self.tags,
            "dataTags": self.stats.data_tags,
            "crud": self.stats.crud,
            "segments": self.stats.segments,
            "segmentCount": self.stats.segments.len(),
            "segmentFrequency": self.stats.segment_frequency,
            "eidCount": self.stats.eid_count,
            "qpCount": self.stats.qp_count,
            "indexLists": self.stats.index_lists,
            "dateCreated": self.created_at,
            "dataSizeBytes": self.stats.data_size_bytes,
            "error": self.error,
        })
    }

    /// `GET /repoview/:name` - same data, in the snake_case this route has
    /// always used.
    fn detail_json(&self) -> Value {
        json!({
            "ok": true,
            "name": self.name,
            "file_path": self.file_path,
            "created_at": self.created_at,
            "annotation": self.annotation,
            "source": self.source,
            "tags": self.tags,
            "eid_count": self.stats.eid_count,
            "qp_count": self.stats.qp_count,
            "data_size_bytes": self.stats.data_size_bytes,
            "tags_in_data": self.stats.data_tags,
            "crud_types": self.stats.crud,
            "segments": self.stats.segments,
            "segment_count": self.stats.segments.len(),
            "segment_frequency": self.stats.segment_frequency,
            "index_lists": self.stats.index_lists,
            "error": self.error,
        })
    }
}

type CatalogRow = (String, Option<String>, String, Option<String>, Option<String>);

fn load_row(flavor: Flavor, state: &AppState, tag_ops: &RepoviewTagMembershipOps, row: CatalogRow) -> RepoviewRow {
    let (name, file_path, created_at, annotation, source) = row;
    let tags = tag_ops.list(&name).unwrap_or_default();

    let (stats, error) = match &file_path {
        None => (RepoviewStats::default(), None),
        Some(path) => match open_membership(path)
            .and_then(|m| compute_stats(&m, &flavor.dir(state, &name)))
        {
            Ok(stats) => (stats, None),
            Err(e) => (RepoviewStats::default(), Some(e)),
        },
    };

    RepoviewRow { name, file_path, created_at, annotation, source, tags, stats, error }
}

pub(crate) fn catalog_rows(state: &AppState, key: &str, params: &[&dyn rusqlite::ToSql]) -> Result<Vec<CatalogRow>, String> {
    let query = state
        .queries
        .get_catalog_query(key)
        .ok_or_else(|| format!("catalog query {key} is missing"))?;
    open_catalog(state)?
        .core
        .cproc(query, params, |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
        .map_err(|e| format!("{e:?}"))
}

/// GET /{repoview,webview}/list - every view with all the columns the UI table
/// shows, in one call (no per-row follow-up requests).
pub async fn list_repoviews(State(state): State<AppState>, Extension(flavor): Extension<Flavor>) -> (StatusCode, Json<Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        move || -> Result<Vec<Value>, String> {
            let rows = catalog_rows(&state, &flavor.catalog_query("LIST"), &[])?;
            let tags = flavor.row_tag_ops(&state)?;
            Ok(rows.into_iter().map(|row| load_row(flavor, &state, &tags, row).list_json()).collect())
        }
    })
    .await;

    match res {
        Ok(Ok(items)) => (StatusCode::OK, Json(json!({ "ok": true, "items": items }))),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

/// GET /repoview/:name - one RepoView, same data as a list row.
pub async fn get_repoview_entry(
    State(state): State<AppState>,
    Extension(flavor): Extension<Flavor>,
    Path(name): Path<String>,
) -> (StatusCode, Json<Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        let name = name.clone();
        move || -> Result<Option<Value>, String> {
            let rows = catalog_rows(&state, &flavor.catalog_query("GET"), &[&name])?;
            let tags = flavor.row_tag_ops(&state)?;
            Ok(rows.into_iter().next().map(|row| load_row(flavor, &state, &tags, row).detail_json()))
        }
    })
    .await;

    match res {
        Ok(Ok(Some(body))) => (StatusCode::OK, Json(body)),
        Ok(Ok(None)) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "ok": false, "error": flavor.not_found(&name) })),
        ),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

// ── Convert to Collection ────────────────────────────────────────────────
//
// The RMB action from the v1.0 notes: a pop-up for adding a new Collection,
// or merging into an existing one - with a warning when the name already
// exists, asking the user to merge or rename. This replaces the earlier
// export-to-collection, which rebuilt lost endpoints from copied data; a
// RepoView no longer holds any data to rebuild from, so converting only
// lists endpoints that still exist in the central tables.

#[derive(Debug, Deserialize)]
pub struct ConvertRequest {
    pub collection: String,
    /// Only needed when `collection` already exists: `"merge"` or `"rename"`.
    #[serde(default)]
    pub on_exists: Option<String>,
    /// With `on_exists: "rename"`: the new Collection's name.
    #[serde(default)]
    pub new_name: Option<String>,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Decision {
    Create(String),
    Merge(String),
    Conflict,
    Invalid(String),
}

/// Pure decision logic for the pop-up's choices, kept separate so it's
/// testable without a server.
pub(crate) fn decide(collection: &str, exists: bool, on_exists: Option<&str>, new_name: Option<&str>) -> Decision {
    if !exists {
        return Decision::Create(collection.to_string());
    }
    match on_exists.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        None | Some("") => Decision::Conflict,
        Some("merge") => Decision::Merge(collection.to_string()),
        Some("rename") => match new_name.map(str::trim).filter(|n| !n.is_empty()) {
            Some(n) if n != collection => Decision::Create(n.to_string()),
            Some(_) => Decision::Invalid("new_name must be different from the existing name".to_string()),
            None => Decision::Invalid("on_exists 'rename' needs a new_name".to_string()),
        },
        Some(other) => Decision::Invalid(format!("unknown on_exists '{other}' - expected 'merge' or 'rename'")),
    }
}

enum ConvertError {
    Conflict(String),
    Bad(String),
}

impl From<String> for ConvertError {
    fn from(e: String) -> Self {
        ConvertError::Bad(e)
    }
}

/// POST /repoview/:name/convert-to-collection
///
/// - `collection` doesn't exist: it's created and the RepoView's endpoints
///   are added.
/// - `collection` exists and no `on_exists`: **409**, `{conflict: true,
///   options: ["merge", "rename"]}` - the pop-up's warning. Nothing changes.
/// - `on_exists: "merge"`: the endpoints are added to the existing one.
/// - `on_exists: "rename"` + `new_name`: a new Collection with that name.
///
/// Only endpoints that still exist centrally (and are still the same URL +
/// method as when listed) are added; the rest are reported in `skipped`.
pub async fn convert_repoview_to_collection(
    State(state): State<AppState>,
    Extension(flavor): Extension<Flavor>,
    Path(name): Path<String>,
    Json(payload): Json<ConvertRequest>,
) -> (StatusCode, Json<Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        let name = name.clone();
        move || -> Result<Value, ConvertError> {
            let membership = open_existing_membership(&state, flavor.kind(), &name)
                .map_err(|_| ConvertError::Bad(flavor.not_found(&name)))?;
            init_snapshot_tables(&membership)?;

            validate_folder_name(&payload.collection, "Collection name")?;
            let catalog = open_catalog(&state)?;
            let exists = catalog
                .get(ViewKind::Collections, &payload.collection)
                .map_err(|e| format!("{e:?}"))?
                .is_some();

            let decision = decide(
                &payload.collection,
                exists,
                payload.on_exists.as_deref(),
                payload.new_name.as_deref(),
            );
            let (target_name, merging) = match decision {
                Decision::Conflict => {
                    return Err(ConvertError::Conflict(format!(
                        "Collection '{}' already exists",
                        payload.collection
                    )))
                }
                Decision::Invalid(why) => return Err(ConvertError::Bad(why)),
                Decision::Merge(n) => (n, true),
                Decision::Create(n) => (n, false),
            };
            if !merging {
                validate_folder_name(&target_name, "Collection name")?;
                // `rename` can still land on another existing name.
                if target_name != payload.collection
                    && catalog
                        .get(ViewKind::Collections, &target_name)
                        .map_err(|e| format!("{e:?}"))?
                        .is_some()
                {
                    return Err(ConvertError::Conflict(format!(
                        "Collection '{target_name}' already exists"
                    )));
                }
            }

            // Which listed endpoints can really become members.
            let snapshot: BTreeMap<String, (String, String)> = snapshot_endpoints(&membership)?
                .into_iter()
                .map(|(eid, url, method)| (eid, (url, method)))
                .collect();
            let mut usable: Vec<String> = Vec::new();
            let mut skipped: Vec<Value> = Vec::new();
            for eid in membership.list_ids().map_err(|e| format!("{e:?}"))? {
                match db::get_endpoint(&state.core, &state.queries, &eid).map_err(|e| format!("{e:?}"))? {
                    None => skipped.push(json!({
                        "endpoint_id": eid,
                        "reason": "no longer exists in the central tables"
                    })),
                    Some(central) => {
                        let reused = snapshot.get(&eid).map_or(false, |(url, method)| {
                            central.endpoint_str != *url
                                || central.method.as_deref().unwrap_or("GET") != method.as_str()
                        });
                        if reused {
                            skipped.push(json!({
                                "endpoint_id": eid,
                                "reason": format!(
                                    "this EID now belongs to a different endpoint ({} {})",
                                    central.method.as_deref().unwrap_or("GET"),
                                    central.endpoint_str
                                )
                            }));
                        } else {
                            usable.push(eid);
                        }
                    }
                }
            }
            if usable.is_empty() {
                return Err(ConvertError::Bad(
                    format!(
                        "none of this {}'s endpoints exist in the central tables, so there is nothing to convert",
                        flavor.label()
                    ),
                ));
            }

            let target = if merging {
                open_existing_membership(&state, ViewKind::Collections, &target_name)?
            } else {
                let path = collection_file_path(&state, &target_name);
                let target = open_membership(&path)?;
                catalog
                    .register(ViewKind::Collections, &target_name, Some(&path), None)
                    .map_err(|e| format!("{e:?}"))?;
                target
            };
            let members_added = target.add_batch(&usable).map_err(|e| format!("{e:?}"))?;

            Ok(json!({
                "ok": skipped.is_empty(),
                "collection": target_name,
                "created_collection": !merging,
                "merged": merging,
                "members_added": members_added,
                "skipped": skipped
            }))
        }
    })
    .await;

    match res {
        Ok(Ok(body)) => {
            state.refresh_dashboard_snapshot();
            (StatusCode::OK, Json(body))
        }
        Ok(Err(ConvertError::Conflict(msg))) => (
            StatusCode::CONFLICT,
            Json(json!({
                "ok": false,
                "conflict": true,
                "error": msg,
                "options": ["merge", "rename"]
            })),
        ),
        Ok(Err(ConvertError::Bad(e))) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("edms-index-{label}-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn open(dir: &FsPath) -> CollectionMembershipOps {
        let ops = CollectionMembershipOps::new(&dir.join("repoview.sqlite").display().to_string());
        ops.initialize().unwrap();
        init_snapshot_tables(&ops).unwrap();
        ops
    }

    fn add(m: &CollectionMembershipOps, eid: &str, url: &str, method: &str, size: Option<i64>, qps: i32, tags: &[&str]) {
        m.add(eid).unwrap();
        m.core
            .proc(
                "INSERT INTO endpoint_snapshot (endpoint_id, endpoint_str, method, annotation, data_size_bytes) VALUES (?, ?, ?, NULL, ?)",
                &[&eid, &url, &method, &size],
            )
            .unwrap();
        for n in 1..=qps {
            m.core
                .proc(
                    "INSERT INTO qp_snapshot (endpoint_id, request_number, method, status_code, response_time_ms) VALUES (?, ?, ?, 200, 5)",
                    &[&eid, &n, &method],
                )
                .unwrap();
        }
        for t in tags {
            m.add_tag(eid, t).unwrap();
        }
    }

    #[test]
    fn stats_come_entirely_from_the_index() {
        let dir = temp_dir("stats");
        let m = open(&dir);
        add(&m, "E0001-AAA", "https://x.com/users/:id/orders", "GET", Some(100), 2, &["prod", "api"]);
        add(&m, "E0002-AAA", "https://x.com/users", "POST", Some(50), 1, &["prod"]);
        add(&m, "E0003-AAA", "https://x.com/items", "get", Some(25), 0, &[]);
        std::fs::write(dir.join("Tables-001.md"), "x").unwrap();
        std::fs::write(dir.join("Tables-002.md"), "x").unwrap();
        std::fs::write(dir.join("Tables-meta.md"), "x").unwrap();
        std::fs::write(dir.join("notes.txt"), "x").unwrap();

        let s = compute_stats(&m, &dir).unwrap();

        assert_eq!(s.eid_count, 3);
        assert_eq!(s.qp_count, 3);
        assert_eq!(s.data_size_bytes, 175);
        assert_eq!(s.crud.get("GET"), Some(&2)); // "get" is folded into GET
        assert_eq!(s.crud.get("POST"), Some(&1));
        assert_eq!(s.data_tags, vec!["api", "prod"]);
        assert_eq!(s.segments, vec![":id", "items", "orders", "users"]);
        assert_eq!(s.index_lists, 2); // the two batches, not Tables-meta.md
        drop(m);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn segments_come_with_how_often_each_occurs() {
        let dir = temp_dir("segfreq");
        let m = open(&dir);
        // Ravi's example: /a/b/c and /z/a/c -> a:2, b:1, c:2, z:1
        add(&m, "E0001-AAA", "https://x.com/a/b/c", "GET", Some(1), 0, &[]);
        add(&m, "E0002-AAA", "https://x.com/z/a/c", "GET", Some(1), 0, &[]);
        let s = compute_stats(&m, &dir).unwrap();
        let expected: BTreeMap<String, usize> =
            [("a", 2), ("b", 1), ("c", 2), ("z", 1)].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        assert_eq!(s.segment_frequency, expected);
        assert_eq!(s.segments, vec!["a", "b", "c", "z"], "the old fields keep their meaning");

        // every occurrence counts, even twice in one URL; a query string or fragment is not a segment
        add(&m, "E0003-AAA", "https://x.com/a/a?x=1", "GET", Some(1), 0, &[]);
        let s = compute_stats(&m, &dir).unwrap();
        assert_eq!(s.segment_frequency["a"], 4);

        // 3 + 3 + 2 path pieces in all
        assert_eq!(s.segment_frequency.values().sum::<usize>(), 8);
        drop(m);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn both_row_shapes_carry_the_frequency_map() {
        let mut frequency = BTreeMap::new();
        frequency.insert("users".to_string(), 3usize);
        frequency.insert(":id".to_string(), 1usize);
        let row = RepoviewRow {
            name: "v".into(),
            file_path: None,
            created_at: "2026-10-07 00:00:00".into(),
            annotation: None,
            source: None,
            tags: vec![],
            stats: RepoviewStats {
                segments: vec![":id".into(), "users".into()],
                segment_frequency: frequency,
                ..Default::default()
            },
            error: None,
        };
        let list = row.list_json();
        assert_eq!(list["segmentFrequency"], json!({ ":id": 1, "users": 3 }));
        assert_eq!(list["segmentCount"], 2);
        assert_eq!(list["segments"], json!([":id", "users"]));
        let detail = row.detail_json();
        assert_eq!(detail["segment_frequency"], json!({ ":id": 1, "users": 3 }));
        assert_eq!(detail["segment_count"], 2);
    }

    #[test]
    fn an_empty_repoview_has_zero_everything() {
        let dir = temp_dir("empty");
        let m = open(&dir);
        let s = compute_stats(&m, &dir).unwrap();
        assert_eq!((s.eid_count, s.qp_count, s.data_size_bytes, s.index_lists), (0, 0, 0, 0));
        assert!(s.segments.is_empty() && s.segment_frequency.is_empty() && s.crud.is_empty() && s.data_tags.is_empty());
        drop(m);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn size_falls_back_to_the_folder_for_an_index_without_per_endpoint_sizes() {
        let dir = temp_dir("fallback");
        let m = open(&dir);
        add(&m, "E0001-AAA", "https://x.com/a", "GET", None, 0, &[]);
        std::fs::write(dir.join("legacy.bin"), vec![0u8; 1000]).unwrap();
        let s = compute_stats(&m, &dir).unwrap();
        assert!(s.data_size_bytes >= 1000);
        drop(m);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_older_index_gains_the_size_column_and_keeps_its_rows() {
        let dir = temp_dir("migrate");
        let ops = CollectionMembershipOps::new(&dir.join("repoview.sqlite").display().to_string());
        ops.initialize().unwrap();
        ops.core
            .proc(
                "CREATE TABLE endpoint_snapshot (endpoint_id TEXT PRIMARY KEY, endpoint_str TEXT NOT NULL, method TEXT, annotation TEXT)",
                &[],
            )
            .unwrap();
        ops.core
            .proc("INSERT INTO endpoint_snapshot VALUES ('E0001-AAA', 'https://x.com/a', 'GET', NULL)", &[])
            .unwrap();

        init_snapshot_tables(&ops).unwrap();
        init_snapshot_tables(&ops).unwrap(); // idempotent

        assert_eq!(scalar(&ops, "SELECT COUNT(*) FROM endpoint_snapshot").unwrap(), 1);
        assert_eq!(
            scalar(&ops, "SELECT COUNT(*) FROM pragma_table_info('endpoint_snapshot') WHERE name = 'data_size_bytes'").unwrap(),
            1
        );
        drop(ops);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn snapshot_endpoints_lists_only_current_members() {
        let dir = temp_dir("members");
        let m = open(&dir);
        add(&m, "E0002-AAA", "https://x.com/b", "POST", None, 0, &[]);
        add(&m, "E0001-AAA", "https://x.com/a", "GET", None, 0, &[]);
        m.core
            .proc(
                "INSERT INTO endpoint_snapshot (endpoint_id, endpoint_str, method, annotation, data_size_bytes) VALUES ('E0099-AAA', 'https://x.com/ghost', 'GET', NULL, 0)",
                &[],
            )
            .unwrap();
        let rows = snapshot_endpoints(&m).unwrap();
        assert_eq!(
            rows,
            vec![
                ("E0001-AAA".to_string(), "https://x.com/a".to_string(), "GET".to_string()),
                ("E0002-AAA".to_string(), "https://x.com/b".to_string(), "POST".to_string()),
            ]
        );
        drop(m);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn convert_decisions_follow_the_popup_choices() {
        assert_eq!(decide("c", false, None, None), Decision::Create("c".into()));
        assert_eq!(decide("c", false, Some("merge"), None), Decision::Create("c".into()));
        assert_eq!(decide("c", true, None, None), Decision::Conflict);
        assert_eq!(decide("c", true, Some(""), None), Decision::Conflict);
        assert_eq!(decide("c", true, Some("MERGE"), None), Decision::Merge("c".into()));
        assert_eq!(decide("c", true, Some("rename"), Some(" d ")), Decision::Create("d".into()));
        assert!(matches!(decide("c", true, Some("rename"), None), Decision::Invalid(_)));
        assert!(matches!(decide("c", true, Some("rename"), Some("c")), Decision::Invalid(_)));
        assert!(matches!(decide("c", true, Some("rename"), Some("  ")), Decision::Invalid(_)));
        assert!(matches!(decide("c", true, Some("overwrite"), None), Decision::Invalid(_)));
    }
}
