use crate::core::EdmsCore;
use crate::error::EdmsResult;
use rusqlite::{Connection, Result};

pub fn initialize_schema_from_core(core: &EdmsCore) -> EdmsResult<()> {
    let conn_guard = core.base.connection.lock().unwrap();
    let conn = conn_guard.as_ref().unwrap();
    initialize_schema(conn).map_err(crate::error::EdmsError::SqliteError)?;
    Ok(())
}

pub fn initialize_schema(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS endpoints (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            endpoint_id TEXT UNIQUE NOT NULL,
            endpoint_str TEXT NOT NULL,
            annotation TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_endpoints_id ON endpoints(endpoint_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_endpoints_str ON endpoints(endpoint_str)",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS request_metadata (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            endpoint_id TEXT NOT NULL,
            request_number INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            method TEXT,
            timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_request_endpoint ON request_metadata(endpoint_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_request_timestamp ON request_metadata(timestamp)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_request_method ON request_metadata(method)",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS response_metadata (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            endpoint_id TEXT NOT NULL,
            request_number INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            status_code INTEGER,
            exit_code INTEGER,
            response_time_ms INTEGER,
            timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_response_endpoint ON response_metadata(endpoint_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_response_status ON response_metadata(status_code)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_response_timestamp ON response_metadata(timestamp)",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS metadata (
            endpoint_id TEXT PRIMARY KEY,
            request_count INTEGER DEFAULT 0,
            response_count INTEGER DEFAULT 0,
            data_size_bytes INTEGER DEFAULT 0,
            last_tested TIMESTAMP,
            avg_response_time_ms INTEGER
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS tags (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            endpoint_id TEXT NOT NULL,
            tag TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(endpoint_id, tag)
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_tags_endpoint ON tags(endpoint_id)",
        [],
    )?;

    conn.execute("CREATE INDEX IF NOT EXISTS idx_tags_tag ON tags(tag)", [])?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS bookmarks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            endpoint_id TEXT NOT NULL,
            folder TEXT,
            notes TEXT,
            timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_bookmarks_endpoint ON bookmarks(endpoint_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_bookmarks_folder ON bookmarks(folder)",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            endpoint_id TEXT NOT NULL,
            action TEXT NOT NULL,
            details TEXT,
            timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_history_endpoint ON history(endpoint_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_history_timestamp ON history(timestamp)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_history_action ON history(action)",
        [],
    )?;

    Ok(())
}
