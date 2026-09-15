//! Tests for URL / Address Prefix Stripper

use compute::remove_url::{
    apply_remove_url_prefix, strip_url_prefix, RemoveUrlRequest, TargetSelection,
};
use rusqlite::Connection;
use tempfile::TempDir;

// ── 1. Unit Tests for strip_url_prefix ────────────────────────────────────────

#[test]
fn test_strip_url_prefix_http_localhost_port() {
    assert_eq!(
        strip_url_prefix("http://localhost:8080/api/v1/users"),
        "/api/v1/users"
    );
}

#[test]
fn test_strip_url_prefix_https_domain_query_fragment() {
    assert_eq!(
        strip_url_prefix("https://api.example.com/items?page=1&limit=50#section"),
        "/items?page=1&limit=50#section"
    );
}

#[test]
fn test_strip_url_prefix_no_scheme_host_port() {
    assert_eq!(strip_url_prefix("localhost:3000/users"), "/users");
    assert_eq!(strip_url_prefix("127.0.0.1:8000/health"), "/health");
}

#[test]
fn test_strip_url_prefix_no_path() {
    assert_eq!(strip_url_prefix("http://localhost:8080"), "/");
    assert_eq!(strip_url_prefix("localhost:8080"), "/");
    assert_eq!(strip_url_prefix("https://example.com"), "/");
}

#[test]
fn test_strip_url_prefix_query_only() {
    assert_eq!(strip_url_prefix("localhost:8080?q=test"), "/?q=test");
    assert_eq!(strip_url_prefix("http://127.0.0.1:8000?q=test"), "/?q=test");
}

#[test]
fn test_strip_url_prefix_already_relative() {
    assert_eq!(strip_url_prefix("/api/v1/users"), "/api/v1/users");
    assert_eq!(strip_url_prefix("/health?ping=1"), "/health?ping=1");
}

#[test]
fn test_strip_url_prefix_empty() {
    assert_eq!(strip_url_prefix(""), "");
    assert_eq!(strip_url_prefix("   "), "");
}

#[test]
fn test_strip_url_prefix_websocket_schemes() {
    assert_eq!(strip_url_prefix("ws://localhost:8080/ws/feed"), "/ws/feed");
    assert_eq!(
        strip_url_prefix("wss://realtime.example.com:9000/socket"),
        "/socket"
    );
}

// ── 2. Database Integration Tests ────────────────────────────────────────────

fn setup_test_db() -> (TempDir, String) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db").to_str().unwrap().to_string();
    let conn = Connection::open(&db_path).unwrap();

    // Use schema initialization from edms
    edms::schema::initialize_schema(&conn).unwrap();

    (dir, db_path)
}

fn insert_endpoint(conn: &Connection, eid: &str, url: &str, method: &str) {
    conn.execute(
        "INSERT INTO endpoints (endpoint_id, endpoint_str, method) VALUES (?, ?, ?)",
        rusqlite::params![eid, url, method],
    )
    .unwrap();
}

fn add_tag(conn: &Connection, eid: &str, tag: &str) {
    conn.execute(
        "INSERT OR IGNORE INTO tags (endpoint_id, tag) VALUES (?, ?)",
        rusqlite::params![eid, tag],
    )
    .unwrap();
}

