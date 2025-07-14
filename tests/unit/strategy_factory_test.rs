//! Unit tests for Strategy Factory initialization fix
//! 
//! These tests validate that the new create_and_initialize_strategy method
//! properly initializes strategies before returning them.

use neural_trader::strategies::{
    MarketContext, Position, PositionSide, Signal, StrategyConfig, 
    StrategyError, StrategyFactory, TradingStrategy,
    momentum::MomentumStrategy,
};
use std::collections::HashMap;
use std::sync::Arc;
use serde_json::json;

#[cfg(test)]
mod factory_initialization_tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_initialize_momentum_strategy_success() {
        // GIVEN: A valid momentum strategy configuration
        let config = StrategyConfig {
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
        };

        // WHEN: We create and initialize the strategy using the new method
        let result = StrategyFactory::create_and_initialize_strategy(config.clone(), None).await;

        // THEN: Strategy should be created and initialized successfully
        assert!(result.is_ok(), "Strategy creation should succeed");
        let strategy = result.unwrap();
        assert_eq!(strategy.name(), "Momentum Strategy");
        
        // Verify the strategy is properly initialized by testing it can generate signals
        let context = MarketContext {
            symbol: "BTC/USD".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1_000_000.0,
            volatility: 0.03,
            timestamp: 1704067200,
        };
        
