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
use crate::storage::{
    BronzeStorage, DictionaryStore, EtlRunStore, LocalParquetStorage, SilverStorage,
};

/// Application state shared across all request handlers.
///
/// Contains references to configuration and storage/config clients.
/// Wrapped in Arc for thread-safe sharing.
///
/// # Type Parameters
///
/// - `B`: Bronze layer storage (e.g., LocalParquetStorage)
/// - `C`: Configuration store (e.g., StreamRegistryAdapter)
/// - `S`: Silver layer storage (e.g., TimescaleStorage)
/// - `D`: Dictionary store (e.g., DictionaryClient)
/// - `E`: ETL run store (e.g., EtlRunClient)
pub struct AppState<B, C, S, D, E>
where
    B: BronzeStorage + Send + Sync + 'static,
    C: ConfigStore + Send + Sync + 'static,
    S: SilverStorage + Send + Sync + 'static,
    D: DictionaryStore + Send + Sync + 'static,
    E: EtlRunStore + Send + Sync + 'static,
{
    /// Application configuration
    pub config: AppConfig,
    /// MCP request handler with storage and config dependencies
    pub handler: Arc<McpHandler<B, C, S, D, E>>,
}

impl<B, C, S, D, E> Clone for AppState<B, C, S, D, E>
where
    B: BronzeStorage + Send + Sync + 'static,
    C: ConfigStore + Send + Sync + 'static,
    S: SilverStorage + Send + Sync + 'static,
    D: DictionaryStore + Send + Sync + 'static,
    E: EtlRunStore + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            handler: Arc::clone(&self.handler),
        }
    }
}

impl<B, C, S, D, E> AppState<B, C, S, D, E>
where
    B: BronzeStorage + Send + Sync + 'static,
    C: ConfigStore + Send + Sync + 'static,
    S: SilverStorage + Send + Sync + 'static,
    D: DictionaryStore + Send + Sync + 'static,
    E: EtlRunStore + Send + Sync + 'static,
{
    /// Create application state with custom dependencies.
    ///
    /// Used for testing with mock implementations or production with real adapters.
    pub fn with_handler(config: AppConfig, handler: Arc<McpHandler<B, C, S, D, E>>) -> Self {
        Self { config, handler }
    }
}

// =============================================================================
// Convenience constructor for Bronze-only (legacy/Phase 1)
// =============================================================================

/// Placeholder Silver storage for Bronze-only mode.
///
/// Returns errors for all operations. Used when Silver layer is not available.
pub struct NoOpSilverStorage;

#[async_trait::async_trait]
impl SilverStorage for NoOpSilverStorage {
    async fn list_tables(&self) -> crate::error::McpResult<Vec<crate::storage::SilverTableInfo>> {
        Err(crate::error::McpError::StorageError(
            "Silver layer not configured".to_string(),
        ))
    }

    async fn describe_table(
        &self,
        _table_name: &str,
    ) -> crate::error::McpResult<crate::storage::SilverTableDescription> {
        Err(crate::error::McpError::StorageError(
            "Silver layer not configured".to_string(),
        ))
    }

    async fn sample(
        &self,
        _table_name: &str,
        _n: usize,
        _filters: Option<crate::storage::SampleFilters>,
    ) -> crate::error::McpResult<Vec<serde_json::Value>> {
        Err(crate::error::McpError::StorageError(
            "Silver layer not configured".to_string(),
        ))
    }

    async fn get_stats(
        &self,
        _table_name: &str,
    ) -> crate::error::McpResult<crate::storage::SilverTableStats> {
        Err(crate::error::McpError::StorageError(
            "Silver layer not configured".to_string(),
        ))
    }
}

/// Placeholder Dictionary store for Bronze-only mode.
pub struct NoOpDictionaryStore;

#[async_trait::async_trait]
impl DictionaryStore for NoOpDictionaryStore {
    async fn search(
        &self,
        _query: &str,
        _layer: Option<String>,
    ) -> crate::error::McpResult<Vec<crate::storage::DictionaryEntry>> {
        Err(crate::error::McpError::StorageError(
            "Dictionary not configured".to_string(),
        ))
    }

