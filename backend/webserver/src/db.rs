use edms::core::EdmsCore;
use edms::error::{EdmsError, EdmsResult};
use edms::query_loader::QueryMap;
use rusqlite::ToSql;
use serde::{Deserialize, Serialize};

pub const ACTIVE_FOLDER: &str = "__active__";
pub const SESSION_BACKUP_FOLDER: &str = "__session_backup__";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointDto {
    pub endpoint_id: String,
    pub endpoint_str: String,
    pub annotation: Option<String>,
    pub method: Option<String>,
}

/// True if `err` is a UNIQUE/PRIMARY KEY constraint violation — the case
/// worth turning into a clean "already exists" message instead of the raw
/// SQLite debug string.
pub fn is_unique_violation(err: &EdmsError) -> bool {
    matches!(
        err,
        EdmsError::SqliteError(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::ConstraintViolation,
                ..
            },
            _,
        ))
    )
}

/* ---------------- endpoints (queries.yaml) ---------------- */

pub fn insert_endpoint(core: &EdmsCore, queries: &QueryMap, ep: &EndpointDto) -> EdmsResult<usize> {
    let q = queries.get_endpoint_query("E1").ok_or(EdmsError::UnknownError)?;
    // E1: INSERT INTO endpoints (endpoint_id, endpoint_str, annotation, method) VALUES (?, ?, ?, ?)
    core.proc(
        q,
        &[
            &ep.endpoint_id,
            &ep.endpoint_str,
            &ep.annotation.as_deref(),
            &ep.method.as_deref(),
        ],
    )
}

pub fn list_endpoints(core: &EdmsCore, queries: &QueryMap) -> EdmsResult<Vec<EndpointDto>> {
    let q = queries.get_endpoint_query("E3").ok_or(EdmsError::UnknownError)?;
    // E3: SELECT endpoint_id, endpoint_str, annotation, method FROM endpoints
    core.cproc(q, &[], |row| {
        Ok(EndpointDto {
            endpoint_id: row.get(0)?,
            endpoint_str: row.get(1)?,
            annotation: row.get(2)?,
            method: row.get(3)?,
        })
    })
}

pub fn get_endpoint(core: &EdmsCore, queries: &QueryMap, endpoint_id: &str) -> EdmsResult<Option<EndpointDto>> {
    let q = queries.get_endpoint_query("E2").ok_or(EdmsError::UnknownError)?;
    // E2: SELECT endpoint_id, endpoint_str, annotation, method FROM endpoints WHERE endpoint_id = ?
    let rows = core.cproc(q, &[&endpoint_id], |row| {
        Ok(EndpointDto {
            endpoint_id: row.get(0)?,
            endpoint_str: row.get(1)?,
            annotation: row.get(2)?,
            method: row.get(3)?,
        })
    })?;
    Ok(rows.into_iter().next())
}

/// Looks up an endpoint by its exact (endpoint_str, method) pair — lets a
/// caller check "does this already exist" before deciding whether to pass
/// an existing endpoint_id vs. let a fresh one get allocated. Uniqueness is
/// scoped to (endpoint_str, method), matching idx_endpoints_str_method —
/// GET and POST against the same URL are legitimately different endpoints.
pub fn find_endpoint_by_str_and_method(
    core: &EdmsCore,
    queries: &QueryMap,
    endpoint_str: &str,
    method: &str,
) -> EdmsResult<Option<EndpointDto>> {
    let q = queries.get_endpoint_query("E8").ok_or(EdmsError::UnknownError)?;
    let rows = core.cproc(q, &[&endpoint_str, &method], |row| {
        Ok(EndpointDto {
            endpoint_id: row.get(0)?,
            endpoint_str: row.get(1)?,
            annotation: row.get(2)?,
            method: row.get(3)?,
        })
    })?;
    Ok(rows.into_iter().next())
}

pub fn update_annotation(core: &EdmsCore, queries: &QueryMap, endpoint_id: &str, annotation: &str) -> EdmsResult<usize> {
    let q = queries.get_endpoint_query("E4").ok_or(EdmsError::UnknownError)?;
    core.proc(q, &[&annotation, &endpoint_id])
}

pub fn delete_endpoint(core: &EdmsCore, queries: &QueryMap, endpoint_id: &str) -> EdmsResult<usize> {
    let q = queries.get_endpoint_query("E5").ok_or(EdmsError::UnknownError)?;
    core.proc(q, &[&endpoint_id])
}

