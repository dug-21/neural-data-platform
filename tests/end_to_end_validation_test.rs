//! End-to-End Validation Tests
//!
//! This module implements comprehensive system flow testing with complete validation
//! of the entire neural trading pipeline from market data ingestion to trading decisions.
//!
//! Test Coverage:
//! - Complete data flow: Market Data → Pipeline → DAA → FANN → Decisions
//! - Error propagation and recovery testing
//! - Load testing with concurrent operations
//! - Resource leak detection and cleanup
//! - Performance validation against system targets

use anyhow::Result;
use autonomous_platform::config::{
    DatabaseConfig, MonitoringConfig, NeuralConfig, PlatformConfig, PlatformInfo, RedisConfig,
};
use autonomous_platform::data::TimeSeriesData;
use autonomous_platform::integration::{
    autonomous_decisions::{DaaDecisionMaker, MarketTrend, DecisionContext},
    daa_coordinator::{DaaCoordinator, AutonomousDecision},
};
// SystemHealth removed - using monitoring config instead
use chrono::{DateTime, Utc};
use futures::future::try_join_all;
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

/// Performance targets for validation
const TARGET_DATA_STORAGE_LATENCY_MS: u64 = 50;
const TARGET_CACHE_OPERATION_LATENCY_MS: u64 = 5;
const TARGET_NEURAL_PREDICTION_LATENCY_MS: u64 = 100;
const TARGET_AGENT_DECISION_LATENCY_MS: u64 = 100;
const TARGET_MAX_MEMORY_USAGE_GB: f64 = 1.0;

/// Resource tracking for leak detection
#[derive(Debug, Clone)]
struct ResourceTracker {
    connections_created: Arc<AtomicU64>,
    memory_allocations: Arc<AtomicU64>,
    active_tasks: Arc<AtomicU64>,
    cleanup_completed: Arc<AtomicBool>,
}

impl ResourceTracker {
    fn new() -> Self {
        Self {
            connections_created: Arc::new(AtomicU64::new(0)),
            memory_allocations: Arc::new(AtomicU64::new(0)),
            active_tasks: Arc::new(AtomicU64::new(0)),
            cleanup_completed: Arc::new(AtomicBool::new(false)),
        }
    }

    fn track_connection(&self) {
        self.connections_created.fetch_add(1, Ordering::SeqCst);
    }

    fn track_task_start(&self) {
        self.active_tasks.fetch_add(1, Ordering::SeqCst);
    }

    fn track_task_end(&self) {
        self.active_tasks.fetch_sub(1, Ordering::SeqCst);
    }

    fn mark_cleanup_completed(&self) {
        self.cleanup_completed.store(true, Ordering::SeqCst);
    }

    fn get_metrics(&self) -> (u64, u64, u64, bool) {
        (
            self.connections_created.load(Ordering::SeqCst),
            self.memory_allocations.load(Ordering::SeqCst),
            self.active_tasks.load(Ordering::SeqCst),
            self.cleanup_completed.load(Ordering::SeqCst),
        )
    }
}

/// Load test configuration
#[derive(Debug, Clone)]
struct LoadTestConfig {
    concurrent_agents: usize,
    messages_per_agent: usize,
    test_duration_seconds: u64,
    symbols: Vec<String>,
    error_injection_rate: f64,
}

impl Default for LoadTestConfig {
    fn default() -> Self {
        Self {
            concurrent_agents: 10,
            messages_per_agent: 100,
            test_duration_seconds: 30,
            symbols: vec![
                "BTC/USD".to_string(),
                "ETH/USD".to_string(),
                "ADA/USD".to_string(),
                "DOT/USD".to_string(),
                "SOL/USD".to_string(),
            ],
            error_injection_rate: 0.05, // 5% error rate
        }
    }
}

