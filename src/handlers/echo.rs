use axum::{Json, http::StatusCode};
use serde_json::Value;

pub async fn echo(payload: Json<Value>) -> (StatusCode, Json<Value>) {
    (StatusCode::OK, payload)
}
