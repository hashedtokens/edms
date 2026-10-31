use crate::core::EdmsCore;
use crate::error::EdmsResult;
use crate::query_loader::QueryMap;
use std::sync::Arc;

/// Row-level tags on a RepoView instance itself — the "Tags (modifiable)"
/// field from the RepoView spec (2026-09-05). Mirrors
/// `CollectionTagMembershipOps` exactly, just against `repoview_tag_memberships`
/// instead of `collection_tag_memberships` — kept as a separate struct rather
/// than generalizing that one, so Collections' already-working code stays
/// untouched.
///
/// The same ops serve WebView's row-level tags (v1.0 gave it the same model):
/// `new_webview` points them at `webview_tag_memberships` (`WT_*` queries).
pub struct RepoviewTagMembershipOps {
    pub core: EdmsCore,
    queries: Arc<QueryMap>,
    /// Query-name prefix: "RT" (repoview) or "WT" (webview).
    prefix: &'static str,
}

impl RepoviewTagMembershipOps {
    pub fn new(db_path: &str) -> Self {
        RepoviewTagMembershipOps {
            core: EdmsCore::new(db_path),
            queries: Arc::new(QueryMap::load()),
            prefix: "RT",
        }
    }

    /// Row-level tags of WebViews instead of RepoViews.
    pub fn new_webview(db_path: &str) -> Self {
        RepoviewTagMembershipOps { prefix: "WT", ..Self::new(db_path) }
    }

    fn query(&self, op: &str) -> &str {
        self.queries.get_merge_query(&format!("{}_{}", self.prefix, op)).unwrap()
    }

    pub fn initialize(&self) -> EdmsResult<()> {
        self.core.connect()
    }

    pub fn shutdown(&self) -> EdmsResult<()> {
        self.core.disconnect()
    }

    pub fn add(&self, repoview_name: &str, tagname: &str) -> EdmsResult<usize> {
        let query = self.query("INSERT");
        self.core.proc(query, &[&repoview_name, &tagname])
    }

    pub fn remove(&self, repoview_name: &str, tagname: &str) -> EdmsResult<usize> {
        let query = self.query("DELETE");
        self.core.proc(query, &[&repoview_name, &tagname])
    }

    pub fn list(&self, repoview_name: &str) -> EdmsResult<Vec<String>> {
        let query = self.query("LIST");
        self.core.cproc(query, &[&repoview_name], |row| row.get(0))
    }

    pub fn repoviews_by_tag(&self, tagname: &str) -> EdmsResult<Vec<String>> {
        let query = self.query("REPOVIEWS_BY_TAG");
        self.core.cproc(query, &[&tagname], |row| row.get(0))
    }
}