    async fn describe_column(
        &self,
        _table_or_stream: &str,
        _column_name: &str,
    ) -> crate::error::McpResult<crate::storage::ColumnDescription> {
        Err(crate::error::McpError::StorageError(
            "Dictionary not configured".to_string(),
        ))
    }

    async fn trace_lineage(
        &self,
        _silver_table: &str,
        _silver_column: &str,
    ) -> crate::error::McpResult<crate::storage::LineageTrace> {
        Err(crate::error::McpError::StorageError(
            "Dictionary not configured".to_string(),
        ))
    }

    async fn list_dq_rules(
        &self,
        _table: Option<String>,
        _column: Option<String>,
    ) -> crate::error::McpResult<Vec<crate::storage::DqRuleInfo>> {
        Err(crate::error::McpError::StorageError(
            "Dictionary not configured".to_string(),
        ))
    }
}

/// Placeholder ETL run store for Bronze-only mode.
pub struct NoOpEtlRunStore;

#[async_trait::async_trait]
impl EtlRunStore for NoOpEtlRunStore {
    async fn get_status(
        &self,
        _stream_id: Option<String>,
    ) -> crate::error::McpResult<Vec<crate::storage::EtlStreamStatus>> {
        Err(crate::error::McpError::StorageError(
            "ETL store not configured".to_string(),
        ))
    }

    async fn get_history(
        &self,
        _stream_id: &str,
        _limit: usize,
        _since: Option<chrono::DateTime<chrono::Utc>>,
        _status_filter: Option<String>,
    ) -> crate::error::McpResult<crate::storage::EtlHistoryResult> {
        Err(crate::error::McpError::StorageError(
            "ETL store not configured".to_string(),
        ))
    }

    async fn get_freshness(
        &self,
        _layer: Option<String>,
    ) -> crate::error::McpResult<crate::storage::FreshnessReport> {
        Err(crate::error::McpError::StorageError(
            "ETL store not configured".to_string(),
        ))
    }
}

impl
    AppState<
        LocalParquetStorage,
        StreamRegistryAdapter,
        NoOpSilverStorage,
        NoOpDictionaryStore,
        NoOpEtlRunStore,
    >
{
    /// Create new application state with StreamRegistry from config-client.
    ///
    /// Uses the shared config-client crate for etcd access.
    /// The StreamRegistry provides cached access to stream configurations.
    ///
    /// This constructor creates a Bronze-only server. Silver, Dictionary,
    /// and ETL tools will return "not configured" errors.
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
        let silver_storage = Arc::new(NoOpSilverStorage);
        let dictionary_store = Arc::new(NoOpDictionaryStore);
        let etl_store = Arc::new(NoOpEtlRunStore);
        let handler = Arc::new(McpHandler::new(
            storage,
            config_store,
            silver_storage,
            dictionary_store,
            etl_store,
        ));

        Self { config, handler }
    }
}

// =============================================================================
// Full TimescaleDB constructor (dp-010 BUG-001 Phase 2)
// =============================================================================

use crate::error::McpError;
use crate::storage::{
    TimescaleDictionaryStore, TimescaleEtlRunStore, TimescalePoolConfig, TimescaleSilverStorage,
};

