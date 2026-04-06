// zipops.rs

#![allow(dead_code)]
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::copy;
use std::path::{Path, PathBuf};
use chrono::Utc;
use rusqlite::Connection;
use uuid::Uuid;

use tracing::warn;
use walkdir::WalkDir;
use zip::{ZipArchive, ZipWriter};
use zip::write::SimpleFileOptions;

// Use a consistent error type throughout
type DynResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Validate that a table name contains only safe identifier characters.
fn validate_identifier(name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        Ok(())
    } else {
        Err(format!("Unsafe SQL identifier: {:?}", name).into())
    }
}

/// Confirm that `path` is strictly inside `root` after canonicalisation.
///
/// Both paths must already exist (or have their parent created) before calling
/// this function so that `canonicalize` does not fail.
fn path_is_inside(root: &Path, path: &Path) -> bool {
    // Create the parent directory so canonicalize succeeds for not-yet-created files.
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match (root.canonicalize(), path.canonicalize()
        .or_else(|_| path.parent().map(|p| p.canonicalize()).unwrap_or(Err(
            std::io::Error::new(std::io::ErrorKind::NotFound, "no parent")
        ))))
    {
        (Ok(canonical_root), Ok(canonical_path)) => {
            canonical_path.starts_with(&canonical_root)
        }
        _ => false,
    }
}

//
// ─────────────────────────────────────────────────────────────────────────────
// 1️⃣  EXPORT BOOKMARK ZIP
// ─────────────────────────────────────────────────────────────────────────────
//

pub fn create_zip_from_bookmarks(
    repo_path: &Path,
    selected_eids: &[String],
    output_zip: &Path,
) -> DynResult<()> {
    let file = File::create(output_zip)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    for entry in WalkDir::new(repo_path) {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let filename = match path.file_name() {
            Some(f) => f.to_string_lossy(),
            None => continue,
        };

        for eid in selected_eids {
            if filename.starts_with(eid.as_str()) {
                let relative = path.strip_prefix(repo_path)?;
                zip.start_file(relative.to_string_lossy(), options)?;

                let mut f = File::open(path)?;
                copy(&mut f, &mut zip)?;
                break;
            }
        }
    }

    zip.finish()?;
    Ok(())
}

//
// ─────────────────────────────────────────────────────────────────────────────
// 2️⃣  IMPORT ZIP 
// ─────────────────────────────────────────────────────────────────────────────
//

pub fn import_zip_impl(
    zip_file: &Path,
    destination_folder: &Path,
) -> DynResult<()> {
    fs::create_dir_all(destination_folder)?;
    // Canonicalise the destination *after* it has been created so the call
    // cannot fail on a non-existent path.
    let canonical_dest = destination_folder.canonicalize()?;

    let file    = File::open(zip_file)?;
    let mut archive = ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;

        // strip any leading `/` or `..` components before joining,
        // then re-verify the fully-resolved output path is inside the
        // destination.  We no longer rely on the parent's canonicalisation
        // alone, which could silently allow traversal when dirs don't exist yet.
        let raw_name = entry.name().to_string();
        let safe_relative: PathBuf = raw_name
            .split('/')
            .filter(|c| !c.is_empty() && *c != "..")
            .collect();

        let outpath = canonical_dest.join(&safe_relative);

        // Double-check after resolution.
        if !outpath.starts_with(&canonical_dest) {
            // FIX #11: use tracing instead of println!
            warn!("Skipping unsafe path: {}", raw_name);
            continue;
        }

        if entry.is_dir() {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = File::create(&outpath)?;
            copy(&mut entry, &mut outfile)?;
        }
    }

    Ok(())
}

//
// ─────────────────────────────────────────────────────────────────────────────
// 3️⃣  MARK ACTIVE FOLDER
// ─────────────────────────────────────────────────────────────────────────────
//

pub fn mark_active_folder(
    session_backup: &Path,
    active_folder: &Path,
    folder_name: &str,
    yaml_config_path: &Path,
) -> DynResult<()> {
    let source      = session_backup.join(folder_name);
    let destination = active_folder.join(folder_name);

    // 1. Ensure source exists so the move doesn't fail
    if !source.exists() {
        tracing::info!("Creating missing source directory: {:?}", source);
        fs::create_dir_all(&source)?;
    }

    // 2. Clear out the destination if it already exists
    if destination.exists() {
        fs::remove_dir_all(&destination)?;
    }

    // 3. Attempt an atomic rename (fast)
    if let Err(e) = fs::rename(&source, &destination) {
        tracing::warn!("Rename failed ({}), falling back to manual copy/delete", e);
        
        // Fallback: Copy using your WalkDir method
        copy_dir_all(&source, &destination)?;
        
        // Clean up the source after successful copy
        fs::remove_dir_all(&source)?;
    }

    // 4. Ensure the config directory exists before writing
    if let Some(parent) = yaml_config_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // 5. Update the YAML configuration
    let yaml_content = format!("active_folder: {}\n", folder_name);
    fs::write(yaml_config_path, yaml_content)?;

    Ok(())
}

