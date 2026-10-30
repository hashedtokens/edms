//! Combine RepoViews (and WebViews - `Flavor` says which) - the list-level tag op from the v1.0 notes (Ravi,
//! 2026-10-05/06): "For both views, this operation simply combines data from
//! two or more folders. Nothing comes from Collections."
//!
//! A RepoView is only an index, so combining is a merge of indexes: members,
//! each endpoint's snapshot row, its QP metadata and its tags. No EQP data
//! and no central table is read or written. Sources are chosen by name, by
//! row tag ("every RepoView tagged release-1" - the tag op), or both.

use axum::{extract::State, http::StatusCode, Extension, Json};
use edms::ops::collection_membership_ops::CollectionMembershipOps;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    handlers::{
        repoview_index::{catalog_rows, decide, init_snapshot_tables, scalar, Decision},
        repoview_tables::{generate_tables_blocking, parse_approach_line, Approach},
        view_catalog::{open_catalog, open_existing_membership, open_membership},
        view_flavor::Flavor,
    },
    state::AppState,
};

// ── Reading, merging and writing an index ────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EndpointRow {
    pub endpoint_str: String,
    pub method: Option<String>,
    pub annotation: Option<String>,
    pub data_size_bytes: Option<i64>,
    /// The URL's path and its query string, split apart (no host).
    pub endpoint_path: Option<String>,
    pub query_param: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct QpRow {
    pub method: Option<String>,
    pub status_code: Option<i32>,
    pub response_time_ms: Option<i32>,
}

/// Everything a RepoView's index holds, in memory.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct IndexData {
    pub members: BTreeSet<String>,
    pub endpoints: BTreeMap<String, EndpointRow>,
    pub qps: BTreeMap<(String, i32), QpRow>,
    pub tags: BTreeSet<(String, String)>,
}

impl IndexData {
    /// Union another index in. The same EID is the same endpoint (EIDs are
    /// unique across the central store), so it's kept once: the first copy of
    /// its row wins, QPs are unioned by number, tags are unioned.
    pub(crate) fn absorb(&mut self, other: IndexData) {
        self.members.extend(other.members);
        for (eid, row) in other.endpoints {
            self.endpoints.entry(eid).or_insert(row);
        }
        for (key, qp) in other.qps {
            self.qps.entry(key).or_insert(qp);
        }
        self.tags.extend(other.tags);
    }
}

