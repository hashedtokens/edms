use axum::{
    routing::{get, post},
    Router,
};
use tower_http::{
    trace::TraceLayer,
};
use tower::limit::{ConcurrencyLimitLayer};
use tower_http::limit::RequestBodyLimitLayer;

use super::handlers;

pub fn create_router() -> Router {
    Router::new()
        //folder init
        .route("/system/status", get(handlers::system_status))
        .route("/system/init",post(handlers::system_init))
        // Health
        .route("/health", get(handlers::health))

        // Zip operations
        .route("/export/collection",  post(handlers::export_collection))
        .route("/export/merge",       post(handlers::export_merge))
        .route("/export/bookmarks",   post(handlers::export_bookmarks))
        .route("/import/zip",         post(handlers::import_zip))

        // Static site
        .route("/static/create",      post(handlers::create_static))
        .route("/static/export",      post(handlers::export_static))

        // Markdown
        .route("/markdown/generate",  post(handlers::generate_markdown))
        .route("/markdown/meta",      post(handlers::generate_meta))

        // Endpoint files
        .route("/endpoint/write",     post(handlers::write_endpoint))
        .route("/endpoint/request",   post(handlers::write_request))
        .route("/endpoint/response",  post(handlers::write_response))

        // Active folder
        .route("/mark-active-folder", post(handlers::mark_active_folder_handler))

        //Middlewares
        .layer(ConcurrencyLimitLayer::new(50))
        .layer(RequestBodyLimitLayer::new(10*1024*1024)) // 10 mb body limt
        .layer(TraceLayer::new_for_http()) // request response logging
}