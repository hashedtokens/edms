use crate::config::AppConfig;
use crate::events::ServerEvent;
use crate::timer::TimerHandle;
use edms::core::EdmsCore;
use edms::query_loader::QueryMap;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, RwLock};

#[derive(Clone)]
pub struct AppState {
    pub core: Arc<EdmsCore>,
    pub queries: Arc<QueryMap>,
    pub events_tx: broadcast::Sender<ServerEvent>,
    pub active_folder: Arc<RwLock<Option<String>>>,
    /// Which real Collection (System 2, `storage/collections/{name}.sqlite`)
    /// is currently loaded into the active bookmark workspace, if any.
    /// Per Mathew (2026-09-08): bookmarking requires a collection to be
    /// loaded first — this is what that check reads, and what "save this
    /// bookmarked endpoint into the collection" writes into.
    pub active_collection: Arc<RwLock<Option<String>>>,
    pub dashboard_conn: Arc<Mutex<Connection>>,
    pub db_path: PathBuf,
    pub storage_root: PathBuf,
    /// Whether `storage_root` came from an explicit, validated source
    /// (EDMS_ROOT env, or config.yaml's `storage.root` pointing at a
    /// directory that actually exists and isn't inside the repo/init/) —
    /// as opposed to the unconfigured fallback. Surfaced via
    /// `/dashboard/static` so the frontend can show the "parent directory
    /// path not enabled" banner (Ravi, 2026-09-14).
    pub storage_configured: bool,
    pub started_at: String,
    pub config: Arc<AppConfig>,
    /// Timer handles for in-flight test runs, keyed by (endpoint_id,
    /// request_number) — so whichever code path learns the test's real
    /// outcome (success or timeout, in callback.rs) can cancel the app's
    /// own independent countdown instead of letting it keep ticking or
    /// fire a second, uncoordinated TestTimeout on its own schedule.
    pub active_timers: Arc<Mutex<HashMap<(String, i32), TimerHandle>>>,
    pub eid_allocator: Arc<compute::eid::allocator::EidAllocator>,
}

impl AppState {
    pub fn new(
        core: Arc<EdmsCore>,
        queries: Arc<QueryMap>,
        dashboard_conn: Connection,
        db_path: PathBuf,
        storage_root: PathBuf,
        storage_configured: bool,
        config: Arc<AppConfig>,
    ) -> Self {
        let (events_tx, _) = broadcast::channel(256);
        let eid_allocator = Arc::new(compute::eid::allocator::EidAllocator::new(
            &db_path.to_string_lossy(),
        ));
        let _ = eid_allocator.initialize();
        Self {
            core,
            queries,
            events_tx,
            active_folder: Arc::new(RwLock::new(None)),
            active_collection: Arc::new(RwLock::new(None)),
            dashboard_conn: Arc::new(Mutex::new(dashboard_conn)),
            db_path,
            storage_root,
            storage_configured,
            started_at: chrono::Utc::now().to_rfc3339(),
            config,
            active_timers: Arc::new(Mutex::new(HashMap::new())),
            eid_allocator,
        }
    }

    /// Removes and cancels the timer for (endpoint_id, request_number), if
    /// one is still running. Call this once the test's real outcome is
    /// known — a no-op if the timer already fired and removed itself.
    /// Returns whether a running timer was actually found and cancelled.
    pub fn cancel_timer(&self, endpoint_id: &str, request_number: i32) -> bool {
        let handle = self
            .active_timers
            .lock()
            .unwrap()
            .remove(&(endpoint_id.to_string(), request_number));
        match handle {
            Some(handle) => {
                handle.cancel();
                true
            }
            None => false,
        }
    }

    pub async fn emit(&self, evt: ServerEvent) {
        let _ = self.events_tx.send(evt);
    }

    /// Where one endpoint's request/response files live, per the Sept-1
    /// storage schema: storage/globalEQPData/{endpoint_id}/ under the
    /// storage root — not the old edms_data/{endpoint_id}/ path some code
    /// still used. Absolute (storage_root already is), so it's unambiguous
    /// regardless of which process's CWD reads/writes it (webserver vs.
    /// the spawned edms-child).
    pub fn endpoint_storage_dir(&self, endpoint_id: &str) -> PathBuf {
        self.storage_root.join("storage").join("globalEQPData").join(endpoint_id)
    }

    /// Takes a new dashboard snapshot using the current state's paths.
    /// Call this after any change that should be reflected immediately
    /// (e.g. a new endpoint being created).
   pub fn refresh_dashboard_snapshot(&self) {
        let conn = self.dashboard_conn.lock().unwrap();
        match crate::dashboard_db::take_snapshot(&conn, &self.db_path, &self.storage_root) {
            Ok(snapshot) => {
                tracing::info!("Dashboard snapshot refreshed: {:?}", snapshot);
            }
            Err(e) => {
                tracing::warn!("Failed to refresh dashboard snapshot: {e}");
            }
        }

        match crate::dashboard_db::rotate_old_snapshots(&conn) {
            Ok(deleted) if deleted > 0 => {
                tracing::info!("Rotated {deleted} snapshot(s) older than 30 days");
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!("Failed to rotate old snapshots: {e}");
            }
        }
    }
}