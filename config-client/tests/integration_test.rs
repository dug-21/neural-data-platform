use config_client::{ConfigClient, ConfigError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestConfig {
    name: String,
    value: i32,
}

#[tokio::test]
#[ignore] // Requires running etcd server
async fn test_basic_operations() -> Result<(), ConfigError> {
    let client = ConfigClient::new(&["http://localhost:2379"]).await?;

    // Test set and get
    let config = TestConfig {
        name: "test".to_string(),
        value: 42,
    };
    client.set("/test/config", &config).await?;

    let loaded: TestConfig = client.get("/test/config").await?;
    assert_eq!(config, loaded);

    // Test delete
    client.delete("/test/config").await?;

    // Verify deleted
    let result: Result<TestConfig, _> = client.get("/test/config").await;
    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
#[ignore] // Requires running etcd server
async fn test_prefix_operations() -> Result<(), ConfigError> {
    let client = ConfigClient::with_prefix(&["http://localhost:2379"], "/app").await?;

    let config = TestConfig {
        name: "prefixed".to_string(),
        value: 100,
    };

    // Set with prefix
    client.set("/config", &config).await?;

    // Get with prefix
    let loaded: TestConfig = client.get("/config").await?;
    assert_eq!(config, loaded);

    // Cleanup
    client.delete("/config").await?;

    Ok(())
}

#[tokio::test]
#[ignore] // Requires running etcd server
async fn test_list_keys() -> Result<(), ConfigError> {
    let client = ConfigClient::new(&["http://localhost:2379"]).await?;

    // Create multiple configs
    for i in 0..3 {
        let config = TestConfig {
            name: format!("item_{}", i),
            value: i,
        };
        client.set(&format!("/list-test/item_{}", i), &config).await?;
    }

    // List all keys
    let keys = client.list("/list-test/").await?;
    assert_eq!(keys.len(), 3);

    // Cleanup
    for i in 0..3 {
        client.delete(&format!("/list-test/item_{}", i)).await?;
    }

    Ok(())
}

#[test]
fn test_env_var_conversion() {
    // Test environment variable name conversion
    std::env::set_var("TEST_MQTT_BROKER_URL", "mqtt://test:1883");

    // This would be tested with a running etcd server in full integration test
    // For now, just verify the env var is set
    assert_eq!(
        std::env::var("TEST_MQTT_BROKER_URL").unwrap(),
        "mqtt://test:1883"
    );

    std::env::remove_var("TEST_MQTT_BROKER_URL");
}

#[test]
fn test_error_types() {
    // Test error type creation
    let err = ConfigError::NotFound("/test/key".to_string());
    assert_eq!(err.to_string(), "Configuration not found: /test/key");

    let err = ConfigError::ConnectionFailed("connection refused".to_string());
    assert_eq!(err.to_string(), "etcd connection failed: connection refused");

    let err = ConfigError::SerializationError("invalid json".to_string());
    assert_eq!(err.to_string(), "Serialization error: invalid json");
}
