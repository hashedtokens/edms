//! Duplicating a collection (`run_collection_duplicate`): a new collection
//! that is a full, independent copy of an existing one.
//!
//! Every test keeps the database file OUTSIDE the storage root, the way Docker
//! has it (`/app/data/edms.db` vs `/app/edms_root`).

use compute::eid::allocator::EidAllocator;
use compute::tagops::collection_duplicate::{run_collection_duplicate, CollectionDuplicateRequest};
use edms::{ops::collection_membership_ops::CollectionMembershipOps, schema::initialize_schema};
use rusqlite::Connection;
use std::{collections::BTreeSet, path::PathBuf};
use tempfile::TempDir;

struct World {
    _dir: TempDir,
    db: String,
    root: PathBuf,
}

impl World {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().join("data");
        let root = dir.path().join("root");
        std::fs::create_dir_all(&data_dir).unwrap();
        std::fs::create_dir_all(root.join("storage").join("collections")).unwrap();
        std::fs::create_dir_all(root.join("storage").join("globalEQPData")).unwrap();
        let db = data_dir.join("edms.db").to_str().unwrap().to_string();
        initialize_schema(&Connection::open(&db).unwrap()).unwrap();
        World { _dir: dir, db, root }
    }

    fn conn(&self) -> Connection {
        Connection::open(&self.db).unwrap()
    }

    fn collection_file(&self, name: &str) -> PathBuf {
        self.root.join("storage").join("collections").join(format!("{name}.sqlite"))
    }

    fn qp_dir(&self, eid: &str) -> PathBuf {
        self.root.join("storage").join("globalEQPData").join(eid)
    }

    /// An endpoint with `qps` saved QPs: files, request and response rows.
    fn endpoint(&self, eid: &str, url: &str, method: &str, annotation: Option<&str>, tags: &[&str], qps: i64) {
        let c = self.conn();
        c.execute(
            "INSERT INTO endpoints (endpoint_id, endpoint_str, method, annotation) VALUES (?, ?, ?, ?)",
            rusqlite::params![eid, url, method, annotation],
        )
        .unwrap();
        for t in tags {
            c.execute("INSERT INTO tags (endpoint_id, tag) VALUES (?, ?)", rusqlite::params![eid, t]).unwrap();
        }
        let d = self.qp_dir(eid);
        std::fs::create_dir_all(&d).unwrap();
        for n in 1..=qps {
            for kind in ["request", "response", "headers"] {
                std::fs::write(d.join(format!("{eid}-{kind}-{n}.json")), format!(r#"{{"{kind}":{n},"of":"{eid}"}}"#)).unwrap();
            }
            c.execute(
                "INSERT INTO request_metadata (endpoint_id, request_number, file_path, method, timestamp) VALUES (?, ?, ?, ?, ?)",
                rusqlite::params![eid, n, d.join(format!("{eid}-request-{n}.json")).display().to_string(), method, format!("2026-11-0{n} 10:00:00")],
            )
            .unwrap();
            c.execute(
                "INSERT INTO response_metadata (endpoint_id, request_number, file_path, status_code, exit_code, response_time_ms, timestamp) VALUES (?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![eid, n, d.join(format!("{eid}-response-{n}.json")).display().to_string(), 200 + n, 0, 10 * n, format!("2026-11-0{n} 10:00:01")],
            )
            .unwrap();
        }
        // the EIDs of a real database are all known to the allocator
        let alloc = EidAllocator::new(&self.db);
        alloc.initialize().unwrap();
        alloc.reserve(eid).unwrap();
    }

    /// A collection the way the app creates one, with its members, the tags
    /// carried in its file, an annotation, collection tags and bookmarks.
    fn collection(&self, name: &str, members: &[&str], annotation: Option<&str>) {
        let file = self.collection_file(name);
        let ops = CollectionMembershipOps::new(file.to_str().unwrap());
        ops.initialize().unwrap();
        for e in members {
            ops.add(e).unwrap();
        }
        ops.set_size_bytes(12345).unwrap();
        let _ = ops.shutdown();
        self.conn()
            .execute(
                "INSERT INTO collections (name, file_path, annotation) VALUES (?, ?, ?)",
                rusqlite::params![name, file.to_str().unwrap(), annotation],
            )
            .unwrap();
    }

    fn bookmark(&self, eid: &str, folder: &str, notes: Option<&str>) {
        self.conn()
            .execute(
                "INSERT INTO bookmarks (endpoint_id, folder, notes, timestamp) VALUES (?, ?, ?, '2026-11-01 09:00:00')",
                rusqlite::params![eid, folder, notes],
            )
            .unwrap();
    }

    fn members(&self, name: &str) -> Vec<String> {
        let ops = CollectionMembershipOps::new(self.collection_file(name).to_str().unwrap());
        ops.initialize().unwrap();
        ops.list_ids().unwrap()
    }

    fn duplicate(&self, source: &str, target: &str) -> Result<serde_json::Value, String> {
        run_collection_duplicate(&CollectionDuplicateRequest {
            db_path: self.db.clone(),
            storage_root: self.root.clone(),
            source_collection_name: source.to_string(),
            target_collection_name: target.to_string(),
        })
    }

    fn count(&self, sql: &str) -> i64 {
        self.conn().query_row(sql, [], |r| r.get(0)).unwrap()
    }
}

/// `src`: E1 (2 QPs, tags, annotation) and E2 (1 QP) are members; E3 is only
/// bookmarked; E99 is listed as a member but its endpoint is gone.
fn source_world() -> World {
    let w = World::new();
    w.endpoint("E0001-AAA", "https://a.test/x", "POST", Some("endpoint one"), &["auth", "smoke"], 2);
    w.endpoint("E0002-AAA", "https://a.test/y", "GET", None, &["other"], 1);
    w.endpoint("E0003-AAA", "https://a.test/z", "PUT", None, &[], 1);
    w.collection("src", &["E0001-AAA", "E0002-AAA", "E0099-AAA"], Some("the source collection"));
    {
        let ops = CollectionMembershipOps::new(w.collection_file("src").to_str().unwrap());
        ops.initialize().unwrap();
        ops.add_tag("E0001-AAA", "carried").unwrap();
        let _ = ops.shutdown();
    }
    w.bookmark("E0001-AAA", "src", Some("note one"));
    w.bookmark("E0002-AAA", "src", None);
    w.bookmark("E0003-AAA", "src", Some("only bookmarked"));
    let c = w.conn();
    c.execute("INSERT INTO collection_tag_memberships (collection_name, tagname) VALUES ('src', 'v1'), ('src', 'v2')", []).unwrap();
    w
}

fn text(p: &PathBuf) -> String {
    std::fs::read_to_string(p).unwrap()
}

#[test]
fn the_copy_has_everything_the_original_has() {
    let w = source_world();
    let out = w.duplicate("src", "src (copy)").unwrap();

    assert_eq!(out["ok"], true);
    assert_eq!(out["endpoints"], 3, "E1, E2 and the bookmarked E3");
    assert_eq!(out["members"], 2, "E1 and E2; E3 was never a member");
    assert_eq!(out["bookmarks"], 3);
    assert_eq!(out["qps"], 4);
    assert_eq!(out["skipped_missing_endpoints"], 1, "E99 has no endpoint to copy");
    assert_eq!(out["collection_tags"], 2);

    // the catalog row, with the annotation, naming the file in the storage root
    let (file_path, annotation): (String, Option<String>) = w
        .conn()
        .query_row("SELECT file_path, annotation FROM collections WHERE name = 'src (copy)'", [], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap();
    assert_eq!(file_path, w.collection_file("src (copy)").to_str().unwrap());
    assert_eq!(annotation.as_deref(), Some("the source collection"));

    // members: two, none of them the original EIDs
    let members = w.members("src (copy)");
    assert_eq!(members.len(), 2);
    let original: BTreeSet<&str> = ["E0001-AAA", "E0002-AAA", "E0003-AAA"].into_iter().collect();
    for m in &members {
        assert!(!original.contains(m.as_str()), "{m} is an original EID, not a copy");
    }

    // each copied endpoint: same URL, method, annotation, tags
    let by_url = |url: &str| -> String {
        w.conn()
            .query_row("SELECT endpoint_id FROM endpoints WHERE endpoint_str = ? AND endpoint_id NOT IN ('E0001-AAA','E0002-AAA','E0003-AAA')", [url], |r| r.get(0))
            .unwrap()
    };
    let (e1, e2, e3) = (by_url("https://a.test/x"), by_url("https://a.test/y"), by_url("https://a.test/z"));
    let (method, ann): (String, Option<String>) = w
        .conn()
        .query_row("SELECT method, annotation FROM endpoints WHERE endpoint_id = ?", [&e1], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap();
    assert_eq!((method.as_str(), ann.as_deref()), ("POST", Some("endpoint one")));
    let tags = |eid: &str| -> BTreeSet<String> {
        let c = w.conn();
        let mut st = c.prepare("SELECT tag FROM tags WHERE endpoint_id = ?").unwrap();
        st.query_map([eid], |r| r.get::<_, String>(0)).unwrap().map(Result::unwrap).collect()
    };
    assert_eq!(tags(&e1), ["auth", "smoke"].iter().map(|s| s.to_string()).collect());
    assert_eq!(tags(&e2), ["other"].iter().map(|s| s.to_string()).collect());
    assert!(tags(&e3).is_empty());

    // the QPs: same numbers, same status codes and timing, files under the new EID with the same content
    let rows: Vec<(i64, Option<i64>, Option<i64>, String)> = {
        let c = w.conn();
        let mut st = c
            .prepare(
                "SELECT r.request_number, p.status_code, p.response_time_ms, r.file_path FROM request_metadata r \
                 JOIN response_metadata p ON p.endpoint_id = r.endpoint_id AND p.request_number = r.request_number \
                 WHERE r.endpoint_id = ? ORDER BY r.request_number",
            )
            .unwrap();
        st.query_map([&e1], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap().map(Result::unwrap).collect()
    };
    assert_eq!(rows.len(), 2, "both QPs of E1 are in the database");
    assert_eq!((rows[0].0, rows[0].1, rows[0].2), (1, Some(201), Some(10)));
    assert_eq!((rows[1].0, rows[1].1, rows[1].2), (2, Some(202), Some(20)));
    assert!(rows[0].3.contains(&e1), "the QP row points at the new EID's file: {}", rows[0].3);
    for n in 1..=2 {
        for kind in ["request", "response", "headers"] {
            let copy = text(&w.qp_dir(&e1).join(format!("{e1}-{kind}-{n}.json")));
            let orig = text(&w.qp_dir("E0001-AAA").join(format!("E0001-AAA-{kind}-{n}.json")));
            assert_eq!(copy, orig, "{kind} {n} is a byte for byte copy");
        }
    }
    assert_eq!(w.count(&format!("SELECT COUNT(*) FROM request_metadata WHERE endpoint_id = '{e2}'")), 1);

    // bookmarks with their notes; E3 is bookmarked but not a member
    let notes = |eid: &str| -> Option<String> {
        w.conn()
            .query_row("SELECT notes FROM bookmarks WHERE endpoint_id = ? AND folder = 'src (copy)'", [eid], |r| r.get(0))
            .unwrap()
    };
    assert_eq!(notes(&e1).as_deref(), Some("note one"));
    assert_eq!(notes(&e3).as_deref(), Some("only bookmarked"));
    assert!(!members.contains(&e3));
    assert_eq!(w.count("SELECT COUNT(*) FROM bookmarks WHERE folder = 'src (copy)'"), 3);

    // collection tags, the tags carried in the file, and the size (members' files only)
    assert_eq!(w.count("SELECT COUNT(*) FROM collection_tag_memberships WHERE collection_name = 'src (copy)'"), 2);
    let ops = CollectionMembershipOps::new(w.collection_file("src (copy)").to_str().unwrap());
    ops.initialize().unwrap();
    let carried = ops.list_all_endpoint_tags().unwrap();
    assert_eq!(carried, vec![(e1.clone(), "carried".to_string())]);
    let expected: u64 = [&e1, &e2]
        .iter()
        .map(|e| std::fs::read_dir(w.qp_dir(e)).unwrap().flatten().map(|f| f.metadata().unwrap().len()).sum::<u64>())
        .sum();
    assert_eq!(ops.size_bytes().unwrap(), Some(expected as i64));
    assert_eq!(out["size_bytes"], expected);
}

#[test]
fn the_original_is_left_exactly_as_it_was() {
    let w = source_world();
    let members_before = w.members("src");
    let counts = |w: &World| {
        (
            w.count("SELECT COUNT(*) FROM request_metadata WHERE endpoint_id IN ('E0001-AAA','E0002-AAA','E0003-AAA')"),
            w.count("SELECT COUNT(*) FROM bookmarks WHERE folder = 'src'"),
            w.count("SELECT COUNT(*) FROM tags WHERE endpoint_id IN ('E0001-AAA','E0002-AAA','E0003-AAA')"),
            w.count("SELECT COUNT(*) FROM collection_tag_memberships WHERE collection_name = 'src'"),
        )
    };
    let before = counts(&w);
    w.duplicate("src", "copy").unwrap();
    assert_eq!(counts(&w), before);
    assert_eq!(w.members("src"), members_before);
    let (annotation, size): (Option<String>, Option<i64>) = {
        let ops = CollectionMembershipOps::new(w.collection_file("src").to_str().unwrap());
        ops.initialize().unwrap();
        (
            w.conn().query_row("SELECT annotation FROM collections WHERE name = 'src'", [], |r| r.get(0)).unwrap(),
            ops.size_bytes().unwrap(),
        )
    };
    assert_eq!(annotation.as_deref(), Some("the source collection"));
    assert_eq!(size, Some(12345), "the original's stored size is untouched");
}

#[test]
fn the_copy_can_be_changed_without_touching_the_original() {
    let w = source_world();
    w.duplicate("src", "copy").unwrap();
    let copy_e1: String = w
        .conn()
        .query_row("SELECT endpoint_id FROM endpoints WHERE endpoint_str = 'https://a.test/x' AND endpoint_id <> 'E0001-AAA'", [], |r| r.get(0))
        .unwrap();
    // remove a QP of the copy the way the app does, rows and files
    w.conn().execute("DELETE FROM request_metadata WHERE endpoint_id = ? AND request_number = 1", [&copy_e1]).unwrap();
    std::fs::remove_file(w.qp_dir(&copy_e1).join(format!("{copy_e1}-request-1.json"))).unwrap();
    assert!(w.qp_dir("E0001-AAA").join("E0001-AAA-request-1.json").is_file(), "the original still has its files");
    assert_eq!(w.count("SELECT COUNT(*) FROM request_metadata WHERE endpoint_id = 'E0001-AAA'"), 2);
}

#[test]
fn an_existing_name_is_refused_and_nothing_is_created() {
    let w = source_world();
    w.collection("taken", &[], None);
    let endpoints_before = w.count("SELECT COUNT(*) FROM endpoints");
    let err = w.duplicate("src", "taken").unwrap_err();
    assert!(err.contains("already exists"), "{err}");
    assert_eq!(w.count("SELECT COUNT(*) FROM endpoints"), endpoints_before);
    assert_eq!(w.count("SELECT COUNT(*) FROM collections"), 2);
}

#[test]
fn a_stray_file_with_the_new_name_is_refused_too() {
    let w = source_world();
    std::fs::write(w.collection_file("leftover"), b"not a collection").unwrap();
    let err = w.duplicate("src", "leftover").unwrap_err();
    assert!(err.contains("already exists"), "{err}");
    assert_eq!(std::fs::read(w.collection_file("leftover")).unwrap(), b"not a collection", "it was not overwritten");
}

#[test]
fn a_missing_source_is_an_error() {
    let w = World::new();
    let err = w.duplicate("nope", "copy").unwrap_err();
    assert!(err.contains("does not exist"), "{err}");
    assert!(!w.collection_file("copy").exists());
}

#[test]
fn a_collection_cannot_be_duplicated_onto_itself() {
    let w = source_world();
    assert!(w.duplicate("src", "src").is_err());
}

#[test]
fn an_empty_collection_duplicates_to_an_empty_collection_with_its_annotation() {
    let w = World::new();
    w.collection("empty", &[], Some("nothing in here"));
    let out = w.duplicate("empty", "empty (copy)").unwrap();
    assert_eq!(out["endpoints"], 0);
    assert!(w.collection_file("empty (copy)").is_file());
    let annotation: Option<String> = w
        .conn()
        .query_row("SELECT annotation FROM collections WHERE name = 'empty (copy)'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(annotation.as_deref(), Some("nothing in here"));
}

#[test]
fn endpoints_that_share_a_url_and_method_stay_separate_in_the_copy() {
    // a merge would fold these two into one; a duplicate is exact
    let w = World::new();
    w.endpoint("E0001-AAA", "https://a.test/same", "GET", None, &[], 1);
    w.endpoint("E0002-AAA", "https://a.test/same", "GET", None, &[], 1);
    w.collection("twins", &["E0001-AAA", "E0002-AAA"], None);
    let out = w.duplicate("twins", "twins (copy)").unwrap();
    assert_eq!(out["members"], 2);
    assert_eq!(w.members("twins (copy)").len(), 2);
}

#[test]
fn a_failed_copy_leaves_nothing_behind() {
    let w = source_world();
    // a trigger that aborts the collection-tag rows makes the transaction fail after the QP
    // files, the collection file, and most of the database rows were already written
    w.conn()
        .execute_batch(
            "CREATE TRIGGER boom BEFORE INSERT ON collection_tag_memberships              BEGIN SELECT RAISE(ABORT, 'boom'); END;",
        )
        .unwrap();

    let before_dirs: BTreeSet<String> = std::fs::read_dir(w.root.join("storage").join("globalEQPData"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    let err = w.duplicate("src", "doomed").unwrap_err();
    assert!(!err.is_empty());

    assert!(!w.collection_file("doomed").exists(), "the collection file was removed");
    assert_eq!(w.count("SELECT COUNT(*) FROM collections WHERE name = 'doomed'"), 0, "no catalog row");
    assert_eq!(w.count("SELECT COUNT(*) FROM bookmarks WHERE folder = 'doomed'"), 0);
    assert_eq!(w.count("SELECT COUNT(*) FROM collection_tag_memberships WHERE collection_name = 'doomed'"), 0);
    // the rows of the transaction were rolled back
    assert_eq!(w.count("SELECT COUNT(*) FROM endpoints"), 3, "only E1, E2 and E3: the copied endpoint rows were rolled back");
    assert_eq!(w.count("SELECT COUNT(*) FROM request_metadata"), 4, "no QP rows were left behind");
    assert_eq!(w.count("SELECT COUNT(*) FROM tags"), 3, "no tag rows were left behind");
    let after_dirs: BTreeSet<String> = std::fs::read_dir(w.root.join("storage").join("globalEQPData"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(after_dirs, before_dirs, "the copied QP folders were removed");
}
