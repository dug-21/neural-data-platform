//! End-to-End Integration Tests
//!
//! This module tests the complete data flow and system integration:
//! - Data Feeds → Pipeline → Event Bus → DAA → FANN → Results
//! - System health checks and monitoring
//! - Error handling and recovery mechanisms
//! - Performance validation across all components

use anyhow::Result;
use autonomous_platform::config::{
    AlertsConfig, BackupConfig, CircuitBreakerConfig, DatabaseConfig, DevelopmentConfig,
    GracefulShutdownConfig, LoggingConfig, MonitoringConfig, NeuralConfig, ObservabilityConfig,
    PerformanceConfig, PlatformConfig, PlatformInfo, RedisConfig, SecurityConfig,
};
use autonomous_platform::data::{RedisCache, TimeSeriesData, TimescaleDBStorage};
use autonomous_platform::integration::{
    data_access::DataAccessLayer,
    autonomous_decisions::{DaaDecisionMaker, MarketTrend},
    daa_coordinator::{DaaCoordinator, AutonomousDecision},
};
use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;

/// Create a test configuration for end-to-end testing
fn create_test_config() -> PlatformConfig {
    PlatformConfig {
        platform: PlatformInfo {
            name: "end-to-end-test-platform".to_string(),
            version: "0.1.0".to_string(),
            environment: "development".to_string(),
            log_level: "info".to_string(),
        },
        database: DatabaseConfig {
            url: "postgres://test@localhost/end_to_end_test".to_string(),
            max_connections: 20,
            min_connections: 5,
            connection_timeout: 30,
            idle_timeout: 600,
            max_query_time: 30,
        },
        redis: RedisConfig {
            url: "redis://localhost:6379".to_string(),
            max_connections: 10,
            default_ttl_seconds: 300,
            connection_timeout_ms: 5000,
            cluster_mode: false,
            pool_max_idle: 10,
            pool_timeout_seconds: 30,
        },
        neural: NeuralConfig {
            memory_gb: 8.0,
            models: vec!["NHITS".to_string(), "DeepAR".to_string(), "TCN".to_string()],
            prediction_cache_ttl: 600,
            model_load_timeout: 300,
            max_concurrent_predictions: 50,
            enable_model_monitoring: true,
            accuracy_threshold: 0.85,
        },
        monitoring: MonitoringConfig {
            metrics_interval_secs: 30,
            quality_threshold: 0.95,
            prometheus_port: Some(8080),
            prometheus_path: "/metrics".to_string(),
            enable_performance_metrics: true,
            enable_memory_monitoring: true,
            enable_error_monitoring: true,
            cpu_usage_threshold: 80.0,
            memory_usage_threshold: 85.0,
            error_rate_threshold: 0.05,
        },
        observability: ObservabilityConfig::default(),
        security: SecurityConfig::default(),
        performance: PerformanceConfig::default(),
        logging: LoggingConfig::default(),
        alerts: AlertsConfig::default(),
        backup: BackupConfig::default(),
        circuit_breaker: CircuitBreakerConfig::default(),
        graceful_shutdown: GracefulShutdownConfig::default(),
        development: DevelopmentConfig::default(),
    }
}

/// Create test market data for feeds
fn create_test_market_data(symbol: &str, price: f64) -> MarketData {
    MarketData {
        symbol: symbol.to_string(),
        timestamp: Utc::now(),
        price,
        volume: vec![1000.0],
        bid: price - 5.0,
        ask: price + 5.0,
        source: "test_feed".to_string(),
        sequence_number: 12345,
        order_book_depth: Some(10),
        metadata: Some(json!({
            "spread": 10.0,
            "last_trade_size": 0.5,
            "volatility": 0.25,
            "liquidity_score": 0.85
        })),
    }
}

/// Create test news data
fn create_test_news_data(symbol: &str) -> NewsData {
    NewsData {
        id: format!("news_{}", Utc::now().timestamp()),
        timestamp: Utc::now(),
        title: format!("{} Market Analysis Update", symbol),
        content: format!(
            "{} shows strong momentum in current market conditions",
            symbol
        ),
        source: "test_news_feed".to_string(),
        symbols: vec![symbol.to_string()],
        sentiment_score: 0.75,
        relevance_score: 0.85,
        category: "market_analysis".to_string(),
        metadata: Some(json!({
            "author": "Market Analyst Bot",
            "tags": ["cryptocurrency", "analysis", "momentum"]
        })),
    }
}

