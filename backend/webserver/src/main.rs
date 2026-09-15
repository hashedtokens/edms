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
    endpoints::{create_endpoint, delete_endpoint},
    logs::get_logs,
    repo::{export_collection, import_collection},
    tags::{add_tag, list_tags_for_endpoint, popular_tags, remove_tag},
    test_view::{
        clear_bookmarks, clear_history, get_saved_headers, get_saved_request, get_saved_response,
        save_bookmark, save_history, stop, ws_add_from_history_to_bookmark,
        ws_delete_from_bookmark, ws_load_bookmarks, ws_load_endpoints, ws_load_history, ws_run,
    },
    view::{home, list_view, test_view, trigger_view_refresh},
    view_catalog::{
        create_collection_entry, create_repoview_entry,
        create_webview_entry, delete_collection_entry, get_collection_entry,
        list_collection_endpoints, list_collections, list_repoviews, list_webviews,
        remove_endpoint_from_collection, rename_collection_entry,
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    //Folder structure
    // EDMS_ROOT lets the deployment pin the storage root explicitly —
    // needed under Docker, where default_root_path() (which walks up
    // looking for a `compute` dir) resolves to `/edms_root` at runtime
    // while the persistent volume may be mounted elsewhere. Falls back
    // to the walk-up behavior for local `cargo run`.
    let root = std::env::var("EDMS_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| compute::folder_manager::default_root_path());
    println!("Initializing EDMS root at: {:?}", root);
    
    compute::folder_manager::verify_and_init(&root)
        .expect("Failed to initialize system folders");

    let log_writer = logging::AppLogWriter::new(&root).expect("Failed to open log file");
    tracing_subscriber::fmt()
        .with_target(false)
        .with_writer(move || log_writer.clone())
        .init();

    let app_config = std::env::var("EDMS_CONFIG_PATH")
        .unwrap_or_else(|_| "config.yaml".to_string());
    let config = Arc::new(config::AppConfig::from_file_or_default(&app_config));

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
        .route("/endpoints/:endpoint_id/delete", post(delete_endpoint))
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
        .route("/collections/:name/delete", post(delete_collection_entry))
        .route("/collections/:name/endpoints/remove", post(remove_endpoint_from_collection))
        .route("/collections/:name/endpoints", get(list_collection_endpoints))
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