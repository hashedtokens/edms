// DB helpers shared by all tag operations.
// SQL keys reference webserver/libs/edms/src/queries.yaml.

use edms::{EdmsCore, query_loader::QueryMap};
use crate::tagops::types::ActivityEntry;
use std::path::Path;

pub type DynResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Open a WAL-mode EdmsCore connection and load the embedded QueryMap.
pub fn open_core(db_path: &str) -> DynResult<(EdmsCore, QueryMap)> {
    let core = EdmsCore::new(db_path);
    core.connect().map_err(|e| format!("db connect: {e:?}"))?;
    Ok((core, QueryMap::load()))
}

/// Resolves the storage root from `db_path`.
pub fn get_storage_root(db_path: &str) -> std::path::PathBuf {
    let p = std::path::Path::new(db_path);
    if let Some(parent) = p.parent() {
        if parent.as_os_str().is_empty() {
            std::path::PathBuf::from(".")
        } else {
            parent.to_path_buf()
        }
    } else {
        std::path::PathBuf::from(".")
    }
}

/// Resolves the path to a collection's dedicated SQLite file, if available.
/// The storage root is taken to be the folder `db_path` sits in - see
/// `resolve_collection_sqlite_path_in` to say where it really is.
pub fn resolve_collection_sqlite_path(core: &EdmsCore, db_path: &str, collection_name: &str) -> Option<std::path::PathBuf> {
    resolve_collection_sqlite_path_in(core, &get_storage_root(db_path), collection_name)
}

/// Same, with the storage root given. (The database file doesn't have to sit
/// in the storage root - in Docker it is `/app/data/edms.db` while the storage
/// is `/app/edms_root` - so a caller that knows the root should say so.)
pub fn resolve_collection_sqlite_path_in(core: &EdmsCore, root: &Path, collection_name: &str) -> Option<std::path::PathBuf> {
    // 1. Try catalog table `collections`
    if let Ok(rows) = core.cproc(
        "SELECT file_path FROM collections WHERE name = ?",
        &[&collection_name],
        |row| row.get::<_, Option<String>>(0),
    ) {
        if let Some(Some(path_str)) = rows.into_iter().next() {
            let p = std::path::PathBuf::from(path_str);
            if p.exists() {
                return Some(p);
            }
        }
    }

    // 2. Try default location: storage/collections/{collection_name}.sqlite
    let default_p = root.join("storage").join("collections").join(format!("{collection_name}.sqlite"));
    if default_p.exists() {
        return Some(default_p);
    }

    None
}

/// Reads all member EIDs for a collection, checking its dedicated SQLite file first,
/// and falling back to the `bookmarks` table.
pub fn read_collection_member_eids(core: &EdmsCore, db_path: &str, collection_name: &str) -> DynResult<Vec<String>> {
    read_collection_member_eids_in(core, &get_storage_root(db_path), collection_name)
}

/// Same, with the storage root given.
pub fn read_collection_member_eids_in(core: &EdmsCore, root: &Path, collection_name: &str) -> DynResult<Vec<String>> {
    if let Some(col_path) = resolve_collection_sqlite_path_in(core, root, collection_name) {
        let membership = edms::ops::collection_membership_ops::CollectionMembershipOps::new(col_path.to_str().unwrap());
        if membership.initialize().is_ok() {
            if let Ok(ids) = membership.list_ids() {
                if !ids.is_empty() {
                    return Ok(ids);
                }
            }
        }
    }

    // Fallback to bookmarks table
    let rows: Vec<String> = core.cproc(
        "SELECT endpoint_id FROM bookmarks WHERE folder = ? ORDER BY timestamp DESC",
        &[&collection_name],
        |row| row.get(0),
    )?;
    Ok(rows)
}

/// Writes member EIDs into the collection's dedicated SQLite file and the central `bookmarks` table.
pub fn write_collection_member_eids(
    core: &EdmsCore,
    db_path: &str,
    collection_name: &str,
    eids: &[String],
) -> DynResult<()> {
    write_collection_member_eids_in(core, &get_storage_root(db_path), collection_name, eids)
}

