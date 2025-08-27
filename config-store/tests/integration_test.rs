use config_store::stores::SecureInMemoryConfigStore;
use config_store::traits::ConfigStore;
use config_store::{ConfigValue, ConfigError};
use std::collections::HashMap;

#[tokio::test]
async fn test_blocks_password_storage() {
    let store = SecureInMemoryConfigStore::new();
    
    // Should reject password storage
    let result = store.set("/password", ConfigValue::String("mysecret".to_string())).await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    println!("Error message: {}", err_msg);
    assert!(err_msg.contains("cannot be stored") || err_msg.contains("password"));
}

#[tokio::test]
async fn test_blocks_api_key_storage() {
    let store = SecureInMemoryConfigStore::new();
    
    // Should reject API key storage
    let result = store.set("/stripe_api_key", ConfigValue::String("sk_live_123456".to_string())).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_blocks_nested_secrets() {
    let store = SecureInMemoryConfigStore::new();
    
    // Create nested object with secret
    let mut config = HashMap::new();
    config.insert("host".to_string(), ConfigValue::String("localhost".to_string()));
    config.insert("password".to_string(), ConfigValue::String("secret123".to_string()));
    
    let result = store.set("/database", ConfigValue::Object(config)).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_allows_normal_config() {
    let store = SecureInMemoryConfigStore::new();
    
    // Should allow normal configuration
    let result = store.set("/host", ConfigValue::String("localhost".to_string())).await;
    assert!(result.is_ok());
    
    // Should be able to retrieve it
    let value = store.get("/host").await;
    assert!(value.is_ok());
    match value.unwrap() {
        ConfigValue::String(s) => assert_eq!(s, "localhost"),
        _ => panic!("Expected string value"),
    }
}

#[tokio::test]
async fn test_validates_path_format() {
    let store = SecureInMemoryConfigStore::new();
    
    // Invalid path formats should be rejected
    let result = store.set("no_leading_slash", ConfigValue::String("value".to_string())).await;
    assert!(result.is_err());
    
    let result = store.set("/../etc/passwd", ConfigValue::String("value".to_string())).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_validates_injection_attempts() {
    let store = SecureInMemoryConfigStore::new();
    
    // SQL injection attempts should be blocked
    let result = store.set("/test'; DROP TABLE users; --", ConfigValue::String("value".to_string())).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_version_history() {
    let store = SecureInMemoryConfigStore::new();
    
    // Set initial value
    store.set("/config/value", ConfigValue::Integer(1)).await.unwrap();
    
    // Update value multiple times
    store.set("/config/value", ConfigValue::Integer(2)).await.unwrap();
    store.set("/config/value", ConfigValue::Integer(3)).await.unwrap();
    
    // Check history exists  
    let history = store.get_history("/config/value").await.unwrap();
    assert!(history.len() >= 1, "History should have at least 1 version, got {}", history.len());
    
    // Current value should be 3
    let current_value = store.get("/config/value").await.unwrap();
    match current_value {
        ConfigValue::Integer(n) => assert_eq!(n, 3),
        _ => panic!("Expected integer value"),
    }
}

#[tokio::test]
async fn test_safe_json_parsing() {
    use config_store::security::SafeJsonParser;
    
    let parser = SafeJsonParser::new();
    
    // Normal JSON should work
    let valid_json = r#"{"name": "test", "value": 123}"#;
    let result = parser.parse(valid_json);
    assert!(result.is_ok());
    
    // Oversized JSON should be rejected
    let huge_json = format!(r#"{{"data": "{}"}}"#, "x".repeat(11_000_000));
    let result = parser.parse(&huge_json);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("exceeds maximum size"));
}

#[tokio::test]
async fn test_path_traversal_protection() {
    use config_store::security::SecureFileLoader;
    use std::path::PathBuf;
    
    let allowed_dirs = vec![PathBuf::from("/tmp")];
    let loader = SecureFileLoader::new(allowed_dirs);
    
    // Path traversal attempts should be blocked
    let result = loader.load_file("../../etc/passwd");
    assert!(result.is_err());
    
    let result = loader.load_file("/etc/passwd");
    assert!(result.is_err());
}

#[tokio::test]
async fn test_rate_limiting() {
    use config_store::security::RateLimiter;
    use std::time::Duration;
    
    let limiter = RateLimiter::new(3, Duration::from_secs(60));
    
    // First 3 requests should succeed
    for _ in 0..3 {
        assert!(limiter.check("client1").is_ok());
    }
    
    // 4th request should be blocked
    assert!(limiter.check("client1").is_err());
    
    // Different client should still work
    assert!(limiter.check("client2").is_ok());
}

#[tokio::test]
async fn test_error_sanitization_in_production() {
    let store = SecureInMemoryConfigStore::new()
        .with_production_mode();
    
    // Try to access non-existent path
    let result = store.get("/non/existent/path").await;
    assert!(result.is_err());
    
    // Error should be sanitized (no path details)
    let error_msg = result.unwrap_err().to_string();
    assert!(!error_msg.contains("/non/existent/path"));
}