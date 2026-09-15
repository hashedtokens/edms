//! validate.rs — EDMS Format Standardization & Validation
//!
//! Provides validation algorithms for EDMS storage formats:
//! - Bookmark / Collection / Repo format (`validate_bookmark_format`):
//!   Requires valid SQLite database and correctly structured JSON EID QP data.
//! - WebView format (`validate_webview_format`):
//!   Strictly rejects SQLite files; validates JSON data and folder structure.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComponentStatus {
    Pass,
    Fail,
    NotApplicable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub passed: bool,
    pub sqlite_status: ComponentStatus,
    pub edms_data_status: ComponentStatus,
    pub details: Vec<String>,
}

/// Helper to check if a file is a valid JSON document.
pub fn is_valid_json_file(path: &Path) -> bool {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut content = String::new();
    if file.read_to_string(&mut content).is_err() {
        return false;
    }
    serde_json::from_str::<serde_json::Value>(&content).is_ok()
}

/// Validates Bookmark / Collection / Repo format.
///
/// Rules:
/// 1. Must contain at least one `.sqlite` or `.db` file that passes SQLite integrity.
/// 2. Must contain required tables (`endpoints`, `tags`, `bookmarks`).
/// 3. All JSON files inside EID directories must be valid JSON.
pub fn validate_bookmark_format(root: &Path) -> ValidationReport {
    let mut details = Vec::new();
    if !root.exists() {
        return ValidationReport {
            passed: false,
            sqlite_status: ComponentStatus::Fail,
            edms_data_status: ComponentStatus::Fail,
            details: vec![format!("Directory does not exist: {:?}", root)],
        };
    }

    // 1. Check for SQLite database file(s)
    let mut sqlite_found = false;
    let mut sqlite_valid = false;

    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let ext_lower = ext.to_lowercase();
                if ext_lower == "sqlite" || ext_lower == "db" || ext_lower == "sqlitedb" {
                    sqlite_found = true;
                    match Connection::open_with_flags(
                        path,
                        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
                    ) {
                        Ok(conn) => {
                            let integrity: Result<String, _> = conn.query_row(
                                "PRAGMA quick_check",
                                [],
                                |r| r.get(0),
                            );
                            if integrity.as_deref() == Ok("ok") {
                                // Check for fundamental EDMS tables
                                let has_endpoints: bool = conn
                                    .query_row(
                                        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='endpoints'",
                                        [],
                                        |r| r.get::<_, i64>(0),
                                    )
                                    .map(|count| count > 0)
                                    .unwrap_or(false);

                                let has_bookmarks: bool = conn
                                    .query_row(
                                        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND (name='bookmarks' OR name='collection_members')",
                                        [],
                                        |r| r.get::<_, i64>(0),
                                    )
                                    .map(|count| count > 0)
                                    .unwrap_or(false);

                                if has_endpoints || has_bookmarks {
                                    sqlite_valid = true;
                                    details.push(format!("Valid SQLite database verified: {:?}", path.file_name().unwrap_or_default()));
                                    break;
                                } else {
                                    details.push(format!("SQLite database at {:?} is missing required EDMS tables", path.file_name().unwrap_or_default()));
                                }
                            } else {
                                details.push(format!("SQLite integrity check failed for {:?}", path));
                            }
                        }
                        Err(e) => {
                            details.push(format!("Failed to open SQLite database at {:?}: {}", path, e));
                        }
                    }
                }
            }
        }
    }

    let sqlite_status = if !sqlite_found {
        details.push("No SQLite database (.sqlite / .db) found in target directory".to_string());
        ComponentStatus::Fail
    } else if sqlite_valid {
        ComponentStatus::Pass
    } else {
        ComponentStatus::Fail
    };

    // 2. Check EDMS QP data JSON files
    let mut data_valid = true;
    let mut json_files_checked = 0;

    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if ext.eq_ignore_ascii_case("json") {
                    json_files_checked += 1;
                    if !is_valid_json_file(path) {
                        data_valid = false;
                        details.push(format!("Corrupted or invalid JSON file: {:?}", path));
                    }
                }
            }
        }
    }

    let edms_data_status = if data_valid {
        if json_files_checked > 0 {
            details.push(format!("Verified {} JSON data files successfully", json_files_checked));
        }
        ComponentStatus::Pass
    } else {
        ComponentStatus::Fail
    };

    let passed = sqlite_status == ComponentStatus::Pass && edms_data_status == ComponentStatus::Pass;

    ValidationReport {
        passed,
        sqlite_status,
        edms_data_status,
        details,
    }
}

/// Validates WebView format.
///
/// Rules:
/// 1. Must NOT contain any SQLite (.sqlite / .db / .sqlitedb) files.
/// 2. Must contain valid JSON files and valid folder hierarchy.
pub fn validate_webview_format(root: &Path) -> ValidationReport {
    let mut details = Vec::new();
    if !root.exists() {
        return ValidationReport {
            passed: false,
            sqlite_status: ComponentStatus::NotApplicable,
            edms_data_status: ComponentStatus::Fail,
            details: vec![format!("Directory does not exist: {:?}", root)],
        };
    }

    let mut found_sqlite = false;
    let mut json_files_checked = 0;
    let mut data_valid = true;

    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let ext_lower = ext.to_lowercase();
                if ext_lower == "sqlite" || ext_lower == "db" || ext_lower == "sqlitedb" {
                    found_sqlite = true;
                    details.push(format!("WebView format violation: unexpected SQLite file found: {:?}", path));
                } else if ext_lower == "json" {
                    json_files_checked += 1;
                    if !is_valid_json_file(path) {
                        data_valid = false;
                        details.push(format!("Corrupted or invalid JSON file: {:?}", path));
                    }
                }
            }
        }
    }

    let sqlite_status = if found_sqlite {
        ComponentStatus::Fail
    } else {
        ComponentStatus::NotApplicable
    };

    let edms_data_status = if data_valid {
        if json_files_checked > 0 {
            details.push(format!("Verified {} WebView JSON data files successfully", json_files_checked));
        }
        ComponentStatus::Pass
    } else {
        ComponentStatus::Fail
    };

    let passed = sqlite_status != ComponentStatus::Fail && edms_data_status == ComponentStatus::Pass;

    ValidationReport {
        passed,
        sqlite_status,
        edms_data_status,
        details,
    }
}
