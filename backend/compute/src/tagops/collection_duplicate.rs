//! Duplicate a Collection: a new collection that is a copy of an existing one,
//! with everything in it copied too, so the copy and the original can be
//! changed independently afterwards.
//!
//! What the copy gets:
//!
//! - every endpoint of the original under a **fresh EID** (the endpoint row
//!   with its URL, method and annotation, and its tags),
//! - every QP of those endpoints, with the same numbers: the three saved
//!   files, and the `request_metadata` / `response_metadata` rows (status
//!   code, timing, dates),
//! - the collection itself: its annotation, its membership (and the tags
//!   carried in its own file), its bookmarks with their notes, its collection
//!   tags, and its size,
//!
//! and nothing from History: a copy has not been run.
//!
//! A merge into a new name is not this: it drops the QP rows and the notes,
//! renumbers the QPs, and folds endpoints that share a URL and method into
//! one. A duplicate is exact.
//!
//! All or nothing: the files and the membership file are written first, the
//! database rows (the catalog row last) in one transaction. A failure removes
//! what was written and hands the EIDs back, so nothing half-made is left.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use edms::ops::collection_membership_ops::CollectionMembershipOps;

use crate::eid::allocator::EidAllocator;
use crate::tagops::db::{
    bulk_fetch_endpoint_details, bulk_fetch_endpoint_tags, open_core, resolve_collection_sqlite_path_in,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionDuplicateRequest {
    pub db_path: String,
    /// Where `storage/collections` and `storage/globalEQPData` are (the
    /// database file doesn't have to sit there - in Docker it doesn't).
    pub storage_root: PathBuf,
    pub source_collection_name: String,
    pub target_collection_name: String,
}

const KINDS: [&str; 3] = ["request", "response", "headers"];

type RequestRow = (String, i64, Option<String>, Option<String>);
type ResponseRow = (String, i64, Option<i64>, Option<i64>, Option<i64>, Option<String>);

/// What was written so far, so a failure can take it back.
#[derive(Default)]
struct Written {
    dirs: Vec<PathBuf>,
    membership_file: Option<PathBuf>,
    eids: Vec<String>,
}

impl Written {
    fn undo(&self, allocator: &EidAllocator) {
        for dir in &self.dirs {
            let _ = std::fs::remove_dir_all(dir);
        }
        if let Some(file) = &self.membership_file {
            let _ = std::fs::remove_file(file);
            for side in ["-wal", "-shm"] {
                let mut name = file.as_os_str().to_os_string();
                name.push(side);
                let _ = std::fs::remove_file(PathBuf::from(name));
            }
        }
        for eid in &self.eids {
            let _ = allocator.release(eid);
        }
    }
}

fn qp_dir(root: &Path, eid: &str) -> PathBuf {
    root.join("storage").join("globalEQPData").join(eid)
}

fn qp_file(root: &Path, eid: &str, kind: &str, number: i64) -> String {
    qp_dir(root, eid).join(format!("{eid}-{kind}-{number}.json")).display().to_string()
}

/// Copies one endpoint's saved QP files to its new EID, keeping the numbers.
/// Returns (files copied, bytes copied).
fn copy_qp_files(root: &Path, old: &str, new: &str) -> std::io::Result<(usize, u64)> {
    let src = qp_dir(root, old);
    if !src.is_dir() {
        return Ok((0, 0));
    }
    let dst = qp_dir(root, new);
    std::fs::create_dir_all(&dst)?;
    let (mut files, mut bytes) = (0usize, 0u64);
    for entry in std::fs::read_dir(&src)?.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(rest) = name.strip_prefix(&format!("{old}-")) else { continue };
        if !KINDS.iter().any(|k| {
            rest.strip_prefix(&format!("{k}-"))
                .and_then(|n| n.strip_suffix(".json"))
                .map(|n| n.parse::<u32>().is_ok())
                .unwrap_or(false)
        }) {
            continue;
        }
        let to = dst.join(format!("{new}-{rest}"));
        bytes += std::fs::copy(entry.path(), to)?;
        files += 1;
    }
    Ok((files, bytes))
}

fn in_list(n: usize) -> String {
    vec!["?"; n].join(", ")
}

