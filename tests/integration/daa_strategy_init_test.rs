//! Integration tests for DAA Coordinator with strategy initialization fix
//! 
//! These tests ensure the DAA coordinator properly initializes strategies
//! before using them in the decision-making process.

use autonomous_platform::integration::daa_coordinator::*;
use autonomous_platform::config::{NeuralConfig, Config};
use autonomous_platform::neural::{NeuralPredictor, PredictionResult};
use autonomous_platform::strategies::{
use neural_trader::utils::market_hours::MarketHours;    TradingStrategy, Signal, MarketContext, Position, PositionSide, 
    StrategyConfig, StrategyError, StrategyFactory
};
use autonomous_platform::data::TimeSeriesData;

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};
use std::collections::HashMap;
use chrono::Utc;
use serde_json::json;

// Helper to create test configuration
fn create_test_config() -> Config {
    Config {
        neural: NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
        },
        strategies: vec![
            StrategyConfig {
                name: "momentum".to_string(),
                enabled: true,
                risk_limit: 0.02,
                position_size: 1.0,
                parameters: HashMap::from([
                    ("fast_period".to_string(), json!(12)),
                    ("slow_period".to_string(), json!(26)),
                    ("rsi_period".to_string(), json!(14)),
                    ("momentum_threshold".to_string(), json!(0.02)),
                ]),
            },
            StrategyConfig {
                name: "momentum".to_string(),
                enabled: true,
                risk_limit: 0.015,
                position_size: 0.5,
                parameters: HashMap::from([
                    ("fast_period".to_string(), json!(10)),
                    ("slow_period".to_string(), json!(20)),
                    ("rsi_period".to_string(), json!(14)),
                    ("momentum_threshold".to_string(), json!(0.015)),
                ]),
            },
        ],
        ..Default::default()
    }
}

// Helper function to create test MarketHours
fn create_test_market_hours() -> Arc<MarketHours> {
    Arc::new(MarketHours::default())
}
#[tokio::test]
async fn test_daa_coordinator_initializes_strategies_before_use() {
    // GIVEN: A DAA coordinator with strategy configurations
    let config = create_test_config();
    let neural_predictor = Arc::new(NeuralPredictor::new(config.neural.clone()).unwrap());
    let (tx, mut rx) = mpsc::channel(100);
    
    let daa_config = DaaConfig::default();
    let coordinator = DaaCoordinator::new(daa_config, neural_predictor.clone(), tx, create_test_market_hours());
    
    // Register strategies using the new initialization method
    for (i, strategy_config) in config.strategies.iter().enumerate() {
        let strategy = StrategyFactory::create_and_initialize_strategy(
            strategy_config.clone(),
            Some(neural_predictor.clone())
        ).await.expect("Strategy should be created and initialized");
        
        coordinator.register_strategy(
            format!("strategy_{}", i),
            strategy
        ).await;
    }
    
    // WHEN: We make a decision
    let market_context = MarketContext {
        symbol: "BTC/USDT".to_string(),
        current_price: 50000.0,
        bid: 49990.0,
        ask: 50010.0,
        volume_24h: 1_000_000.0,
        volatility: 0.02,
        timestamp: Utc::now().timestamp(),
    };
    
    let historical_data = vec![
        TimeSeriesData {
            symbol: "BTC/USDT".to_string(),
            timestamp: Utc::now(),
            open: 49800.0,
            high: 50100.0,
            low: 49700.0,
            close: 50000.0,
            volume: 1000.0,
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("BTC".to_string()),
            value: Some(50000.0),
            metadata: None,
        }
    ];
    
    let decision = coordinator.make_decision(
        &market_context,
        None,
        &historical_data,
    ).await;
    
    // THEN: Decision should be made successfully without initialization errors
    assert!(decision.is_ok(), "Decision making should succeed");
    let decision = decision.unwrap();
    
    // Verify we received the decision through the channel
    let received = timeout(Duration::from_secs(1), rx.recv()).await;
    assert!(received.is_ok(), "Should receive decision through channel");
    
    // The decision should include strategy votes (not errors)
    assert!(!decision.reasoning.is_empty(), "Should have reasoning");
    assert!(
        decision.reasoning.iter().any(|r| r.contains("votes")),
        "Reasoning should include strategy votes, not initialization errors"
    );
}

