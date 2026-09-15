//! Tests for Table View (Imports, Exports, Takeout) and EDMS Format Standardization

use compute::table_view::{
    check_item_format, compute_size, format_size, move_item_to_view, scan_imports_table,
    takeout_item, ViewPurpose,
};
use compute::validate::{
    validate_bookmark_format, validate_webview_format, ComponentStatus,
};
use rusqlite::Connection;
use std::fs::{self, File};
use std::io::Write;
use tempfile::TempDir;

// ── 1. Validation Tests ───────────────────────────────────────────────────────

#[test]
fn test_validate_bookmark_format_valid() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    // Create valid SQLite database
    let db_path = root.join("collection.sqlite");
    let conn = Connection::open(&db_path).unwrap();
    edms::schema::initialize_schema(&conn).unwrap();
    drop(conn);

    // Create valid EID folder and QP JSON files
    let eid_dir = root.join("E0001-AAA");
    fs::create_dir_all(&eid_dir).unwrap();
    fs::write(
        eid_dir.join("E0001-AAA-request-1.json"),
        r#"{"url":"/api/users","method":"GET"}"#,
    )
    .unwrap();
    fs::write(
        eid_dir.join("E0001-AAA-response-1.json"),
        r#"{"status":200,"body":"ok"}"#,
    )
    .unwrap();

    let report = validate_bookmark_format(root);
    assert!(report.passed, "Valid collection must pass: {:?}", report.details);
    assert_eq!(report.sqlite_status, ComponentStatus::Pass);
    assert_eq!(report.edms_data_status, ComponentStatus::Pass);
}

#[test]
fn test_validate_bookmark_format_missing_sqlite_fails() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    // Only JSON files, no SQLite DB
    let eid_dir = root.join("E0001-AAA");
    fs::create_dir_all(&eid_dir).unwrap();
    fs::write(eid_dir.join("data.json"), r#"{"key":"val"}"#).unwrap();

    let report = validate_bookmark_format(root);
    assert!(!report.passed, "Missing SQLite DB must fail bookmark format");
    assert_eq!(report.sqlite_status, ComponentStatus::Fail);
}

#[test]
fn test_validate_bookmark_format_corrupted_json_fails() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    // Valid SQLite DB
    let db_path = root.join("collection.sqlite");
    let conn = Connection::open(&db_path).unwrap();
    edms::schema::initialize_schema(&conn).unwrap();
    drop(conn);

    // Corrupted JSON
    let eid_dir = root.join("E0001-AAA");
    fs::create_dir_all(&eid_dir).unwrap();
    fs::write(eid_dir.join("corrupted.json"), "{ invalid json:").unwrap();

    let report = validate_bookmark_format(root);
    assert!(!report.passed, "Corrupted JSON must fail data status");
    assert_eq!(report.edms_data_status, ComponentStatus::Fail);
}

