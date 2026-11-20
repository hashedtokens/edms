//! Bookmark ↔ Collection (Mathew, 2026-09-08; redesigned 2026-09-22).
//!
//! Originally, "active bookmarks" was the draft state of whichever single
//! Collection (`storage/collections/{name}.sqlite`) was loaded into
//! `state.active_collection` — one shared slot for the whole server, so
//! only one collection could ever be loaded anywhere at once, and loading
//! a second one in another tab silently evicted the first.
//!
//! Now the collection is explicit in every request instead of implicit
//! shared state: the central `bookmarks` table's `folder` column holds
//! the real collection name directly (no more `__active__`), so bookmarks
//! for different collections never collide, and there's no more
//! "load a collection first" gate or session-backup mechanism — nothing
//! gets evicted, because nothing is shared.
//!
//! 1. Collection tab creates an empty, named collection (`/collections/create`
//!    — unchanged, in view_catalog.rs).
//! 2. `ws_subscribe_collection` streams a specific collection's bookmarks —
//!    resolving full endpoint data, not just EIDs — via the same
//!    subscription machinery test_view.rs already had, just scoped by an
//!    explicit `:collection` path param now.
//! 3. History → bookmark (test_view.rs's `save_bookmark`/`ws_add_from_history_to_bookmark`)
//!    adds tested endpoints directly into a named collection's bookmarks.
//! 4. Per-endpoint, in bookmark view: `save_to_collection` persists a
//!    bookmarked endpoint's membership (EID + timestamp only) into that
//!    same collection's file; `remove_from_collection` drops that
//!    membership but leaves it bookmarked (per Mathew: "let it just be
//!    bookmarked then not part of the collection"). Deleting from bookmark
//!    view entirely is test_view.rs's existing `ws_delete_from_bookmark`.

use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use crate::{db, events::ServerEvent, handlers::view_catalog::open_existing_collection_membership, state::AppState};

/// GET /bookmarks/:collection/load — collection-scoped bookmark
/// subscription: a snapshot of that collection's bookmarks, then streams
/// events. Unlike the old version, this never writes anything (no wipe,
/// no copy, no backup) — it's a pure read, matching the "WS subscribes,
/// REST mutates" pattern used everywhere else in this codebase.
pub async fn ws_load_collection(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(collection): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_ws_subscribe_collection(socket, state, collection).await;
    })
}

async fn handle_ws_subscribe_collection(mut socket: WebSocket, state: AppState, collection: String) {
    let res = tokio::task::spawn_blocking({
        let st = state.clone();
        let c = collection.clone();
        move || -> Result<usize, String> {
            db::bookmarks_count(&st.core, &st.queries, &c).map_err(|e| format!("{e:?}"))
        }
    })
    .await;

    let count = match res {
        Ok(Ok(count)) => count,
        Ok(Err(e)) => {
            let resp = json!({"type":"error","message": e});
            let _ = socket.send(Message::Text(resp.to_string())).await;
            return;
        }
        Err(e) => {
            let resp = json!({"type":"error","message": format!("{e}")});
            let _ = socket.send(Message::Text(resp.to_string())).await;
            return;
        }
    };

    let resp = json!({
        "type": "collection_loaded",
        "collection": collection,
        "bookmark_count": count
    });
    let _ = socket.send(Message::Text(resp.to_string())).await;

    // Stream events after load — same pattern every other subscribe-style
    // WS route in this codebase uses. BookmarksUpdated carries `collection`
    // now, so the client filters to events about this specific one.
    let mut rx = state.events_tx.subscribe();
    while let Ok(evt) = rx.recv().await {
        let msg = json!({"type":"event","event": evt});
        if socket.send(Message::Text(msg.to_string())).await.is_err() {
            break;
        }
    }
}

/// POST /bookmarks/:collection/:endpoint_id/save — persists a bookmarked
/// endpoint's membership (EID + timestamp only, never a copy of its data)
/// into the named collection. 400 if it isn't currently bookmarked there.
pub async fn save_to_collection(
    State(state): State<AppState>,
    Path((collection, endpoint_id)): Path<(String, String)>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let st = state.clone();
        let eid = endpoint_id.clone();
        let collection = collection.clone();
        move || -> Result<usize, String> {
            let bookmarked = db::bookmark_exists(&st.core, &st.queries, &collection, &eid)
                .map_err(|e| format!("{e:?}"))?;
            if !bookmarked {
                return Err(format!("'{eid}' is not bookmarked in '{collection}'"));
            }
            let membership = open_existing_collection_membership(&st, &collection)?;
            let joining = crate::handlers::collection_size::not_yet_members(&membership, std::slice::from_ref(&eid))?;
            crate::handlers::collection_size::ensure_size(&st, &membership)?;
            let inserted = membership.add(&eid).map_err(|e| format!("{e:?}"))?;
            crate::handlers::collection_size::members_added(&st, &membership, &joining);
            Ok(inserted)
        }
    })
    .await;

    match res {
        Ok(Ok(inserted)) => {
            state.refresh_dashboard_snapshot();
            state
                .emit(ServerEvent::CollectionMembershipUpdated { collection: collection.clone() })
                .await;
            (
                StatusCode::OK,
                Json(json!({ "ok": true, "collection": collection, "inserted": inserted })),
            )
        }
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

/// POST /bookmarks/:collection/:endpoint_id/unsave — drops an endpoint's
/// membership from the named collection. Per Mathew (2026-09-08): it stays
/// bookmarked/visible afterward — only the collection membership is
/// removed, not the bookmark itself.
pub async fn remove_from_collection(
    State(state): State<AppState>,
    Path((collection, endpoint_id)): Path<(String, String)>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let st = state.clone();
        let eid = endpoint_id.clone();
        let collection = collection.clone();
        move || -> Result<usize, String> {
            let membership = open_existing_collection_membership(&st, &collection)?;
            crate::handlers::collection_size::ensure_size(&st, &membership)?;
            let deleted = membership.remove(&eid).map_err(|e| format!("{e:?}"))?;
            if deleted > 0 {
                crate::handlers::collection_size::members_removed(&st, &membership, std::slice::from_ref(&eid));
            }
            Ok(deleted)
        }
    })
    .await;

    match res {
        Ok(Ok(deleted)) => {
            state.refresh_dashboard_snapshot();
            state
                .emit(ServerEvent::CollectionMembershipUpdated { collection: collection.clone() })
                .await;
            (
                StatusCode::OK,
                Json(json!({ "ok": true, "collection": collection, "deleted": deleted })),
            )
        }
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}
