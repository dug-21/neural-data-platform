//! Unit tests for DAA Coordinator (orchestration component)

use autonomous_platform::integration::daa_coordinator::*;
use autonomous_platform::config::NeuralConfig;
use autonomous_platform::neural::{NeuralPredictor, PredictionResult};
use autonomous_platform::strategies::{TradingStrategy, Signal, MarketContext, Position, PositionSide, StrategyConfig, StrategyError};
use neural_trader::utils::market_hours::MarketHours;
use autonomous_platform::data::TimeSeriesData;

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};
use std::sync::atomic::{AtomicBool, Ordering};
use async_trait::async_trait;
use std::collections::HashMap;
use chrono::Utc;

// Mock implementations for testing
struct MockTradingStrategy {
    signal: Signal,
    should_fail: bool,
    name: String,
}

#[async_trait]
impl TradingStrategy for MockTradingStrategy {
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn initialize(&mut self, _config: StrategyConfig) -> Result<(), StrategyError> {
        Ok(())
    }
    
    async fn generate_signal(
        &self,
        _market_context: &MarketContext,
        _current_position: Option<&Position>,
    ) -> Result<Signal, StrategyError> {
        if self.should_fail {
            return Err(StrategyError::Execution("Mock strategy failure".to_string()));
        }
        Ok(self.signal.clone())
    }
    
    async fn update_parameters(
        &mut self,
        _parameters: HashMap<String, serde_json::Value>,
    ) -> Result<(), StrategyError> {
        Ok(())
    }
    
    fn get_metrics(&self) -> HashMap<String, f64> {
        let mut metrics = HashMap::new();
        metrics.insert("test_metric".to_string(), 1.0);
        metrics
    }
    
    fn can_execute(&self, _context: &MarketContext) -> Result<bool, StrategyError> {
        Ok(!self.should_fail)
    }
}

// Helper function to create test market context
fn create_test_market_context() -> MarketContext {
    MarketContext {
        symbol: "BTC/USDT".to_string(),
        current_price: 50000.0,
        bid: 49990.0,
        ask: 50010.0,
        volume_24h: 1000000.0,
        volatility: 0.02,
        timestamp: Utc::now().timestamp(),
    }
}

// Helper function to create test position
fn create_test_position() -> Position {
    Position {
        symbol: "BTC/USDT".to_string(),
        side: PositionSide::Long,
        size: 0.1,
        entry_price: 49500.0,
        current_price: 50000.0,
        unrealized_pnl: 50.0, // (50000 - 49500) * 0.1
        timestamp: Utc::now().timestamp(),
    }
}

// Helper function to create test time series data
fn create_test_time_series_data() -> Vec<TimeSeriesData> {
    vec![
        TimeSeriesData {
            symbol: "BTC/USDT".to_string(),
            timestamp: Utc::now(),
            open: 49700.0,
            high: 49850.0,
            low: 49650.0,
            close: 49800.0,
            volume: vec![100.0],
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("BTC".to_string()),
            value: Some(49800.0),
            metadata: None,
        },
        TimeSeriesData {
            symbol: "BTC/USDT".to_string(),
            timestamp: Utc::now(),
            open: 49800.0,
            high: 49950.0,
            low: 49750.0,
            close: 49900.0,
            volume: vec![110.0],
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("BTC".to_string()),
            value: Some(49900.0),
            metadata: None,
        },
        TimeSeriesData {
            symbol: "BTC/USDT".to_string(),
            timestamp: Utc::now(),
            open: 49900.0,
            high: 50050.0,
            low: 49850.0,
            close: 50000.0,
            volume: vec![120.0],
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("BTC".to_string()),
            value: Some(50000.0),
            metadata: None,
        },
    ]
}

// Helper function to create test MarketHours
fn create_test_market_hours() -> Arc<MarketHours> {
    Arc::new(MarketHours::default())
}
#[tokio::test]
async fn test_daa_coordinator_creation() {
    let neural_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
    let (tx, _rx) = mpsc::channel(100);
    
    let config = DaaConfig::default();
    let _coordinator = DaaCoordinator::new(config.clone(), neural_predictor, tx, create_test_market_hours());
    
    // Verify config has expected default values
    assert_eq!(config.enabled, true);
    assert_eq!(config.min_confidence, 0.75);
    assert_eq!(config.max_risk_per_trade, 0.02);
    assert_eq!(config.max_positions, 5);
    assert_eq!(config.consensus_threshold, 0.7);
    assert_eq!(config.enable_adaptation, true);
}

