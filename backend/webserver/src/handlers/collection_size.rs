//! Running size of a Collection: the total size of the QP files of every
//! member endpoint (`storage/globalEQPData/{EID}/`), stored in the
//! collection's own SQLite file (`collection_stats.size_bytes`).
//!
//! It is updated incrementally instead of re-walking the disk on every list:
//! +size when an endpoint joins, -size when it leaves, +/- the difference
//! when a member's QPs change (new QP, deleted QP, updated QP, endpoint
//! deleted). A collection from before this existed has no stored size yet;
//! the first read (`ensure_size`) counts it once and stores it. Until then
//! the adjust calls do nothing, so that first count is never doubled.
//!
//! Everything here blocks - callers run it inside `spawn_blocking`. A size
//! that can't be updated never fails the request that caused it: the number
//! is a display figure, not data.

use edms::ops::collection_membership_ops::CollectionMembershipOps;
use edms::ops::view_ops::ViewKind;

use crate::{
    handlers::view_catalog::{open_catalog, open_membership},
    state::AppState,
};

/// Total size in bytes of one endpoint's QP files (0 if it has none yet).
pub(crate) fn endpoint_size(state: &AppState, endpoint_id: &str) -> i64 {
    compute::table_view::compute_size(&state.endpoint_storage_dir(endpoint_id)) as i64
}

fn size_of(state: &AppState, endpoint_ids: &[String]) -> i64 {
    endpoint_ids.iter().map(|eid| endpoint_size(state, eid)).sum()
}

/// The stored size, counting it first if this collection never had one.
pub(crate) fn ensure_size(state: &AppState, membership: &CollectionMembershipOps) -> Result<i64, String> {
    if let Some(size) = membership.size_bytes().map_err(|e| format!("{e:?}"))? {
        return Ok(size);
    }
    let members = membership.list_ids().map_err(|e| format!("{e:?}"))?;
    let size = size_of(state, &members);
    membership.set_size_bytes(size).map_err(|e| format!("{e:?}"))?;
    Ok(size)
}

/// Endpoints in `ids` that are not members yet. Call before adding them:
/// only these grow the collection (adding is idempotent), and the size has
/// to be counted (`ensure_size`) before they go in.
pub(crate) fn not_yet_members(membership: &CollectionMembershipOps, ids: &[String]) -> Result<Vec<String>, String> {
    let current = membership.list_set().map_err(|e| format!("{e:?}"))?;
    let mut seen = std::collections::HashSet::new();
    Ok(ids
        .iter()
        .filter(|id| !current.contains(*id) && seen.insert((*id).clone()))
        .cloned()
        .collect())
}

/// Endpoints joined this collection: add their size.
pub(crate) fn members_added(state: &AppState, membership: &CollectionMembershipOps, added: &[String]) {
    let _ = membership.adjust_size_bytes(size_of(state, added));
}

/// Endpoints left this collection: subtract their size.
pub(crate) fn members_removed(state: &AppState, membership: &CollectionMembershipOps, removed: &[String]) {
    let _ = membership.adjust_size_bytes(-size_of(state, removed));
}

/// One endpoint's QP files changed by `delta` bytes: every collection that
/// holds it changes by the same amount.
pub(crate) fn endpoint_changed(state: &AppState, endpoint_id: &str, delta: i64) {
    if delta == 0 {
        return;
    }
    let Ok(catalog) = open_catalog(state) else { return };
    let Ok(rows) = catalog.list(ViewKind::Collections) else { return };
    for (_name, file_path, _created_at, _annotation) in rows {
        let Some(path) = file_path else { continue };
        let Ok(membership) = open_membership(&path) else { continue };
        if membership.list_set().map(|m| m.contains(endpoint_id)).unwrap_or(false) {
            let _ = membership.adjust_size_bytes(delta);
        }
    }
}

/// For async handlers that change an endpoint's QP files themselves:
/// `size_before` first, make the change, then `settle` reports the
/// difference to every collection that holds the endpoint.
pub(crate) async fn size_before(state: &AppState, endpoint_id: &str) -> i64 {
    let (st, eid) = (state.clone(), endpoint_id.to_string());
    tokio::task::spawn_blocking(move || endpoint_size(&st, &eid)).await.unwrap_or(0)
}

pub(crate) async fn settle(state: &AppState, endpoint_id: &str, before: i64) {
    let (st, eid) = (state.clone(), endpoint_id.to_string());
    let _ = tokio::task::spawn_blocking(move || {
        let delta = endpoint_size(&st, &eid) - before;
        endpoint_changed(&st, &eid, delta);
    })
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(dir: &std::path::Path, file: &str) -> CollectionMembershipOps {
        let ops = CollectionMembershipOps::new(&dir.join(file).display().to_string());
        ops.initialize().unwrap();
        ops
    }

    #[test]
    fn adjust_does_nothing_until_the_size_is_first_set() {
        let dir = std::env::temp_dir().join(format!("edms-colsize-a-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let m = open(&dir, "c.sqlite");
        assert_eq!(m.size_bytes().unwrap(), None);
        m.adjust_size_bytes(500).unwrap();
        assert_eq!(m.size_bytes().unwrap(), None);
        m.set_size_bytes(100).unwrap();
        m.adjust_size_bytes(50).unwrap();
        assert_eq!(m.size_bytes().unwrap(), Some(150));
        m.adjust_size_bytes(-30).unwrap();
        assert_eq!(m.size_bytes().unwrap(), Some(120));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_size_never_goes_below_zero() {
        let dir = std::env::temp_dir().join(format!("edms-colsize-b-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let m = open(&dir, "c.sqlite");
        m.set_size_bytes(40).unwrap();
        m.adjust_size_bytes(-1000).unwrap();
        assert_eq!(m.size_bytes().unwrap(), Some(0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_new_unique_members_are_counted_as_added() {
        let dir = std::env::temp_dir().join(format!("edms-colsize-c-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let m = open(&dir, "c.sqlite");
        m.add("E1").unwrap();
        let ids = vec!["E1".to_string(), "E2".to_string(), "E2".to_string(), "E3".to_string()];
        assert_eq!(not_yet_members(&m, &ids).unwrap(), vec!["E2".to_string(), "E3".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