fn get_endpoint_str(conn: &Connection, eid: &str) -> String {
    conn.query_row(
        "SELECT endpoint_str FROM endpoints WHERE endpoint_id = ?",
        [eid],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn test_apply_remove_url_prefix_selected_endpoints() {
    let (_dir, db_path) = setup_test_db();
    let conn = Connection::open(&db_path).unwrap();

    insert_endpoint(&conn, "E0001-AAA", "http://localhost:8080/api/users", "GET");
    insert_endpoint(&conn, "E0002-AAA", "https://api.external.com:9000/api/orders", "POST");
    drop(conn);

    let summary = apply_remove_url_prefix(RemoveUrlRequest {
        db_path: db_path.clone(),
        target: TargetSelection::SelectedEndpoints(vec!["E0001-AAA".to_string()]),
    })
    .unwrap();

    assert_eq!(summary.mode, "SelectedEndpoints");
    assert_eq!(summary.total_scanned, 1);
    assert_eq!(summary.updated_count, 1);
    assert_eq!(summary.unchanged_count, 0);

    let conn = Connection::open(&db_path).unwrap();
    assert_eq!(get_endpoint_str(&conn, "E0001-AAA"), "/api/users");
    assert_eq!(
        get_endpoint_str(&conn, "E0002-AAA"),
        "https://api.external.com:9000/api/orders"
    );
}

#[test]
fn test_apply_remove_url_prefix_selected_tags() {
    let (_dir, db_path) = setup_test_db();
    let conn = Connection::open(&db_path).unwrap();

    insert_endpoint(&conn, "E0001-AAA", "http://localhost:8080/api/users", "GET");
    insert_endpoint(&conn, "E0002-AAA", "https://api.external.com:9000/api/orders", "POST");
    insert_endpoint(&conn, "E0003-AAA", "http://127.0.0.1:5000/admin/panel", "GET");

    add_tag(&conn, "E0001-AAA", "service-a");
    add_tag(&conn, "E0002-AAA", "service-b");
    add_tag(&conn, "E0003-AAA", "service-a");
    drop(conn);

    let summary = apply_remove_url_prefix(RemoveUrlRequest {
        db_path: db_path.clone(),
        target: TargetSelection::SelectedTags(vec!["service-a".to_string()]),
    })
    .unwrap();

    assert_eq!(summary.mode, "SelectedTags");
    assert_eq!(summary.total_scanned, 2);
    assert_eq!(summary.updated_count, 2);
    assert_eq!(summary.unchanged_count, 0);

    let conn = Connection::open(&db_path).unwrap();
    assert_eq!(get_endpoint_str(&conn, "E0001-AAA"), "/api/users");
    assert_eq!(
        get_endpoint_str(&conn, "E0002-AAA"),
        "https://api.external.com:9000/api/orders"
    );
    assert_eq!(get_endpoint_str(&conn, "E0003-AAA"), "/admin/panel");
}

#[test]
fn test_apply_remove_url_prefix_all_and_idempotency() {
    let (_dir, db_path) = setup_test_db();
    let conn = Connection::open(&db_path).unwrap();

    insert_endpoint(&conn, "E0001-AAA", "http://localhost:8080/api/users", "GET");
    insert_endpoint(&conn, "E0002-AAA", "https://api.external.com:9000/api/orders", "POST");
    insert_endpoint(&conn, "E0003-AAA", "/already/clean", "GET");
    drop(conn);

    // First run with All
    let summary1 = apply_remove_url_prefix(RemoveUrlRequest {
        db_path: db_path.clone(),
        target: TargetSelection::All,
    })
    .unwrap();

    assert_eq!(summary1.mode, "All");
    assert_eq!(summary1.total_scanned, 3);
    assert_eq!(summary1.updated_count, 2);
    assert_eq!(summary1.unchanged_count, 1);

    let conn = Connection::open(&db_path).unwrap();
    assert_eq!(get_endpoint_str(&conn, "E0001-AAA"), "/api/users");
    assert_eq!(get_endpoint_str(&conn, "E0002-AAA"), "/api/orders");
    assert_eq!(get_endpoint_str(&conn, "E0003-AAA"), "/already/clean");
    drop(conn);

    // Second run with All (must be idempotent: 0 updated, 3 unchanged)
    let summary2 = apply_remove_url_prefix(RemoveUrlRequest {
        db_path: db_path.clone(),
        target: TargetSelection::All,
    })
    .unwrap();

    assert_eq!(summary2.total_scanned, 3);
    assert_eq!(summary2.updated_count, 0);
    assert_eq!(summary2.unchanged_count, 3);
}