#[tokio::test]
async fn test_component_initialization_with_strategies() {
    let neural_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
    let (tx, _rx) = mpsc::channel(100);
    
    let config = DaaConfig::default();
    let coordinator = DaaCoordinator::new(config, neural_predictor, tx, create_test_market_hours());
    
    // Register multiple strategies
    let strategy1 = Box::new(MockTradingStrategy {
        signal: Signal::Buy { 
            confidence: 0.8, 
            size: Some(0.1),
            reason: "Test buy signal".to_string(),
        },
        should_fail: false,
        name: "momentum".to_string(),
    });
    let strategy2 = Box::new(MockTradingStrategy {
        signal: Signal::Hold { reason: "Waiting for confirmation".to_string() },
        should_fail: false,
        name: "ma_crossover".to_string(),
    });
    
    coordinator.register_strategy("momentum".to_string(), strategy1).await;
    coordinator.register_strategy("ma_crossover".to_string(), strategy2).await;
    
    // Can't directly verify strategies are registered without exposing internal state
    // But the operations should complete without panic
}

#[tokio::test]
async fn test_event_loop_processing_with_position() {
    let neural_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
    let (tx, mut rx) = mpsc::channel(100);
    
    let config = DaaConfig::default();
    let coordinator = DaaCoordinator::new(config, neural_predictor, tx, create_test_market_hours());
    
    // Register a strategy that signals sell
    let strategy = Box::new(MockTradingStrategy {
        signal: Signal::Sell { 
            confidence: 0.9, 
            size: Some(0.1),
            reason: "Exit signal detected".to_string(),
        },
        should_fail: false,
        name: "trend_following".to_string(),
    });
    coordinator.register_strategy("trend_following".to_string(), strategy).await;
    
    let market_context = create_test_market_context();
    let position = create_test_position();
    let historical_data = create_test_time_series_data();
    
    // Make decision with existing position
    let decision = coordinator.make_decision(
        &market_context,
        Some(&position),
        &historical_data,
    ).await.unwrap();
    
    // Should receive decision through channel
    let received_decision = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("Timeout waiting for decision")
        .expect("Channel closed");
    
    assert_eq!(received_decision.timestamp, decision.timestamp);
}

#[tokio::test]
async fn test_error_handling_strategy_failure() {
    let neural_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
    let (tx, _rx) = mpsc::channel(100);
    
    let config = DaaConfig::default();
    let coordinator = DaaCoordinator::new(config, neural_predictor, tx, create_test_market_hours());
    
    // Register failing strategies
    let failing_strategy = Box::new(MockTradingStrategy {
        signal: Signal::Buy { 
            confidence: 0.8, 
            size: Some(0.1),
            reason: "Failing strategy signal".to_string(),
        },
        should_fail: true,
        name: "failing".to_string(),
    });
    let working_strategy = Box::new(MockTradingStrategy {
        signal: Signal::Buy { 
            confidence: 0.85, 
            size: Some(0.1),
            reason: "Working strategy signal".to_string(),
        },
        should_fail: false,
        name: "working".to_string(),
    });
    
    coordinator.register_strategy("failing".to_string(), failing_strategy).await;
    coordinator.register_strategy("working".to_string(), working_strategy).await;
    
    let market_context = create_test_market_context();
    let historical_data = create_test_time_series_data();
    
    // Should handle strategy failure gracefully
    let decision = coordinator.make_decision(
        &market_context,
        None,
        &historical_data,
    ).await.unwrap();
    
    // Decision should be made with working strategy only
    assert!(decision.reasoning.iter().any(|r| r.contains("working votes BUY")));
}

