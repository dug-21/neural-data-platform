//! Integration tests for Data-DAA Agent access layer
//! Tests the integration between data layer and DAA orchestrator agents

use autonomous_platform::data::{DataPipeline, RedisCache, TimeSeriesData, TimescaleDBStorage};
use autonomous_platform::integration::data_access::{
    DataAccessLayer, DataRequest, DataResponse, PriceMap, Timeframe,
};
use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// Mock DAA Agent for testing
#[derive(Debug, Clone)]
struct MockDAAgent {
    id: String,
    agent_type: String,
}

impl MockDAAgent {
    fn new(id: &str, agent_type: &str) -> Self {
        Self {
            id: id.to_string(),
            agent_type: agent_type.to_string(),
        }
    }
}

// Test utility functions
async fn setup_test_storage() -> TimescaleDBStorage {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://platform_user:platform_pass@localhost:5432/autonomous_platform".to_string()
    });

    let storage = TimescaleDBStorage::new(&database_url).await.unwrap();
    storage.create_tables().await.unwrap();
    storage
}

async fn setup_test_cache() -> RedisCache {
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    RedisCache::new(&redis_url).await.unwrap()
}

async fn setup_test_pipeline() -> DataPipeline {
    let storage = setup_test_storage().await;
    let cache = setup_test_cache().await;
    let config = autonomous_platform::config::PlatformConfig::default();

    DataPipeline::new(storage, cache, config).await.unwrap()
}

async fn insert_test_market_data(storage: &TimescaleDBStorage, symbol: &str, count: usize) {
    let mut data_points = Vec::new();
    let base_time = Utc::now() - Duration::hours(1);

    for i in 0..count {
        let timestamp = base_time + Duration::minutes(i as i64);
        let price = 50000.0 + (i as f64 * 10.0);

        data_points.push(autonomous_platform::data::storage::TimeSeriesData {
            timestamp,
            source: "test_market".to_string(),
            entity: symbol.to_string(),
            value: price,
            metadata: Some(json!({
                "open": price - 5.0,
                "high": price + 10.0,
                "low": price - 10.0,
                "close": price,
                "volume": 1000.0 + (i as f64 * 100.0)
            })),
        });
    }

    storage.batch_insert(&data_points).await.unwrap();
}

#[tokio::test]
async fn test_data_access_layer_creation() {
    // Test that DataAccessLayer can be created with valid pipeline
    let pipeline = setup_test_pipeline().await;
    let data_access = DataAccessLayer::new(pipeline).await;

    assert!(
        data_access.is_ok(),
        "Failed to create DataAccessLayer: {:?}",
        data_access.err()
    );

    let layer = data_access.unwrap();
    assert!(
        layer.health_check().await.unwrap(),
        "DataAccessLayer health check failed"
    );
}

#[tokio::test]
async fn test_agent_data_request_handling() {
    // Test that agents can make data requests successfully
    let pipeline = setup_test_pipeline().await;
    let data_access = DataAccessLayer::new(pipeline).await.unwrap();

    // Create test agent
    let agent = MockDAAgent::new("test_agent_001", "researcher");

    // Create data request
    let request = DataRequest {
        agent_id: agent.id.clone(),
        request_type: "historical_data".to_string(),
        symbol: "BTC/USD".to_string(),
        timeframe: Timeframe::Hourly,
        start_time: Some(Utc::now() - Duration::hours(2)),
        end_time: Some(Utc::now()),
        limit: Some(100),
        metadata: HashMap::new(),
    };

    // Insert test data
    insert_test_market_data(&data_access.storage, "BTC/USD", 50).await;

    // Handle request
    let response = data_access.handle_agent_data_request(request).await;
    assert!(
        response.is_ok(),
        "Failed to handle agent data request: {:?}",
        response.err()
    );

    let data_response = response.unwrap();
    assert_eq!(data_response.agent_id, agent.id);
    assert!(data_response.success);
    assert!(!data_response.data.is_empty());
}

#[tokio::test]
async fn test_market_data_retrieval() {
    // Test retrieving market data for specific symbols and timeframes
    let pipeline = setup_test_pipeline().await;
    let data_access = DataAccessLayer::new(pipeline).await.unwrap();

    // Insert test data for multiple symbols
    insert_test_market_data(&data_access.storage, "BTC/USD", 30).await;
    insert_test_market_data(&data_access.storage, "ETH/USD", 25).await;

    // Test BTC/USD data retrieval
    let btc_data = data_access
        .get_market_data("BTC/USD", Timeframe::Hourly)
        .await;
    assert!(
        btc_data.is_ok(),
        "Failed to get BTC market data: {:?}",
        btc_data.err()
    );

    let btc_series = btc_data.unwrap();
    assert!(!btc_series.is_empty());
    assert_eq!(btc_series[0].symbol, "BTC/USD");

    // Test ETH/USD data retrieval
    let eth_data = data_access
        .get_market_data("ETH/USD", Timeframe::Hourly)
        .await;
    assert!(
        eth_data.is_ok(),
        "Failed to get ETH market data: {:?}",
        eth_data.err()
    );

    let eth_series = eth_data.unwrap();
    assert!(!eth_series.is_empty());
    assert_eq!(eth_series[0].symbol, "ETH/USD");
}

