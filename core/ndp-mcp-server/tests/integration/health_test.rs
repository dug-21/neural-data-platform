//! Integration tests for the health check endpoint.
//!
//! Tests:
//! - Health endpoint returns 200 OK
//! - Response includes status and version
//! - Response is valid JSON

use axum::http::StatusCode;
use axum_test::TestServer;
use serde_json::Value;
use std::sync::Arc;

use ndp_mcp_server::etcd::{ConfigStore, StreamConfig};
use ndp_mcp_server::storage::{
    BronzeStorage, ColumnDescription, DictionaryEntry, DictionaryStore, DqRuleInfo,
    EtlHistoryResult, EtlRunStore, EtlStreamStatus, FreshnessReport, LineageTrace, SampleFilters,
    SilverStorage, SilverTableDescription, SilverTableInfo, SilverTableStats, StreamStorageInfo,
};
use ndp_mcp_server::{create_router, AppConfig, AppState, McpError, McpHandler, McpResult};

// Mock implementations for testing
struct MockStorage;
struct MockConfigStore;
struct MockSilverStorage;
struct MockDictionaryStore;
struct MockEtlRunStore;

#[async_trait::async_trait]
impl BronzeStorage for MockStorage {
    async fn list_streams(&self) -> McpResult<Vec<StreamStorageInfo>> {
        Ok(vec![])
    }
    async fn get_schema(
        &self,
        _stream_id: &str,
    ) -> McpResult<ndp_mcp_server::storage::ParquetSchemaInfo> {
        Err(McpError::StreamNotFound("mock".to_string()))
    }
    async fn sample(&self, _stream_id: &str, _n: usize) -> McpResult<Vec<Value>> {
        Err(McpError::StreamNotFound("mock".to_string()))
    }
    async fn latest_partition(&self, _stream_id: &str) -> McpResult<Option<String>> {
        Ok(None)
    }
}

#[async_trait::async_trait]
impl ConfigStore for MockConfigStore {
    async fn list_streams(&self) -> McpResult<Vec<String>> {
        Ok(vec!["test-stream".to_string()])
    }
    async fn get_config(&self, stream_id: &str) -> McpResult<StreamConfig> {
        Ok(StreamConfig {
            stream_id: stream_id.to_string(),
            enabled: true,
            ..Default::default()
        })
    }
    async fn get_enabled_streams(&self) -> McpResult<Vec<StreamConfig>> {
        Ok(vec![])
    }
    async fn validate(&self) -> McpResult<()> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl SilverStorage for MockSilverStorage {
    async fn list_tables(&self) -> McpResult<Vec<SilverTableInfo>> {
        Err(McpError::StorageError(
            "Silver layer not configured".to_string(),
        ))
    }

    async fn describe_table(&self, _table_name: &str) -> McpResult<SilverTableDescription> {
        Err(McpError::StorageError(
            "Silver layer not configured".to_string(),
        ))
    }

    async fn sample(
        &self,
        _table_name: &str,
        _n: usize,
        _filters: Option<SampleFilters>,
    ) -> McpResult<Vec<Value>> {
        Err(McpError::StorageError(
            "Silver layer not configured".to_string(),
        ))
    }

    async fn get_stats(&self, _table_name: &str) -> McpResult<SilverTableStats> {
        Err(McpError::StorageError(
            "Silver layer not configured".to_string(),
        ))
    }
}

#[async_trait::async_trait]
impl DictionaryStore for MockDictionaryStore {
    async fn search(&self, _query: &str, _layer: Option<String>) -> McpResult<Vec<DictionaryEntry>> {
        Err(McpError::StorageError(
            "Dictionary not configured".to_string(),
        ))
    }

    async fn describe_column(
        &self,
        _table_or_stream: &str,
        _column_name: &str,
    ) -> McpResult<ColumnDescription> {
        Err(McpError::StorageError(
            "Dictionary not configured".to_string(),
        ))
    }

    async fn trace_lineage(
        &self,
        _silver_table: &str,
        _silver_column: &str,
    ) -> McpResult<LineageTrace> {
        Err(McpError::StorageError(
            "Dictionary not configured".to_string(),
        ))
    }

    async fn list_dq_rules(
        &self,
        _table: Option<String>,
        _column: Option<String>,
    ) -> McpResult<Vec<DqRuleInfo>> {
        Err(McpError::StorageError(
            "Dictionary not configured".to_string(),
        ))
    }
}

#[async_trait::async_trait]
impl EtlRunStore for MockEtlRunStore {
    async fn get_status(&self, _stream_id: Option<String>) -> McpResult<Vec<EtlStreamStatus>> {
        Err(McpError::StorageError(
            "ETL store not configured".to_string(),
        ))
    }

    async fn get_history(
        &self,
        _stream_id: &str,
        _limit: usize,
        _since: Option<chrono::DateTime<chrono::Utc>>,
        _status_filter: Option<String>,
    ) -> McpResult<EtlHistoryResult> {
        Err(McpError::StorageError(
            "ETL store not configured".to_string(),
        ))
    }

    async fn get_freshness(&self, _layer: Option<String>) -> McpResult<FreshnessReport> {
        Err(McpError::StorageError(
            "ETL store not configured".to_string(),
        ))
    }
}

/// Helper function to create test server.
///
/// Creates an axum TestServer with the application router configured
/// for testing without starting a real TCP listener.
async fn create_test_server() -> TestServer {
    let config = AppConfig::default();
    let storage = Arc::new(MockStorage);
    let config_store = Arc::new(MockConfigStore);
    let silver_storage = Arc::new(MockSilverStorage);
    let dictionary_store = Arc::new(MockDictionaryStore);
    let etl_store = Arc::new(MockEtlRunStore);
    let handler = Arc::new(McpHandler::new(
        storage,
        config_store,
        silver_storage,
        dictionary_store,
        etl_store,
    ));
    let state = Arc::new(AppState::with_handler(config, handler));
    let app = create_router(state);

    TestServer::new(app).unwrap()
}

#[tokio::test]
async fn test_health_endpoint_returns_ok() {
    let server = create_test_server().await;

    let response = server.get("/health").await;

    assert_eq!(response.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn test_health_endpoint_returns_json() {
    let server = create_test_server().await;

    let response = server.get("/health").await;

    // Verify content type
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(content_type.contains("application/json"));

    // Verify valid JSON
    let body: Value = response.json();
    assert!(body.is_object());
}

#[tokio::test]
async fn test_health_endpoint_includes_status() {
    let server = create_test_server().await;

    let response = server.get("/health").await;
    let body: Value = response.json();

    // Must include status field
    assert!(body.get("status").is_some());
    assert_eq!(body["status"], "healthy");
}

#[tokio::test]
async fn test_health_endpoint_includes_version() {
    let server = create_test_server().await;

    let response = server.get("/health").await;
    let body: Value = response.json();

    // Must include version field
    assert!(body.get("version").is_some());

    // Version should be a non-empty string
    let version = body["version"].as_str().unwrap();
    assert!(!version.is_empty());
}
