use crate::error::EdmsResult;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub struct EdmsBase {
    pub db_path: String,
    pub connection: Arc<Mutex<Option<Connection>>>,
}

impl EdmsBase {
    pub fn new(db_path: &str) -> Self {
        EdmsBase {
            db_path: db_path.to_string(),
            connection: Arc::new(Mutex::new(None)),
        }
    }

    pub fn connect(&self) -> EdmsResult<()> {
        let mut conn_guard = self
            .connection
            .lock()
            .map_err(|_| crate::error::EdmsError::SqliteFileLocked)?;

        if conn_guard.is_some() {
            println!("[NOTIFICATION] Connection already exists: {}", self.db_path);
            return Ok(());
        }

        let conn = Connection::open(&self.db_path)?;
        conn.execute("PRAGMA foreign_keys = OFF", [])?;
        *conn_guard = Some(conn);

        println!("[NOTIFICATION] Connected: {}", self.db_path);
        Ok(())
    }

    pub fn disconnect(&self) -> EdmsResult<()> {
        let mut conn_guard = self
            .connection
            .lock()
            .map_err(|_| crate::error::EdmsError::SqliteFileLocked)?;

        if conn_guard.is_none() {
            println!("[NOTIFICATION] No active connection: {}", self.db_path);
            return Ok(());
        }

        *conn_guard = None;
        println!("[NOTIFICATION] Disconnected: {}", self.db_path);
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        if let Ok(guard) = self.connection.lock() {
            guard.is_some()
        } else {
            false
        }
    }

    pub fn execute(&self, query: &str, params: &[&dyn rusqlite::ToSql]) -> EdmsResult<usize> {
        let conn_guard = self
            .connection
            .lock()
            .map_err(|_| crate::error::EdmsError::SqliteFileLocked)?;

        let conn = conn_guard
            .as_ref()
            .ok_or(crate::error::EdmsError::UnknownError)?;

        let rows = conn.execute(query, params)?;
        println!("[QUERY] Executed (rows: {rows})");
        Ok(rows)
    }
}
