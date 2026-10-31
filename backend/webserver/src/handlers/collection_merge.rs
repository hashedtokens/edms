//! Collection-level merging (Ravi, 2026-10-18): the UI's Merge dropdown,
//! `[Tag, Collection]`. Both create a NEW collection or add to ONE existing
//! one; the collections merged *from* are never touched.
//!
//! - **Collection**: `{"sources": ["C1", "C2"]}` - everything in C1 and C2.
//! - **Tag**: `{"sources": [{"collection": "C1", "tags": ["T1", "T2"]}, ...]}`
//!   - only the endpoints carrying one of the chosen tags. A top-level
//!   `"tags"` applies to every source named without tags of its own.
//!
//! The work is compute's `tagops_merge` (reads the sources, allocates fresh
//! EIDs for endpoints the target doesn't have yet, copies their QP files,
//! writes the target's membership). It runs as a job, like Import/Export: the
//! request returns `202` with a `job_id`, and the result - what was added, what
//! the target held before and after - arrives as a `ViewIoDone` event (op
//! `merge`) and on `GET /jobs/:id`.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::{
    handlers::view_catalog::{open_catalog, validate_folder_name},
    ipc::IpcCallback,
    jobs::JobStatus,
    state::AppState,
};
use edms::ops::view_ops::ViewKind;

const OP: &str = "merge";
const VIEW: &str = "collections";

/// A source: a bare collection name (merge all of it), or a collection with
/// the tags to pick from it.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum SourceSpec {
    Name(String),
    Detail {
        collection: String,
        #[serde(default)]
        tags: Vec<String>,
    },
}

#[derive(Debug, Default, Deserialize)]
pub struct MergeBody {
    #[serde(default)]
    pub sources: Vec<SourceSpec>,
    /// Applied to every source that has no tags of its own.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// One source after checking: its name and the tags to filter by (empty = all).
#[derive(Debug, PartialEq, Eq)]
pub struct Source {
    pub collection: String,
    pub tags: Vec<String>,
}

/// Turns the request into a clean list: names trimmed, tags trimmed and
/// de-duplicated, a collection named twice merged into one entry (all of it
/// wins over some of it), the target refused as its own source. Pure, so the
/// rules are testable without a server.
pub fn normalize_sources(target: &str, body: MergeBody) -> Result<Vec<Source>, String> {
    if body.sources.is_empty() {
        return Err("Pick at least one collection to merge from".to_string());
    }
    let shared = clean_tags(body.tags);

    let mut order: Vec<String> = Vec::new();
    let mut by_name: HashMap<String, Option<Vec<String>>> = HashMap::new(); // None = all of it
    for spec in body.sources {
        let (name, tags) = match spec {
            SourceSpec::Name(name) => (name, shared.clone()),
            SourceSpec::Detail { collection, tags } => {
                let own = clean_tags(tags);
                (collection, if own.is_empty() { shared.clone() } else { own })
            }
        };
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err("A source collection's name can't be empty".to_string());
        }
        if name == target {
            return Err(format!("Can't merge '{name}' into itself - pick a different target or leave it out of the sources"));
        }
        match by_name.get_mut(&name) {
            None => {
                order.push(name.clone());
                by_name.insert(name, if tags.is_empty() { None } else { Some(tags) });
            }
            Some(existing) => {
                *existing = match (existing.take(), tags.is_empty()) {
                    (None, _) | (_, true) => None,
                    (Some(mut have), false) => {
                        for t in tags {
                            if !have.contains(&t) {
                                have.push(t);
                            }
                        }
                        Some(have)
                    }
                };
            }
        }
    }
    Ok(order
        .into_iter()
        .map(|name| {
            let tags = by_name.remove(&name).flatten().unwrap_or_default();
            Source { collection: name, tags }
        })
        .collect())
}

