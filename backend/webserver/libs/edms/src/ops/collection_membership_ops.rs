use crate::core::EdmsCore;
use crate::error::EdmsResult;
use crate::query_loader::QueryMap;
use std::sync::Arc;

/// One row from a collection's own file: which endpoint, and when it was
/// added to this collection.
#[derive(Debug, Clone)]
pub struct MembershipEntry {
    pub endpoint_id: String,
    pub added_at: String,
}

/// Operates on ONE collection's own SQLite file — not the shared edms.db.
/// Construct with that specific file's path (e.g.
/// `storage/collections/{name}.sqlite`). The file holds just endpoint IDs +
/// timestamps; the endpoint's real data (URL, tags, request/response
/// history) always lives centrally in the main database — this is a
/// membership list, never a copy.
pub struct CollectionMembershipOps {
    pub core: EdmsCore,
    queries: Arc<QueryMap>,
}

impl CollectionMembershipOps {
    pub fn new(file_path: &str) -> Self {
        CollectionMembershipOps {
            core: EdmsCore::new(file_path),
            queries: Arc::new(QueryMap::load()),
        }
    }

    /// Connects and ensures this collection's own table exists. Safe to
    /// call every time, including against a brand-new empty file — SQLite
    /// creates the file on first connect, and `CREATE TABLE IF NOT EXISTS`
    /// makes the schema step idempotent.
    pub fn initialize(&self) -> EdmsResult<()> {
        self.core.connect()?;
        self.core.proc(
            "CREATE TABLE IF NOT EXISTS membership (
                endpoint_id TEXT NOT NULL UNIQUE,
                added_at    TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            &[],
        )?;
        // Per-endpoint tags carried into this collection (e.g. via the
        // "export current EQP tags" move-to-collection flow) — separate
        // from the central tags table, which stays the source of truth
        // for an endpoint's own tags regardless of collection membership.
        self.core.proc(
            "CREATE TABLE IF NOT EXISTS endpoint_tags (
                endpoint_id TEXT NOT NULL,
                tag         TEXT NOT NULL,
                added_at    TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(endpoint_id, tag)
            )",
            &[],
        )?;
        Ok(())
    }

    pub fn shutdown(&self) -> EdmsResult<()> {
        self.core.disconnect()
    }

    pub fn add(&self, endpoint_id: &str) -> EdmsResult<usize> {
        let q = self
            .queries
            .get_collection_membership_query("ADD")
            .ok_or(crate::error::EdmsError::UnknownError)?;
        self.core.proc(q, &[&endpoint_id])
    }

    pub fn remove(&self, endpoint_id: &str) -> EdmsResult<usize> {
        let q = self
            .queries
            .get_collection_membership_query("REMOVE")
            .ok_or(crate::error::EdmsError::UnknownError)?;
        self.core.proc(q, &[&endpoint_id])
    }

    pub fn list(&self) -> EdmsResult<Vec<MembershipEntry>> {
        let q = self
            .queries
            .get_collection_membership_query("LIST")
            .ok_or(crate::error::EdmsError::UnknownError)?;
        self.core.cproc(q, &[], |row| {
            Ok(MembershipEntry {
                endpoint_id: row.get(0)?,
                added_at: row.get(1)?,
            })
        })
    }

    /// Lists only the endpoint IDs in this collection.
    pub fn list_ids(&self) -> EdmsResult<Vec<String>> {
        let rows = self.list()?;
        Ok(rows.into_iter().map(|m| m.endpoint_id).collect())
    }

    /// Returns the endpoint IDs in this collection as a HashSet for fast O(1) lookups.
    pub fn list_set(&self) -> EdmsResult<std::collections::HashSet<String>> {
        let ids = self.list_ids()?;
        Ok(ids.into_iter().collect())
    }

    /// Adds multiple endpoints to this collection's file in a single transaction.
    pub fn add_batch(&self, endpoint_ids: &[String]) -> EdmsResult<usize> {
        if endpoint_ids.is_empty() {
            return Ok(0);
        }

        let conn_guard = self
            .core
            .base
            .connection
            .lock()
            .map_err(|_| crate::error::EdmsError::SqliteFileLocked)?;
        let conn = conn_guard
            .as_ref()
            .ok_or(crate::error::EdmsError::UnknownError)?;

        conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = (|| -> EdmsResult<usize> {
            let mut count = 0;
            let mut stmt = conn.prepare("INSERT OR IGNORE INTO membership (endpoint_id) VALUES (?)")?;
            for eid in endpoint_ids {
                count += stmt.execute([eid])?;
            }
            Ok(count)
        })();

        match result {
            Ok(count) => {
                conn.execute_batch("COMMIT")?;
                Ok(count)
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    pub fn count(&self) -> EdmsResult<i64> {
        let q = self
            .queries
            .get_collection_membership_query("COUNT")
            .ok_or(crate::error::EdmsError::UnknownError)?;
        let rows: Vec<i64> = self.core.cproc(q, &[], |row| row.get(0))?;
        Ok(rows.first().copied().unwrap_or(0))
    }

    pub fn count_tags_for_endpoint(&self, endpoint_id: &str) -> EdmsResult<i64> {
        let q = self
            .queries
            .get_collection_membership_query("COUNT_TAGS_FOR_ENDPOINT")
            .ok_or(crate::error::EdmsError::UnknownError)?;
        let rows: Vec<i64> = self.core.cproc(q, &[&endpoint_id], |row| row.get(0))?;
        Ok(rows.first().copied().unwrap_or(0))
    }

    pub fn add_tag(&self, endpoint_id: &str, tag: &str) -> EdmsResult<usize> {
        let q = self
            .queries
            .get_collection_membership_query("ADD_TAG")
            .ok_or(crate::error::EdmsError::UnknownError)?;
        self.core.proc(q, &[&endpoint_id, &tag])
    }

    pub fn list_tags_for_endpoint(&self, endpoint_id: &str) -> EdmsResult<Vec<String>> {
        let q = self
            .queries
            .get_collection_membership_query("LIST_TAGS_FOR_ENDPOINT")
            .ok_or(crate::error::EdmsError::UnknownError)?;
        self.core.cproc(q, &[&endpoint_id], |row| row.get(0))
    }

    pub fn list_all_endpoint_tags(&self) -> EdmsResult<Vec<(String, String)>> {
        let q = self
            .queries
            .get_collection_membership_query("LIST_ALL_ENDPOINT_TAGS")
            .ok_or(crate::error::EdmsError::UnknownError)?;
        self.core.cproc(q, &[], |row| Ok((row.get(0)?, row.get(1)?)))
    }
}
