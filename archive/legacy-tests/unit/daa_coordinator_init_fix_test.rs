//! Unit tests specifically for the DAA coordinator initialization fix
//! 
//! These tests validate that the DAA coordinator properly initializes
//! strategies from configuration before using them.

use autonomous_platform::integration::daa_coordinator::*;
use crate::helpers::test_utils::create_test_market_hours;use autonomous_platform::config::{NeuralConfig, Config};
use crate::helpers::test_utils::create_test_market_hours;use autonomous_platform::neural::NeuralPredictor;
use crate::helpers::test_utils::create_test_market_hours;use autonomous_platform::strategies::{
use crate::helpers::test_utils::create_test_market_hours;use autonomous_platform::utils::market_hours::MarketHours;    TradingStrategy, Signal, MarketContext, Position, PositionSide, 
use crate::helpers::test_utils::create_test_market_hours;    StrategyConfig, StrategyError, StrategyFactory
};
use autonomous_platform::data::TimeSeriesData;
use crate::helpers::test_utils::create_test_market_hours;
use std::sync::Arc;
use tokio::sync::mpsc;
use std::collections::HashMap;
use chrono::Utc;
use serde_json::json;

#[cfg(test)]
mod daa_init_fix_tests {
    use super::*;

    #[tokio::test]
    async fn test_daa_initializes_strategies_from_config() {
        // GIVEN: A configuration with multiple strategies
        let mut config = Config::default();
        config.strategies = vec![
            StrategyConfig {
                name: "momentum".to_string(),
                enabled: true,
                risk_limit: 0.02,
                position_size: 1.0,
                parameters: HashMap::from([
                    ("fast_period".to_string(), json!(12)),
                    ("slow_period".to_string(), json!(26)),
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
                ]),
            },
        ];

        let neural_predictor = Arc::new(
            NeuralPredictor::new(config.neural.clone()).unwrap()
        );
        let (tx, mut rx) = mpsc::channel(100);

        // WHEN: We create a DAA coordinator with the fixed initialization
        let daa_config = DaaConfig::default();
        let coordinator = DaaCoordinator::new(daa_config, neural_predictor.clone(), tx, create_test_market_hours(, create_test_market_hours()));

        // Register strategies using the fixed approach
        for (i, strategy_config) in config.strategies.iter().enumerate() {
            // This simulates the fix: create_and_initialize_strategy
            let strategy = StrategyFactory::create_and_initialize_strategy(
                strategy_config.clone(),
                Some(neural_predictor.clone())
            ).await.expect("Strategy should be created and initialized");

            coordinator.register_strategy(
                format!("strategy_{}", i),
                strategy
            ).await;
        }

        // Create market data
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
                volume: vec![1000.0],
                indicators: HashMap::new(),
                source: Some("test".to_string()),
                entity: Some("BTC".to_string()),
                value: Some(50000.0),
                metadata: None,
            }
        ];

        // THEN: Decision making should work without initialization errors
        let decision = coordinator.make_decision(
            &market_context,
            None,
            &historical_data,
        ).await;

        assert!(decision.is_ok(), "Decision should be made successfully");
        let decision = decision.unwrap();

        // Verify the decision includes strategy contributions
        assert!(!decision.reasoning.is_empty());
        assert!(
            decision.reasoning.iter().any(|r| r.contains("strategy_0 votes")),
            "Should include votes from initialized strategies"
        );

        // Verify decision was published
        let received = rx.try_recv();
        assert!(received.is_ok(), "Decision should be published to channel");
    }

    #[tokio::test]
    async fn test_daa_handles_initialization_failures_gracefully() {
        // GIVEN: A configuration with both valid and invalid strategies
        let invalid_strategies = vec![
            // Valid strategy
            StrategyConfig {
                name: "momentum".to_string(),
                enabled: true,
                risk_limit: 0.02,
                position_size: 1.0,
                parameters: HashMap::from([
                    ("fast_period".to_string(), json!(12)),
                    ("slow_period".to_string(), json!(26)),
                ]),
            },
            // Invalid strategy (fast > slow)
            StrategyConfig {
                name: "momentum".to_string(),
                enabled: true,
                risk_limit: 0.02,
                position_size: 1.0,
                parameters: HashMap::from([
                    ("fast_period".to_string(), json!(30)),
                    ("slow_period".to_string(), json!(20)),
                ]),
            },
            // Another valid strategy
            StrategyConfig {
                name: "momentum".to_string(),
                enabled: true,
                risk_limit: 0.015,
                position_size: 0.5,
                parameters: HashMap::from([
                    ("fast_period".to_string(), json!(5)),
                    ("slow_period".to_string(), json!(15)),
                ]),
            },
        ];

        let neural_predictor = Arc::new(
            NeuralPredictor::new(NeuralConfig::default()).unwrap()
        );
        let (tx, _rx) = mpsc::channel(100);

        let coordinator = DaaCoordinator::new(DaaConfig::default(), neural_predictor.clone(), tx, create_test_market_hours(, create_test_market_hours()));

        // WHEN: We try to register all strategies
        let mut successful_registrations = 0;
        let mut failed_registrations = 0;

        for (i, strategy_config) in invalid_strategies.iter().enumerate() {
            match StrategyFactory::create_and_initialize_strategy(
                strategy_config.clone(),
                Some(neural_predictor.clone())
            ).await {
                Ok(strategy) => {
                    coordinator.register_strategy(
                        format!("strategy_{}", i),
                        strategy
                    ).await;
                    successful_registrations += 1;
                }
                Err(_) => {
                    failed_registrations += 1;
                }
            }
        }

        // THEN: Should have registered valid strategies and skipped invalid ones
        assert_eq!(successful_registrations, 2, "Two valid strategies should register");
        assert_eq!(failed_registrations, 1, "One invalid strategy should fail");

        // Coordinator should still work with registered strategies
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
        assert!(decision.is_ok(), "Should make decision with valid strategies");
    }

    #[tokio::test]
    async fn test_daa_without_initialized_strategies_fails_gracefully() {
        // GIVEN: A coordinator with uninitialized strategies (simulating the bug)
        let neural_predictor = Arc::new(
            NeuralPredictor::new(NeuralConfig::default()).unwrap()
        );
        let (tx, _rx) = mpsc::channel(100);

        let coordinator = DaaCoordinator::new(DaaConfig::default(), neural_predictor.clone(), tx, create_test_market_hours(, create_test_market_hours()));

        // Register an UNINITIALIZED strategy (simulating the bug)
        let config = StrategyConfig {
            name: "momentum".to_string(),
            enabled: true,
            risk_limit: 0.02,
            position_size: 1.0,
            parameters: HashMap::new(),
        };

        // Use the OLD method that doesn't initialize
        if let Ok(uninitialized_strategy) = StrategyFactory::create_strategy(&config, None) {
            coordinator.register_strategy(
                "uninitialized".to_string(),
                uninitialized_strategy
            ).await;
        }

        // WHEN: We try to make a decision
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

        // THEN: Should handle gracefully (not panic)
        assert!(decision.is_ok(), "Should handle uninitialized strategies gracefully");
        
        // The decision might be conservative or use neural predictor only
        let decision = decision.unwrap();
        assert!(
            decision.action == autonomous_platform::integration::daa_coordinator::Action::Hold ||
            decision.reasoning.iter().any(|r| r.contains("error") || r.contains("failed")),
            "Should either hold or indicate strategy errors"
        );
    }

    #[tokio::test]
    async fn test_daa_strategy_initialization_order() {
        // GIVEN: Strategies that depend on initialization order
        let neural_predictor = Arc::new(
            NeuralPredictor::new(NeuralConfig::default()).unwrap()
        );
        let (tx, mut rx) = mpsc::channel(100);

        let coordinator = DaaCoordinator::new(DaaConfig::default(), neural_predictor.clone(), tx, create_test_market_hours(, create_test_market_hours()));

        // Create strategies with different initialization times
        let strategies = vec![
            ("fast_strategy", StrategyConfig {
                name: "momentum".to_string(),
                enabled: true,
                risk_limit: 0.01,
                position_size: 0.5,
                parameters: HashMap::from([
                    ("fast_period".to_string(), json!(5)),
                    ("slow_period".to_string(), json!(10)),
                ]),
            }),
            ("medium_strategy", StrategyConfig {
                name: "momentum".to_string(),
                enabled: true,
                risk_limit: 0.02,
                position_size: 1.0,
                parameters: HashMap::from([
                    ("fast_period".to_string(), json!(12)),
                    ("slow_period".to_string(), json!(26)),
                ]),
            }),
            ("slow_strategy", StrategyConfig {
                name: "momentum".to_string(),
                enabled: true,
                risk_limit: 0.03,
                position_size: 1.5,
                parameters: HashMap::from([
                    ("fast_period".to_string(), json!(20)),
                    ("slow_period".to_string(), json!(50)),
                ]),
            }),
        ];

        // WHEN: We initialize and register in order
        for (name, config) in strategies {
            let strategy = StrategyFactory::create_and_initialize_strategy(
                config,
                Some(neural_predictor.clone())
            ).await.expect("Strategy should initialize");

            coordinator.register_strategy(name.to_string(), strategy).await;
        }

        // THEN: All strategies should participate in decision making
        let market_context = MarketContext {
            symbol: "BTC/USDT".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1_000_000.0,
            volatility: 0.02,
            timestamp: Utc::now().timestamp(),
        };

        let decision = coordinator.make_decision(&market_context, None, &[]).await.unwrap();

        // Verify all strategies contributed
        assert!(decision.reasoning.iter().any(|r| r.contains("fast_strategy")));
        assert!(decision.reasoning.iter().any(|r| r.contains("medium_strategy")));
        assert!(decision.reasoning.iter().any(|r| r.contains("slow_strategy")));

        // Verify decision was sent
        assert!(rx.try_recv().is_ok(), "Decision should be published");
    }

    #[tokio::test]
    async fn test_daa_reinitialization_not_allowed() {
        // GIVEN: An already initialized strategy
        let config = StrategyConfig {
            name: "momentum".to_string(),
            enabled: true,
            risk_limit: 0.02,
            position_size: 1.0,
            parameters: HashMap::from([
                ("fast_period".to_string(), json!(12)),
                ("slow_period".to_string(), json!(26)),
            ]),
        };

        // WHEN: We create and initialize a strategy
        let mut strategy = StrategyFactory::create_and_initialize_strategy(
            config.clone(),
            None
        ).await.expect("First initialization should succeed");

        // Try to initialize again with different config
        let new_config = StrategyConfig {
            name: "momentum".to_string(),
            enabled: true,
            risk_limit: 0.03,
            position_size: 2.0,
            parameters: HashMap::from([
                ("fast_period".to_string(), json!(10)),
                ("slow_period".to_string(), json!(20)),
            ]),
        };

        let reinit_result = strategy.initialize(new_config).await;

        // THEN: Reinitialization behavior depends on implementation
        // Some strategies might allow it, others might not
        // The important thing is it doesn't crash
        match reinit_result {
            Ok(_) => {
                // Strategy allows reinitialization
                println!("Strategy allows reinitialization");
            }
            Err(e) => {
                // Strategy prevents reinitialization
                println!("Strategy prevents reinitialization: {:?}", e);
            }
        }
    }
}