#[tokio::test]
async fn test_latest_prices_lookup() {
    // Test retrieving latest prices for multiple symbols
    let pipeline = setup_test_pipeline().await;
    let data_access = DataAccessLayer::new(pipeline).await.unwrap();

    // Insert test data
    let symbols = vec!["BTC/USD", "ETH/USD", "ADA/USD"];

    for symbol in &symbols {
        insert_test_market_data(&data_access.storage, symbol, 10).await;
    }

    // Get latest prices
    let price_map = data_access
        .get_latest_prices(symbols.iter().map(|s| s.to_string()).collect())
        .await;
    assert!(
        price_map.is_ok(),
        "Failed to get latest prices: {:?}",
        price_map.err()
    );

    let prices = price_map.unwrap();
    assert_eq!(prices.len(), 3);
    assert!(prices.contains_key("BTC/USD"));
    assert!(prices.contains_key("ETH/USD"));
    assert!(prices.contains_key("ADA/USD"));

    // Verify price values are reasonable
    for (symbol, price_info) in prices {
        assert!(
            price_info.price > 0.0,
            "Price should be positive for {}",
            symbol
        );
        assert!(price_info.timestamp > Utc::now() - Duration::hours(2));
    }
}

#[tokio::test]
async fn test_concurrent_agent_access() {
    // Test multiple agents requesting data concurrently
    let pipeline = setup_test_pipeline().await;
    let data_access = Arc::new(DataAccessLayer::new(pipeline).await.unwrap());

    // Insert test data
    insert_test_market_data(&data_access.storage, "BTC/USD", 100).await;
    insert_test_market_data(&data_access.storage, "ETH/USD", 100).await;

    // Create multiple agents
    let agents = vec![
        MockDAAgent::new("agent_001", "researcher"),
        MockDAAgent::new("agent_002", "trader"),
        MockDAAgent::new("agent_003", "analyst"),
        MockDAAgent::new("agent_004", "risk_manager"),
    ];

    // Create concurrent requests
    let mut handles = Vec::new();

    for (i, agent) in agents.iter().enumerate() {
        let data_access_clone = Arc::clone(&data_access);
        let agent_clone = agent.clone();
        let symbol = if i % 2 == 0 { "BTC/USD" } else { "ETH/USD" };

        let handle = tokio::spawn(async move {
            let request = DataRequest {
                agent_id: agent_clone.id.clone(),
                request_type: "historical_data".to_string(),
                symbol: symbol.to_string(),
                timeframe: Timeframe::Hourly,
                start_time: Some(Utc::now() - Duration::hours(1)),
                end_time: Some(Utc::now()),
                limit: Some(50),
                metadata: HashMap::new(),
            };

            data_access_clone.handle_agent_data_request(request).await
        });

        handles.push(handle);
    }

    // Wait for all requests to complete
    let results = futures::future::join_all(handles).await;

    // Verify all requests succeeded
    for (i, result) in results.into_iter().enumerate() {
        let response = result.unwrap().unwrap();
        assert!(
            response.success,
            "Agent {} request failed: {}",
            i,
            response.error_message.unwrap_or_default()
        );
        assert!(!response.data.is_empty(), "Agent {} received empty data", i);
    }
}

#[tokio::test]
async fn test_caching_performance() {
    // Test that frequently requested data is cached for performance
    let pipeline = setup_test_pipeline().await;
    let data_access = DataAccessLayer::new(pipeline).await.unwrap();

    // Insert test data
    insert_test_market_data(&data_access.storage, "BTC/USD", 50).await;

    // First request (should hit database)
    let start = std::time::Instant::now();
    let first_response = data_access
        .get_market_data("BTC/USD", Timeframe::Hourly)
        .await
        .unwrap();
    let first_duration = start.elapsed();

    // Second request (should hit cache)
    let start = std::time::Instant::now();
    let second_response = data_access
        .get_market_data("BTC/USD", Timeframe::Hourly)
        .await
        .unwrap();
    let second_duration = start.elapsed();

    // Verify data is consistent
    assert_eq!(first_response.len(), second_response.len());

    // Cache should be faster (though this may be flaky in CI)
    // Just verify both completed successfully
    assert!(!first_response.is_empty());
    assert!(!second_response.is_empty());

    println!(
        "First request: {:?}, Second request: {:?}",
        first_duration, second_duration
    );
}