//
// ─────────────────────────────────────────────────────────────────────────────
// 4️⃣  EXPORT COLLECTION 
// ─────────────────────────────────────────────────────────────────────────────
//

/// Sync, path-based entry point for internal callers (e.g. system_runner).
/// The async `export_collection` handler is for Axum routes only.
pub fn zip_collection(source: &Path, output: &Path) -> DynResult<()> {
    zip_folder(source, output)
}

//
// ─────────────────────────────────────────────────────────────────────────────
// 5️⃣  EXPORT MERGE
// ─────────────────────────────────────────────────────────────────────────────
//

pub fn export_merge(
    input_zips: &[&Path],
    folder_sources: &[&Path],
    output_zip: &Path,
) -> DynResult<()> {
    let file = File::create(output_zip)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    let mut added_files = HashSet::new();

    for zip_path in input_zips {
        let file = File::open(zip_path)?;
        let mut archive = ZipArchive::new(file)?;

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let name = entry.name().to_string();

            if added_files.contains(&name) {
                continue;
            }

            zip.start_file(&name, options)?;
            copy(&mut entry, &mut zip)?;
            added_files.insert(name);
        }
    }

    for folder in folder_sources {
        for entry in WalkDir::new(folder) {
            let entry = entry?;
            let path  = entry.path();

            if !path.is_file() {
                continue;
            }

            let relative = path.strip_prefix(folder)?.to_string_lossy().to_string();

            if added_files.contains(&relative) {
                continue;
            }

            zip.start_file(&relative, options)?;
            let mut f = File::open(path)?;
            copy(&mut f, &mut zip)?;
            added_files.insert(relative);
        }
    }

    zip.finish()?;
    Ok(())
}

//
// ─────────────────────────────────────────────────────────────────────────────
// 6️⃣  EXPORT STATIC WEBSITE
// ─────────────────────────────────────────────────────────────────────────────
//

pub fn export_static_website(
    static_folder: &Path,
    output_zip: &Path,
) -> DynResult<()> {
    zip_folder(static_folder, output_zip)
}

//
// ─────────────────────────────────────────────────────────────────────────────
// 7️⃣  CREATE STATIC WEBSITE
// ─────────────────────────────────────────────────────────────────────────────
//

pub fn create_static_website(
    bookmark_collection_path: &Path,
    output_folder: &Path,
) -> DynResult<()> {
    if output_folder.exists() {
        fs::remove_dir_all(output_folder)?;
    }
    fs::create_dir_all(output_folder)?;

    for entry in WalkDir::new(bookmark_collection_path) {
        let entry = entry?;
        let path  = entry.path();

        if !path.is_file() {
            continue;
        }

        let relative = path.strip_prefix(bookmark_collection_path)?;
        let new_path = output_folder.join(relative);

        if let Some(parent) = new_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::copy(path, &new_path)?;
    }

    Ok(())
}

//
// ─────────────────────────────────────────────────────────────────────────────
// MERGE ZIP FILES
// ─────────────────────────────────────────────────────────────────────────────
//

pub fn merge_zipfiles(
    filename: &str,
    input_list: &[PathBuf],
    destination_root: &Path,
) -> DynResult<PathBuf> {
    // append both a timestamp AND a UUID to guarantee uniqueness even
    // when the function is called multiple times within the same second.
    let ts   = Utc::now().timestamp_millis();
    let uid  = Uuid::new_v4().to_string();
    let output_name = format!("{}-{}-{}.zip", filename, ts, uid);
    let output_zip  = destination_root.join(&output_name);

    // Temporary workspace.
    let temp_dir = destination_root.join("merge_workspace");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    // extract zips into the workspace FIRST so that merge_sqlitefiles
    // can find the SQLite databases that live inside them.
    for input in input_list {
        if input.is_file() {
            extract_zip(input, &temp_dir)?;
        } else if input.is_dir() {
            copy_dir_all(input, &temp_dir)?;
        }
    }

    // Now merge all SQLite databases found in the workspace.
    let merged_sqlite = merge_sqlitefiles(&temp_dir, &[temp_dir.clone()])?;

    // Place the merged database at a known location inside the workspace.
    let dest_db = temp_dir.join("merged.db");
    if merged_sqlite != dest_db {
        fs::copy(&merged_sqlite, &dest_db)?;
    }

    // Create the final zip from the workspace.
    create_zip_from_dir(&temp_dir, &output_zip)?;

    // Cleanup.
    fs::remove_dir_all(&temp_dir)?;

    Ok(output_zip)
}

//
// ─────────────────────────────────────────────────────────────────────────────
// MERGE SQLITE FILES
// ─────────────────────────────────────────────────────────────────────────────
//