fn clean_tags(tags: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in tags {
        let t = t.trim().to_string();
        if !t.is_empty() && !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

/// What a failed request is: a status and a message.
type Fail = (StatusCode, String);

fn bad(message: impl Into<String>) -> Fail {
    (StatusCode::BAD_REQUEST, message.into())
}

/// POST /collections/:target/merge  (and the older POST /dataview/:folder/merge,
/// which the UI's Merge dialog already calls: `folder` is the target).
pub async fn merge_collections(
    State(state): State<AppState>,
    Path(target): Path<String>,
    body: Option<Json<MergeBody>>,
) -> (StatusCode, Json<Value>) {
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        let target = target.clone();
        move || start_merge(&state, &target, body)
    })
    .await;
    match res {
        Ok(Ok(started)) => (StatusCode::ACCEPTED, Json(started)),
        Ok(Err((status, error))) => (status, Json(json!({ "ok": false, "error": error }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": format!("join error: {e}") })),
        ),
    }
}

fn start_merge(state: &AppState, target: &str, body: MergeBody) -> Result<Value, Fail> {
    let target = target.trim();
    validate_folder_name(target, "Collection name").map_err(bad)?;
    let sources = normalize_sources(target, body).map_err(bad)?;

    // Every source has to exist; the target may or may not.
    let catalog = open_catalog(state).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    for s in &sources {
        let found = catalog
            .get(ViewKind::Collections, &s.collection)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;
        if found.is_none() {
            return Err((StatusCode::NOT_FOUND, format!("Collection '{}' does not exist", s.collection)));
        }
    }
    let target_exists = catalog
        .get(ViewKind::Collections, target)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?
        .is_some();

    // Two merges into the same collection at once would race on its file.
    let busy = state
        .jobs
        .list()
        .iter()
        .any(|j| j.status == JobStatus::Running && j.op == OP && j.name.as_deref() == Some(target));
    if busy {
        return Err((StatusCode::CONFLICT, format!("A merge into '{target}' is already running")));
    }

    let job_id = state.jobs.create(OP, Some(VIEW), Some(target));
    let started = crate::ipc::spawn_child(
        "collection_merge",
        json!({
            "job_id": job_id,
            "db_path": state.db_path.display().to_string(),
            "storage_root": state.storage_root,
            "target_collection_name": target,
            "sources": sources
                .iter()
                .map(|s| json!({ "collection_id": s.collection, "tags": s.tags }))
                .collect::<Vec<_>>(),
        }),
        3000,
    );
    if !started {
        let error = "couldn't start the compute process (is edms-child built?)".to_string();
        if let Some(job) = state.jobs.fail(&job_id, error.clone()) {
            crate::handlers::jobs::emit_done(state, &job);
        }
        return Err((StatusCode::INTERNAL_SERVER_ERROR, error));
    }

    Ok(json!({
        "ok": true,
        "status": "running",
        "job_id": job_id,
        "target": target,
        "folder": target,
        "target_exists": target_exists,
        "mode": if sources.iter().all(|s| s.tags.is_empty()) { "collection" } else { "tag" },
        "sources": sources
            .iter()
            .map(|s| json!({ "collection": s.collection, "tags": s.tags }))
            .collect::<Vec<_>>(),
    }))
}

/// The merge child reported back: complete the job (the UI hears of it as a
/// `ViewIoDone`) and refresh the dashboard's counts - a merge adds endpoints.
pub fn finish_merge(state: &AppState, callback: &IpcCallback) {
    crate::handlers::jobs::complete_from_callback(state, callback);
    state.refresh_dashboard_snapshot();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(items: &[&str]) -> Vec<SourceSpec> {
        items.iter().map(|s| SourceSpec::Name(s.to_string())).collect()
    }

    fn body(sources: Vec<SourceSpec>, tags: &[&str]) -> MergeBody {
        MergeBody { sources, tags: tags.iter().map(|s| s.to_string()).collect() }
    }

    fn src(collection: &str, tags: &[&str]) -> Source {
        Source { collection: collection.to_string(), tags: tags.iter().map(|s| s.to_string()).collect() }
    }

    #[test]
    fn bare_names_mean_the_whole_collection() {
        let out = normalize_sources("t", body(names(&["a", "b"]), &[])).unwrap();
        assert_eq!(out, vec![src("a", &[]), src("b", &[])]);
    }

    #[test]
    fn a_source_can_carry_its_own_tags() {
        let b = body(
            vec![
                SourceSpec::Detail { collection: "a".into(), tags: vec!["x".into(), "y".into()] },
                SourceSpec::Detail { collection: "b".into(), tags: vec![] },
            ],
            &[],
        );
        assert_eq!(normalize_sources("t", b).unwrap(), vec![src("a", &["x", "y"]), src("b", &[])]);
    }

    #[test]
    fn top_level_tags_apply_to_sources_without_their_own() {
        let b = body(
            vec![
                SourceSpec::Name("a".into()),
                SourceSpec::Detail { collection: "b".into(), tags: vec!["own".into()] },
            ],
            &["shared"],
        );
        assert_eq!(normalize_sources("t", b).unwrap(), vec![src("a", &["shared"]), src("b", &["own"])]);
    }

    #[test]
    fn names_and_tags_are_trimmed_and_tags_deduplicated() {
        let b = body(
            vec![SourceSpec::Detail { collection: "  a ".into(), tags: vec![" x".into(), "x".into(), "  ".into(), "y ".into()] }],
            &[],
        );
        assert_eq!(normalize_sources("t", b).unwrap(), vec![src("a", &["x", "y"])]);
    }

    #[test]
    fn a_collection_named_twice_is_one_source_and_all_of_it_wins() {
        let b = body(
            vec![
                SourceSpec::Detail { collection: "a".into(), tags: vec!["x".into()] },
                SourceSpec::Name("a".into()),
                SourceSpec::Detail { collection: "b".into(), tags: vec!["x".into()] },
                SourceSpec::Detail { collection: "b".into(), tags: vec!["y".into()] },
            ],
            &[],
        );
        assert_eq!(normalize_sources("t", b).unwrap(), vec![src("a", &[]), src("b", &["x", "y"])]);
    }

    #[test]
    fn the_target_cannot_be_one_of_its_own_sources() {
        let err = normalize_sources("a", body(names(&["b", "a"]), &[])).unwrap_err();
        assert!(err.contains("into itself"), "{err}");
    }

    #[test]
    fn no_sources_and_blank_names_are_refused() {
        assert!(normalize_sources("t", body(vec![], &[])).unwrap_err().contains("at least one"));
        assert!(normalize_sources("t", body(names(&["  "]), &[])).unwrap_err().contains("empty"));
    }

    #[test]
    fn the_body_reads_the_shapes_the_ui_sends() {
        // what the Merge dialog sends today
        let legacy: MergeBody = serde_json::from_value(json!({ "sources": ["a", "b"] })).unwrap();
        assert_eq!(normalize_sources("t", legacy).unwrap(), vec![src("a", &[]), src("b", &[])]);
        // the Tag option
        let tagged: MergeBody = serde_json::from_value(json!({
            "sources": [{ "collection": "a", "tags": ["x"] }, "b"], "tags": ["z"]
        }))
        .unwrap();
        assert_eq!(normalize_sources("t", tagged).unwrap(), vec![src("a", &["x"]), src("b", &["z"])]);
        // no body at all is "nothing to merge", not a crash
        assert!(normalize_sources("t", MergeBody::default()).is_err());
    }
}
