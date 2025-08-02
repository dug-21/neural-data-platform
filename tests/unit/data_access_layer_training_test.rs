//! Unit tests for DataAccessLayer training methods
//! Tests the training-specific functionality including data retrieval and feature engineering support

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json::json;
use mockall::predicate::*;
use mockall::mock;

use autonomous_platform::integration::data_access::{
    DataAccessLayer, DataRequest, DataResponse, Timeframe, 
    TrainingDataRequest, FeatureConfig, PriceInfo, PriceMap
};
use autonomous_platform::data::{TimescaleDBStorage, RedisCache, TimeSeriesData};

// Mock TimescaleDBStorage
mock! {
    pub TimescaleDBStorage {
        pub async fn query_range(
            &self,
            symbol: &str,
            start_time: DateTime<Utc>,
            end_time: DateTime<Utc>,
        ) -> Result<Vec<autonomous_platform::data::storage::TimeSeriesData>>;
        
        pub async fn get_statistics(
            &self,
            entity: &str,
            start_time: DateTime<Utc>,
            end_time: DateTime<Utc>,
            interval: &str,
        ) -> Result<Vec<autonomous_platform::data::storage::AggregatedStats>>;
    }
}

// Mock RedisCache
mock! {
    pub RedisCache {
        pub async fn get<T: serde::de::DeserializeOwned + Send>(&self, key: &str) -> Result<Option<T>>;
        pub async fn set<T: serde::Serialize + Send + Sync>(&self, key: &str, value: &T, ttl: Option<u64>) -> Result<()>;
    }
}

// Helper function to create test TimeSeriesData
fn create_test_time_series_data(symbol: &str, timestamp: DateTime<Utc>, price: f64) -> TimeSeriesData {
    TimeSeriesData {
        symbol: symbol.to_string(),
        timestamp,
        open: price - 5.0,
        high: price + 10.0,
        low: price - 10.0,
        close: price,
        volume: vec![1000.0],
        indicators: HashMap::new(),
        source: Some("test".to_string()),
        entity: Some(symbol.to_string()),
        value: Some(price),
        metadata: Some(json!({
            "open": price - 5.0,
            "high": price + 10.0,
            "low": price - 10.0,
            "close": price,
            "volume": 1000.0,
            "indicators": {}
        })),
    }
}

// Helper function to create storage TimeSeriesData
fn create_storage_time_series_data(
    symbol: &str, 
    timestamp: DateTime<Utc>, 
    price: f64
) -> autonomous_platform::data::storage::TimeSeriesData {
    autonomous_platform::data::storage::TimeSeriesData {
        timestamp,
        source: "test".to_string(),
        entity: symbol.to_string(),
        value: price,
        metadata: Some(json!({
            "open": price - 5.0,
            "high": price + 10.0,
            "low": price - 10.0,
            "close": price,
            "volume": 1000.0,
            "indicators": {}
        })),
    }
}

#[tokio::test]
async fn test_get_market_data_cache_hit() {
    // Test that cached data is returned when available
    let mut mock_storage = MockTimescaleDBStorage::new();
    let mut mock_cache = MockRedisCache::new();
    
    let cache_key = "market_data:BTC/USD:Hourly";
    let cached_data = vec![
        create_test_time_series_data("BTC/USD", Utc::now() - Duration::minutes(30), 50000.0),
        create_test_time_series_data("BTC/USD", Utc::now() - Duration::minutes(20), 50100.0),
        create_test_time_series_data("BTC/USD", Utc::now() - Duration::minutes(10), 50200.0),
    ];
    
    // Mock cache hit
    mock_cache
        .expect_get::<Vec<TimeSeriesData>>()
        .with(eq(cache_key))
        .times(1)
        .returning(move |_| Ok(Some(cached_data.clone())));
    
    // Storage should not be called on cache hit
    mock_storage
        .expect_query_range()
        .times(0);
    
    let data_access = DataAccessLayer::new(
        Arc::new(mock_storage),
        Arc::new(mock_cache),
    ).await.unwrap();
    
    let result = data_access.get_market_data("BTC/USD", Timeframe::Hourly).await;
    assert!(result.is_ok());
    
    let data = result.unwrap();
    assert_eq!(data.len(), 3);
    assert_eq!(data[0].symbol, "BTC/USD");
    assert_eq!(data[0].close, 50000.0);
}

