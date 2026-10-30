//! The Import/Export table: what is sitting in `storage/imports/`, and a
//! format check for a row. It uses compute's `table_view` as it is.
//!
//! Not wired on purpose: `table_view::move_item_to_view`. It copies a raw
//! folder to `storage/{repoview|webview|collections}/{name}`, and
//! `storage/repoview` / `storage/webview` are where registered RepoViews and
//! WebViews live, so a "move" could write into one and would anyway create
//! no catalog row, fresh EIDs or index. For RepoViews and WebViews, importing
//! a row *is* the move: `POST /repoview/import`, `/webview/import` or
//! `/collections/import` with its `folder` or `zip`.

use axum::{extract::State, http::StatusCode, Json};
use compute::table_view::{scan_imports_table, TableItem, ViewPurpose};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};

use crate::state::AppState;

fn imports_root(state: &AppState) -> PathBuf {
    state.storage_root.join("storage").join("imports")
}

/// `repo/` -> RepoView, `webview/` -> WebView, `collections/` -> Collections:
/// the folders `storage/imports/{compressed,uncompressed}/` are split by.
fn purpose_of_folder(first: &str) -> Option<ViewPurpose> {
    match first {
        "repo" | "repoview" => Some(ViewPurpose::RepoView),
        "webview" => Some(ViewPurpose::WebView),
        "collections" => Some(ViewPurpose::Collections),
        _ => None,
    }
}

fn purpose_name(purpose: ViewPurpose) -> &'static str {
    match purpose {
        ViewPurpose::Collections => "Collections",
        ViewPurpose::RepoView => "RepoView",
        ViewPurpose::WebView => "WebView",
    }
}

/// One table row in the shape the UI's table reads (`name`, `type`, `view`,
/// `purpose`, `size`, `formatCheck`) plus what a follow-up call needs
/// (`section` + `path` for `check`, `folder`/`zip` for `import`).
///
/// `purpose` comes from the folder the item is in, not from compute's
/// `infer_purpose`, which looks at the *whole* path and so can be fooled by a
/// parent folder that happens to contain "repo".
fn row_json(item: &TableItem) -> Option<Value> {
    let parts: Vec<&str> = item.relative_path.split(['/', '\\']).collect();
    // `{purpose folder}/{item}`: compute's scan also lists the folders
    // inside a takeout (globalEQPData, ...), which aren't importable items.
    if parts.len() != 2 {
        return None;
    }
    let purpose = purpose_of_folder(parts[0]);
    let section = if item.is_compressed { "compressed" } else { "uncompressed" };
    Some(json!({
        "name": item.name,
        "type": "Import",
        "view": section,
        "purpose": purpose.map(purpose_name),
        "size": item.size_display,
        "size_bytes": item.size_bytes,
        "formatCheck": null,
        "section": section,
        "path": format!("{}/{}", parts[0], parts[1]),
        // what to pass to POST /{repoview,webview}/import for this row
        "import_with": match (purpose, item.is_compressed) {
            (Some(ViewPurpose::RepoView), true) => json!({ "route": "/repoview/import", "zip": item.name }),
            (Some(ViewPurpose::RepoView), false) => json!({ "route": "/repoview/import", "folder": item.name }),
            (Some(ViewPurpose::WebView), true) => json!({ "route": "/webview/import", "zip": item.name }),
            (Some(ViewPurpose::WebView), false) => json!({ "route": "/webview/import", "folder": item.name }),
            (Some(ViewPurpose::Collections), true) => json!({ "route": "/collections/import", "zip": item.name }),
            (Some(ViewPurpose::Collections), false) => json!({ "route": "/collections/import", "folder": item.name }),
            _ => Value::Null,
        },
    }))
}

