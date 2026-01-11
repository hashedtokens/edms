use edms::core::EdmsCore;
use edms::error::{EdmsError, EdmsResult};
use edms::query_loader::QueryMap;
use rusqlite::ToSql;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const ACTIVE_FOLDER: &str = "__active__";
pub const SESSION_BACKUP_FOLDER: &str = "__session_backup__";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointDto {
    pub endpoint_id: String,
    pub endpoint_str: String,
    pub annotation: Option<String>,
}

/* ---------------- endpoints (queries.yaml) ---------------- */

pub fn insert_endpoint(core: &EdmsCore, queries: &QueryMap, ep: &EndpointDto) -> EdmsResult<usize> {
    let q = queries.get_endpoint_query("E1").ok_or(EdmsError::UnknownError)?;
    // E1: INSERT INTO endpoints (endpoint_id, endpoint_str, annotation) VALUES (?, ?, ?)
    core.proc(q, &[&ep.endpoint_id, &ep.endpoint_str, &ep.annotation.as_deref()])
}

pub fn list_endpoints(core: &EdmsCore, queries: &QueryMap) -> EdmsResult<Vec<EndpointDto>> {
    let q = queries.get_endpoint_query("E3").ok_or(EdmsError::UnknownError)?;
    core.cproc(q, &[], |row| {
        Ok(EndpointDto {
            endpoint_id: row.get(1)?,
            endpoint_str: row.get(2)?,
            annotation: row.get(3)?,
        })
    })
}

pub fn get_endpoint(core: &EdmsCore, queries: &QueryMap, endpoint_id: &str) -> EdmsResult<Option<EndpointDto>> {
    let q = queries.get_endpoint_query("E2").ok_or(EdmsError::UnknownError)?;
    let rows = core.cproc(q, &[&endpoint_id], |row| {
        Ok(EndpointDto {
            endpoint_id: row.get(1)?,
            endpoint_str: row.get(2)?,
            annotation: row.get(3)?,
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

/* ---------------- history table (direct SQL) ---------------- */

pub fn history_count(core: &EdmsCore) -> EdmsResult<usize> {
    let q = "SELECT COUNT(*) FROM history";
    let rows: Vec<i64> = core.cproc(q, &[], |row| row.get(0))?;
    Ok(rows.first().copied().unwrap_or(0) as usize)
}

pub fn clear_history(core: &EdmsCore) -> EdmsResult<usize> {
    core.proc("DELETE FROM history", &[])
}

pub fn insert_history(core: &EdmsCore, endpoint_id: &str, action: &str, details: Option<&str>) -> EdmsResult<usize> {
    core.proc(
        "INSERT INTO history (endpoint_id, action, details) VALUES (?, ?, ?)",
        &[&endpoint_id, &action, &details],
    )
}

pub fn list_history_endpoint_ids(core: &EdmsCore) -> EdmsResult<Vec<String>> {
    // Most recent first
    let q = "SELECT endpoint_id FROM history ORDER BY timestamp DESC";
    core.cproc(q, &[], |row| row.get(0))
}

/* ---------------- bookmarks table (direct SQL) ---------------- */

pub fn bookmarks_count_active(core: &EdmsCore) -> EdmsResult<usize> {
    let q = "SELECT COUNT(*) FROM bookmarks WHERE folder = ?";
    let rows: Vec<i64> = core.cproc(q, &[&ACTIVE_FOLDER], |row| row.get(0))?;
    Ok(rows.first().copied().unwrap_or(0) as usize)
}

pub fn clear_bookmarks_active(core: &EdmsCore) -> EdmsResult<usize> {
    core.proc("DELETE FROM bookmarks WHERE folder = ?", &[&ACTIVE_FOLDER])
}

pub fn list_bookmarked_endpoints_active(core: &EdmsCore) -> EdmsResult<Vec<String>> {
    // Returns endpoint_ids in active list
    let q = "SELECT endpoint_id FROM bookmarks WHERE folder = ? ORDER BY timestamp DESC";
    core.cproc(q, &[&ACTIVE_FOLDER], |row| row.get(0))
}

pub fn insert_bookmark_active(core: &EdmsCore, endpoint_id: &str, notes: Option<&str>) -> EdmsResult<usize> {
    core.proc(
        "INSERT INTO bookmarks (endpoint_id, folder, notes) VALUES (?, ?, ?)",
        &[&endpoint_id, &ACTIVE_FOLDER, &notes],
    )
}

pub fn delete_bookmark_active(core: &EdmsCore, endpoint_id: &str) -> EdmsResult<usize> {
    core.proc(
        "DELETE FROM bookmarks WHERE folder = ? AND endpoint_id = ?",
        &[&ACTIVE_FOLDER, &endpoint_id],
    )
}

/* ---------------- collections (folder column) ---------------- */

pub fn create_collection_from_active(core: &EdmsCore, collection: &str) -> EdmsResult<usize> {
    // Copy active bookmarks into folder=collection
    // Simple: insert rows for each active endpoint_id
    let endpoint_ids = list_bookmarked_endpoints_active(core)?;
    let mut inserted = 0usize;

    for eid in endpoint_ids {
       
        
        inserted += core.proc(
            "INSERT INTO bookmarks (endpoint_id, folder, notes) VALUES (?, ?, NULL)",
            &[&eid, &collection],
        )?;
    }

    Ok(inserted)
}

pub fn load_collection_into_active(core: &EdmsCore, collection: &str) -> EdmsResult<(bool, usize)> {
    // Move current active -> session backup, then load requested collection -> active
    let active_count = bookmarks_count_active(core)?;
    let moved_to_backup = active_count > 0;

    if moved_to_backup {
        // copy active into backup
        let endpoint_ids = list_bookmarked_endpoints_active(core)?;
        for eid in endpoint_ids {
            let _ = core.proc(
                "INSERT INTO bookmarks (endpoint_id, folder, notes) VALUES (?, ?, NULL)",
                &[&eid, &SESSION_BACKUP_FOLDER],
            )?;
        }
    }

    // Replace active with collection
    clear_bookmarks_active(core)?;
    let q = "SELECT endpoint_id FROM bookmarks WHERE folder = ? ORDER BY timestamp DESC";
    let ids: Vec<String> = core.cproc(q, &[&collection], |row| row.get(0))?;
    let mut loaded = 0usize;
    for eid in ids {
        loaded += insert_bookmark_active(core, &eid, None)?;
    }

    Ok((moved_to_backup, loaded))
}



pub fn endpoints_for_ids(core: &EdmsCore, queries: &QueryMap, ids: &[String]) -> EdmsResult<Vec<EndpointDto>> {
    // Not super efficient but fine for “make it work”
    let mut out = Vec::new();
    for id in ids {
        if let Some(ep) = get_endpoint(core, queries, id)? {
            out.push(ep);
        }
    }
    Ok(out)
}