#[tokio::test]
async fn test_get_market_data_cache_miss() {
    // Test that data is fetched from storage on cache miss
    let mut mock_storage = MockTimescaleDBStorage::new();
    let mut mock_cache = MockRedisCache::new();
    
    let cache_key = "market_data:ETH/USD:Daily";
    let end_time = Utc::now();
    let start_time = end_time - Duration::days(30);
    
    let storage_data = vec![
        create_storage_time_series_data("ETH/USD", end_time - Duration::days(2), 3000.0),
        create_storage_time_series_data("ETH/USD", end_time - Duration::days(1), 3100.0),
        create_storage_time_series_data("ETH/USD", end_time, 3200.0),
    ];
    
    // Mock cache miss
    mock_cache
        .expect_get::<Vec<TimeSeriesData>>()
        .with(eq(cache_key))
        .times(1)
        .returning(|_| Ok(None));
    
    // Mock storage query
    mock_storage
        .expect_query_range()
        .with(eq("ETH/USD"), always(), always())
        .times(1)
        .returning(move |_, _, _| Ok(storage_data.clone()));
    
    // Mock cache set
    mock_cache
        .expect_set::<Vec<TimeSeriesData>>()
        .with(eq(cache_key), always(), eq(Some(21600u64))) // 6 hours for daily
        .times(1)
        .returning(|_, _, _| Ok(()));
    
    let data_access = DataAccessLayer::new(
        Arc::new(mock_storage),
        Arc::new(mock_cache),
    ).await.unwrap();
    
    let result = data_access.get_market_data("ETH/USD", Timeframe::Daily).await;
    assert!(result.is_ok());
    
    let data = result.unwrap();
    assert_eq!(data.len(), 3);
    assert_eq!(data[0].symbol, "ETH/USD");
    assert_eq!(data[0].close, 3000.0);
}

#[tokio::test]
async fn test_get_latest_prices_multiple_symbols() {
    // Test retrieving latest prices for multiple symbols
    let mut mock_storage = MockTimescaleDBStorage::new();
    let mut mock_cache = MockRedisCache::new();
    
    let symbols = vec!["BTC/USD".to_string(), "ETH/USD".to_string(), "ADA/USD".to_string()];
    
    // Mock cache responses (mix of hits and misses)
    mock_cache
        .expect_get::<TimeSeriesData>()
        .with(eq("data:BTC/USD:latest"))
        .times(1)
        .returning(|_| Ok(Some(create_test_time_series_data("BTC/USD", Utc::now(), 50000.0))));
    
    mock_cache
        .expect_get::<TimeSeriesData>()
        .with(eq("data:ETH/USD:latest"))
        .times(1)
        .returning(|_| Ok(None)); // Cache miss
    
    mock_cache
        .expect_get::<TimeSeriesData>()
        .with(eq("data:ADA/USD:latest"))
        .times(1)
        .returning(|_| Ok(None)); // Cache miss
    
    // Mock storage queries for cache misses
    let eth_data = vec![create_storage_time_series_data("ETH/USD", Utc::now() - Duration::minutes(5), 3000.0)];
    mock_storage
        .expect_query_range()
        .with(eq("ETH/USD"), always(), always())
        .times(1)
        .returning(move |_, _, _| Ok(eth_data.clone()));
    
    let ada_data = vec![create_storage_time_series_data("ADA/USD", Utc::now() - Duration::minutes(3), 0.5)];
    mock_storage
        .expect_query_range()
        .with(eq("ADA/USD"), always(), always())
        .times(1)
        .returning(move |_, _, _| Ok(ada_data.clone()));
    
    let data_access = DataAccessLayer::new(
        Arc::new(mock_storage),
        Arc::new(mock_cache),
    ).await.unwrap();
    
    let result = data_access.get_latest_prices(symbols).await;
    assert!(result.is_ok());
    
    let price_map = result.unwrap();
    assert_eq!(price_map.len(), 3);
    
    // Verify BTC price (from cache)
    assert!(price_map.contains_key("BTC/USD"));
    assert_eq!(price_map["BTC/USD"].price, 50000.0);
    assert_eq!(price_map["BTC/USD"].source, "cache");
    
    // Verify ETH price (from database)
    assert!(price_map.contains_key("ETH/USD"));
    assert_eq!(price_map["ETH/USD"].price, 3000.0);
    assert_eq!(price_map["ETH/USD"].source, "database");
    
    // Verify ADA price (from database)
    assert!(price_map.contains_key("ADA/USD"));
    assert_eq!(price_map["ADA/USD"].price, 0.5);
    assert_eq!(price_map["ADA/USD"].source, "database");
}

