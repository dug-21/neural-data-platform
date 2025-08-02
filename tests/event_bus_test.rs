//! Event Bus Integration Tests
//!
//! Tests for the event bus that connects streaming pipeline to DAA event consumption.
//! Focuses on event format compatibility, reliable delivery, and DAA integration.

use anyhow::Result;
use autonomous_platform::config::PlatformConfig;
use autonomous_platform::data::{DataPipeline, RedisCache, TimescaleDBStorage};
use autonomous_platform::integration::data_access::{DataAccessLayer, DataRequest, Timeframe};
use autonomous_platform::streaming::{
    EventBusIntegration, EventRouter, EventSerializer, MarketEvent, NewsEvent, QualityEvent,
    SystemEvent,
};
use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::timeout;

/// Create test configuration
fn create_test_config() -> PlatformConfig {
    autonomous_platform::config::PlatformConfig {
        platform: autonomous_platform::config::PlatformInfo {
            name: "event-bus-test".to_string(),
            version: "0.1.0".to_string(),
        },
        database: autonomous_platform::config::DatabaseConfig {
            url: "postgres://test@localhost/event_bus_test".to_string(),
            max_connections: 5,
            min_connections: 1,
        },
        redis: autonomous_platform::config::RedisConfig {
            url: "redis://localhost:6379".to_string(),
            max_connections: 3,
            default_ttl_seconds: 300,
        },
        neural: autonomous_platform::config::NeuralConfig {
            memory_gb: 0.5,
            models: vec!["test_model".to_string()],
            prediction_cache_ttl: 300,
        },
        monitoring: autonomous_platform::config::MonitoringConfig {
            metrics_interval_secs: 30,
            quality_threshold: 0.95,
        },
    }
}

/// Create test data pipeline
async fn setup_test_pipeline() -> Result<DataPipeline> {
    let config = create_test_config();
    let storage = TimescaleDBStorage::new(&config.database.url).await?;
    let cache = RedisCache::new(&config.redis.url).await?;
    DataPipeline::new(storage, cache, config).await
}

/// Create test market event
fn create_test_market_event() -> MarketEvent {
    MarketEvent {
        symbol: "BTC/USD".to_string(),
        timestamp: Utc::now(),
        event_type: "price_update".to_string(),
        price: 45000.0,
        volume: vec![100.0],
        bid: 44990.0,
        ask: 45010.0,
        spread: 20.0,
        order_book_depth: Some(10),
        sequence_number: 12345,
        source: "test_exchange".to_string(),
        quality_score: 0.95,
        metadata: Some(json!({
            "volatility": 0.25,
            "trend": "bullish",
            "confidence": 0.85
        })),
    }
}

/// Create test news event
fn create_test_news_event() -> NewsEvent {
    NewsEvent {
        id: "news_001".to_string(),
        timestamp: Utc::now(),
        event_type: "news_update".to_string(),
        title: "Bitcoin Surges on Institutional Adoption".to_string(),
        content: "Major institutions announce Bitcoin allocation strategies...".to_string(),
        source: "financial_news".to_string(),
        category: "market_analysis".to_string(),
        symbols: vec!["BTC".to_string(), "BTCUSD".to_string()],
        sentiment_score: 0.75,
        relevance_score: 0.90,
        quality_score: 0.88,
        language: "en".to_string(),
        author: Some("Market Analyst".to_string()),
        tags: vec!["cryptocurrency".to_string(), "institutional".to_string()],
        metadata: Some(json!({
            "impact_score": 0.8,
            "urgency": "high",
            "region": "global"
        })),
    }
}