impl
    AppState<
        LocalParquetStorage,
        StreamRegistryAdapter,
        TimescaleSilverStorage,
        TimescaleDictionaryStore,
        TimescaleEtlRunStore,
    >
{
    /// Create application state with full Silver layer support.
    ///
    /// Uses TimescaleDB for Silver, Dictionary, and ETL storage.
    /// All 15 MCP tools will be fully functional.
    ///
    /// # Arguments
    ///
    /// * `config` - Application configuration (must have `timescale_url` set)
    /// * `registry` - StreamRegistry instance from config-client
    ///
    /// # Errors
    ///
    /// Returns `McpError::Config` if `timescale_url` is not configured.
    /// Returns `McpError::StorageError` if TimescaleDB connection fails.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = AppConfig::from_env()?;
    /// let registry = config.create_stream_registry().await?;
    /// if config.has_timescale() {
    ///     let state = AppState::with_timescale(config, registry).await?;
    /// }
    /// ```
    pub async fn with_timescale(
        config: AppConfig,
        registry: StreamRegistry,
    ) -> Result<Self, McpError> {
        let timescale_url = config
            .timescale_url
            .as_ref()
            .ok_or_else(|| McpError::Config("NDP_TIMESCALE_URL is required".to_string()))?;

        tracing::info!(
            max_connections = config.timescale_max_connections,
            timeout_secs = config.timescale_connect_timeout_secs,
            "Creating TimescaleDB storage adapters"
        );

        // Create pool configuration from AppConfig
        let pool_config = TimescalePoolConfig {
            max_size: config.timescale_max_connections,
            min_idle: Some(1),
            connection_timeout_secs: config.timescale_connect_timeout_secs,
            idle_timeout_secs: 30,
        };

        // Create Bronze storage (local Parquet)
        let storage = Arc::new(LocalParquetStorage::new(&config.raw_path));
        let config_store = Arc::new(StreamRegistryAdapter::new(registry));

        // Create TimescaleDB adapters
        let silver_storage = Arc::new(
            TimescaleSilverStorage::with_config(timescale_url, pool_config).await?,
        );
        let dictionary_store = Arc::new(TimescaleDictionaryStore::new(timescale_url).await?);
        let etl_store = Arc::new(TimescaleEtlRunStore::new(timescale_url).await?);

        let handler = Arc::new(McpHandler::new(
            storage,
            config_store,
            silver_storage,
            dictionary_store,
            etl_store,
        ));

        tracing::info!("TimescaleDB storage adapters ready - all 15 MCP tools enabled");

        Ok(Self { config, handler })
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
pub fn create_router<B, C, S, D, E>(state: Arc<AppState<B, C, S, D, E>>) -> Router
where
    B: BronzeStorage + Send + Sync + 'static,
    C: ConfigStore + Send + Sync + 'static,
    S: SilverStorage + Send + Sync + 'static,
    D: DictionaryStore + Send + Sync + 'static,
    E: EtlRunStore + Send + Sync + 'static,
{
    Router::new()
        .route("/mcp", post(mcp_handler::<B, C, S, D, E>))
        .route("/health", get(health_check::<B, C, S, D, E>))
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
async fn mcp_handler<B, C, S, D, E>(
    State(state): State<Arc<AppState<B, C, S, D, E>>>,
    Json(request): Json<JsonRpcRequest>,
) -> impl IntoResponse
where
    B: BronzeStorage + Send + Sync + 'static,
    C: ConfigStore + Send + Sync + 'static,
    S: SilverStorage + Send + Sync + 'static,
    D: DictionaryStore + Send + Sync + 'static,
    E: EtlRunStore + Send + Sync + 'static,
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
async fn health_check<B, C, S, D, E>(
    State(_state): State<Arc<AppState<B, C, S, D, E>>>,
) -> (StatusCode, Json<HealthResponse>)
where
    B: BronzeStorage + Send + Sync + 'static,
    C: ConfigStore + Send + Sync + 'static,
    S: SilverStorage + Send + Sync + 'static,
    D: DictionaryStore + Send + Sync + 'static,
    E: EtlRunStore + Send + Sync + 'static,
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
    use crate::storage::{MockBronzeStorage, MockDictionaryStore, MockEtlRunStore, MockSilverStorage};

    fn create_test_state() -> Arc<
        AppState<
            MockBronzeStorage,
            MockConfigStore,
            MockSilverStorage,
            MockDictionaryStore,
            MockEtlRunStore,
        >,
    > {
        let config = AppConfig::default();
        let storage = Arc::new(MockBronzeStorage::new());
        let config_store = Arc::new(MockConfigStore::new());
        let silver_storage = Arc::new(MockSilverStorage::new());
        let dictionary_store = Arc::new(MockDictionaryStore::new());
        let etl_store = Arc::new(MockEtlRunStore::new());
        let handler = Arc::new(McpHandler::new(
            storage,
            config_store,
            silver_storage,
            dictionary_store,
            etl_store,
        ));
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
        let response =
            JsonRpcResponse::error(Some(serde_json::json!(1)), -32601, "Method not found");
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