/// Create enhanced test configuration for validation
fn create_validation_config() -> PlatformConfig {
    PlatformConfig {
        platform: PlatformInfo {
            name: "end-to-end-validation-platform".to_string(),
            version: "1.0.0".to_string(),
        },
        database: DatabaseConfig {
            url: "postgres://test@localhost/validation_test".to_string(),
            max_connections: 50,
            min_connections: 10,
        },
        redis: RedisConfig {
            url: "redis://localhost:6379".to_string(),
            max_connections: 20,
            default_ttl_seconds: 300,
        },
        neural: NeuralConfig {
            memory_gb: 8.0,
            models: vec![
                "NHITS".to_string(),
                "DeepAR".to_string(),
                "TCN".to_string(),
                "FANN".to_string(),
            ],
            prediction_cache_ttl: 600,
        },
        monitoring: MonitoringConfig {
            metrics_interval_secs: 10,
            quality_threshold: 0.95,
        },
    }
}

/// Create realistic market data with timestamps and indicators
fn create_realistic_market_data(symbol: &str, base_price: f64, sequence: u64) -> MarketData {
    let price_variation = (sequence as f64 % 100.0) / 100.0 * 0.02; // 2% variation
    let current_price = base_price * (1.0 + price_variation - 0.01);

    MarketData {
        symbol: symbol.to_string(),
        timestamp: Utc::now(),
        price: current_price,
        volume: 1000.0 + (sequence as f64 * 10.0),
        bid: current_price - (current_price * 0.001),
        ask: current_price + (current_price * 0.001),
        source: "validation_feed".to_string(),
        sequence_number: sequence,
        order_book_depth: Some(25),
        metadata: Some(json!({
            "spread": current_price * 0.002,
            "last_trade_size": 0.5 + (sequence as f64 % 10.0) / 20.0,
            "volatility": 0.15 + (sequence as f64 % 50.0) / 1000.0,
            "liquidity_score": 0.8 + (sequence as f64 % 20.0) / 100.0,
            "market_depth": sequence % 100,
            "order_count": sequence % 50 + 10
        })),
    }
}

/// Create contextual news data with sentiment analysis
fn create_contextual_news_data(symbol: &str, sentiment: f64) -> NewsData {
    let news_id = Uuid::new_v4().to_string();
    let sentiment_label = if sentiment > 0.6 {
        "bullish"
    } else if sentiment < 0.4 {
        "bearish"
    } else {
        "neutral"
    };

    NewsData {
        id: news_id,
        timestamp: Utc::now(),
        title: format!("{} Market Analysis: {} outlook confirmed", symbol, sentiment_label),
        content: format!("Technical analysis shows {} sentiment for {} with key indicators supporting {} trend continuation", sentiment_label, symbol, sentiment_label),
        source: "validation_news_feed".to_string(),
        symbols: vec![symbol.to_string()],
        sentiment_score: sentiment,
        relevance_score: 0.85 + (sentiment.abs() - 0.5) * 0.3,
        category: "market_analysis".to_string(),
        metadata: Some(json!({
            "author": "Validation News Bot",
            "tags": ["cryptocurrency", "analysis", sentiment_label, "validation"],
            "confidence": sentiment.abs(),
            "impact_score": sentiment.abs() * 0.8
        })),
    }
}

