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
    // 1. Endpoints
    conn.execute(
        "CREATE TABLE IF NOT EXISTS endpoints (
            endpoint_id TEXT PRIMARY KEY,
            endpoint_str TEXT NOT NULL,
            annotation TEXT,
            method TEXT NOT NULL DEFAULT 'GET',
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    // Migration for endpoints: drop surrogate `id` if present, normalize method
    let has_ep_id: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('endpoints') WHERE name = 'id'",
        [],
        |row| row.get(0),
    )?;
    if has_ep_id > 0 {
        conn.execute(
            "CREATE TABLE endpoints_new (
                endpoint_id TEXT PRIMARY KEY,
                endpoint_str TEXT NOT NULL,
                annotation TEXT,
                method TEXT NOT NULL DEFAULT 'GET',
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;
        conn.execute(
            "INSERT INTO endpoints_new (endpoint_id, endpoint_str, annotation, method, created_at, updated_at)
             SELECT endpoint_id, endpoint_str, annotation, COALESCE(method, 'GET'), created_at, updated_at FROM endpoints",
            [],
        )?;
        conn.execute("DROP TABLE endpoints", [])?;
        conn.execute("ALTER TABLE endpoints_new RENAME TO endpoints", [])?;
    } else {
        let has_method: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('endpoints') WHERE name = 'method'",
            [],
            |row| row.get(0),
        )?;
        if has_method == 0 {
            conn.execute("ALTER TABLE endpoints ADD COLUMN method TEXT NOT NULL DEFAULT 'GET'", [])?;
        }
    }

    // Drop redundant index on endpoint_id (now PRIMARY KEY)
    conn.execute("DROP INDEX IF EXISTS idx_endpoints_id", [])?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_endpoints_str ON endpoints(endpoint_str)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_endpoints_str_method ON endpoints(endpoint_str, method)",
        [],
    )?;

    // 2. Request Metadata
    conn.execute(
        "CREATE TABLE IF NOT EXISTS request_metadata (
            endpoint_id TEXT NOT NULL,
            request_number INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            method TEXT,
            timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    let has_req_id: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('request_metadata') WHERE name = 'id'",
        [],
        |row| row.get(0),
    )?;
    if has_req_id > 0 {
        conn.execute(
            "CREATE TABLE request_metadata_new (
                endpoint_id TEXT NOT NULL,
                request_number INTEGER NOT NULL,
                file_path TEXT NOT NULL,
                method TEXT,
                timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;
        conn.execute(
            "INSERT INTO request_metadata_new (endpoint_id, request_number, file_path, method, timestamp)
             SELECT endpoint_id, request_number, file_path, method, timestamp FROM request_metadata",
            [],
        )?;
        conn.execute("DROP TABLE request_metadata", [])?;
        conn.execute("ALTER TABLE request_metadata_new RENAME TO request_metadata", [])?;
    }

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

    // 3. Response Metadata
    conn.execute(
        "CREATE TABLE IF NOT EXISTS response_metadata (
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

    let has_resp_id: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('response_metadata') WHERE name = 'id'",
        [],
        |row| row.get(0),
    )?;
    if has_resp_id > 0 {
        conn.execute(
            "CREATE TABLE response_metadata_new (
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
            "INSERT INTO response_metadata_new (endpoint_id, request_number, file_path, status_code, exit_code, response_time_ms, timestamp)
             SELECT endpoint_id, request_number, file_path, status_code, exit_code, response_time_ms, timestamp FROM response_metadata",
            [],
        )?;
        conn.execute("DROP TABLE response_metadata", [])?;
        conn.execute("ALTER TABLE response_metadata_new RENAME TO response_metadata", [])?;
    }

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

    // 4. Metadata (summary)
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

    // 5. Tags
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tags (
            endpoint_id TEXT NOT NULL,
            tag TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (endpoint_id, tag)
        )",
        [],
    )?;

    let has_tags_id: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('tags') WHERE name = 'id'",
        [],
        |row| row.get(0),
    )?;
    if has_tags_id > 0 {
        conn.execute(
            "CREATE TABLE tags_new (
                endpoint_id TEXT NOT NULL,
                tag TEXT NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (endpoint_id, tag)
            )",
            [],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO tags_new (endpoint_id, tag, created_at)
             SELECT endpoint_id, tag, created_at FROM tags",
            [],
        )?;
        conn.execute("DROP TABLE tags", [])?;
        conn.execute("ALTER TABLE tags_new RENAME TO tags", [])?;
    }

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_tags_endpoint ON tags(endpoint_id)",
        [],
    )?;

    conn.execute("CREATE INDEX IF NOT EXISTS idx_tags_tag ON tags(tag)", [])?;

    // 6. Bookmarks
    conn.execute(
        "CREATE TABLE IF NOT EXISTS bookmarks (
            endpoint_id TEXT NOT NULL,
            folder TEXT,
            notes TEXT,
            timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(endpoint_id, folder)
        )",
        [],
    )?;

    let has_bm_id: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('bookmarks') WHERE name = 'id'",
        [],
        |row| row.get(0),
    )?;
    if has_bm_id > 0 {
        let has_unique: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pragma_index_list('bookmarks') WHERE origin = 'u'",
            [], |r| r.get(0)
        ).unwrap_or(0);

        if has_unique == 0 {
            // Dedup: prefer row with non-null notes, then most recent (MAX id)
            conn.execute("
                DELETE FROM bookmarks
                WHERE id NOT IN (
                    SELECT CASE
                        WHEN MAX(CASE WHEN notes IS NOT NULL THEN id ELSE 0 END) > 0
                             THEN MAX(CASE WHEN notes IS NOT NULL THEN id ELSE NULL END)
                        ELSE MAX(id)
                    END
                    FROM bookmarks GROUP BY endpoint_id, folder
                )", []
            )?;
        }

        conn.execute("
            CREATE TABLE bookmarks_new (
                endpoint_id TEXT NOT NULL,
                folder      TEXT,
                notes       TEXT,
                timestamp   TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(endpoint_id, folder)
            )", []
        )?;
        conn.execute("INSERT OR IGNORE INTO bookmarks_new (endpoint_id, folder, notes, timestamp) SELECT endpoint_id, folder, notes, timestamp FROM bookmarks", [])?;
        conn.execute("DROP TABLE bookmarks", [])?;
        conn.execute("ALTER TABLE bookmarks_new RENAME TO bookmarks", [])?;
    }

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_bookmarks_endpoint ON bookmarks(endpoint_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_bookmarks_folder ON bookmarks(folder)",
        [],
    )?;

    // 7. History (KEPT UNTOUCHED with id INTEGER PRIMARY KEY AUTOINCREMENT)
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

    // 8. Catalog tables: collections, webview, repoview
    for table in ["collections", "webview", "repoview"] {
        conn.execute(
            &format!(
                "CREATE TABLE IF NOT EXISTS {table} (
                    name TEXT PRIMARY KEY,
                    file_path TEXT,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )"
            ),
            [],
        )?;

        let has_cat_id: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = 'id'"),
            [],
            |row| row.get(0),
        )?;
        if has_cat_id > 0 {
            conn.execute(
                &format!(
                    "CREATE TABLE {table}_new (
                        name TEXT PRIMARY KEY,
                        file_path TEXT,
                        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                    )"
                ),
                [],
            )?;
            conn.execute(
                &format!("INSERT OR IGNORE INTO {table}_new (name, file_path, created_at) SELECT name, file_path, created_at FROM {table}"),
                [],
            )?;
            conn.execute(&format!("DROP TABLE {table}"), [])?;
            conn.execute(&format!("ALTER TABLE {table}_new RENAME TO {table}"), [])?;
        }
    }

    // 9. Central tag-count rollups
    for table in ["collections_tags", "webview_tags", "repoview_tags"] {
        conn.execute(
            &format!(
                "CREATE TABLE IF NOT EXISTS {table} (
                    tagname TEXT NOT NULL UNIQUE,
                    count INTEGER NOT NULL DEFAULT 0
                )"
            ),
            [],
        )?;
    }

    // 10. Per-collection tag memberships (C(T)) for merge operations
    conn.execute(
        "CREATE TABLE IF NOT EXISTS collection_tag_memberships (
            collection_name TEXT NOT NULL,
            tagname TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (collection_name, tagname)
        )",
        [],
    )?;

    // 11. EID allocation tracking table for gap-list allocator
    conn.execute(
        "CREATE TABLE IF NOT EXISTS eid_allocation (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            watermark INTEGER NOT NULL DEFAULT 0,
            gaps_json TEXT NOT NULL DEFAULT '[]'
        )",
        [],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO eid_allocation (id, watermark, gaps_json) VALUES (1, 0, '[]')",
        [],
    )?;

    Ok(())
}
