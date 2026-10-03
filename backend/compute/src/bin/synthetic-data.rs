//! Offline demo storage. Uses the application's schema and QP writers.
use compute::{endpoint_writer, folder_manager::FolderLayout};
use rusqlite::{Connection, params};
use serde_json::json;
use std::{error::Error, fs, io::Write, path::Path};
use walkdir::WalkDir;
use zip::{ZipWriter, write::SimpleFileOptions};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const ENDPOINT_COUNT: u64 = 25;
const TIME: &str = "2026-09-01T12:00:00Z";

// true means initialized; false means an existing installation was skipped.
fn seed(root: &Path, db_path: &Path) -> Result<bool> {
    // Never mix synthetic records with an existing installation.
    if !root.is_dir() {
        return Err("Create the storage directory before initializing dummy data".into());
    }
    if fs::read_dir(root)?.next().is_some() {
        return Ok(false);
    }
    if db_path.exists() {
        return Ok(false);
    }
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }
    FolderLayout::new(root).create_edmsfolders()?;
    let db = Connection::open(db_path)?;
    edms::schema::initialize_schema(&db)?;
    for index in 1..=ENDPOINT_COUNT {
        let (method, status) = [("GET", 200), ("POST", 201), ("PUT", 200), ("DELETE", 204)]
            [((index - 1) % 4) as usize];
        let eid = compute::eid::format_eid(index);
        let url = format!("https://example.invalid/demo/items/{index}");
        db.execute("INSERT INTO endpoints(endpoint_id, endpoint_str, method, annotation, created_at, updated_at) VALUES (?1, ?2, ?3, 'Synthetic demo; no network request was made', ?4, ?4)", params![eid, url, method, TIME])?;
        let dir = root.join("storage/globalEQPData").join(&eid);
        for number in 1..=2 {
            endpoint_writer::write_request_file(
                &dir,
                &eid,
                number,
                &json!({"demo": true, "item_id": index, "revision": number}).to_string(),
            )?;
            endpoint_writer::write_response_file(
                &dir,
                &eid,
                number,
                &json!({"demo": true, "id": index, "status": status}).to_string(),
            )?;
            endpoint_writer::write_headers_file(&dir, &eid, number, &json!({"request_headers": {"accept": "application/json"}, "response_headers": {"content-type": "application/json", "x-edms-synthetic": "true"}}).to_string())?;
            let request = dir.join(format!("{eid}-request-{number}.json"));
            let response = dir.join(format!("{eid}-response-{number}.json"));
            db.execute("INSERT INTO request_metadata(endpoint_id, request_number, file_path, method, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)", params![eid, number as i64, request.to_string_lossy(), method, TIME])?;
            db.execute("INSERT INTO response_metadata(endpoint_id, request_number, file_path, status_code, exit_code, response_time_ms, timestamp) VALUES (?1, ?2, ?3, ?4, 0, 25, ?5)", params![eid, number as i64, response.to_string_lossy(), status, TIME])?;
        }
        let bytes = fs::read_dir(&dir)?.try_fold(0u64, |total, entry| {
            Ok::<_, std::io::Error>(total + entry?.metadata()?.len())
        })?;
        db.execute(
            "INSERT INTO metadata VALUES (?1, 2, 2, ?2, ?3, 25)",
            params![eid, i64::try_from(bytes)?, TIME],
        )?;
        db.execute(
            "INSERT INTO tags(endpoint_id, tag, created_at) VALUES (?1, 'synthetic', ?2)",
            params![eid, TIME],
        )?;
        db.execute("INSERT INTO bookmarks(endpoint_id, folder, notes, timestamp) VALUES (?1, 'demo-items', 'Synthetic bookmark', ?2)", params![eid, TIME])?;
        db.execute("INSERT INTO history(endpoint_id, action, details, timestamp) VALUES (?1, 'test', 'Synthetic 25ms result; no request sent', ?2)", params![eid, TIME])?;
    }
    db.execute(
        "UPDATE eid_allocation SET watermark=?1 WHERE id=1",
        [ENDPOINT_COUNT as i64],
    )?;
    let collection_path = root.join("storage/collections/demo-items.sqlite");
    let collection = Connection::open(&collection_path)?;
    collection.execute_batch("CREATE TABLE membership(endpoint_id TEXT NOT NULL UNIQUE, added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP); CREATE TABLE endpoint_tags(endpoint_id TEXT NOT NULL, tag TEXT NOT NULL, added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, UNIQUE(endpoint_id,tag));")?;
    for index in 1..=ENDPOINT_COUNT {
        let eid = compute::eid::format_eid(index);
        collection.execute("INSERT INTO membership VALUES (?1, ?2)", params![eid, TIME])?;
        collection.execute(
            "INSERT INTO endpoint_tags VALUES (?1, 'synthetic', ?2)",
            params![eid, TIME],
        )?;
    }
    db.execute("INSERT INTO collections(name,file_path,annotation,created_at) VALUES ('demo-items',?1,'Synthetic items collection',?2)", params![collection_path.to_string_lossy(), TIME])?;
    db.execute(
        "INSERT INTO collection_tag_memberships VALUES ('demo-items','synthetic',?1)",
        [TIME],
    )?;
    for table in ["collections_tags", "webview_tags", "repoview_tags"] {
        db.execute(&format!("INSERT INTO {table} VALUES ('synthetic',1)"), [])?;
    }
    for table in ["webview", "repoview"] {
        // These catalogs currently have no backing SQLite file in EDMS.
        db.execute(&format!("INSERT INTO {table}(name,annotation,created_at) VALUES ('demo-items','Synthetic preview',?1)"), [TIME])?;
    }
    drop(collection);
    drop(db);
    fs::write(
        root.join("account-data/demo.json"),
        json!({"synthetic": true, "name": "Demo workspace", "authenticated": false}).to_string(),
    )?;
    fs::write(
        root.join("storage/history/demo.json"),
        json!({"synthetic": true, "timestamp": TIME, "action": "initialize-demo"}).to_string(),
    )?;
    let markdown = "# Synthetic EDMS data\n\n25 example.invalid endpoints with two QP pairs each. No network calls were made.\n";
    let html = "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>Synthetic EDMS data</title><h1>Synthetic EDMS data</h1><p>25 offline demo endpoints.</p></html>";
    for folder in [
        "repo",
        "session-backup",
        "active",
        "exports",
        "temp",
        "docs",
        "storage/repoview",
        "storage/webview",
        "storage/takeout",
    ] {
        fs::write(root.join(folder).join("demo.md"), markdown)?;
    }
    fs::write(root.join("storage/webview/index.html"), html)?;
    for direction in ["imports", "exports"] {
        for purpose in ["repo", "collections", "webview"] {
            let base = root.join(format!(
                "storage/{direction}/uncompressed/{purpose}/demo-items"
            ));
            fs::create_dir_all(&base)?;
            fs::write(base.join("README.md"), markdown)?;
            copy_tree(
                &root.join("storage/globalEQPData"),
                &base.join("globalEQPData"),
            )?;
            if purpose == "webview" {
                fs::write(base.join("index.html"), html)?;
            } else {
                fs::copy(db_path, base.join("edms.sqlite"))?;
            }
            let archive = root.join(format!(
                "storage/{direction}/compressed/{purpose}/demo-items.zip"
            ));
            let mut zip = ZipWriter::new(fs::File::create(archive)?);
            for entry in WalkDir::new(&base) {
                let entry = entry?;
                if entry.file_type().is_file() {
                    zip.start_file(
                        entry.path().strip_prefix(&base)?.to_string_lossy(),
                        SimpleFileOptions::default(),
                    )?;
                    zip.write_all(&fs::read(entry.path())?)?;
                }
            }
            zip.finish()?;
        }
    }
    fs::write(
        root.join("app.log"),
        "2026-09-01T12:00:00Z INFO synthetic demo initialized offline\n",
    )?;
    Ok(true)
}

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    for entry in WalkDir::new(source) {
        let entry = entry?;
        let target = destination.join(entry.path().strip_prefix(source)?);
        if entry.file_type().is_dir() {
            fs::create_dir_all(target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let root = std::env::args_os()
        .nth(1)
        .ok_or("Usage: synthetic-data <existing-empty-storage-directory> [database-path]")?;
    let root = Path::new(&root);
    let db_path = std::env::args_os()
        .nth(2)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("edms.db"));
    if seed(root, &db_path)? {
        println!(
            "Synthetic storage initialized: 25 endpoints, 50 QPs, collection and view samples, import/export archives."
        );
    } else {
        println!("Existing storage or database found; synthetic seeding skipped without changes.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_database_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("storage-root");
        fs::create_dir(&root).unwrap();
        let db = dir.path().join("existing.db");
        fs::write(&db, "keep me").unwrap();
        assert!(!seed(&root, &db).unwrap());
        assert_eq!(fs::read_to_string(db).unwrap(), "keep me");
        assert!(fs::read_dir(root).unwrap().next().is_none());
    }

    #[test]
    fn populated_storage_without_database_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("keep.txt"), "existing storage").unwrap();
        let db = dir.path().join("edms.db");
        assert!(!seed(dir.path(), &db).unwrap());
        assert!(!db.exists());
        assert_eq!(
            fs::read_to_string(dir.path().join("keep.txt")).unwrap(),
            "existing storage"
        );
    }

    #[test]
    fn storage_matches_app_schema_and_preserves_existing_data() {
        let dir = tempfile::tempdir().unwrap();
        assert!(seed(dir.path(), &dir.path().join("edms.db")).unwrap());
        let db = Connection::open(dir.path().join("edms.db")).unwrap();
        edms::schema::initialize_schema(&db).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM endpoints", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            25
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM response_metadata", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            50
        );
        assert_eq!(
            db.query_row("SELECT watermark FROM eid_allocation", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            25
        );
        let allocated =
            compute::eid::EidAllocator::new(&dir.path().join("edms.db").to_string_lossy());
        allocated.initialize().unwrap();
        assert_eq!(allocated.allocate().unwrap(), "E0026-AAA");
        assert!(matches!(
            FolderLayout::new(dir.path()).verify_edmsfolders(),
            compute::folder_manager::FolderStatus::Ok
        ));
        for entry in WalkDir::new(dir.path())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_dir())
        {
            assert!(
                fs::read_dir(entry.path()).unwrap().next().is_some(),
                "empty folder: {:?}",
                entry.path()
            );
        }
        let imports = dir.path().join("storage/imports/uncompressed");
        assert!(
            compute::validate::validate_bookmark_format(&imports.join("repo/demo-items")).passed
        );
        assert!(
            compute::validate::validate_bookmark_format(&imports.join("collections/demo-items"))
                .passed
        );
        assert!(
            compute::validate::validate_webview_format(&imports.join("webview/demo-items")).passed
        );
        for direction in ["imports", "exports"] {
            for purpose in ["repo", "collections", "webview"] {
                let file = fs::File::open(dir.path().join(format!(
                    "storage/{direction}/compressed/{purpose}/demo-items.zip"
                )))
                .unwrap();
                let mut zip = zip::ZipArchive::new(file).unwrap();
                for n in 0..zip.len() {
                    std::io::copy(&mut zip.by_index(n).unwrap(), &mut std::io::sink()).unwrap();
                }
            }
        }
        let before: Vec<_> = WalkDir::new(dir.path())
            .into_iter()
            .map(|e| e.unwrap())
            .filter(|e| e.file_type().is_file())
            .map(|e| (e.path().to_owned(), fs::read(e.path()).unwrap()))
            .collect();
        assert!(!seed(dir.path(), &dir.path().join("edms.db")).unwrap());
        for (path, content) in before {
            assert_eq!(fs::read(path).unwrap(), content);
        }
        assert_eq!(
            db.query_row("SELECT count(*) FROM endpoints", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            25
        );
        assert!(
            seed(
                &dir.path().join("missing"),
                &dir.path().join("missing/edms.db")
            )
            .is_err()
        );
    }
}