/// Same, with the storage root given.
pub fn write_collection_member_eids_in(
    core: &EdmsCore,
    root: &Path,
    collection_name: &str,
    eids: &[String],
) -> DynResult<()> {
    if eids.is_empty() {
        return Ok(());
    }

    // 1. Update the collection's own SQLite file: the one the catalog already
    // names if there is one (a collection the app created), else the default
    // `storage/collections/{name}.sqlite` (created now).
    let col_file = match resolve_collection_sqlite_path_in(core, root, collection_name) {
        Some(existing) => existing,
        None => {
            let col_dir = root.join("storage").join("collections");
            std::fs::create_dir_all(&col_dir)
                .map_err(|e| format!("can't create {}: {e}", col_dir.display()))?;
            col_dir.join(format!("{collection_name}.sqlite"))
        }
    };
    let col_file_str = col_file
        .to_str()
        .ok_or_else(|| format!("collection path {} isn't valid UTF-8", col_file.display()))?
        .to_string();
    // This is what the app lists the collection's endpoints from, so a failure
    // here must fail the merge - it must not look like it worked.
    let membership = edms::ops::collection_membership_ops::CollectionMembershipOps::new(&col_file_str);
    membership
        .initialize()
        .map_err(|e| format!("can't open collection file {col_file_str}: {e:?}"))?;
    membership
        .add_batch(eids)
        .map_err(|e| format!("can't add members to {col_file_str}: {e:?}"))?;

    // 2. Update catalog row if table `collections` exists.
    // Uses INSERT OR IGNORE so an already-registered collection is a no-op.
    // Failure (e.g. schema not yet initialised) is non-fatal because
    // resolve_collection_sqlite_path falls back to the default disk path,
    // but we warn so the root cause is visible in diagnostics.
    let col_path_str = col_file_str.clone();
    if let Err(e) = core.proc(
        "INSERT OR IGNORE INTO collections (name, file_path) VALUES (?, ?)",
        &[&collection_name, &col_path_str],
    ) {
        tracing::warn!(
            collection = collection_name,
            path = %col_path_str,
            error = %e,
            "Failed to register collection in catalog; path resolution will fall back to disk"
        );
    }

    // 3. Update bookmarks table for backward compatibility
    core.base.execute("BEGIN IMMEDIATE", &[])?;
    let res = (|| -> DynResult<()> {
        for chunk in eids.chunks(500) {
            for eid in chunk {
                core.proc(
                    "INSERT OR IGNORE INTO bookmarks (endpoint_id, folder, notes) VALUES (?, ?, NULL)",
                    &[eid, &collection_name],
                )?;
            }
        }
        Ok(())
    })();

    match res {
        Ok(()) => {
            core.base.execute("COMMIT", &[])?;
            Ok(())
        }
        Err(e) => {
            let _ = core.base.execute("ROLLBACK", &[]);
            Err(e)
        }
    }
}

/// Distinct endpoint_ids in `collection_id` that carry any tag from `tags`.
/// When `tags` is empty, returns all endpoints in the collection.
/// When non-empty, filters the collection members by checking the `tags` table.
pub(super) fn endpoints_with_tags_in_collection(
    core:          &EdmsCore,
    _queries:      &QueryMap,
    collection_id: &str,
    tags:          &[String],
) -> DynResult<Vec<String>> {
    endpoints_with_tags_in_collection_in(core, &get_storage_root(&core.base.db_path), collection_id, tags)
}

/// Same, with the storage root given.
pub(super) fn endpoints_with_tags_in_collection_in(
    core:          &EdmsCore,
    root:          &Path,
    collection_id: &str,
    tags:          &[String],
) -> DynResult<Vec<String>> {
    let member_eids = read_collection_member_eids_in(core, root, collection_id)?;
    if member_eids.is_empty() || tags.is_empty() {
        return Ok(member_eids);
    }

    // Filter member_eids by tags in batches of 500
    let mut filtered = Vec::new();
    let tag_placeholders = tags.iter().map(|_| "?").collect::<Vec<_>>().join(", ");

    for chunk in member_eids.chunks(500) {
        let eid_placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!(
            "SELECT DISTINCT endpoint_id FROM tags WHERE endpoint_id IN ({eid_placeholders}) AND tag IN ({tag_placeholders})"
        );

        let mut params: Vec<&dyn rusqlite::ToSql> = Vec::new();
        for eid in chunk {
            params.push(eid);
        }
        for tag in tags {
            params.push(tag);
        }

        let matching: Vec<String> = core.cproc(&sql, params.as_slice(), |row| row.get(0))?;
        filtered.extend(matching);
    }

    Ok(filtered)
}