#[tokio::test]
async fn test_graceful_shutdown() {
    let neural_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
    let (tx, mut rx) = mpsc::channel(100);
    
    let config = DaaConfig::default();
    let coordinator = Arc::new(DaaCoordinator::new(config, neural_predictor, tx, create_test_market_hours()));
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    
    // Spawn background task to simulate event loop
    let coordinator_clone = Arc::clone(&coordinator);
    let shutdown_clone = Arc::clone(&shutdown_flag);
    let handle = tokio::spawn(async move {
        while !shutdown_clone.load(Ordering::Relaxed) {
            let market_context = create_test_market_context();
            let historical_data = create_test_time_series_data();
            
            let _ = coordinator_clone.make_decision(
                &market_context,
                None,
                &historical_data,
            ).await;
            
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    
    // Let it run for a bit
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Signal shutdown
    shutdown_flag.store(true, Ordering::Relaxed);
    
    // Wait for graceful shutdown
    let _ = timeout(Duration::from_secs(1), handle).await;
    
    // Verify we received some decisions
    let mut decision_count = 0;
    while let Ok(_) = rx.try_recv() {
        decision_count += 1;
    }
    assert!(decision_count > 0, "Should have processed at least one decision");
}

#[tokio::test]
async fn test_decision_with_high_volatility() {
    let neural_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
    let (tx, _rx) = mpsc::channel(100);
    
    let config = DaaConfig::default();
    let coordinator = DaaCoordinator::new(config.clone(), neural_predictor, tx, create_test_market_hours());
    
    // Test with high volatility market
    let mut market_context = create_test_market_context();
    market_context.volatility = 0.1; // 10% volatility
    let historical_data = create_test_time_series_data();
    
    // Should make conservative decision with high volatility
    let decision = coordinator.make_decision(
        &market_context,
        None,
        &historical_data,
    ).await.unwrap();
    
    // Check that risk assessment was done (reflected in decision)
    assert_eq!(decision.risk_assessment.market_risk, 0.1);
    assert!(decision.risk_assessment.volatility_adjusted_size < config.max_risk_per_trade);
}

#[tokio::test]
async fn test_performance_metrics_update() {
    let neural_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
    let (tx, _rx) = mpsc::channel(100);
    
    let config = DaaConfig::default();
    let coordinator = DaaCoordinator::new(config, neural_predictor, tx, create_test_market_hours());
    
    // Initial metrics should be default
    let initial_metrics = coordinator.get_metrics().await;
    assert_eq!(initial_metrics.total_decisions, 0);
    assert_eq!(initial_metrics.avg_confidence, 0.0);
    
    // Make a decision
    let market_context = create_test_market_context();
    let historical_data = create_test_time_series_data();
    
    let _decision = coordinator.make_decision(
        &market_context,
        None,
        &historical_data,
    ).await.unwrap();
    
    // Metrics should be updated
    let updated_metrics = coordinator.get_metrics().await;
    assert_eq!(updated_metrics.total_decisions, 1);
    assert!(updated_metrics.avg_confidence > 0.0);
}

#[tokio::test]
async fn test_concurrent_decision_making() {
    let neural_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
    let (tx, mut rx) = mpsc::channel(100);
    
    let config = DaaConfig::default();
    let coordinator = Arc::new(DaaCoordinator::new(config, neural_predictor, tx, create_test_market_hours()));
    
    // Spawn multiple concurrent decision tasks
    let mut handles = vec![];
    for i in 0..5 {
        let coordinator_clone = Arc::clone(&coordinator);
        let handle = tokio::spawn(async move {
            let mut market_context = create_test_market_context();
            market_context.current_price += i as f64 * 100.0; // Vary the price
            let historical_data = create_test_time_series_data();
            
            coordinator_clone.make_decision(
                &market_context,
                None,
                &historical_data,
            ).await
        });
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    for handle in handles {
        assert!(handle.await.is_ok());
    }
    
    // Should have received all decisions
    let mut decision_count = 0;
    while let Ok(_) = rx.try_recv() {
        decision_count += 1;
    }
    assert_eq!(decision_count, 5);
    
    // Verify metrics are consistent
    let metrics = coordinator.get_metrics().await;
    assert_eq!(metrics.total_decisions, 5);
}
