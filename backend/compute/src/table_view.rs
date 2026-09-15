//! table_view.rs — Table View for Imports, Exports, and Takeout
//!
//! Provides real-time non-indexed scanning of `storage/imports/`,
//! format checking, moving into designated views, and SQLite-stripped Takeout.

use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io;
use std::path::Path;
use walkdir::WalkDir;
use zip::ZipArchive;

use crate::validate::{
    validate_bookmark_format, validate_webview_format, ComponentStatus, ValidationReport,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewPurpose {
    Collections,
    RepoView,
    WebView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableItem {
    pub name: String,
    pub relative_path: String,
    pub full_path: String,
    pub is_compressed: bool,
    pub purpose: Option<ViewPurpose>,
    pub size_bytes: u64,
    pub size_display: String,
    pub format_check: Option<ValidationReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableViewResponse {
    pub compressed: Vec<TableItem>,
    pub uncompressed: Vec<TableItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveResult {
    pub success: bool,
    pub destination: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakeoutResult {
    pub success: bool,
    pub destination: String,
    pub files_copied: usize,
    pub sqlite_files_stripped: usize,
    pub collision: bool,
    pub message: String,
}

/// Helper to format byte count into human-readable string.
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Recursively computes the total size of a file or directory.
pub fn compute_size(path: &Path) -> u64 {
    if path.is_file() {
        return path.metadata().map(|m| m.len()).unwrap_or(0);
    }
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}

/// Infers the view purpose from directory names (e.g. /collections/, /repoview/, /webview/).
pub fn infer_purpose(path: &Path) -> Option<ViewPurpose> {
    let s = path.to_string_lossy().to_lowercase();
    if s.contains("collection") {
        Some(ViewPurpose::Collections)
    } else if s.contains("repoview") || s.contains("repo") {
        Some(ViewPurpose::RepoView)
    } else if s.contains("webview") {
        Some(ViewPurpose::WebView)
    } else {
        None
    }
}

/// Scans the `storage/imports/` directory in real-time without indexing.
pub fn scan_imports_table(imports_dir: &Path) -> TableViewResponse {
    let mut compressed = Vec::new();
    let mut uncompressed = Vec::new();

    if !imports_dir.exists() {
        return TableViewResponse {
            compressed,
            uncompressed,
        };
    }

    let compressed_root = imports_dir.join("compressed");
    let uncompressed_root = imports_dir.join("uncompressed");

    // Scan compressed (.zip files)
    if compressed_root.exists() {
        for entry in WalkDir::new(&compressed_root)
            .min_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    if ext.eq_ignore_ascii_case("zip") {
                        let size_bytes = compute_size(path);
                        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        let relative_path = path
                            .strip_prefix(&compressed_root)
                            .unwrap_or(path)
                            .to_string_lossy()
                            .to_string();

                        compressed.push(TableItem {
                            name,
                            relative_path,
                            full_path: path.to_string_lossy().to_string(),
                            is_compressed: true,
                            purpose: infer_purpose(path),
                            size_bytes,
                            size_display: format_size(size_bytes),
                            format_check: None,
                        });
                    }
                }
            }
        }
    }

    // Scan uncompressed (directories)
    if uncompressed_root.exists() {
        // Find top-level item directories under uncompressed_root or its purpose subdirs
        for entry in WalkDir::new(&uncompressed_root)
            .min_depth(1)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_dir() {
                // If it contains files or subdirs directly
                let is_leaf_or_target = WalkDir::new(path)
                    .max_depth(1)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .any(|e| e.file_type().is_file() || e.path().extension().is_some());

                let rel_str = path
                    .strip_prefix(&uncompressed_root)
                    .unwrap_or(path)
                    .to_string_lossy();
                let depth = rel_str.split(['/', '\\']).count();

                if depth >= 2 || (depth == 1 && is_leaf_or_target) {
                    let size_bytes = compute_size(path);
                    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    let relative_path = rel_str.to_string();

                    // Avoid duplicate parent insertions
                    if !uncompressed.iter().any(|item: &TableItem| item.full_path == path.to_string_lossy()) {
                        uncompressed.push(TableItem {
                            name,
                            relative_path,
                            full_path: path.to_string_lossy().to_string(),
                            is_compressed: false,
                            purpose: infer_purpose(path),
                            size_bytes,
                            size_display: format_size(size_bytes),
                            format_check: None,
                        });
                    }
                }
            }
        }
    }

    TableViewResponse {
        compressed,
        uncompressed,
    }
}

/// Checks the format of an item (directory or zip) for a given purpose.
pub fn check_item_format(item_path: &Path, purpose: ViewPurpose) -> ValidationReport {
    if !item_path.exists() {
        return ValidationReport {
            passed: false,
            sqlite_status: ComponentStatus::Fail,
            edms_data_status: ComponentStatus::Fail,
            details: vec![format!("Path does not exist: {:?}", item_path)],
        };
    }

    // If it's a zip file, unpack to temporary directory and run check
    if item_path.is_file() {
        let temp_dir = match tempfile::TempDir::new() {
            Ok(d) => d,
            Err(e) => {
                return ValidationReport {
                    passed: false,
                    sqlite_status: ComponentStatus::Fail,
                    edms_data_status: ComponentStatus::Fail,
                    details: vec![format!("Failed to create temporary directory: {}", e)],
                };
            }
        };

        let file = match File::open(item_path) {
            Ok(f) => f,
            Err(e) => {
                return ValidationReport {
                    passed: false,
                    sqlite_status: ComponentStatus::Fail,
                    edms_data_status: ComponentStatus::Fail,
                    details: vec![format!("Failed to open zip file: {}", e)],
                };
            }
        };

        let mut archive = match ZipArchive::new(file) {
            Ok(a) => a,
            Err(e) => {
                return ValidationReport {
                    passed: false,
                    sqlite_status: ComponentStatus::Fail,
                    edms_data_status: ComponentStatus::Fail,
                    details: vec![format!("Failed to read zip archive: {}", e)],
                };
            }
        };

        if let Err(e) = archive.extract(temp_dir.path()) {
            return ValidationReport {
                passed: false,
                sqlite_status: ComponentStatus::Fail,
                edms_data_status: ComponentStatus::Fail,
                details: vec![format!("Failed to extract zip archive for inspection: {}", e)],
            };
        }

        return match purpose {
            ViewPurpose::Collections | ViewPurpose::RepoView => {
                validate_bookmark_format(temp_dir.path())
            }
            ViewPurpose::WebView => validate_webview_format(temp_dir.path()),
        };
    }

    // Uncompressed directory
    match purpose {
        ViewPurpose::Collections | ViewPurpose::RepoView => validate_bookmark_format(item_path),
        ViewPurpose::WebView => validate_webview_format(item_path),
    }
}

/// Moves an imported item (or unzips) into the designated storage view.
pub fn move_item_to_view(
    storage_root: &Path,
    item_path: &Path,
    purpose: ViewPurpose,
) -> Result<MoveResult, Box<dyn std::error::Error + Send + Sync>> {
    let dest_folder_name = match purpose {
        ViewPurpose::Collections => "collections",
        ViewPurpose::RepoView => "repoview",
        ViewPurpose::WebView => "webview",
    };

    let target_base = storage_root.join(dest_folder_name);
    fs::create_dir_all(&target_base)?;

    let raw_stem = item_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let dest_path = target_base.join(&raw_stem);

    if item_path.is_file() {
        // Zip file: extract directly into target
        let file = File::open(item_path)?;
        let mut archive = ZipArchive::new(file)?;
        fs::create_dir_all(&dest_path)?;
        archive.extract(&dest_path)?;
    } else {
        // Directory: copy or move into destination
        copy_dir_all(item_path, &dest_path)?;
    }

    Ok(MoveResult {
        success: true,
        destination: dest_path.to_string_lossy().to_string(),
        message: format!("Successfully moved {:?} to {:?}", item_path.file_name().unwrap_or_default(), dest_path),
    })
}

/// Recursively copies a directory tree.
fn copy_dir_all(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_child = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest_child)?;
        } else {
            fs::copy(entry.path(), dest_child)?;
        }
    }
    Ok(())
}