/// Bulk fetch endpoint details: (endpoint_str, method, annotation)
pub fn bulk_fetch_endpoint_details(
    core: &EdmsCore,
    eids: &[String],
) -> DynResult<std::collections::HashMap<String, (String, String, Option<String>)>> {
    let mut map = std::collections::HashMap::new();
    if eids.is_empty() {
        return Ok(map);
    }

    for chunk in eids.chunks(500) {
        let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!(
            "SELECT endpoint_id, endpoint_str, COALESCE(method, ''), annotation FROM endpoints WHERE endpoint_id IN ({placeholders})"
        );
        let params: Vec<&dyn rusqlite::ToSql> = chunk.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

        let rows: Vec<(String, String, String, Option<String>)> = core.cproc(&sql, params.as_slice(), |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;

        for (eid, ep_str, method, annotation) in rows {
            map.insert(eid, (ep_str, method, annotation));
        }
    }

    Ok(map)
}

/// Bulk fetch tags for endpoints
pub fn bulk_fetch_endpoint_tags(
    core: &EdmsCore,
    eids: &[String],
) -> DynResult<std::collections::HashMap<String, Vec<String>>> {
    let mut map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    if eids.is_empty() {
        return Ok(map);
    }

    for chunk in eids.chunks(500) {
        let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!(
            "SELECT endpoint_id, tag FROM tags WHERE endpoint_id IN ({placeholders})"
        );
        let params: Vec<&dyn rusqlite::ToSql> = chunk.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

        let rows: Vec<(String, String)> = core.cproc(&sql, params.as_slice(), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

        for (eid, tag) in rows {
            map.entry(eid).or_default().push(tag);
        }
    }

    Ok(map)
}

/// Bulk fetch all existing endpoint logical identities in central DB: `(endpoint_str, method) -> endpoint_id`.
pub fn bulk_fetch_target_identity_map(
    core: &EdmsCore,
) -> DynResult<std::collections::HashMap<(String, String), String>> {
    let mut map = std::collections::HashMap::new();
    let rows: Vec<(String, String, String)> = core.cproc(
        "SELECT endpoint_id, endpoint_str, COALESCE(method, '') FROM endpoints",
        &[],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;

    for (eid, ep_str, method) in rows {
        map.insert((ep_str, method), eid);
    }

    Ok(map)
}

/// Insert endpoint into folder idempotently using M_INSERT_BM.
pub(super) fn insert_if_absent(
    core:        &EdmsCore,
    queries:     &QueryMap,
    endpoint_id: &str,
    folder:      &str,
) -> DynResult<()> {
    // Rely on INSERT OR IGNORE (M_INSERT_BM) instead of check-then-insert.
    let q = queries.get_merge_query("M_INSERT_BM").ok_or("missing M_INSERT_BM")?;
    core.proc(q, &[&endpoint_id, &folder])?;
    Ok(())
}

pub(super) fn log_entry(log: &mut Vec<ActivityEntry>, message: String) {
    let ts = chrono::Utc::now().to_rfc3339();
    tracing::info!("{}", message);
    log.push(ActivityEntry { timestamp: ts, message });
}

#[allow(dead_code)]
pub struct MergeClassification {
    pub source_eid: String,
    pub endpoint_str: String,
    pub method: String,
    pub annotation: Option<String>,
    pub target_eid: Option<String>,
}

#[allow(dead_code)]
pub(super) fn classify_batch(
    core: &EdmsCore,
    queries: &QueryMap,
    target_collection: &str,
    source_eids: &[String],
) -> DynResult<Vec<MergeClassification>> {
    if source_eids.is_empty() { return Ok(vec![]); }

    let placeholders = source_eids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let q = queries.get_merge_query("M_CLASSIFY").ok_or("missing M_CLASSIFY")?;
    let sql = format!("{q} ({placeholders})");

    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&target_collection];
    for eid in source_eids { params.push(eid); }

    Ok(core.cproc(&sql, params.as_slice(), |row| {
        Ok(MergeClassification {
            source_eid: row.get(0)?,
            endpoint_str: row.get(1)?,
            method: row.get(2)?,
            annotation: row.get(3)?,
            target_eid: row.get(4)?,
        })
    })?)
}

#[allow(dead_code)]
pub(super) fn union_tags(core: &EdmsCore, queries: &QueryMap, source_eid: &str, target_eid: &str) -> DynResult<usize> {
    let q = queries.get_merge_query("M_UNION_TAGS").ok_or("missing M_UNION_TAGS")?;
    Ok(core.proc(q, &[&target_eid, &source_eid])?)
}

#[allow(dead_code)]
pub(super) fn insert_endpoint(core: &EdmsCore, queries: &QueryMap, new_eid: &str, endpoint_str: &str, annotation: Option<&str>, method: &str) -> DynResult<usize> {
    let q = queries.get_merge_query("M_INSERT_EP").ok_or("missing M_INSERT_EP")?;
    Ok(core.proc(q, &[&new_eid, &endpoint_str, &annotation, &method])?)
}

#[allow(dead_code)]
pub(super) fn insert_bookmark(core: &EdmsCore, queries: &QueryMap, eid: &str, folder: &str) -> DynResult<usize> {
    let q = queries.get_merge_query("M_INSERT_BM").ok_or("missing M_INSERT_BM")?;
    Ok(core.proc(q, &[&eid, &folder])?)
}

#[allow(dead_code)]
pub(super) fn get_endpoint_details(core: &EdmsCore, queries: &QueryMap, eid: &str) -> DynResult<(String, String)> {
    let q = queries.get_endpoint_query("E2").ok_or("missing E2")?;
    let mut results = core.cproc(q, &[&eid], |row| Ok((row.get(1)?, row.get(3).unwrap_or_else(|_| "".to_string()))))?;
    results.pop().ok_or_else(|| "Endpoint not found".into())
}
