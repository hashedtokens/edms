//! A WebView's "Full Preview": the files in that WebView's own folder
//! (`storage/webview/{name}/`), served as they are so a browser can open them.
//!
//! - `GET /webview/:name/preview` serves `index.html` from the folder.
//! - `GET /webview/:name/preview/*path` serves that file from the folder.
//!
//! Nothing is generated or converted here (the front page is still rendered by
//! the frontend from `front-page.json`); this only points at the folder. So
//! the one thing that matters is that a request can never leave it:
//!
//! - every path segment is checked (no `..`, `.`, empty, `\`, `:` or NUL),
//! - the result is canonicalised and must still be inside the folder, which
//!   also refuses a symlink that points out of it,
//! - only files are served (never a directory listing),
//! - the SQLite index (and its `-wal` / `-shm` / `-journal` files) and
//!   half-written `.tmp` files are never served.
//!
//! A folder can hold HTML someone put there, and this is the app's own
//! origin, so every answer carries `nosniff` and a Content-Security-Policy
//! that sandboxes the page and forbids scripts.

use std::path::{Path as FsPath, PathBuf};

use axum::{
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use edms::ops::view_ops::ViewKind;
use serde_json::json;

use crate::{
    handlers::{view_catalog::open_catalog, view_flavor::Flavor},
    state::AppState,
};

/// What `/webview/:name/preview` (no file named) serves.
pub const DEFAULT_PAGE: &str = "index.html";

/// Largest file served. A preview is a page and its assets, not a download.
pub const MAX_PREVIEW_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// Scripts off, the page sandboxed (no forms, no popups, no same-origin
/// access to the app), and only the folder's own files and `data:` images.
const CSP: &str = "default-src 'self' data:; script-src 'none'; object-src 'none'; \
                   style-src 'self' 'unsafe-inline'; sandbox";

#[derive(Debug, PartialEq)]
enum PreviewError {
    /// 400: the request itself is wrong (a bad name or path).
    Bad(String),
    /// 404: no such WebView or no such file.
    NotFound(String),
    /// 500
    Internal(String),
}

fn error_response(e: PreviewError) -> Response {
    let (status, msg) = match e {
        PreviewError::Bad(m) => (StatusCode::BAD_REQUEST, m),
        PreviewError::NotFound(m) => (StatusCode::NOT_FOUND, m),
        PreviewError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
    };
    (status, Json(json!({ "ok": false, "error": msg }))).into_response()
}

/// Splits a request path into safe segments, or says why it is not safe.
fn safe_segments(rel: &str) -> Result<Vec<&str>, PreviewError> {
    let mut out = Vec::new();
    for seg in rel.split('/') {
        if seg.is_empty() {
            // `a//b` and a trailing `/` name no file
            return Err(PreviewError::Bad("path has an empty segment".to_string()));
        }
        if seg == "." || seg == ".." {
            return Err(PreviewError::Bad("path can't contain '.' or '..'".to_string()));
        }
        if seg.contains(['\\', ':', '\0']) {
            return Err(PreviewError::Bad("path can't contain '\\', ':' or NUL".to_string()));
        }
        out.push(seg);
    }
    if out.is_empty() {
        return Err(PreviewError::Bad("path is empty".to_string()));
    }
    Ok(out)
}

/// Files that live in the folder but are not part of a page: the SQLite index
/// (with its sidecar files) and the temp file a save is renamed from.
fn is_private_file(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    [".sqlite", ".sqlite3", ".db", ".tmp", "-wal", "-shm", "-journal"]
        .iter()
        .any(|suffix| lower.ends_with(suffix))
        || lower.contains(".sqlite-")
        || lower.contains(".sqlite3-")
        || lower.contains(".db-")
}

/// The file to serve for `rel` inside `root`, which must exist and be a plain
/// file that is really inside `root`.
fn resolve(root: &FsPath, rel: &str) -> Result<PathBuf, PreviewError> {
    let segments = safe_segments(rel)?;
    if segments.iter().any(|s| is_private_file(s)) {
        // the same answer as a file that isn't there: don't confirm it exists
        return Err(PreviewError::NotFound(format!("'{rel}' not found")));
    }
    let candidate = segments.iter().fold(root.to_path_buf(), |p, s| p.join(s));

    let not_found = || PreviewError::NotFound(format!("'{rel}' not found"));
    let real_root = root
        .canonicalize()
        .map_err(|_| PreviewError::NotFound("this WebView has no folder yet".to_string()))?;
    let real = candidate.canonicalize().map_err(|_| not_found())?;
    if !real.starts_with(&real_root) {
        // a symlink (or junction) pointing out of the folder
        return Err(not_found());
    }
    if !real.is_file() {
        return Err(not_found());
    }
    if real.file_name().and_then(|n| n.to_str()).is_some_and(is_private_file) {
        return Err(not_found());
    }
    Ok(real)
}

/// Content-Type by file extension; anything unknown is a download.
fn content_type(path: &FsPath) -> &'static str {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "txt" | "md" => "text/plain; charset=utf-8",
        "csv" => "text/csv; charset=utf-8",
        "xml" => "application/xml",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// The WebView must exist, and its name must be safe to build a path from.
fn existing_dir(state: &AppState, name: &str) -> Result<PathBuf, PreviewError> {
    let flavor = Flavor::Web;
    flavor.validate_name(name).map_err(PreviewError::Bad)?;
    let exists = open_catalog(state)
        .map_err(PreviewError::Internal)?
        .get(ViewKind::Webview, name)
        .map_err(|e| PreviewError::Internal(format!("{e:?}")))?
        .is_some();
    if !exists {
        return Err(PreviewError::NotFound(flavor.not_found(name)));
    }
    Ok(flavor.dir(state, name))
}

async fn serve(state: AppState, name: String, rel: String) -> Response {
    let found = tokio::task::spawn_blocking(move || -> Result<(PathBuf, Vec<u8>), PreviewError> {
        let dir = existing_dir(&state, &name)?;
        let file = resolve(&dir, &rel)?;
        let len = std::fs::metadata(&file).map_err(|e| PreviewError::Internal(e.to_string()))?.len();
        if len > MAX_PREVIEW_FILE_BYTES {
            return Err(PreviewError::Bad(format!(
                "'{rel}' is {len} bytes; a preview serves files up to {MAX_PREVIEW_FILE_BYTES}"
            )));
        }
        let bytes = std::fs::read(&file).map_err(|e| PreviewError::Internal(e.to_string()))?;
        Ok((file, bytes))
    })
    .await;

    match found {
        Ok(Ok((file, bytes))) => {
            let mut res = (StatusCode::OK, bytes).into_response();
            let h = res.headers_mut();
            h.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type(&file)));
            h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
            h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP));
            // the folder changes whenever the WebView is edited
            h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            res
        }
        Ok(Err(e)) => error_response(e),
        Err(e) => error_response(PreviewError::Internal(e.to_string())),
    }
}

