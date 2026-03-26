use crate::core::EdmsCore;
use crate::error::EdmsResult;
use crate::query_loader::QueryMap;
use std::sync::Arc;

pub struct TagOps {
    pub core: EdmsCore,
    queries: Arc<QueryMap>,
}

impl TagOps {
    pub fn new(db_path: &str) -> Self {
        TagOps {
            core: EdmsCore::new(db_path),
            queries: Arc::new(QueryMap::load_or_default()),
        }
    }

    pub fn initialize(&self) -> EdmsResult<()> {
        self.core.connect()
    }

    pub fn shutdown(&self) -> EdmsResult<()> {
        self.core.disconnect()
    }

    pub fn add(&self, endpoint_id: &str, tag: &str) -> EdmsResult<usize> {
        let query = self.queries.get_tag_query("T1").unwrap();
        self.core.proc(query, &[&endpoint_id, &tag])
    }

    pub fn get_by_endpoint(&self, endpoint_id: &str) -> EdmsResult<Vec<String>> {
        let query = self.queries.get_tag_query("T2").unwrap();
        self.core.cproc(query, &[&endpoint_id], |row| row.get(0))
    }

    pub fn get_endpoints_by_tag(&self, tag: &str) -> EdmsResult<Vec<String>> {
        let query = self.queries.get_tag_query("T3").unwrap();
        self.core.cproc(query, &[&tag], |row| row.get(0))
    }

    pub fn remove(&self, endpoint_id: &str, tag: &str) -> EdmsResult<usize> {
        let query = self.queries.get_tag_query("T4").unwrap();
        self.core.proc(query, &[&endpoint_id, &tag])
    }

    pub fn remove_all(&self, endpoint_id: &str) -> EdmsResult<usize> {
        let query = self.queries.get_tag_query("T5").unwrap();
        self.core.proc(query, &[&endpoint_id])
    }

    pub fn count(&self, endpoint_id: &str) -> EdmsResult<i32> {
        let query = self.queries.get_tag_query("T6").unwrap();
        let results = self.core.cproc(query, &[&endpoint_id], |row| row.get(0))?;
        Ok(results.first().copied().unwrap_or(0))
    }

    pub fn get_popular_tags(&self) -> EdmsResult<Vec<(String, i32)>> {
        let query = self.queries.get_tag_query("T7").unwrap();
        self.core
            .cproc(query, &[], |row| Ok((row.get(0)?, row.get(1)?)))
    }
}