#[tokio::test]
async fn test_handle_historical_data_request() {
    // Test handling historical data requests with time filtering
    let mut mock_storage = MockTimescaleDBStorage::new();
    let mut mock_cache = MockRedisCache::new();
    
    let request = DataRequest {
        agent_id: "test_agent".to_string(),
        request_type: "historical_data".to_string(),
        symbol: "BTC/USD".to_string(),
        timeframe: Timeframe::FiveMinute,
        start_time: Some(Utc::now() - Duration::hours(2)),
        end_time: Some(Utc::now()),
        limit: Some(10),
        metadata: HashMap::new(),
    };
    
    let cache_key = "market_data:BTC/USD:FiveMinute";
    let mut cached_data = Vec::new();
    let now = Utc::now();
    
    // Create 20 data points over 2 hours
    for i in 0..20 {
        let timestamp = now - Duration::minutes(i * 5);
        cached_data.push(create_test_time_series_data("BTC/USD", timestamp, 50000.0 + (i as f64 * 10.0)));
    }
    
    // Mock cache hit
    mock_cache
        .expect_get::<Vec<TimeSeriesData>>()
        .with(eq(cache_key))
        .times(1)
        .returning(move |_| Ok(Some(cached_data.clone())));
    
    let data_access = DataAccessLayer::new(
        Arc::new(mock_storage),
        Arc::new(mock_cache),
    ).await.unwrap();
    
    let result = data_access.handle_agent_data_request(request.clone()).await;
    assert!(result.is_ok());
    
    let response = result.unwrap();
    assert_eq!(response.agent_id, "test_agent");
    assert!(response.success);
    assert!(response.data.len() <= 10); // Limited by request
    assert_eq!(response.data_source, "database");
}

#[tokio::test]
async fn test_handle_aggregated_stats_request() {
    // Test handling aggregated statistics requests
    let mut mock_storage = MockTimescaleDBStorage::new();
    let mut mock_cache = MockRedisCache::new();
    
    let request = DataRequest {
        agent_id: "analyst_agent".to_string(),
        request_type: "aggregated_stats".to_string(),
        symbol: "ETH/USD".to_string(),
        timeframe: Timeframe::Hourly,
        start_time: Some(Utc::now() - Duration::hours(24)),
        end_time: Some(Utc::now()),
        limit: None,
        metadata: HashMap::new(),
    };
    
    let stats = vec![
        autonomous_platform::data::storage::AggregatedStats {
            bucket: Utc::now() - Duration::hours(2),
            entity: "ETH/USD".to_string(),
            avg_value: Some(3000.0),
            min_value: Some(2950.0),
            max_value: Some(3050.0),
            stddev: Some(25.0),
            count: 60,
        },
        autonomous_platform::data::storage::AggregatedStats {
            bucket: Utc::now() - Duration::hours(1),
            entity: "ETH/USD".to_string(),
            avg_value: Some(3100.0),
            min_value: Some(3050.0),
            max_value: Some(3150.0),
            stddev: Some(20.0),
            count: 60,
        },
    ];
    
    mock_storage
        .expect_get_statistics()
        .with(eq("ETH/USD"), always(), always(), eq("1 hour"))
        .times(1)
        .returning(move |_, _, _, _| Ok(stats.clone()));
    
    let data_access = DataAccessLayer::new(
        Arc::new(mock_storage),
        Arc::new(mock_cache),
    ).await.unwrap();
    
    let result = data_access.handle_agent_data_request(request).await;
    assert!(result.is_ok());
    
    let response = result.unwrap();
    assert_eq!(response.agent_id, "analyst_agent");
    assert!(response.success);
    assert_eq!(response.data.len(), 2);
    assert_eq!(response.data_source, "database");
    
    // Verify aggregated data structure
    let first_stat = &response.data[0];
    assert!(first_stat.indicators.contains_key("avg"));
    assert!(first_stat.indicators.contains_key("min"));
    assert!(first_stat.indicators.contains_key("max"));
    assert!(first_stat.indicators.contains_key("stddev"));
    assert!(first_stat.indicators.contains_key("count"));
}

