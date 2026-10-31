//! RepoView and WebView are the same thing to the backend (Ravi, v1.0: "UI is
//! 99% identical for both"): a list - a SQLite index of members plus
//! snapshots - with no EQP data, built from a Collection, combined, taken out
//! and imported the same way. They differ in names (catalog table, folder,
//! route prefix, row-tag table), in what else the folder holds (a RepoView
//! has `Tables-*.md`; a WebView has `front-page.json`), and in where imports
//! are read from. `Flavor` carries those differences so one set of handlers
//! serves both; the routes are registered once per flavor with the flavor
//! attached as an `Extension` (see `main.rs`).

use axum::{http::StatusCode, Json};
use edms::ops::view_ops::ViewKind;
use serde_json::{json, Value};

use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavor {
    Repo,
    Web,
}

impl Flavor {
    pub fn kind(self) -> ViewKind {
        match self {
            Flavor::Repo => ViewKind::Repoview,
            Flavor::Web => ViewKind::Webview,
        }
    }

    /// For messages: "RepoView 'x' does not exist".
    pub fn label(self) -> &'static str {
        match self {
            Flavor::Repo => "RepoView",
            Flavor::Web => "WebView",
        }
    }

    /// The route prefix, without slashes.
    pub fn route(self) -> &'static str {
        match self {
            Flavor::Repo => "repoview",
            Flavor::Web => "webview",
        }
    }

    /// `storage/{this}/{name}/`
    pub fn storage_folder(self) -> &'static str {
        match self {
            Flavor::Repo => "repoview",
            Flavor::Web => "webview",
        }
    }

    /// The index file inside a view's folder.
    pub fn index_file(self) -> &'static str {
        match self {
            Flavor::Repo => "repoview.sqlite",
            Flavor::Web => "webview.sqlite",
        }
    }

    /// Where a takeout from elsewhere is placed for import:
    /// `storage/imports/uncompressed/{this}/{folder}` (compute's folder schema).
    pub fn import_folder(self) -> &'static str {
        match self {
            Flavor::Repo => "repo",
            Flavor::Web => "webview",
        }
    }

    /// Key into the YAML catalog queries, e.g. `catalog_query("GET")`.
    pub fn catalog_query(self, op: &str) -> String {
        match self {
            Flavor::Repo => format!("REPOVIEW_{op}"),
            Flavor::Web => format!("WEBVIEW_{op}"),
        }
    }

    /// Central table of row-level tags and its name column.
    pub fn tag_table(self) -> (&'static str, &'static str) {
        match self {
            Flavor::Repo => ("repoview_tag_memberships", "repoview_name"),
            Flavor::Web => ("webview_tag_memberships", "webview_name"),
        }
    }

    /// Only RepoViews carry generated `Tables-*.md`; a WebView carries
    /// `front-page.json` instead.
    pub fn has_tables(self) -> bool {
        self == Flavor::Repo
    }

    pub fn row_tag_ops(self, state: &AppState) -> Result<edms::ops::repoview_tag_ops::RepoviewTagMembershipOps, String> {
        let path = state.db_path.display().to_string();
        let ops = match self {
            Flavor::Repo => edms::ops::repoview_tag_ops::RepoviewTagMembershipOps::new(&path),
            Flavor::Web => edms::ops::repoview_tag_ops::RepoviewTagMembershipOps::new_webview(&path),
        };
        ops.initialize().map_err(|e| format!("{e:?}"))?;
        Ok(ops)
    }

    pub fn dir(self, state: &AppState, name: &str) -> std::path::PathBuf {
        state.storage_root.join("storage").join(self.storage_folder()).join(name)
    }

    pub fn file_path(self, state: &AppState, name: &str) -> String {
        self.dir(state, name).join(self.index_file()).display().to_string()
    }

    /// A name becomes a real folder name and `delete` removes that whole
    /// folder, so it must never point anywhere else (see
    /// `validate_folder_name`), and mustn't be a word that is a fixed route
    /// under this prefix (it could never be fetched by name).
    pub fn validate_name(self, name: &str) -> Result<(), String> {
        crate::handlers::view_catalog::validate_folder_name(name, &format!("{} name", self.label()))?;
        const RESERVED: [&str; 7] = ["list", "create", "delete", "import", "combine", "tags", "by-tag"];
        if RESERVED.iter().any(|word| name.eq_ignore_ascii_case(word)) {
            return Err(format!(
                "'{name}' is reserved (it's a fixed /{} route) - pick another {} name",
                self.route(),
                self.label()
            ));
        }
        Ok(())
    }

    /// 404 body for a missing view; 400 for everything else uses `bad`.
    pub fn not_found(self, name: &str) -> String {
        format!("{} '{name}' does not exist", self.label())
    }
}

pub fn bad(e: String) -> (StatusCode, Json<Value>) {
    (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_flavor_has_its_own_names() {
        assert_eq!(Flavor::Repo.route(), "repoview");
        assert_eq!(Flavor::Web.route(), "webview");
        assert_eq!(Flavor::Repo.catalog_query("GET"), "REPOVIEW_GET");
        assert_eq!(Flavor::Web.catalog_query("CREATE"), "WEBVIEW_CREATE");
        assert_eq!(Flavor::Repo.tag_table(), ("repoview_tag_memberships", "repoview_name"));
        assert_eq!(Flavor::Web.tag_table(), ("webview_tag_memberships", "webview_name"));
        // different folders and index files, so one can never overwrite the other
        assert_ne!(Flavor::Repo.storage_folder(), Flavor::Web.storage_folder());
        assert_ne!(Flavor::Repo.index_file(), Flavor::Web.index_file());
        assert_ne!(Flavor::Repo.import_folder(), Flavor::Web.import_folder());
    }

    #[test]
    fn only_a_repoview_has_tables() {
        assert!(Flavor::Repo.has_tables());
        assert!(!Flavor::Web.has_tables());
    }

    #[test]
    fn names_are_checked_the_same_way_with_the_right_wording() {
        for flavor in [Flavor::Repo, Flavor::Web] {
            assert!(flavor.validate_name("release-1").is_ok());
            for bad in ["..", "a/b", "", "list", "COMBINE", "by-tag"] {
                assert!(flavor.validate_name(bad).is_err(), "{bad:?}");
            }
        }
        assert!(Flavor::Web.validate_name("combine").unwrap_err().contains("/webview route"));
        assert!(Flavor::Repo.validate_name("combine").unwrap_err().contains("/repoview route"));
        assert_eq!(Flavor::Web.not_found("x"), "WebView 'x' does not exist");
    }
}
