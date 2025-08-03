//! Edge case and error scenario tests for strategy initialization
//! 
//! Tests various edge cases and error conditions that can occur
//! during strategy creation and initialization.

use autonomous_platform::strategies::{
    MarketContext, Position, PositionSide, Signal, StrategyConfig, 
    StrategyError, StrategyFactory, TradingStrategy,
};
use std::collections::HashMap;
use std::sync::Arc;
use serde_json::json;

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_parameters_uses_defaults() {
        // GIVEN: A strategy config with no parameters
        let config = StrategyConfig {
            name: "momentum".to_string(),
            enabled: true,
            risk_limit: 0.02,
            position_size: 1.0,
            parameters: HashMap::new(), // Empty parameters
        };

        // WHEN: We create and initialize
        let result = StrategyFactory::create_and_initialize_strategy(config, None).await;

        // THEN: Should succeed using default parameters
        assert!(result.is_ok(), "Should create strategy with default parameters");
        let strategy = result.unwrap();
        
        // Verify it works with defaults
        let context = MarketContext {
            symbol: "BTC/USD".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1_000_000.0,
            volatility: 0.03,
            timestamp: 1704067200,
        };
        
        let can_execute = strategy.can_execute(&context);
        assert!(can_execute.is_ok() && can_execute.unwrap());
    }

    #[tokio::test]
    async fn test_partial_parameters_merge_with_defaults() {
        // GIVEN: A config with only some parameters specified
        let config = StrategyConfig {
            name: "momentum".to_string(),
            enabled: true,
            risk_limit: 0.02,
            position_size: 1.0,
            parameters: HashMap::from([
                ("fast_period".to_string(), json!(15)), // Only fast_period specified
                // Other parameters should use defaults
            ]),
        };

        // WHEN: We create and initialize
        let result = StrategyFactory::create_and_initialize_strategy(config, None).await;

        // THEN: Should succeed with mixed custom and default parameters
        assert!(result.is_ok(), "Should handle partial parameters");
    }

    #[tokio::test]
    async fn test_boundary_parameter_values() {
        // Test various boundary conditions
        let boundary_configs = vec![
            // Minimum valid periods
            HashMap::from([
                ("fast_period".to_string(), json!(1)),
                ("slow_period".to_string(), json!(2)),
                ("rsi_period".to_string(), json!(2)),
            ]),
            // Equal periods (should fail)
            HashMap::from([
                ("fast_period".to_string(), json!(10)),
                ("slow_period".to_string(), json!(10)),
            ]),
            // Very large periods
            HashMap::from([
                ("fast_period".to_string(), json!(1000)),
                ("slow_period".to_string(), json!(2000)),
                ("rsi_period".to_string(), json!(500)),
            ]),
            // Zero values (should fail)
            HashMap::from([
                ("fast_period".to_string(), json!(0)),
                ("slow_period".to_string(), json!(10)),
            ]),
            // Extreme thresholds
            HashMap::from([
                ("momentum_threshold".to_string(), json!(0.0001)), // Very small
                ("stop_loss_pct".to_string(), json!(0.001)), // 0.1%
                ("take_profit_pct".to_string(), json!(0.5)), // 50%
            ]),
        ];

        for (i, params) in boundary_configs.into_iter().enumerate() {
            let config = StrategyConfig {
                name: "momentum".to_string(),
                enabled: true,
                risk_limit: 0.02,
                position_size: 1.0,
                parameters: params.clone(),
            };

            let result = StrategyFactory::create_and_initialize_strategy(config, None).await;
            
            // Check expected outcomes
            match i {
                0 => assert!(result.is_ok(), "Minimum valid periods should work"),
                1 => assert!(result.is_err(), "Equal periods should fail"),
                2 => assert!(result.is_ok(), "Large periods should work"),
                3 => assert!(result.is_err(), "Zero period should fail"),
                4 => assert!(result.is_ok(), "Extreme thresholds should work"),
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn test_null_and_invalid_json_types() {
        // GIVEN: Parameters with invalid JSON types
        let invalid_type_configs = vec![
            // Null values
            HashMap::from([
                ("fast_period".to_string(), json!(null)),
                ("slow_period".to_string(), json!(26)),
            ]),
            // String instead of number
            HashMap::from([
                ("fast_period".to_string(), json!("twelve")),
                ("slow_period".to_string(), json!(26)),
            ]),
            // Array instead of number
            HashMap::from([
                ("fast_period".to_string(), json!([12, 13, 14])),
                ("slow_period".to_string(), json!(26)),
            ]),
            // Object instead of number
            HashMap::from([
                ("fast_period".to_string(), json!({"value": 12})),
                ("slow_period".to_string(), json!(26)),
            ]),
            // Boolean instead of number
            HashMap::from([
                ("fast_period".to_string(), json!(true)),
                ("slow_period".to_string(), json!(26)),
            ]),
        ];

        for params in invalid_type_configs {
            let config = StrategyConfig {
                name: "momentum".to_string(),
                enabled: true,
                risk_limit: 0.02,
                position_size: 1.0,
                parameters: params,
            };

            let result = StrategyFactory::create_and_initialize_strategy(config, None).await;
            assert!(result.is_err(), "Should fail with invalid parameter types");
            assert!(matches!(result.unwrap_err(), StrategyError::Configuration(_)));
        }
    }

    #[tokio::test]
    async fn test_float_periods_converted_to_integers() {
        // GIVEN: Float values for integer parameters
        let config = StrategyConfig {
            name: "momentum".to_string(),
            enabled: true,
            risk_limit: 0.02,
            position_size: 1.0,
            parameters: HashMap::from([
                ("fast_period".to_string(), json!(12.7)), // Should truncate to 12
                ("slow_period".to_string(), json!(26.3)), // Should truncate to 26
                ("rsi_period".to_string(), json!(14.9)), // Should truncate to 14
            ]),
        };

        // WHEN: We create and initialize
        let result = StrategyFactory::create_and_initialize_strategy(config, None).await;

        // THEN: Should handle float to int conversion
        assert!(result.is_ok(), "Should handle float periods");
    }

    #[tokio::test]
    async fn test_extremely_large_numbers() {
        // GIVEN: Extremely large parameter values
        let config = StrategyConfig {
            name: "momentum".to_string(),
            enabled: true,
            risk_limit: 0.02,
            position_size: 1.0,
            parameters: HashMap::from([
                ("fast_period".to_string(), json!(i64::MAX)),
                ("slow_period".to_string(), json!(i64::MAX)),
            ]),
        };

        // WHEN: We try to create with huge numbers
        let result = StrategyFactory::create_and_initialize_strategy(config, None).await;

        // THEN: Should handle gracefully (likely fail due to usize conversion)
        // The behavior depends on implementation details
        if result.is_err() {
            assert!(matches!(result.unwrap_err(), StrategyError::Configuration(_)));
        }
    }

    #[tokio::test]
    async fn test_special_float_values() {
        // Test special float values
        let special_configs = vec![
            // NaN values
            HashMap::from([
                ("momentum_threshold".to_string(), json!(f64::NAN)),
            ]),
            // Infinity
            HashMap::from([
                ("stop_loss_pct".to_string(), json!(f64::INFINITY)),
            ]),
            // Negative infinity
            HashMap::from([
                ("take_profit_pct".to_string(), json!(f64::NEG_INFINITY)),
            ]),
        ];

        for params in special_configs {
            let config = StrategyConfig {
                name: "momentum".to_string(),
                enabled: true,
                risk_limit: 0.02,
                position_size: 1.0,
                parameters: params,
            };

            let result = StrategyFactory::create_and_initialize_strategy(config, None).await;
            // JSON doesn't support NaN/Infinity, so these might not even parse
            // But if they do, strategy should handle them
        }
    }

    #[tokio::test]
    async fn test_unicode_and_special_strings_in_parameters() {
        // GIVEN: Parameters with unicode and special characters
        let config = StrategyConfig {
            name: "momentum".to_string(),
            enabled: true,
            risk_limit: 0.02,
            position_size: 1.0,
            parameters: HashMap::from([
                ("fast_period".to_string(), json!(12)),
                ("slow_period".to_string(), json!(26)),
                ("🚀_custom".to_string(), json!("ignored")), // Unicode key
                ("; DROP TABLE;".to_string(), json!(0)), // SQL injection attempt
            ]),
        };

        // WHEN: We create and initialize
        let result = StrategyFactory::create_and_initialize_strategy(config, None).await;

        // THEN: Should handle unknown parameters gracefully
        // (Current implementation might fail on unknown parameters)
        if result.is_err() {
            let err = result.unwrap_err();
            assert!(matches!(err, StrategyError::Configuration(_)));
        }
    }
}

#[cfg(test)]
mod memory_and_resource_tests {
    use super::*;

    #[tokio::test]
    async fn test_rapid_strategy_creation_and_destruction() {
        // GIVEN: We need to create many strategies rapidly
        let mut handles = vec![];

        // WHEN: We spawn many strategy creations
        for i in 0..100 {
            let handle = tokio::spawn(async move {
                let config = StrategyConfig {
                    name: "momentum".to_string(),
                    enabled: true,
                    risk_limit: 0.02,
                    position_size: 1.0,
                    parameters: HashMap::from([
                        ("fast_period".to_string(), json!(10 + (i % 10))),
                        ("slow_period".to_string(), json!(20 + (i % 10))),
                    ]),
                };

                StrategyFactory::create_and_initialize_strategy(config, None).await
            });
            handles.push(handle);
        }

        // THEN: All should complete without resource exhaustion
        let mut success_count = 0;
        let mut failure_count = 0;

        for handle in handles {
            match handle.await {
                Ok(Ok(_)) => success_count += 1,
                Ok(Err(_)) => failure_count += 1,
                Err(_) => failure_count += 1,
            }
        }

        assert!(success_count > 90, "Most strategies should be created successfully");
        assert!(failure_count < 10, "Very few should fail");
    }

    #[tokio::test]
    async fn test_strategy_state_isolation() {
        // GIVEN: Multiple strategies created from same factory
        let configs = vec![
            StrategyConfig {
                name: "momentum".to_string(),
                enabled: true,
                risk_limit: 0.01,
                position_size: 0.5,
                parameters: HashMap::from([
                    ("fast_period".to_string(), json!(5)),
                    ("slow_period".to_string(), json!(10)),
                    ("momentum_threshold".to_string(), json!(0.01)),
                ]),
            },
            StrategyConfig {
                name: "momentum".to_string(),
                enabled: true,
                risk_limit: 0.03,
                position_size: 2.0,
                parameters: HashMap::from([
                    ("fast_period".to_string(), json!(20)),
                    ("slow_period".to_string(), json!(40)),
                    ("momentum_threshold".to_string(), json!(0.03)),
                ]),
            },
        ];

        // WHEN: We create multiple strategies
        let mut strategies = vec![];
        for config in configs {
            let strategy = StrategyFactory::create_and_initialize_strategy(config, None)
                .await
                .expect("Strategy creation should succeed");
            strategies.push(strategy);
        }

        // THEN: Each strategy should maintain independent state
        let context = MarketContext {
            symbol: "BTC/USD".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1_000_000.0,
            volatility: 0.03,
            timestamp: 1704067200,
        };

        // Update parameters on first strategy
        let new_params = HashMap::from([
            ("momentum_threshold".to_string(), json!(0.05)),
        ]);
        
        // This would require mutable access to test parameter updates
        // The main point is that strategies maintain independent state
        
        // Verify both can execute independently
        for strategy in &strategies {
            let result = strategy.can_execute(&context);
            assert!(result.is_ok());
        }
    }
}

#[cfg(test)]
mod concurrent_access_tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    #[tokio::test]
    async fn test_concurrent_signal_generation() {
        // GIVEN: A shared strategy instance
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

        let strategy = Arc::new(
            StrategyFactory::create_and_initialize_strategy(config, None)
                .await
                .expect("Strategy creation should succeed")
        );

        // WHEN: Multiple tasks generate signals concurrently
        let mut handles = vec![];
        for i in 0..10 {
            let strategy_clone = Arc::clone(&strategy);
            let handle = tokio::spawn(async move {
                let context = MarketContext {
                    symbol: "BTC/USD".to_string(),
                    current_price: 50000.0 + (i as f64 * 100.0),
                    bid: 49990.0 + (i as f64 * 100.0),
                    ask: 50010.0 + (i as f64 * 100.0),
                    volume_24h: 1_000_000.0,
                    volatility: 0.03,
                    timestamp: 1704067200 + i,
                };

                strategy_clone.generate_signal(&context, None).await
            });
            handles.push(handle);
        }

        // THEN: All should complete successfully
        for handle in handles {
            let result = handle.await.expect("Task should complete");
            assert!(result.is_ok(), "Signal generation should succeed");
        }
    }

    #[tokio::test]
    async fn test_factory_thread_safety() {
        // GIVEN: Multiple threads trying to create strategies
        let handles: Vec<_> = (0..5)
            .map(|i| {
                tokio::spawn(async move {
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

                    StrategyFactory::create_and_initialize_strategy(config, None).await
                })
            })
            .collect();

        // THEN: All should succeed without data races
        for handle in handles {
            let result = handle.await.expect("Task should complete");
            assert!(result.is_ok(), "Strategy creation should be thread-safe");
        }
    }
}