/* ---------------- request/response metadata (queries.yaml) ---------------- */

pub fn get_next_request_number(core: &EdmsCore, queries: &QueryMap, endpoint_id: &str) -> EdmsResult<i32> {
    let q = queries.get_request_query("R6").ok_or(EdmsError::UnknownError)?;
    let rows: Vec<Option<i32>> = core.cproc(q, &[&endpoint_id], |row| row.get(0))?;
    match rows.first() {
        Some(Some(max)) => Ok(max + 1),
        _ => Ok(1),
    }
}

pub fn insert_request_metadata(
    core: &EdmsCore,
    queries: &QueryMap,
    endpoint_id: &str,
    request_number: i32,
    file_path: &str,
    method: &str,
) -> EdmsResult<usize> {
    let q = queries.get_request_query("R1").ok_or(EdmsError::UnknownError)?;
    core.proc(q, &[&endpoint_id, &request_number, &file_path, &method])
}

pub fn insert_response_metadata(
    core: &EdmsCore,
    queries: &QueryMap,
    endpoint_id: &str,
    request_number: i32,
    file_path: &str,
    status_code: i32,
    response_time_ms: Option<i32>,
) -> EdmsResult<usize> {
    let q = queries.get_response_query("RES1").ok_or(EdmsError::UnknownError)?;
    core.proc(q, &[&endpoint_id, &request_number, &file_path, &status_code, &response_time_ms])
}

/// A "QP" (request/response pair) — one test run's saved request+response,
/// identified by `request_number`. `status_code`/`response_time_ms` are
/// `None` when the response metadata hasn't landed yet (request written,
/// call still in flight).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QpSummary {
    pub request_number: i32,
    pub method: Option<String>,
    pub timestamp: Option<String>,
    pub status_code: Option<i32>,
    pub response_time_ms: Option<i32>,
}

pub fn list_qps_for_endpoint(core: &EdmsCore, queries: &QueryMap, endpoint_id: &str) -> EdmsResult<Vec<QpSummary>> {
    let q = queries.get_request_query("R8").ok_or(EdmsError::UnknownError)?;
    core.cproc(q, &[&endpoint_id], |row| {
        Ok(QpSummary {
            request_number: row.get(0)?,
            method: row.get(1)?,
            timestamp: row.get(2)?,
            status_code: row.get(3)?,
            response_time_ms: row.get(4)?,
        })
    })
}

/// Deletes one QP pair's metadata rows (request_metadata + response_metadata).
/// Caller is responsible for also removing the saved JSON files on disk.
pub fn delete_qp(core: &EdmsCore, queries: &QueryMap, endpoint_id: &str, request_number: i32) -> EdmsResult<usize> {
    let rq = queries.get_request_query("R7").ok_or(EdmsError::UnknownError)?;
    let req_rows = core.proc(rq, &[&endpoint_id, &request_number])?;

    let resq = queries.get_response_query("RES8").ok_or(EdmsError::UnknownError)?;
    let res_rows = core.proc(resq, &[&endpoint_id, &request_number])?;

    Ok(req_rows + res_rows)
}

/* ---------------- history (queries.yaml) ---------------- */

pub fn history_count(core: &EdmsCore, queries: &QueryMap) -> EdmsResult<usize> {
    let q = queries.get_history_query("H1").ok_or(EdmsError::UnknownError)?;
    let rows: Vec<i64> = core.cproc(q, &[], |row| row.get(0))?;
    Ok(rows.first().copied().unwrap_or(0) as usize)
}

pub fn clear_history(core: &EdmsCore, queries: &QueryMap) -> EdmsResult<usize> {
    let q = queries.get_history_query("H2").ok_or(EdmsError::UnknownError)?;
    core.proc(q, &[])
}

pub fn insert_history(core: &EdmsCore, queries: &QueryMap, endpoint_id: &str, action: &str, details: Option<&str>) -> EdmsResult<usize> {
    let q = queries.get_history_query("H3").ok_or(EdmsError::UnknownError)?;
    core.proc(
        q,
        &[&endpoint_id, &action, &details],
    )
}

