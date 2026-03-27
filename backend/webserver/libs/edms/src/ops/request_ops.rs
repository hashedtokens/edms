use crate::core::EdmsCore;
use crate::error::EdmsResult;
use crate::query_loader::QueryMap;
use std::sync::Arc;

pub struct RequestOps {
    pub core: EdmsCore,
    queries: Arc<QueryMap>,
}

impl RequestOps {
    pub fn new(db_path: &str) -> Self {
        RequestOps {
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
        request_number: i32,
        file_path: &str,
        method: &str,
    ) -> EdmsResult<usize> {
        let query = self.queries.get_request_query("R1").unwrap();
        self.core
            .proc(query, &[&endpoint_id, &request_number, &file_path, &method])
    }

    pub fn get_by_endpoint(
        &self,
        endpoint_id: &str,
    ) -> EdmsResult<Vec<(String, i32, String, String)>> {
        let query = self.queries.get_request_query("R2").unwrap();
        self.core.cproc(query, &[&endpoint_id], |row| {
            Ok((row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
        })
    }

    pub fn get_by_method(&self, method: &str) -> EdmsResult<Vec<(String, i32, String, String)>> {
        let query = self.queries.get_request_query("R4").unwrap();
        self.core.cproc(query, &[&method], |row| {
            Ok((row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
        })
    }

    pub fn count(&self, endpoint_id: &str) -> EdmsResult<i32> {
        let query = self.queries.get_request_query("R5").unwrap();
        let results = self.core.cproc(query, &[&endpoint_id], |row| row.get(0))?;
        Ok(results.first().copied().unwrap_or(0))
    }

    pub fn get_next_number(&self, endpoint_id: &str) -> EdmsResult<i32> {
        let query = self.queries.get_request_query("R6").unwrap();
        let results: Vec<Option<i32>> =
            self.core.cproc(query, &[&endpoint_id], |row| row.get(0))?;

        match results.first() {
            Some(Some(max)) => Ok(max + 1),
            _ => Ok(1),
        }
    }
}
