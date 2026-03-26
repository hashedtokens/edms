use crate::core::EdmsCore;
use crate::error::EdmsResult;
use crate::query_loader::QueryMap;
use std::sync::Arc;

pub struct EndpointOps {
    pub core: EdmsCore,
    queries: Arc<QueryMap>,
}

impl EndpointOps {
    pub fn new(db_path: &str) -> Self {
        EndpointOps {
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

    pub fn insert(
        &self,
        endpoint_id: &str,
        endpoint_str: &str,
        annotation: Option<&str>,
    ) -> EdmsResult<usize> {
        let query = self.queries.get_endpoint_query("E1").unwrap();
        self.core
            .proc(query, &[&endpoint_id, &endpoint_str, &annotation])
    }

    pub fn get_by_id(
        &self,
        endpoint_id: &str,
    ) -> EdmsResult<Vec<(String, String, Option<String>)>> {
        let query = self.queries.get_endpoint_query("E2").unwrap();
        self.core.cproc(query, &[&endpoint_id], |row| {
            Ok((row.get(1)?, row.get(2)?, row.get(3)?))
        })
    }

    pub fn get_all(&self) -> EdmsResult<Vec<(String, String, Option<String>)>> {
        let query = self.queries.get_endpoint_query("E3").unwrap();
        self.core.cproc(query, &[], |row| {
            Ok((row.get(1)?, row.get(2)?, row.get(3)?))
        })
    }

    pub fn update_annotation(&self, annotation: &str, endpoint_id: &str) -> EdmsResult<usize> {
        let query = self.queries.get_endpoint_query("E4").unwrap();
        self.core.proc(query, &[&annotation, &endpoint_id])
    }

    pub fn delete(&self, endpoint_id: &str) -> EdmsResult<usize> {
        let query = self.queries.get_endpoint_query("E5").unwrap();
        self.core.proc(query, &[&endpoint_id])
    }

    pub fn count(&self) -> EdmsResult<i32> {
        let query = self.queries.get_endpoint_query("E6").unwrap();
        let results = self.core.cproc(query, &[], |row| row.get(0))?;
        Ok(results.first().copied().unwrap_or(0))
    }
}