pub(crate) fn read_index(membership: &CollectionMembershipOps) -> Result<IndexData, String> {
    init_snapshot_tables(membership)?;
    let mut data = IndexData::default();
    let err = |e: edms::error::EdmsError| format!("{e:?}");

    let members: Vec<String> = membership
        .core
        .cproc("SELECT endpoint_id FROM membership", &[], |r| r.get(0))
        .map_err(err)?;
    data.members.extend(members);

    let endpoints: Vec<(String, EndpointRow)> = membership
        .core
        .cproc(
            "SELECT endpoint_id, endpoint_str, method, annotation, data_size_bytes,
                    endpoint_path, query_param, created_at, updated_at FROM endpoint_snapshot
             WHERE endpoint_id IN (SELECT endpoint_id FROM membership)",
            &[],
            |r| {
                Ok((
                    r.get(0)?,
                    EndpointRow {
                        endpoint_str: r.get(1)?,
                        method: r.get(2)?,
                        annotation: r.get(3)?,
                        data_size_bytes: r.get(4)?,
                        endpoint_path: r.get(5)?,
                        query_param: r.get(6)?,
                        created_at: r.get(7)?,
                        updated_at: r.get(8)?,
                    },
                ))
            },
        )
        .map_err(err)?;
    data.endpoints.extend(endpoints);

    let qps: Vec<((String, i32), QpRow)> = membership
        .core
        .cproc(
            "SELECT endpoint_id, request_number, method, status_code, response_time_ms FROM qp_snapshot
             WHERE endpoint_id IN (SELECT endpoint_id FROM membership)",
            &[],
            |r| {
                Ok((
                    (r.get(0)?, r.get(1)?),
                    QpRow { method: r.get(2)?, status_code: r.get(3)?, response_time_ms: r.get(4)? },
                ))
            },
        )
        .map_err(err)?;
    data.qps.extend(qps);

    let tags: Vec<(String, String)> = membership
        .core
        .cproc(
            "SELECT endpoint_id, tag FROM endpoint_tags WHERE endpoint_id IN (SELECT endpoint_id FROM membership)",
            &[],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(err)?;
    data.tags.extend(tags);

    Ok(data)
}

/// Writes `data` into a RepoView's index in one transaction. Everything is
/// `INSERT OR IGNORE`, so rows the target already has are left exactly as they
/// are and only genuinely new ones are added. Returns how many members were
/// new.
pub(crate) fn write_index(target: &CollectionMembershipOps, data: &IndexData) -> Result<usize, String> {
    init_snapshot_tables(target)?;
    let before = scalar(target, "SELECT COUNT(*) FROM membership")?;

    target.core.proc("BEGIN IMMEDIATE", &[]).map_err(|e| format!("{e:?}"))?;
    let written = (|| -> Result<(), String> {
        let err = |e: edms::error::EdmsError| format!("{e:?}");
        for eid in &data.members {
            target
                .core
                .proc("INSERT OR IGNORE INTO membership (endpoint_id) VALUES (?)", &[eid])
                .map_err(err)?;
        }
        for (eid, row) in &data.endpoints {
            target
                .core
                .proc(
                    "INSERT OR IGNORE INTO endpoint_snapshot (endpoint_id, endpoint_str, method, annotation, data_size_bytes, endpoint_path, query_param, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    &[
                        eid,
                        &row.endpoint_str,
                        &row.method,
                        &row.annotation,
                        &row.data_size_bytes,
                        &row.endpoint_path,
                        &row.query_param,
                        &row.created_at,
                        &row.updated_at,
                    ],
                )
                .map_err(err)?;
        }
        for ((eid, number), qp) in &data.qps {
            target
                .core
                .proc(
                    "INSERT OR IGNORE INTO qp_snapshot (endpoint_id, request_number, method, status_code, response_time_ms) VALUES (?, ?, ?, ?, ?)",
                    &[eid, number, &qp.method, &qp.status_code, &qp.response_time_ms],
                )
                .map_err(err)?;
        }
        for (eid, tag) in &data.tags {
            target
                .core
                .proc("INSERT OR IGNORE INTO endpoint_tags (endpoint_id, tag) VALUES (?, ?)", &[eid, tag])
                .map_err(err)?;
        }
        Ok(())
    })();

    match written {
        Ok(()) => target.core.proc("COMMIT", &[]).map_err(|e| format!("{e:?}"))?,
        Err(e) => {
            let _ = target.core.proc("ROLLBACK", &[]);
            return Err(e);
        }
    };
    let after = scalar(target, "SELECT COUNT(*) FROM membership")?;
    Ok((after - before).max(0) as usize)
}

/// The Collections a combined RepoView ultimately came from: the distinct
/// `source` of each input, joined, or none if no input has one.
pub(crate) fn combined_source(sources: &[Option<String>]) -> Option<String> {
    // An input may itself be a combined RepoView ("a, b"), so split first.
    let distinct: BTreeSet<&str> = sources
        .iter()
        .flatten()
        .flat_map(|s| s.split(','))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if distinct.is_empty() {
        None
    } else {
        Some(distinct.into_iter().collect::<Vec<_>>().join(", "))
    }
}

// ── The route ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CombineRequest {
    /// Name of the combined RepoView.
    pub name: String,
    /// RepoViews to combine, by name.
    #[serde(default)]
    pub sources: Vec<String>,
    /// And/or: every RepoView carrying any of these row tags.
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub annotation: Option<String>,
    /// Only needed when `name` already exists: `"merge"` or `"rename"`.
    #[serde(default)]
    pub on_exists: Option<String>,
    #[serde(default)]
    pub new_name: Option<String>,
}

enum CombineError {
    Conflict(String),
    Bad(String),
}

impl From<String> for CombineError {
    fn from(e: String) -> Self {
        CombineError::Bad(e)
    }
}

