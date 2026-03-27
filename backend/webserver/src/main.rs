mod db;
mod events;
mod handlers;
mod ipc;
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
    bookmarks::{create_collection, ws_load_collection},
    callback::ipc_callback,
    dataview::{dashboard, delete_folder, merge_folder, ws_make_folder_active},
    repo::export_collection,
    test_view::{
        clear_bookmarks, clear_history, save_bookmark, save_history, stop,
        ws_add_from_history_to_bookmark, ws_delete_from_bookmark, ws_load_bookmarks,
        ws_load_endpoints, ws_run,
    },
    view::{home, list_view, test_view},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_target(false).init();

    let db_path = std::env::var("EDMS_DB_PATH").unwrap_or_else(|_| "edms.db".to_string());

    // Core + schema init
    let core = Arc::new(EdmsCore::new(&db_path));
    core.connect().map_err(|e| anyhow::anyhow!("{e:?}"))?;
    initialize_schema_from_core(&core).map_err(|e| anyhow::anyhow!("{e:?}"))?;

    let queries = Arc::new(QueryMap::load_or_default());
    let state = state::AppState::new(core, queries);

    let app = Router::new()
        // Views (REST)
        .route("/home", get(home))
        .route("/test-view", get(test_view))
        .route("/list-view", get(list_view))
        // Test-view (WS + REST)
        .route("/test-view/endpoints/load", get(ws_load_endpoints))
        .route("/test-view/bookmarks/load", get(ws_load_bookmarks))
        .route("/test-view/run", get(ws_run))
        .route("/test-view/stop", post(stop))
        .route("/test-view/save/history", post(save_history))
        .route("/test-view/save/bookmark", post(save_bookmark))
        .route("/test-view/:bookmark/add", get(ws_add_from_history_to_bookmark))
        .route("/test-view/:bookmark/delete", get(ws_delete_from_bookmark))
        .route("/test-view/history/clearall", post(clear_history))
        .route("/test-view/bookmark/clearall", post(clear_bookmarks))
        // Bookmarks collection
        .route("/bookmarks/:collection/create", post(create_collection))
        .route("/bookmarks/:collection/load", get(ws_load_collection))
        // Dataview
        .route("/dataview/:folder/delete", post(delete_folder))
        .route("/dataview/:folder/merge", post(merge_folder))
        .route("/dataview/:folder/active", get(ws_make_folder_active))
        .route("/dataview/dashboard", get(dashboard))
        // Repo export
        .route("/repo/:collection/:filename/export", get(export_collection))
        // Internal: IPC callback (child processes report back here)
        .route("/internal/callback", post(ipc_callback))
        // Middleware
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("Listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}