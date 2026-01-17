//! NDP MCP Server - Entry Point
//!
//! This is the main entry point for the NDP MCP server, providing data exploration
//! tools for both Bronze (Parquet) and Silver (TimescaleDB) layers via the
//! Model Context Protocol.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  Client (Claude Code)                                       │
//! │                                                              │
//! │   POST /mcp (JSON-RPC) ──► MCP Handler ──► Tool Executor   │
//! │   GET  /health         ──► Health Check                    │
//! │                                                              │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Configuration
//!
//! All configuration via environment variables:
//! - `NDP_MCP_LISTEN`: Server bind address (default: "0.0.0.0:9100")
//! - `NDP_ETCD_ENDPOINTS`: etcd endpoints, comma-separated (default: "http://localhost:2379")
//! - `NDP_RAW_PATH`: Bronze layer data path (default: "/data/raw")
//! - `RUST_LOG`: Log level (default: "info")
//!
//! # TimescaleDB (Optional - enables Silver layer tools)
//!
//! - `NDP_TIMESCALE_URL`: PostgreSQL connection URL (optional)
//! - `NDP_TIMESCALE_MAX_CONNECTIONS`: Max pool connections (default: 5)
//! - `NDP_TIMESCALE_CONNECT_TIMEOUT_SECS`: Connection timeout (default: 10)
//!
//! When `NDP_TIMESCALE_URL` is set, all 15 MCP tools are enabled (Bronze + Silver).
//! Without it, only the 4 Bronze layer tools are available.

mod config;
mod error;
mod etcd;
mod mcp;
mod server;
mod storage;

use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::config::AppConfig;
use crate::server::{create_router, AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration from environment
    let config = AppConfig::from_env()?;
    config.validate()?;

    // Initialize tracing with JSON output for structured logging
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level)),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    let has_timescale = config.has_timescale();
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        listen_addr = %config.listen_addr,
        raw_path = %config.raw_path,
        etcd_endpoints = ?config.etcd_endpoints,
        timescale_enabled = has_timescale,
        "Starting NDP MCP Server"
    );

    // Create StreamRegistry via config-client (preferred approach)
    tracing::info!("Connecting to etcd via config-client...");
    let registry = config.create_stream_registry().await?;
    tracing::info!("StreamRegistry connected successfully");

    // Create router based on configuration
    // dp-010 BUG-001 fix: Use TimescaleDB adapters when NDP_TIMESCALE_URL is set
    let app = if has_timescale {
        tracing::info!(
            url = %config.timescale_url.as_ref().map(|_| "[redacted]").unwrap_or("none"),
            max_connections = config.timescale_max_connections,
            "Initializing TimescaleDB storage adapters..."
        );

        let state = AppState::with_timescale(config.clone(), registry).await?;
        tracing::info!("All 15 MCP tools enabled (Bronze + Silver + Dictionary + ETL)");
        create_router(Arc::new(state))
    } else {
        tracing::info!("No TimescaleDB URL configured - Bronze-only mode (4 tools)");
        let state = Arc::new(AppState::with_registry(config.clone(), registry));

        tracing::debug!(
            raw_path = %config.raw_path,
            "Initialized LocalParquetStorage"
        );
        tracing::debug!(
            etcd_endpoints = ?config.etcd_endpoints,
            "Initialized StreamRegistryAdapter"
        );

        create_router(state)
    };

    // Create TCP listener
    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    tracing::info!(addr = %config.listen_addr, "Server listening");

    // Start server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Server shutdown complete");
    Ok(())
}

/// Signal handler for graceful shutdown.
///
/// Listens for SIGTERM (Docker/Kubernetes) and SIGINT (Ctrl+C).
/// On signal receipt, allows in-flight requests to complete before shutdown.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C, initiating graceful shutdown...");
        }
        _ = terminate => {
            tracing::info!("Received SIGTERM, initiating graceful shutdown...");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_loads() {
        // Uses defaults when env vars not set
        let config = AppConfig::from_env().unwrap();
        assert!(config.validate().is_ok());
    }
}
