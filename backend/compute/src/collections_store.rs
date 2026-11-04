//! The real storage behind the collections Dev API ops: the central
//! database, the storage folder and the collection files, through the same
//! tables and files the webserver uses.
//!
//! Endpoint, QP and History steps are `QpStore`'s (shared with the other Dev
//! API ops). What is here is the Collection side of a Save: the collection
//! itself (file + catalog row, as `POST /collections/create` makes it) and an
//! endpoint joining it (bookmark row, membership, stored size, as
//! `POST /test-view/save/bookmark` followed by
//! `POST /bookmarks/:collection/:endpoint_id/save` leave it).

use crate::collections_ops::{QpToStore, StorageBackend};
use crate::qp_store::{NewQp, QpStore, Stored};
use edms::ops::collection_membership_ops::CollectionMembershipOps;
use edms::ops::view_ops::{ViewCatalogOps, ViewKind};

fn err<E: std::fmt::Debug>(e: E) -> String {
    format!("{e:?}")
}

pub struct CollectionStore {
    qp: QpStore,
    catalog: Option<ViewCatalogOps>,
}

impl CollectionStore {
    pub fn new(qp: QpStore) -> Self {
        CollectionStore { qp, catalog: None }
    }

    fn catalog(&mut self) -> Result<&ViewCatalogOps, String> {
        if self.catalog.is_none() {
            let ops = ViewCatalogOps::new(self.qp.db_path());
            ops.initialize().map_err(err)?;
            self.catalog = Some(ops);
        }
        Ok(self.catalog.as_ref().expect("just set"))
    }

    /// `storage/collections/{name}.sqlite`, where `POST /collections/create`
    /// puts a collection's own file.
    fn file_path(&self, name: &str) -> String {
        self.qp
            .root()
            .join("storage")
            .join("collections")
            .join(format!("{name}.sqlite"))
            .display()
            .to_string()
    }

    /// The membership file the catalog names for an existing collection.
    fn open_membership(&mut self, name: &str) -> Result<CollectionMembershipOps, String> {
        let row = self
            .catalog()?
            .get(ViewKind::Collections, name)
            .map_err(err)?
            .ok_or_else(|| format!("Collection '{name}' does not exist"))?;
        let path = row.1.ok_or_else(|| format!("Collection '{name}' has no file yet"))?;
        let ops = CollectionMembershipOps::new(&path);
        ops.initialize().map_err(err)?;
        Ok(ops)
    }

    /// The stored size, counting it first if this collection never had one
    /// (a collection from before sizes were kept). Do it before adding a
    /// member, so that first count is never doubled.
    fn ensure_size(&self, membership: &CollectionMembershipOps) -> Result<i64, String> {
        if let Some(size) = membership.size_bytes().map_err(err)? {
            return Ok(size);
        }
        let size: i64 = membership
            .list_ids()
            .map_err(err)?
            .iter()
            .map(|eid| self.qp.endpoint_size(eid))
            .sum();
        membership.set_size_bytes(size).map_err(err)?;
        Ok(size)
    }
}

