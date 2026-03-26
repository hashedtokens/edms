use crate::core::EdmsCore;
use crate::error::EdmsResult;
use crate::query_loader::QueryMap;
use std::sync::Arc;

pub struct ResponseOps {
    pub core: EdmsCore,
    queries: Arc<QueryMap>,
}

impl ResponseOps {
    pub fn new(db_path: &str) -> Self {
        ResponseOps {
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
        status_code: i32,
        response_time_ms: Option<i32>,
    ) -> EdmsResult<usize> {
        let query = self.queries.get_response_query("RES1").unwrap();
        self.core.proc(
            query,
            &[
                &endpoint_id,
                &request_number,
                &file_path,
                &status_code,
                &response_time_ms,
            ],
        )
    }

    pub fn get_by_endpoint(
        &self,
        endpoint_id: &str,
    ) -> EdmsResult<Vec<(String, i32, String, i32)>> {
        let query = self.queries.get_response_query("RES2").unwrap();
        self.core.cproc(query, &[&endpoint_id], |row| {
            Ok((row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
        })
    }

    pub fn get_by_status(&self, status_code: i32) -> EdmsResult<Vec<(String, i32, i32)>> {
        let query = self.queries.get_response_query("RES7").unwrap();
        self.core.cproc(query, &[&status_code], |row| {
            Ok((row.get(1)?, row.get(2)?, row.get(4)?))
        })
    }

    pub fn get_all_errors(&self) -> EdmsResult<Vec<(String, i32, i32)>> {
        let query = self.queries.get_response_query("RES3").unwrap();
        self.core.cproc(query, &[], |row| {
            Ok((row.get(1)?, row.get(2)?, row.get(4)?))
        })
    }

    pub fn count_errors_by_endpoint(&self) -> EdmsResult<Vec<(String, i32)>> {
        let query = self.queries.get_response_query("RES6").unwrap();
        self.core
            .cproc(query, &[], |row| Ok((row.get(0)?, row.get(1)?)))
    }

    pub fn count(&self, endpoint_id: &str) -> EdmsResult<i32> {
        let query = self.queries.get_response_query("RES5").unwrap();
        let results = self.core.cproc(query, &[&endpoint_id], |row| row.get(0))?;
        Ok(results.first().copied().unwrap_or(0))
    }

    pub fn avg_response_time(&self, endpoint_id: &str) -> EdmsResult<Option<f64>> {
        let query = self.queries.get_response_query("RES4").unwrap();
        let results: Vec<Option<f64>> =
            self.core.cproc(query, &[&endpoint_id], |row| row.get(0))?;
        Ok(results.first().and_then(|&x| x))
    }
}