pub fn merge_sqlitefiles(
    workspace: &Path,
    input_list: &[PathBuf],
) -> DynResult<PathBuf> {
    let merged_path = workspace.join("merged.sqlite");
    let merge_conn  = Connection::open(&merged_path)?;

    for input in input_list {
        let db_path = find_sqlite(input)?;

        if let Some(db) = db_path {
            // Skip the output file itself to avoid self-merging.
            if db.canonicalize().ok() == merged_path.canonicalize().ok() {
                continue;
            }

            let src_conn = Connection::open(&db)?;

            let mut stmt = src_conn.prepare(
                "SELECT name FROM sqlite_master WHERE type='table';"
            )?;

            let table_names: Vec<String> = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect();

            //ATTACH once per database, outside the per-table loop.
            merge_conn.execute(
                &format!("ATTACH DATABASE '{}' AS src", db.display()),
                [],
            )?;

            for table in &table_names {
                //validate identifier to prevent SQL injection.
                validate_identifier(table)
                    .map_err(|_e| rusqlite::Error::InvalidQuery)?;

                //    create the table in the merged DB if it does not
                // exist yet, copying the schema from the source.
                merge_conn.execute(
                    &format!(
                        "CREATE TABLE IF NOT EXISTS \"{table}\" AS \
                         SELECT * FROM src.\"{table}\" WHERE 0",
                        table = table
                    ),
                    [],
                )?;

                // Safe insert — identifiers are quoted and validated.
                merge_conn.execute(
                    &format!(
                        "INSERT INTO \"{table}\" SELECT * FROM src.\"{table}\"",
                        table = table
                    ),
                    [],
                )?;
            }

            merge_conn.execute("DETACH DATABASE src", [])?;
        }
    }

    Ok(merged_path)
}

//
// ─────────────────────────────────────────────────────────────────────────────
// HELPERS
// ─────────────────────────────────────────────────────────────────────────────
//

pub fn zip_folder(source_dir: &Path, output_file: &Path) -> DynResult<()> {
    let file = File::create(output_file)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    for entry in WalkDir::new(source_dir) {
        let entry = entry?;
        let path  = entry.path();

        if !path.is_file() {
            continue;
        }

        let name = path.strip_prefix(source_dir)?;
        zip.start_file(name.to_string_lossy(), options)?;
        let mut f = File::open(path)?;
        copy(&mut f, &mut zip)?;
    }

    zip.finish()?;
    Ok(())
}

fn find_sqlite(input: &Path) -> DynResult<Option<PathBuf>> {
    // Bare zip files are not walked here — they should be extracted first.
    if input.is_file() {
        return Ok(
            if input.extension().unwrap_or_default() == "sqlite" {
                Some(input.to_path_buf())
            } else {
                None
            }
        );
    }

    for entry in WalkDir::new(input) {
        let entry = entry?;
        let path  = entry.path();

        if path.extension().unwrap_or_default() == "sqlite" {
            return Ok(Some(path.to_path_buf()));
        }
    }

    Ok(None)
}

fn copy_dir_all(src: &Path, dst: &Path) -> DynResult<()> {
    let mut seen: HashSet<PathBuf> = HashSet::new();

    for entry in WalkDir::new(src) {
        // Explicitly map the error to our DynResult type
        let entry = entry.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        let rel = entry.path().strip_prefix(src)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        
        let dest_path = dst.join(rel);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&dest_path).map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        } else {
            if seen.contains(rel) {
                warn!("Skipping duplicate file during copy: {}", rel.display());
                continue;
            }
            fs::copy(entry.path(), &dest_path).map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            seen.insert(rel.to_path_buf());
        }
    }
    Ok(())
}

fn extract_zip(zip_path: &Path, dest: &Path) -> DynResult<()> {
    fs::create_dir_all(dest)?;
    let canonical_dest = dest.canonicalize()?;

    let file = File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let raw_name = file.name().to_string();

        // Strip dangerous path components before joining.
        let safe_relative: PathBuf = raw_name
            .split('/')
            .filter(|c| !c.is_empty() && *c != "..")
            .collect();

        let outpath = canonical_dest.join(&safe_relative);

        // Final safety check after resolution.
        if !outpath.starts_with(&canonical_dest) {
            warn!("Skipping unsafe path in zip: {}", raw_name);
            continue;
        }

        if raw_name.ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                fs::create_dir_all(p)?;
            }
            let mut outfile = File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }

    Ok(())
}

fn create_zip_from_dir(
    src_dir: &Path,
    zip_path: &Path,
) -> DynResult<()> {
    let file = File::create(zip_path)?;
    let mut zip = ZipWriter::new(file);

    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for entry in WalkDir::new(src_dir) {
        let entry = entry?;
        let path  = entry.path();
        let name  = path.strip_prefix(src_dir)?;

        if path.is_file() {
            zip.start_file(name.to_string_lossy(), options)?;
            let mut f = File::open(path)?;
            std::io::copy(&mut f, &mut zip)?;
        }
    }

    zip.finish()?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Input Validation 
// ─────────────────────────────────────────────────────────────────────────────

fn validate_abs_path(p: &str, field: &str) -> Result<PathBuf, String> {
    if p.is_empty() {
        return Err(format!("{} must not be empty", field));
    }
    let path = PathBuf::from(p);
    if !path.is_absolute() {
        return Err(format!("{} must be an absolute path", field));
    }
    Ok(path)
}