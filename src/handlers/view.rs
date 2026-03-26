use axum::{http::StatusCode, Json};
use serde_json::json;

pub async fn home() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "view": "home",
            "message": "loads the primary dashboard"
        })),
    )
}

pub async fn test_view() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "view": "test-view",
            "message": "loads the view for testing endpoints"
        })),
    )
}

pub async fn list_view() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "view": "list-view",
            "message": "loads the view for listing endpoints"
        })),
    )
}