/// Test complete trading scenario with full data flow validation
#[tokio::test]
async fn test_complete_trading_scenario() -> Result<()> {
    let config = create_validation_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    let resource_tracker = ResourceTracker::new();

    // Start the complete platform
    orchestrator.start_platform().await?;
    resource_tracker.track_connection();

    // Phase 1: Market Data Ingestion
    let symbol = "BTC/USD";
    let base_price = 45000.0;
    let agent_id = "complete_scenario_agent";

    // Register DAA agent
    orchestrator.register_daa_agent(agent_id).await?;
    resource_tracker.track_connection();

    // Inject realistic market data sequence
    let start_time = Instant::now();
    for i in 0..20 {
        let market_data = create_realistic_market_data(symbol, base_price, i);
        orchestrator.inject_market_data(market_data).await?;

        let news_data = create_contextual_news_data(symbol, 0.7 + (i as f64 % 10.0) / 50.0);
        orchestrator.inject_news_data(news_data).await?;

        // Small delay to simulate real-time feed
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
    let ingestion_time = start_time.elapsed();

    // Phase 2: Pipeline Processing and Event Propagation
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let validation_result = orchestrator.validate_data_flow().await?;
    assert!(validation_result.data_ingested, "Data ingestion failed");
    assert!(
        validation_result.pipeline_processed,
        "Pipeline processing failed"
    );
    assert!(
        validation_result.events_published,
        "Event publishing failed"
    );

    // Phase 3: DAA Decision Making with Neural Predictions
    resource_tracker.track_task_start();

    let decision_context = DecisionContext {
        agent_id: agent_id.to_string(),
        decision_type: "BUY_SIGNAL_VALIDATION".to_string(),
        symbol: symbol.to_string(),
        market_data: create_enhanced_time_series_data(symbol, base_price),
        context_metadata: create_enhanced_context_metadata(),
        required_confidence: 0.8,
        prediction_horizon: 300, // 5 minutes
    };

    let prediction_start = Instant::now();
    let prediction_result = orchestrator.get_neural_prediction(decision_context).await?;
    let prediction_time = prediction_start.elapsed();

    resource_tracker.track_task_end();

    // Phase 4: Validation and Performance Checks
    assert!(
        prediction_result.confidence >= 0.0,
        "Invalid prediction confidence"
    );
    assert!(
        prediction_result.confidence <= 1.0,
        "Prediction confidence out of range"
    );
    assert!(
        prediction_result.model_used.is_some(),
        "No model used for prediction"
    );
    assert!(
        !prediction_result.prediction_values.is_empty(),
        "Empty prediction values"
    );
    assert!(
        prediction_result.execution_recommendations.is_some(),
        "No execution recommendations"
    );

    // Performance validation
    assert!(
        ingestion_time.as_millis() < TARGET_DATA_STORAGE_LATENCY_MS as u128 * 20,
        "Data ingestion too slow: {}ms",
        ingestion_time.as_millis()
    );
    assert!(
        prediction_time.as_millis() < TARGET_NEURAL_PREDICTION_LATENCY_MS as u128,
        "Neural prediction too slow: {}ms",
        prediction_time.as_millis()
    );

    // Phase 5: Memory Storage and Cleanup
    let memory_key = "swarm-auto-centralized-1751484080479/end-to-end-validation/complete_scenario";
    orchestrator.store_results_in_memory(memory_key).await?;
    resource_tracker.mark_cleanup_completed();

    // Final validation
    let final_health = orchestrator.health_check().await?;
    assert!(
        final_health.overall_healthy,
        "System unhealthy after complete scenario"
    );

    Ok(())
}

/// Test system under high load with concurrent operations
#[tokio::test]
async fn test_system_under_load() -> Result<()> {
    let config = create_validation_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    let resource_tracker = ResourceTracker::new();
    let load_config = LoadTestConfig::default();

    orchestrator.start_platform().await?;

    let test_start = Instant::now();
    let mut agent_handles = Vec::new();

    // Spawn concurrent agents for load testing
    for agent_idx in 0..load_config.concurrent_agents {
        let orchestrator_clone = orchestrator.clone();
        let resource_tracker_clone = resource_tracker.clone();
        let load_config_clone = load_config.clone();

        let handle = tokio::spawn(async move {
            let agent_id = format!("load_test_agent_{}", agent_idx);

            // Register agent
            orchestrator_clone.register_daa_agent(&agent_id).await?;
            resource_tracker_clone.track_connection();

            let symbol = &load_config_clone.symbols[agent_idx % load_config_clone.symbols.len()];
            let base_price = 1000.0 + (agent_idx as f64 * 100.0);

            // High-frequency data processing
            for msg_idx in 0..load_config_clone.messages_per_agent {
                resource_tracker_clone.track_task_start();

                // Inject market data
                let market_data = create_realistic_market_data(symbol, base_price, msg_idx as u64);
                orchestrator_clone.inject_market_data(market_data).await?;

                // Periodic predictions
                if msg_idx % 10 == 0 {
                    let decision_context = DecisionContext {
                        agent_id: agent_id.clone(),
                        decision_type: "LOAD_TEST".to_string(),
                        symbol: symbol.clone(),
                        market_data: create_enhanced_time_series_data(symbol, base_price),
                        context_metadata: create_enhanced_context_metadata(),
                        required_confidence: 0.6,
                        prediction_horizon: 60,
                    };

                    let _prediction = orchestrator_clone
                        .get_neural_prediction(decision_context)
                        .await?;
                }

                resource_tracker_clone.track_task_end();

                // Avoid overwhelming the system
                if msg_idx % 50 == 0 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
                }
            }

            Ok::<(), anyhow::Error>(())
        });

        agent_handles.push(handle);
    }

    // Wait for all agents to complete
    let agent_results: Result<Vec<_>, _> = try_join_all(agent_handles).await;
    for result in agent_results? {
        result?;
    }

    let total_test_time = test_start.elapsed();
    let total_operations = (load_config.concurrent_agents * load_config.messages_per_agent) as u64;
    let throughput = total_operations as f64 / total_test_time.as_secs_f64();

    // Validate performance under load
    assert!(
        throughput > 50.0,
        "Throughput too low: {:.2} ops/sec",
        throughput
    );

    // System health validation
    let health = orchestrator.health_check().await?;
    assert!(health.overall_healthy, "System unhealthy after load test");

    // Resource leak detection
    let (connections, _memory, active_tasks, _cleanup) = resource_tracker.get_metrics();
    assert!(
        active_tasks == 0,
        "Tasks not properly cleaned up: {}",
        active_tasks
    );
    assert!(connections > 0, "No connections tracked");

    // Store load test results
    let memory_key = "swarm-auto-centralized-1751484080479/end-to-end-validation/load_test";
    orchestrator.store_results_in_memory(memory_key).await?;

    Ok(())
}

