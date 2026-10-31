//! A WebView's front page - the "Modify Frontpage" pop-up from the v1.0 notes
//! ("WebView will have an additional RMB (Modify Frontpage) - opens a Pop
//! Up"). The folder holds `front-page.json` next to its SQLite index.
//!
//! What's in it is the rich-text editor's JSON document (Shivanshu,
//! 2026-10-07: `{"type":"doc","content":[...]}`, which the frontend turns
//! back into editable content or into HTML). The backend only stores and
//! returns JSON - it does not look inside, and does no HTML conversion.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use edms::ops::view_ops::ViewKind;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    handlers::{
        repoview_ie::FRONT_PAGE_FILE,
        view_catalog::open_catalog,
        view_flavor::Flavor,
    },
    state::AppState,
};

/// Largest front page accepted. An editor document can carry embedded
/// images, so this is generous; the route's request-body limit is raised to
/// match (axum's default is 2 MB).
pub const MAX_FRONT_PAGE_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug, Deserialize)]
pub struct SaveFrontPageRequest {
    /// The whole front page: any JSON value except `null`. Replaces the
    /// previous one.
    pub front_page: Value,
}

enum FrontPageError {
    NotFound(String),
    Bad(String),
}

impl From<String> for FrontPageError {
    fn from(e: String) -> Self {
        FrontPageError::Bad(e)
    }
}

fn respond(res: Result<Result<Value, FrontPageError>, tokio::task::JoinError>) -> (StatusCode, Json<Value>) {
    match res {
        Ok(Ok(body)) => (StatusCode::OK, Json(body)),
        Ok(Err(FrontPageError::NotFound(e))) => (StatusCode::NOT_FOUND, Json(json!({ "ok": false, "error": e }))),
        Ok(Err(FrontPageError::Bad(e))) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

/// Every WebView folder holds a `front-page.json` (v1.0 notes). A new one
/// starts as an empty object, so the "Modify Frontpage" pop-up always has a
/// file to open. Leaves an existing file alone.
pub(crate) fn ensure_default(dir: &std::path::Path) -> Result<(), String> {
    let path = dir.join(FRONT_PAGE_FILE);
    if path.is_file() {
        return Ok(());
    }
    std::fs::write(&path, "{}").map_err(|e| format!("couldn't create {FRONT_PAGE_FILE}: {e}"))
}

/// The WebView must exist (a front page only belongs to a real one) and its
/// name must be safe to build a path from.
fn existing_dir(state: &AppState, name: &str) -> Result<std::path::PathBuf, FrontPageError> {
    let flavor = Flavor::Web;
    flavor.validate_name(name)?;
    let exists = open_catalog(state)?
        .get(ViewKind::Webview, name)
        .map_err(|e| format!("{e:?}"))?
        .is_some();
    if !exists {
        return Err(FrontPageError::NotFound(flavor.not_found(name)));
    }
    Ok(flavor.dir(state, name))
}

/// GET /webview/:name/front-page
///
/// `{ok, name, exists, front_page}` - `front_page` is `null` and `exists`
/// `false` until one has been saved (the pop-up opens empty).
pub async fn get_front_page(
    State(state): State<AppState>,
    Extension(_flavor): Extension<Flavor>,
    Path(name): Path<String>,
) -> (StatusCode, Json<Value>) {
    let res = tokio::task::spawn_blocking(move || -> Result<Value, FrontPageError> {
        let path = existing_dir(&state, &name)?.join(FRONT_PAGE_FILE);
        if !path.is_file() {
            return Ok(json!({ "ok": true, "name": name, "exists": false, "front_page": null }));
        }
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let front_page: Value = serde_json::from_str(&text)
            .map_err(|e| format!("front-page.json of '{name}' is not valid JSON: {e}"))?;
        Ok(json!({ "ok": true, "name": name, "exists": true, "front_page": front_page }))
    })
    .await;
    respond(res)
}

/// POST /webview/:name/front-page  `{front_page: <any JSON>}`
///
/// Saves it as `front-page.json`, replacing the previous one. Written to a
/// temp file first and renamed into place, so a failed save never leaves a
/// half-written front page.
pub async fn save_front_page(
    State(state): State<AppState>,
    Extension(_flavor): Extension<Flavor>,
    Path(name): Path<String>,
    Json(payload): Json<SaveFrontPageRequest>,
) -> (StatusCode, Json<Value>) {
    let res = tokio::task::spawn_blocking(move || -> Result<Value, FrontPageError> {
        // `null` would read back as "no front page" (`front_page: null`).
        if payload.front_page.is_null() {
            return Err(FrontPageError::Bad("`front_page` can't be null (send any other JSON value)".to_string()));
        }
        let text = serde_json::to_string_pretty(&payload.front_page).map_err(|e| e.to_string())?;
        if text.len() > MAX_FRONT_PAGE_BYTES {
            return Err(FrontPageError::Bad(format!(
                "front page is {} bytes; the limit is {MAX_FRONT_PAGE_BYTES}",
                text.len()
            )));
        }

        let dir = existing_dir(&state, &name)?;
        if !dir.is_dir() {
            return Err(FrontPageError::Bad(format!("WebView '{name}' has no folder yet")));
        }
        let path = dir.join(FRONT_PAGE_FILE);
        let temp = dir.join(format!("{FRONT_PAGE_FILE}.tmp"));
        std::fs::write(&temp, &text).map_err(|e| e.to_string())?;
        if let Err(e) = std::fs::rename(&temp, &path) {
            // Windows won't rename over an existing file on every setup.
            let _ = std::fs::remove_file(&path);
            if let Err(e2) = std::fs::rename(&temp, &path) {
                let _ = std::fs::remove_file(&temp);
                return Err(FrontPageError::Bad(format!("couldn't save the front page: {e}, {e2}")));
            }
        }
        Ok(json!({ "ok": true, "name": name, "bytes": text.len() }))
    })
    .await;
    respond(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_size_limit_leaves_room_for_an_editor_document_with_images() {
        assert!(MAX_FRONT_PAGE_BYTES >= 10 * 1024 * 1024);
    }

    #[test]
    fn a_new_front_page_is_an_empty_object_and_an_existing_one_is_kept() {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("edms-frontpage-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();

        ensure_default(&dir).unwrap();
        let text = std::fs::read_to_string(dir.join(FRONT_PAGE_FILE)).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), json!({}));

        std::fs::write(dir.join(FRONT_PAGE_FILE), r#"{"title":"mine"}"#).unwrap();
        ensure_default(&dir).unwrap();
        let text = std::fs::read_to_string(dir.join(FRONT_PAGE_FILE)).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), json!({"title": "mine"}));

        std::fs::remove_dir_all(dir).unwrap();
    }
}