/// Test Platform Orchestrator Creation and Initialization
#[tokio::test]
async fn test_platform_orchestrator_creation() -> Result<()> {
    let config = create_test_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;

    // Verify orchestrator initialization
    assert!(orchestrator.is_initialized());

    // Check initial health status
    let health = orchestrator.health_check().await?;
    assert!(health.overall_healthy);
    assert!(health.streaming_pipeline_healthy);
    assert!(health.data_pipeline_healthy);
    assert!(health.neural_system_healthy);

    Ok(())
}

/// Test Complete Platform Startup
#[tokio::test]
async fn test_platform_startup() -> Result<()> {
    let config = create_test_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;

    // Start the complete platform
    orchestrator.start_platform().await?;

    // Verify all components are running
    let health = orchestrator.health_check().await?;
    assert!(health.overall_healthy);
    assert!(health.components_started);

    // Verify subscriptions are active
    let subscriptions = orchestrator.get_active_subscriptions().await?;
    assert!(!subscriptions.is_empty());

    Ok(())
}

/// Test Data Flow: Feeds → Pipeline → Event Bus
#[tokio::test]
async fn test_data_ingestion_and_processing() -> Result<()> {
    let config = create_test_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;

    // Send test market data through the pipeline
    let market_data = create_test_market_data("BTC/USD", 45000.0);
    orchestrator.inject_market_data(market_data.clone()).await?;

    // Allow processing time
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify data was processed and stored
    let validation_result = orchestrator.validate_data_flow().await?;
    assert!(validation_result.data_ingested);
    assert!(validation_result.pipeline_processed);
    assert!(validation_result.events_published);

    // Verify data appears in pipeline
    let latest_data = orchestrator.get_latest_market_data("BTC/USD").await?;
    assert!(latest_data.is_some());
    let data = latest_data.unwrap();
    assert_eq!(data.symbol, "BTC/USD");
    assert!((data.close - 45000.0).abs() < 0.01);

    Ok(())
}

/// Test Event Bus → DAA Integration
#[tokio::test]
async fn test_event_bus_daa_integration() -> Result<()> {
    let config = create_test_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;

    // Setup DAA agent to listen for events
    let agent_id = "test_daa_agent";
    orchestrator.register_daa_agent(agent_id).await?;

    // Send market data to trigger events
    let market_data = create_test_market_data("ETH/USD", 3000.0);
    orchestrator.inject_market_data(market_data).await?;

    // Allow event processing time
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Verify DAA agent received events
    let agent_events = orchestrator.get_agent_events(agent_id).await?;
    assert!(!agent_events.is_empty());

    let event = &agent_events[0];
    assert_eq!(event.event_type, "market_data_update");
    assert_eq!(event.symbol, "ETH/USD");

    Ok(())
}

/// Test DAA → FANN Neural Prediction Integration
#[tokio::test]
async fn test_daa_fann_neural_prediction_integration() -> Result<()> {
    let config = create_test_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;

    // Register DAA agent
    let agent_id = "prediction_agent";
    orchestrator.register_daa_agent(agent_id).await?;

    // Send market data
    let market_data = create_test_market_data("BTC/USD", 46000.0);
    orchestrator.inject_market_data(market_data).await?;

    // Trigger DAA decision making
    let decision_context = DecisionContext {
        agent_id: agent_id.to_string(),
        decision_type: "BUY_SIGNAL".to_string(),
        symbol: "BTC/USD".to_string(),
        market_data: create_time_series_data("BTC/USD", 46000.0),
        context_metadata: create_context_metadata(),
        required_confidence: 0.8,
        prediction_horizon: 60,
    };

    let prediction_result = orchestrator.get_neural_prediction(decision_context).await?;

    // Verify FANN integration
    assert!(prediction_result.confidence >= 0.0);
    assert!(prediction_result.model_used.is_some());
    assert!(!prediction_result.prediction_values.is_empty());
    assert!(prediction_result.execution_recommendations.is_some());

    Ok(())
}