pub fn list_history_endpoint_ids(core: &EdmsCore, queries: &QueryMap) -> EdmsResult<Vec<String>> {
    // Most recent first
    let q = queries.get_history_query("H4").ok_or(EdmsError::UnknownError)?;
    core.cproc(q, &[], |row| row.get(0))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub endpoint_id: String,
    pub action: String,
    pub details: Option<String>,
    pub timestamp: String,
}

pub fn list_history(core: &EdmsCore, queries: &QueryMap) -> EdmsResult<Vec<HistoryEntry>> {
    let q = queries.get_history_query("H5").ok_or(EdmsError::UnknownError)?;
    core.cproc(q, &[], |row| {
        Ok(HistoryEntry {
            id: row.get(0)?,
            endpoint_id: row.get(1)?,
            action: row.get(2)?,
            details: row.get(3)?,
            timestamp: row.get(4)?,
        })
    })
}

/* ---------------- bookmarks table (direct SQL) ---------------- */

/// Count bookmarks in an arbitrary folder (not just `active`).
pub fn bookmarks_count(core: &EdmsCore, queries: &QueryMap, folder: &str) -> EdmsResult<usize> {
    let q = queries.get_bookmark_query("B1").ok_or(EdmsError::UnknownError)?;
    let rows: Vec<i64> = core.cproc(q, &[&folder], |row| row.get(0))?;
    Ok(rows.first().copied().unwrap_or(0) as usize)
}

pub fn bookmarks_count_active(core: &EdmsCore, queries: &QueryMap) -> EdmsResult<usize> {
    bookmarks_count(core, queries, ACTIVE_FOLDER)
}

pub fn clear_bookmarks_active(core: &EdmsCore, queries: &QueryMap) -> EdmsResult<usize> {
    let q = queries.get_bookmark_query("B2").ok_or(EdmsError::UnknownError)?;
    core.proc(q, &[&ACTIVE_FOLDER])
}

pub fn list_bookmarked_endpoints_active(core: &EdmsCore, queries: &QueryMap) -> EdmsResult<Vec<String>> {
    // Returns endpoint_ids in active list
    let q = queries.get_bookmark_query("B3").ok_or(EdmsError::UnknownError)?;
    core.cproc(q, &[&ACTIVE_FOLDER], |row| row.get(0))
}

/// Same as `list_bookmarked_endpoints_active`, but also carries when each
/// one was bookmarked — the "updated" field bookmark-view consumers expect
/// (previously always null: it was tracked in the `bookmarks` table the
/// whole time, just never selected past this query).
pub fn list_bookmarked_endpoints_active_with_timestamps(
    core: &EdmsCore,
    queries: &QueryMap,
) -> EdmsResult<Vec<(String, String)>> {
    let q = queries.get_bookmark_query("B10").ok_or(EdmsError::UnknownError)?;
    core.cproc(q, &[&ACTIVE_FOLDER], |row| Ok((row.get(0)?, row.get(1)?)))
}

/// Bookmark an endpoint into an arbitrary folder (not just `active`).
/// Used by any caller that has a real target folder in hand — e.g. from a
/// URL path param — rather than always meaning the working set.
pub fn insert_bookmark(core: &EdmsCore, queries: &QueryMap, endpoint_id: &str, folder: &str, notes: Option<&str>) -> EdmsResult<usize> {
    // Check if already bookmarked to avoid duplicates
    let b4 = queries.get_bookmark_query("B4").ok_or(EdmsError::UnknownError)?;
    let existing: Vec<i64> = core.cproc(
        b4,
        &[&folder, &endpoint_id],
        |row| row.get(0)
    )?;

    if existing.first().copied().unwrap_or(0) > 0 {
        // Already bookmarked, return 0 rows affected
        return Ok(0);
    }

    let b5 = queries.get_bookmark_query("B5").ok_or(EdmsError::UnknownError)?;
    core.proc(
        b5,
        &[&endpoint_id, &folder, &notes],
    )
}

/// Remove a bookmark from an arbitrary folder (not just `active`).
pub fn delete_bookmark(core: &EdmsCore, queries: &QueryMap, endpoint_id: &str, folder: &str) -> EdmsResult<usize> {
    let b6 = queries.get_bookmark_query("B6").ok_or(EdmsError::UnknownError)?;
    core.proc(
        b6,
        &[&folder, &endpoint_id],
    )
}