/// Create test quality event
fn create_test_quality_event() -> QualityEvent {
    QualityEvent {
        timestamp: Utc::now(),
        event_type: "quality_alert".to_string(),
        source: "data_validator".to_string(),
        severity: "warning".to_string(),
        quality_metric: "data_completeness".to_string(),
        current_value: 0.89,
        threshold_value: 0.95,
        affected_symbols: vec!["BTC/USD".to_string(), "ETH/USD".to_string()],
        description: "Data completeness dropped below threshold".to_string(),
        remediation_actions: vec![
            "Restart data feed".to_string(),
            "Switch to backup source".to_string(),
        ],
        metadata: Some(json!({
            "data_source": "primary_feed",
            "missing_points": 15,
            "recovery_eta": "2 minutes"
        })),
    }
}

/// Create test system event
fn create_test_system_event() -> SystemEvent {
    SystemEvent {
        timestamp: Utc::now(),
        event_type: "system_health".to_string(),
        component: "streaming_pipeline".to_string(),
        status: "healthy".to_string(),
        cpu_usage: 45.2,
        memory_usage: 67.8,
        disk_usage: 23.1,
        network_latency_ms: 15.5,
        throughput_per_second: 1250.0,
        error_rate: 0.001,
        uptime_seconds: 86400,
        health_score: 0.97,
        metadata: Some(json!({
            "version": "1.0.0",
            "instance_id": "pipeline-001",
            "datacenter": "us-east-1"
        })),
    }
}

#[tokio::test]
async fn test_event_bus_creation() -> Result<()> {
    let pipeline = setup_test_pipeline().await?;
    let data_access = DataAccessLayer::new(pipeline).await?;

    let event_bus = EventBusIntegration::new(Arc::new(data_access)).await?;

    // Verify event bus is created successfully
    assert!(event_bus.health_check().await?);

    // Verify DAA integration is working
    assert!(event_bus.daa_integration_health().await?);

    Ok(())
}

#[tokio::test]
async fn test_market_event_publishing() -> Result<()> {
    let pipeline = setup_test_pipeline().await?;
    let data_access = DataAccessLayer::new(pipeline).await?;
    let event_bus = EventBusIntegration::new(Arc::new(data_access)).await?;

    let market_event = create_test_market_event();

    // Publish market event
    event_bus.publish_market_event(market_event.clone()).await?;

    // Verify event was published and routed correctly
    let published_events = event_bus.get_published_events("market").await?;
    assert_eq!(published_events.len(), 1);

    let published_event = &published_events[0];
    assert_eq!(published_event.symbol, market_event.symbol);
    assert_eq!(published_event.price, market_event.price);

    Ok(())
}

#[tokio::test]
async fn test_news_event_publishing() -> Result<()> {
    let pipeline = setup_test_pipeline().await?;
    let data_access = DataAccessLayer::new(pipeline).await?;
    let event_bus = EventBusIntegration::new(Arc::new(data_access)).await?;

    let news_event = create_test_news_event();

    // Publish news event
    event_bus.publish_news_event(news_event.clone()).await?;

    // Verify event was published and routed correctly
    let published_events = event_bus.get_published_events("news").await?;
    assert_eq!(published_events.len(), 1);

    let published_event = &published_events[0];
    assert_eq!(published_event.title, news_event.title);
    assert_eq!(published_event.sentiment_score, news_event.sentiment_score);

    Ok(())
}

#[tokio::test]
async fn test_quality_event_publishing() -> Result<()> {
    let pipeline = setup_test_pipeline().await?;
    let data_access = DataAccessLayer::new(pipeline).await?;
    let event_bus = EventBusIntegration::new(Arc::new(data_access)).await?;

    let quality_event = create_test_quality_event();

    // Publish quality event
    event_bus
        .publish_quality_event(quality_event.clone())
        .await?;

    // Verify event was published and routed correctly
    let published_events = event_bus.get_published_events("quality").await?;
    assert_eq!(published_events.len(), 1);

    let published_event = &published_events[0];
    assert_eq!(published_event.severity, quality_event.severity);
    assert_eq!(published_event.quality_metric, quality_event.quality_metric);

    Ok(())
}