/// Test Complete End-to-End Data Flow
#[tokio::test]
async fn test_complete_end_to_end_data_flow() -> Result<()> {
    let config = create_test_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;

    // Test with multiple symbols and data types
    let symbols = vec!["BTC/USD", "ETH/USD", "ADA/USD"];
    let mut agents = Vec::new();

    // Register multiple DAA agents
    for (i, symbol) in symbols.iter().enumerate() {
        let agent_id = format!("agent_{}", i);
        orchestrator.register_daa_agent(&agent_id).await?;
        agents.push(agent_id);
    }

    // Send market data for all symbols
    for (i, symbol) in symbols.iter().enumerate() {
        let market_data = create_test_market_data(symbol, 1000.0 + (i as f64 * 100.0));
        orchestrator.inject_market_data(market_data).await?;

        let news_data = create_test_news_data(symbol);
        orchestrator.inject_news_data(news_data).await?;
    }

    // Allow processing time
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Validate complete data flow
    let validation_result = orchestrator.validate_data_flow().await?;
    assert!(validation_result.data_ingested);
    assert!(validation_result.pipeline_processed);
    assert!(validation_result.events_published);
    assert!(validation_result.agents_responded);
    assert!(validation_result.predictions_generated);

    // Verify each agent received appropriate events
    for agent_id in &agents {
        let events = orchestrator.get_agent_events(agent_id).await?;
        assert!(!events.is_empty());
    }

    // Verify predictions were generated
    let prediction_metrics = orchestrator.get_prediction_metrics().await?;
    assert!(prediction_metrics.total_predictions > 0);
    assert!(prediction_metrics.average_confidence > 0.0);
    assert!(prediction_metrics.models_used.len() > 0);

    Ok(())
}

/// Test System Health Monitoring
#[tokio::test]
async fn test_system_health_monitoring() -> Result<()> {
    let config = create_test_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;

    // Initial health check
    let health = orchestrator.health_check().await?;
    assert!(health.overall_healthy);

    // Generate some load
    for i in 0..10 {
        let market_data = create_test_market_data("LOAD/TEST", 100.0 + i as f64);
        orchestrator.inject_market_data(market_data).await?;
    }

    // Allow processing
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Check health after load
    let health_after_load = orchestrator.health_check().await?;
    assert!(health_after_load.overall_healthy);

    // Verify metrics are being collected
    assert!(health_after_load.metrics.total_requests > 0);
    assert!(health_after_load.metrics.processing_latency_ms > 0.0);
    assert!(health_after_load.metrics.throughput_per_second > 0.0);

    Ok(())
}

/// Test Error Handling and Recovery
#[tokio::test]
async fn test_error_handling_and_recovery() -> Result<()> {
    let config = create_test_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;

    // Inject invalid data to trigger error handling
    let invalid_market_data = MarketData {
        symbol: "".to_string(), // Invalid empty symbol
        timestamp: Utc::now(),
        price: -100.0, // Invalid negative price
        volume: -50.0, // Invalid negative volume
        bid: 0.0,
        ask: 0.0,
        source: "error_test".to_string(),
        sequence_number: 0,
        order_book_depth: None,
        metadata: None,
    };

    // This should fail gracefully
    let result = orchestrator.inject_market_data(invalid_market_data).await;
    assert!(result.is_err());

    // System should remain healthy after error
    let health = orchestrator.health_check().await?;
    assert!(health.overall_healthy);

    // Error metrics should be tracked
    assert!(health.metrics.error_count > 0);

    // System should continue processing valid data
    let valid_data = create_test_market_data("BTC/USD", 45000.0);
    let result = orchestrator.inject_market_data(valid_data).await;
    assert!(result.is_ok());

    Ok(())
}

/// Test Performance Under Load
#[tokio::test]
async fn test_performance_under_load() -> Result<()> {
    let config = create_test_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;

    let start_time = Instant::now();
    let num_messages = 100;

    // Send high volume of market data
    let mut handles = Vec::new();
    for i in 0..num_messages {
        let orchestrator_clone = orchestrator.clone();
        let handle = tokio::spawn(async move {
            let market_data =
                create_test_market_data(&format!("PERF{}/USD", i % 10), 1000.0 + (i as f64));
            orchestrator_clone.inject_market_data(market_data).await
        });
        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        handle.await??;
    }

    let processing_time = start_time.elapsed();

    // Verify performance metrics
    let health = orchestrator.health_check().await?;
    assert!(health.overall_healthy);

    let throughput = num_messages as f64 / processing_time.as_secs_f64();
    assert!(throughput > 10.0); // Should handle at least 10 messages per second

    // Verify all messages were processed
    assert!(health.metrics.total_requests >= num_messages as u64);

    Ok(())
}

