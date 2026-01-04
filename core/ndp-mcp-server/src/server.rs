//! HTTP server setup using axum framework.
//!
//! Implements the HTTP transport layer for MCP protocol as defined in ADR-001.
//! Routes:
//! - POST /mcp: MCP JSON-RPC endpoint
//! - GET /health: Health check endpoint
//!
//! # Configuration Store
//!
//! The server uses `StreamRegistryAdapter` which wraps config-client's StreamRegistry
//! for accessing stream configurations from etcd.
//!
//! Use `AppState::with_registry()` to create the application state.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use config_client::StreamRegistry;
use serde::Serialize;
use std::sync::Arc;
use tower_http::trace::TraceLayer;

use crate::config::AppConfig;
use crate::etcd::{ConfigStore, StreamRegistryAdapter};
use crate::mcp::{JsonRpcRequest, McpHandler};
use crate::storage::{BronzeStorage, LocalParquetStorage};

/// Application state shared across all request handlers.
///
/// Contains references to configuration and storage/config clients.
/// Wrapped in Arc for thread-safe sharing.
pub struct AppState<S = LocalParquetStorage, C = StreamRegistryAdapter>
where
    S: BronzeStorage + Send + Sync + 'static,
    C: ConfigStore + Send + Sync + 'static,
{
    /// Application configuration
    pub config: AppConfig,
    /// MCP request handler with storage and config dependencies
    pub handler: Arc<McpHandler<S, C>>,
}

impl<S, C> Clone for AppState<S, C>
where
    S: BronzeStorage + Send + Sync + 'static,
    C: ConfigStore + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            handler: Arc::clone(&self.handler),
        }
    }
}

impl AppState<LocalParquetStorage, StreamRegistryAdapter> {
    /// Create new application state with StreamRegistry from config-client.
    ///
    /// Uses the shared config-client crate for etcd access.
    /// The StreamRegistry provides cached access to stream configurations.
    ///
    /// # Arguments
    ///
    /// * `config` - Application configuration
    /// * `registry` - StreamRegistry instance from config-client
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = AppConfig::from_env()?;
    /// let registry = config.create_stream_registry().await?;
    /// let state = AppState::with_registry(config, registry);
    /// ```
    pub fn with_registry(config: AppConfig, registry: StreamRegistry) -> Self {
        let storage = Arc::new(LocalParquetStorage::new(&config.raw_path));
        let config_store = Arc::new(StreamRegistryAdapter::new(registry));
        let handler = Arc::new(McpHandler::new(storage, config_store));

        Self { config, handler }
    }
}

impl<S, C> AppState<S, C>
where
    S: BronzeStorage + Send + Sync + 'static,
    C: ConfigStore + Send + Sync + 'static,
{
    /// Create application state with custom dependencies.
    ///
    /// Used for testing with mock implementations.
    pub fn with_handler(config: AppConfig, handler: Arc<McpHandler<S, C>>) -> Self {
        Self { config, handler }
    }
}

/// Create the axum router with all routes configured.
///
/// # Routes
///
/// - `POST /mcp`: MCP JSON-RPC protocol endpoint
/// - `GET /health`: Health check endpoint
///
/// # Middleware
///
/// - TraceLayer: Request/response tracing for observability
pub fn create_router<S, C>(state: Arc<AppState<S, C>>) -> Router
where
    S: BronzeStorage + Send + Sync + 'static,
    C: ConfigStore + Send + Sync + 'static,
{
    Router::new()
        .route("/mcp", post(mcp_handler::<S, C>))
        .route("/health", get(health_check::<S, C>))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

// =============================================================================
// MCP Protocol Handler
// =============================================================================

/// MCP JSON-RPC endpoint handler.
///
/// Delegates all MCP requests to the McpHandler which implements
/// the full protocol logic including tool execution.
///
/// Returns JSON-RPC 2.0 formatted responses.
async fn mcp_handler<S, C>(
    State(state): State<Arc<AppState<S, C>>>,
    Json(request): Json<JsonRpcRequest>,
) -> impl IntoResponse
where
    S: BronzeStorage + Send + Sync + 'static,
    C: ConfigStore + Send + Sync + 'static,
{
    let response = state.handler.handle(request).await;
    Json(response)
}

// =============================================================================
// Health Check
// =============================================================================

/// Health check response structure.
#[derive(Serialize)]
pub struct HealthResponse {
    /// Server health status
    pub status: String,
    /// Server version
    pub version: String,
    /// Component health (optional details)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<HealthComponents>,
}

/// Component health details.
#[derive(Serialize)]
pub struct HealthComponents {
    /// etcd connection status
    pub etcd: ComponentStatus,
    /// Storage layer status
    pub storage: ComponentStatus,
}

/// Individual component status.
#[derive(Serialize)]
pub struct ComponentStatus {
    /// Whether component is healthy
    pub healthy: bool,
    /// Status message
    pub message: String,
}

/// Health check endpoint handler.
///
/// Returns server health status with version information.
/// Used by load balancers and orchestrators for health monitoring.
async fn health_check<S, C>(
    State(_state): State<Arc<AppState<S, C>>>,
) -> (StatusCode, Json<HealthResponse>)
where
    S: BronzeStorage + Send + Sync + 'static,
    C: ConfigStore + Send + Sync + 'static,
{
    // TODO: Add actual component health checks in Phase 4
    let response = HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        components: None, // Will be populated with component health in Phase 4
    };

    (StatusCode::OK, Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::etcd::MockConfigStore;
    use crate::mcp::JsonRpcResponse;
    use crate::storage::MockBronzeStorage;

    fn create_test_state() -> Arc<AppState<MockBronzeStorage, MockConfigStore>> {
        let config = AppConfig::default();
        let storage = Arc::new(MockBronzeStorage::new());
        let config_store = Arc::new(MockConfigStore::new());
        let handler = Arc::new(McpHandler::new(storage, config_store));
        Arc::new(AppState::with_handler(config, handler))
    }

    #[test]
    fn test_json_rpc_response_success() {
        let response = JsonRpcResponse::success(
            Some(serde_json::json!(1)),
            serde_json::json!({"test": "value"}),
        );
        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_json_rpc_response_error() {
        let response = JsonRpcResponse::error(
            Some(serde_json::json!(1)),
            -32601,
            "Method not found",
        );
        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());
        assert_eq!(response.error.as_ref().unwrap().code, -32601);
    }

    #[test]
    fn test_create_router() {
        let state = create_test_state();
        let _router = create_router(state);
        // Router creation should succeed
    }

    #[test]
    fn test_app_state_clone() {
        let state = create_test_state();
        let cloned = state.as_ref().clone();
        assert_eq!(cloned.config.listen_addr, state.config.listen_addr);
    }
}
