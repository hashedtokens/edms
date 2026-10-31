//! The Merge dropdown's job (`run_collection_merge`): merge whole collections
//! ("Collection") or only the endpoints carrying chosen tags ("Tag") into a new
//! or an existing collection, leaving the sources alone.
//!
//! Every test keeps the database file OUTSIDE the storage root, the way Docker
//! has it (`/app/data/edms.db` vs `/app/edms_root`), because that is exactly
//! where a merge used to put its files in the wrong place.

use compute::tagops::{
    collection_merge::{run_collection_merge, CollectionMergeRequest},
    types::CollectionTagFilter,
};
use edms::{ops::collection_membership_ops::CollectionMembershipOps, schema::initialize_schema};
use rusqlite::Connection;
use std::{collections::BTreeSet, path::PathBuf};
use tempfile::TempDir;

struct World {
    _dir: TempDir,
    db: String,
    root: PathBuf,
    data_dir: PathBuf,
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
        World { _dir: dir, db, root, data_dir }
    }

    fn conn(&self) -> Connection {
        Connection::open(&self.db).unwrap()
    }

    fn endpoint(&self, eid: &str, url: &str, method: &str, tags: &[&str]) {
        let c = self.conn();
        c.execute(
            "INSERT INTO endpoints (endpoint_id, endpoint_str, method) VALUES (?, ?, ?)",
            rusqlite::params![eid, url, method],
        )
        .unwrap();
        for t in tags {
            c.execute("INSERT INTO tags (endpoint_id, tag) VALUES (?, ?)", rusqlite::params![eid, t]).unwrap();
        }
        // one saved QP each
        let d = self.root.join("storage").join("globalEQPData").join(eid);
        std::fs::create_dir_all(&d).unwrap();
        for kind in ["request", "response", "headers"] {
            std::fs::write(d.join(format!("{eid}-{kind}-1.json")), format!(r#"{{"from":"{eid}"}}"#)).unwrap();
        }
    }

    /// A collection the way the app creates one: a file under the root and a
    /// catalog row that names it.
    fn collection(&self, name: &str, eids: &[&str]) {
        let file = self.root.join("storage").join("collections").join(format!("{name}.sqlite"));
        let ops = CollectionMembershipOps::new(file.to_str().unwrap());
        ops.initialize().unwrap();
        for e in eids {
            ops.add(e).unwrap();
        }
        self.conn()
            .execute(
                "INSERT INTO collections (name, file_path) VALUES (?, ?)",
                rusqlite::params![name, file.to_str().unwrap()],
            )
            .unwrap();
    }

    fn members(&self, name: &str) -> BTreeSet<String> {
        let file = self.root.join("storage").join("collections").join(format!("{name}.sqlite"));
        if !file.exists() {
            return BTreeSet::new();
        }
        let ops = CollectionMembershipOps::new(file.to_str().unwrap());
        ops.initialize().unwrap();
        ops.list_ids().unwrap().into_iter().collect()
    }

    fn catalog_path(&self, name: &str) -> Option<String> {
        self.conn()
            .query_row("SELECT file_path FROM collections WHERE name = ?", [name], |r| r.get(0))
            .ok()
    }

    fn url_method(&self, eid: &str) -> (String, String) {
        self.conn()
            .query_row("SELECT endpoint_str, method FROM endpoints WHERE endpoint_id = ?", [eid], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap()
    }

    fn tags_of(&self, eid: &str) -> BTreeSet<String> {
        let c = self.conn();
        let mut st = c.prepare("SELECT tag FROM tags WHERE endpoint_id = ?").unwrap();
        st.query_map([eid], |r| r.get::<_, String>(0)).unwrap().map(Result::unwrap).collect()
    }

    fn qp_files(&self, eid: &str) -> Vec<String> {
        let d = self.root.join("storage").join("globalEQPData").join(eid);
        let mut v: Vec<String> = std::fs::read_dir(&d)
            .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect())
            .unwrap_or_default();
        v.sort();
        v
    }

    fn merge(&self, target: &str, sources: &[(&str, &[&str])]) -> Result<serde_json::Value, String> {
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
    }
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// Two collections that overlap: E1 is in both, and E3 is the same URL+method
/// as E2 under a different EID.
fn overlapping() -> World {
    let w = World::new();
    w.endpoint("E0001-AAA", "https://api.x.com/posts", "GET", &["read-only"]);
    w.endpoint("E0002-AAA", "https://api.x.com/posts/1", "GET", &["read-only", "stable"]);
    w.endpoint("E0003-AAA", "https://api.x.com/posts/1", "GET", &["duplicate"]);
    w.endpoint("E0004-AAA", "https://api.x.com/posts", "POST", &["write"]);
    w.collection("alpha", &["E0001-AAA", "E0002-AAA"]);
    w.collection("beta", &["E0001-AAA", "E0003-AAA", "E0004-AAA"]);
    w
}

// ── Collection level: whole collections ──────────────────────────────────

#[test]
fn two_collections_merge_into_a_new_one_and_the_sources_are_left_alone() {
    let w = overlapping();
    let alpha_before = w.members("alpha");
    let beta_before = w.members("beta");

    let out = w.merge("combined", &[("alpha", &[]), ("beta", &[])]).unwrap();

    assert_eq!(out["ok"], true);
    assert_eq!(out["target_existed"], false);
    assert_eq!(out["target_created"], true);
    assert_eq!(out["members_before"], 0);
    // posts GET, posts/1 GET (E2 and E3 are one endpoint), posts POST
    assert_eq!(out["members_after"], 3, "{out}");
    assert_eq!(out["added"], 3);
    assert_eq!(w.members("combined").len(), 3);

    // what is in it, by URL + method
    let mut got: Vec<(String, String)> = w.members("combined").iter().map(|e| w.url_method(e)).collect();
    got.sort();
    assert_eq!(
        got,
        vec![
            ("https://api.x.com/posts".to_string(), "GET".to_string()),
            ("https://api.x.com/posts".to_string(), "POST".to_string()),
            ("https://api.x.com/posts/1".to_string(), "GET".to_string()),
        ]
    );

    // the sources are exactly as they were
    assert_eq!(w.members("alpha"), alpha_before);
    assert_eq!(w.members("beta"), beta_before);
    // and the merged endpoints are copies with their own EIDs, so editing the
    // new collection can never touch alpha or beta
    for e in w.members("combined") {
        assert!(!alpha_before.contains(&e) && !beta_before.contains(&e), "{e} is a source EID, not a copy");
    }
    // each source's request is reported
    assert_eq!(out["sources"][0]["endpoints"], 2);
    assert_eq!(out["sources"][1]["endpoints"], 3);
}

#[test]
fn the_merge_writes_into_the_storage_root_not_next_to_the_database() {
    let w = overlapping();
    w.merge("combined", &[("alpha", &[]), ("beta", &[])]).unwrap();

    let in_root = w.root.join("storage").join("collections").join("combined.sqlite");
    assert!(in_root.is_file(), "the new collection's file belongs in the storage root");
    assert_eq!(w.catalog_path("combined").as_deref(), in_root.to_str(), "the catalog names that file");
    assert!(
        !w.data_dir.join("storage").exists(),
        "nothing may be created beside the database file (Docker's /app/data)"
    );
}

#[test]
fn qp_files_are_copied_to_the_new_endpoints_in_the_storage_root() {
    let w = overlapping();
    w.merge("combined", &[("alpha", &[])]).unwrap();

    for new_eid in w.members("combined") {
        let files = w.qp_files(&new_eid);
        assert_eq!(
            files,
            vec![
                format!("{new_eid}-headers-1.json"),
                format!("{new_eid}-request-1.json"),
                format!("{new_eid}-response-1.json"),
            ],
            "{new_eid} should have its own copy of the QP"
        );
    }
    // the source's own files are still there, untouched
    assert_eq!(w.qp_files("E0001-AAA").len(), 3);
    assert_eq!(w.qp_files("E0002-AAA").len(), 3);
}

#[test]
fn tags_come_across_and_endpoint_tags_are_unioned() {
    let w = overlapping();
    w.merge("combined", &[("alpha", &[]), ("beta", &[])]).unwrap();
    let combined = w.members("combined");
    let posts_1 = combined.iter().find(|e| w.url_method(e).0.ends_with("/posts/1")).unwrap();
    // E2 had read-only+stable, E3 (same endpoint) had duplicate
    assert_eq!(w.tags_of(posts_1), set(&["duplicate", "read-only", "stable"]));
}

#[test]
fn merging_into_an_existing_collection_adds_and_keeps_what_was_there() {
    let w = overlapping();
    w.endpoint("E0010-AAA", "https://api.x.com/only-in-target", "GET", &[]);
    w.collection("target", &["E0010-AAA"]);

    let out = w.merge("target", &[("alpha", &[])]).unwrap();

    assert_eq!(out["target_existed"], true);
    assert_eq!(out["target_created"], false);
    assert_eq!(out["members_before"], 1);
    assert_eq!(out["members_after"], 3);
    assert_eq!(out["added"], 2);
    let members = w.members("target");
    assert!(members.contains("E0010-AAA"), "the target's own endpoint stays");
    assert_eq!(members.len(), 3);
    // its catalog row wasn't replaced
    assert!(w.catalog_path("target").unwrap().ends_with("target.sqlite"));
}

#[test]
fn an_endpoint_already_in_the_target_is_matched_not_duplicated() {
    let w = overlapping();
    // target already has "posts GET" under its own EID
    w.endpoint("E0020-AAA", "https://api.x.com/posts", "GET", &["mine"]);
    w.collection("target", &["E0020-AAA"]);

    let out = w.merge("target", &[("alpha", &[])]).unwrap();

    // alpha has posts GET (matched) and posts/1 GET (new)
    assert_eq!(out["members_after"], 2, "{out}");
    assert_eq!(out["added"], 1);
    assert!(w.members("target").contains("E0020-AAA"));
    // the matched endpoint gains alpha's tag and keeps its own
    assert_eq!(w.tags_of("E0020-AAA"), set(&["mine", "read-only"]));
}

#[test]
fn running_the_same_merge_twice_changes_nothing_the_second_time() {
    let w = overlapping();
    w.merge("combined", &[("alpha", &[]), ("beta", &[])]).unwrap();
    let after_first = w.members("combined");

    let second = w.merge("combined", &[("alpha", &[]), ("beta", &[])]).unwrap();

    assert_eq!(second["added"], 0, "{second}");
    assert_eq!(second["target_existed"], true);
    assert_eq!(w.members("combined"), after_first, "no new EIDs, no new rows");
}

// ── Tag level: only the tagged endpoints ─────────────────────────────────

#[test]
fn a_tag_merge_takes_only_the_endpoints_with_those_tags_from_each_source() {
    let w = overlapping();

    // alpha: only "stable"  -> posts/1 ;  beta: only "write" -> posts POST
    let out = w.merge("picked", &[("alpha", &["stable"]), ("beta", &["write"])]).unwrap();

    assert_eq!(out["target_created"], true);
    assert_eq!(out["members_after"], 2, "{out}");
    let mut got: Vec<(String, String)> = w.members("picked").iter().map(|e| w.url_method(e)).collect();
    got.sort();
    assert_eq!(
        got,
        vec![
            ("https://api.x.com/posts".to_string(), "POST".to_string()),
            ("https://api.x.com/posts/1".to_string(), "GET".to_string()),
        ]
    );
    // how many each source offered after filtering
    assert_eq!(out["sources"][0]["endpoints"], 1);
    assert_eq!(out["sources"][0]["tags"][0], "stable");
    assert_eq!(out["sources"][1]["endpoints"], 1);
}

#[test]
fn several_tags_in_one_filter_mean_any_of_them() {
    let w = overlapping();
    let out = w.merge("picked", &[("beta", &["write", "duplicate"])]).unwrap();
    assert_eq!(out["members_after"], 2, "write (E4) or duplicate (E3): {out}");
}

#[test]
fn a_tag_merge_into_an_existing_collection_adds_only_the_tagged_ones() {
    let w = overlapping();
    w.endpoint("E0010-AAA", "https://api.x.com/mine", "GET", &[]);
    w.collection("target", &["E0010-AAA"]);

    let out = w.merge("target", &[("beta", &["write"])]).unwrap();

    assert_eq!(out["added"], 1);
    assert_eq!(w.members("target").len(), 2);
    assert!(w.members("target").contains("E0010-AAA"));
}

#[test]
fn a_tag_merge_does_not_pull_in_the_sources_own_collection_tags() {
    let w = overlapping();
    w.conn()
        .execute("INSERT INTO collection_tag_memberships (collection_name, tagname) VALUES ('alpha', 'team-a')", [])
        .unwrap();
    w.merge("picked", &[("alpha", &["stable"])]).unwrap();
    let n: i64 = w
        .conn()
        .query_row("SELECT COUNT(*) FROM collection_tag_memberships WHERE collection_name = 'picked'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0, "only a whole-collection merge carries the collection's tags");

    w.merge("whole", &[("alpha", &[])]).unwrap();
    let n: i64 = w
        .conn()
        .query_row("SELECT COUNT(*) FROM collection_tag_memberships WHERE collection_name = 'whole'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1, "a whole-collection merge does");
}

// ── Nothing to do / refused ──────────────────────────────────────────────

#[test]
fn a_merge_that_matches_nothing_creates_no_collection() {
    let w = overlapping();
    let out = w.merge("empty", &[("alpha", &["no-such-tag"])]).unwrap();

    assert_eq!(out["target_created"], false);
    assert_eq!(out["members_after"], 0);
    assert_eq!(out["added"], 0);
    assert!(w.catalog_path("empty").is_none(), "no catalog row for an empty merge");
    assert!(!w.root.join("storage").join("collections").join("empty.sqlite").exists());
}

#[test]
fn a_collection_cannot_be_merged_into_itself() {
    let w = overlapping();
    let err = w.merge("alpha", &[("alpha", &[]), ("beta", &[])]).unwrap_err();
    assert!(err.contains("Self-merge"), "{err}");
    assert_eq!(w.members("alpha"), set(&["E0001-AAA", "E0002-AAA"]));
}

#[test]
fn no_sources_is_refused() {
    let w = overlapping();
    let err = w.merge("combined", &[]).unwrap_err();
    assert!(err.contains("No sources"), "{err}");
}

#[test]
fn a_failure_to_write_the_collection_file_fails_the_merge_instead_of_pretending() {
    let w = overlapping();
    // a directory where the new collection's file must go
    std::fs::create_dir_all(w.root.join("storage").join("collections").join("blocked.sqlite")).unwrap();

    let result = w.merge("blocked", &[("alpha", &[])]);

    assert!(result.is_err(), "the merge must not report success when nothing could be written: {result:?}");
}
