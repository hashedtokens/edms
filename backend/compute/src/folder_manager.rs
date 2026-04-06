use std::fs;
use std::path::{Path, PathBuf};
use serde::Serialize;
use tracing::{info, warn};


#[derive(Debug)]
pub enum FolderStatus {
    Ok,
    Missing(Vec<String>),
    Corrupted,
}

pub struct FolderLayout {
    pub root: PathBuf,
}

impl FolderLayout {
    pub fn new<P: AsRef<Path>>(root: P) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn init_edmsfolders(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.root.exists() {
            self.create_edmsfolders()?;
        }
        Ok(())
    }

    pub fn verify_edmsfolders(&self) -> FolderStatus {
        let required = vec![
            "repo",
            "session-backup",
            "active",
            "exports",
            "temp",
            "docs",
        ];

        let mut missing = Vec::new();

        for folder in required {
            let path = self.root.join(folder);
            if !path.exists() {
                missing.push(folder.to_string());
            }
        }

        if missing.is_empty() {
            FolderStatus::Ok
        } else {
            FolderStatus::Missing(missing)
        }
    }

    pub fn create_edmsfolders(&self) -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(self.root.join("repo"))?;
        fs::create_dir_all(self.root.join("session-backup"))?;
        fs::create_dir_all(self.root.join("active"))?;
        fs::create_dir_all(self.root.join("exports"))?;
        fs::create_dir_all(self.root.join("temp"))?;
        fs::create_dir_all(self.root.join("docs"))?;
        Ok(())
    }

    pub fn reset_structure(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.root.exists() {
            fs::remove_dir_all(&self.root)?;
        }
        self.create_edmsfolders()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_folder_creation_and_verification() {
        let dir = tempdir().unwrap();
        let layout = FolderLayout::new(dir.path());

        layout.create_edmsfolders().unwrap();

        let status = layout.verify_edmsfolders();
        match status {
            FolderStatus::Ok => {}
            _ => panic!("Folder structure not valid"),
        }
    }
}


#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InitAction {
    /// Everything was already in order — nothing changed
    AlreadyHealthy,
    /// Some folders were missing — they were created
    FoldersCreated,
    /// Structure was corrupted — it was fully reset
    StructureReset,
}

// ── Report returned to the caller (serialised directly into JSON response) ────

#[derive(Debug, Serialize)]
pub struct SystemInitReport {
    pub success:         bool,
    pub action:          InitAction,
    pub missing_folders: Vec<String>,   // empty unless action == FoldersCreated
    pub root_path:       String,
    pub message:         String,
}

// ── Core logic — called by the handler, also callable from system_runner ──────

/// Verify the edms folder structure under `root_path`.
/// If anything is wrong, fix it automatically and report what was done.
///
/// This never prompts for input — it is safe to call in Docker, CI, or tests.
pub fn verify_and_init(root_path: &Path) -> Result<SystemInitReport, Box<dyn std::error::Error + Send + Sync>> {

    let layout = FolderLayout::new(root_path);

    // Always run init first (idempotent — safe to call repeatedly)
    let _ = layout.init_edmsfolders();

    let report = match layout.verify_edmsfolders() {

        FolderStatus::Ok => {
            info!(root = %root_path.display(), "Folder structure healthy — no action needed");

            SystemInitReport {
                success:         true,
                action:          InitAction::AlreadyHealthy,
                missing_folders: vec![],
                root_path:       root_path.display().to_string(),
                message:         "Folder structure is healthy. Server is ready.".into(),
            }
        }

        FolderStatus::Missing(missing) => {
            warn!(
                root    = %root_path.display(),
                missing = ?missing,
                "Missing folders detected — creating automatically"
            );

            let _ = layout.create_edmsfolders();

            SystemInitReport {
                success:         true,
                action:          InitAction::FoldersCreated,
                missing_folders: missing.clone(),
                root_path:       root_path.display().to_string(),
                message:         format!(
                    "{} missing folder(s) were created. Server is ready.",
                    missing.len()
                ),
            }
        }

        FolderStatus::Corrupted => {
            warn!(root = %root_path.display(), "Folder structure corrupted — resetting");

            let _ = layout.reset_structure();

            SystemInitReport {
                success:         true,
                action:          InitAction::StructureReset,
                missing_folders: vec![],
                root_path:       root_path.display().to_string(),
                message:         "Folder structure was corrupted and has been fully reset. Server is ready.".into(),
            }
        }
    };

    info!(
        action  = ?report.action,
        root    = %root_path.display(),
        message = %report.message,
        "System init complete"
    );

    Ok(report)
}

pub fn default_root_path() -> PathBuf {
    // This finds the "edms_sys" parent directory regardless of which crate calls it
    let mut path = std::env::current_dir().unwrap();
    while !path.join("compute").exists() && path.parent().is_some() {
        path = path.parent().unwrap().to_path_buf();
    }
    path.join("edms_root")
}