#[tokio::test]
async fn test_system_event_publishing() -> Result<()> {
    let pipeline = setup_test_pipeline().await?;
    let data_access = DataAccessLayer::new(pipeline).await?;
    let event_bus = EventBusIntegration::new(Arc::new(data_access)).await?;

    let system_event = create_test_system_event();

    // Publish system event
    event_bus.publish_system_event(system_event.clone()).await?;

    // Verify event was published and routed correctly
    let published_events = event_bus.get_published_events("system").await?;
    assert_eq!(published_events.len(), 1);

    let published_event = &published_events[0];
    assert_eq!(published_event.component, system_event.component);
    assert_eq!(published_event.health_score, system_event.health_score);

    Ok(())
}

#[tokio::test]
async fn test_event_routing_to_daa_agents() -> Result<()> {
    let pipeline = setup_test_pipeline().await?;
    let data_access = DataAccessLayer::new(pipeline).await?;
    let event_bus = EventBusIntegration::new(Arc::new(data_access)).await?;

    // Create a mock DAA agent receiver
    let (tx, mut rx) = mpsc::channel(100);
    event_bus
        .register_daa_agent("test_agent_001".to_string(), tx)
        .await?;

    // Publish various events
    let market_event = create_test_market_event();
    event_bus.publish_market_event(market_event.clone()).await?;

    let news_event = create_test_news_event();
    event_bus.publish_news_event(news_event.clone()).await?;

    // Route events to DAA agents
    event_bus.route_events_to_daa().await?;

    // Verify agent received events
    let received_events = timeout(tokio::time::Duration::from_millis(1000), async {
        let mut events = Vec::new();
        while let Some(event) = rx.recv().await {
            events.push(event);
            if events.len() >= 2 {
                break;
            }
        }
        events
    })
    .await?;

    assert_eq!(received_events.len(), 2);
    assert!(received_events
        .iter()
        .any(|e| e.event_type == "market_event"));
    assert!(received_events.iter().any(|e| e.event_type == "news_event"));

    Ok(())
}

#[tokio::test]
async fn test_event_serialization_deserialization() -> Result<()> {
    let serializer = EventSerializer::new();

    // Test market event serialization
    let market_event = create_test_market_event();
    let serialized = serializer.serialize_market_event(&market_event)?;
    let deserialized = serializer.deserialize_market_event(&serialized)?;

    assert_eq!(market_event.symbol, deserialized.symbol);
    assert_eq!(market_event.price, deserialized.price);
    assert_eq!(market_event.volume, deserialized.volume);

    // Test news event serialization
    let news_event = create_test_news_event();
    let serialized = serializer.serialize_news_event(&news_event)?;
    let deserialized = serializer.deserialize_news_event(&serialized)?;

    assert_eq!(news_event.title, deserialized.title);
    assert_eq!(news_event.sentiment_score, deserialized.sentiment_score);
    assert_eq!(news_event.symbols, deserialized.symbols);

    Ok(())
}

#[tokio::test]
async fn test_event_filtering_and_routing() -> Result<()> {
    let pipeline = setup_test_pipeline().await?;
    let data_access = DataAccessLayer::new(pipeline).await?;
    let event_bus = EventBusIntegration::new(Arc::new(data_access)).await?;

    // Create event router with filtering rules
    let mut router = EventRouter::new();
    router.add_filter_rule("high_quality_only", "quality_score > 0.9")?;
    router.add_filter_rule("btc_events_only", "symbol == 'BTC/USD'")?;

    // Register router with event bus
    event_bus.set_event_router(router).await?;

    // Create events with different quality scores
    let mut high_quality_event = create_test_market_event();
    high_quality_event.quality_score = 0.95;

    let mut low_quality_event = create_test_market_event();
    low_quality_event.quality_score = 0.85;
    low_quality_event.symbol = "ETH/USD".to_string();

    // Publish events
    event_bus
        .publish_market_event(high_quality_event.clone())
        .await?;
    event_bus
        .publish_market_event(low_quality_event.clone())
        .await?;

    // Route with filtering
    event_bus.route_events_to_daa().await?;

    // Verify only high-quality BTC events were routed
    let routed_events = event_bus.get_routed_events().await?;
    assert_eq!(routed_events.len(), 1);
    assert_eq!(routed_events[0].symbol, "BTC/USD");
    assert!(routed_events[0].quality_score > 0.9);

    Ok(())
}