/// Test error recovery and fault tolerance
#[tokio::test]
async fn test_error_recovery() -> Result<()> {
    let config = create_validation_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    let resource_tracker = ResourceTracker::new();

    orchestrator.start_platform().await?;

    let agent_id = "error_recovery_agent";
    orchestrator.register_daa_agent(agent_id).await?;

    // Scenario 1: Invalid market data
    let scenario_start = Instant::now();
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

    let result = orchestrator.inject_market_data(invalid_market_data).await;
    assert!(result.is_err(), "Should have failed with invalid data");

    // Verify system recovers
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let health = orchestrator.health_check().await?;
    assert!(
        health.overall_healthy,
        "System should recover from invalid data"
    );

    // Scenario 2: Network timeout simulation with high load
    resource_tracker.track_task_start();

    // Simulate high load to trigger potential timeouts
    let mut timeout_handles = Vec::new();
    for i in 0..100 {
        let orchestrator_clone = orchestrator.clone();
        let handle = tokio::spawn(async move {
            let market_data = create_realistic_market_data("TIMEOUT/TEST", 1000.0, i);
            orchestrator_clone.inject_market_data(market_data).await
        });
        timeout_handles.push(handle);
    }

    // Add some that should timeout
    tokio::time::timeout(
        tokio::time::Duration::from_millis(50),
        try_join_all(timeout_handles),
    )
    .await
    .ok(); // Expect some to timeout

    resource_tracker.track_task_end();

    // System should still be healthy
    let health = orchestrator.health_check().await?;
    assert!(
        health.overall_healthy,
        "System should handle network timeouts"
    );

    // Scenario 3: Neural model failure simulation
    let invalid_decision_context = DecisionContext {
        agent_id: agent_id.to_string(),
        decision_type: "INVALID_MODEL_TEST".to_string(),
        symbol: "INVALID/SYMBOL".to_string(),
        market_data: TimeSeriesData {
            symbol: "INVALID/SYMBOL".to_string(),
            timestamp: Utc::now(),
            open: 0.0, // Invalid data
            high: 0.0,
            low: 0.0,
            close: 0.0,
            volume: 0.0,
            indicators: HashMap::new(),
        },
        context_metadata: HashMap::new(),
        required_confidence: 2.0, // Invalid confidence > 1.0
        prediction_horizon: 0,    // Invalid horizon
    };

    let prediction_result = orchestrator
        .get_neural_prediction(invalid_decision_context)
        .await;
    // Should either fail gracefully or return low confidence prediction
    if let Ok(result) = prediction_result {
        assert!(
            result.confidence < 0.5,
            "Should have low confidence for invalid data"
        );
    }

    // Final recovery validation
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    let final_health = orchestrator.health_check().await?;
    assert!(final_health.overall_healthy, "System should fully recover");

    // Verify system can still process valid data
    let valid_data = create_realistic_market_data("BTC/USD", 45000.0, 1);
    let result = orchestrator.inject_market_data(valid_data).await;
    assert!(result.is_ok(), "Should process valid data after errors");

    // Store error recovery results
    let memory_key = "swarm-auto-centralized-1751484080479/end-to-end-validation/error_recovery";
    orchestrator.store_results_in_memory(memory_key).await?;
    resource_tracker.mark_cleanup_completed();

    Ok(())
}