#[tokio::test]
async fn test_request_validation() {
    // Test various validation scenarios
    let mock_storage = MockTimescaleDBStorage::new();
    let mock_cache = MockRedisCache::new();
    
    let data_access = DataAccessLayer::new(
        Arc::new(mock_storage),
        Arc::new(mock_cache),
    ).await.unwrap();
    
    // Test empty agent ID
    let invalid_request = DataRequest {
        agent_id: "".to_string(),
        request_type: "historical_data".to_string(),
        symbol: "BTC/USD".to_string(),
        timeframe: Timeframe::Hourly,
        start_time: None,
        end_time: None,
        limit: None,
        metadata: HashMap::new(),
    };
    
    let result = data_access.handle_agent_data_request(invalid_request).await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(!response.success);
    assert!(response.error_message.is_some());
    assert!(response.error_message.unwrap().contains("Agent ID cannot be empty"));
    
    // Test empty symbol
    let invalid_request = DataRequest {
        agent_id: "test_agent".to_string(),
        request_type: "historical_data".to_string(),
        symbol: "".to_string(),
        timeframe: Timeframe::Hourly,
        start_time: None,
        end_time: None,
        limit: None,
        metadata: HashMap::new(),
    };
    
    let result = data_access.handle_agent_data_request(invalid_request).await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(!response.success);
    assert!(response.error_message.unwrap().contains("Symbol cannot be empty"));
    
    // Test invalid time range
    let invalid_request = DataRequest {
        agent_id: "test_agent".to_string(),
        request_type: "historical_data".to_string(),
        symbol: "BTC/USD".to_string(),
        timeframe: Timeframe::Hourly,
        start_time: Some(Utc::now()),
        end_time: Some(Utc::now() - Duration::hours(1)), // End before start
        limit: None,
        metadata: HashMap::new(),
    };
    
    let result = data_access.handle_agent_data_request(invalid_request).await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(!response.success);
    assert!(response.error_message.unwrap().contains("Start time must be before end time"));
}

#[tokio::test]
async fn test_subscription_management() {
    // Test subscription and unsubscription flow
    let mock_storage = MockTimescaleDBStorage::new();
    let mock_cache = MockRedisCache::new();
    
    let data_access = DataAccessLayer::new(
        Arc::new(mock_storage),
        Arc::new(mock_cache),
    ).await.unwrap();
    
    // Test subscription request
    let sub_request = DataRequest {
        agent_id: "streaming_agent".to_string(),
        request_type: "subscribe_stream".to_string(),
        symbol: "BTC/USD".to_string(),
        timeframe: Timeframe::Minute,
        start_time: None,
        end_time: None,
        limit: None,
        metadata: {
            let mut meta = HashMap::new();
            meta.insert("stream_type".to_string(), "price_updates".to_string());
            meta
        },
    };
    
    let sub_result = data_access.handle_agent_data_request(sub_request).await;
    assert!(sub_result.is_ok());
    
    let sub_response = sub_result.unwrap();
    assert!(sub_response.success);
    assert!(sub_response.metadata.contains_key("subscription_id"));
    assert_eq!(sub_response.metadata["status"], "active");
    
    let subscription_id = sub_response.metadata["subscription_id"].clone();
    
    // Test unsubscription request
    let unsub_request = DataRequest {
        agent_id: "streaming_agent".to_string(),
        request_type: "unsubscribe_stream".to_string(),
        symbol: "BTC/USD".to_string(),
        timeframe: Timeframe::Minute,
        start_time: None,
        end_time: None,
        limit: None,
        metadata: {
            let mut meta = HashMap::new();
            meta.insert("subscription_id".to_string(), subscription_id);
            meta
        },
    };
    
    let unsub_result = data_access.handle_agent_data_request(unsub_request).await;
    assert!(unsub_result.is_ok());
    
    let unsub_response = unsub_result.unwrap();
    assert!(unsub_response.success);
    assert_eq!(unsub_response.metadata["status"], "removed");
}

