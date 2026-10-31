use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::{handlers::view_flavor::Flavor, state::AppState};

#[derive(Debug, Deserialize)]
pub struct TagRequest {
    pub tag: String,
}

/// POST /{repoview,webview}/:name/membership-tags/add
pub async fn add_repoview_tag(
    State(state): State<AppState>,
    Extension(flavor): Extension<Flavor>,
    Path(name): Path<String>,
    Json(payload): Json<TagRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        move || -> Result<usize, String> {
            let ops = flavor.row_tag_ops(&state)?;
            ops.add(&name, &payload.tag).map_err(|e| format!("{e:?}"))
        }
    })
    .await;

    match res {
        Ok(Ok(rows)) => {
            state.refresh_dashboard_snapshot();
            (StatusCode::OK, Json(json!({ "ok": true, "inserted": rows })))
        }
        Ok(Err(e)) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": e })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

/// POST /{repoview,webview}/:name/membership-tags/remove
pub async fn remove_repoview_tag(
    State(state): State<AppState>,
    Extension(flavor): Extension<Flavor>,
    Path(name): Path<String>,
    Json(payload): Json<TagRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        move || -> Result<usize, String> {
            let ops = flavor.row_tag_ops(&state)?;
            ops.remove(&name, &payload.tag).map_err(|e| format!("{e:?}"))
        }
    })
    .await;

    match res {
        Ok(Ok(rows)) => {
            state.refresh_dashboard_snapshot();
            (StatusCode::OK, Json(json!({ "ok": true, "deleted": rows })))
        }
        Ok(Err(e)) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": e })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

/// GET /{repoview,webview}/:name/membership-tags
pub async fn list_repoview_tags_for_name(
    State(state): State<AppState>,
    Extension(flavor): Extension<Flavor>,
    Path(name): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        move || -> Result<Vec<String>, String> {
            let ops = flavor.row_tag_ops(&state)?;
            ops.list(&name).map_err(|e| format!("{e:?}"))
        }
    })
    .await;

    match res {
        Ok(Ok(tags)) => (StatusCode::OK, Json(json!({ "tags": tags }))),
        Ok(Err(e)) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": e })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

/// GET /{repoview,webview}/by-tag/:tagname
pub async fn repoviews_by_tag(
    State(state): State<AppState>,
    Extension(flavor): Extension<Flavor>,
    Path(tagname): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        move || -> Result<Vec<String>, String> {
            let ops = flavor.row_tag_ops(&state)?;
            ops.repoviews_by_tag(&tagname).map_err(|e| format!("{e:?}"))
        }
    })
    .await;

    match res {
        Ok(Ok(views)) => {
            // `{"repoviews": [...]}` or `{"webviews": [...]}`
            let mut body = serde_json::Map::new();
            body.insert(format!("{}s", flavor.route()), json!(views));
            (StatusCode::OK, Json(serde_json::Value::Object(body)))
        }
        Ok(Err(e)) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": e })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}
