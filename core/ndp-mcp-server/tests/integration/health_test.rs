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

// Import server components - these would be from the main crate
// For integration tests, we need to construct the router directly

/// Helper function to create test server.
///
/// Creates an axum TestServer with the application router configured
/// for testing without starting a real TCP listener.
async fn create_test_server() -> TestServer {
    // Import from the crate being tested using public re-exports
    use ndp_mcp_server::{create_router, AppConfig, AppState};

    let config = AppConfig::default();
    let state = Arc::new(AppState::new(config));
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