#[tokio::test]
async fn test_batch_event_processing() -> Result<()> {
    let pipeline = setup_test_pipeline().await?;
    let data_access = DataAccessLayer::new(pipeline).await?;
    let event_bus = EventBusIntegration::new(Arc::new(data_access)).await?;

    // Configure batch processing
    event_bus.configure_batch_processing(50, 1000).await?; // 50 events or 1000ms timeout

    // Create batch of market events
    let mut market_events = Vec::new();
    for i in 0..25 {
        let mut event = create_test_market_event();
        event.symbol = format!("TEST{}/USD", i);
        event.price += i as f64 * 10.0;
        market_events.push(event);
    }

    // Publish batch
    event_bus
        .batch_publish_market_events(market_events.clone())
        .await?;

    // Verify batch was processed
    let batch_stats = event_bus.get_batch_processing_stats().await?;
    assert_eq!(batch_stats.total_batches, 1);
    assert_eq!(batch_stats.events_in_last_batch, 25);
    assert!(batch_stats.average_batch_processing_time_ms > 0.0);

    Ok(())
}

#[tokio::test]
async fn test_error_handling_and_retry_logic() -> Result<()> {
    let pipeline = setup_test_pipeline().await?;
    let data_access = DataAccessLayer::new(pipeline).await?;
    let event_bus = EventBusIntegration::new(Arc::new(data_access)).await?;

    // Configure retry settings
    let retry_config = autonomous_platform::streaming::RetryConfig {
        max_attempts: 3,
        base_delay_ms: 100,
        max_delay_ms: 1000,
        backoff_multiplier: 2.0,
    };
    event_bus.configure_retry_logic(retry_config).await?;

    // Create an event that will trigger a failure
    let mut failing_event = create_test_market_event();
    failing_event.symbol = "INVALID/SYMBOL/FORMAT".to_string(); // Invalid format

    // Attempt to publish the failing event
    let result = event_bus.publish_market_event(failing_event).await;

    // Should eventually succeed or exhaust retries
    let error_stats = event_bus.get_error_stats().await?;
    assert!(error_stats.total_failed_events > 0);
    assert!(error_stats.total_retry_attempts > 0);
    assert!(error_stats.retry_attempts <= 3); // Max attempts

    Ok(())
}

#[tokio::test]
async fn test_daa_event_format_compatibility() -> Result<()> {
    let pipeline = setup_test_pipeline().await?;
    let data_access = DataAccessLayer::new(pipeline).await?;
    let event_bus = EventBusIntegration::new(Arc::new(data_access)).await?;

    // Create market event and convert to DAA format
    let market_event = create_test_market_event();
    let daa_event = event_bus.convert_to_daa_format(&market_event).await?;

    // Verify DAA event format
    assert_eq!(daa_event.event_type, "market_update");
    assert!(daa_event.payload.contains_key("symbol"));
    assert!(daa_event.payload.contains_key("price"));
    assert!(daa_event.payload.contains_key("timestamp"));
    assert!(daa_event.payload.contains_key("quality_score"));

    // Verify compatibility with existing DAA agent expectations
    let data_request = DataRequest {
        agent_id: "test_daa_agent".to_string(),
        request_type: "process_event".to_string(),
        symbol: market_event.symbol.clone(),
        timeframe: Timeframe::Minute,
        start_time: None,
        end_time: None,
        limit: None,
        metadata: {
            let mut meta = HashMap::new();
            meta.insert("event_id".to_string(), daa_event.id.clone());
            meta
        },
    };

    // This should work with existing DAA infrastructure
    let response = data_access.handle_agent_data_request(data_request).await?;
    assert!(response.success);

    Ok(())
}

