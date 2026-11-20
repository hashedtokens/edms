//! What a merge brings with it besides the endpoints: the QPs the app can see
//! (files AND request/response rows), the bookmark notes, the collection's
//! annotation, no second copy of a QP the target already has, and an exact
//! stored size.
//!
//! Every test keeps the database file OUTSIDE the storage root, the way Docker
//! has it (`/app/data/edms.db` vs `/app/edms_root`).

use compute::eid::allocator::EidAllocator;
use compute::tagops::{
    collection_merge::{run_collection_merge, CollectionMergeRequest},
    types::CollectionTagFilter,
};
use edms::{ops::collection_membership_ops::CollectionMembershipOps, schema::initialize_schema};
use rusqlite::Connection;
use std::{collections::BTreeSet, path::PathBuf};
use tempfile::TempDir;

/// One saved QP: request, response, headers (the combined file's JSON), status, time.
#[derive(Clone)]
struct Qp {
    request: &'static str,
    response: &'static str,
    status: Option<i32>,
    time: Option<i32>,
}

const HEADERS: &str = r#"{"request_headers":{},"response_headers":{"content-type":"application/json"}}"#;

fn qp(n: i32) -> Qp {
    match n {
        1 => Qp { request: r#"{"q":1}"#, response: r#"{"r":1}"#, status: Some(200), time: Some(11) },
        2 => Qp { request: r#"{"q":2}"#, response: r#"{"r":2}"#, status: Some(404), time: Some(22) },
        _ => Qp { request: r#"{"q":3}"#, response: r#"{"r":3}"#, status: Some(500), time: Some(33) },
    }
}

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

    /// An endpoint with the given QPs (files and rows). `rows: false` makes
    /// the QPs the way an old merge left them: files with no rows.
    fn endpoint(&self, eid: &str, url: &str, method: &str, qps: &[Qp], rows: bool) {
        let c = self.conn();
        c.execute(
            "INSERT INTO endpoints (endpoint_id, endpoint_str, method) VALUES (?, ?, ?)",
            rusqlite::params![eid, url, method],
        )
        .unwrap();
        let d = self.qp_dir(eid);
        std::fs::create_dir_all(&d).unwrap();
        for (i, q) in qps.iter().enumerate() {
            let n = i as i64 + 1;
            std::fs::write(d.join(format!("{eid}-request-{n}.json")), q.request).unwrap();
            std::fs::write(d.join(format!("{eid}-response-{n}.json")), q.response).unwrap();
            std::fs::write(d.join(format!("{eid}-headers-{n}.json")), HEADERS).unwrap();
            if rows {
                c.execute(
                    "INSERT INTO request_metadata (endpoint_id, request_number, file_path, method) VALUES (?, ?, ?, ?)",
                    rusqlite::params![eid, n, d.join(format!("{eid}-request-{n}.json")).display().to_string(), method],
                )
                .unwrap();
                c.execute(
                    "INSERT INTO response_metadata (endpoint_id, request_number, file_path, status_code, response_time_ms) VALUES (?, ?, ?, ?, ?)",
                    rusqlite::params![eid, n, d.join(format!("{eid}-response-{n}.json")).display().to_string(), q.status, q.time],
                )
                .unwrap();
            }
        }
        let alloc = EidAllocator::new(&self.db);
        alloc.initialize().unwrap();
        alloc.reserve(eid).unwrap();
    }

    fn collection(&self, name: &str, members: &[&str], annotation: Option<&str>) {
        let file = self.collection_file(name);
        let ops = CollectionMembershipOps::new(file.to_str().unwrap());
        ops.initialize().unwrap();
        for e in members {
            ops.add(e).unwrap();
        }
        let _ = ops.shutdown();
        let c = self.conn();
        c.execute(
            "INSERT INTO collections (name, file_path, annotation) VALUES (?, ?, ?)",
            rusqlite::params![name, file.to_str().unwrap(), annotation],
        )
        .unwrap();
        for e in members {
            c.execute("INSERT OR IGNORE INTO bookmarks (endpoint_id, folder, notes) VALUES (?, ?, NULL)", rusqlite::params![e, name]).unwrap();
        }
    }

    fn note(&self, eid: &str, folder: &str, note: &str) {
        self.conn()
            .execute("UPDATE bookmarks SET notes = ? WHERE endpoint_id = ? AND folder = ?", rusqlite::params![note, eid, folder])
            .unwrap();
    }

    fn members(&self, name: &str) -> Vec<String> {
        let ops = CollectionMembershipOps::new(self.collection_file(name).to_str().unwrap());
        ops.initialize().unwrap();
        ops.list_ids().unwrap()
    }

    fn merge(&self, target: &str, sources: &[(&str, &[&str])]) -> serde_json::Value {
        run_collection_merge(&CollectionMergeRequest {
            db_path: self.db.clone(),
            storage_root: self.root.clone(),
            target_collection_name: target.to_string(),
            sources: sources
                .iter()
                .map(|(c, t)| CollectionTagFilter {
                    collection_id: c.to_string(),
                    tags: t.iter().map(|s| s.to_string()).collect(),
                })
                .collect(),
        })
        .unwrap()
    }

    fn eid_of(&self, url: &str, method: &str, not_in: &[&str]) -> String {
        let c = self.conn();
        let mut st = c.prepare("SELECT endpoint_id FROM endpoints WHERE endpoint_str = ? AND method = ?").unwrap();
        let all: Vec<String> = st.query_map(rusqlite::params![url, method], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
        all.into_iter().find(|e| !not_in.contains(&e.as_str())).unwrap()
    }

    /// The QP numbers the app lists for an endpoint (the rows), with status codes and times.
    fn qp_rows(&self, eid: &str) -> Vec<(i64, Option<i64>, Option<i64>)> {
        let c = self.conn();
        let mut st = c
            .prepare(
                "SELECT r.request_number, p.status_code, p.response_time_ms FROM request_metadata r \
                 LEFT JOIN response_metadata p ON p.endpoint_id = r.endpoint_id AND p.request_number = r.request_number \
                 WHERE r.endpoint_id = ? ORDER BY r.request_number",
            )
            .unwrap();
        st.query_map([eid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(Result::unwrap).collect()
    }

    fn files(&self, eid: &str) -> BTreeSet<String> {
        std::fs::read_dir(self.qp_dir(eid))
            .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect())
            .unwrap_or_default()
    }

    fn annotation(&self, name: &str) -> Option<String> {
        self.conn().query_row("SELECT annotation FROM collections WHERE name = ?", [name], |r| r.get(0)).unwrap()
    }

    fn notes(&self, folder: &str) -> Vec<(String, Option<String>)> {
        let c = self.conn();
        let mut st = c.prepare("SELECT endpoint_id, notes FROM bookmarks WHERE folder = ? ORDER BY endpoint_id").unwrap();
        st.query_map([folder], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(Result::unwrap).collect()
    }

    fn stored_size(&self, name: &str) -> Option<i64> {
        let ops = CollectionMembershipOps::new(self.collection_file(name).to_str().unwrap());
        ops.initialize().unwrap();
        ops.size_bytes().unwrap()
    }

    fn real_size(&self, name: &str) -> i64 {
        self.members(name)
            .iter()
            .map(|e| {
                std::fs::read_dir(self.qp_dir(e))
                    .map(|rd| rd.flatten().map(|f| f.metadata().unwrap().len() as i64).sum::<i64>())
                    .unwrap_or(0)
            })
            .sum()
    }
}

// ── The QPs the app can see ──────────────────────────────────────────────

#[test]
fn merged_endpoints_show_their_qps_in_the_app_not_just_their_files() {
    let w = World::new();
    w.endpoint("E0001-AAA", "https://a.test/x", "POST", &[qp(1), qp(2)], true);
    w.endpoint("E0002-AAA", "https://a.test/y", "GET", &[qp(3)], true);
    w.collection("src", &["E0001-AAA", "E0002-AAA"], None);

    let out = w.merge("dst", &[("src", &[])]);
    assert_eq!(out["qps_added"], 3, "{out}");
    assert_eq!(out["qps_already_there"], 0);

    let x = w.eid_of("https://a.test/x", "POST", &["E0001-AAA"]);
    let y = w.eid_of("https://a.test/y", "GET", &["E0002-AAA"]);
    // the rows the app lists a QP from, with the status code and timing of the source
    assert_eq!(w.qp_rows(&x), vec![(1, Some(200), Some(11)), (2, Some(404), Some(22))]);
    assert_eq!(w.qp_rows(&y), vec![(1, Some(500), Some(33))]);
    // and the files, under the new EID, with the same content
    assert_eq!(w.files(&x).len(), 6);
    assert_eq!(std::fs::read_to_string(w.qp_dir(&x).join(format!("{x}-request-2.json"))).unwrap(), qp(2).request);
    assert_eq!(std::fs::read_to_string(w.qp_dir(&x).join(format!("{x}-response-1.json"))).unwrap(), qp(1).response);
    // the row points at the file under the new EID
    let path: String = w
        .conn()
        .query_row("SELECT file_path FROM request_metadata WHERE endpoint_id = ? AND request_number = 1", [&x], |r| r.get(0))
        .unwrap();
    assert!(path.contains(&x), "{path}");
}

#[test]
fn the_sources_qps_are_left_as_they_were() {
    let w = World::new();
    w.endpoint("E0001-AAA", "https://a.test/x", "POST", &[qp(1), qp(2)], true);
    w.collection("src", &["E0001-AAA"], Some("source"));
    w.merge("dst", &[("src", &[])]);
    assert_eq!(w.qp_rows("E0001-AAA"), vec![(1, Some(200), Some(11)), (2, Some(404), Some(22))]);
    assert_eq!(w.files("E0001-AAA").len(), 6);
    assert_eq!(w.annotation("src").as_deref(), Some("source"));
}

#[test]
fn qps_an_old_merge_left_without_rows_are_still_copied_and_now_get_rows() {
    let w = World::new();
    w.endpoint("E0001-AAA", "https://a.test/old", "GET", &[qp(1)], false);
    w.collection("src", &["E0001-AAA"], None);
    let out = w.merge("dst", &[("src", &[])]);
    assert_eq!(out["qps_added"], 1);
    let copy = w.eid_of("https://a.test/old", "GET", &["E0001-AAA"]);
    assert_eq!(w.qp_rows(&copy), vec![(1, None, None)], "no status was ever recorded, none is made up");
}

#[test]
fn a_qp_with_only_a_request_file_is_copied_with_defaults_for_the_rest() {
    let w = World::new();
    w.endpoint("E0001-AAA", "https://a.test/thin", "GET", &[], false);
    std::fs::create_dir_all(w.qp_dir("E0001-AAA")).unwrap();
    std::fs::write(w.qp_dir("E0001-AAA").join("E0001-AAA-request-1.json"), r#"{"only":"request"}"#).unwrap();
    w.collection("src", &["E0001-AAA"], None);
    w.merge("dst", &[("src", &[])]);
    let copy = w.eid_of("https://a.test/thin", "GET", &["E0001-AAA"]);
    assert!(w.files(&copy).contains(&format!("{copy}-request-1.json")));
    assert_eq!(w.qp_rows(&copy).len(), 1);
}

// ── No second copy of a QP the target already has ────────────────────────

#[test]
fn a_qp_the_target_endpoint_already_has_is_not_copied_again() {
    let w = World::new();
    // the same endpoint in both collections; the source has QPs 1 and 2, the target already has QP 1's twin
    w.endpoint("E0001-AAA", "https://a.test/x", "GET", &[qp(1), qp(2)], true);
    w.endpoint("E0002-AAA", "https://a.test/x", "GET", &[qp(1)], true);
    w.collection("src", &["E0001-AAA"], None);
    w.collection("dst", &["E0002-AAA"], None);

    let out = w.merge("dst", &[("src", &[])]);
    assert_eq!(out["qps_added"], 1, "only the QP the target did not have: {out}");
    assert_eq!(out["qps_already_there"], 1);
    assert_eq!(w.qp_rows("E0002-AAA"), vec![(1, Some(200), Some(11)), (2, Some(404), Some(22))]);
    assert_eq!(w.files("E0002-AAA").len(), 6, "two QPs, three files each");
}

#[test]
fn merging_the_same_collection_twice_adds_nothing_the_second_time() {
    let w = World::new();
    w.endpoint("E0001-AAA", "https://a.test/x", "POST", &[qp(1), qp(2)], true);
    w.collection("src", &["E0001-AAA"], None);

    let first = w.merge("dst", &[("src", &[])]);
    assert_eq!(first["qps_added"], 2);
    let copy = w.eid_of("https://a.test/x", "POST", &["E0001-AAA"]);
    let (rows, files) = (w.qp_rows(&copy), w.files(&copy));

    let second = w.merge("dst", &[("src", &[])]);
    assert_eq!(second["qps_added"], 0, "{second}");
    assert_eq!(second["qps_already_there"], 2);
    assert_eq!(w.qp_rows(&copy), rows);
    assert_eq!(w.files(&copy), files);
    assert_eq!(w.members("dst").len(), 1);
}

#[test]
fn a_merge_into_an_existing_endpoint_numbers_the_new_qps_after_its_own() {
    let w = World::new();
    w.endpoint("E0001-AAA", "https://a.test/x", "GET", &[qp(3)], true);
    w.endpoint("E0002-AAA", "https://a.test/x", "GET", &[qp(1), qp(2)], true);
    w.collection("src", &["E0001-AAA"], None);
    w.collection("dst", &["E0002-AAA"], None);
    w.merge("dst", &[("src", &[])]);
    assert_eq!(w.qp_rows("E0002-AAA").iter().map(|r| r.0).collect::<Vec<_>>(), vec![1, 2, 3]);
    assert_eq!(w.qp_rows("E0002-AAA")[2], (3, Some(500), Some(33)));
}

// ── Notes and annotation ─────────────────────────────────────────────────

#[test]
fn bookmark_notes_come_along() {
    let w = World::new();
    w.endpoint("E0001-AAA", "https://a.test/x", "GET", &[qp(1)], true);
    w.endpoint("E0002-AAA", "https://a.test/y", "GET", &[qp(1)], true);
    w.collection("src", &["E0001-AAA", "E0002-AAA"], None);
    w.note("E0001-AAA", "src", "note on x");
    w.merge("dst", &[("src", &[])]);

    let x = w.eid_of("https://a.test/x", "GET", &["E0001-AAA"]);
    let y = w.eid_of("https://a.test/y", "GET", &["E0002-AAA"]);
    let notes: std::collections::HashMap<String, Option<String>> = w.notes("dst").into_iter().collect();
    assert_eq!(notes[&x].as_deref(), Some("note on x"));
    assert_eq!(notes[&y], None, "no note in the source, none in the copy");
}

#[test]
fn a_note_the_target_wrote_is_kept_and_an_empty_one_is_filled() {
    let w = World::new();
    w.endpoint("E0001-AAA", "https://a.test/x", "GET", &[qp(1)], true);
    w.endpoint("E0002-AAA", "https://a.test/y", "GET", &[qp(1)], true);
    w.endpoint("E0003-AAA", "https://a.test/x", "GET", &[qp(1)], true);
    w.endpoint("E0004-AAA", "https://a.test/y", "GET", &[qp(1)], true);
    w.collection("src", &["E0001-AAA", "E0002-AAA"], None);
    w.note("E0001-AAA", "src", "source note x");
    w.note("E0002-AAA", "src", "source note y");
    w.collection("dst", &["E0003-AAA", "E0004-AAA"], None);
    w.note("E0003-AAA", "dst", "the target's own note");

    w.merge("dst", &[("src", &[])]);
    let notes: std::collections::HashMap<String, Option<String>> = w.notes("dst").into_iter().collect();
    assert_eq!(notes["E0003-AAA"].as_deref(), Some("the target's own note"), "never replaced");
    assert_eq!(notes["E0004-AAA"].as_deref(), Some("source note y"), "an empty note takes the source's");
}

#[test]
fn a_new_target_takes_the_annotation_of_the_first_source_that_has_one() {
    let w = World::new();
    w.endpoint("E0001-AAA", "https://a.test/x", "GET", &[qp(1)], true);
    w.endpoint("E0002-AAA", "https://a.test/y", "GET", &[qp(1)], true);
    w.endpoint("E0003-AAA", "https://a.test/z", "GET", &[qp(1)], true);
    w.collection("plain", &["E0001-AAA"], None);
    w.collection("second", &["E0002-AAA"], Some("from the second"));
    w.collection("third", &["E0003-AAA"], Some("from the third"));
    w.merge("dst", &[("plain", &[]), ("second", &[]), ("third", &[])]);
    assert_eq!(w.annotation("dst").as_deref(), Some("from the second"));
}

#[test]
fn an_existing_target_keeps_its_own_annotation() {
    let w = World::new();
    w.endpoint("E0001-AAA", "https://a.test/x", "GET", &[qp(1)], true);
    w.endpoint("E0002-AAA", "https://a.test/y", "GET", &[qp(1)], true);
    w.collection("src", &["E0001-AAA"], Some("the source's"));
    w.collection("dst", &["E0002-AAA"], Some("the target's"));
    w.merge("dst", &[("src", &[])]);
    assert_eq!(w.annotation("dst").as_deref(), Some("the target's"));

    let w2 = World::new();
    w2.endpoint("E0001-AAA", "https://a.test/x", "GET", &[qp(1)], true);
    w2.endpoint("E0002-AAA", "https://a.test/y", "GET", &[qp(1)], true);
    w2.collection("src", &["E0001-AAA"], Some("the source's"));
    w2.collection("dst", &["E0002-AAA"], None);
    w2.merge("dst", &[("src", &[])]);
    assert_eq!(w2.annotation("dst"), None, "an existing target without one is not given the source's");
}

#[test]
fn a_merge_that_matches_nothing_creates_nothing() {
    let w = World::new();
    w.collection("empty", &[], Some("nothing"));
    let out = w.merge("dst", &[("empty", &[])]);
    assert_eq!(out["target_created"], false);
    assert!(!w.collection_file("dst").exists());
    assert_eq!(w.conn().query_row::<i64, _, _>("SELECT COUNT(*) FROM collections WHERE name = 'dst'", [], |r| r.get(0)).unwrap(), 0);
}

// ── Tag merges ───────────────────────────────────────────────────────────

#[test]
fn a_tag_merge_brings_qps_and_notes_of_only_the_chosen_endpoints() {
    let w = World::new();
    w.endpoint("E0001-AAA", "https://a.test/x", "GET", &[qp(1), qp(2)], true);
    w.endpoint("E0002-AAA", "https://a.test/y", "GET", &[qp(3)], true);
    w.conn().execute("INSERT INTO tags (endpoint_id, tag) VALUES ('E0001-AAA', 'pick')", []).unwrap();
    w.collection("src", &["E0001-AAA", "E0002-AAA"], None);
    w.note("E0001-AAA", "src", "picked");
    w.note("E0002-AAA", "src", "not picked");

    let out = w.merge("dst", &[("src", &["pick"])]);
    assert_eq!(out["qps_added"], 2);
    let members = w.members("dst");
    assert_eq!(members.len(), 1);
    assert_eq!(w.qp_rows(&members[0]).len(), 2);
    assert_eq!(w.notes("dst")[0].1.as_deref(), Some("picked"));
}

// ── The stored size ──────────────────────────────────────────────────────

#[test]
fn a_new_targets_stored_size_is_what_its_files_take() {
    let w = World::new();
    w.endpoint("E0001-AAA", "https://a.test/x", "POST", &[qp(1), qp(2)], true);
    w.endpoint("E0002-AAA", "https://a.test/y", "GET", &[qp(3)], true);
    w.collection("src", &["E0001-AAA", "E0002-AAA"], None);
    w.merge("dst", &[("src", &[])]);
    let real = w.real_size("dst");
    assert!(real > 0);
    assert_eq!(w.stored_size("dst"), Some(real));
}

#[test]
fn an_existing_targets_size_follows_the_merge_even_when_it_was_stale() {
    let w = World::new();
    w.endpoint("E0001-AAA", "https://a.test/x", "GET", &[qp(1), qp(2)], true);
    w.endpoint("E0002-AAA", "https://a.test/x", "GET", &[qp(3)], true);
    w.endpoint("E0003-AAA", "https://a.test/other", "GET", &[qp(1)], true);
    w.collection("src", &["E0001-AAA", "E0003-AAA"], None);
    w.collection("dst", &["E0002-AAA"], None);
    {
        // the target's stored size is already wrong before the merge
        let ops = CollectionMembershipOps::new(w.collection_file("dst").to_str().unwrap());
        ops.initialize().unwrap();
        ops.set_size_bytes(7).unwrap();
        let _ = ops.shutdown();
    }
    w.merge("dst", &[("src", &[])]);
    assert_eq!(w.members("dst").len(), 2);
    assert_eq!(w.stored_size("dst"), Some(w.real_size("dst")));
}

#[test]
fn another_collection_holding_a_grown_endpoint_is_told_too() {
    let w = World::new();
    w.endpoint("E0001-AAA", "https://a.test/x", "GET", &[qp(1), qp(2)], true);
    w.endpoint("E0002-AAA", "https://a.test/x", "GET", &[qp(3)], true);
    w.collection("src", &["E0001-AAA"], None);
    w.collection("dst", &["E0002-AAA"], None);
    w.collection("bystander", &["E0002-AAA"], None);
    for name in ["dst", "bystander"] {
        let ops = CollectionMembershipOps::new(w.collection_file(name).to_str().unwrap());
        ops.initialize().unwrap();
        ops.set_size_bytes(w.real_size(name)).unwrap();
        let _ = ops.shutdown();
    }
    w.merge("dst", &[("src", &[])]);
    assert_eq!(w.stored_size("bystander"), Some(w.real_size("bystander")), "E2 grew, so every collection holding it did");
}
