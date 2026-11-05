//! Storing one QP in the central database and on disk, the way a Run does it
//! in the webserver, but from the compute side (the Dev API ops run in
//! `edms-child`, which has no `AppState`).
//!
//! One `QpStore` is one connection to `edms.db` plus the storage root. It
//! knows how to find or create an endpoint (real EIDs from the allocator),
//! check a QP against the ones the endpoint already has, store a QP (the
//! three files under `storage/globalEQPData/{EID}/` and the request/response
//! rows) and record a History row. It is shared by the Dev API ops, so the
//! history op and the collections ops cannot drift apart.
//!
//! A QP's size is part of every Collection that holds its endpoint
//! (`collection_stats.size_bytes`, kept incrementally), so storing a QP also
//! reports the change to those collections.

use crate::eid::{is_valid_eid, EidAllocator};
use crate::endpoint_writer::{write_headers_file, write_request_file, write_response_file};
use crate::qp_service;
use crate::table_view::compute_size;
use edms::core::EdmsCore;
use edms::ops::collection_membership_ops::CollectionMembershipOps;
use edms::ops::view_ops::{ViewCatalogOps, ViewKind};
use edms::query_loader::QueryMap;
use std::path::{Path, PathBuf};

fn err<E: std::fmt::Debug>(e: E) -> String {
    format!("{e:?}")
}

/// A QP ready to be stored. The three texts are exactly what goes in the
/// files: `request` and `response` are JSON, `headers` is
/// `{"request_headers":..,"response_headers":..}`.
pub struct NewQp<'a> {
    pub endpoint_id: &'a str,
    /// The endpoint's URL, for the duplicate check's fingerprint
    pub endpoint_str: &'a str,
    pub method: &'a str,
    pub request: &'a str,
    pub response: &'a str,
    pub headers: &'a str,
    /// `None` when no real HTTP call happened (a QP inserted by hand).
    pub status_code: Option<i32>,
    pub response_time_ms: Option<i32>,
}

/// What `store_qp` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stored {
    /// Stored as this request number
    New(i32),
    /// An identical QP already exists (this number); nothing was stored
    Duplicate(i32),
}

pub struct QpStore {
    core: EdmsCore,
    queries: QueryMap,
    db_path: String,
    root: PathBuf,
    allocator: EidAllocator,
}

impl QpStore {
    /// Opens `db_path` (the central database the webserver already set up)
    /// with `root` as the storage root (the folder that holds `storage/`).
    pub fn open(db_path: &str, root: &Path) -> Result<Self, String> {
        let core = EdmsCore::new(db_path);
        core.connect()
            .map_err(|e| format!("can't open the database {db_path}: {e:?}"))?;
        Ok(QpStore {
            core,
            queries: QueryMap::load(),
            db_path: db_path.to_string(),
            root: root.to_path_buf(),
            allocator: EidAllocator::new(db_path),
        })
    }

    pub fn core(&self) -> &EdmsCore {
        &self.core
    }

    pub fn queries(&self) -> &QueryMap {
        &self.queries
    }

