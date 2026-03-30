use std::net::SocketAddr;
use std::path::PathBuf;

use tracing::{info,error};
use tracing_subscriber::EnvFilter;
mod api;
mod config;
mod errors;
mod folder_manager;
mod lock_manager;
mod markdown_generator;
mod markdown_meta;
mod endpoint_writer;
 mod zipops;

#[tokio::main]
async fn main() {

    // ── 1. Initialise structured logging ─────────────────────────────────────
    //
    // Controlled by the RUST_LOG environment variable at runtime:
    //   RUST_LOG=info  cargo run       → normal output
    //   RUST_LOG=debug cargo run       → verbose
    //   RUST_LOG=warn  cargo run       → quiet (warnings + errors only)
    //
    // Defaults to "info" if RUST_LOG is not set.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(true)
        .with_line_number(true)
        .compact()
        .init();

    info!("Starting EDMS Systems Server");

    // ── 2. Folder structure verification ─────────────────────────────────────
    //
    // Replaces the old interactive yes/no prompt which broke Docker and CI.
    // Now auto-initialises on startup and exposes /system/init + /system/status
    // endpoints for on-demand re-initialisation via the API.
    //
    // Behaviour:
    //   Ok         → log and continue
    //   Missing    → create missing folders automatically, log what was created
    //   Corrupted  → reset structure automatically, log the action
    //
    // To override the root path (e.g. in Docker), set the EDMS_ROOT env var:
    //   EDMS_ROOT=/app/edms_root ./endpoint_mng_sys
    let root: PathBuf = std::env::var("EDMS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| folder_manager::default_root_path());

    match folder_manager::verify_and_init(&root) {
        Ok(report) => {
            info!(
                action   = ?report.action,
                root     = %report.root_path,
                missing  = ?report.missing_folders,
                "{}",
                report.message
            );
        }
        Err(e) => {
            // Folder init failure is fatal — the server cannot operate
            // without its directory structure.
            error!(error = %e, "Folder initialisation failed — cannot start server");
            std::process::exit(1);
        }
    }

    // ── 3. Build router ───────────────────────────────────────────────────────
    let app = api::routes::create_router();

    // ── 4. Bind address ───────────────────────────────────────────────────────
    //
    // 0.0.0.0 is required for Docker port forwarding to work.
    // 127.0.0.1 (loopback) only accepts connections from inside the container.
    //
    // The port can be overridden with the PORT env var:
    //   PORT=8080 ./endpoint_mng_sys
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(5000);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l)  => { info!(address = %addr, "Server listening"); l }
        Err(e) => {
            error!(address = %addr, error = %e, "Failed to bind — is port already in use?");
            std::process::exit(1);
        }
    };

    // ── 5. Serve with graceful shutdown ───────────────────────────────────────
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap_or_else(|e| error!(error = %e, "Server error"));
}

// ── Graceful shutdown on Ctrl+C / SIGTERM ─────────────────────────────────────
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install Ctrl+C handler");
    info!("Shutdown signal received — stopping server gracefully");
}