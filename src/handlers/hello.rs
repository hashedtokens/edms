use axum::{extract::Path, Json};
use serde_json::json;

pub async fn hello(Path(name): Path<String>) -> Json<serde_json::Value> {
    Json(json!({ "message": format!("Hello, {}!", name) }))
}
