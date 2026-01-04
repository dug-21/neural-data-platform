//! Integration tests for the MCP protocol endpoints.
//!
//! Tests:
//! - MCP initialize returns capabilities
//! - tools/list returns tool definitions
//! - tools/call routes to correct handlers
//! - Error responses follow JSON-RPC 2.0 format

use axum::http::StatusCode;
use axum_test::TestServer;
use serde_json::{json, Value};
use std::sync::Arc;

use ndp_mcp_server::etcd::{ConfigStore, StreamConfig};
use ndp_mcp_server::storage::{BronzeStorage, StreamStorageInfo};
use ndp_mcp_server::{create_router, AppConfig, AppState, McpError, McpHandler, McpResult};

// Mock implementations for testing
struct MockStorage;
struct MockConfigStore;

#[async_trait::async_trait]
impl BronzeStorage for MockStorage {
    async fn list_streams(&self) -> McpResult<Vec<StreamStorageInfo>> {
        Ok(vec![StreamStorageInfo {
            stream_id: "test-stream".to_string(),
            latest_partition: None,
            file_size_bytes: None,
            file_modified: None,
            row_count: None,
        }])
    }
    async fn get_schema(&self, _stream_id: &str) -> McpResult<ndp_mcp_server::storage::ParquetSchemaInfo> {
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

/// Helper function to create test server.
async fn create_test_server() -> TestServer {
    let config = AppConfig::default();
    let storage = Arc::new(MockStorage);
    let config_store = Arc::new(MockConfigStore);
    let handler = Arc::new(McpHandler::new(storage, config_store));
    let state = Arc::new(AppState::with_handler(config, handler));
    let app = create_router(state);

    TestServer::new(app).unwrap()
}

#[tokio::test]
async fn test_mcp_initialize_returns_capabilities() {
    let server = create_test_server().await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    });

    let response = server
        .post("/mcp")
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let body: Value = response.json();

    // Verify JSON-RPC structure
    assert_eq!(body["jsonrpc"], "2.0");
    assert_eq!(body["id"], 1);

    // Verify result contains expected fields
    let result = &body["result"];
    assert!(result.get("protocolVersion").is_some());
    assert!(result.get("serverInfo").is_some());
    assert!(result.get("capabilities").is_some());

    // Verify server info
    assert_eq!(result["serverInfo"]["name"], "ndp-mcp-server");
}

#[tokio::test]
async fn test_mcp_tools_list_returns_tools() {
    let server = create_test_server().await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });

    let response = server
        .post("/mcp")
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let body: Value = response.json();
    let result = &body["result"];

    // Verify tools array exists
    let tools = result["tools"].as_array().expect("tools should be array");

    // Verify expected tools are present
    let tool_names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();

    assert!(tool_names.contains(&"list_streams"));
    assert!(tool_names.contains(&"describe_schema"));
    assert!(tool_names.contains(&"validate_config"));
    assert!(tool_names.contains(&"sample_data"));
}

#[tokio::test]
async fn test_mcp_tools_have_input_schemas() {
    let server = create_test_server().await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/list",
        "params": {}
    });

    let response = server
        .post("/mcp")
        .json(&request)
        .await;

    let body: Value = response.json();
    let tools = body["result"]["tools"].as_array().unwrap();

    // Each tool must have inputSchema
    for tool in tools {
        assert!(
            tool.get("inputSchema").is_some(),
            "Tool {} missing inputSchema",
            tool["name"]
        );
    }
}

#[tokio::test]
async fn test_mcp_unknown_method_returns_error() {
    let server = create_test_server().await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "unknown/method",
        "params": {}
    });

    let response = server
        .post("/mcp")
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let body: Value = response.json();

    // Should return error, not result
    assert!(body.get("error").is_some());
    assert!(body.get("result").is_none());

    // Error code should be -32601 (Method not found)
    assert_eq!(body["error"]["code"], -32601);
}

#[tokio::test]
async fn test_mcp_tools_call_list_streams() {
    let server = create_test_server().await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "list_streams",
            "arguments": {}
        }
    });

    let response = server
        .post("/mcp")
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let body: Value = response.json();

    // Should have result with content
    let result = &body["result"];
    assert!(result.get("content").is_some());

    let content = result["content"].as_array().expect("content should be array");
    assert!(!content.is_empty());
}

#[tokio::test]
async fn test_mcp_tools_call_unknown_tool() {
    let server = create_test_server().await;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "tools/call",
        "params": {
            "name": "unknown_tool",
            "arguments": {}
        }
    });

    let response = server
        .post("/mcp")
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let body: Value = response.json();

    // Should return error for unknown tool
    assert!(body.get("error").is_some());
    assert_eq!(body["error"]["code"], -32602);
}

#[tokio::test]
async fn test_mcp_preserves_request_id() {
    let server = create_test_server().await;

    // Test with string ID
    let request = json!({
        "jsonrpc": "2.0",
        "id": "test-id-123",
        "method": "initialize",
        "params": {}
    });

    let response = server
        .post("/mcp")
        .json(&request)
        .await;

    let body: Value = response.json();
    assert_eq!(body["id"], "test-id-123");
}

#[tokio::test]
async fn test_mcp_handles_null_id() {
    let server = create_test_server().await;

    // Null ID (notification-style, though we still respond)
    let request = json!({
        "jsonrpc": "2.0",
        "id": null,
        "method": "tools/list",
        "params": {}
    });

    let response = server
        .post("/mcp")
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let body: Value = response.json();
    assert!(body["id"].is_null());
}
