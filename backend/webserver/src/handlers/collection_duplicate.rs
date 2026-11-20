//! Duplicate a Collection: `POST /collections/:name/duplicate`.
//!
//! The UI's Duplicate used to create an empty collection called "name (copy)"
//! and fill it from the list it had on screen, which holds no endpoints, so
//! the copy came out empty. This does the whole thing in the backend: a new
//! collection with a copy of everything in the original (its endpoints under
//! fresh EIDs, their QPs, tags, bookmarks and notes, the collection's
//! annotation and tags) - see compute's `tagops::collection_duplicate`.
//!
//! It runs as a job, like Merge and Import/Export, because a big collection
//! means a lot of files: the request returns `202` with a `job_id`, and the
//! result - what was copied - arrives as a `ViewIoDone` event (op
//! `duplicate`) and on `GET /jobs/:id`. Nothing is visible until the copy is
//! complete, and a failure leaves nothing behind.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    handlers::view_catalog::{open_catalog, validate_folder_name},
    ipc::IpcCallback,
    jobs::JobStatus,
    state::AppState,
};
use edms::ops::view_ops::ViewKind;

const OP: &str = "duplicate";
const VIEW: &str = "collections";

#[derive(Debug, Default, Deserialize)]
pub struct DuplicateBody {
    /// The new collection's name. Left out, it is "<name> (copy)", or
    /// "<name> (copy 2)", "(copy 3)"... if that is taken.
    #[serde(default)]
    pub new_name: Option<String>,
}

/// What a failed request is: a status and a message.
type Fail = (StatusCode, String);

fn bad(message: impl Into<String>) -> Fail {
    (StatusCode::BAD_REQUEST, message.into())
}

/// The first free name of "<source> (copy)", "<source> (copy 2)", ... Pure,
/// so the rule is testable without a server.
pub fn pick_copy_name(source: &str, mut taken: impl FnMut(&str) -> bool) -> String {
    let first = format!("{source} (copy)");
    if !taken(&first) {
        return first;
    }
    let mut n = 2u32;
    loop {
        let name = format!("{source} (copy {n})");
        if !taken(&name) {
            return name;
        }
        n += 1;
    }
}

/// POST /collections/:name/duplicate   `{"new_name"?}`
pub async fn duplicate_collection(
    State(state): State<AppState>,
    Path(source): Path<String>,
    body: Option<Json<DuplicateBody>>,
) -> (StatusCode, Json<Value>) {
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        move || start_duplicate(&state, &source, body)
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

fn start_duplicate(state: &AppState, source: &str, body: DuplicateBody) -> Result<Value, Fail> {
    let source = source.trim();
    validate_folder_name(source, "Collection name").map_err(bad)?;
    let internal = |e: String| (StatusCode::INTERNAL_SERVER_ERROR, e);

    let catalog = open_catalog(state).map_err(internal)?;
    let exists = |name: &str| -> Result<bool, Fail> {
        catalog
            .get(ViewKind::Collections, name)
            .map(|row| row.is_some())
            .map_err(|e| internal(format!("{e:?}")))
    };
    if !exists(source)? {
        return Err((StatusCode::NOT_FOUND, format!("Collection '{source}' does not exist")));
    }

    let asked = body.new_name.as_deref().map(str::trim).filter(|n| !n.is_empty());
    let target = match asked {
        Some(name) => {
            validate_folder_name(name, "Collection name").map_err(bad)?;
            if name == source || exists(name)? {
                return Err((StatusCode::CONFLICT, format!("Collection '{name}' already exists")));
            }
            name.to_string()
        }
        None => {
            // look every candidate up in the catalog; a failed lookup stops the search
            let mut lookup_failed: Option<Fail> = None;
            let name = pick_copy_name(source, |candidate| match exists(candidate) {
                Ok(found) => found,
                Err(e) => {
                    lookup_failed = Some(e);
                    false
                }
            });
            if let Some(e) = lookup_failed {
                return Err(e);
            }
            validate_folder_name(&name, "Collection name").map_err(|e| {
                bad(format!("{e}. Send a shorter \"new_name\" for the copy"))
            })?;
            name
        }
    };

    // Two duplicates onto the same name at once would race on its file.
    let busy = state
        .jobs
        .list()
        .iter()
        .any(|j| j.status == JobStatus::Running && j.op == OP && j.name.as_deref() == Some(target.as_str()));
    if busy {
        return Err((StatusCode::CONFLICT, format!("'{target}' is already being created")));
    }

    let job_id = state.jobs.create(OP, Some(VIEW), Some(&target));
    let started = crate::ipc::spawn_child(
        "collection_duplicate",
        json!({
            "job_id": job_id,
            "db_path": state.db_path.display().to_string(),
            "storage_root": state.storage_root,
            "source_collection_name": source,
            "target_collection_name": target,
        }),
        3000,
    );
    if !started {
        let error = "couldn't start the compute process (is edms-child built?)".to_string();
        if let Some(job) = state.jobs.fail(&job_id, error.clone()) {
            crate::handlers::jobs::emit_done(state, &job);
        }
        return Err(internal(error));
    }

    Ok(json!({
        "ok": true,
        "status": "running",
        "job_id": job_id,
        "source": source,
        "target": target,
        "folder": target,
    }))
}

/// The duplicate child reported back: complete the job (the UI hears of it as
/// a `ViewIoDone`) and refresh the dashboard's counts - a duplicate adds
/// endpoints and a collection.
pub fn finish_duplicate(state: &AppState, callback: &IpcCallback) {
    crate::handlers::jobs::complete_from_callback(state, callback);
    state.refresh_dashboard_snapshot();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn the_copy_is_called_name_copy() {
        assert_eq!(pick_copy_name("Users", |_| false), "Users (copy)");
    }

    #[test]
    fn a_taken_name_counts_up() {
        let taken: HashSet<&str> = ["Users (copy)", "Users (copy 2)"].into_iter().collect();
        assert_eq!(pick_copy_name("Users", |n| taken.contains(n)), "Users (copy 3)");
    }

    #[test]
    fn the_default_names_are_valid_folder_names() {
        for name in ["Users (copy)", "Users (copy 12)", "my collection (copy)"] {
            assert!(validate_folder_name(name, "Collection name").is_ok(), "{name}");
        }
    }

    #[test]
    fn a_very_long_name_cannot_take_the_default_suffix() {
        let long = "a".repeat(98);
        let name = pick_copy_name(&long, |_| false);
        assert!(validate_folder_name(&name, "Collection name").is_err(), "100 characters is the limit");
    }

    #[test]
    fn the_body_is_optional_and_new_name_may_be_left_out() {
        let none: DuplicateBody = serde_json::from_str("{}").unwrap();
        assert!(none.new_name.is_none());
        let some: DuplicateBody = serde_json::from_str(r#"{"new_name":"Copy of Users"}"#).unwrap();
        assert_eq!(some.new_name.as_deref(), Some("Copy of Users"));
    }
}
