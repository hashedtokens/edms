// compute/tests/zipops_test.rs

use compute::zipops::*;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

// Use the same DynResult type as your zipops functions
type DynResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create test files
    fn create_test_file(dir: &PathBuf, name: &str, content: &str) -> PathBuf {
        let file_path = dir.join(name);
        fs::write(&file_path, content).unwrap();
        file_path
    }

    #[test]
    fn test_zip_folder() -> DynResult<()> {
        let temp_dir = tempdir()?;
        let source = temp_dir.path().join("source");
        let output = temp_dir.path().join("output.zip");

        fs::create_dir_all(&source)?;
        create_test_file(&source, "file1.txt", "content1");
        create_test_file(&source, "file2.txt", "content2");

        zip_folder(&source, &output)?;

        assert!(output.exists());
        assert!(output.metadata()?.len() > 0);

        Ok(())
    }

    #[test]
    fn test_create_zip_from_bookmarks() -> DynResult<()> {
        let temp_dir = tempdir()?;
        let repo = temp_dir.path().join("repo");
        let output = temp_dir.path().join("bookmarks.zip");

        fs::create_dir_all(&repo)?;
        create_test_file(&repo, "eid123_file.txt", "content");
        create_test_file(&repo, "eid456_file.txt", "content");
        create_test_file(&repo, "other.txt", "content");

        let selected_eids = vec!["eid123".to_string(), "eid456".to_string()];

        create_zip_from_bookmarks(&repo, &selected_eids, &output)?;

        assert!(output.exists());
        Ok(())
    }

    #[test]
    fn test_import_zip_impl() -> DynResult<()> {
        let temp_dir = tempdir()?;
        let source_dir = temp_dir.path().join("source");
        let output_dir = temp_dir.path().join("output");
        let zip_path = temp_dir.path().join("test.zip");

        // Create a test zip
        fs::create_dir_all(&source_dir)?;
        create_test_file(&source_dir, "test.txt", "hello");
        zip_folder(&source_dir, &zip_path)?;

        // Import it
        import_zip_impl(&zip_path, &output_dir)?;

        assert!(output_dir.join("test.txt").exists());
        Ok(())
    }

    #[test]
    fn test_export_merge() -> DynResult<()> {
        let temp_dir = tempdir()?;
        let zip1_dir = temp_dir.path().join("zip1");
        let zip2_dir = temp_dir.path().join("zip2");
        let folder = temp_dir.path().join("folder");
        let output = temp_dir.path().join("merged.zip");

        // Create test zips
        fs::create_dir_all(&zip1_dir)?;
        fs::create_dir_all(&zip2_dir)?;
        fs::create_dir_all(&folder)?;

        create_test_file(&zip1_dir, "file1.txt", "from zip1");
        create_test_file(&zip2_dir, "file2.txt", "from zip2");
        create_test_file(&folder, "file3.txt", "from folder");

        let zip1 = temp_dir.path().join("zip1.zip");
        let zip2 = temp_dir.path().join("zip2.zip");
        zip_folder(&zip1_dir, &zip1)?;
        zip_folder(&zip2_dir, &zip2)?;

        let input_zips = vec![zip1.as_path(), zip2.as_path()];
        let folders = vec![folder.as_path()];

        export_merge(&input_zips, &folders, &output)?;

        assert!(output.exists());
        Ok(())
    }

    #[test]
    fn test_mark_active_folder() -> DynResult<()> {
        let temp_dir = tempdir()?;
        let session_backup = temp_dir.path().join("session-backup");
        let active_folder = temp_dir.path().join("active");
        let config_path = temp_dir.path().join("config.yaml");

        let folder_name = "test_folder";

        // Create source folder
        let source = session_backup.join(folder_name);
        fs::create_dir_all(&source)?;
        create_test_file(&source, "test.txt", "content");

        mark_active_folder(&session_backup, &active_folder, folder_name, &config_path)?;

        // Verify destination exists
        assert!(active_folder.join(folder_name).exists());
        assert!(active_folder.join(folder_name).join("test.txt").exists());

        // Verify config was created
        assert!(config_path.exists());
        let config_content = fs::read_to_string(config_path)?;
        assert!(config_content.contains(folder_name));

        Ok(())
    }

    #[test]
    fn test_merge_zipfiles() -> DynResult<()> {
        let temp_dir = tempdir()?;
        let zip1_dir = temp_dir.path().join("zip1");
        let zip2_dir = temp_dir.path().join("zip2");

        fs::create_dir_all(&zip1_dir)?;
        fs::create_dir_all(&zip2_dir)?;

        create_test_file(&zip1_dir, "test1.txt", "content1");
        create_test_file(&zip2_dir, "test2.txt", "content2");

        let zip1 = temp_dir.path().join("zip1.zip");
        let zip2 = temp_dir.path().join("zip2.zip");
        zip_folder(&zip1_dir, &zip1)?;
        zip_folder(&zip2_dir, &zip2)?;

        let inputs = vec![zip1, zip2];
        let result = merge_zipfiles("merged", &inputs, temp_dir.path())?;

        assert!(result.exists());
        Ok(())
    }

    #[test]
    fn test_merge_sqlitefiles() -> DynResult<()> {
        let temp_dir = tempdir()?;
        let workspace = temp_dir.path();

        // Create test SQLite databases
        let db1 = workspace.join("test1.sqlite");
        let db2 = workspace.join("test2.sqlite");

        let conn1 = rusqlite::Connection::open(&db1)?;
        let conn2 = rusqlite::Connection::open(&db2)?;

        conn1.execute("CREATE TABLE users (id INTEGER, name TEXT)", [])?;
        conn1.execute("INSERT INTO users VALUES (1, 'Alice')", [])?;

        conn2.execute("CREATE TABLE users (id INTEGER, name TEXT)", [])?;
        conn2.execute("INSERT INTO users VALUES (2, 'Bob')", [])?;

        let inputs = vec![workspace.to_path_buf()];
        let merged = merge_sqlitefiles(workspace, &inputs)?;

        assert!(merged.exists());

        // Verify merged data
        let merged_conn = rusqlite::Connection::open(&merged)?;
        let count: i64 =
            merged_conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;
        assert_eq!(count, 2);

        Ok(())
    }

    #[test]
    fn test_create_static_website() -> DynResult<()> {
        let temp_dir = tempdir()?;
        let source = temp_dir.path().join("source");
        let output = temp_dir.path().join("output");

        fs::create_dir_all(&source)?;
        create_test_file(&source, "index.html", "<html>test</html>");
        create_test_file(&source, "style.css", "body {}");

        create_static_website(&source, &output)?;

        assert!(output.join("index.html").exists());
        assert!(output.join("style.css").exists());

        Ok(())
    }

    #[test]
    fn test_export_static_website() -> DynResult<()> {
        let temp_dir = tempdir()?;
        let source = temp_dir.path().join("static");
        let output = temp_dir.path().join("static.zip");

        fs::create_dir_all(&source)?;
        create_test_file(&source, "index.html", "<html>test</html>");

        export_static_website(&source, &output)?;

        assert!(output.exists());
        Ok(())
    }

    #[test]
    fn test_path_security() {
        let temp_dir = tempdir().unwrap();
        let root = temp_dir.path();

        let inside = root.join("test.txt");
        assert!(inside.starts_with(root));

        let outside = PathBuf::from("/etc/passwd");
        assert!(!outside.starts_with(root));
    }

    #[test]
    fn test_zip_collection() -> DynResult<()> {
        let temp_dir = tempdir()?;
        let source = temp_dir.path().join("collection");
        let output = temp_dir.path().join("collection.zip");

        fs::create_dir_all(&source)?;
        create_test_file(&source, "doc1.txt", "doc1 content");
        create_test_file(&source, "doc2.txt", "doc2 content");

        zip_collection(&source, &output)?;

        assert!(output.exists());
        assert!(output.metadata()?.len() > 0);

        Ok(())
    }
}
