//! Integration tests for etcd configuration loading
//!
//! Requires etcd running: docker compose up etcd

use config_client::ConfigClient;
use serde_json::json;

/// Test that we can load config from etcd
#[tokio::test]
#[ignore] // Run with: cargo test --test etcd_config_test -- --ignored
async fn test_load_config_from_etcd() {
    // Setup: populate etcd with test config
    let client = ConfigClient::with_prefix(
        &["http://localhost:2379"],
        "/air-quality"
    ).await.expect("Failed to connect to etcd");

    // Set test values
    client.set("/server/host", &json!("127.0.0.1")).await.unwrap();
    client.set("/server/port", &json!(9090)).await.unwrap();
    client.set("/mqtt/broker_url", &json!("test-broker")).await.unwrap();
    client.set("/mqtt/port", &json!(1884)).await.unwrap();
    client.set("/storage/base_path", &json!("./test-data")).await.unwrap();

    // Load config using our module
    let config = air_quality_app::load_from_etcd().await
        .expect("Failed to load config");

    assert_eq!(config.server.host, "127.0.0.1");
    assert_eq!(config.server.port, 9090);
    assert_eq!(config.mqtt.broker_url, "test-broker");
    assert_eq!(config.mqtt.port, 1884);
    assert_eq!(config.storage.base_path, "./test-data");

    // Cleanup
    client.delete("/server/host").await.ok();
    client.delete("/server/port").await.ok();
    client.delete("/mqtt/broker_url").await.ok();
    client.delete("/mqtt/port").await.ok();
    client.delete("/storage/base_path").await.ok();
}

/// Test environment variable override
#[tokio::test]
#[ignore]
async fn test_env_override() {
    std::env::set_var("AIR_QUALITY_SERVER_PORT", "7777");

    let client = ConfigClient::with_prefix(
        &["http://localhost:2379"],
        "/air-quality"
    ).await.expect("Failed to connect to etcd");

    // Set etcd value (should be overridden)
    client.set("/server/port", &json!(8080)).await.unwrap();

    // Env var should take precedence
    let port: u16 = client.get_with_env("/server/port", "AIR_QUALITY").await.unwrap();
    assert_eq!(port, 7777);

    std::env::remove_var("AIR_QUALITY_SERVER_PORT");
}

/// Test watch for config changes
#[tokio::test]
#[ignore]
async fn test_watch_config_changes() {
    use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
    use tokio::time::{sleep, Duration};

    let client = ConfigClient::with_prefix(
        &["http://localhost:2379"],
        "/air-quality"
    ).await.expect("Failed to connect to etcd");

    let changed = Arc::new(AtomicBool::new(false));
    let changed_clone = changed.clone();

    // Start watching
    let handle = client.watch("/test", move |_key, _value| {
        changed_clone.store(true, Ordering::SeqCst);
    }).await.unwrap();

    // Give watch time to start
    sleep(Duration::from_millis(100)).await;

    // Make a change
    client.set("/test/value", &json!("changed")).await.unwrap();

    // Wait for notification
    sleep(Duration::from_millis(500)).await;

    assert!(changed.load(Ordering::SeqCst), "Watch callback should have been triggered");

    handle.cancel().await;
    client.delete("/test/value").await.ok();
}
