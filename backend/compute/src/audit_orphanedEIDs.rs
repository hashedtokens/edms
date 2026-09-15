//! audit_orphanedEIDs.rs — Pre-Purge Orphaned EID Audit Report and Purge Execution
//!
//! Scans globalEQPData and database endpoints to detect unreferenced / orphaned EIDs:
//! 1. `generate_audit_report`: Writes audit summary to JSON on disk (e.g. tmp/audit.json).
//! 2. `execute_purge`: Removes orphaned disk folders and DB rows based on the audit report.

use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    pub timestamp: String,
    pub total_disk_eids: usize,
    pub total_db_endpoints: usize,
    pub total_db_bookmarks: usize,
    pub orphaned_disk_eids: Vec<String>,
    pub orphaned_db_endpoints: Vec<String>,
    pub report_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurgeResult {
    pub disk_eids_purged: usize,
    pub db_endpoints_purged: usize,
    pub db_tags_purged: usize,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRequest {
    pub db_path: String,
    pub eqp_dir: String,
    pub output_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurgeRequest {
    pub db_path: String,
    pub eqp_dir: String,
    pub audit_report_path: Option<String>,
}

/// Generates an audit report of orphaned EIDs between disk storage (`globalEQPData`)
/// and the SQLite database. Writes the report to `output_path`.
pub fn generate_audit_report(
    db_path: &Path,
    eqp_dir: &Path,
    output_path: &Path,
) -> Result<AuditReport, Box<dyn std::error::Error + Send + Sync>> {
    let conn = Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;

    // 1. Fetch DB endpoints
    let mut stmt = conn.prepare("SELECT endpoint_id FROM endpoints")?;
    let db_endpoints: HashSet<String> = stmt
        .query_map([], |r| r.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    // 2. Fetch DB bookmarks / collection members
    let mut stmt = conn.prepare("SELECT DISTINCT endpoint_id FROM bookmarks")?;
    let db_bookmarks: HashSet<String> = stmt
        .query_map([], |r| r.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    // 3. Scan disk EIDs in globalEQPData
    let mut disk_eids = Vec::new();
    if eqp_dir.exists() {
        for entry in fs::read_dir(eqp_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    disk_eids.push(name.to_string());
                }
            }
        }
    }

    // 4. Calculate orphans
    let mut orphaned_disk_eids: Vec<String> = disk_eids
        .iter()
        .filter(|eid| !db_endpoints.contains(*eid) && !db_bookmarks.contains(*eid))
        .cloned()
        .collect();
    orphaned_disk_eids.sort();

    let mut orphaned_db_endpoints: Vec<String> = db_endpoints
        .iter()
        .filter(|eid| !db_bookmarks.contains(*eid))
        .cloned()
        .collect();
    orphaned_db_endpoints.sort();

    let report = AuditReport {
        timestamp: Utc::now().to_rfc3339(),
        total_disk_eids: disk_eids.len(),
        total_db_endpoints: db_endpoints.len(),
        total_db_bookmarks: db_bookmarks.len(),
        orphaned_disk_eids,
        orphaned_db_endpoints,
        report_path: output_path.to_string_lossy().to_string(),
    };

    // Ensure output directory exists and write JSON report
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json_bytes = serde_json::to_vec_pretty(&report)?;
    let mut file = File::create(output_path)?;
    file.write_all(&json_bytes)?;

    Ok(report)
}

/// Executes the purge of orphaned disk folders and DB rows based on the audit report.
pub fn execute_purge(
    db_path: &Path,
    eqp_dir: &Path,
    report: &AuditReport,
) -> Result<PurgeResult, Box<dyn std::error::Error + Send + Sync>> {
    let mut disk_eids_purged = 0;

    // 1. Remove orphaned disk folders from globalEQPData
    for eid in &report.orphaned_disk_eids {
        let dir_to_remove = eqp_dir.join(eid);
        if dir_to_remove.exists() {
            fs::remove_dir_all(&dir_to_remove)?;
            disk_eids_purged += 1;
        }
    }

    // 2. Remove orphaned endpoints and associated tags from SQLite DB
    let mut conn = Connection::open(db_path)?;
    let tx = conn.transaction()?;

    let mut db_endpoints_purged = 0;
    let mut db_tags_purged = 0;

    {
        let mut del_ep_stmt = tx.prepare("DELETE FROM endpoints WHERE endpoint_id = ?1")?;
        let mut del_tag_stmt = tx.prepare("DELETE FROM tags WHERE endpoint_id = ?1")?;

        for eid in &report.orphaned_db_endpoints {
            let tags_deleted = del_tag_stmt.execute(rusqlite::params![eid])?;
            db_tags_purged += tags_deleted;

            let ep_deleted = del_ep_stmt.execute(rusqlite::params![eid])?;
            db_endpoints_purged += ep_deleted;
        }
    }
    tx.commit()?;

    Ok(PurgeResult {
        disk_eids_purged,
        db_endpoints_purged,
        db_tags_purged,
        success: true,
        message: format!(
            "Purge completed: {} disk directories removed, {} database endpoints deleted, {} tags cleaned",
            disk_eids_purged, db_endpoints_purged, db_tags_purged
        ),
    })
}