/// GET /import-export/table - everything under `storage/imports/`, scanned
/// live (nothing is indexed), one row per zip or takeout folder. Not format
/// checked (`formatCheck: null`): that can take a while for a big item, so it
/// is a job, `POST /import-export/check`.
pub async fn import_export_table(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let root = imports_root(&state);
    match tokio::task::spawn_blocking(move || scan_imports_table(&root)).await {
        Ok(scan) => {
            let items: Vec<Value> = scan.compressed.iter().chain(scan.uncompressed.iter()).filter_map(row_json).collect();
            (StatusCode::OK, Json(json!({ "ok": true, "items": items })))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct CheckRequest {
    /// `"compressed"` or `"uncompressed"` - a row's `section`.
    pub section: String,
    /// A row's `path`, e.g. `repo/inventory.zip` or `webview/telemetry`.
    pub path: String,
}

/// An item under `storage/imports/{section}/`, resolved from the names the
/// table gave out. They are plain names (nothing the caller types is joined
/// onto a path unchecked) and the result must be a zip file or a folder.
fn resolve_item(imports: &Path, section: &str, rel: &str) -> Result<(PathBuf, ViewPurpose), String> {
    if section != "compressed" && section != "uncompressed" {
        return Err("`section` must be \"compressed\" or \"uncompressed\"".to_string());
    }
    let parts: Vec<&str> = rel.split(['/', '\\']).collect();
    if parts.len() != 2 || parts.iter().any(|p| p.is_empty() || *p == "." || *p == ".." || p.contains(':') || p.contains('\0')) {
        return Err("`path` must look like `repo/name` (as listed by GET /import-export/table)".to_string());
    }
    let purpose = purpose_of_folder(parts[0]).ok_or_else(|| format!("'{}' is not one of repo/, webview/, collections/", parts[0]))?;

    let full = imports.join(section).join(parts[0]).join(parts[1]);
    // Belt and braces: every component of what was joined is a plain name.
    debug_assert!(full.strip_prefix(imports).map_or(false, |r| r.components().all(|c| matches!(c, Component::Normal(_)))));

    if section == "compressed" {
        let is_zip = full.extension().map_or(false, |e| e.eq_ignore_ascii_case("zip"));
        if !is_zip || !full.is_file() {
            return Err(format!("'{rel}' is not a .zip under storage/imports/compressed/"));
        }
    } else if !full.is_dir() {
        return Err(format!("'{rel}' is not a folder under storage/imports/uncompressed/"));
    }
    Ok((full, purpose))
}

/// POST /import-export/check `{section, path}` -> `202 {job_id}`
///
/// Format-checks one row for its purpose (SQLite present / JSON valid, per
/// compute's `validate`). A zip is unpacked safely first. The report arrives as the job's
/// result (`ViewIoDone`, or `GET /jobs/:id`): `{passed, report:{...}}`.
pub async fn import_export_check(
    State(state): State<AppState>,
    Json(payload): Json<CheckRequest>,
) -> (StatusCode, Json<Value>) {
    let res = tokio::task::spawn_blocking({
        let state = state.clone();
        move || -> Result<Value, String> {
            let (item_path, purpose) = resolve_item(&imports_root(&state), &payload.section, &payload.path)?;
            let name = payload.path.rsplit(['/', '\\']).next().unwrap_or("").to_string();
            let job_id = state.jobs.create("format_check", None, Some(&name));
            let started = crate::ipc::spawn_child(
                "view_table_check",
                json!({ "job_id": job_id, "item_path": item_path, "purpose": purpose }),
                3000,
            );
            if !started {
                let error = "couldn't start the compute process (is edms-child built?)".to_string();
                if let Some(job) = state.jobs.fail(&job_id, error.clone()) {
                    crate::handlers::jobs::emit_done(&state, &job);
                }
                return Err(error);
            }
            Ok(json!({ "ok": true, "status": "running", "job_id": job_id, "purpose": purpose_name(purpose) }))
        }
    })
    .await;

    match res {
        Ok(Ok(body)) => (StatusCode::ACCEPTED, Json(body)),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_imports(label: &str) -> PathBuf {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("edms-ie-table-{label}-{unique}"));
        std::fs::create_dir_all(dir.join("compressed/repo")).unwrap();
        std::fs::create_dir_all(dir.join("uncompressed/webview/telemetry")).unwrap();
        std::fs::write(dir.join("compressed/repo/inventory.zip"), "zip").unwrap();
        std::fs::write(dir.join("compressed/repo/notes.txt"), "not a zip").unwrap();
        dir
    }

    #[test]
    fn rows_name_their_purpose_from_the_folder_not_the_whole_path() {
        let imports = temp_imports("rows");
        // a parent folder that merely contains "repo" must not turn a webview into a RepoView
        let scan = scan_imports_table(&imports);
        let rows: Vec<Value> = scan.compressed.iter().chain(scan.uncompressed.iter()).filter_map(row_json).collect();

        let zip = rows.iter().find(|r| r["name"] == "inventory.zip").unwrap();
        assert_eq!(zip["purpose"], "RepoView");
        assert_eq!(zip["view"], "compressed");
        assert_eq!(zip["path"], "repo/inventory.zip");
        assert_eq!(zip["import_with"], json!({ "route": "/repoview/import", "zip": "inventory.zip" }));
        assert!(zip["formatCheck"].is_null());

        let folder = rows.iter().find(|r| r["name"] == "telemetry").unwrap();
        assert_eq!(folder["purpose"], "WebView");
        assert_eq!(folder["import_with"], json!({ "route": "/webview/import", "folder": "telemetry" }));

        assert!(rows.iter().all(|r| r["name"] != "notes.txt"), "only zips are listed as compressed items");

        // a Collections row is importable now too
        std::fs::create_dir_all(imports.join("compressed/collections")).unwrap();
        std::fs::create_dir_all(imports.join("uncompressed/collections/auth-suite")).unwrap();
        std::fs::write(imports.join("compressed/collections/auth.zip"), "zip").unwrap();
        let scan = scan_imports_table(&imports);
        let rows: Vec<Value> = scan.compressed.iter().chain(scan.uncompressed.iter()).filter_map(row_json).collect();
        let zip = rows.iter().find(|r| r["name"] == "auth.zip").unwrap();
        assert_eq!(zip["purpose"], "Collections");
        assert_eq!(zip["import_with"], json!({ "route": "/collections/import", "zip": "auth.zip" }));
        let folder = rows.iter().find(|r| r["name"] == "auth-suite").unwrap();
        assert_eq!(folder["import_with"], json!({ "route": "/collections/import", "folder": "auth-suite" }));
        std::fs::remove_dir_all(imports).unwrap();
    }

    #[test]
    fn folders_inside_a_takeout_are_not_rows() {
        let imports = temp_imports("nested");
        std::fs::create_dir_all(imports.join("uncompressed/repo/rA/globalEQPData/E0001-AAA")).unwrap();
        std::fs::write(imports.join("uncompressed/repo/rA/repoview-manifest.json"), "{}").unwrap();
        let scan = scan_imports_table(&imports);
        let names: Vec<String> = scan.uncompressed.iter().filter_map(row_json).map(|r| r["name"].as_str().unwrap().to_string()).collect();
        assert!(names.contains(&"rA".to_string()));
        assert!(!names.contains(&"globalEQPData".to_string()), "{names:?}");
        assert!(!names.contains(&"E0001-AAA".to_string()), "{names:?}");
        std::fs::remove_dir_all(imports).unwrap();
    }

    #[test]
    fn a_check_resolves_only_rows_the_table_could_have_listed() {
        let imports = temp_imports("resolve");
        let (path, purpose) = resolve_item(&imports, "compressed", "repo/inventory.zip").unwrap();
        assert!(path.is_file());
        assert_eq!(purpose, ViewPurpose::RepoView);
        let (path, purpose) = resolve_item(&imports, "uncompressed", "webview/telemetry").unwrap();
        assert!(path.is_dir());
        assert_eq!(purpose, ViewPurpose::WebView);

        for (section, rel) in [
            ("compressed", "../../etc/passwd"),
            ("compressed", "repo/../../secret.zip"),
            ("compressed", "/abs/path.zip"),
            ("compressed", "repo\\..\\x.zip"),
            ("compressed", "repo/C:evil.zip"),
            ("compressed", "inventory.zip"),            // no purpose folder
            ("compressed", "repo/a/b.zip"),             // too deep
            ("compressed", "repo/notes.txt"),           // not a zip
            ("compressed", "repo/missing.zip"),
            ("uncompressed", "webview/missing"),
            ("uncompressed", "elsewhere/telemetry"),    // not a purpose folder
            ("sideways", "repo/inventory.zip"),
        ] {
            assert!(resolve_item(&imports, section, rel).is_err(), "{section} {rel:?} must be refused");
        }
        std::fs::remove_dir_all(imports).unwrap();
    }
}