        let signal_result = strategy.generate_signal(&context, None).await;
        assert!(signal_result.is_ok(), "Initialized strategy should generate signals");
    }

    #[tokio::test]
    async fn test_create_and_initialize_with_invalid_config() {
        // GIVEN: Invalid strategy configurations
        let test_cases = vec![
            (
                // Unknown strategy type
                StrategyConfig {
                    name: "unknown_strategy".to_string(),
                    enabled: true,
                    risk_limit: 0.02,
                    position_size: 1.0,
                    parameters: HashMap::new(),
                },
                "Unknown strategy",
            ),
            (
                // Invalid momentum parameters (fast >= slow)
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
                "Fast period must be less than slow period",
            ),
            (
                // Invalid parameter types
                StrategyConfig {
                    name: "momentum".to_string(),
                    enabled: true,
                    risk_limit: 0.02,
                    position_size: 1.0,
                    parameters: HashMap::from([
                        ("fast_period".to_string(), json!("not_a_number")),
                        ("slow_period".to_string(), json!(26)),
                    ]),
                },
                "Invalid fast_period",
            ),
        ];

        for (config, expected_error) in test_cases {
            // WHEN: We try to create and initialize with invalid config
            let result = StrategyFactory::create_and_initialize_strategy(config.clone(), None).await;

            // THEN: Should return appropriate error
            assert!(result.is_err(), "Should fail for config: {:?}", config);
            let error_msg = result.unwrap_err().to_string();
            assert!(
                error_msg.contains(expected_error),
                "Error message should contain '{}', got: '{}'",
                expected_error,
                error_msg
            );
        }
    }

    #[tokio::test]
    async fn test_create_and_initialize_neural_enhanced_strategy() {
        // GIVEN: A neural enhanced strategy configuration
        let neural_config = neural_trader::config::NeuralConfig::default();
        let neural_predictor = Arc::new(
            neural_trader::neural::NeuralPredictor::new(neural_config).unwrap()
        );
        
        let config = StrategyConfig {
            name: "neural_enhanced".to_string(),
            enabled: true,
            risk_limit: 0.02,
            position_size: 1.0,
            parameters: HashMap::from([
                ("confidence_threshold".to_string(), json!(0.75)),
                ("use_ml_signals".to_string(), json!(true)),
            ]),
        };

        // WHEN: We create and initialize with neural predictor
        let result = StrategyFactory::create_and_initialize_strategy(
            config.clone(),
            Some(neural_predictor.clone())
        ).await;

        // THEN: Should succeed
        assert!(result.is_ok(), "Neural strategy creation should succeed");
        let strategy = result.unwrap();
        assert_eq!(strategy.name(), "Neural Enhanced Strategy");
    }

    #[tokio::test]
    async fn test_create_and_initialize_neural_without_predictor() {
        // GIVEN: A neural enhanced strategy configuration without predictor
        let config = StrategyConfig {
            name: "neural_enhanced".to_string(),
            enabled: true,
            risk_limit: 0.02,
            position_size: 1.0,
            parameters: HashMap::new(),
        };

        // WHEN: We try to create without neural predictor
        let result = StrategyFactory::create_and_initialize_strategy(config.clone(), None).await;

        // THEN: Should fail with appropriate error
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Neural predictor required"));
    }

    #[tokio::test]
    async fn test_old_create_strategy_still_requires_manual_init() {
        // GIVEN: A momentum strategy config
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

        // WHEN: We use the old create_strategy method (uninitialized)
        let result = StrategyFactory::create_strategy(&config, None);
        assert!(result.is_ok());
        let mut strategy = result.unwrap();

        // THEN: Strategy is created but not initialized
        // It should not be able to generate signals properly until initialized
        let context = MarketContext {
            symbol: "BTC/USD".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1_000_000.0,
            volatility: 0.03,
            timestamp: 1704067200,
        };

        // Generate signal before initialization - should work but return Hold due to no data
        let signal = strategy.generate_signal(&context, None).await.unwrap();
        matches!(signal, Signal::Hold { .. });

        // Now initialize manually
        let init_result = strategy.initialize(config).await;
        assert!(init_result.is_ok());
    }

    #[tokio::test]
    async fn test_concurrent_strategy_creation() {
        // GIVEN: Multiple strategy configurations
        let configs = vec![
            StrategyConfig {
                name: "momentum".to_string(),
                enabled: true,
                risk_limit: 0.02,
                position_size: 1.0,
                parameters: HashMap::from([
                    ("fast_period".to_string(), json!(10)),
                    ("slow_period".to_string(), json!(20)),
                ]),
            },
            StrategyConfig {
                name: "momentum".to_string(),
                enabled: true,
                risk_limit: 0.03,
                position_size: 0.5,
                parameters: HashMap::from([
                    ("fast_period".to_string(), json!(15)),
                    ("slow_period".to_string(), json!(30)),
                ]),
            },
            StrategyConfig {
                name: "momentum".to_string(),
                enabled: true,
                risk_limit: 0.01,
                position_size: 2.0,
                parameters: HashMap::from([
                    ("fast_period".to_string(), json!(5)),
                    ("slow_period".to_string(), json!(15)),
                ]),
            },
        ];

        // WHEN: We create strategies concurrently
        let mut handles = vec![];
        for config in configs {
            let handle = tokio::spawn(async move {
                StrategyFactory::create_and_initialize_strategy(config, None).await
            });
            handles.push(handle);
        }

        // THEN: All should succeed
        let mut success_count = 0;
        for handle in handles {
            let result = handle.await.unwrap();
            if result.is_ok() {
                success_count += 1;
            }
        }
        assert_eq!(success_count, 3, "All concurrent creations should succeed");
    }

    #[tokio::test] 
    async fn test_strategy_initialization_preserves_config() {
        // GIVEN: A specific configuration
        let config = StrategyConfig {
            name: "momentum".to_string(),
            enabled: true,
            risk_limit: 0.025,
            position_size: 1.5,
            parameters: HashMap::from([
                ("fast_period".to_string(), json!(15)),
                ("slow_period".to_string(), json!(35)),
                ("rsi_period".to_string(), json!(20)),
                ("momentum_threshold".to_string(), json!(0.015)),
                ("stop_loss_pct".to_string(), json!(0.03)),
                ("take_profit_pct".to_string(), json!(0.06)),
            ]),
        };

        // WHEN: We create and initialize
        let result = StrategyFactory::create_and_initialize_strategy(config.clone(), None).await;
        assert!(result.is_ok());
        let strategy = result.unwrap();

        // THEN: The strategy should be properly configured
        // We can't directly inspect internal config, but we can verify behavior
        // by testing stop loss and take profit triggers at the configured levels
        
        // Test stop loss at 3%
        let position = Position {
            symbol: "BTC/USD".to_string(),
            side: PositionSide::Long,
            size: 1.5,
            entry_price: 50000.0,
            current_price: 48400.0, // 3.2% loss
            unrealized_pnl: -2400.0,
            timestamp: 1704067200,
        };
        
        let context = MarketContext {
            symbol: "BTC/USD".to_string(),
            current_price: 48400.0,
            bid: 48390.0,
            ask: 48410.0,
            volume_24h: 1_000_000.0,
            volatility: 0.03,
            timestamp: 1704067200,
        };

        let signal = strategy.generate_signal(&context, Some(&position)).await.unwrap();
        match signal {
            Signal::Sell { reason, .. } => {
                assert!(reason.contains("Stop loss"), "Should trigger stop loss at 3%");
            }
            _ => panic!("Expected sell signal for stop loss"),
        }
    }
}

