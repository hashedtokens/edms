//! Integration + unit tests for the merge operations system.
//!
//! Tests run against real SQLite files (tempfile). Every test that involves
//! the merge coordinator asserts DB state *before and after* the operation —
//! not just that the function returned Ok.

use edms::schema::initialize_schema;
use rusqlite::Connection;
use std::collections::HashSet;
use tempfile::TempDir;

// ── test helpers ────────────────────────────────────────────────────────────

/// Open a temp SQLite file, run initialize_schema, return (dir, path).
/// `dir` must be kept alive for the duration of the test.
fn setup_db() -> (TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db").to_str().unwrap().to_string();
    let conn = Connection::open(&path).unwrap();
    initialize_schema(&conn).unwrap();
    (dir, path)
}

fn raw(path: &str) -> Connection {
    let conn = Connection::open(path).unwrap();
    conn.execute("PRAGMA foreign_keys = OFF", []).unwrap();
    conn
}

fn insert_ep(conn: &Connection, eid: &str, url: &str, method: Option<&str>) {
    conn.execute(
        "INSERT OR IGNORE INTO endpoints (endpoint_id, endpoint_str, method) VALUES (?, ?, ?)",
        rusqlite::params![eid, url, method],
    )
    .unwrap();
}

fn insert_bm(conn: &Connection, eid: &str, folder: &str) {
    conn.execute(
        "INSERT OR IGNORE INTO bookmarks (endpoint_id, folder) VALUES (?, ?)",
        rusqlite::params![eid, folder],
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

fn add_ct(conn: &Connection, collection: &str, tag: &str) {
    conn.execute(
        "INSERT OR IGNORE INTO collection_tag_memberships (collection_name, tagname) VALUES (?, ?)",
        rusqlite::params![collection, tag],
    )
    .unwrap();
}

fn tags_for(conn: &Connection, eid: &str) -> HashSet<String> {
    let mut stmt = conn
        .prepare("SELECT tag FROM tags WHERE endpoint_id = ?")
        .unwrap();
    stmt.query_map([eid], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}

fn bookmarks_for(conn: &Connection, folder: &str) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT endpoint_id FROM bookmarks WHERE folder = ? ORDER BY endpoint_id")
        .unwrap();
    stmt.query_map([folder], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}

fn ct_tags_for(conn: &Connection, collection: &str) -> HashSet<String> {
    let mut stmt = conn
        .prepare(
            "SELECT tagname FROM collection_tag_memberships WHERE collection_name = ?",
        )
        .unwrap();
    stmt.query_map([collection], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}

fn endpoint_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM endpoints", [], |r| r.get(0))
        .unwrap()
}

fn bookmark_count(conn: &Connection, folder: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM bookmarks WHERE folder = ?",
        [folder],
        |r| r.get(0),
    )
    .unwrap()
}

fn eid_for_url_method(conn: &Connection, url: &str, method: Option<&str>) -> Option<String> {
    conn.query_row(
        "SELECT endpoint_id FROM endpoints WHERE endpoint_str = ? AND COALESCE(method,'') = ?",
        rusqlite::params![url, method.unwrap_or("")],
        |r| r.get(0),
    )
    .ok()
}

fn run_merge(
    db_path: &str,
    target: &str,
    sources: Vec<(&str, Vec<&str>)>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use compute::tagops::{merge::merge_by_tags_coordinator, types::{ActivityEntry, CollectionTagFilter}};
    let mut log: Vec<ActivityEntry> = Vec::new();
    let srcs: Vec<CollectionTagFilter> = sources
        .into_iter()
        .map(|(cid, tags)| CollectionTagFilter {
            collection_id: cid.to_string(),
            tags: tags.into_iter().map(|t| t.to_string()).collect(),
        })
        .collect();
    merge_by_tags_coordinator(db_path, target, &srcs, &mut log)
}

// ── 1. net-new endpoint ──────────────────────────────────────────────────────

#[test]
fn test_net_new_endpoint_created() {
    let (_dir, path) = setup_db();
    let conn = raw(&path);
    insert_ep(&conn, "src-1", "https://api.example.com/users", Some("GET"));
    insert_bm(&conn, "src-1", "src-col");
    add_tag(&conn, "src-1", "api");

    let ep_before = endpoint_count(&conn);
    drop(conn);

    run_merge(&path, "dst-col", vec![("src-col", vec![])]).unwrap();

    let conn = raw(&path);
    assert_eq!(endpoint_count(&conn), ep_before + 1);
    assert_eq!(bookmark_count(&conn, "dst-col"), 1);

    // The new endpoint in dst-col must have the tag
    let bms = bookmarks_for(&conn, "dst-col");
    assert_eq!(bms.len(), 1);
    let tags = tags_for(&conn, &bms[0]);
    assert!(tags.contains("api"), "tag must be copied to net-new endpoint");
}

// ── 2. matched endpoint — tag union, same endpoint_id retained ───────────────

#[test]
fn test_matched_endpoint_tag_union() {
    let (_dir, path) = setup_db();
    let conn = raw(&path);

    // Source has GET /users with tag "api"
    insert_ep(&conn, "src-ep", "https://api.example.com/users", Some("GET"));
    insert_bm(&conn, "src-ep", "src-col");
    add_tag(&conn, "src-ep", "api");

    // Target already has GET /users with tag "prod" under dst-col
    insert_ep(&conn, "dst-ep", "https://api.example.com/users", Some("GET"));
    insert_bm(&conn, "dst-ep", "dst-col");
    add_tag(&conn, "dst-ep", "prod");

    let ep_before = endpoint_count(&conn);
    drop(conn);

    run_merge(&path, "dst-col", vec![("src-col", vec![])]).unwrap();

    let conn = raw(&path);
    // No new endpoint created
    assert_eq!(endpoint_count(&conn), ep_before);
    // dst-ep must now carry both tags
    let tags = tags_for(&conn, "dst-ep");
    assert!(tags.contains("api"), "source tag must be unioned in");
    assert!(tags.contains("prod"), "target tag must be preserved");
    // src-ep must not appear in dst-col
    let bms: Vec<String> = bookmarks_for(&conn, "dst-col");
    assert!(bms.contains(&"dst-ep".to_string()));
    assert!(!bms.contains(&"src-ep".to_string()));
}

// ── 3. GET vs PUT on same URL → two distinct logical endpoints ───────────────

#[test]
fn test_get_and_put_same_url_are_distinct() {
    let (_dir, path) = setup_db();
    let conn = raw(&path);

    insert_ep(&conn, "src-get", "https://api.example.com/items", Some("GET"));
    insert_ep(&conn, "src-put", "https://api.example.com/items", Some("PUT"));
    insert_bm(&conn, "src-get", "src-col");
    insert_bm(&conn, "src-put", "src-col");
    add_tag(&conn, "src-get", "read");
    add_tag(&conn, "src-put", "write");

    let ep_before = endpoint_count(&conn);
    drop(conn);

    run_merge(&path, "dst-col", vec![("src-col", vec![])]).unwrap();

    let conn = raw(&path);
    // Both must be created net-new
    assert_eq!(endpoint_count(&conn), ep_before + 2);
    assert_eq!(bookmark_count(&conn, "dst-col"), 2);

    let get_eid = eid_for_url_method(&conn, "https://api.example.com/items", Some("GET")).unwrap();
    let put_eid = eid_for_url_method(&conn, "https://api.example.com/items", Some("PUT")).unwrap();
    assert_ne!(get_eid, put_eid);
    assert!(tags_for(&conn, &get_eid).contains("read"));
    assert!(tags_for(&conn, &put_eid).contains("write"));
}

// ── 4. NULL method normalisation ─────────────────────────────────────────────

#[test]
fn test_null_method_matches_null_method() {
    let (_dir, path) = setup_db();
    let conn = raw(&path);

    insert_ep(&conn, "src-n", "https://api.example.com/ping", None);
    insert_bm(&conn, "src-n", "src-col");
    // dst already has the same URL with NULL method
    insert_ep(&conn, "dst-n", "https://api.example.com/ping", None);
    insert_bm(&conn, "dst-n", "dst-col");
    add_tag(&conn, "dst-n", "health");

    let ep_before = endpoint_count(&conn);
    drop(conn);

    run_merge(&path, "dst-col", vec![("src-col", vec![])]).unwrap();

    let conn = raw(&path);
    // Should MATCH, not create a new endpoint
    assert_eq!(endpoint_count(&conn), ep_before);
}

// ── 5. Source deduplication ───────────────────────────────────────────────────

#[test]
fn test_duplicate_source_logical_endpoints_deduped() {
    let (_dir, path) = setup_db();
    let conn = raw(&path);

    // Two EIDs with the same logical identity in source
    insert_ep(&conn, "src-a", "https://api.example.com/dupe", Some("GET"));
    insert_ep(&conn, "src-b", "https://api.example.com/dupe", Some("GET"));
    insert_bm(&conn, "src-a", "src-col");
    insert_bm(&conn, "src-b", "src-col");
    add_tag(&conn, "src-a", "tag-a");
    add_tag(&conn, "src-b", "tag-b");

    let ep_before = endpoint_count(&conn);
    drop(conn);

    run_merge(&path, "dst-col", vec![("src-col", vec![])]).unwrap();

    let conn = raw(&path);
    // Only one logical endpoint should appear in target
    assert_eq!(bookmark_count(&conn, "dst-col"), 1);
    // One new endpoint (net-new)
    assert_eq!(endpoint_count(&conn), ep_before + 1);
}

// ── 6. Bookmark uniqueness — idempotent insert ────────────────────────────────

#[test]
fn test_bookmark_insert_is_idempotent() {
    let (_dir, path) = setup_db();
    let conn = raw(&path);

    insert_ep(&conn, "ep-1", "https://api.example.com/x", Some("GET"));
    insert_bm(&conn, "ep-1", "src-col");

    // dst already has this endpoint bookmarked
    insert_ep(&conn, "dst-1", "https://api.example.com/x", Some("GET"));
    insert_bm(&conn, "dst-1", "dst-col");
    drop(conn);

    run_merge(&path, "dst-col", vec![("src-col", vec![])]).unwrap();

    let conn = raw(&path);
    // Still exactly one bookmark in dst-col for this endpoint
    assert_eq!(bookmark_count(&conn, "dst-col"), 1);
}

// ── 7. Merge idempotency ───────────────────────────────────────────────────────

#[test]
fn test_merge_idempotency() {
    let (_dir, path) = setup_db();
    let conn = raw(&path);

    insert_ep(&conn, "src-1", "https://api.example.com/a", Some("GET"));
    insert_ep(&conn, "src-2", "https://api.example.com/b", Some("POST"));
    insert_bm(&conn, "src-1", "src-col");
    insert_bm(&conn, "src-2", "src-col");
    add_tag(&conn, "src-1", "api");
    add_tag(&conn, "src-2", "write");
    add_ct(&conn, "src-col", "production");
    drop(conn);

    run_merge(&path, "dst-col", vec![("src-col", vec![])]).unwrap();

    let conn = raw(&path);
    let ep_count_1 = endpoint_count(&conn);
    let bm_count_1 = bookmark_count(&conn, "dst-col");
    let ct_1 = ct_tags_for(&conn, "dst-col");
    drop(conn);

    // Second identical merge
    run_merge(&path, "dst-col", vec![("src-col", vec![])]).unwrap();

    let conn = raw(&path);
    assert_eq!(endpoint_count(&conn), ep_count_1, "idempotent: no new endpoints");
    assert_eq!(bookmark_count(&conn, "dst-col"), bm_count_1, "idempotent: no new bookmarks");
    assert_eq!(ct_tags_for(&conn, "dst-col"), ct_1, "idempotent: no new CT tags");
}

// ── 8. Self-merge rejected ────────────────────────────────────────────────────

#[test]
fn test_self_merge_rejected() {
    let (_dir, path) = setup_db();
    let result = run_merge(&path, "col-a", vec![("col-a", vec![])]);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("Self-merge"), "error must mention self-merge, got: {msg}");
}

// ── 9. Empty sources rejected ────────────────────────────────────────────────

#[test]
fn test_empty_sources_rejected() {
    let (_dir, path) = setup_db();
    let result = run_merge(&path, "dst", vec![]);
    assert!(result.is_err());
}

// ── 10. Filtered merge: C(T) must NOT change ──────────────────────────────────

#[test]
fn test_filtered_merge_does_not_touch_ct() {
    let (_dir, path) = setup_db();
    let conn = raw(&path);

    insert_ep(&conn, "s1", "https://api.example.com/users", Some("GET"));
    insert_bm(&conn, "s1", "src-col");
    add_tag(&conn, "s1", "api");

    // Source collection has C(T) tags
    add_ct(&conn, "src-col", "production");
    add_ct(&conn, "src-col", "backend");

    // Target already has a C(T) tag
    add_ct(&conn, "dst-col", "internal");
    drop(conn);

    // Filtered merge: tags = ["api"]
    run_merge(&path, "dst-col", vec![("src-col", vec!["api"])]).unwrap();

    let conn = raw(&path);
    let ct = ct_tags_for(&conn, "dst-col");
    // Target C(T) must be unchanged: only "internal"
    assert_eq!(ct, HashSet::from(["internal".to_string()]),
        "filtered merge must not modify target C(T)");
}

// ── 11. Whole-folder merge: C(T) IS merged ────────────────────────────────────

#[test]
fn test_whole_folder_merge_includes_ct() {
    let (_dir, path) = setup_db();
    let conn = raw(&path);

    insert_ep(&conn, "s1", "https://api.example.com/a", Some("GET"));
    insert_bm(&conn, "s1", "src-col");

    add_ct(&conn, "src-col", "api");
    add_ct(&conn, "src-col", "production");
    add_ct(&conn, "dst-col", "production");
    add_ct(&conn, "dst-col", "internal");
    drop(conn);

    // Whole-folder merge: tags = []
    run_merge(&path, "dst-col", vec![("src-col", vec![])]).unwrap();

    let conn = raw(&path);
    let ct = ct_tags_for(&conn, "dst-col");
    assert_eq!(
        ct,
        HashSet::from([
            "api".to_string(),
            "production".to_string(),
            "internal".to_string()
        ]),
        "whole-folder merge must union source C(T) into target"
    );
}

// ── 12. Target-only C(T) tags preserved after whole-folder merge ──────────────

#[test]
fn test_whole_folder_merge_preserves_target_only_ct() {
    let (_dir, path) = setup_db();
    let conn = raw(&path);

    insert_ep(&conn, "s1", "https://api.example.com/b", Some("GET"));
    insert_bm(&conn, "s1", "src-col");
    add_ct(&conn, "src-col", "src-only");
    add_ct(&conn, "dst-col", "dst-only");
    drop(conn);

    run_merge(&path, "dst-col", vec![("src-col", vec![])]).unwrap();

    let conn = raw(&path);
    let ct = ct_tags_for(&conn, "dst-col");
    assert!(ct.contains("dst-only"), "target-only CT tag must be preserved");
    assert!(ct.contains("src-only"), "source CT tag must be added");
}

// ── 13. C(T) idempotency: repeated whole-folder merge same result ─────────────

#[test]
fn test_ct_idempotency() {
    let (_dir, path) = setup_db();
    let conn = raw(&path);

    insert_ep(&conn, "s1", "https://api.example.com/c", Some("GET"));
    insert_bm(&conn, "s1", "src-col");
    add_ct(&conn, "src-col", "tag-x");
    drop(conn);

    run_merge(&path, "dst-col", vec![("src-col", vec![])]).unwrap();
    let ct_after_first = ct_tags_for(&raw(&path), "dst-col");

    run_merge(&path, "dst-col", vec![("src-col", vec![])]).unwrap();
    let ct_after_second = ct_tags_for(&raw(&path), "dst-col");

    assert_eq!(ct_after_first, ct_after_second, "CT merge must be idempotent");
}

// ── 14. C(T) add / remove / list ─────────────────────────────────────────────

#[test]
fn test_ct_add_remove_list() {
    use compute::tagops::collection_tags::{
        add_collection_tag, list_collection_tags, remove_collection_tag,
    };
    use compute::tagops::db::open_core;

    let (_dir, path) = setup_db();
    let (core, queries) = open_core(&path).unwrap();

    add_collection_tag(&core, &queries, "col-a", "prod").unwrap();
    add_collection_tag(&core, &queries, "col-a", "v2").unwrap();

    let tags = list_collection_tags(&core, &queries, "col-a").unwrap();
    assert!(tags.contains(&"prod".to_string()));
    assert!(tags.contains(&"v2".to_string()));

    remove_collection_tag(&core, &queries, "col-a", "prod").unwrap();
    let tags = list_collection_tags(&core, &queries, "col-a").unwrap();
    assert!(!tags.contains(&"prod".to_string()));
    assert!(tags.contains(&"v2".to_string()));
}

// ── 15. Duplicate C(T) insert is ignored ─────────────────────────────────────

#[test]
fn test_ct_duplicate_insert_ignored() {
    use compute::tagops::collection_tags::{add_collection_tag, list_collection_tags};
    use compute::tagops::db::open_core;

    let (_dir, path) = setup_db();
    let (core, queries) = open_core(&path).unwrap();

    add_collection_tag(&core, &queries, "col-a", "tag").unwrap();
    add_collection_tag(&core, &queries, "col-a", "tag").unwrap(); // duplicate

    let tags = list_collection_tags(&core, &queries, "col-a").unwrap();
    assert_eq!(tags.iter().filter(|t| *t == "tag").count(), 1);
}

// ── 16. Collections by tag ────────────────────────────────────────────────────

#[test]
fn test_collections_by_tag() {
    use compute::tagops::collection_tags::{add_collection_tag, collections_with_tag};
    use compute::tagops::db::open_core;

    let (_dir, path) = setup_db();
    let (core, queries) = open_core(&path).unwrap();

    add_collection_tag(&core, &queries, "col-a", "backend").unwrap();
    add_collection_tag(&core, &queries, "col-b", "frontend").unwrap();
    add_collection_tag(&core, &queries, "col-c", "backend").unwrap();

    let cols = collections_with_tag(&core, &queries, "backend").unwrap();
    let col_set: HashSet<String> = cols.into_iter().collect();
    assert!(col_set.contains("col-a"));
    assert!(col_set.contains("col-c"));
    assert!(!col_set.contains("col-b"));
}

// ── 17. Endpoint tags and C(T) tags are independent ───────────────────────────

#[test]
fn test_endpoint_tags_and_ct_tags_are_independent() {
    let (_dir, path) = setup_db();
    let conn = raw(&path);

    // Endpoint has tag "api"
    insert_ep(&conn, "ep-1", "https://api.example.com/z", Some("GET"));
    insert_bm(&conn, "ep-1", "src-col");
    add_tag(&conn, "ep-1", "api");

    // Collection has C(T) tag "api" (same name, different table)
    add_ct(&conn, "src-col", "api");
    drop(conn);

    // Verify they exist independently
    let conn = raw(&path);
    let ep_tags = tags_for(&conn, "ep-1");
    let ct = ct_tags_for(&conn, "src-col");
    assert!(ep_tags.contains("api"), "endpoint tag must exist");
    assert!(ct.contains("api"), "CT tag must exist");

    // Now run a filtered merge: C(T) on dst must stay empty
    drop(conn);
    run_merge(&path, "dst-col", vec![("src-col", vec!["api"])]).unwrap();

    let conn = raw(&path);
    assert!(
        ct_tags_for(&conn, "dst-col").is_empty(),
        "filtered merge must not copy C(T) even if tag name matches endpoint tag"
    );
}

// ── 18. An endpoint with no QPs gets no QP rows from a merge ─────────────────

#[test]
fn test_merge_of_an_endpoint_without_qps_creates_no_metadata_rows() {
    let (_dir, path) = setup_db();
    let conn = raw(&path);
    insert_ep(&conn, "src-1", "https://api.example.com/meta", Some("GET"));
    insert_bm(&conn, "src-1", "src-col");
    drop(conn);

    run_merge(&path, "dst-col", vec![("src-col", vec![])]).unwrap();

    let conn = raw(&path);
    let req_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM request_metadata", [], |r| r.get(0))
        .unwrap();
    let res_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM response_metadata", [], |r| r.get(0))
        .unwrap();
    assert_eq!(req_count, 0, "an endpoint with no QPs has no request_metadata rows to copy");
    assert_eq!(res_count, 0, "an endpoint with no QPs has no response_metadata rows to copy");
}

// ── 19. Batch boundary: >500 endpoints → multiple batches ────────────────────

#[test]
fn test_batch_boundary_multiple_batches() {
    let (_dir, path) = setup_db();
    let conn = raw(&path);

    // Insert 505 distinct endpoints in src-col
    for i in 0..505_usize {
        let eid = format!("src-ep-{i}");
        let url = format!("https://api.example.com/endpoint/{i}");
        insert_ep(&conn, &eid, &url, Some("GET"));
        insert_bm(&conn, &eid, "src-col");
    }
    drop(conn);

    run_merge(&path, "dst-col", vec![("src-col", vec![])]).unwrap();

    let conn = raw(&path);
    assert_eq!(
        bookmark_count(&conn, "dst-col"),
        505,
        "all 505 endpoints must appear in dst-col"
    );
}

// ── 20. QueryMap has all merge keys ──────────────────────────────────────────

#[test]
fn test_querymap_merge_keys_exist() {
    use edms::query_loader::QueryMap;
    let q = QueryMap::load();
    for key in &["M_CLASSIFY", "M_UNION_TAGS", "M_INSERT_EP", "M_INSERT_BM",
                 "CT_INSERT", "CT_DELETE", "CT_LIST", "CT_COLLECTIONS_BY_TAG", "CT_MERGE"]
    {
        assert!(q.get_merge_query(key).is_some(), "missing merge query: {key}");
    }
}

// ── 21. Tag union does not duplicate tags ─────────────────────────────────────

#[test]
fn test_tag_union_no_duplicates() {
    let (_dir, path) = setup_db();
    let conn = raw(&path);

    // Both source and target have tag "shared"
    insert_ep(&conn, "src-1", "https://api.example.com/shared", Some("GET"));
    insert_ep(&conn, "dst-1", "https://api.example.com/shared", Some("GET"));
    insert_bm(&conn, "src-1", "src-col");
    insert_bm(&conn, "dst-1", "dst-col");
    add_tag(&conn, "src-1", "shared");
    add_tag(&conn, "dst-1", "shared");
    add_tag(&conn, "src-1", "src-only");
    drop(conn);

    run_merge(&path, "dst-col", vec![("src-col", vec![])]).unwrap();

    let conn = raw(&path);
    let tags = tags_for(&conn, "dst-1");
    assert_eq!(
        tags.iter().filter(|t| *t == "shared").count(),
        1,
        "shared tag must not be duplicated"
    );
    assert!(tags.contains("src-only"), "src-only tag must be unioned");
}

// ── 22. Multi-DB collection files & physical QP filesystem merge ─────────────

#[test]
fn test_multidb_collection_files_and_physical_qp_merge() {
    use compute::eid::is_valid_eid;
    use edms::ops::collection_membership_ops::CollectionMembershipOps;

    let (dir, path) = setup_db();
    let root = dir.path();
    let collections_dir = root.join("storage").join("collections");
    let eqp_dir = root.join("storage").join("globalEQPData");
    std::fs::create_dir_all(&collections_dir).unwrap();
    std::fs::create_dir_all(&eqp_dir).unwrap();

    let conn = raw(&path);

    // EIDs
    let src_shared_eid = "E0001-AAA";
    let src_new_eid = "E0002-AAA";
    let dst_shared_eid = "E0010-AAA";

    // Insert into central endpoints
    insert_ep(&conn, src_shared_eid, "https://api.example.com/data", Some("GET"));
    insert_ep(&conn, src_new_eid, "https://api.example.com/netnew", Some("POST"));
    insert_ep(&conn, dst_shared_eid, "https://api.example.com/data", Some("GET"));

    add_tag(&conn, src_shared_eid, "from-src");
    add_tag(&conn, dst_shared_eid, "from-dst");
    add_tag(&conn, src_new_eid, "brand-new");

    drop(conn);

    // Create dedicated collection SQLite files
    let src_col_file = collections_dir.join("col-src.sqlite");
    let dst_col_file = collections_dir.join("col-dst.sqlite");

    let src_ops = CollectionMembershipOps::new(src_col_file.to_str().unwrap());
    src_ops.initialize().unwrap();
    src_ops.add(src_shared_eid).unwrap();
    src_ops.add(src_new_eid).unwrap();

    let dst_ops = CollectionMembershipOps::new(dst_col_file.to_str().unwrap());
    dst_ops.initialize().unwrap();
    dst_ops.add(dst_shared_eid).unwrap();

    // Setup physical QP files
    let src_shared_dir = eqp_dir.join(src_shared_eid);
    let src_new_dir = eqp_dir.join(src_new_eid);
    let dst_shared_dir = eqp_dir.join(dst_shared_eid);
    std::fs::create_dir_all(&src_shared_dir).unwrap();
    std::fs::create_dir_all(&src_new_dir).unwrap();
    std::fs::create_dir_all(&dst_shared_dir).unwrap();

    // src_shared has request 1
    std::fs::write(src_shared_dir.join(format!("{src_shared_eid}-request-1.json")), r#"{"req":"src-1"}"#).unwrap();
    std::fs::write(src_shared_dir.join(format!("{src_shared_eid}-response-1.json")), r#"{"res":"src-1"}"#).unwrap();
    std::fs::write(src_shared_dir.join(format!("{src_shared_eid}-headers-1.json")), r#"{"head":"src-1"}"#).unwrap();

    // src_new has request 1
    std::fs::write(src_new_dir.join(format!("{src_new_eid}-request-1.json")), r#"{"req":"new-1"}"#).unwrap();

    // dst_shared already has request 1
    std::fs::write(dst_shared_dir.join(format!("{dst_shared_eid}-request-1.json")), r#"{"req":"dst-1"}"#).unwrap();

    // Run Merge of col-src into col-dst
    run_merge(&path, "col-dst", vec![("col-src", vec![])]).unwrap();

    // 1. Assert target collection SQLite file has updated membership
    let dst_members = dst_ops.list_set().unwrap();
    assert_eq!(dst_members.len(), 2, "target collection should have 2 members");
    assert!(dst_members.contains(dst_shared_eid), "matched target EID retained");

    // The second member should be a valid canonical EID (not UUID!)
    let net_new_eid = dst_members.iter().find(|id| *id != dst_shared_eid).unwrap();
    assert!(is_valid_eid(net_new_eid), "net-new EID must be canonical format, got: {net_new_eid}");

    // 2. Assert tags unioned on matched endpoint and copied on net-new
    let conn = raw(&path);
    let dst_tags = tags_for(&conn, dst_shared_eid);
    assert!(dst_tags.contains("from-src"), "source tag unioned into matched target");
    assert!(dst_tags.contains("from-dst"), "existing target tag preserved");

    let new_tags = tags_for(&conn, net_new_eid);
    assert!(new_tags.contains("brand-new"), "net-new endpoint has source tags");

    // 3. Assert physical QP files merged and re-indexed
    // Matched endpoint E0010-AAA should now have request-1 AND request-2 (copied from E0001-AAA with index 2)
    assert!(dst_shared_dir.join(format!("{dst_shared_eid}-request-1.json")).exists());
    assert!(dst_shared_dir.join(format!("{dst_shared_eid}-request-2.json")).exists(),
        "source QP file must be copied and re-indexed to request-2");
    assert!(dst_shared_dir.join(format!("{dst_shared_eid}-response-2.json")).exists());
    assert!(dst_shared_dir.join(format!("{dst_shared_eid}-headers-2.json")).exists());

    // Net-new endpoint should have its physical QP dir created with request-1
    let net_new_dir = eqp_dir.join(net_new_eid);
    assert!(net_new_dir.exists(), "physical QP directory for net-new EID must be created");
    assert!(net_new_dir.join(format!("{net_new_eid}-request-1.json")).exists(),
        "QP file for net-new endpoint must be renamed to match new EID");
}