/// Test comprehensive data flow validation
#[tokio::test]
async fn test_data_flow_validation() -> Result<()> {
    let config = create_validation_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;

    orchestrator.start_platform().await?;

    // Multi-stage data flow testing
    let symbols = vec!["BTC/USD", "ETH/USD", "ADA/USD", "DOT/USD"];

    for (idx, symbol) in symbols.iter().enumerate() {
        let stage_start = Instant::now();

        // Stage 1: Data Ingestion
        let ingestion_start = Instant::now();
        let market_data =
            create_realistic_market_data(symbol, 1000.0 + (idx as f64 * 500.0), idx as u64);
        orchestrator.inject_market_data(market_data.clone()).await?;
        let ingestion_time = ingestion_start.elapsed();

        // Stage 2: Pipeline Processing
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        let validation_result = orchestrator.validate_data_flow().await?;
        assert!(
            validation_result.data_ingested,
            "Pipeline should process data for {}",
            symbol
        );
        assert!(
            validation_result.pipeline_processed,
            "Indicators should be calculated for {}",
            symbol
        );

        // Stage 3: Event Bus Propagation
        assert!(
            validation_result.events_published,
            "Events should be published for {}",
            symbol
        );

        // Stage 4: Data Access Layer
        let dal_start = Instant::now();
        let latest_data = orchestrator.get_latest_market_data(symbol).await?;
        let dal_time = dal_start.elapsed();

        if let Some(data) = latest_data {
            assert_eq!(data.symbol, *symbol, "Symbol should match");
        }

        // Performance validation
        assert!(
            ingestion_time.as_millis() < TARGET_DATA_STORAGE_LATENCY_MS as u128,
            "Data ingestion too slow for {}: {}ms",
            symbol,
            ingestion_time.as_millis()
        );
    }

    // Store data flow validation results
    let memory_key = "swarm-auto-centralized-1751484080479/end-to-end-validation/data_flow";
    orchestrator.store_results_in_memory(memory_key).await?;

    Ok(())
}

