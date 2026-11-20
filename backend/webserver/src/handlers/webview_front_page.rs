//! A WebView's front page - the "Modify Frontpage" pop-up from the v1.0 notes
//! ("WebView will have an additional RMB (Modify Frontpage) - opens a Pop
//! Up"). The folder holds `front-page.json` next to its SQLite index.
//!
//! What's in it is the rich-text editor's JSON document (Shivanshu,
//! 2026-10-07: `{"type":"doc","content":[...]}`, which the frontend turns
//! back into editable content or into HTML). The backend only stores and
//! returns JSON - it does not look inside, and does no HTML conversion.
//!
//! The "Full Preview" (`webview_preview`) serves `index.html` from the same
//! folder. The backend can't make that page (no conversion), so the frontend
//! sends it with the save: `{front_page, html?}`. When `html` is given it is
//! written as `index.html` together with `front-page.json`; when it is left
//! out, the existing `index.html` is left alone.

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
        webview_preview::DEFAULT_PAGE as INDEX_FILE,
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
    /// The rendered page for "Full Preview", saved as `index.html`. Optional:
    /// left out, `index.html` is not touched. Can't be empty.
    #[serde(default)]
    pub html: Option<String>,
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
            return Ok(json!({ "ok": true, "name": name, "exists": false, "front_page": null, "index_html": false }));
        }
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let front_page: Value = serde_json::from_str(&text)
            .map_err(|e| format!("front-page.json of '{name}' is not valid JSON: {e}"))?;
        let index_html = path.with_file_name(INDEX_FILE).is_file();
        Ok(json!({ "ok": true, "name": name, "exists": true, "front_page": front_page, "index_html": index_html }))
    })
    .await;
    respond(res)
}

/// POST /webview/:name/front-page  `{front_page: <any JSON>, html?: <string>}`
///
/// Saves it as `front-page.json`, replacing the previous one, and `html` (if
/// given) as `index.html`, which is what `GET /webview/:name/preview` serves.
/// Both are written to temp files first and renamed into place, so a failed
/// save never leaves a half-written front page or a preview that doesn't
/// match it.
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

        if let Some(html) = &payload.html {
            check_html(html)?;
        }

        let dir = existing_dir(&state, &name)?;
        if !dir.is_dir() {
            return Err(FrontPageError::Bad(format!("WebView '{name}' has no folder yet")));
        }
        // temp files first (a failure here changes nothing), then the renames
        let mut files = vec![(FRONT_PAGE_FILE, text.as_str())];
        if let Some(html) = &payload.html {
            files.push((INDEX_FILE, html.as_str()));
        }
        write_all(&dir, &files).map_err(FrontPageError::Bad)?;
        Ok(json!({
            "ok": true,
            "name": name,
            "bytes": text.len(),
            "index_html": payload.html.is_some(),
            "html_bytes": payload.html.as_ref().map(|h| h.len()),
        }))
    })
    .await;
    respond(res)
}

/// `html` is a page: not empty (a blank preview helps nobody) and no bigger
/// than a front page may be.
fn check_html(html: &str) -> Result<(), FrontPageError> {
    if html.trim().is_empty() {
        return Err(FrontPageError::Bad(
            "`html` can't be empty (leave it out to keep the current index.html)".to_string(),
        ));
    }
    if html.len() > MAX_FRONT_PAGE_BYTES {
        return Err(FrontPageError::Bad(format!(
            "`html` is {} bytes; the limit is {MAX_FRONT_PAGE_BYTES}",
            html.len()
        )));
    }
    Ok(())
}

/// Writes every `(file name, text)` into `dir` through `{name}.tmp` files:
/// all temp files are written before any is renamed into place, so a failure
/// leaves the existing files as they were.
fn write_all(dir: &std::path::Path, files: &[(&str, &str)]) -> Result<(), String> {
    let temp_of = |file: &str| dir.join(format!("{file}.tmp"));
    for (file, text) in files {
        if let Err(e) = std::fs::write(temp_of(file), text) {
            for (f, _) in files {
                let _ = std::fs::remove_file(temp_of(f));
            }
            return Err(format!("couldn't save {file}: {e}"));
        }
    }
    for (file, _) in files {
        let (path, temp) = (dir.join(file), temp_of(file));
        if let Err(e) = std::fs::rename(&temp, &path) {
            // Windows won't rename over an existing file on every setup.
            let _ = std::fs::remove_file(&path);
            if let Err(e2) = std::fs::rename(&temp, &path) {
                let _ = std::fs::remove_file(&temp);
                return Err(format!("couldn't save {file}: {e}, {e2}"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("edms-frontpage-w-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_front_page_and_the_preview_are_written_together() {
        let dir = temp_dir();
        std::fs::write(dir.join("index.html"), "old").unwrap();
        write_all(&dir, &[(FRONT_PAGE_FILE, "{}"), (INDEX_FILE, "<h1>new</h1>")]).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join(FRONT_PAGE_FILE)).unwrap(), "{}");
        assert_eq!(std::fs::read_to_string(dir.join("index.html")).unwrap(), "<h1>new</h1>");
        // no temp file is left behind
        assert!(!dir.join("index.html.tmp").exists() && !dir.join("front-page.json.tmp").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn leaving_html_out_keeps_the_current_index_html() {
        let dir = temp_dir();
        std::fs::write(dir.join("index.html"), "keep me").unwrap();
        write_all(&dir, &[(FRONT_PAGE_FILE, r#"{"a":1}"#)]).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("index.html")).unwrap(), "keep me");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_failed_write_changes_nothing() {
        let dir = temp_dir();
        std::fs::write(dir.join(FRONT_PAGE_FILE), "old").unwrap();
        // a folder in the way of index.html.tmp makes the second write fail
        std::fs::create_dir(dir.join("index.html.tmp")).unwrap();
        assert!(write_all(&dir, &[(FRONT_PAGE_FILE, "new"), (INDEX_FILE, "<p>")]).is_err());
        assert_eq!(std::fs::read_to_string(dir.join(FRONT_PAGE_FILE)).unwrap(), "old");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn html_must_not_be_empty_or_huge() {
        assert!(check_html("<p>hi</p>").is_ok());
        assert!(check_html("").is_err());
        assert!(check_html("  \n ").is_err());
        assert!(check_html(&"x".repeat(MAX_FRONT_PAGE_BYTES + 1)).is_err());
    }

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
