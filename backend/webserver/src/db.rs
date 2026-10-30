use edms::core::EdmsCore;
use edms::error::{EdmsError, EdmsResult};
use edms::query_loader::QueryMap;
use rusqlite::ToSql;
use serde::{Deserialize, Serialize};

// ACTIVE_FOLDER / SESSION_BACKUP_FOLDER retired (2026-09-22, Option A bookmark
// redesign): there is no longer a single global "active" bucket — every
// bookmark now belongs to an explicit collection (folder = collection name),
// so there is nothing to back up when switching, and no gate to check
// before writing. See handlers/bookmarks.rs and handlers/test_view.rs.

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

/// When an endpoint was created and when it last changed, for the RepoView
/// index and the takeout manifest. The central `updated_at` only moves when
/// the endpoint's own row is written (creating it, or editing its annotation),
/// so "last changed" is the newest of that, its newest QP and its newest tag:
/// what a person would call the last time anyone touched it. `None` if there
/// is no such endpoint.
pub fn get_endpoint_dates(
    core: &EdmsCore,
    endpoint_id: &str,
) -> EdmsResult<Option<(Option<String>, Option<String>)>> {
    let rows = core.cproc(
        "SELECT e.created_at,
                MAX(COALESCE(e.updated_at, e.created_at, ''),
                    COALESCE((SELECT MAX(timestamp) FROM request_metadata WHERE endpoint_id = e.endpoint_id), ''),
                    COALESCE((SELECT MAX(created_at) FROM tags WHERE endpoint_id = e.endpoint_id), ''))
         FROM endpoints e WHERE e.endpoint_id = ?",
        &[&endpoint_id],
        |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?)),
    )?;
    Ok(rows
        .into_iter()
        .next()
        .map(|(created, updated)| (created, updated.filter(|u| !u.is_empty()))))
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
    let rows = core.proc(q, &[&endpoint_id])?;
    if rows > 0 {
        let _ = delete_endpoint_data(core, endpoint_id);
    }
    Ok(rows)
}

