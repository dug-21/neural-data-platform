//! Integration Tests for Neural Trading Binary

use neural_trading::*;
use tokio_test;

#[tokio::test]
async fn test_trading_system_initialization() {
    let config = TradingConfig::default();
    let system = TradingSystem::new(config).await;
    
    assert!(system.is_ok(), "Trading system should initialize successfully");
}

#[tokio::test]
async fn test_risk_limits_validation() {
    use neural_trading::risk::limits::RiskLimits;
    
    let limits = RiskLimits::default();
    
    // Test position size validation
    assert!(limits.validate_position_size("AAPL", 100.0, 100000.0));
    assert!(!limits.validate_position_size("AAPL", 10000.0, 100000.0)); // Too large
    
    // Test daily loss validation
    assert!(limits.validate_daily_loss(0.01)); // 1% loss is OK
    assert!(!limits.validate_daily_loss(0.03)); // 3% loss exceeds limit
}

#[tokio::test]
async fn test_order_management() {
    use neural_trading::execution::orders::{OrderManager, Order, OrderSide, OrderType};
    
    let manager = OrderManager::new();
    
    // Create test order
    let order = Order::new_market_order("SPY".to_string(), OrderSide::Buy, 100.0);
    let order_id = order.id;
    
    // Add order
    let result = manager.add_order(order).await;
    assert!(result.is_ok());
    
    // Retrieve order
    let retrieved = manager.get_order(order_id).await;
    assert!(retrieved.is_ok());
    assert!(retrieved.unwrap().is_some());
}

#[tokio::test]
async fn test_neural_predictor_mock() {
    use neural_trading::inference::predictor::NeuralPredictor;
    
    let predictor = NeuralPredictor::new("./mock_models").await;
    assert!(predictor.is_ok());
    
    let predictor = predictor.unwrap();
    
    // Test trend prediction
    let trend = predictor.predict_trend().await;
    assert!(trend.is_ok());
    
    let trend = trend.unwrap();
    assert!(trend.confidence >= 0.0 && trend.confidence <= 1.0);
    assert!(trend.time_horizon > 0);
}

#[tokio::test] 
async fn test_inference_cache() {
    use neural_trading::inference::cache::InferenceCache;
    
    let cache = InferenceCache::new(100, 60); // 100 entries, 60 sec TTL
    
    // Cache a prediction
    cache.cache_prediction("test_key".to_string(), "test_value".to_string()).await;
    
    // Retrieve prediction
    let cached = cache.get_prediction("test_key").await;
    assert!(cached.is_some());
    assert_eq!(cached.unwrap(), "test_value");
    
    // Test cache stats
    let stats = cache.get_cache_stats().await;
    assert_eq!(stats.total_entries, 1);
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
async fn test_config_loading() {
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
async fn test_market_event_processing() {
    use neural_trading::events::consumer::{MarketEvent, EventType, EventPriority};
    use chrono::Utc;
    use uuid::Uuid;
    
    let event = MarketEvent {
        event_id: Uuid::new_v4(),
        event_type: EventType::PriceUpdate,
        symbol: "SPY".to_string(),
        timestamp: Utc::now(),
        data: serde_json::json!({
            "price": 445.0,
            "volume": 1000000
        }),
        priority: EventPriority::Normal,
    };
    
    // Validate event structure
    assert!(!event.symbol.is_empty());
    assert!(matches!(event.event_type, EventType::PriceUpdate));
    assert!(matches!(event.priority, EventPriority::Normal));
}