/// Runs the duplicate and says what it did. The source is only read.
pub fn run_collection_duplicate(req: &CollectionDuplicateRequest) -> Result<Value, String> {
    let root = &req.storage_root;
    let source = req.source_collection_name.as_str();
    let target = req.target_collection_name.as_str();
    if source == target {
        return Err(format!("Can't duplicate '{source}' onto itself"));
    }

    let (core, _queries) = open_core(&req.db_path).map_err(|e| e.to_string())?;
    let dbe = |e: edms::error::EdmsError| format!("{e:?}");

    // the source has to exist, the target must not
    let src_file = resolve_collection_sqlite_path_in(&core, root, source)
        .ok_or_else(|| format!("Collection '{source}' does not exist"))?;
    let in_catalog: Vec<i64> = core
        .cproc("SELECT COUNT(*) FROM collections WHERE name = ?", &[&target], |r| r.get(0))
        .map_err(dbe)?;
    let target_file = root.join("storage").join("collections").join(format!("{target}.sqlite"));
    if in_catalog.first().copied().unwrap_or(0) > 0
        || resolve_collection_sqlite_path_in(&core, root, target).is_some()
        || target_file.exists()
    {
        return Err(format!("Collection '{target}' already exists"));
    }

    // ---- read everything from the source ----
    let src_ops = CollectionMembershipOps::new(src_file.to_str().ok_or("the source collection path is not valid UTF-8")?);
    src_ops.initialize().map_err(dbe)?;
    let members: Vec<String> = src_ops.list_ids().map_err(dbe)?;
    let carried_tags: Vec<(String, String)> = src_ops.list_all_endpoint_tags().map_err(dbe)?;
    let _ = src_ops.shutdown();

    let bookmarks: Vec<(String, Option<String>, Option<String>)> = core
        .cproc(
            "SELECT endpoint_id, notes, timestamp FROM bookmarks WHERE folder = ? ORDER BY timestamp",
            &[&source],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(dbe)?;

    // members first, then endpoints that are only bookmarked
    let member_set: HashSet<&String> = members.iter().collect();
    let mut wanted: Vec<String> = members.clone();
    let mut seen: HashSet<String> = members.iter().cloned().collect();
    for (eid, _, _) in &bookmarks {
        if seen.insert(eid.clone()) {
            wanted.push(eid.clone());
        }
    }

    // an endpoint that was deleted but is still listed has nothing to copy
    let details = bulk_fetch_endpoint_details(&core, &wanted).map_err(|e| e.to_string())?;
    let live: Vec<String> = wanted.iter().filter(|e| details.contains_key(*e)).cloned().collect();
    let skipped = wanted.len() - live.len();
    let tags = bulk_fetch_endpoint_tags(&core, &live).map_err(|e| e.to_string())?;

    let mut request_rows: Vec<RequestRow> = Vec::new();
    let mut response_rows: Vec<ResponseRow> = Vec::new();
    for chunk in live.chunks(500) {
        let marks = in_list(chunk.len());
        let params: Vec<&dyn rusqlite::ToSql> = chunk.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        request_rows.extend(
            core.cproc(
                &format!(
                    "SELECT endpoint_id, request_number, method, timestamp FROM request_metadata \
                     WHERE endpoint_id IN ({marks}) ORDER BY endpoint_id, request_number"
                ),
                &params,
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(dbe)?,
        );
        response_rows.extend(
            core.cproc(
                &format!(
                    "SELECT endpoint_id, request_number, status_code, exit_code, response_time_ms, timestamp \
                     FROM response_metadata WHERE endpoint_id IN ({marks}) ORDER BY endpoint_id, request_number"
                ),
                &params,
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )
            .map_err(dbe)?,
        );
    }

    let annotation: Option<String> = core
        .cproc("SELECT annotation FROM collections WHERE name = ?", &[&source], |r| r.get::<_, Option<String>>(0))
        .map_err(dbe)?
        .into_iter()
        .next()
        .flatten();
    let collection_tags: Vec<String> = core
        .cproc(
            "SELECT tagname FROM collection_tag_memberships WHERE collection_name = ? ORDER BY tagname",
            &[&source],
            |r| r.get(0),
        )
        .map_err(dbe)?;

    // ---- write the copy ----
    let allocator = EidAllocator::new(&req.db_path);
    allocator.initialize().map_err(|e| format!("EID allocator: {e:?}"))?;
    let new_eids = allocator
        .allocate_batch(live.len())
        .map_err(|e| format!("couldn't allocate EIDs: {e:?}"))?;
    let new_of: HashMap<&str, &String> = live.iter().map(String::as_str).zip(new_eids.iter()).collect();

    let mut written = Written { eids: new_eids.clone(), ..Written::default() };
    let outcome = (|| -> Result<Value, String> {
        // 1. the QP files
        let (mut files, mut member_bytes) = (0usize, 0u64);
        for (old, new) in live.iter().zip(new_eids.iter()) {
            written.dirs.push(qp_dir(root, new));
            let (n, bytes) = copy_qp_files(root, old, new).map_err(|e| format!("couldn't copy the QP files of {old}: {e}"))?;
            files += n;
            if member_set.contains(old) {
                member_bytes += bytes;
            }
        }

        // 2. the collection's own file: members, carried tags, size
        std::fs::create_dir_all(root.join("storage").join("collections"))
            .map_err(|e| format!("can't create the collections folder: {e}"))?;
        let target_file_str = target_file.to_str().ok_or("the new collection path is not valid UTF-8")?.to_string();
        written.membership_file = Some(target_file.clone());
        let ops = CollectionMembershipOps::new(&target_file_str);
        ops.initialize().map_err(|e| format!("can't create {target_file_str}: {e:?}"))?;
        let new_members: Vec<String> = members.iter().filter_map(|m| new_of.get(m.as_str()).map(|n| (*n).clone())).collect();
        ops.add_batch(&new_members).map_err(|e| format!("can't write the members: {e:?}"))?;
        for (eid, tag) in &carried_tags {
            if let Some(new) = new_of.get(eid.as_str()) {
                ops.add_tag(new, tag).map_err(|e| format!("can't write a member tag: {e:?}"))?;
            }
        }
        ops.set_size_bytes(member_bytes as i64).map_err(|e| format!("can't write the size: {e:?}"))?;
        let _ = ops.shutdown();

        // 3. the database rows, in one transaction, the catalog row last
        core.base.execute("BEGIN IMMEDIATE", &[]).map_err(|e| format!("{e:?}"))?;
        let rows = (|| -> Result<(), String> {
            let e = |x: edms::error::EdmsError| format!("{x:?}");
            for old in &live {
                let new = new_of[old.as_str()];
                let (ep_str, method, ep_annotation) = &details[old];
                core.proc(
                    "INSERT INTO endpoints (endpoint_id, endpoint_str, method, annotation) VALUES (?, ?, ?, ?)",
                    &[new, ep_str, method, ep_annotation],
                )
                .map_err(e)?;
                for tag in tags.get(old).into_iter().flatten() {
                    core.proc("INSERT OR IGNORE INTO tags (endpoint_id, tag) VALUES (?, ?)", &[new, tag]).map_err(e)?;
                }
            }
            for (old, number, method, ts) in &request_rows {
                let new = new_of[old.as_str()];
                let path = qp_file(root, new, "request", *number);
                core.proc(
                    "INSERT INTO request_metadata (endpoint_id, request_number, file_path, method, timestamp) VALUES (?, ?, ?, ?, ?)",
                    &[new, number, &path, method, ts],
                )
                .map_err(e)?;
            }
            for (old, number, status, exit, time_ms, ts) in &response_rows {
                let new = new_of[old.as_str()];
                let path = qp_file(root, new, "response", *number);
                core.proc(
                    "INSERT INTO response_metadata (endpoint_id, request_number, file_path, status_code, exit_code, response_time_ms, timestamp) \
                     VALUES (?, ?, ?, ?, ?, ?, ?)",
                    &[new, number, &path, status, exit, time_ms, ts],
                )
                .map_err(e)?;
            }
            for (old, notes, ts) in &bookmarks {
                if let Some(new) = new_of.get(old.as_str()) {
                    core.proc(
                        "INSERT OR IGNORE INTO bookmarks (endpoint_id, folder, notes, timestamp) VALUES (?, ?, ?, ?)",
                        &[*new, &target, notes, ts],
                    )
                    .map_err(e)?;
                }
            }
            for tag in &collection_tags {
                core.proc(
                    "INSERT OR IGNORE INTO collection_tag_memberships (collection_name, tagname) VALUES (?, ?)",
                    &[&target, tag],
                )
                .map_err(e)?;
            }
            core.proc(
                "INSERT INTO collections (name, file_path, annotation) VALUES (?, ?, ?)",
                &[&target, &target_file_str, &annotation],
            )
            .map_err(e)?;
            Ok(())
        })();
        match rows {
            Ok(()) => core.base.execute("COMMIT", &[]).map_err(|e| format!("{e:?}"))?,
            Err(msg) => {
                let _ = core.base.execute("ROLLBACK", &[]);
                return Err(msg);
            }
        };

        Ok(json!({
            "ok": true,
            "source": source,
            "target": target,
            "endpoints": live.len(),
            "members": new_members.len(),
            "bookmarks": bookmarks.iter().filter(|(e, _, _)| new_of.contains_key(e.as_str())).count(),
            "qps": request_rows.len(),
            "files": files,
            "size_bytes": member_bytes,
            "collection_tags": collection_tags.len(),
            "skipped_missing_endpoints": skipped,
        }))
    })();

    if outcome.is_err() {
        written.undo(&allocator);
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_round_trips_the_way_the_webserver_sends_it() {
        let req: CollectionDuplicateRequest = serde_json::from_value(json!({
            "job_id": "ignored-extra-field",
            "db_path": "/app/data/edms.db",
            "storage_root": "/app/edms_root",
            "source_collection_name": "a",
            "target_collection_name": "a (copy)"
        }))
        .unwrap();
        assert_eq!(req.target_collection_name, "a (copy)");
        assert_eq!(req.storage_root, PathBuf::from("/app/edms_root"));
    }

    #[test]
    fn a_missing_storage_root_is_a_bad_request_not_a_guess() {
        let r: Result<CollectionDuplicateRequest, _> = serde_json::from_value(json!({
            "db_path": "/x/edms.db", "source_collection_name": "a", "target_collection_name": "b"
        }));
        assert!(r.is_err());
    }
}
