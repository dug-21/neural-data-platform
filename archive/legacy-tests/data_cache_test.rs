use anyhow::Result;
use serde::{Deserialize, Serialize};
use serial_test::serial;
use std::time::Duration;
use tokio::time::sleep;

// Import the cache module we'll implement
use autonomous_platform::data::{PredictionResult, RedisCache};

// Test data structures
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestData {
    id: u64,
    value: String,
}

// Helper function to get Redis URL for testing
fn get_test_redis_url() -> String {
    std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
}

// Helper function to create test cache instance
async fn create_test_cache() -> Result<RedisCache> {
    RedisCache::new(&get_test_redis_url()).await
}

#[tokio::test]
#[serial]
async fn test_redis_connection() {
    // Test that we can successfully connect to Redis
    let cache_result = create_test_cache().await;
    assert!(
        cache_result.is_ok(),
        "Failed to connect to Redis: {:?}",
        cache_result.err()
    );
}

#[tokio::test]
#[serial]
async fn test_set_and_get_operations() {
    let cache = create_test_cache().await.expect("Failed to create cache");

    // Test basic set/get with a test object
    let test_data = TestData {
        id: 1,
        value: "test_value".to_string(),
    };

    // Set the value
    let set_result = cache.set("test_key", &test_data, None).await;
    assert!(
        set_result.is_ok(),
        "Failed to set value: {:?}",
        set_result.err()
    );

    // Get the value back
    let get_result: Result<Option<TestData>> = cache.get("test_key").await;
    assert!(
        get_result.is_ok(),
        "Failed to get value: {:?}",
        get_result.err()
    );

    let retrieved = get_result.unwrap();
    assert!(retrieved.is_some(), "Value should exist");
    assert_eq!(
        retrieved.unwrap(),
        test_data,
        "Retrieved value doesn't match"
    );

    // Clean up
    let _ = cache.invalidate("test_key").await;
}

#[tokio::test]
#[serial]
async fn test_get_nonexistent_key() {
    let cache = create_test_cache().await.expect("Failed to create cache");

    // Try to get a non-existent key
    let get_result: Result<Option<TestData>> = cache.get("nonexistent_key").await;
    assert!(
        get_result.is_ok(),
        "Get should not fail for non-existent key"
    );
    assert!(
        get_result.unwrap().is_none(),
        "Should return None for non-existent key"
    );
}

#[tokio::test]
#[serial]
async fn test_ttl_expiration() {
    let cache = create_test_cache().await.expect("Failed to create cache");

    let test_data = TestData {
        id: 2,
        value: "expiring_value".to_string(),
    };

    // Set value with 2 second TTL
    let set_result = cache.set("ttl_test_key", &test_data, Some(2)).await;
    assert!(set_result.is_ok(), "Failed to set value with TTL");

    // Value should exist immediately
    let get_result: Result<Option<TestData>> = cache.get("ttl_test_key").await;
    assert!(
        get_result.is_ok() && get_result.unwrap().is_some(),
        "Value should exist immediately"
    );

    // Wait for TTL to expire
    sleep(Duration::from_secs(3)).await;

    // Value should no longer exist
    let get_result_after: Result<Option<TestData>> = cache.get("ttl_test_key").await;
    assert!(get_result_after.is_ok(), "Get should succeed");
    assert!(
        get_result_after.unwrap().is_none(),
        "Value should have expired"
    );
}

#[tokio::test]
#[serial]
async fn test_cache_invalidation() {
    let cache = create_test_cache().await.expect("Failed to create cache");

    let test_data = TestData {
        id: 3,
        value: "to_be_invalidated".to_string(),
    };

    // Set a value
    cache
        .set("invalidate_test_key", &test_data, None)
        .await
        .expect("Failed to set value");

    // Verify it exists
    let get_result: Result<Option<TestData>> = cache.get("invalidate_test_key").await;
    assert!(
        get_result.is_ok() && get_result.unwrap().is_some(),
        "Value should exist"
    );

    // Invalidate the key
    let invalidate_result = cache.invalidate("invalidate_test_key").await;
    assert!(invalidate_result.is_ok(), "Failed to invalidate key");

    // Verify it no longer exists
    let get_result_after: Result<Option<TestData>> = cache.get("invalidate_test_key").await;
    assert!(get_result_after.is_ok(), "Get should succeed");
    assert!(
        get_result_after.unwrap().is_none(),
        "Value should be invalidated"
    );
}