pub fn insert_bookmark_active(core: &EdmsCore, queries: &QueryMap, endpoint_id: &str, notes: Option<&str>) -> EdmsResult<usize> {
    insert_bookmark(core, queries, endpoint_id, ACTIVE_FOLDER, notes)
}

pub fn delete_bookmark_active(core: &EdmsCore, queries: &QueryMap, endpoint_id: &str) -> EdmsResult<usize> {
    delete_bookmark(core, queries, endpoint_id, ACTIVE_FOLDER)
}

/* ---------------- collections (folder column) ---------------- */
//
// System 1 (arbitrary named bookmark folders as pseudo-collections) is
// retired as of the bookmark/collection merge (Mathew, 2026-09-08).
// `create_collection_from_active`/`load_collection_into_active` — which
// used to snapshot/restore against the `bookmarks` table's `folder`
// column — are gone. Real collections are System 2
// (`storage/collections/{name}.sqlite`, via CollectionMembershipOps);
// see handlers/bookmarks.rs for the load/save/unsave flow that replaces
// them, using this backup helper as its "don't lose unsaved active
// bookmarks on switch" step.

/// Backs up whatever's currently in `active` into the single rolling
/// `__session_backup__` slot, overwriting whatever backup was there
/// before. Returns whether there was anything to back up.
///
/// Per Mathew (2026-09-08): kept as the existing single-slot behavior —
/// not per-collection — so only the *most recent* switch is protected,
/// by design, not every collection ever visited.
pub fn backup_active_bookmarks(core: &EdmsCore, queries: &QueryMap) -> EdmsResult<bool> {
    let active_count = bookmarks_count_active(core, queries)?;
    if active_count == 0 {
        return Ok(false);
    }

    let b2 = queries.get_bookmark_query("B2").ok_or(EdmsError::UnknownError)?;
    let _ = core.proc(b2, &[&SESSION_BACKUP_FOLDER]);

    let endpoint_ids = list_bookmarked_endpoints_active(core, queries)?;
    let b7 = queries.get_bookmark_query("B7").ok_or(EdmsError::UnknownError)?;
    for eid in endpoint_ids {
        core.proc(b7, &[&eid, &SESSION_BACKUP_FOLDER])?;
    }
    Ok(true)
}

pub fn restore_from_backup(core: &EdmsCore, queries: &QueryMap) -> EdmsResult<usize> {
    // Clear current active
    clear_bookmarks_active(core, queries)?;
    
    // Copy backup to active
    let b3 = queries.get_bookmark_query("B3").ok_or(EdmsError::UnknownError)?;
    let ids: Vec<String> = core.cproc(b3, &[&SESSION_BACKUP_FOLDER], |row| row.get(0))?;
    
    let mut restored = 0usize;
    for eid in ids {
        restored += insert_bookmark_active(core, queries, &eid, None)?;
    }
    
    Ok(restored)
}

pub fn clear_session_backup(core: &EdmsCore, queries: &QueryMap) -> EdmsResult<usize> {
    let b2 = queries.get_bookmark_query("B2").ok_or(EdmsError::UnknownError)?;
    core.proc(b2, &[&SESSION_BACKUP_FOLDER])
}

pub fn endpoints_for_ids(core: &EdmsCore, _queries: &QueryMap, ids: &[String]) -> EdmsResult<Vec<EndpointDto>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    // Build IN clause with one placeholder per id
    let placeholders: Vec<&str> = ids.iter().map(|_| "?").collect();
    let query = format!(
        "SELECT endpoint_id, endpoint_str, annotation, method FROM endpoints WHERE endpoint_id IN ({})",
        placeholders.join(", ")
    );

    // Convert ids to params
    let params: Vec<&dyn ToSql> = ids.iter().map(|s| s as &dyn ToSql).collect();

    // Execute batch query
    let endpoints: Vec<EndpointDto> = core.cproc(&query, params.as_slice(), |row| {
        Ok(EndpointDto {
            endpoint_id: row.get(0)?,
            endpoint_str: row.get(1)?,
            annotation: row.get(2)?,
            method: row.get(3)?,
        })
    })?;

    // Preserve the original order from `ids`
    let mut result = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(ep) = endpoints.iter().find(|e| &e.endpoint_id == id) {
            result.push(ep.clone());
        }
    }

    Ok(result)
}
