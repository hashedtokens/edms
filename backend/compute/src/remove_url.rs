//! remove_url — URL / Address Prefix Stripper
//!
//! Provides utilities to strip scheme, host, and port from endpoints,
//! leaving only the relative path (with query/fragment if present).
//!
//! Supports:
//! 1. Pre-DB write: `strip_url_prefix(raw: &str) -> String`
//! 2. Post-DB write (Bookmark View):
//!    - `Apply (Selected)` by endpoint IDs or tags
//!    - `Apply (All)` globally across the entire database

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

/// Selection target for post-DB URL prefix removal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "mode", content = "payload")]
pub enum TargetSelection {
    /// Specific endpoint identifiers (EIDs)
    SelectedEndpoints(Vec<String>),
    /// Endpoints associated with specific tags
    SelectedTags(Vec<String>),
    /// All endpoints in the database globally
    All,
}

/// Request payload for removing URL prefix from stored endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveUrlRequest {
    pub db_path: String,
    pub target: TargetSelection,
}

/// Summary report of the URL stripping operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveUrlSummary {
    pub mode: String,
    pub total_scanned: usize,
    pub updated_count: usize,
    pub unchanged_count: usize,
}

/// Strips scheme (`http://`, `https://`, `ws://`, `wss://`) and host:port authority
/// from an endpoint string, preserving the path, query string, and fragment.
///
/// Examples:
/// - `"http://localhost:8080/api/v1/users"` -> `"/api/v1/users"`
/// - `"https://api.example.com/items?q=1#top"` -> `"/items?q=1#top"`
/// - `"localhost:3000/users"` -> `"/users"`
/// - `"127.0.0.1:8000/health"` -> `"/health"`
/// - `"/api/users"` -> `"/api/users"` (already relative)
/// - `""` -> `""`
pub fn strip_url_prefix(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // If it already starts with a single slash and not double slash, check if it's already a relative path
    if trimmed.starts_with('/') && !trimmed.starts_with("//") {
        return trimmed.to_string();
    }

    let without_scheme = if let Some(pos) = trimmed.find("://") {
        &trimmed[pos + 3..]
    } else if let Some(stripped) = trimmed.strip_prefix("//") {
        stripped
    } else {
        trimmed
    };

    // Find where the path begins (first '/' or '?' or '#')
    let path_start = without_scheme.find(|c| c == '/' || c == '?' || c == '#');

    match path_start {
        Some(idx) => {
            let path_part = &without_scheme[idx..];
            if path_part.starts_with('/') {
                path_part.to_string()
            } else {
                // Starts with '?' or '#' directly after authority, e.g. localhost:8080?q=1
                format!("/{}", path_part)
            }
        }
        None => {
            // It's just a host/port with no path (e.g. "localhost:8080" or "https://api.example.com")
            "/".to_string()
        }
    }
}

/// Applies URL prefix removal to endpoints in the SQLite database based on `TargetSelection`.
pub fn apply_remove_url_prefix(
    req: RemoveUrlRequest,
) -> Result<RemoveUrlSummary, Box<dyn std::error::Error + Send + Sync>> {
    let path = Path::new(&req.db_path);
    if !path.exists() {
        return Err(format!("Database file not found: {}", req.db_path).into());
    }

    let mut conn = Connection::open(path)?;

    // Resolve target endpoint IDs
    let target_eids: Option<HashSet<String>> = match &req.target {
        TargetSelection::SelectedEndpoints(eids) => Some(eids.iter().cloned().collect()),
        TargetSelection::SelectedTags(tags) => {
            if tags.is_empty() {
                Some(HashSet::new())
            } else {
                let placeholders = vec!["?"; tags.len()].join(",");
                let query = format!(
                    "SELECT DISTINCT endpoint_id FROM tags WHERE tag IN ({})",
                    placeholders
                );
                let mut stmt = conn.prepare(&query)?;
                let params: Vec<&dyn rusqlite::ToSql> =
                    tags.iter().map(|t| t as &dyn rusqlite::ToSql).collect();
                let eids = stmt
                    .query_map(params.as_slice(), |r| r.get::<_, String>(0))?
                    .collect::<Result<HashSet<String>, _>>()?;
                Some(eids)
            }
        }
        TargetSelection::All => None,
    };

    // Read matching endpoints
    let rows: Vec<(String, String)> = match &target_eids {
        Some(eids) if eids.is_empty() => Vec::new(),
        Some(eids) => {
            let mut result = Vec::new();
            let mut stmt = conn.prepare("SELECT endpoint_id, endpoint_str FROM endpoints WHERE endpoint_id = ?1")?;
            for eid in eids {
                if let Ok(mut cursor) = stmt.query(params![eid]) {
                    if let Ok(Some(row)) = cursor.next() {
                        let id: String = row.get(0)?;
                        let s: String = row.get(1)?;
                        result.push((id, s));
                    }
                }
            }
            result
        }
        None => {
            let mut stmt = conn.prepare("SELECT endpoint_id, endpoint_str FROM endpoints")?;
            let mapped = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            mapped.collect::<Result<Vec<_>, _>>()?
        }
    };

    let total_scanned = rows.len();
    let mut updated_count = 0;
    let mut unchanged_count = 0;

    let tx = conn.transaction()?;
    {
        let mut update_stmt = tx.prepare("UPDATE endpoints SET endpoint_str = ?1 WHERE endpoint_id = ?2")?;
        for (eid, original_str) in rows {
            let stripped = strip_url_prefix(&original_str);
            if stripped != original_str {
                update_stmt.execute(params![stripped, eid])?;
                updated_count += 1;
            } else {
                unchanged_count += 1;
            }
        }
    }
    tx.commit()?;

    let mode = match req.target {
        TargetSelection::SelectedEndpoints(_) => "SelectedEndpoints".to_string(),
        TargetSelection::SelectedTags(_) => "SelectedTags".to_string(),
        TargetSelection::All => "All".to_string(),
    };

    Ok(RemoveUrlSummary {
        mode,
        total_scanned,
        updated_count,
        unchanged_count,
    })
}