#[cfg(test)]
mod factory_error_handling_tests {
    use super::*;

    #[tokio::test]
    async fn test_initialization_rollback_on_error() {
        // GIVEN: A config that will fail during initialization
        let config = StrategyConfig {
            name: "momentum".to_string(),
            enabled: true,
            risk_limit: 0.02,
            position_size: 1.0,
            parameters: HashMap::from([
                ("fast_period".to_string(), json!(-5)), // Invalid negative period
                ("slow_period".to_string(), json!(26)),
            ]),
        };

        // WHEN: We try to create and initialize
        let result = StrategyFactory::create_and_initialize_strategy(config, None).await;

        // THEN: Should fail without leaving partially initialized strategy
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), StrategyError::Configuration(_)));
    }

    #[tokio::test]
    async fn test_factory_handles_panic_in_initialization() {
        // This test would require a strategy that panics during initialization
        // Since our current strategies don't panic, this is more of a design consideration
        // The factory should use catch_unwind if we want to handle panics gracefully
    }
}

#[cfg(test)]
mod factory_integration_tests {
    use super::*;
    use neural_trader::adapters::DataAdapter;
    use neural_trader::data::TimeSeriesData;
    use chrono::Utc;

    #[tokio::test]
    async fn test_strategy_works_with_real_data_flow() {
        // GIVEN: A properly initialized strategy
        let config = StrategyConfig {
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
        };

        let strategy = StrategyFactory::create_and_initialize_strategy(config, None)
            .await
            .expect("Strategy creation should succeed");

        // WHEN: We simulate market data updates
        let market_context = MarketContext {
            symbol: "BTC/USD".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1_000_000.0,
            volatility: 0.03,
            timestamp: Utc::now().timestamp(),
        };

        // THEN: Strategy should handle the data properly
        let can_execute = strategy.can_execute(&market_context).unwrap();
        assert!(can_execute, "Strategy should be able to execute in normal conditions");

        let signal = strategy.generate_signal(&market_context, None).await.unwrap();
        assert!(matches!(signal, Signal::Hold { .. }), "Should hold with insufficient data");
    }

    #[tokio::test]
    async fn test_multiple_strategies_independent_state() {
        // GIVEN: Two strategies with different configurations
        let config1 = StrategyConfig {
            name: "momentum".to_string(),
            enabled: true,
            risk_limit: 0.02,
            position_size: 1.0,
            parameters: HashMap::from([
                ("fast_period".to_string(), json!(10)),
                ("slow_period".to_string(), json!(20)),
            ]),
        };

        let config2 = StrategyConfig {
            name: "momentum".to_string(),
            enabled: true,
            risk_limit: 0.03,
            position_size: 0.5,
            parameters: HashMap::from([
                ("fast_period".to_string(), json!(15)),
                ("slow_period".to_string(), json!(30)),
            ]),
        };

        // WHEN: We create both strategies
        let strategy1 = StrategyFactory::create_and_initialize_strategy(config1, None)
            .await
            .expect("First strategy should be created");
        let strategy2 = StrategyFactory::create_and_initialize_strategy(config2, None)
            .await
            .expect("Second strategy should be created");

        // THEN: They should maintain independent state
        let context = MarketContext {
            symbol: "BTC/USD".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1_000_000.0,
            volatility: 0.03,
            timestamp: 1704067200,
        };

        let signal1 = strategy1.generate_signal(&context, None).await.unwrap();
        let signal2 = strategy2.generate_signal(&context, None).await.unwrap();

        // Both should work independently
        assert!(matches!(signal1, Signal::Hold { .. }));
        assert!(matches!(signal2, Signal::Hold { .. }));
    }
}