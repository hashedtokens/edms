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

pub async fn trigger_view_refresh(
    axum::extract::State(state): axum::extract::State<crate::state::AppState>,
    Json(payload): Json<serde_json::Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    let view_type = payload
        .get("view_type")
        .or_else(|| payload.get("ViewType"))
        .and_then(|v| v.as_str())
        .unwrap_or("general")
        .to_string();
    let count = payload
        .get("count")
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as usize;

    let _ = state.events_tx.send(crate::events::ServerEvent::ViewRefresh {
        view_type: view_type.clone(),
        count,
    });

    (
        StatusCode::OK,
        Json(json!({
            "status": "notified",
            "view_type": view_type,
            "count": count
        })),
    )
}