impl StorageBackend for CollectionStore {
    fn ensure_collection(&mut self, name: &str, annotation: Option<&str>) -> Result<bool, String> {
        let path = self.file_path(name);
        let qp = &self.qp;
        // Look and create in one locked section: of several calls creating the
        // same collection at once, one makes it and the rest find it. Only the
        // central connection is used inside it (the catalog's own would wait
        // for the lock this one holds).
        qp.locked(|| {
            let exists: Vec<i64> = qp
                .core()
                .cproc("SELECT COUNT(*) FROM collections WHERE name = ?", &[&name], |row| row.get(0))
                .map_err(err)?;
            if exists.first().copied().unwrap_or(0) > 0 {
                return Ok(false);
            }
            if let Some(dir) = std::path::Path::new(&path).parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("can't create {}: {e}", dir.display()))?;
            }
            // The file and its schema first: no catalog row should point at a
            // file that doesn't exist.
            let membership = CollectionMembershipOps::new(&path);
            membership.initialize().map_err(err)?;
            let create = qp
                .queries()
                .get_catalog_query("COLLECTIONS_CREATE")
                .ok_or("query COLLECTIONS_CREATE is missing")?;
            let file_path: Option<&str> = Some(&path);
            qp.core().proc(create, &[&name, &file_path, &annotation]).map_err(err)?;
            Ok(true)
        })
    }

    fn find_or_create_endpoint(
        &mut self,
        endpoint_id: Option<&str>,
        endpoint_str: &str,
        method: &str,
        annotation: Option<&str>,
    ) -> Result<(String, bool), String> {
        self.qp.find_or_create_endpoint(endpoint_id, endpoint_str, method, annotation)
    }

    fn discard_endpoint(&mut self, endpoint_id: &str) -> Result<(), String> {
        self.qp.discard_endpoint(endpoint_id)
    }

    fn store_qp(&mut self, qp: &QpToStore) -> Result<Stored, String> {
        self.qp.store_qp(&NewQp {
            endpoint_id: qp.endpoint_id,
            endpoint_str: qp.endpoint_str,
            method: qp.method,
            request: qp.request,
            response: qp.response,
            headers: qp.headers,
            status_code: qp.status_code,
            response_time_ms: qp.response_time_ms,
        })
    }

    fn record_history(&mut self, endpoint_id: &str, details: &str) -> Result<(), String> {
        self.qp.record_history(endpoint_id, "test", Some(details))
    }

    fn add_to_collection(&mut self, collection: &str, endpoint_id: &str, notes: Option<&str>) -> Result<(), String> {
        let membership = self.open_membership(collection)?;
        // One locked section, like storing a QP: whether the endpoint is
        // joining, the size it brings and the QPs other calls are writing to it
        // have to be seen as one picture.
        self.qp.locked(|| {
            let joining = !membership.list_set().map_err(err)?.contains(endpoint_id);
            if joining {
                self.ensure_size(&membership)?;
            }

            // The bookmark (what Test View's Bookmark makes); an existing one is
            // kept, and gets the notes if some were given.
            self.qp
                .core()
                .proc(
                    "INSERT OR IGNORE INTO bookmarks (endpoint_id, folder, notes) VALUES (?, ?, ?)",
                    &[&endpoint_id, &collection, &notes],
                )
                .map_err(err)?;
            if notes.is_some() {
                self.qp
                    .core()
                    .proc(
                        "UPDATE bookmarks SET notes = ? WHERE endpoint_id = ? AND folder = ?",
                        &[&notes, &endpoint_id, &collection],
                    )
                    .map_err(err)?;
            }

            // The membership (what Save makes), and the size it brings.
            membership.add(endpoint_id).map_err(err)?;
            if joining {
                let _ = membership.adjust_size_bytes(self.qp.endpoint_size(endpoint_id));
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_status::ApiStatus;
    use crate::collections_ops::{
        process_collection_op, CollectionOpRequest, EndpointResponse, HttpCall, HttpFailure,
    };
    use edms::core::EdmsCore;
    use serde_json::{json, Value};
    use std::path::Path;

    /// A scratch EDMS: an empty database with the real schema, and a storage
    /// root with the real folders.
    struct Scratch {
        _dir: tempfile::TempDir,
        db: String,
        root: std::path::PathBuf,
    }

    fn scratch() -> Scratch {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        crate::folder_manager::verify_and_init(&root).unwrap();
        let db = dir.path().join("edms.db").display().to_string();
        let core = EdmsCore::new(&db);
        core.connect().unwrap();
        edms::schema::initialize_schema_from_core(&core).unwrap();
        Scratch { _dir: dir, db, root }
    }

    impl Scratch {
        fn storage(&self) -> CollectionStore {
            CollectionStore::new(QpStore::open(&self.db, &self.root).unwrap())
        }

        fn core(&self) -> EdmsCore {
            let c = EdmsCore::new(&self.db);
            c.connect().unwrap();
            c
        }

        fn count(&self, sql: &str) -> i64 {
            let rows: Vec<i64> = self.core().cproc(sql, &[], |r| r.get(0)).unwrap();
            rows[0]
        }

        fn collection_file(&self, name: &str) -> String {
            self.root.join("storage").join("collections").join(format!("{name}.sqlite")).display().to_string()
        }

        fn membership(&self, name: &str) -> CollectionMembershipOps {
            let m = CollectionMembershipOps::new(&self.collection_file(name));
            m.initialize().unwrap();
            m
        }

        fn eqp_bytes(&self, eid: &str) -> i64 {
            let dir = self.root.join("storage").join("globalEQPData").join(eid);
            std::fs::read_dir(&dir)
                .map(|d| d.flatten().map(|e| e.metadata().unwrap().len() as i64).sum())
                .unwrap_or(0)
        }
    }

    fn insert_req(collection: &str, url: &str, req: Value, res: Value) -> CollectionOpRequest {
        CollectionOpRequest {
            ops: "insert".to_string(),
            endpoint_str: url.to_string(),
            collection: collection.to_string(),
            method: Some("POST".to_string()),
            request_body: Some(req),
            response_body: Some(res),
            ..Default::default()
        }
    }

    async fn never(_: HttpCall) -> Result<EndpointResponse, HttpFailure> {
        unreachable!("an insert makes no HTTP call")
    }

    fn answer(status: u16, body: Value) -> EndpointResponse {
        EndpointResponse {
            status,
            headers: json!({"content-type": "application/json"}),
            body,
            time_ms: 87,
        }
    }

    #[tokio::test]
    async fn an_insert_leaves_what_a_run_a_bookmark_and_a_save_leave() {
        let s = scratch();
        let mut req = insert_req("auth", "https://api.example.test/v1/login", json!({"user": "alice"}), json!({"token": "xyz"}));
        req.collection_annotation = Some("login flows".to_string());
        req.annotation = Some("log in".to_string());
        req.request_headers = Some(json!({"X-A": "1"}));
        req.response_headers = Some(json!({"content-type": "application/json"}));
        req.notes = Some("first".to_string());

        let res = process_collection_op(req, &mut s.storage(), never).await;
        assert_eq!(res.result.status, ApiStatus::Ok, "{:?}", res.result.detail);
        let eid = res.result.endpoint_id.clone().unwrap();
        assert!(crate::eid::is_valid_eid(&eid), "a real EID, got {eid}");
        assert_eq!(res.result.request_number, Some(1));
        assert_eq!(res.result.collection_created, Some(true));
        assert_eq!(res.result.endpoint_created, Some(true));

        // the endpoint row
        assert_eq!(s.count("SELECT COUNT(*) FROM endpoints"), 1);
        let rows: Vec<(String, String, Option<String>)> = s
            .core()
            .cproc("SELECT endpoint_str, method, annotation FROM endpoints", &[], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap();
        assert_eq!(rows[0], ("https://api.example.test/v1/login".to_string(), "POST".to_string(), Some("log in".to_string())));

        // the three files, in the endpoint's own folder
        let dir = s.root.join("storage").join("globalEQPData").join(&eid);
        let read = |kind: &str| std::fs::read_to_string(dir.join(format!("{eid}-{kind}-1.json"))).unwrap();
        assert_eq!(read("request"), r#"{"user":"alice"}"#);
        assert_eq!(read("response"), r#"{"token":"xyz"}"#);
        assert_eq!(
            read("headers"),
            r#"{"request_headers":{"X-A":"1"},"response_headers":{"content-type":"application/json"}}"#
        );

        // the metadata rows; no status or timing, since nothing was sent
        assert_eq!(s.count("SELECT COUNT(*) FROM request_metadata"), 1);
        assert_eq!(s.count("SELECT COUNT(*) FROM response_metadata WHERE status_code IS NULL AND response_time_ms IS NULL"), 1);
        // an insert is not a run
        assert_eq!(s.count("SELECT COUNT(*) FROM history"), 0);

        // the collection: file, catalog row with its annotation, bookmark, member
        assert!(Path::new(&s.collection_file("auth")).exists());
        let cat: Vec<(String, Option<String>)> = s
            .core()
            .cproc("SELECT file_path, annotation FROM collections WHERE name = 'auth'", &[], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!(cat[0], (s.collection_file("auth"), Some("login flows".to_string())));
        assert_eq!(s.count("SELECT COUNT(*) FROM bookmarks WHERE folder = 'auth' AND notes = 'first'"), 1);
        assert_eq!(s.membership("auth").list_ids().unwrap(), vec![eid.clone()]);

        // the collection's size is the real size on disk of its QP files
        assert!(s.eqp_bytes(&eid) > 0);
        assert_eq!(s.membership("auth").size_bytes().unwrap(), Some(s.eqp_bytes(&eid)));
    }

    #[tokio::test]
    async fn the_same_qp_twice_is_a_duplicate_and_changes_nothing() {
        let s = scratch();
        let mk = || insert_req("auth", "https://api.example.test/v1/login", json!({"user": "alice"}), json!({"token": "xyz"}));
        let first = process_collection_op(mk(), &mut s.storage(), never).await;
        let eid = first.result.endpoint_id.clone().unwrap();
        let size = s.membership("auth").size_bytes().unwrap();

        let again = insert_req("auth", "https://api.example.test/v1/login", json!({"user": "alice"}), json!({"token": "xyz"}));
        let second = process_collection_op(again, &mut s.storage(), never).await;
        assert_eq!(second.result.status, ApiStatus::DuplicateQp);
        assert_eq!(second.result.duplicate_of, Some(1));
        assert_eq!(second.result.endpoint_id.as_deref(), Some(eid.as_str()));
        assert_eq!(second.result.endpoint_created, Some(false));
        assert_eq!(second.result.collection_created, Some(false));

        assert_eq!(s.count("SELECT COUNT(*) FROM request_metadata"), 1);
        assert_eq!(s.membership("auth").size_bytes().unwrap(), size);
        assert_eq!(s.membership("auth").list_ids().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_different_qp_for_the_same_endpoint_is_number_two_and_grows_the_size() {
        let s = scratch();
        let url = "https://api.example.test/v1/login";
        let a = process_collection_op(insert_req("auth", url, json!({"user": "alice"}), json!({"token": "xyz"})), &mut s.storage(), never).await;
        let eid = a.result.endpoint_id.clone().unwrap();
        let b = process_collection_op(insert_req("auth", url, json!({"user": "bob"}), json!({"token": "a-much-longer-token-value"})), &mut s.storage(), never).await;

        assert_eq!(b.result.status, ApiStatus::Ok);
        assert_eq!(b.result.request_number, Some(2));
        assert_eq!(b.result.endpoint_created, Some(false), "found by URL + method, not created again");
        assert_eq!(b.result.endpoint_id.as_deref(), Some(eid.as_str()));
        assert_eq!(s.count("SELECT COUNT(*) FROM endpoints"), 1);
        assert_eq!(s.membership("auth").size_bytes().unwrap(), Some(s.eqp_bytes(&eid)));
    }

    #[tokio::test]
    async fn a_new_qp_grows_every_collection_that_holds_the_endpoint() {
        let s = scratch();
        let url = "https://api.example.test/v1/login";
        let a = process_collection_op(insert_req("one", url, json!({"n": 1}), json!({"r": 1})), &mut s.storage(), never).await;
        let eid = a.result.endpoint_id.clone().unwrap();
        // the same endpoint joins a second collection (its QP is a duplicate)
        let dup = process_collection_op(insert_req("two", url, json!({"n": 1}), json!({"r": 1})), &mut s.storage(), never).await;
        assert_eq!(dup.result.status, ApiStatus::DuplicateQp);
        assert_eq!(s.membership("two").size_bytes().unwrap(), Some(s.eqp_bytes(&eid)));

        // a new QP, pushed through the first collection, grows both
        process_collection_op(insert_req("one", url, json!({"n": 2}), json!({"r": "two"})), &mut s.storage(), never).await;
        assert_eq!(s.membership("one").size_bytes().unwrap(), Some(s.eqp_bytes(&eid)));
        assert_eq!(s.membership("two").size_bytes().unwrap(), Some(s.eqp_bytes(&eid)));
    }

    #[tokio::test]
    async fn an_old_collection_without_a_stored_size_is_counted_once() {
        let s = scratch();
        let url = "https://api.example.test/v1/a";
        let a = process_collection_op(insert_req("old", url, json!({"n": 1}), json!({"r": 1})), &mut s.storage(), never).await;
        let eid_a = a.result.endpoint_id.clone().unwrap();
        // pretend the collection predates sizes: drop its stored size
        s.membership("old").core.proc("DELETE FROM collection_stats", &[]).unwrap();
        assert_eq!(s.membership("old").size_bytes().unwrap(), None);

        let b = process_collection_op(insert_req("old", "https://api.example.test/v1/b", json!({"n": 1}), json!({"r": 1})), &mut s.storage(), never).await;
        let eid_b = b.result.endpoint_id.clone().unwrap();
        assert_eq!(
            s.membership("old").size_bytes().unwrap(),
            Some(s.eqp_bytes(&eid_a) + s.eqp_bytes(&eid_b)),
            "the old member counted once, the new one added"
        );
    }

    #[tokio::test]
    async fn test_and_insert_stores_a_404_and_a_history_row() {
        let s = scratch();
        let req = CollectionOpRequest {
            ops: "test_and_insert".to_string(),
            endpoint_str: "https://api.example.test:8443/v1/missing?x=1".to_string(),
            collection: "checks".to_string(),
            method: Some("GET".to_string()),
            headers: Some(json!({"Authorization": "Bearer t"})),
            ..Default::default()
        };
        let res = process_collection_op(req, &mut s.storage(), |call| async move {
            assert_eq!(call.headers, Some(json!({"Authorization": "Bearer t"})));
            Ok(answer(404, json!({"error": "not found"})))
        })
        .await;
        assert_eq!(res.result.status, ApiStatus::Ok, "a 404 is a completed test, not a failure");
        let eid = res.result.endpoint_id.clone().unwrap();

        let rows: Vec<(Option<i32>, Option<i32>)> = s
            .core()
            .cproc("SELECT status_code, response_time_ms FROM response_metadata", &[], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!(rows, vec![(Some(404), Some(87))]);

        let hist: Vec<(String, String, Option<String>, Option<String>)> = s
            .core()
            .cproc("SELECT endpoint_id, action, details, base_url FROM history", &[], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap();
        assert_eq!(
            hist,
            vec![(eid.clone(), "test".to_string(), Some("404 in 87ms".to_string()), Some("https://api.example.test:8443".to_string()))]
        );

        let dir = s.root.join("storage").join("globalEQPData").join(&eid);
        let headers = std::fs::read_to_string(dir.join(format!("{eid}-headers-1.json"))).unwrap();
        assert_eq!(
            headers,
            r#"{"request_headers":{"Authorization":"Bearer t"},"response_headers":{"content-type":"application/json"}}"#
        );
        assert_eq!(std::fs::read_to_string(dir.join(format!("{eid}-response-1.json"))).unwrap(), r#"{"error":"not found"}"#);
        assert_eq!(s.membership("checks").list_ids().unwrap(), vec![eid]);
    }

    #[tokio::test]
    async fn a_test_that_times_out_leaves_nothing_behind() {
        let s = scratch();
        let req = CollectionOpRequest {
            ops: "test_and_insert".to_string(),
            endpoint_str: "https://slow.example.test/".to_string(),
            collection: "checks".to_string(),
            method: Some("GET".to_string()),
            ..Default::default()
        };
        let res = process_collection_op(req, &mut s.storage(), |_| async {
            Err(HttpFailure::TimedOut("timed out".to_string()))
        })
        .await;
        assert_eq!(res.result.status, ApiStatus::TestTimedOut);
        assert_eq!(s.count("SELECT COUNT(*) FROM endpoints"), 0);
        assert_eq!(s.count("SELECT COUNT(*) FROM collections"), 0);
        assert_eq!(s.count("SELECT COUNT(*) FROM history"), 0);
        assert!(!Path::new(&s.collection_file("checks")).exists());
    }

    #[tokio::test]
    async fn endpoints_get_distinct_real_eids_and_a_given_eid_is_honoured() {
        let s = scratch();
        let a = process_collection_op(insert_req("c", "https://x.test/a", json!({}), json!({})), &mut s.storage(), never).await;
        let b = process_collection_op(insert_req("c", "https://x.test/b", json!({}), json!({})), &mut s.storage(), never).await;
        assert_ne!(a.result.endpoint_id, b.result.endpoint_id);

        let mut given = insert_req("c", "https://x.test/c", json!({}), json!({}));
        given.endpoint_id = Some("E0099-AAA".to_string());
        let c = process_collection_op(given, &mut s.storage(), never).await;
        assert_eq!(c.result.endpoint_id.as_deref(), Some("E0099-AAA"));

        let mut bad = insert_req("c", "https://x.test/d", json!({}), json!({}));
        bad.endpoint_id = Some("not-an-eid".to_string());
        let d = process_collection_op(bad, &mut s.storage(), never).await;
        assert_eq!(d.result.status, ApiStatus::InternalError);
        assert_eq!(s.count("SELECT COUNT(*) FROM endpoints"), 3, "a bad EID creates nothing");
    }

    /// `n` calls at once, each with its own connection, as parallel `edms-child`
    /// processes have.
    fn in_parallel(s: &Scratch, n: usize, make: impl Fn(usize) -> CollectionOpRequest + Sync) -> Vec<crate::api_status::ApiResult> {
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..n)
                .map(|i| {
                    let (db, root, make) = (&s.db, &s.root, &make);
                    scope.spawn(move || {
                        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
                        let mut storage = CollectionStore::new(QpStore::open(db, root).unwrap());
                        rt.block_on(process_collection_op(make(i), &mut storage, never)).result
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        })
    }

    #[test]
    fn parallel_calls_to_one_endpoint_keep_the_numbers_and_the_size_exact() {
        let s = scratch();
        let results = in_parallel(&s, 8, |i| {
            insert_req("par", "https://x.test/par", json!({"i": i}), json!({"r": i * 11}))
        });
        assert!(results.iter().all(|r| r.status == ApiStatus::Ok), "{results:?}");
        let mut numbers: Vec<i32> = results.iter().map(|r| r.request_number.unwrap()).collect();
        numbers.sort();
        assert_eq!(numbers, (1..=8).collect::<Vec<_>>(), "every call got its own number");
        assert_eq!(s.count("SELECT COUNT(*) FROM endpoints"), 1);
        assert_eq!(s.count("SELECT COUNT(*) FROM request_metadata"), 8);
        let eid = results[0].endpoint_id.clone().unwrap();
        assert_eq!(s.membership("par").size_bytes().unwrap(), Some(s.eqp_bytes(&eid)));
    }

    #[test]
    fn parallel_identical_calls_store_the_qp_once() {
        let s = scratch();
        let results = in_parallel(&s, 6, |_| {
            insert_req("same", "https://x.test/same", json!({"a": 1}), json!({"b": 2}))
        });
        let stored = results.iter().filter(|r| r.status == ApiStatus::Ok).count();
        let dups = results.iter().filter(|r| r.status == ApiStatus::DuplicateQp).count();
        assert_eq!((stored, dups), (1, 5), "{results:?}");
        assert_eq!(s.count("SELECT COUNT(*) FROM request_metadata"), 1);
        assert_eq!(s.count("SELECT COUNT(*) FROM endpoints"), 1, "one endpoint, however many raced to create it");
        assert_eq!(s.membership("same").list_ids().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn the_same_url_with_another_method_is_another_endpoint() {
        let s = scratch();
        let url = "https://x.test/users";
        let post = process_collection_op(insert_req("c", url, json!({}), json!({})), &mut s.storage(), never).await;
        let mut get = insert_req("c", url, json!({}), json!({}));
        get.method = Some("GET".to_string());
        let get = process_collection_op(get, &mut s.storage(), never).await;
        assert_ne!(post.result.endpoint_id, get.result.endpoint_id);
        assert_eq!(get.result.endpoint_created, Some(true));
    }

    #[tokio::test]
    async fn a_second_call_reuses_the_collection_and_keeps_its_annotation() {
        let s = scratch();
        let mut first = insert_req("c", "https://x.test/a", json!({}), json!({}));
        first.collection_annotation = Some("original".to_string());
        process_collection_op(first, &mut s.storage(), never).await;

        let mut second = insert_req("c", "https://x.test/b", json!({}), json!({}));
        second.collection_annotation = Some("ignored".to_string());
        let res = process_collection_op(second, &mut s.storage(), never).await;
        assert_eq!(res.result.collection_created, Some(false));

        let ann: Vec<Option<String>> = s.core().cproc("SELECT annotation FROM collections WHERE name = 'c'", &[], |r| r.get(0)).unwrap();
        assert_eq!(ann, vec![Some("original".to_string())]);
        assert_eq!(s.membership("c").list_ids().unwrap().len(), 2);
    }
}
