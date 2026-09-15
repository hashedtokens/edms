//! Tests for Pre-Purge Orphaned EID Audit Report and Purge Execution

use compute::audit_orphanedEIDs::{
    execute_purge, generate_audit_report, AuditReport,
};
use rusqlite::Connection;
use std::fs::{self, File};
use tempfile::TempDir;

fn setup_test_env() -> (TempDir, String, String) {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    let db_path = root.join("test.db").to_str().unwrap().to_string();
    let conn = Connection::open(&db_path).unwrap();
    edms::schema::initialize_schema(&conn).unwrap();

    let eqp_dir = root.join("storage").join("globalEQPData");
    fs::create_dir_all(&eqp_dir).unwrap();

    (dir, db_path, eqp_dir.to_str().unwrap().to_string())
}

#[test]
fn test_generate_audit_report_detects_disk_and_db_orphans() {
    let (_dir, db_path, eqp_dir) = setup_test_env();
    let conn = Connection::open(&db_path).unwrap();

    // 1. Insert endpoints into DB
    conn.execute(
        "INSERT INTO endpoints (endpoint_id, endpoint_str, method) VALUES ('E0001-AAA', '/active/1', 'GET')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO endpoints (endpoint_id, endpoint_str, method) VALUES ('E0002-AAA', '/active/2', 'POST')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO endpoints (endpoint_id, endpoint_str, method) VALUES ('E0003-AAA', '/orphan/db', 'GET')",
        [],
    ).unwrap();

    // 2. Insert bookmarks only for E0001 and E0002 (E0003 has no bookmark -> DB orphan)
    conn.execute(
        "INSERT INTO bookmarks (endpoint_id, folder) VALUES ('E0001-AAA', 'col-1')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO bookmarks (endpoint_id, folder) VALUES ('E0002-AAA', 'col-1')",
        [],
    ).unwrap();
    drop(conn);

    // 3. Create disk folders in globalEQPData
    let eqp_path = std::path::Path::new(&eqp_dir);
    fs::create_dir_all(eqp_path.join("E0001-AAA")).unwrap();
    fs::create_dir_all(eqp_path.join("E0002-AAA")).unwrap();
    fs::create_dir_all(eqp_path.join("E9999-ZZZ")).unwrap(); // Disk orphan (not in DB)

    let report_path = eqp_path.parent().unwrap().join("audit.json");
    let report = generate_audit_report(
        std::path::Path::new(&db_path),
        eqp_path,
        &report_path,
    ).unwrap();

    // Verify report contents
    assert_eq!(report.total_disk_eids, 3);
    assert_eq!(report.total_db_endpoints, 3);
    assert_eq!(report.total_db_bookmarks, 2);
    assert_eq!(report.orphaned_disk_eids, vec!["E9999-ZZZ"]);
    assert_eq!(report.orphaned_db_endpoints, vec!["E0003-AAA"]);

    // Verify report JSON file was written
    assert!(report_path.exists());
    let file = File::open(&report_path).unwrap();
    let parsed: AuditReport = serde_json::from_reader(file).unwrap();
    assert_eq!(parsed.orphaned_disk_eids, vec!["E9999-ZZZ"]);
}

#[test]
fn test_execute_purge_cleans_disk_folders_and_db_rows() {
    let (_dir, db_path, eqp_dir) = setup_test_env();
    let conn = Connection::open(&db_path).unwrap();

    conn.execute(
        "INSERT INTO endpoints (endpoint_id, endpoint_str, method) VALUES ('E0001-AAA', '/keep', 'GET')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO endpoints (endpoint_id, endpoint_str, method) VALUES ('E0002-AAA', '/purge', 'GET')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO tags (endpoint_id, tag) VALUES ('E0002-AAA', 'obsolete')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO bookmarks (endpoint_id, folder) VALUES ('E0001-AAA', 'col-1')",
        [],
    ).unwrap();
    drop(conn);

    let eqp_path = std::path::Path::new(&eqp_dir);
    let keep_dir = eqp_path.join("E0001-AAA");
    let orphan_dir = eqp_path.join("E8888-ZZZ");
    fs::create_dir_all(&keep_dir).unwrap();
    fs::create_dir_all(&orphan_dir).unwrap();

    let report_path = eqp_path.parent().unwrap().join("audit.json");
    let report = generate_audit_report(
        std::path::Path::new(&db_path),
        eqp_path,
        &report_path,
    ).unwrap();

    let purge_res = execute_purge(
        std::path::Path::new(&db_path),
        eqp_path,
        &report,
    ).unwrap();

    assert!(purge_res.success);
    assert_eq!(purge_res.disk_eids_purged, 1);
    assert_eq!(purge_res.db_endpoints_purged, 1);
    assert_eq!(purge_res.db_tags_purged, 1);

    // Verify disk state
    assert!(keep_dir.exists(), "Active directory must be retained");
    assert!(!orphan_dir.exists(), "Orphaned disk directory must be purged");

    // Verify DB state
    let conn = Connection::open(&db_path).unwrap();
    let count_keep: i64 = conn.query_row(
        "SELECT COUNT(*) FROM endpoints WHERE endpoint_id = 'E0001-AAA'",
        [],
        |r| r.get(0),
    ).unwrap();
    let count_purged: i64 = conn.query_row(
        "SELECT COUNT(*) FROM endpoints WHERE endpoint_id = 'E0002-AAA'",
        [],
        |r| r.get(0),
    ).unwrap();
    let tag_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tags WHERE endpoint_id = 'E0002-AAA'",
        [],
        |r| r.get(0),
    ).unwrap();

    assert_eq!(count_keep, 1);
    assert_eq!(count_purged, 0, "Purged endpoint must be deleted from DB");
    assert_eq!(tag_count, 0, "Purged tags must be deleted from DB");
}