/// Copies a folder to `storage/takeout/{dest_name}/` while stripping all SQLite files.
pub fn takeout_item(
    source_path: &Path,
    dest_name: &str,
    storage_root: &Path,
    overwrite: bool,
) -> Result<TakeoutResult, Box<dyn std::error::Error + Send + Sync>> {
    let takeout_root = storage_root.join("takeout");
    fs::create_dir_all(&takeout_root)?;

    let dest_dir = takeout_root.join(dest_name);

    if dest_dir.exists() {
        if !overwrite {
            return Ok(TakeoutResult {
                success: false,
                destination: dest_dir.to_string_lossy().to_string(),
                files_copied: 0,
                sqlite_files_stripped: 0,
                collision: true,
                message: format!("Destination folder already exists: {:?}. Rename or enable overwrite.", dest_name),
            });
        } else {
            fs::remove_dir_all(&dest_dir)?;
        }
    }

    fs::create_dir_all(&dest_dir)?;

    let mut files_copied = 0;
    let mut sqlite_files_stripped = 0;

    for entry in WalkDir::new(source_path).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        let rel = p.strip_prefix(source_path)?;
        let target_file_path = dest_dir.join(rel);

        if p.is_dir() {
            fs::create_dir_all(&target_file_path)?;
        } else if p.is_file() {
            let is_sqlite = if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                let lower = ext.to_lowercase();
                lower == "sqlite"
                    || lower == "db"
                    || lower == "sqlitedb"
                    || lower == "sqlite-wal"
                    || lower == "sqlite-shm"
            } else {
                false
            };

            if is_sqlite {
                sqlite_files_stripped += 1;
            } else {
                if let Some(parent) = target_file_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(p, &target_file_path)?;
                files_copied += 1;
            }
        }
    }

    Ok(TakeoutResult {
        success: true,
        destination: dest_dir.to_string_lossy().to_string(),
        files_copied,
        sqlite_files_stripped,
        collision: false,
        message: format!(
            "Takeout complete: {} files copied, {} SQLite database files stripped",
            files_copied, sqlite_files_stripped
        ),
    })
}