/// GET /webview/:name/preview
///
/// The folder's `index.html`; **404** if it has none.
pub async fn preview_webview_index(State(state): State<AppState>, Path(name): Path<String>) -> Response {
    serve(state, name, DEFAULT_PAGE.to_string()).await
}

/// GET /webview/:name/preview/*path
///
/// That file from the WebView's folder. **404** for a WebView or file that
/// doesn't exist (and for the SQLite index, which is never served), **400**
/// for a path that tries to leave the folder.
pub async fn preview_webview_file(
    State(state): State<AppState>,
    Path((name, path)): Path<(String, String)>,
) -> Response {
    serve(state, name, path).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("edms-preview-{tag}-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// root with `index.html`, `front-page.json`, `assets/site.css`, the SQLite index
    fn folder() -> PathBuf {
        let root = temp_dir("root");
        std::fs::write(root.join("index.html"), "<h1>hi</h1>").unwrap();
        std::fs::write(root.join("front-page.json"), "{}").unwrap();
        std::fs::write(root.join("webview.sqlite"), "SQLite format 3").unwrap();
        std::fs::write(root.join("webview.sqlite-wal"), "x").unwrap();
        std::fs::write(root.join("front-page.json.tmp"), "{").unwrap();
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::write(root.join("assets").join("site.css"), "body{}").unwrap();
        root
    }

    #[test]
    fn a_file_in_the_folder_and_in_a_subfolder_are_found() {
        let root = folder();
        assert!(resolve(&root, "index.html").unwrap().ends_with("index.html"));
        assert!(resolve(&root, "front-page.json").is_ok());
        assert!(resolve(&root, "assets/site.css").unwrap().ends_with("site.css"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_missing_file_and_a_directory_are_not_found() {
        let root = folder();
        assert!(matches!(resolve(&root, "nope.html"), Err(PreviewError::NotFound(_))));
        // never a directory listing
        assert!(matches!(resolve(&root, "assets"), Err(PreviewError::NotFound(_))));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_path_that_tries_to_leave_the_folder_is_refused() {
        let root = folder();
        let outside = temp_dir("outside");
        std::fs::write(outside.join("secret.txt"), "secret").unwrap();
        for bad in [
            "../secret.txt",
            "assets/../../secret.txt",
            "./index.html",
            "assets//site.css",
            "assets/",
            "..\\secret.txt",
            "assets\\site.css",
            "C:/Windows/win.ini",
            "C:\\Windows\\win.ini",
            "index.html:stream",
            "",
        ] {
            assert!(matches!(resolve(&root, bad), Err(PreviewError::Bad(_))), "{bad:?} should be refused");
        }
        // an absolute path is just another segment name that doesn't exist
        assert!(resolve(&root, &outside.join("secret.txt").display().to_string().replace('\\', "/")).is_err());
        std::fs::remove_dir_all(root).unwrap();
        std::fs::remove_dir_all(outside).unwrap();
    }

    #[test]
    fn the_sqlite_index_and_temp_files_are_never_served() {
        let root = folder();
        for hidden in ["webview.sqlite", "WebView.SQLite", "webview.sqlite-wal", "front-page.json.tmp"] {
            // the same answer as a file that isn't there
            match resolve(&root, hidden) {
                Err(PreviewError::NotFound(_)) => {}
                other => panic!("{hidden}: {other:?}"),
            }
        }
        assert!(is_private_file("webview.sqlite-shm"));
        assert!(is_private_file("webview.sqlite-journal"));
        assert!(!is_private_file("index.html"));
        assert!(!is_private_file("sqlite-notes.md"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_folder_that_does_not_exist_is_not_found() {
        let missing = std::env::temp_dir().join("edms-preview-no-such-folder-xyz");
        assert!(matches!(resolve(&missing, "index.html"), Err(PreviewError::NotFound(_))));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_out_of_the_folder_is_refused() {
        let root = folder();
        let outside = temp_dir("link-target");
        std::fs::write(outside.join("secret.txt"), "secret").unwrap();
        std::os::unix::fs::symlink(outside.join("secret.txt"), root.join("link.txt")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("linkdir")).unwrap();
        assert!(matches!(resolve(&root, "link.txt"), Err(PreviewError::NotFound(_))));
        assert!(matches!(resolve(&root, "linkdir/secret.txt"), Err(PreviewError::NotFound(_))));
        std::fs::remove_dir_all(root).unwrap();
        std::fs::remove_dir_all(outside).unwrap();
    }

    #[test]
    fn content_types_follow_the_extension() {
        let ct = |f: &str| content_type(FsPath::new(f));
        assert_eq!(ct("index.html"), "text/html; charset=utf-8");
        assert_eq!(ct("A.HTM"), "text/html; charset=utf-8");
        assert_eq!(ct("site.css"), "text/css; charset=utf-8");
        assert_eq!(ct("front-page.json"), "application/json");
        assert_eq!(ct("logo.png"), "image/png");
        assert_eq!(ct("logo.svg"), "image/svg+xml");
        assert_eq!(ct("noext"), "application/octet-stream");
        assert_eq!(ct("a.exe"), "application/octet-stream");
    }

    #[test]
    fn the_policy_forbids_scripts_and_sandboxes_the_page() {
        assert!(CSP.contains("script-src 'none'"));
        assert!(CSP.contains("sandbox"));
    }
}
