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
    // FIX #8: Use explicit column selection in query or handle by name
    // Assuming E3 is: SELECT id, endpoint_id, endpoint_str, annotation FROM endpoints
    core.cproc(q, &[], |row| {
        Ok(EndpointDto {
            // If using positional indices, document the expected query format
            // Better: ensure your queries.yaml has explicit column order
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

// NEW: Get history entries with full details for proper identification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub endpoint_id: String,
    pub action: String,
    pub details: Option<String>,
    pub timestamp: String,
}

pub fn list_history(core: &EdmsCore) -> EdmsResult<Vec<HistoryEntry>> {
    let q = "SELECT id, endpoint_id, action, details, timestamp FROM history ORDER BY timestamp DESC";
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

// FIX #5: Use INSERT OR IGNORE to prevent duplicates
pub fn insert_bookmark_active(core: &EdmsCore, endpoint_id: &str, notes: Option<&str>) -> EdmsResult<usize> {
    // Check if already bookmarked first (alternative to INSERT OR IGNORE if your schema doesn't have unique constraint)
    let existing: Vec<i64> = core.cproc(
        "SELECT COUNT(*) FROM bookmarks WHERE folder = ? AND endpoint_id = ?",
        &[&ACTIVE_FOLDER, &endpoint_id],
        |row| row.get(0)
    )?;
    
    if existing.first().copied().unwrap_or(0) > 0 {
        // Already bookmarked, return 0 rows affected
        return Ok(0);
    }
    
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
    let endpoint_ids = list_bookmarked_endpoints_active(core)?;
    let mut inserted = 0usize;

    for eid in endpoint_ids {
        // Check for duplicates in target collection too
        let existing: Vec<i64> = core.cproc(
            "SELECT COUNT(*) FROM bookmarks WHERE folder = ? AND endpoint_id = ?",
            &[&collection, &eid],
            |row| row.get(0)
        )?;
        
        if existing.first().copied().unwrap_or(0) == 0 {
            inserted += core.proc(
                "INSERT INTO bookmarks (endpoint_id, folder, notes) VALUES (?, ?, NULL)",
                &[&eid, &collection],
            )?;
        }
    }

    Ok(inserted)
}

// FIX #9: This should ideally use a transaction, but since EdmsCore might not expose
// transaction API directly, we'll document the limitation and do our best
pub fn load_collection_into_active(core: &EdmsCore, collection: &str) -> EdmsResult<(bool, usize)> {
    let active_count = bookmarks_count_active(core)?;
    let moved_to_backup = active_count > 0;

    if moved_to_backup {
        // FIX #10: Clear old backup before creating new one
        let _ = core.proc("DELETE FROM bookmarks WHERE folder = ?", &[&SESSION_BACKUP_FOLDER]);
        
        // Copy active into backup
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

// NEW: Restore from session backup
pub fn restore_from_backup(core: &EdmsCore) -> EdmsResult<usize> {
    // Clear current active
    clear_bookmarks_active(core)?;
    
    // Copy backup to active
    let q = "SELECT endpoint_id FROM bookmarks WHERE folder = ? ORDER BY timestamp DESC";
    let ids: Vec<String> = core.cproc(q, &[&SESSION_BACKUP_FOLDER], |row| row.get(0))?;
    
    let mut restored = 0usize;
    for eid in ids {
        restored += insert_bookmark_active(core, &eid, None)?;
    }
    
    Ok(restored)
}

// NEW: Clear session backup explicitly
pub fn clear_session_backup(core: &EdmsCore) -> EdmsResult<usize> {
    core.proc("DELETE FROM bookmarks WHERE folder = ?", &[&SESSION_BACKUP_FOLDER])
}

// FIX #11: Batch query instead of N+1
pub fn endpoints_for_ids(core: &EdmsCore, queries: &QueryMap, ids: &[String]) -> EdmsResult<Vec<EndpointDto>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    
    // Build a query with placeholders for all IDs
    // Note: SQLite has a limit on number of parameters (default 999), but for typical use this is fine
    let placeholders: Vec<&str> = ids.iter().map(|_| "?").collect();
    let query = format!(
        "SELECT id, endpoint_id, endpoint_str, annotation FROM endpoints WHERE endpoint_id IN ({})",
        placeholders.join(", ")
    );
    
    // Convert ids to params
    let params: Vec<&dyn ToSql> = ids.iter().map(|s| s as &dyn ToSql).collect();
    
    // Execute batch query
    let endpoints: Vec<EndpointDto> = core.cproc(&query, params.as_slice(), |row| {
        Ok(EndpointDto {
            endpoint_id: row.get(1)?,
            endpoint_str: row.get(2)?,
            annotation: row.get(3)?,
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

// Alternative simpler version if the above doesn't work with your EdmsCore API:
pub fn endpoints_for_ids_simple(core: &EdmsCore, queries: &QueryMap, ids: &[String]) -> EdmsResult<Vec<EndpointDto>> {
    // Fallback to N queries if batch doesn't work
    // At least we tried!
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(ep) = get_endpoint(core, queries, id)? {
            out.push(ep);
        }
    }
    Ok(out)
}