/// Test Memory Storage Integration
#[tokio::test]
async fn test_memory_storage_integration() -> Result<()> {
    let config = create_test_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;

    // Process some data
    let market_data = create_test_market_data("BTC/USD", 45000.0);
    orchestrator.inject_market_data(market_data).await?;

    // Register agent and get prediction
    let agent_id = "memory_test_agent";
    orchestrator.register_daa_agent(agent_id).await?;

    let decision_context = DecisionContext {
        agent_id: agent_id.to_string(),
        decision_type: "MEMORY_TEST".to_string(),
        symbol: "BTC/USD".to_string(),
        market_data: create_time_series_data("BTC/USD", 45000.0),
        context_metadata: create_context_metadata(),
        required_confidence: 0.7,
        prediction_horizon: 60,
    };

    let prediction_result = orchestrator.get_neural_prediction(decision_context).await?;

    // Store results in memory
    let memory_key = "swarm-auto-centralized-1751484080479/end-to-end-integration/results";
    orchestrator.store_results_in_memory(memory_key).await?;

    // Verify memory storage
    let memory_data = orchestrator.get_memory_data(memory_key).await?;
    assert!(memory_data.contains_key("system_health"));
    assert!(memory_data.contains_key("validation_results"));
    assert!(memory_data.contains_key("prediction_results"));
    assert!(memory_data.contains_key("performance_metrics"));

    Ok(())
}

/// Test Concurrent Multi-Agent Processing
#[tokio::test]
async fn test_concurrent_multi_agent_processing() -> Result<()> {
    let config = create_test_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;

    let num_agents = 5;
    let mut agent_handles = Vec::new();

    // Spawn multiple agents concurrently
    for i in 0..num_agents {
        let orchestrator_clone = orchestrator.clone();
        let agent_id = format!("concurrent_agent_{}", i);

        let handle = tokio::spawn(async move {
            // Register agent
            orchestrator_clone.register_daa_agent(&agent_id).await?;

            // Process market data
            let market_data = create_test_market_data(
                &format!("CONCURRENT{}/USD", i),
                1000.0 + (i as f64 * 100.0),
            );
            orchestrator_clone.inject_market_data(market_data).await?;

            // Get prediction
            let decision_context = DecisionContext {
                agent_id: agent_id.clone(),
                decision_type: "CONCURRENT_TEST".to_string(),
                symbol: format!("CONCURRENT{}/USD", i),
                market_data: create_time_series_data(
                    &format!("CONCURRENT{}/USD", i),
                    1000.0 + (i as f64 * 100.0),
                ),
                context_metadata: create_context_metadata(),
                required_confidence: 0.7,
                prediction_horizon: 60,
            };

            orchestrator_clone
                .get_neural_prediction(decision_context)
                .await
        });

        agent_handles.push(handle);
    }

    // Wait for all agents to complete
    let mut prediction_results = Vec::new();
    for handle in agent_handles {
        let result = handle.await??;
        prediction_results.push(result);
    }

    // Verify all agents completed successfully
    assert_eq!(prediction_results.len(), num_agents);
    for result in &prediction_results {
        assert!(result.confidence >= 0.0);
        assert!(result.model_used.is_some());
    }

    // Verify system health after concurrent processing
    let health = orchestrator.health_check().await?;
    assert!(health.overall_healthy);
    assert!(health.metrics.total_requests >= num_agents as u64);

    Ok(())
}

// Helper functions for test data creation

fn create_time_series_data(symbol: &str, price: f64) -> TimeSeriesData {
    TimeSeriesData {
        symbol: symbol.to_string(),
        timestamp: Utc::now(),
        open: price - 10.0,
        high: price + 20.0,
        low: price - 20.0,
        close: price,
        volume: vec![1000.0],
        indicators: {
            let mut indicators = HashMap::new();
            indicators.insert("RSI".to_string(), 65.5);
            indicators.insert("MACD".to_string(), 250.0);
            indicators.insert("SMA_20".to_string(), price - 5.0);
            indicators
        },
    }
}

fn create_context_metadata() -> HashMap<String, serde_json::Value> {
    let mut metadata = HashMap::new();
    metadata.insert("strategy".to_string(), json!("end_to_end_test"));
    metadata.insert("risk_level".to_string(), json!(0.02));
    metadata.insert("position_size".to_string(), json!(0.1));
    metadata.insert("max_drawdown".to_string(), json!(0.05));
    metadata.insert("test_mode".to_string(), json!(true));
    metadata
}
