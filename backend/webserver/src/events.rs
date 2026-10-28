use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ServerEvent {
    // ── Existing events ──────────────────────────────────────────────
    ActiveWorkspaceEndpointsLoaded { count: usize },
    ActiveWorkspaceBookmarksLoaded { count: usize },
    ActiveWorkspaceHistoryLoaded { count: usize },
    CollectionLoaded { collection: String, moved_to_backup: bool },
    HistoryUpdated { count: usize },
    /// Carries which collection changed (folder = collection name) so a
    /// client with a specific collection open can filter to just that one
    /// and ignore updates for collections it isn't currently showing —
    /// needed once more than one collection can be loaded at once across
    /// different tabs (2026-09-22).
    BookmarksUpdated { collection: String, count: usize },
    FolderBecameActive { folder: String },

    // ── Test runs (Run Test) ─────────────────────────────────────────
    // A run saves nothing and has no endpoint or QP number yet, so its
    // events carry a `run_id`. `POST /test-view/save/test` with that
    // `run_id` saves it.
    RunStarted { run_id: String, endpoint_str: String, method: String },
    RunTick { run_id: String, elapsed_ms: u64, remaining_ms: u64, limit_ms: u64 },
    /// The call came back. Carries everything the Test View box shows, so
    /// the page reads nothing from disk for a run that was never saved.
    RunFinished {
        run_id: String,
        endpoint_str: String,
        method: String,
        status_code: i32,
        response_time_ms: i32,
        request_body: serde_json::Value,
        response_body: serde_json::Value,
        request_headers: serde_json::Value,
        response_headers: serde_json::Value,
    },
    RunTimeout { run_id: String },
    RunCancelled { run_id: String },

    Error { message: String },

    ExportReady { message: String },
    ImportReady { message: String },
    CrudOperationsUpdated { computed_at: String },
    ViewTagsUpdated { view: String },

    /// A collection's real membership changed (save/unsave) — as opposed
    /// to `BookmarksUpdated`, which is about the draft/bookmark state.
    /// Previously nothing was emitted here at all (2026-09-22): a save or
    /// unsave silently succeeded with no way for any other tab to know.
    /// WS-triggers-a-refresh, REST-delivers-the-data, same as
    /// ViewTagsUpdated above — re-fetch GET /collections/:name/endpoints.
    CollectionMembershipUpdated { collection: String },

    QpDeleted { endpoint_id: String, request_number: i32 },

    /// A QP's request/response body was overwritten in place via
    /// POST /test-view/:endpoint_id/qps/:request_number/update — not a real
    /// test run, so no RunFinished/HistoryUpdated accompanies this.
    QpUpdated { endpoint_id: String, request_number: i32 },

    /// A brand-new QP was created via POST /test-view/:endpoint_id/qps/create
    /// without running a real HTTP test (per Shivanshu, 2026-09-29).
    QpCreated { endpoint_id: String, request_number: i32 },

    /// A QP run or create was blocked because an identical request snapshot already exists.
    DuplicateQpBlocked {
        endpoint_id: String,
        request_number: i32,
        duplicate_of: i32,
    },

    EndpointAnnotationUpdated { endpoint_id: String },

    ViewRefresh { view_type: String, count: usize },

    /// Progress of a long Import/Export job (see `jobs.rs`), e.g. endpoints
    /// copied so far. `progress` is `{step, done, total}`. Streamed on
    /// `GET /jobs/ws`.
    ViewIoProgress {
        job_id: String,
        op: String,
        view: Option<String>,
        name: Option<String>,
        progress: serde_json::Value,
    },

    /// An Import/Export job finished, successfully or not: `result` is what
    /// the HTTP request used to return directly, `error` is set when `ok` is
    /// false.
    ViewIoDone {
        job_id: String,
        op: String,
        view: Option<String>,
        name: Option<String>,
        ok: bool,
        result: Option<serde_json::Value>,
        error: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_refresh_event_serialization() {
        let event = ServerEvent::ViewRefresh {
            view_type: "Bookmark".to_string(),
            count: 3,
        };
        let serialized = serde_json::to_string(&event).unwrap();
        assert!(serialized.contains(r#""type":"ViewRefresh""#));
        assert!(serialized.contains(r#""view_type":"Bookmark""#));
        assert!(serialized.contains(r#""count":3"#));
    }
}