#[test]
fn test_validate_webview_format_valid() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    // Valid webview: only JSON and static files, no SQLite
    fs::write(root.join("manifest.json"), r#"{"name":"webview-demo"}"#).unwrap();
    fs::write(root.join("endpoints.json"), r#"[{"url":"/users"}]"#).unwrap();

    let report = validate_webview_format(root);
    assert!(report.passed, "Clean webview folder must pass: {:?}", report.details);
    assert_eq!(report.sqlite_status, ComponentStatus::NotApplicable);
    assert_eq!(report.edms_data_status, ComponentStatus::Pass);
}

#[test]
fn test_validate_webview_format_sqlite_present_fails() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    // Webview with forbidden SQLite DB
    fs::write(root.join("manifest.json"), r#"{"name":"webview-demo"}"#).unwrap();
    fs::write(root.join("rogue.sqlite"), "SQLite format 3\0").unwrap();

    let report = validate_webview_format(root);
    assert!(!report.passed, "WebView with SQLite DB must fail");
    assert_eq!(report.sqlite_status, ComponentStatus::Fail);
}

// ── 2. Table View Scanning & Operations Tests ─────────────────────────────────

#[test]
fn test_scan_imports_table_finds_compressed_and_uncompressed() {
    let dir = TempDir::new().unwrap();
    let imports_dir = dir.path();

    let comp_dir = imports_dir.join("compressed").join("collections");
    let uncomp_dir = imports_dir.join("uncompressed").join("collections").join("folder_a");
    fs::create_dir_all(&comp_dir).unwrap();
    fs::create_dir_all(&uncomp_dir).unwrap();

    // Create a zip file in compressed
    let zip_path = comp_dir.join("archive.zip");
    let file = File::create(&zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file("sample.txt", zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(b"content").unwrap();
    zip.finish().unwrap();

    // Create a file in uncompressed folder
    fs::write(uncomp_dir.join("data.json"), r#"{"a":1}"#).unwrap();

    let resp = scan_imports_table(imports_dir);
    assert_eq!(resp.compressed.len(), 1);
    assert_eq!(resp.compressed[0].name, "archive.zip");
    assert_eq!(resp.compressed[0].purpose, Some(ViewPurpose::Collections));

    assert!(!resp.uncompressed.is_empty());
    assert!(resp.uncompressed.iter().any(|item| item.name == "folder_a"));
}

#[test]
fn test_move_item_to_view() {
    let dir = TempDir::new().unwrap();
    let storage_root = dir.path();

    let source_dir = storage_root.join("source_folder");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(source_dir.join("sample.txt"), "hello edms").unwrap();

    let res = move_item_to_view(storage_root, &source_dir, ViewPurpose::Collections).unwrap();
    assert!(res.success);

    let target_file = storage_root.join("collections").join("source_folder").join("sample.txt");
    assert!(target_file.exists(), "Moved file must exist in storage/collections/");
}

#[test]
fn test_takeout_item_strips_sqlite_and_handles_collision() {
    let dir = TempDir::new().unwrap();
    let storage_root = dir.path();

    // Setup source folder with SQLite database, WAL, and JSON/markdown files
    let source_folder = storage_root.join("my_collection");
    fs::create_dir_all(&source_folder).unwrap();

    fs::write(source_folder.join("collection.sqlite"), "binary sqlite data").unwrap();
    fs::write(source_folder.join("collection.sqlite-wal"), "wal data").unwrap();
    fs::write(source_folder.join("collection.sqlite-shm"), "shm data").unwrap();
    fs::write(source_folder.join("metadata.json"), r#"{"exported":true}"#).unwrap();
    fs::write(source_folder.join("README.md"), "# Collection Documentation").unwrap();

    // 1. First Takeout run (overwrite: false)
    let res1 = takeout_item(&source_folder, "takeout_export", storage_root, false).unwrap();
    assert!(res1.success);
    assert_eq!(res1.files_copied, 2, "Only JSON and Markdown should be copied");
    assert_eq!(res1.sqlite_files_stripped, 3, "All 3 SQLite files must be stripped");

    let dest_dir = storage_root.join("takeout").join("takeout_export");
    assert!(!dest_dir.join("collection.sqlite").exists(), "SQLite DB must be stripped");
    assert!(!dest_dir.join("collection.sqlite-wal").exists(), "WAL must be stripped");
    assert!(dest_dir.join("metadata.json").exists(), "metadata.json must be preserved");
    assert!(dest_dir.join("README.md").exists(), "README.md must be preserved");

    // 2. Collision test (overwrite: false)
    let res2 = takeout_item(&source_folder, "takeout_export", storage_root, false).unwrap();
    assert!(!res2.success);
    assert!(res2.collision, "Must report collision when destination already exists");

    // 3. Overwrite test (overwrite: true)
    let res3 = takeout_item(&source_folder, "takeout_export", storage_root, true).unwrap();
    assert!(res3.success);
    assert!(!res3.collision);
    assert_eq!(res3.files_copied, 2);
}

#[test]
fn test_size_helpers() {
    assert_eq!(format_size(500), "500 B");
    assert_eq!(format_size(1536), "1.50 KB");
    assert_eq!(format_size(2 * 1024 * 1024), "2.00 MB");
}

#[test]
fn test_check_item_format_and_compute_size() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    let file_path = root.join("test.txt");
    fs::write(&file_path, "12345678").unwrap();

    let size = compute_size(&file_path);
    assert_eq!(size, 8);

    let report = check_item_format(root, ViewPurpose::WebView);
    assert!(report.passed);
}