#[tokio::test]
async fn test_daa_handles_mixed_initialized_uninitialized_strategies() {
    // GIVEN: A coordinator with both initialized and uninitialized strategies
    let neural_config = NeuralConfig::default();
    let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
    let (tx, _rx) = mpsc::channel(100);
    
    let daa_config = DaaConfig::default();
    let coordinator = DaaCoordinator::new(daa_config, neural_predictor.clone(), tx, create_test_market_hours());
    
    // Register an initialized strategy
    let config1 = StrategyConfig {
        name: "momentum".to_string(),
        enabled: true,
        risk_limit: 0.02,
        position_size: 1.0,
        parameters: HashMap::from([
            ("fast_period".to_string(), json!(12)),
            ("slow_period".to_string(), json!(26)),
        ]),
    };
    
    let initialized_strategy = StrategyFactory::create_and_initialize_strategy(
        config1,
        None
    ).await.expect("Should create initialized strategy");
    
    coordinator.register_strategy("initialized".to_string(), initialized_strategy).await;
    
    // Register an uninitialized strategy (using old method)
    let config2 = StrategyConfig {
        name: "momentum".to_string(),
        enabled: true,
        risk_limit: 0.02,
        position_size: 1.0,
        parameters: HashMap::new(),
    };
    
    let uninitialized_strategy = StrategyFactory::create_strategy(&config2, None)
        .expect("Should create uninitialized strategy");
    
    coordinator.register_strategy("uninitialized".to_string(), uninitialized_strategy).await;
    
    // WHEN: We make a decision
    let market_context = MarketContext {
        symbol: "BTC/USDT".to_string(),
        current_price: 50000.0,
        bid: 49990.0,
        ask: 50010.0,
        volume_24h: 1_000_000.0,
        volatility: 0.02,
        timestamp: Utc::now().timestamp(),
    };
    
    let historical_data = vec![];
    
    let decision = coordinator.make_decision(
        &market_context,
        None,
        &historical_data,
    ).await;
    
    // THEN: Should handle gracefully, using only initialized strategy
    assert!(decision.is_ok(), "Should make decision despite uninitialized strategy");
    let decision = decision.unwrap();
    
    // Should have used the initialized strategy
    assert!(
        decision.reasoning.iter().any(|r| r.contains("initialized votes")),
        "Should include vote from initialized strategy"
    );
}

#[tokio::test]
async fn test_daa_strategy_initialization_with_invalid_config() {
    // GIVEN: Invalid strategy configurations
    let neural_predictor = Arc::new(
        NeuralPredictor::new(NeuralConfig::default()).unwrap()
    );
    let (tx, _rx) = mpsc::channel(100);
    
    let coordinator = DaaCoordinator::new(DaaConfig::default(), neural_predictor.clone(), tx, create_test_market_hours());
    
    // Try to register strategy with invalid config
    let invalid_config = StrategyConfig {
        name: "momentum".to_string(),
        enabled: true,
        risk_limit: 0.02,
        position_size: 1.0,
        parameters: HashMap::from([
            ("fast_period".to_string(), json!(30)),
            ("slow_period".to_string(), json!(20)), // Fast > Slow (invalid)
        ]),
    };
    
    // WHEN: We try to create and register an invalid strategy
    let result = StrategyFactory::create_and_initialize_strategy(
        invalid_config,
        None
    ).await;
    
    // THEN: Creation should fail
    assert!(result.is_err(), "Should fail to create strategy with invalid config");
    
    // Coordinator should continue working without the failed strategy
    let market_context = MarketContext {
        symbol: "BTC/USDT".to_string(),
        current_price: 50000.0,
        bid: 49990.0,
        ask: 50010.0,
        volume_24h: 1_000_000.0,
        volatility: 0.02,
        timestamp: Utc::now().timestamp(),
    };
    
    let decision = coordinator.make_decision(&market_context, None, &[]).await;
    assert!(decision.is_ok(), "Coordinator should work without strategies");
}