#[tokio::test]
async fn test_performance_metrics() {
    // Test performance metrics collection
    let mut mock_storage = MockTimescaleDBStorage::new();
    let mut mock_cache = MockRedisCache::new();
    
    // Set up expectations for multiple requests
    mock_cache
        .expect_get::<Vec<TimeSeriesData>>()
        .times(5)
        .returning(|_| Ok(None)); // All cache misses
    
    let storage_data = vec![create_storage_time_series_data("BTC/USD", Utc::now(), 50000.0)];
    mock_storage
        .expect_query_range()
        .times(5)
        .returning(move |_, _, _| Ok(storage_data.clone()));
    
    mock_cache
        .expect_set::<Vec<TimeSeriesData>>()
        .times(5)
        .returning(|_, _, _| Ok(()));
    
    let data_access = DataAccessLayer::new(
        Arc::new(mock_storage),
        Arc::new(mock_cache),
    ).await.unwrap();
    
    // Make several requests
    for i in 0..5 {
        let request = DataRequest {
            agent_id: format!("agent_{}", i),
            request_type: "historical_data".to_string(),
            symbol: "BTC/USD".to_string(),
            timeframe: Timeframe::Hourly,
            start_time: None,
            end_time: None,
            limit: Some(10),
            metadata: HashMap::new(),
        };
        
        let _ = data_access.handle_agent_data_request(request).await;
    }
    
    // Get performance metrics
    let metrics = data_access.get_performance_metrics().await;
    assert!(metrics.is_ok());
    
    let perf = metrics.unwrap();
    assert_eq!(perf.total_requests, 5);
    assert_eq!(perf.success_rate, 1.0);
    assert_eq!(perf.cache_hit_rate, 0.0); // All were misses
    assert!(perf.average_response_time_ms >= 0.0);
    assert!(perf.active_agent_count >= 5);
}

#[tokio::test]
async fn test_concurrent_access() {
    // Test thread-safe concurrent access
    let mock_storage = MockTimescaleDBStorage::new();
    let mut mock_cache = MockRedisCache::new();
    
    // Set up cache to handle concurrent reads
    mock_cache
        .expect_get::<Vec<TimeSeriesData>>()
        .times(10)
        .returning(|_| Ok(Some(vec![create_test_time_series_data("BTC/USD", Utc::now(), 50000.0)])));
    
    let data_access = Arc::new(DataAccessLayer::new(
        Arc::new(mock_storage),
        Arc::new(mock_cache),
    ).await.unwrap());
    
    let mut handles = Vec::new();
    
    // Spawn 10 concurrent tasks
    for i in 0..10 {
        let data_access_clone = Arc::clone(&data_access);
        let handle = tokio::spawn(async move {
            let request = DataRequest {
                agent_id: format!("concurrent_agent_{}", i),
                request_type: "historical_data".to_string(),
                symbol: "BTC/USD".to_string(),
                timeframe: Timeframe::Hourly,
                start_time: None,
                end_time: None,
                limit: Some(10),
                metadata: HashMap::new(),
            };
            
            data_access_clone.handle_agent_data_request(request).await
        });
        
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    let results = futures::future::join_all(handles).await;
    
    // Verify all requests succeeded
    for (i, result) in results.into_iter().enumerate() {
        assert!(result.is_ok(), "Task {} panicked", i);
        let response = result.unwrap();
        assert!(response.is_ok(), "Request {} failed", i);
        assert!(response.unwrap().success, "Response {} was not successful", i);
    }
}

#[tokio::test]
async fn test_training_data_request_structure() {
    // Test the TrainingDataRequest structure and feature configuration
    let training_request = TrainingDataRequest {
        symbol: "BTC/USD".to_string(),
        start_date: Utc::now() - Duration::days(30),
        end_date: Utc::now(),
        granularity: Timeframe::Hourly,
        features: vec![
            "open".to_string(),
            "high".to_string(),
            "low".to_string(),
            "close".to_string(),
            "volume".to_string(),
            "rsi".to_string(),
            "macd".to_string(),
        ],
        include_indicators: true,
    };
    
    // Verify structure
    assert_eq!(training_request.symbol, "BTC/USD");
    assert_eq!(training_request.features.len(), 7);
    assert!(training_request.include_indicators);
    
    // Test FeatureConfig
    let feature_config = FeatureConfig {
        price_features: vec!["open".to_string(), "high".to_string(), "low".to_string(), "close".to_string()],
        technical_indicators: vec!["rsi".to_string(), "macd".to_string(), "bb".to_string()],
        lookback_window: 50,
        normalize: true,
    };
    
    assert_eq!(feature_config.price_features.len(), 4);
    assert_eq!(feature_config.technical_indicators.len(), 3);
    assert_eq!(feature_config.lookback_window, 50);
    assert!(feature_config.normalize);
}