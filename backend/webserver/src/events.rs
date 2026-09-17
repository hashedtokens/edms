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
    BookmarksUpdated { count: usize },
    FolderBecameActive { folder: String },
    TestStarted { endpoint_id: String, request_number: i32 },
    TestFinished {
        endpoint_id: String,
        request_number: i32,
        status_code: i32,
        response_time_ms: i32,
        response_file: String,
    },
    TestTimeout { endpoint_id: String, request_number: i32 },
    Error { message: String },

    ExportReady { message: String },
    ImportReady { message: String },
    CrudOperationsUpdated { computed_at: String },
    TimerTick {
        endpoint_id: String,
        request_number: i32,
        elapsed_ms: u64,
        remaining_ms: u64,
        limit_ms: u64,
    },

    
    TimerCancelled {
        endpoint_id: String,
        request_number: i32,
        elapsed_ms: u64,
    },

    ViewTagsUpdated { view: String },

    QpDeleted { endpoint_id: String, request_number: i32 },

    EndpointAnnotationUpdated { endpoint_id: String },

    ViewRefresh { view_type: String, count: usize },
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

