mod config;
mod dashboard_db;
mod date_watcher;
mod db;
mod events;
mod handlers;
mod ipc;
mod logging;
mod state;
mod timer;

use axum::{
    routing::{get, post},
    Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;

use edms::core::EdmsCore;
use edms::query_loader::QueryMap;
use edms::schema::initialize_schema_from_core;
use std::{net::SocketAddr, sync::Arc};

use handlers::{
    bookmarks::{remove_from_collection, save_to_collection, ws_load_collection},
    callback::ipc_callback,
    dashboard::{
        compare_daily_snapshots, execute_purge_orphaned, get_crud_operations, get_dashboard_snapshot,
        get_dashboard_snapshot_history, get_purge_audit_report, get_static_data,
        refresh_crud_operations,
    },
    dataview::{dashboard, delete_folder, merge_folder, ws_make_folder_active},
    endpoints::{create_endpoint, delete_endpoint, lookup_endpoint, update_endpoint_annotation},
    logs::get_logs,
    repo::{export_collection, import_collection},
    tags::{add_tag, list_tags_for_endpoint, popular_tags, remove_tag},
    test_view::{
        clear_bookmarks, clear_history, delete_qp, get_saved_headers, get_saved_request,
        get_saved_response, list_qps, save_bookmark, save_history, stop,
        ws_add_from_history_to_bookmark, ws_delete_from_bookmark, ws_load_bookmarks,
        ws_load_endpoints, ws_load_history, ws_run,
    },
    view::{home, list_view, test_view, trigger_view_refresh},
    view_catalog::{
        annotate_collection_entry, create_collection_entry, create_repoview_entry,
        create_webview_entry, delete_collection_entry, get_collection_entry,
        import_tags_into_collection, list_collection_endpoint_tags, list_collection_endpoints,
        list_collections, list_repoviews, list_webviews, remove_endpoint_from_collection,
        rename_collection_entry,
    },
    view_tags::{
        create_collections_tag, create_repoview_tag, create_webview_tag, delete_collections_tags,
        delete_repoview_tags, delete_webview_tags, list_collections_tags, list_repoview_tags,
        list_webview_tags, rename_collections_tag, rename_repoview_tag, rename_webview_tag,
    },
    collection_tag_memberships::{
        add_collection_tag, collections_by_tag, list_collection_tags, remove_collection_tag,
    },
};

/// Resolves the EDMS storage root, per Ravi (2026-09-14): one single point
/// of configuration, a relative path, and the app never creates the
/// top-level directory itself — the user has to. Priority:
///
/// 1. `EDMS_ROOT` env var — an explicit override (this is what
///    docker-compose sets); always trusted as-is, no existence/location
///    checks, since it's already an explicit deployment decision.
/// 2. `config.yaml`'s `storage.root` — a relative path, resolved against
///    the process's working directory. Rejected (falls through to #3) if
///    it resolves inside this repo or the `init/` folder, or if the
///    directory doesn't exist yet.
/// 3. Neither — the old walk-up default, flagged unconfigured.
///
/// Returns `(root, configured)`. `configured = false` means: still
/// running (never hard-blocks), but the frontend should show a warning —
/// see `storage_configured` on `AppState`, surfaced via `/dashboard/static`.
fn resolve_storage_root(config: &config::AppConfig) -> (std::path::PathBuf, bool) {
    if let Ok(env_root) = std::env::var("EDMS_ROOT") {
        return (std::path::PathBuf::from(env_root), true);
    }

    if let Some(rel) = config.storage.root.as_deref().filter(|s| !s.trim().is_empty()) {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        // `Path::join` is purely lexical — it does NOT resolve `..`
        // components, so ".../webserver/../../../foo" would otherwise
        // still lexically "start with" the repo root even though it
        // points outside it. Normalize before any containment check.
        // Can't use `canonicalize()` here — the directory usually doesn't
        // exist yet (that's the whole point of this check).
        let resolved = normalize_lexically(&cwd.join(rel));

        let inside_init = resolved
            .components()
            .any(|c| c.as_os_str() == std::ffi::OsStr::new("init"));
        let inside_repo = find_repo_root(&cwd)
            .map(|repo_root| resolved.starts_with(&normalize_lexically(&repo_root)))
            .unwrap_or(false);

        if inside_init || inside_repo {
            eprintln!(
                "[storage] configured storage.root '{}' resolves inside the \
                 repo/init directory — refusing to use it. Point it \
                 somewhere outside the repo instead.",
                resolved.display()
            );
        } else if !resolved.exists() {
            eprintln!(
                "[storage] configured storage.root '{}' does not exist yet — \
                 create it, then restart. Running against a fallback \
                 location until then.",
                resolved.display()
            );
        } else {
            return (resolved, true);
        }
    }

    (compute::folder_manager::default_root_path(), false)
}

/// Lexically resolves `.`/`..` components without touching the
/// filesystem (unlike `canonicalize()`, works on paths that don't exist
/// yet). `..` past the root is just dropped, same as most shells.
fn normalize_lexically(path: &std::path::Path) -> std::path::PathBuf {
    let mut out = std::path::PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Walks up from `start` looking for a `.git` directory, to detect when a
/// configured storage path resolves inside this checkout.
fn find_repo_root(start: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        if d.join(".git").exists() {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Config loads first — storage-root resolution below depends on it.
    let app_config = std::env::var("EDMS_CONFIG_PATH")
        .unwrap_or_else(|_| "config.yaml".to_string());
    let config = Arc::new(config::AppConfig::from_file_or_default(&app_config));

    //Folder structure
    let (root, storage_configured) = resolve_storage_root(&config);
    println!(
        "Initializing EDMS root at: {:?} (configured: {})",
        root, storage_configured
    );
    if !storage_configured {
        eprintln!(
            "[storage] WARNING: no valid storage.root configured — running \
             against a fallback location. Set `storage.root` in {app_config} \
             to a relative path outside this repo, create that directory \
             yourself, then restart."
        );
    }

    compute::folder_manager::verify_and_init(&root)
        .expect("Failed to initialize system folders");

    let log_writer = logging::AppLogWriter::new(&root).expect("Failed to open log file");
    tracing_subscriber::fmt()
        .with_target(false)
        .with_writer(move || log_writer.clone())
        .init();

    let db_path = std::env::var("EDMS_DB_PATH").unwrap_or_else(|_| "edms.db".to_string());


    let core = Arc::new(EdmsCore::new(&db_path));
    core.connect().map_err(|e| anyhow::anyhow!("{e:?}"))?;
    initialize_schema_from_core(&core).map_err(|e| anyhow::anyhow!("{e:?}"))?;

    let dashboard_conn = dashboard_db::init_dashboard_db(&root)
        .expect("Failed to initialize dashboard database");

    match dashboard_db::take_snapshot(&dashboard_conn, std::path::Path::new(&db_path), &root) {
        Ok(snapshot) => info!("Dashboard snapshot taken: {:?}", snapshot),
        Err(e) => tracing::warn!("Failed to take dashboard snapshot: {e}"),
    }

    match dashboard_db::rotate_old_snapshots(&dashboard_conn) {
        Ok(deleted) if deleted > 0 => info!("Rotated {deleted} old dashboard snapshot(s) on startup"),
        Ok(_) => {}
        Err(e) => tracing::warn!("Failed to rotate old snapshots on startup: {e}"),
    }

    let queries = Arc::new(QueryMap::load());
    let state = state::AppState::new(
        core,
        queries,
        dashboard_conn,
        std::path::PathBuf::from(&db_path),
        root.clone(),
        storage_configured,
        config,
    );

    // Date-change watcher — captures a daily snapshot for the day that just
    // ended whenever the calendar date actually rolls over. Polls rather
    // than a true OS push-notification (no portable one exists), but keyed
    // on comparing the actual date, not a 24h interval, so it can't drift
    // out of alignment with real calendar days.
    //
    // `last_date` starts from the DB's own memory (latest captured daily
    // snapshot), not blindly from today — so a restart during downtime that
    // spanned a rollover logs the gap instead of silently losing track of it.
    {
        let state = state.clone();
        tokio::spawn(async move {
            let mut last_date = {
                let conn = state.dashboard_conn.lock().unwrap();
                date_watcher::initial_last_date(&conn, chrono::Utc::now().date_naive())
            };
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                let current_date = chrono::Utc::now().date_naive();
                let conn = state.dashboard_conn.lock().unwrap();
                last_date = date_watcher::check_date_rollover(
                    &conn,
                    &state.db_path,
                    &state.storage_root,
                    last_date,
                    current_date,
                );
            }
        });
    }

    let app = Router::new()
        .route("/home", get(home))
        .route("/endpoints/create", post(create_endpoint))
        .route("/endpoints/lookup", get(lookup_endpoint))
        .route("/endpoints/:endpoint_id/delete", post(delete_endpoint))
        .route("/endpoints/:endpoint_id/annotation", post(update_endpoint_annotation))
        .route("/test-view", get(test_view))
        .route("/list-view", get(list_view))
        .route("/view/refresh", post(trigger_view_refresh))
        .route("/test-view/endpoints/load", get(ws_load_endpoints))
        .route("/test-view/bookmarks/load", get(ws_load_bookmarks))
        .route("/test-view/history/load", get(ws_load_history))
        .route("/test-view/run", get(ws_run))
        .route("/test-view/stop", post(stop))
        .route("/test-view/save/history", post(save_history))
        .route("/test-view/save/bookmark", post(save_bookmark))
        .route("/test-view/:bookmark/add", get(ws_add_from_history_to_bookmark))
        .route("/test-view/:bookmark/delete", get(ws_delete_from_bookmark))
        .route("/test-view/:endpoint_id/request/:request_number", get(get_saved_request))
        .route("/test-view/:endpoint_id/response/:request_number", get(get_saved_response))
        .route("/test-view/:endpoint_id/headers/:request_number", get(get_saved_headers))
        .route("/test-view/:endpoint_id/qps", get(list_qps))
        .route("/test-view/:endpoint_id/qps/:request_number/delete", post(delete_qp))
        .route("/test-view/history/clearall", post(clear_history))
        .route("/test-view/bookmark/clearall", post(clear_bookmarks))
        .route("/bookmarks/:collection/load", get(ws_load_collection))
        .route("/bookmarks/active/:endpoint_id/save", post(save_to_collection))
        .route("/bookmarks/active/:endpoint_id/unsave", post(remove_from_collection))
        .route("/dataview/:folder/delete", post(delete_folder))
        .route("/dataview/:folder/merge", post(merge_folder))
        .route("/dataview/:folder/active", get(ws_make_folder_active))
        .route("/dataview/dashboard", get(dashboard))
        .route("/dashboard/snapshot", get(get_dashboard_snapshot))
        .route("/dashboard/snapshot/history", get(get_dashboard_snapshot_history))
        .route("/dashboard/static", get(get_static_data))
        .route("/dashboard/crud-operations", get(get_crud_operations))
        .route("/dashboard/crud-operations/refresh", post(refresh_crud_operations))
        .route("/dashboard/compare", get(compare_daily_snapshots))
        .route("/purge_audit_report", get(get_purge_audit_report))
        .route("/purge_orphaned", post(execute_purge_orphaned))
        .route("/collections/create", post(create_collection_entry))
        .route("/collections/list", get(list_collections))
        .route("/collections/:name", get(get_collection_entry))
        .route("/collections/:name/rename", post(rename_collection_entry))
        .route("/collections/:name/annotation", post(annotate_collection_entry))
        .route("/collections/:name/delete", post(delete_collection_entry))
        .route("/collections/:name/endpoints/remove", post(remove_endpoint_from_collection))
        .route("/collections/:name/endpoints", get(list_collection_endpoints))
        .route("/collections/:name/tags/import", post(import_tags_into_collection))
        .route("/collections/:name/tags/endpoints", get(list_collection_endpoint_tags))
        .route("/collections/tags/create", post(create_collections_tag))
        .route("/collections/tags/delete", post(delete_collections_tags))
        .route("/collections/tags/rename", post(rename_collections_tag))
        .route("/collections/tags/list", get(list_collections_tags))
        .route("/webview/create", post(create_webview_entry))
        .route("/webview/list", get(list_webviews))
        .route("/webview/tags/create", post(create_webview_tag))
        .route("/webview/tags/delete", post(delete_webview_tags))
        .route("/webview/tags/rename", post(rename_webview_tag))
        .route("/webview/tags/list", get(list_webview_tags))
        .route("/repoview/create", post(create_repoview_entry))
        .route("/repoview/list", get(list_repoviews))
        .route("/repoview/tags/create", post(create_repoview_tag))
        .route("/repoview/tags/delete", post(delete_repoview_tags))
        .route("/repoview/tags/rename", post(rename_repoview_tag))
        .route("/repoview/tags/list", get(list_repoview_tags))
        .route("/tags/popular", get(popular_tags))
        .route("/tags/:endpoint_id", get(list_tags_for_endpoint))
        .route("/tags/:endpoint_id/add", post(add_tag))
        .route("/tags/:endpoint_id/remove", post(remove_tag))
        .route("/collections/:name/membership-tags/add", post(add_collection_tag))
        .route("/collections/:name/membership-tags/remove", post(remove_collection_tag))
        .route("/collections/:name/membership-tags", get(list_collection_tags))
        .route("/collections/by-tag/:tagname", get(collections_by_tag))
        .route("/repo/:collection/:filename/export", get(export_collection))
        .route("/repo/:collection/:filename/import", post(import_collection))
        .route("/internal/callback", post(ipc_callback))
        .route("/logs", get(get_logs))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("Listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;

    // CRUD Operations: global, one-time processing on launch (Ravi's [1]
    // approach) — spun up via the compute service, same as the Refresh
    // button. Fire-and-forget; result lands later via /internal/callback.
    // Spawned after the listener is bound so the child's callback POST has
    // somewhere to land.
    ipc::spawn_child(
        "compute_crud_operations",
        serde_json::json!({ "db_path": db_path.clone() }),
        3000,
    );

    axum::serve(listener, app).await?;

    Ok(())
}