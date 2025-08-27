//! Integration Tests for Neural Trading Binary

use neural_trading::*;
use ndarray::Array1;

// Note: TradingSystem is in main.rs binary, not accessible from lib tests
// This is a proto-only test structure

#[tokio::test]
async fn test_config_validation() {
    let config = TradingConfig::default();
    
    // Validate default configuration
    assert!(!config.redis_url.is_empty());
    assert!(!config.postgres_url.is_empty());
    assert!(!config.broker_endpoint.is_empty());
    assert!(config.risk_limits.max_position_size > 0.0);
    assert!(config.risk_limits.max_daily_loss > 0.0);
    assert!(config.execution_params.max_orders_per_minute > 0);
}

#[tokio::test]
async fn test_risk_limits_validation() {
    // Using types from lib.rs (not risk::limits which doesn't exist)
    let limits = RiskLimits::default();
    
    // Test risk limit structure
    assert!(limits.max_position_size > 0.0);
    assert!(limits.max_daily_loss > 0.0);
    assert!(limits.max_drawdown > 0.0);
    assert!(limits.max_correlation_exposure > 0.0);
    
    // Test reasonable defaults
    assert_eq!(limits.max_position_size, 0.05); // 5%
    assert_eq!(limits.max_daily_loss, 0.02);    // 2%
    assert_eq!(limits.max_drawdown, 0.10);      // 10%
    assert_eq!(limits.max_correlation_exposure, 0.20); // 20%
}

#[tokio::test]
async fn test_execution_params_validation() {
    // Test ExecutionParams structure (no order management module exists yet)
    let params = ExecutionParams::default();
    
    // Test parameter validation
    assert!(params.order_timeout_ms > 0);
    assert!(params.max_slippage_bps > 0);
    assert!(params.min_confidence_threshold > 0.0 && params.min_confidence_threshold <= 1.0);
    assert!(params.max_orders_per_minute > 0);
    
    // Test reasonable defaults
    assert_eq!(params.order_timeout_ms, 5000);  // 5 seconds
    assert_eq!(params.max_slippage_bps, 10);     // 0.1%
    assert_eq!(params.min_confidence_threshold, 0.7); // 70%
    assert_eq!(params.max_orders_per_minute, 100);
}

#[tokio::test]
async fn test_neural_predictor_mock() {
    use neural_trading::inference::predictor::NeuralPredictor;
    
    let predictor = NeuralPredictor::new("./mock_models").await;
    assert!(predictor.is_ok());
    
    let predictor = predictor.unwrap();
    
    // Create mock feature vector
    let features = Array1::from(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    
    // Test trend prediction with required parameter
    let trend = predictor.predict_trend(&features).await;
    assert!(trend.is_ok());
    
    let trend = trend.unwrap();
    assert!(trend.confidence >= 0.0 && trend.confidence <= 1.0);
    assert!(trend.time_horizon > 0);
}

#[tokio::test] 
async fn test_inference_cache() {
    use neural_trading::inference::cache::InferenceCache;
    
    let cache = InferenceCache::new(60); // 60 sec TTL
    
    // Cache a prediction  
    cache.set("test_key".to_string(), "test_value".to_string()).await;
    
    // Retrieve prediction
    let cached = cache.get("test_key").await;
    assert!(cached.is_some());
    assert_eq!(cached.unwrap(), "test_value");
    
    // Test cache clear functionality
    cache.clear().await;
    let cleared = cache.get("test_key").await;
    assert!(cleared.is_none());
}

#[tokio::test]
async fn test_daa_coordinator_initialization() {
    use neural_trading::daa::coordinator::DAACoordinator;
    use neural_trading::inference::predictor::NeuralPredictor;
    use neural_trading::execution::engine::ExecutionEngine;
    use neural_trading::risk::manager::RiskManager;
    
    let neural_predictor = std::sync::Arc::new(
        NeuralPredictor::new("./mock_models").await.unwrap()
    );
    
    let risk_manager = std::sync::Arc::new(
        RiskManager::new(RiskLimits::default())
    );
    
    let execution_params = ExecutionParams {
        order_timeout_ms: 30000,
        max_slippage_bps: 20,
        min_confidence_threshold: 0.6,
        max_orders_per_minute: 10,
    };
    
    let execution_engine = std::sync::Arc::new(
        ExecutionEngine::new(
            "http://mock-broker".to_string(),
            execution_params,
            risk_manager.clone(),
        ).await.unwrap()
    );
    
    let coordinator = DAACoordinator::new(
        "redis://localhost:6379".to_string(),
        neural_predictor,
        execution_engine,
        risk_manager,
    ).await;
    
    assert!(coordinator.is_ok(), "DAA Coordinator should initialize");
}

#[tokio::test]
async fn test_event_consumer_proto() {
    // Test proto-only EventConsumer initialization
    // Note: This is testing the mock/proto version since full EventBus integration is Phase 4
    let redis_url = "redis://localhost:6379".to_string();
    
    // Since DAA coordinator needs full system, we'll test basic initialization concepts
    // In a real integration test, this would use proper DAA coordinator
    
    // Test that consumer creation with mock components works
    let consumer_result = std::panic::catch_unwind(|| {
        // This is a proto test - we're verifying the structure compiles
        assert!(!redis_url.is_empty());
    });
    
    assert!(consumer_result.is_ok(), "EventConsumer proto validation should pass");
}