/// POST /repoview/combine
///
/// Combines two or more RepoViews into one:
/// - `name` doesn't exist: a new RepoView is created from at least two
///   sources (to copy just one, use duplicate).
/// - `name` exists and no `on_exists`: **409** `{conflict:true,
///   options:["merge","rename"]}`, nothing changes - the same choice as
///   Convert to Collection.
/// - `on_exists:"merge"`: the sources are merged into that RepoView, which is
///   one of the folders being combined; rows it already has are left alone.
/// - `on_exists:"rename"` + `new_name`: a new RepoView with that name.
///
/// The sources are never changed. The new RepoView's `source` lists the
/// Collections its inputs came from, and its row tags are the union of theirs.
pub async fn combine_repoviews(
    State(state): State<AppState>,
    Extension(flavor): Extension<Flavor>,
    Json(payload): Json<CombineRequest>,
) -> (StatusCode, Json<Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        move || combine_blocking(flavor, &state, &payload)
    })
    .await;

    match res {
        Ok(Ok(body)) => {
            state.refresh_dashboard_snapshot();
            (StatusCode::OK, Json(body))
        }
        Ok(Err(CombineError::Conflict(message))) => (
            StatusCode::CONFLICT,
            Json(json!({ "ok": false, "conflict": true, "error": message, "options": ["merge", "rename"] })),
        ),
        Ok(Err(CombineError::Bad(e))) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

fn combine_blocking(flavor: Flavor, state: &AppState, req: &CombineRequest) -> Result<Value, CombineError> {
    flavor.validate_name(&req.name)?;
    if req.sources.is_empty() && req.tags.is_empty() {
        return Err(CombineError::Bad(format!("give `sources` ({} names) and/or `tags` to pick them by", flavor.label())));
    }

    let catalog = open_catalog(state)?;
    let existing: BTreeSet<String> = catalog_rows(state, &flavor.catalog_query("LIST"), &[])?
        .into_iter()
        .map(|row| row.0)
        .collect();

    // Sources named outright must exist - a typo shouldn't quietly combine
    // fewer folders than asked for.
    let mut picked: BTreeSet<String> = BTreeSet::new();
    let missing: Vec<&String> = req.sources.iter().filter(|s| !existing.contains(*s)).collect();
    if !missing.is_empty() {
        return Err(CombineError::Bad(format!("{}s not found: {missing:?}", flavor.label())));
    }
    picked.extend(req.sources.iter().cloned());

    // Sources picked by tag: only ones that still exist (a tag row can
    // outlive its RepoView in older data).
    if !req.tags.is_empty() {
        let tag_ops = flavor.row_tag_ops(state)?;
        for tag in &req.tags {
            for name in tag_ops.repoviews_by_tag(tag).map_err(|e| format!("{e:?}"))? {
                if existing.contains(&name) {
                    picked.insert(name);
                }
            }
        }
    }

    // Where the result goes.
    let target_exists = existing.contains(&req.name);
    let (target_name, merging) = match decide(&req.name, target_exists, req.on_exists.as_deref(), req.new_name.as_deref()) {
        Decision::Conflict => {
            return Err(CombineError::Conflict(format!("{} '{}' already exists", flavor.label(), req.name)))
        }
        Decision::Invalid(why) => return Err(CombineError::Bad(why)),
        Decision::Merge(n) => (n, true),
        Decision::Create(n) => (n, false),
    };
    if !merging {
        flavor.validate_name(&target_name)?;
        if existing.contains(&target_name) {
            return Err(CombineError::Conflict(format!("{} '{target_name}' already exists", flavor.label())));
        }
    }

    // The target can't be its own input.
    picked.remove(&target_name);
    let sources: Vec<String> = picked.into_iter().collect();
    if merging && sources.is_empty() {
        return Err(CombineError::Bad(format!("nothing to merge into it: no other {} was picked", flavor.label())));
    }
    if !merging && sources.len() < 2 {
        return Err(CombineError::Bad(format!(
            "combining needs at least two {}s, but {} matched (use duplicate to copy one)",
            flavor.label(),
            sources.len()
        )));
    }

    // Read every input fully before anything is written.
    let mut combined = IndexData::default();
    let mut origin: Vec<Option<String>> = Vec::new();
    let mut row_tags: BTreeSet<String> = BTreeSet::new();
    let row_tag_ops = flavor.row_tag_ops(state)?;
    for name in &sources {
        let membership = open_existing_membership(state, flavor.kind(), name)?;
        combined.absorb(read_index(&membership)?);
        origin.push(catalog_rows(state, &flavor.catalog_query("GET"), &[name])?.into_iter().next().and_then(|row| row.4));
        row_tags.extend(row_tag_ops.list(name).map_err(|e| format!("{e:?}"))?);
    }

    let (members_added, had_tables) = if merging {
        let target = open_existing_membership(state, flavor.kind(), &target_name)?;
        let dir = flavor.dir(state, &target_name);
        let had_tables = dir.join("Tables-meta.md").is_file();
        (write_index(&target, &combined)?, had_tables)
    } else {
        let dir = flavor.dir(state, &target_name);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let file_path = flavor.file_path(state, &target_name);
        let created = (|| -> Result<usize, String> {
            let target = open_membership(&file_path)?;
            let added = write_index(&target, &combined)?;
            if flavor == Flavor::Web {
                crate::handlers::webview_front_page::ensure_default(&dir)?;
            }
            let query = state
                .queries
                .get_catalog_query(&flavor.catalog_query("CREATE"))
                .ok_or_else(|| format!("query {} is missing", flavor.catalog_query("CREATE")))?;
            catalog
                .core
                .proc(query, &[&target_name, &file_path, &req.annotation, &combined_source(&origin)])
                .map_err(|e| format!("{e:?}"))?;
            for tag in &row_tags {
                let _ = row_tag_ops.add(&target_name, tag);
            }
            Ok(added)
        })();
        match created {
            Ok(added) => (added, false),
            Err(e) => {
                let _ = std::fs::remove_dir_all(&dir);
                return Err(CombineError::Bad(e));
            }
        }
    };

    // A merge changes what an existing RepoView lists, so its Tables (if it
    // had any) would silently go stale - rebuild them, same approach.
    let tables_regenerated = merging
        && had_tables
        && std::fs::read_to_string(flavor.dir(state, &target_name).join("Tables-meta.md"))
            .ok()
            .and_then(|meta| parse_approach_line(&meta))
            .and_then(|api_name| Approach::parse(Some(api_name)).ok())
            .map(|approach| generate_tables_blocking(state, &target_name, approach, 100).is_ok())
            .unwrap_or(false);

    let endpoints_total = {
        let target = open_existing_membership(state, flavor.kind(), &target_name)?;
        scalar(&target, "SELECT COUNT(*) FROM membership")?
    };

    Ok(json!({
        "ok": true,
        "name": target_name,
        "created": !merging,
        "merged": merging,
        "sources": sources,
        "endpoints_added": members_added,
        "endpoints_total": endpoints_total,
        "tables_regenerated": tables_regenerated
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("edms-combine-{label}-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn open(dir: &std::path::Path, file: &str) -> CollectionMembershipOps {
        let ops = CollectionMembershipOps::new(&dir.join(file).display().to_string());
        ops.initialize().unwrap();
        init_snapshot_tables(&ops).unwrap();
        ops
    }

    fn add(m: &CollectionMembershipOps, eid: &str, url: &str, method: &str, qps: &[(i32, i32)], tags: &[&str]) {
        m.add(eid).unwrap();
        m.core
            .proc(
                "INSERT INTO endpoint_snapshot (endpoint_id, endpoint_str, method, annotation, data_size_bytes) VALUES (?, ?, ?, NULL, 10)",
                &[&eid, &url, &method],
            )
            .unwrap();
        for (n, status) in qps {
            m.core
                .proc(
                    "INSERT INTO qp_snapshot (endpoint_id, request_number, method, status_code, response_time_ms) VALUES (?, ?, ?, ?, 5)",
                    &[&eid, n, &method, status],
                )
                .unwrap();
        }
        for t in tags {
            m.add_tag(eid, t).unwrap();
        }
    }

    #[test]
    fn two_indexes_union_with_a_shared_endpoint_kept_once() {
        let dir = temp_dir("union");
        let a = open(&dir, "a.sqlite");
        let b = open(&dir, "b.sqlite");
        add(&a, "E0001-AAA", "https://x.com/users", "GET", &[(1, 200)], &["t1"]);
        add(&a, "E0002-AAA", "https://x.com/orders", "POST", &[], &[]);
        // E0001 again in b (same endpoint) with another QP and another tag, plus E0003
        add(&b, "E0001-AAA", "https://x.com/users", "GET", &[(1, 200), (2, 500)], &["t2"]);
        add(&b, "E0003-AAA", "https://x.com/items", "PATCH", &[(1, 204)], &["t1"]);

        let mut combined = IndexData::default();
        combined.absorb(read_index(&a).unwrap());
        combined.absorb(read_index(&b).unwrap());

        assert_eq!(combined.members.len(), 3);
        assert_eq!(combined.endpoints.len(), 3);
        // both of E0001's QPs, once each
        assert_eq!(combined.qps.keys().filter(|(e, _)| e == "E0001-AAA").count(), 2);
        // its tags from both sides
        let t: Vec<&str> = combined.tags.iter().filter(|(e, _)| e == "E0001-AAA").map(|(_, t)| t.as_str()).collect();
        assert_eq!(t, vec!["t1", "t2"]);

        drop(a);
        drop(b);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn writing_into_an_empty_index_reproduces_the_union() {
        let dir = temp_dir("write");
        let a = open(&dir, "a.sqlite");
        let b = open(&dir, "b.sqlite");
        add(&a, "E0001-AAA", "https://x.com/users", "GET", &[(1, 200)], &["t1"]);
        add(&b, "E0002-AAA", "https://x.com/orders", "POST", &[(1, 201)], &["t2"]);
        let mut combined = IndexData::default();
        combined.absorb(read_index(&a).unwrap());
        combined.absorb(read_index(&b).unwrap());

        let target = open(&dir, "out.sqlite");
        let added = write_index(&target, &combined).unwrap();

        assert_eq!(added, 2);
        assert_eq!(read_index(&target).unwrap(), combined);
        drop(a);
        drop(b);
        drop(target);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn merging_into_an_existing_index_adds_only_what_is_new_and_leaves_its_rows_alone() {
        let dir = temp_dir("merge");
        let target = open(&dir, "t.sqlite");
        add(&target, "E0001-AAA", "https://x.com/users", "GET", &[(1, 200)], &["mine"]);
        let src = open(&dir, "s.sqlite");
        // same EID but a *different* row in the source: the target's must win
        add(&src, "E0001-AAA", "https://x.com/CHANGED", "DELETE", &[(1, 500)], &["theirs"]);
        add(&src, "E0002-AAA", "https://x.com/orders", "POST", &[], &[]);

        let mut incoming = IndexData::default();
        incoming.absorb(read_index(&src).unwrap());
        let added = write_index(&target, &incoming).unwrap();

        assert_eq!(added, 1, "only E0002 is new");
        let after = read_index(&target).unwrap();
        assert_eq!(after.endpoints["E0001-AAA"].endpoint_str, "https://x.com/users");
        assert_eq!(after.qps[&("E0001-AAA".to_string(), 1)].status_code, Some(200));
        // tags are additive, though
        let t: Vec<&str> = after.tags.iter().filter(|(e, _)| e == "E0001-AAA").map(|(_, t)| t.as_str()).collect();
        assert_eq!(t, vec!["mine", "theirs"]);
        drop(target);
        drop(src);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_failed_write_rolls_back_and_leaves_the_index_untouched() {
        let dir = temp_dir("rollback");
        let target = open(&dir, "t.sqlite");
        add(&target, "E0001-AAA", "https://x.com/users", "GET", &[], &[]);
        let before = read_index(&target).unwrap();

        // a snapshot row with a NULL url violates NOT NULL part-way through
        let mut bad = IndexData::default();
        bad.members.insert("E0002-AAA".into());
        bad.members.insert("E0003-AAA".into());
        bad.endpoints.insert(
            "E0002-AAA".into(),
            EndpointRow {
                endpoint_str: "https://x.com/ok".into(),
                method: None,
                annotation: None,
                data_size_bytes: None,
                endpoint_path: None,
                query_param: None,
                created_at: None,
                updated_at: None,
            },
        );
        target
            .core
            .proc("CREATE TRIGGER boom BEFORE INSERT ON qp_snapshot BEGIN SELECT RAISE(ABORT, 'boom'); END", &[])
            .unwrap();
        bad.qps.insert(("E0002-AAA".into(), 1), QpRow { method: None, status_code: None, response_time_ms: None });

        assert!(write_index(&target, &bad).is_err());
        assert_eq!(read_index(&target).unwrap(), before, "nothing from the failed write may remain");
        drop(target);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_combined_source_lists_each_collection_once() {
        assert_eq!(combined_source(&[Some("b".into()), Some("a".into()), Some("b".into())]), Some("a, b".into()));
        assert_eq!(combined_source(&[None, Some("a".into()), None]), Some("a".into()));
        assert_eq!(combined_source(&[None, None]), None);
        assert_eq!(combined_source(&[Some("".into())]), None);
        assert_eq!(combined_source(&[]), None);
        // combining an already-combined view doesn't repeat its collections
        assert_eq!(combined_source(&[Some("a, b".into()), Some("a".into()), Some("b, c".into())]), Some("a, b, c".into()));
    }
}
