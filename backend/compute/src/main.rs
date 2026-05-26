use axum::{
    Router,
    routing::{get, post},
};
use compute::api::handlers::{
    create_static, export_bookmarks, export_collection, export_merge, export_static,
    generate_markdown, generate_meta, health, import_zip, mark_active_folder_handler, system_init,
    system_status, write_endpoint, write_request, write_response,
};
use std::{error::Error, net::SocketAddr};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let root = compute::folder_manager::default_root_path();
    compute::folder_manager::verify_and_init(&root)?;

    let app = Router::new()
        .route("/health", get(health))
        .route("/system/status", get(system_status))
        .route("/system/init", post(system_init))
        .route("/export/collection", post(export_collection))
        .route("/export/merge", post(export_merge))
        .route("/import/zip", post(import_zip))
        .route("/export/bookmarks", post(export_bookmarks))
        .route("/static/create", post(create_static))
        .route("/static/export", post(export_static))
        .route("/markdown/generate", post(generate_markdown))
        .route("/markdown/meta", post(generate_meta))
        .route("/endpoint/write", post(write_endpoint))
        .route("/request/write", post(write_request))
        .route("/response/write", post(write_response))
        .route("/folders/active", post(mark_active_folder_handler))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let port = std::env::var("COMPUTE_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    info!("Compute service listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