/// Cascade for endpoint deletion: removes all QP metadata rows (request_metadata,
/// response_metadata), history rows, tags, and summary metadata so that a recycled EID starts fresh without
/// phantom QPs or stale history.
pub fn delete_endpoint_data(core: &EdmsCore, endpoint_id: &str) -> EdmsResult<()> {
    let _ = core.proc(
        "DELETE FROM request_metadata WHERE endpoint_id = ?",
        &[&endpoint_id],
    );
    let _ = core.proc(
        "DELETE FROM response_metadata WHERE endpoint_id = ?",
        &[&endpoint_id],
    );
    let _ = core.proc(
        "DELETE FROM history WHERE endpoint_id = ?",
        &[&endpoint_id],
    );
    let _ = core.proc(
        "DELETE FROM tags WHERE endpoint_id = ?",
        &[&endpoint_id],
    );
    let _ = core.proc(
        "DELETE FROM metadata WHERE endpoint_id = ?",
        &[&endpoint_id],
    );
    Ok(())
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
    status_code: Option<i32>,
    response_time_ms: Option<i32>,
) -> EdmsResult<usize> {
    let _ = core.proc(
        "DELETE FROM response_metadata WHERE endpoint_id = ? AND request_number = ?",
        &[&endpoint_id, &request_number],
    );
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

/// Whether a specific QP pair's request_metadata row exists — used by the
/// update route to 404 on an unknown request_number rather than silently
/// writing files for a pair the DB never recorded.
pub fn qp_exists(core: &EdmsCore, queries: &QueryMap, endpoint_id: &str, request_number: i32) -> EdmsResult<bool> {
    let q = queries.get_request_query("R9").ok_or(EdmsError::UnknownError)?;
    let rows: Vec<i64> = core.cproc(q, &[&endpoint_id, &request_number], |row| row.get(0))?;
    Ok(rows.first().copied().unwrap_or(0) > 0)
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

/// Total bookmarks across every collection — the closest equivalent to the
/// old "active" bucket's count now that there's no single privileged one.
pub fn bookmarks_count_total(core: &EdmsCore, queries: &QueryMap) -> EdmsResult<usize> {
    let q = queries.get_bookmark_query("B11").ok_or(EdmsError::UnknownError)?;
    let rows: Vec<i64> = core.cproc(q, &[], |row| row.get(0))?;
    Ok(rows.first().copied().unwrap_or(0) as usize)
}

/// List endpoint IDs bookmarked into a specific folder (collection), with
/// when each one was bookmarked — the "updated" field bookmark-view
/// consumers expect.
pub fn list_bookmarked_endpoints_with_timestamps(
    core: &EdmsCore,
    queries: &QueryMap,
    folder: &str,
) -> EdmsResult<Vec<(String, String)>> {
    let q = queries.get_bookmark_query("B10").ok_or(EdmsError::UnknownError)?;
    core.cproc(q, &[&folder], |row| Ok((row.get(0)?, row.get(1)?)))
}

/// Whether an endpoint is currently bookmarked into a specific folder.
pub fn bookmark_exists(core: &EdmsCore, queries: &QueryMap, folder: &str, endpoint_id: &str) -> EdmsResult<bool> {
    let q = queries.get_bookmark_query("B4").ok_or(EdmsError::UnknownError)?;
    let rows: Vec<i64> = core.cproc(q, &[&folder, &endpoint_id], |row| row.get(0))?;
    Ok(rows.first().copied().unwrap_or(0) > 0)
}

/// Cascade for collection deletion: drops every bookmark that belonged to
/// it, so deleting a collection doesn't leave orphaned rows behind.
pub fn delete_bookmarks_for_folder(core: &EdmsCore, queries: &QueryMap, folder: &str) -> EdmsResult<usize> {
    let q = queries.get_bookmark_query("B2").ok_or(EdmsError::UnknownError)?;
    core.proc(q, &[&folder])
}

/// Cascade for collection rename: keeps its bookmarks attached under the
/// new name instead of stranding them under the old one.
pub fn rename_bookmarks_folder(core: &EdmsCore, queries: &QueryMap, old_folder: &str, new_folder: &str) -> EdmsResult<usize> {
    let q = queries.get_bookmark_query("B12").ok_or(EdmsError::UnknownError)?;
    core.proc(q, &[&new_folder, &old_folder])
}

/// Cascade for endpoint deletion: drops all of its bookmarks, across every
/// collection, not just one.
pub fn delete_bookmarks_for_endpoint(core: &EdmsCore, queries: &QueryMap, endpoint_id: &str) -> EdmsResult<usize> {
    let q = queries.get_bookmark_query("B13").ok_or(EdmsError::UnknownError)?;
    core.proc(q, &[&endpoint_id])
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

/* ---------------- collections (folder column) ---------------- */
//
// System 1 (arbitrary named bookmark folders as pseudo-collections) is
// retired as of the bookmark/collection merge (Mathew, 2026-09-08). Real
// collections are System 2 (`storage/collections/{name}.sqlite`, via
// CollectionMembershipOps) — `folder` here now holds a real collection
// name directly. The single-"active"-bucket layer that used to sit
// between them (insert_bookmark_active, backup_active_bookmarks,
// restore_from_backup, clear_session_backup, __session_backup__) is
// retired too (2026-09-22): with bookmarks keyed by the actual collection
// name instead of one shared slot, there's nothing to back up when
// switching — every collection's bookmarks are already durably separate.

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
pub fn reassign_qp_numbers(
    core: &EdmsCore,
    endpoint_id: &str,
    storage_dir: &str,
) -> EdmsResult<usize> {
    let existing: Vec<i32> = core.cproc(
        "SELECT DISTINCT request_number FROM request_metadata WHERE endpoint_id = ? ORDER BY request_number ASC",
        &[&endpoint_id],
        |row| row.get(0),
    )?;

    let mut renamed = 0usize;

    for (zero_idx, old_n) in existing.iter().enumerate() {
        let new_n = (zero_idx as i32) + 1;
        if *old_n == new_n {
            continue;
        }

        let new_req_path = format!("{}/{}-request-{}.json", storage_dir, endpoint_id, new_n);
        let new_res_path = format!("{}/{}-response-{}.json", storage_dir, endpoint_id, new_n);
        let req_str: &str = &new_req_path;
        let res_str: &str = &new_res_path;

        core.proc(
            "UPDATE request_metadata SET request_number = ?, file_path = ? WHERE endpoint_id = ? AND request_number = ?",
            &[&new_n, &req_str, &endpoint_id, old_n],
        )?;

        core.proc(
            "UPDATE response_metadata SET request_number = ?, file_path = ? WHERE endpoint_id = ? AND request_number = ?",
            &[&new_n, &res_str, &endpoint_id, old_n],
        )?;

        renamed += 1;
    }

    Ok(renamed)
}