    pub fn db_path(&self) -> &str {
        &self.db_path
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `storage/globalEQPData/{eid}`: where one endpoint's QP files live.
    pub fn eqp_dir(&self, eid: &str) -> PathBuf {
        self.root.join("storage").join("globalEQPData").join(eid)
    }

    /// Total size in bytes of one endpoint's QP files (0 if it has none).
    pub fn endpoint_size(&self, eid: &str) -> i64 {
        compute_size(&self.eqp_dir(eid)) as i64
    }

    // ── endpoints ────────────────────────────────────────────────────────

    fn endpoint_str_of(&self, eid: &str) -> Result<Option<String>, String> {
        let q = self.queries.get_endpoint_query("E2").ok_or("query E2 is missing")?;
        let rows: Vec<String> = self.core.cproc(q, &[&eid], |row| row.get(1)).map_err(err)?;
        Ok(rows.into_iter().next())
    }

    /// Get-or-create, the same rules as a Run in Test View: an `endpoint_id`
    /// that exists is used as it is; otherwise an endpoint with the same URL
    /// and method is reused; otherwise a new row is made, with `endpoint_id`
    /// if one was given (it must be a canonical EID) or a fresh EID from the
    /// allocator. Returns `(eid, created)`.
    pub fn find_or_create_endpoint(
        &self,
        endpoint_id: Option<&str>,
        endpoint_str: &str,
        method: &str,
        annotation: Option<&str>,
    ) -> Result<(String, bool), String> {
        let wanted = endpoint_id.map(str::trim).filter(|s| !s.is_empty());
        if let Some(id) = wanted {
            if self.endpoint_str_of(id)?.is_some() {
                return Ok((id.to_string(), false));
            }
        }

        let q8 = self.queries.get_endpoint_query("E8").ok_or("query E8 is missing")?;
        let found: Vec<String> = self
            .core
            .cproc(q8, &[&endpoint_str, &method], |row| row.get(0))
            .map_err(err)?;
        if let Some(existing) = found.into_iter().next() {
            return Ok((existing, false));
        }

        // The EID comes from the allocator before the write lock is taken (the
        // allocator has its own transaction on the same file).
        let (eid, allocated) = match wanted {
            None => (self.allocator.allocate().map_err(|e| format!("can't allocate an EID: {e}"))?, true),
            Some(custom) => {
                if !is_valid_eid(custom) {
                    return Err(format!(
                        "'{custom}' is not a canonical EID (expected something like E0001-AAA)"
                    ));
                }
                self.allocator
                    .reserve(custom)
                    .map_err(|e| format!("can't reserve EID '{custom}': {e}"))?;
                (custom.to_string(), false)
            }
        };

        // Look again, and insert, in one locked section: of several calls
        // creating the same endpoint at once, one makes it and the rest find it.
        let q1 = self.queries.get_endpoint_query("E1").ok_or("query E1 is missing")?;
        let method_param: Option<&str> = Some(method);
        let made = self.locked(|| {
            let again: Vec<String> = self
                .core
                .cproc(q8, &[&endpoint_str, &method], |row| row.get(0))
                .map_err(err)?;
            if let Some(existing) = again.into_iter().next() {
                return Ok(Err(existing));
            }
            self.core
                .proc(q1, &[&eid, &endpoint_str, &annotation, &method_param])
                .map_err(|e| format!("can't create endpoint {eid}: {e:?}"))?;
            Ok(Ok(()))
        });
        match made {
            Ok(Ok(())) => Ok((eid, true)),
            Ok(Err(existing)) => {
                // Another call made it first: give back the EID taken for it.
                if allocated {
                    let _ = self.allocator.release(&eid);
                }
                Ok((existing, false))
            }
            Err(e) => {
                if allocated {
                    let _ = self.allocator.release(&eid);
                }
                Err(e)
            }
        }
    }

    /// Takes back an endpoint this call just created, after a later step
    /// failed, so a failed call does not leave a half-made endpoint (or its
    /// EID) behind.
    pub fn discard_endpoint(&self, eid: &str) -> Result<(), String> {
        for sql in [
            "DELETE FROM request_metadata WHERE endpoint_id = ?",
            "DELETE FROM response_metadata WHERE endpoint_id = ?",
            "DELETE FROM history WHERE endpoint_id = ?",
            "DELETE FROM tags WHERE endpoint_id = ?",
            "DELETE FROM metadata WHERE endpoint_id = ?",
            "DELETE FROM bookmarks WHERE endpoint_id = ?",
            "DELETE FROM endpoints WHERE endpoint_id = ?",
        ] {
            self.core.proc(sql, &[&eid]).map_err(err)?;
        }
        let dir = self.eqp_dir(eid);
        if dir.exists() {
            let _ = std::fs::remove_dir_all(&dir);
        }
        let _ = self.allocator.release(eid);
        Ok(())
    }

    // ── QPs ──────────────────────────────────────────────────────────────

    /// The number of an identical QP the endpoint already has, if any (the
    /// same fingerprint rules as a Run: see `qp_service`).
    pub fn find_duplicate(
        &self,
        eid: &str,
        method: &str,
        endpoint_str: &str,
        request: &str,
        response: &str,
        headers: &str,
    ) -> Option<i32> {
        qp_service::find_duplicate_qp(
            &self.eqp_dir(eid),
            eid,
            method,
            endpoint_str,
            request,
            response,
            headers,
            None,
        )
        .filter(|n| *n > 0)
    }

    /// Runs `f` while holding the database's write lock (waiting for it if
    /// another writer has it: the webserver, or another Dev API call). A
    /// step that reads the state of the disk and then changes it has to be
    /// one such section, or two parallel calls act on the same picture of it
    /// (they take the same request number, both store the same QP, count each
    /// other's files twice). The database changes are rolled back if `f`
    /// fails.
    pub fn locked<T>(&self, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        self.core.base.execute("BEGIN IMMEDIATE", &[]).map_err(err)?;
        match f() {
            Ok(v) => match self.core.base.execute("COMMIT", &[]) {
                Ok(_) => Ok(v),
                Err(e) => {
                    let _ = self.core.base.execute("ROLLBACK", &[]);
                    Err(err(e))
                }
            },
            Err(e) => {
                let _ = self.core.base.execute("ROLLBACK", &[]);
                Err(e)
            }
        }
    }

    /// Stores a QP, unless the endpoint already has an identical one. All in
    /// one locked section: the duplicate check, the next request number, the
    /// three files, the request/response rows, and telling every Collection
    /// that holds the endpoint how many bytes it grew by (exactly the bytes
    /// of this QP's files).
    pub fn store_qp(&self, qp: &NewQp) -> Result<Stored, String> {
        let eid = qp.endpoint_id;
        let dir = self.eqp_dir(eid);
        let q6 = self.queries.get_request_query("R6").ok_or("query R6 is missing")?;
        let q1 = self.queries.get_request_query("R1").ok_or("query R1 is missing")?;
        let q_res = self.queries.get_response_query("RES1").ok_or("query RES1 is missing")?;
        let reserved = std::cell::Cell::new(0_i32);

        let stored = self.locked(|| {
            if let Some(dup) =
                self.find_duplicate(eid, qp.method, qp.endpoint_str, qp.request, qp.response, qp.headers)
            {
                return Ok(Stored::Duplicate(dup));
            }

            // A number past anything the database or the disk already has.
            let on_disk = qp_service::list_existing_qp_numbers(&dir, eid).into_iter().max().unwrap_or(0);
            let rows: Vec<Option<i32>> = self.core.cproc(q6, &[&eid], |row| row.get(0)).map_err(err)?;
            let number = rows.first().copied().flatten().unwrap_or(0).max(on_disk) + 1;
            reserved.set(number);

            let n = number as usize;
            write_request_file(&dir, eid, n, qp.request).map_err(|e| format!("can't write the request file: {e}"))?;
            write_response_file(&dir, eid, n, qp.response).map_err(|e| format!("can't write the response file: {e}"))?;
            write_headers_file(&dir, eid, n, qp.headers).map_err(|e| format!("can't write the headers file: {e}"))?;

            let request_path = dir.join(format!("{eid}-request-{number}.json")).display().to_string();
            let response_path = dir.join(format!("{eid}-response-{number}.json")).display().to_string();
            self.core.proc(q1, &[&eid, &number, &request_path, &qp.method]).map_err(err)?;
            // Same as the webserver's insert_response_metadata: replace any row
            // already there for this number, then insert.
            self.core
                .proc(
                    "DELETE FROM response_metadata WHERE endpoint_id = ? AND request_number = ?",
                    &[&eid, &number],
                )
                .map_err(err)?;
            self.core
                .proc(q_res, &[&eid, &number, &response_path, &qp.status_code, &qp.response_time_ms])
                .map_err(err)?;

            let added: i64 = ["request", "response", "headers"]
                .iter()
                .map(|kind| {
                    std::fs::metadata(dir.join(format!("{eid}-{kind}-{number}.json")))
                        .map(|m| m.len() as i64)
                        .unwrap_or(0)
                })
                .sum();
            self.adjust_holding_collections(eid, added);
            Ok(Stored::New(number))
        });

        if stored.is_err() && reserved.get() > 0 {
            // The rows rolled back with the transaction; the files did not.
            for kind in ["request", "response", "headers"] {
                let _ = std::fs::remove_file(dir.join(format!("{eid}-{kind}-{}.json", reserved.get())));
            }
        }
        stored
    }

    // ── History ──────────────────────────────────────────────────────────

    /// One History row, with the endpoint's base URL (the domain) filled in,
    /// as every insert path of the webserver does.
    pub fn record_history(&self, eid: &str, action: &str, details: Option<&str>) -> Result<(), String> {
        let q = self.queries.get_history_query("H3").ok_or("query H3 is missing")?;
        let base_url = self
            .endpoint_str_of(eid)?
            .and_then(|s| edms::base_url::base_url_of(&s));
        self.core.proc(q, &[&eid, &action, &details, &base_url]).map_err(err)?;
        Ok(())
    }

    // ── Collection sizes ─────────────────────────────────────────────────

    /// Every Collection that lists `eid` as a member changes by `delta` bytes.
    /// A size that can't be updated never fails the call that caused it: it
    /// is a display figure, not data.
    pub fn adjust_holding_collections(&self, eid: &str, delta: i64) {
        if delta == 0 {
            return;
        }
        let catalog = ViewCatalogOps::new(&self.db_path);
        if catalog.initialize().is_err() {
            return;
        }
        let Ok(rows) = catalog.list(ViewKind::Collections) else { return };
        for (_name, file_path, _created_at, _annotation) in rows {
            let Some(path) = file_path else { continue };
            if !Path::new(&path).exists() {
                continue;
            }
            let membership = CollectionMembershipOps::new(&path);
            if membership.initialize().is_err() {
                continue;
            }
            if membership.list_set().map(|m| m.contains(eid)).unwrap_or(false) {
                let _ = membership.adjust_size_bytes(delta);
            }
        }
    }
}