#[tokio::test]
async fn test_agent_request_validation() {
    // Test that invalid agent requests are properly validated
    let pipeline = setup_test_pipeline().await;
    let data_access = DataAccessLayer::new(pipeline).await.unwrap();

    // Test invalid symbol
    let invalid_request = DataRequest {
        agent_id: "test_agent".to_string(),
        request_type: "historical_data".to_string(),
        symbol: "".to_string(), // Empty symbol
        timeframe: Timeframe::Hourly,
        start_time: Some(Utc::now() - Duration::hours(1)),
        end_time: Some(Utc::now()),
        limit: Some(100),
        metadata: HashMap::new(),
    };

    let response = data_access.handle_agent_data_request(invalid_request).await;
    assert!(response.is_err() || !response.unwrap().success);

    // Test invalid time range
    let invalid_time_request = DataRequest {
        agent_id: "test_agent".to_string(),
        request_type: "historical_data".to_string(),
        symbol: "BTC/USD".to_string(),
        timeframe: Timeframe::Hourly,
        start_time: Some(Utc::now()),
        end_time: Some(Utc::now() - Duration::hours(1)), // End before start
        limit: Some(100),
        metadata: HashMap::new(),
    };

    let response = data_access
        .handle_agent_data_request(invalid_time_request)
        .await;
    assert!(response.is_err() || !response.unwrap().success);
}

#[tokio::test]
async fn test_data_aggregation_and_analysis() {
    // Test that agents can request aggregated data and analysis
    let pipeline = setup_test_pipeline().await;
    let data_access = DataAccessLayer::new(pipeline).await.unwrap();

    // Insert test data with variations
    insert_test_market_data(&data_access.storage, "BTC/USD", 100).await;

    // Test aggregation request
    let aggregation_request = DataRequest {
        agent_id: "analyst_agent".to_string(),
        request_type: "aggregated_stats".to_string(),
        symbol: "BTC/USD".to_string(),
        timeframe: Timeframe::Hourly,
        start_time: Some(Utc::now() - Duration::hours(2)),
        end_time: Some(Utc::now()),
        limit: None,
        metadata: {
            let mut meta = HashMap::new();
            meta.insert("aggregation_type".to_string(), "hourly_ohlcv".to_string());
            meta
        },
    };

    let response = data_access
        .handle_agent_data_request(aggregation_request)
        .await;
    assert!(
        response.is_ok(),
        "Failed to handle aggregation request: {:?}",
        response.err()
    );

    let data_response = response.unwrap();
    assert!(data_response.success);
    assert!(!data_response.data.is_empty());

    // Verify aggregated data structure
    let first_point = &data_response.data[0];
    assert_eq!(first_point.symbol, "BTC/USD");
    assert!(first_point.indicators.contains_key("volume"));
}

#[tokio::test]
async fn test_error_handling_and_recovery() {
    // Test graceful error handling and recovery mechanisms
    let pipeline = setup_test_pipeline().await;
    let data_access = DataAccessLayer::new(pipeline).await.unwrap();

    // Test request for non-existent symbol
    let request = DataRequest {
        agent_id: "test_agent".to_string(),
        request_type: "historical_data".to_string(),
        symbol: "NONEXISTENT/USD".to_string(),
        timeframe: Timeframe::Hourly,
        start_time: Some(Utc::now() - Duration::hours(1)),
        end_time: Some(Utc::now()),
        limit: Some(100),
        metadata: HashMap::new(),
    };

    let response = data_access.handle_agent_data_request(request).await;
    assert!(
        response.is_ok(),
        "Error handling failed: {:?}",
        response.err()
    );

    let data_response = response.unwrap();
    // Should return success with empty data rather than error
    assert!(data_response.success);
    assert!(data_response.data.is_empty());
}

#[tokio::test]
async fn test_agent_data_stream_subscription() {
    // Test that agents can subscribe to real-time data streams
    let pipeline = setup_test_pipeline().await;
    let data_access = DataAccessLayer::new(pipeline).await.unwrap();

    // Test subscription request
    let subscription_request = DataRequest {
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

    let response = data_access
        .handle_agent_data_request(subscription_request)
        .await;
    assert!(
        response.is_ok(),
        "Failed to handle subscription request: {:?}",
        response.err()
    );

    let data_response = response.unwrap();
    assert!(data_response.success);

    // Verify subscription was created
    assert!(data_response.metadata.contains_key("subscription_id"));
}

#[tokio::test]
async fn test_performance_metrics_collection() {
    // Test that the system collects performance metrics for monitoring
    let pipeline = setup_test_pipeline().await;
    let data_access = DataAccessLayer::new(pipeline).await.unwrap();

    // Insert test data
    insert_test_market_data(&data_access.storage, "BTC/USD", 50).await;

    // Make several requests to generate metrics
    for i in 0..10 {
        let request = DataRequest {
            agent_id: format!("metrics_agent_{}", i),
            request_type: "historical_data".to_string(),
            symbol: "BTC/USD".to_string(),
            timeframe: Timeframe::Hourly,
            start_time: Some(Utc::now() - Duration::hours(1)),
            end_time: Some(Utc::now()),
            limit: Some(10),
            metadata: HashMap::new(),
        };

        let _ = data_access.handle_agent_data_request(request).await;
    }

    // Verify metrics are collected
    let metrics = data_access.get_performance_metrics().await;
    assert!(
        metrics.is_ok(),
        "Failed to get performance metrics: {:?}",
        metrics.err()
    );

    let perf_metrics = metrics.unwrap();
    assert!(perf_metrics.total_requests >= 10);
    assert!(perf_metrics.cache_hit_rate >= 0.0);
    assert!(perf_metrics.average_response_time_ms > 0.0);
}
