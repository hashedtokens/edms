//! The Merge dropdown's two operations as one job for the webserver:
//!
//! - **Collection**: merge whole collections into a target
//!   (`sources[].tags` empty).
//! - **Tag**: merge only the endpoints carrying chosen tags
//!   (`sources[].tags` filled in).
//!
//! Both are `merge_by_tags_in`; this wraps it so the caller gets a structured
//! result - what was added, what the target held before and after, the
//! Activity Log - instead of just the log text.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::tagops::{
    db::{
        endpoints_with_tags_in_collection_in, open_core, read_collection_member_eids_in,
        resolve_collection_sqlite_path_in,
    },
    merge::merge_by_tags_in,
    types::{ActivityEntry, CollectionTagFilter},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionMergeRequest {
    pub db_path: String,
    /// Where `storage/collections` and `storage/globalEQPData` are. The
    /// database file doesn't have to sit there (in Docker it doesn't).
    pub storage_root: PathBuf,
    pub target_collection_name: String,
    pub sources: Vec<CollectionTagFilter>,
}

/// Runs the merge and reports on it. The sources are only read.
pub fn run_collection_merge(req: &CollectionMergeRequest) -> Result<Value, String> {
    let (core, queries) = open_core(&req.db_path).map_err(|e| e.to_string())?;
    let _ = &queries;

    let target_existed = resolve_collection_sqlite_path_in(&core, &req.storage_root, &req.target_collection_name).is_some();
    let members_before = read_collection_member_eids_in(&core, &req.storage_root, &req.target_collection_name)
        .map_err(|e| e.to_string())?
        .len();

    // How many endpoints each source offers the merge (before any write).
    let mut offered = Vec::new();
    for src in &req.sources {
        let n = endpoints_with_tags_in_collection_in(&core, &req.storage_root, &src.collection_id, &src.tags)
            .map_err(|e| e.to_string())?
            .len();
        offered.push(json!({ "collection": src.collection_id, "tags": src.tags, "endpoints": n }));
    }

    let mut log: Vec<ActivityEntry> = Vec::new();
    merge_by_tags_in(&req.db_path, &req.storage_root, &req.target_collection_name, &req.sources, &mut log)
        .map_err(|e| e.to_string())?;

    let members_after = read_collection_member_eids_in(&core, &req.storage_root, &req.target_collection_name)
        .map_err(|e| e.to_string())?
        .len();

    Ok(json!({
        "ok": true,
        "target": req.target_collection_name,
        // a merge that matched nothing creates nothing
        "target_created": !target_existed && members_after > 0,
        "target_existed": target_existed,
        "members_before": members_before,
        "members_after": members_after,
        "added": members_after.saturating_sub(members_before),
        "sources": offered,
        "log": log,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_round_trips_the_way_the_webserver_sends_it() {
        let req: CollectionMergeRequest = serde_json::from_value(json!({
            "job_id": "ignored-extra-field",
            "db_path": "/app/data/edms.db",
            "storage_root": "/app/edms_root",
            "target_collection_name": "all",
            "sources": [{ "collection_id": "a", "tags": [] }, { "collection_id": "b", "tags": ["read-only"] }]
        }))
        .unwrap();
        assert_eq!(req.storage_root, PathBuf::from("/app/edms_root"));
        assert_eq!(req.sources.len(), 2);
        assert!(req.sources[0].tags.is_empty());
        assert_eq!(req.sources[1].tags, vec!["read-only".to_string()]);
    }

    #[test]
    fn a_missing_storage_root_is_a_bad_request_not_a_guess() {
        let r: Result<CollectionMergeRequest, _> = serde_json::from_value(json!({
            "db_path": "/x/edms.db", "target_collection_name": "t", "sources": []
        }));
        assert!(r.is_err(), "the webserver must say where the storage is");
    }
}