#[tokio::test]
async fn test_concurrent_event_publishing() -> Result<()> {
    let pipeline = setup_test_pipeline().await?;
    let data_access = DataAccessLayer::new(pipeline).await?;
    let event_bus = Arc::new(EventBusIntegration::new(Arc::new(data_access)).await?);

    // Publish events concurrently from multiple tasks
    let mut handles = Vec::new();

    for i in 0..10 {
        let event_bus_clone = Arc::clone(&event_bus);
        let handle = tokio::spawn(async move {
            let mut market_event = create_test_market_event();
            market_event.symbol = format!("CONCURRENT{}/USD", i);
            market_event.sequence_number = i;

            event_bus_clone.publish_market_event(market_event).await
        });
        handles.push(handle);
    }

    // Wait for all concurrent publishes to complete
    for handle in handles {
        handle.await??;
    }

    // Verify all events were published
    let published_events = event_bus.get_published_events("market").await?;
    assert_eq!(published_events.len(), 10);

    // Verify events have correct sequence numbers
    let mut sequence_numbers: Vec<u64> =
        published_events.iter().map(|e| e.sequence_number).collect();
    sequence_numbers.sort();

    assert_eq!(sequence_numbers, (0..10).collect::<Vec<u64>>());

    Ok(())
}

#[tokio::test]
async fn test_event_bus_performance_metrics() -> Result<()> {
    let pipeline = setup_test_pipeline().await?;
    let data_access = DataAccessLayer::new(pipeline).await?;
    let event_bus = EventBusIntegration::new(Arc::new(data_access)).await?;

    // Enable performance monitoring
    event_bus.enable_performance_monitoring(true).await?;

    // Publish various events to generate metrics
    for _ in 0..100 {
        let market_event = create_test_market_event();
        event_bus.publish_market_event(market_event).await?;
    }

    for _ in 0..50 {
        let news_event = create_test_news_event();
        event_bus.publish_news_event(news_event).await?;
    }

    // Get performance metrics
    let metrics = event_bus.get_performance_metrics().await?;

    assert_eq!(metrics.total_events_published, 150);
    assert!(metrics.events_per_second > 0.0);
    assert!(metrics.average_publish_latency_ms >= 0.0);
    assert!(metrics.success_rate >= 0.95); // Should be high success rate
    assert_eq!(metrics.events_by_type.get("market").unwrap_or(&0), &100);
    assert_eq!(metrics.events_by_type.get("news").unwrap_or(&0), &50);

    Ok(())
}

#[tokio::test]
async fn test_memory_storage_integration() -> Result<()> {
    let pipeline = setup_test_pipeline().await?;
    let data_access = DataAccessLayer::new(pipeline).await?;
    let event_bus = EventBusIntegration::new(Arc::new(data_access)).await?;

    // Publish some events
    let market_event = create_test_market_event();
    event_bus.publish_market_event(market_event.clone()).await?;

    let news_event = create_test_news_event();
    event_bus.publish_news_event(news_event.clone()).await?;

    // Store results in memory for DAA agents
    let memory_key = "swarm-auto-centralized-1751484080479/event-bus-integration/results";
    event_bus.store_results_in_memory(memory_key).await?;

    // Verify memory storage
    let memory_data = event_bus.get_memory_data(memory_key).await?;
    assert!(memory_data.contains_key("total_events_published"));
    assert!(memory_data.contains_key("event_types_processed"));
    assert!(memory_data.contains_key("daa_integration_status"));
    assert!(memory_data.contains_key("performance_metrics"));

    // Verify specific event data
    let event_summary = memory_data
        .get("event_summary")
        .expect("Event summary should exist");
    assert!(event_summary.contains("market_events: 1"));
    assert!(event_summary.contains("news_events: 1"));

    Ok(())
}