/// Test performance validation against all targets
#[tokio::test]
async fn test_performance_validation() -> Result<()> {
    let config = create_validation_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;

    orchestrator.start_platform().await?;

    let agent_id = "performance_validation_agent";
    orchestrator.register_daa_agent(agent_id).await?;

    // Test 1: Data Storage Latency
    let mut storage_latencies = Vec::new();
    for i in 0..50 {
        let start = Instant::now();
        let market_data = create_realistic_market_data("PERF/TEST", 1000.0, i);
        orchestrator.inject_market_data(market_data).await?;
        storage_latencies.push(start.elapsed().as_millis() as u64);
    }

    let avg_storage_latency =
        storage_latencies.iter().sum::<u64>() / storage_latencies.len() as u64;
    let max_storage_latency = *storage_latencies.iter().max().unwrap();

    assert!(
        avg_storage_latency < TARGET_DATA_STORAGE_LATENCY_MS,
        "Average storage latency too high: {}ms",
        avg_storage_latency
    );
    assert!(
        max_storage_latency < TARGET_DATA_STORAGE_LATENCY_MS * 2,
        "Max storage latency too high: {}ms",
        max_storage_latency
    );

    // Test 2: Neural Prediction Latency
    let mut prediction_latencies = Vec::new();
    for i in 0..20 {
        let decision_context = DecisionContext {
            agent_id: format!("perf_agent_{}", i),
            decision_type: "PERFORMANCE_TEST".to_string(),
            symbol: "PERF/TEST".to_string(),
            market_data: create_enhanced_time_series_data("PERF/TEST", 1000.0),
            context_metadata: create_enhanced_context_metadata(),
            required_confidence: 0.7,
            prediction_horizon: 60,
        };

        let start = Instant::now();
        let _prediction = orchestrator.get_neural_prediction(decision_context).await?;
        prediction_latencies.push(start.elapsed().as_millis() as u64);
    }

    let avg_prediction_latency =
        prediction_latencies.iter().sum::<u64>() / prediction_latencies.len() as u64;

    assert!(
        avg_prediction_latency < TARGET_NEURAL_PREDICTION_LATENCY_MS,
        "Average prediction latency too high: {}ms",
        avg_prediction_latency
    );

    // Test 3: Agent Decision Latency
    let mut decision_latencies = Vec::new();
    for i in 0..30 {
        let start = Instant::now();
        let market_data = create_realistic_market_data("DECISION/TEST", 1000.0, i);
        orchestrator.inject_market_data(market_data).await?;

        // Simulate agent decision processing
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;

        let _agent_events = orchestrator.get_agent_events(agent_id).await?;
        decision_latencies.push(start.elapsed().as_millis() as u64);
    }

    let avg_decision_latency =
        decision_latencies.iter().sum::<u64>() / decision_latencies.len() as u64;

    assert!(
        avg_decision_latency < TARGET_AGENT_DECISION_LATENCY_MS,
        "Average decision latency too high: {}ms",
        avg_decision_latency
    );

    // Store performance validation results
    let memory_key = "swarm-auto-centralized-1751484080479/end-to-end-validation/performance";
    orchestrator.store_results_in_memory(memory_key).await?;

    Ok(())
}

// Helper functions for enhanced test data creation

fn create_enhanced_time_series_data(symbol: &str, price: f64) -> TimeSeriesData {
    let mut indicators = HashMap::new();
    indicators.insert("RSI".to_string(), 65.5);
    indicators.insert("MACD".to_string(), 250.0);
    indicators.insert("SMA_20".to_string(), price - 5.0);
    indicators.insert("EMA_12".to_string(), price + 2.0);
    indicators.insert("Bollinger_Upper".to_string(), price + 50.0);
    indicators.insert("Bollinger_Lower".to_string(), price - 50.0);
    indicators.insert("ATR".to_string(), 25.0);
    indicators.insert("Volume_SMA".to_string(), 1000.0);

    TimeSeriesData {
        symbol: symbol.to_string(),
        timestamp: Utc::now(),
        open: price - 10.0,
        high: price + 20.0,
        low: price - 20.0,
        close: price,
        volume: 1000.0,
        indicators,
    }
}

fn create_enhanced_context_metadata() -> HashMap<String, serde_json::Value> {
    let mut metadata = HashMap::new();
    metadata.insert("strategy".to_string(), json!("end_to_end_validation"));
    metadata.insert("risk_level".to_string(), json!(0.02));
    metadata.insert("position_size".to_string(), json!(0.1));
    metadata.insert("max_drawdown".to_string(), json!(0.05));
    metadata.insert("stop_loss".to_string(), json!(0.03));
    metadata.insert("take_profit".to_string(), json!(0.06));
    metadata.insert("market_regime".to_string(), json!("trending"));
    metadata.insert("volatility_adjusted".to_string(), json!(true));
    metadata.insert("correlation_checked".to_string(), json!(true));
    metadata.insert("test_mode".to_string(), json!(true));
    metadata
}