#[tokio::test]
async fn test_daa_concurrent_strategy_registration_and_usage() {
    // GIVEN: A coordinator that will receive strategies concurrently
    let neural_predictor = Arc::new(
        NeuralPredictor::new(NeuralConfig::default()).unwrap()
    );
    let (tx, mut rx) = mpsc::channel(100);
    
    let coordinator = Arc::new(DaaCoordinator::new(
        DaaConfig::default(),
        neural_predictor.clone(),
        tx
    ));
    
    // WHEN: We register strategies concurrently while making decisions
    let coordinator_clone1 = Arc::clone(&coordinator);
    let neural_clone1 = Arc::clone(&neural_predictor);
    let register_task1 = tokio::spawn(async move {
        let config = StrategyConfig {
            name: "momentum".to_string(),
            enabled: true,
            risk_limit: 0.02,
            position_size: 1.0,
            parameters: HashMap::from([
                ("fast_period".to_string(), json!(10)),
                ("slow_period".to_string(), json!(20)),
            ]),
        };
        
        let strategy = StrategyFactory::create_and_initialize_strategy(
            config,
            Some(neural_clone1)
        ).await.expect("Strategy 1 should be created");
        
        coordinator_clone1.register_strategy("strategy1".to_string(), strategy).await;
    });
    
    let coordinator_clone2 = Arc::clone(&coordinator);
    let neural_clone2 = Arc::clone(&neural_predictor);
    let register_task2 = tokio::spawn(async move {
        let config = StrategyConfig {
            name: "momentum".to_string(),
            enabled: true,
            risk_limit: 0.015,
            position_size: 0.5,
            parameters: HashMap::from([
                ("fast_period".to_string(), json!(15)),
                ("slow_period".to_string(), json!(30)),
            ]),
        };
        
        let strategy = StrategyFactory::create_and_initialize_strategy(
            config,
            Some(neural_clone2)
        ).await.expect("Strategy 2 should be created");
        
        coordinator_clone2.register_strategy("strategy2".to_string(), strategy).await;
    });
    
    // Start making decisions concurrently
    let coordinator_clone3 = Arc::clone(&coordinator);
    let decision_task = tokio::spawn(async move {
        let market_context = MarketContext {
            symbol: "BTC/USDT".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1_000_000.0,
            volatility: 0.02,
            timestamp: Utc::now().timestamp(),
        };
        
        // Make multiple decisions
        let mut results = vec![];
        for _ in 0..5 {
            let result = coordinator_clone3.make_decision(
                &market_context,
                None,
                &[]
            ).await;
            results.push(result);
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        results
    });
    
    // Wait for all tasks
    let _ = tokio::join!(register_task1, register_task2);
    let decision_results = decision_task.await.unwrap();
    
    // THEN: All operations should complete successfully
    for result in decision_results {
        assert!(result.is_ok(), "All decisions should succeed");
    }
    
    // Verify we received decisions
    let mut decision_count = 0;
    while let Ok(_) = rx.try_recv() {
        decision_count += 1;
    }
    assert!(decision_count > 0, "Should have made at least one decision");
}

#[tokio::test]
async fn test_daa_strategy_initialization_preserves_state() {
    // GIVEN: A coordinator with initialized strategies
    let neural_predictor = Arc::new(
        NeuralPredictor::new(NeuralConfig::default()).unwrap()
    );
    let (tx, _rx) = mpsc::channel(100);
    
    let coordinator = DaaCoordinator::new(DaaConfig::default(), neural_predictor.clone(), tx, create_test_market_hours());
    
    // Register strategy with specific parameters
    let config = StrategyConfig {
        name: "momentum".to_string(),
        enabled: true,
        risk_limit: 0.025,
        position_size: 1.5,
        parameters: HashMap::from([
            ("fast_period".to_string(), json!(8)),
            ("slow_period".to_string(), json!(21)),
            ("momentum_threshold".to_string(), json!(0.01)),
        ]),
    };
    
    let strategy = StrategyFactory::create_and_initialize_strategy(config, None)
        .await
        .expect("Strategy should be created");
    
    coordinator.register_strategy("custom_momentum".to_string(), strategy).await;
    
    // WHEN: We use the strategy multiple times
    let market_context = MarketContext {
        symbol: "BTC/USDT".to_string(),
        current_price: 50000.0,
        bid: 49990.0,
        ask: 50010.0,
        volume_24h: 1_000_000.0,
        volatility: 0.02,
        timestamp: Utc::now().timestamp(),
    };
    
    let decision1 = coordinator.make_decision(&market_context, None, &[]).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let decision2 = coordinator.make_decision(&market_context, None, &[]).await.unwrap();
    
    // THEN: Strategy should maintain its initialized state
    assert!(decision1.reasoning.iter().any(|r| r.contains("custom_momentum")));
    assert!(decision2.reasoning.iter().any(|r| r.contains("custom_momentum")));
    
    // Both decisions should be consistent (same market conditions)
    assert_eq!(decision1.action, decision2.action, "Same conditions should yield same action");
}

#[tokio::test]
async fn test_daa_handles_strategy_runtime_errors_gracefully() {
    // GIVEN: A coordinator with a strategy that might fail at runtime
    let neural_predictor = Arc::new(
        NeuralPredictor::new(NeuralConfig::default()).unwrap()
    );
    let (tx, mut rx) = mpsc::channel(100);
    
    let coordinator = DaaCoordinator::new(DaaConfig::default(), neural_predictor.clone(), tx, create_test_market_hours());
    
    // Register multiple strategies
    for i in 0..3 {
        let config = StrategyConfig {
            name: "momentum".to_string(),
            enabled: true,
            risk_limit: 0.02,
            position_size: 1.0,
            parameters: HashMap::from([
                ("fast_period".to_string(), json!(10 + i)),
                ("slow_period".to_string(), json!(20 + i * 2)),
            ]),
        };
        
        let strategy = StrategyFactory::create_and_initialize_strategy(config, None)
            .await
            .expect("Strategy should be created");
        
        coordinator.register_strategy(format!("strategy_{}", i), strategy).await;
    }
    
    // WHEN: We make decisions with edge case market conditions
    let edge_case_contexts = vec![
        // Zero volume
        MarketContext {
            symbol: "BTC/USDT".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 0.0, // Zero volume
            volatility: 0.02,
            timestamp: Utc::now().timestamp(),
        },
        // Extreme volatility
        MarketContext {
            symbol: "BTC/USDT".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1_000_000.0,
            volatility: 0.8, // 80% volatility
            timestamp: Utc::now().timestamp(),
        },
        // Large spread
        MarketContext {
            symbol: "BTC/USDT".to_string(),
            current_price: 50000.0,
            bid: 45000.0,
            ask: 55000.0, // 10k spread
            volume_24h: 1_000_000.0,
            volatility: 0.02,
            timestamp: Utc::now().timestamp(),
        },
    ];
    
    for context in edge_case_contexts {
        let result = coordinator.make_decision(&context, None, &[]).await;
        
        // THEN: Should handle edge cases gracefully
        assert!(result.is_ok(), "Should handle edge case: {:?}", context);
        let decision = result.unwrap();
        
        // Should still produce a decision
        assert!(!decision.reasoning.is_empty());
        
        // Verify decision was sent
        let received = timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(received.is_ok(), "Should receive decision for edge case");
    }
}