#[tokio::test]
#[serial]
async fn test_prediction_caching() {
    let cache = create_test_cache().await.expect("Failed to create cache");

    let prediction = PredictionResult {
        symbol: "BTCUSD".to_string(),
        prediction: 45000.0,
        confidence: 0.85,
        timestamp: chrono::Utc::now().timestamp(),
    };

    // Cache the prediction with 60 second TTL
    let set_result = cache
        .set_prediction("prediction:BTCUSD", &prediction, 60)
        .await;
    assert!(
        set_result.is_ok(),
        "Failed to cache prediction: {:?}",
        set_result.err()
    );

    // Retrieve the prediction
    let get_result = cache.get_prediction("prediction:BTCUSD").await;
    assert!(
        get_result.is_ok(),
        "Failed to get prediction: {:?}",
        get_result.err()
    );

    let retrieved = get_result.unwrap();
    assert!(retrieved.is_some(), "Prediction should exist");

    let cached_prediction = retrieved.unwrap();
    assert_eq!(cached_prediction.symbol, prediction.symbol);
    assert_eq!(cached_prediction.prediction, prediction.prediction);
    assert_eq!(cached_prediction.confidence, prediction.confidence);
    assert_eq!(cached_prediction.timestamp, prediction.timestamp);

    // Clean up
    let _ = cache.invalidate("prediction:BTCUSD").await;
}

#[tokio::test]
#[serial]
async fn test_multiple_concurrent_operations() {
    let cache = create_test_cache().await.expect("Failed to create cache");

    // Test concurrent set operations
    let mut handles = vec![];

    for i in 0..5 {
        let cache_clone = cache.clone();
        let handle = tokio::spawn(async move {
            let test_data = TestData {
                id: i,
                value: format!("concurrent_value_{}", i),
            };
            cache_clone
                .set(&format!("concurrent_key_{}", i), &test_data, Some(10))
                .await
        });
        handles.push(handle);
    }

    // Wait for all operations to complete
    for handle in handles {
        let result = handle.await.expect("Task panicked");
        assert!(result.is_ok(), "Concurrent set failed");
    }

    // Verify all values were set correctly
    for i in 0..5 {
        let get_result: Result<Option<TestData>> =
            cache.get(&format!("concurrent_key_{}", i)).await;
        assert!(get_result.is_ok(), "Failed to get concurrent value");

        let value = get_result.unwrap();
        assert!(value.is_some(), "Concurrent value {} should exist", i);

        let data = value.unwrap();
        assert_eq!(data.id, i as u64);
        assert_eq!(data.value, format!("concurrent_value_{}", i));
    }

    // Clean up
    for i in 0..5 {
        let _ = cache.invalidate(&format!("concurrent_key_{}", i)).await;
    }
}

#[tokio::test]
#[serial]
async fn test_error_handling_invalid_json() {
    let cache = create_test_cache().await.expect("Failed to create cache");

    // Set a string value directly (not as JSON)
    let client = redis::Client::open(get_test_redis_url()).expect("Failed to create Redis client");
    let mut conn = client
        .get_multiplexed_async_connection()
        .await
        .expect("Failed to get connection");

    // Set a raw string that's not valid JSON
    let _: () = redis::cmd("SET")
        .arg("invalid_json_key")
        .arg("not json data")
        .query_async(&mut conn)
        .await
        .expect("Failed to set raw value");

    // Try to get it as TestData - should handle the error gracefully
    let get_result: Result<Option<TestData>> = cache.get("invalid_json_key").await;

    // The operation should either:
    // 1. Return Ok(None) if the implementation treats invalid JSON as missing data
    // 2. Return an Err if the implementation propagates deserialization errors
    // Both are acceptable behaviors
    match get_result {
        Ok(None) => {
            // Implementation treats invalid JSON as missing data
            assert!(true, "Invalid JSON handled as None");
        }
        Err(_) => {
            // Implementation propagates the error
            assert!(true, "Invalid JSON error propagated");
        }
        Ok(Some(_)) => {
            panic!("Should not successfully deserialize invalid JSON");
        }
    }

    // Clean up
    let _: () = redis::cmd("DEL")
        .arg("invalid_json_key")
        .query_async(&mut conn)
        .await
        .expect("Failed to delete key");
}
