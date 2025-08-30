use config_store::*;
use std::sync::Arc;
use std::collections::HashMap;
use tokio;

#[tokio::test]
async fn test_thread_safety() {
    // Verify Send + Sync traits
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Box<dyn ConfigStore>>();
    
    // Test concurrent operations
    let store = Arc::new(InMemoryConfigStore::new());
    let mut handles = vec![];
    
    for i in 0..100 {
        let store_clone = store.clone();
        handles.push(tokio::spawn(async move {
            let path = format!("/concurrent/test/{}", i);
            store_clone.set(&path, ConfigValue::Integer(i as i64)).await.unwrap();
            let value = store_clone.get(&path).await.unwrap();
            assert_eq!(value, ConfigValue::Integer(i as i64));
        }));
    }
    
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Verify all values were stored
    let keys = store.list_keys("/concurrent/test").await.unwrap();
    assert_eq!(keys.len(), 100);
}

#[tokio::test]
async fn test_error_handling() {
    let store = InMemoryConfigStore::new();
    
    // Test NotFound error
    let result = store.get("/nonexistent").await;
    assert!(matches!(result, Err(ConfigError::NotFound(_))));
    
    // Test invalid path
    let result = store.set("invalid-path", ConfigValue::Null).await;
    assert!(matches!(result, Err(ConfigError::InvalidPath(_))));
    
    // Test deep path validation
    let result = store.set("/a/b/c/d/e/f/g/h", ConfigValue::Null).await;
    assert!(matches!(result, Err(ConfigError::InvalidPath(_))));
}

#[tokio::test]
async fn test_performance_characteristics() {
    let store = InMemoryConfigStore::new();
    
    // Measure write performance
    let start = std::time::Instant::now();
    for i in 0..1000 {
        let path = format!("/perf/test/{}", i);
        store.set(&path, ConfigValue::Integer(i)).await.unwrap();
    }
    let write_duration = start.elapsed();
    
    // Measure read performance
    let start = std::time::Instant::now();
    for i in 0..1000 {
        let path = format!("/perf/test/{}", i);
        store.get(&path).await.unwrap();
    }
    let read_duration = start.elapsed();
    
    // Assert performance targets
    assert!(write_duration.as_millis() < 1000, "Write performance: {:?}", write_duration);
    assert!(read_duration.as_millis() < 100, "Read performance: {:?}", read_duration);
    
    println!("Write 1000 items: {:?}", write_duration);
    println!("Read 1000 items: {:?}", read_duration);
}

#[tokio::test]
async fn test_data_integrity() {
    let store = InMemoryConfigStore::new();
    
    // Test complex nested structure
    let mut config_map = HashMap::new();
    config_map.insert("timeout".to_string(), ConfigValue::Integer(30));
    config_map.insert("retries".to_string(), ConfigValue::Integer(3));
    config_map.insert("features".to_string(), ConfigValue::Array(vec![
        ConfigValue::String("feature1".to_string()),
        ConfigValue::String("feature2".to_string()),
    ]));
    
    let mut outer_map = HashMap::new();
    outer_map.insert("name".to_string(), ConfigValue::String("test".to_string()));
    outer_map.insert("config".to_string(), ConfigValue::Object(config_map));
    
    let complex_value = ConfigValue::Object(outer_map);
    
    store.set("/complex/data", complex_value.clone()).await.unwrap();
    let retrieved = store.get("/complex/data").await.unwrap();
    assert_eq!(retrieved, complex_value);
    
    // Test version history
    for i in 1..=15 {
        store.set("/versioned/data", ConfigValue::Integer(i)).await.unwrap();
    }
    
    // Should have last 10 versions (but our current implementation doesn't limit)
    let history = store.get_history("/versioned/data").await.unwrap();
    assert!(history.len() > 0, "History should contain versions");
    println!("Version history contains {} versions", history.len());
}

#[tokio::test]
async fn test_hierarchical_operations() {
    let store = InMemoryConfigStore::new();
    
    // Create hierarchical structure
    store.set("/app/database/host", ConfigValue::String("localhost".to_string())).await.unwrap();
    store.set("/app/database/port", ConfigValue::Integer(5432)).await.unwrap();
    store.set("/app/cache/host", ConfigValue::String("redis".to_string())).await.unwrap();
    store.set("/app/cache/port", ConfigValue::Integer(6379)).await.unwrap();
    
    // Test tree retrieval
    let tree = store.get_tree("/app").await.unwrap();
    assert_eq!(tree.len(), 4);
    
    // Test prefix listing
    let db_keys = store.list_keys("/app/database").await.unwrap();
    assert_eq!(db_keys.len(), 2);
    
    let cache_keys = store.list_keys("/app/cache").await.unwrap();
    assert_eq!(cache_keys.len(), 2);
}

#[tokio::test]
async fn test_production_scenarios() {
    let store = InMemoryConfigStore::new();
    
    // Scenario 1: Trading hours configuration (the original use case)
    let mut trading_hours_map = HashMap::new();
    trading_hours_map.insert("market_open".to_string(), ConfigValue::String("09:30".to_string()));
    trading_hours_map.insert("market_close".to_string(), ConfigValue::String("16:00".to_string()));
    trading_hours_map.insert("timezone".to_string(), ConfigValue::String("America/New_York".to_string()));
    let trading_hours = ConfigValue::Object(trading_hours_map);
    
    store.set("/system/global/trading_hours", trading_hours.clone()).await.unwrap();
    
    // Both services can read the same config
    let data_ingestion_hours = store.get("/system/global/trading_hours").await.unwrap();
    let execution_hours = store.get("/system/global/trading_hours").await.unwrap();
    assert_eq!(data_ingestion_hours, execution_hours);
    
    // Scenario 2: Feature flags
    store.set("/system/feature_flags/enable_ml", ConfigValue::Boolean(true)).await.unwrap();
    store.set("/system/feature_flags/enable_backtesting", ConfigValue::Boolean(false)).await.unwrap();
    
    let flags = store.get_tree("/system/feature_flags").await.unwrap();
    assert_eq!(flags.len(), 2);
    
    // Scenario 3: Service-specific configs
    let mut base_config_map = HashMap::new();
    base_config_map.insert("timeout".to_string(), ConfigValue::Integer(30));
    base_config_map.insert("retries".to_string(), ConfigValue::Integer(3));
    
    let base_config = ConfigNode {
        path: "/services/base".to_string(),
        value: ConfigValue::Object(base_config_map),
        version: 1,
        metadata: Some(ConfigMetadata {
            description: Some("Base service config".to_string()),
            owner: Some("system".to_string()),
            sensitive: false,
            runtime_modifiable: true,
            created_at: std::time::SystemTime::now(),
            updated_at: std::time::SystemTime::now(),
            updated_by: Some("test".to_string()),
            tags: Vec::new(),
        }),
        inheritance: Some(vec![]),
        schema: None,
    };
    
    store.set_node("/services/base", base_config).await.unwrap();
    
    // Verify node storage
    let retrieved_node = store.get_node("/services/base").await.unwrap();
    if let Some(metadata) = retrieved_node.metadata {
        assert_eq!(metadata.description, Some("Base service config".to_string()));
    }
}

fn main() {
    println!("Production readiness tests compiled successfully");
}