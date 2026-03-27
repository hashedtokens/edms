use crate::base::EdmsBase;
use crate::error::EdmsResult;
use rusqlite::Row;
use std::collections::HashMap;

pub struct EdmsCore {
    pub base: EdmsBase,
    pub mq: HashMap<&'static str, &'static str>,
}

impl EdmsCore {
    pub fn new(db_path: &str) -> Self {
        let mut mq = HashMap::new();

        // only keep Q3 which is used by list_tables()
        mq.insert(
            "Q3",
            "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name",
        );

        EdmsCore {
            base: EdmsBase::new(db_path),
            mq,
        }
    }

    pub fn connect(&self) -> EdmsResult<()> {
        self.base.connect()
    }

    pub fn disconnect(&self) -> EdmsResult<()> {
        self.base.disconnect()
    }

    pub fn proc(&self, query: &str, params: &[&dyn rusqlite::ToSql]) -> EdmsResult<usize> {
        self.base.execute(query, params)
    }

    pub fn cproc<T, F>(
        &self,
        query: &str,
        params: &[&dyn rusqlite::ToSql],
        mut processor: F,
    ) -> EdmsResult<Vec<T>>
    where
        F: FnMut(&Row) -> Result<T, rusqlite::Error>,
    {
        let conn_guard = self
            .base
            .connection
            .lock()
            .map_err(|_| crate::error::EdmsError::SqliteFileLocked)?;

        let conn = conn_guard
            .as_ref()
            .ok_or(crate::error::EdmsError::UnknownError)?;

        let mut stmt = conn.prepare(query)?;
        let rows = stmt.query_map(params, |row| processor(row))?;
        let mut results = Vec::new();
        for row_result in rows {
            results.push(row_result?);
        }

        println!("[QUERY] Processed (results: {})", results.len());
        Ok(results)
    }

    pub fn list_tables(&self) -> EdmsResult<Vec<String>> {
        let query = self.mq.get("Q3").unwrap();
        self.cproc(query, &[], |row| row.get(0))
    }

    pub fn table_exists(&self, table_name: &str) -> EdmsResult<bool> {
        let query = "SELECT name FROM sqlite_master WHERE type='table' AND name = ?";
        let result = self.cproc(query, &[&table_name], |row| row.get::<_, String>(0))?;
        Ok(!result.is_empty())
    }
}
