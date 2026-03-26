/// Return codes:
/// - 0: Success
/// - -1: Unknown failure
/// - 100+: Query-specific errors
use crate::error::{EdmsError, EdmsResult};
use rusqlite::Connection;
use std::path::Path;

pub fn check_sqlite_exists(db_path: &str) -> EdmsResult<()> {
    if Path::new(db_path).exists() {
        Ok(())
    } else {
        Err(EdmsError::SqliteFileNotFound)
    }
}

pub fn check_sqlite_exists_code(db_path: &str) -> i32 {
    match check_sqlite_exists(db_path) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

pub fn check_sqlite_lock(db_path: &str) -> EdmsResult<()> {
    check_sqlite_exists(db_path)?;
    let conn = Connection::open(db_path)?;
    match conn.execute("BEGIN IMMEDIATE", []) {
        Ok(_) => {
            let _ = conn.execute("ROLLBACK", []);
            Ok(())
        }
        Err(e) => {
            if e.to_string().contains("locked") {
                Err(EdmsError::SqliteFileLocked)
            } else {
                Err(EdmsError::SqliteError(e))
            }
        }
    }
}

pub fn check_sqlite_lock_code(db_path: &str) -> i32 {
    match check_sqlite_lock(db_path) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

pub fn check_sqlite_consistency(db_path: &str) -> EdmsResult<String> {
    check_sqlite_exists(db_path)?;

    let conn = Connection::open(db_path)?;

    let result: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;

    if result == "ok" {
        Ok(result)
    } else {
        Err(EdmsError::SqliteConsistencyFailed)
    }
}

pub fn check_sqlite_consistency_code(db_path: &str) -> i32 {
    match check_sqlite_consistency(db_path) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

pub fn execute_sqlite_query(conn: &Connection, query: &str, query_id: i32) -> EdmsResult<usize> {
    conn.execute(query, [])
        .map_err(|_| EdmsError::QueryExecutionFailed(100 + query_id))
}

/// # Returns
/// * `0` - Query successful
/// * `100 + query_id` - Query failed
pub fn execute_sqlite_query_code(conn: &Connection, query: &str, query_id: i32) -> i32 {
    match execute_sqlite_query(conn, query, query_id) {
        Ok(_) => 0,
        Err(EdmsError::QueryExecutionFailed(code)) => code,
        Err(_) => -1,
    }
}

pub fn initialize_sqlite_schema(db_path: &str) -> EdmsResult<Connection> {
    let conn = Connection::open(db_path)?;
    conn.execute("PRAGMA foreign_keys = OFF", [])?;
    crate::schema::initialize_schema(&conn).map_err(EdmsError::SqliteError)?;

    Ok(conn)
}

pub fn initialize_sqlite_schema_code(db_path: &str) -> i32 {
    match initialize_sqlite_schema(db_path) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}
