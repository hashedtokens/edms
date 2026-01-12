use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ServerEvent {
    // Spreadsheet-style notifications
    ActiveWorkspaceEndpointsLoaded { count: usize },
    ActiveWorkspaceBookmarksLoaded { count: usize },
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
}
