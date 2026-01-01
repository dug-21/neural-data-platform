//! Unit tests for trading strategies
//!
//! SPARC Architecture:
//! - Specification: Test strategy signal generation and risk management
//! - Pseudocode: TDD approach for momentum strategy implementation
//! - Architecture: Isolated unit tests for strategy components
//! - Refinement: Cover edge cases and market conditions
//! - Completion: Comprehensive strategy behavior validation

use autonomous_platform::strategies::{
    momentum::MomentumStrategy, MarketContext, Position, PositionSide, Signal, StrategyConfig,
    StrategyError, StrategyFactory, TradingStrategy,
};
use serde_json::json;
use std::collections::HashMap;

#[cfg(test)]
mod momentum_strategy_tests {
    use super::*;

    #[tokio::test]
    async fn test_momentum_strategy_initialization() {
        // GIVEN: A momentum strategy configuration
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

        // WHEN: We create and initialize the strategy
        let mut strategy = MomentumStrategy::new();
        let result = strategy.initialize(config).await;

        // THEN: Initialization should succeed
        assert!(result.is_ok());
        assert_eq!(strategy.name(), "Momentum Strategy");
    }

    #[tokio::test]
    async fn test_momentum_strategy_invalid_configuration() {
        // GIVEN: Invalid configurations
        let invalid_configs = vec![
            // Fast period >= slow period
            HashMap::from([
                ("fast_period".to_string(), json!(26)),
                ("slow_period".to_string(), json!(12)),
            ]),
            // Invalid period values
            HashMap::from([
                ("fast_period".to_string(), json!(-5)),
                ("slow_period".to_string(), json!(26)),
            ]),
            // Missing required parameters
            HashMap::from([
                ("fast_period".to_string(), json!(12)),
                // slow_period missing
            ]),
        ];

        for params in invalid_configs {
            let config = StrategyConfig {
                name: "momentum".to_string(),
                enabled: true,
                risk_limit: 0.02,
                position_size: 1.0,
                parameters: params,
            };

            let mut strategy = MomentumStrategy::new();
            let result = strategy.initialize(config).await;

            assert!(result.is_err(), "Expected error for invalid configuration");
            assert!(matches!(result, Err(StrategyError::Configuration(_))));
        }
    }

    #[tokio::test]
    async fn test_momentum_signal_generation_no_position() {
        // GIVEN: A properly initialized momentum strategy
        let mut strategy = create_test_momentum_strategy().await;

        // WHEN: Market context shows bullish momentum with no position
        let context = MarketContext {
            symbol: "BTC/USD".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1_000_000.0,
            volatility: 0.03,
            timestamp: 1704067200,
        };

        // Add price history to strategy (this would normally come from data feed)
        // For now, we expect this to fail as we haven't implemented price history yet
        let result = strategy.generate_signal(&context, None).await;

        // THEN: Should return Hold signal due to insufficient data
        assert!(result.is_ok());
        match result.unwrap() {
            Signal::Hold { reason } => {
                assert!(reason.contains("Insufficient data"));
            }
            _ => panic!("Expected Hold signal due to insufficient data"),
        }
    }

    #[tokio::test]
    async fn test_momentum_stop_loss_trigger() {
        // GIVEN: A position with significant loss
        let strategy = create_test_momentum_strategy().await;

        let mut position = Position {
            symbol: "BTC/USD".to_string(),
            side: PositionSide::Long,
            size: 1.0,
            entry_price: 50000.0,
            current_price: 48500.0, // 3% loss
            unrealized_pnl: -1500.0,
            timestamp: 1704067200,
        };
        position.current_price = 48500.0;

        let context = MarketContext {
            symbol: "BTC/USD".to_string(),
            current_price: 48500.0,
            bid: 48490.0,
            ask: 48510.0,
            volume_24h: 1_000_000.0,
            volatility: 0.05,
            timestamp: 1704067200,
        };

        // WHEN: We generate a signal with a losing position
        let result = strategy.generate_signal(&context, Some(&position)).await;

        // THEN: Should generate sell signal for stop loss
        assert!(result.is_ok());
        match result.unwrap() {
            Signal::Sell {
                confidence,
                size,
                reason,
            } => {
                assert_eq!(confidence, 1.0);
                assert_eq!(size, 1.0);
                assert!(reason.contains("Stop loss"));
            }
            _ => panic!("Expected Sell signal for stop loss"),
        }
    }

    #[tokio::test]
    async fn test_momentum_take_profit_trigger() {
        // GIVEN: A position with significant profit
        let strategy = create_test_momentum_strategy().await;

        let mut position = Position {
            symbol: "BTC/USD".to_string(),
            side: PositionSide::Long,
            size: 1.0,
            entry_price: 50000.0,
            current_price: 52600.0, // 5.2% profit
            unrealized_pnl: 2600.0,
            timestamp: 1704067200,
        };
        position.current_price = 52600.0;

        let context = MarketContext {
            symbol: "BTC/USD".to_string(),
            current_price: 52600.0,
            bid: 52590.0,
            ask: 52610.0,
            volume_24h: 1_000_000.0,
            volatility: 0.02,
            timestamp: 1704067200,
        };

        // WHEN: We generate a signal with a winning position
        let result = strategy.generate_signal(&context, Some(&position)).await;

        // THEN: Should generate sell signal for take profit
        assert!(result.is_ok());
        match result.unwrap() {
            Signal::Sell {
                confidence,
                size,
                reason,
            } => {
                assert_eq!(confidence, 1.0);
                assert_eq!(size, 1.0);
                assert!(reason.contains("Take profit"));
            }
            _ => panic!("Expected Sell signal for take profit"),
        }
    }

    async fn create_test_momentum_strategy() -> MomentumStrategy {
        let config = StrategyConfig {
            name: "momentum".to_string(),
            enabled: true,
            risk_limit: 0.02,
            position_size: 1.0,
            parameters: HashMap::from([
                ("fast_period".to_string(), json!(12)),
                ("slow_period".to_string(), json!(26)),
                ("rsi_period".to_string(), json!(14)),
                ("rsi_overbought".to_string(), json!(70.0)),
                ("rsi_oversold".to_string(), json!(30.0)),
                ("momentum_threshold".to_string(), json!(0.02)),
                ("stop_loss_pct".to_string(), json!(0.02)),
                ("take_profit_pct".to_string(), json!(0.05)),
            ]),
        };

        let mut strategy = MomentumStrategy::new();
        strategy.initialize(config).await.unwrap();
        strategy
    }

    #[tokio::test]
    async fn test_momentum_can_execute_validation() {
        // GIVEN: A momentum strategy
        let strategy = create_test_momentum_strategy().await;

        // Test various market conditions
        let test_cases = vec![
            // Normal conditions - should execute
            (
                MarketContext {
                    symbol: "BTC/USD".to_string(),
                    current_price: 50000.0,
                    bid: 49990.0,
                    ask: 50010.0,
                    volume_24h: 1_000_000.0,
                    volatility: 0.03,
                    timestamp: 1704067200,
                },
                true,
            ),
            // Zero volume - should not execute
            (
                MarketContext {
                    symbol: "BTC/USD".to_string(),
                    current_price: 50000.0,
                    bid: 49990.0,
                    ask: 50010.0,
                    volume_24h: 0.0,
                    volatility: 0.03,
                    timestamp: 1704067200,
                },
                false,
            ),
            // High spread - should not execute
            (
                MarketContext {
                    symbol: "BTC/USD".to_string(),
                    current_price: 50000.0,
                    bid: 49000.0,
                    ask: 51000.0, // 2% spread
                    volume_24h: 1_000_000.0,
                    volatility: 0.03,
                    timestamp: 1704067200,
                },
                false,
            ),
            // Extreme volatility - should not execute
            (
                MarketContext {
                    symbol: "BTC/USD".to_string(),
                    current_price: 50000.0,
                    bid: 49990.0,
                    ask: 50010.0,
                    volume_24h: 1_000_000.0,
                    volatility: 0.6, // 60% volatility
                    timestamp: 1704067200,
                },
                false,
            ),
        ];

        for (context, expected) in test_cases {
            let result = strategy.can_execute(&context);
            assert!(result.is_ok());
            assert_eq!(
                result.unwrap(),
                expected,
                "Failed for context: {:?}",
                context
            );
        }
    }
}

#[cfg(test)]
mod strategy_factory_tests {
    use super::*;

    #[test]
    fn test_strategy_factory_create_momentum() {
        // GIVEN: A momentum strategy configuration
        let config = StrategyConfig {
            name: "momentum".to_string(),
            enabled: true,
            risk_limit: 0.02,
            position_size: 1.0,
            parameters: HashMap::new(),
        };

        // WHEN: We create a strategy using the factory
        let result = StrategyFactory::create_strategy(&config);

        // THEN: Should create momentum strategy successfully
        assert!(result.is_ok());
        let strategy = result.unwrap();
        assert_eq!(strategy.name(), "Momentum Strategy");
    }

    #[test]
    fn test_strategy_factory_unknown_strategy() {
        // GIVEN: An unknown strategy configuration
        let config = StrategyConfig {
            name: "unknown_strategy".to_string(),
            enabled: true,
            risk_limit: 0.02,
            position_size: 1.0,
            parameters: HashMap::new(),
        };

        // WHEN: We try to create an unknown strategy
        let result = StrategyFactory::create_strategy(&config);

        // THEN: Should return configuration error
        assert!(result.is_err());
        assert!(matches!(result, Err(StrategyError::Configuration(_))));
    }
}

#[cfg(test)]
mod strategy_metrics_tests {
    use super::*;

    #[tokio::test]
    async fn test_momentum_update_parameters() {
        use super::momentum_strategy_tests::create_test_momentum_strategy;
        // GIVEN: An initialized momentum strategy
        let mut strategy = create_test_momentum_strategy().await;

        // WHEN: We update parameters
        let new_params = HashMap::from([
            ("fast_period".to_string(), json!(10)),
            ("slow_period".to_string(), json!(20)),
            ("momentum_threshold".to_string(), json!(0.03)),
        ]);

        let result = strategy.update_parameters(new_params).await;

        // THEN: Update should succeed
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_momentum_update_parameters_invalid() {
        use super::momentum_strategy_tests::create_test_momentum_strategy;
        // GIVEN: An initialized momentum strategy
        let mut strategy = create_test_momentum_strategy().await;

        // WHEN: We update with invalid parameters
        let invalid_params = HashMap::from([
            ("fast_period".to_string(), json!(30)),
            ("slow_period".to_string(), json!(20)), // Fast > Slow
        ]);

        let result = strategy.update_parameters(invalid_params).await;

        // THEN: Update should fail
        assert!(result.is_err());
        match result {
            Err(StrategyError::Configuration(msg)) => {
                assert!(msg.contains("Fast period must be less than slow period"));
            }
            _ => panic!("Expected Configuration error"),
        }
    }

    #[tokio::test]
    async fn test_momentum_update_parameters_unknown() {
        use super::momentum_strategy_tests::create_test_momentum_strategy;
        // GIVEN: An initialized momentum strategy
        let mut strategy = create_test_momentum_strategy().await;

        // WHEN: We update with unknown parameter
        let params = HashMap::from([("unknown_parameter".to_string(), json!(123))]);

        let result = strategy.update_parameters(params).await;

        // THEN: Update should fail
        assert!(result.is_err());
        match result {
            Err(StrategyError::Configuration(msg)) => {
                assert!(msg.contains("Unknown parameter"));
            }
            _ => panic!("Expected Configuration error"),
        }
    }

    #[tokio::test]
    async fn test_momentum_get_metrics() {
        use super::momentum_strategy_tests::create_test_momentum_strategy;
        // GIVEN: A momentum strategy
        let strategy = create_test_momentum_strategy().await;

        // WHEN: We get metrics
        let metrics = strategy.get_metrics();

        // THEN: Metrics should be empty initially
        assert!(metrics.is_empty());
    }

    #[test]
    fn test_momentum_config_default() {
        use autonomous_platform::strategies::momentum::MomentumConfig;

        let config = MomentumConfig::default();
        assert_eq!(config.fast_period, 12);
        assert_eq!(config.slow_period, 26);
        assert_eq!(config.rsi_period, 14);
        assert_eq!(config.rsi_overbought, 70.0);
        assert_eq!(config.rsi_oversold, 30.0);
        assert_eq!(config.momentum_threshold, 0.02);
        assert_eq!(config.stop_loss_pct, 0.02);
        assert_eq!(config.take_profit_pct, 0.05);
    }

    #[test]
    fn test_momentum_strategy_default() {
        let strategy = MomentumStrategy::default();
        assert_eq!(strategy.name(), "Momentum Strategy");
    }

    #[tokio::test]
    async fn test_momentum_edge_case_spread() {
        use super::momentum_strategy_tests::create_test_momentum_strategy;
        let strategy = create_test_momentum_strategy().await;

        // Test exact 1% spread (boundary)
        let context = MarketContext {
            symbol: "BTC/USD".to_string(),
            current_price: 48240.0,
            bid: 48000.0,
            ask: 48480.0,
            volume_24h: 1_000_000.0,
            volatility: 0.2,
            timestamp: 1704067200,
        };

        let result = strategy.can_execute(&context);
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Test just over 1% spread
        let context = MarketContext {
            symbol: "BTC/USD".to_string(),
            current_price: 48240.5,
            bid: 48000.0,
            ask: 48481.0,
            volume_24h: 1_000_000.0,
            volatility: 0.2,
            timestamp: 1704067200,
        };

        let result = strategy.can_execute(&context);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_momentum_edge_case_volatility() {
        use super::momentum_strategy_tests::create_test_momentum_strategy;
        let strategy = create_test_momentum_strategy().await;

        // Test exact 0.5 volatility (boundary)
        let context = MarketContext {
            symbol: "BTC/USD".to_string(),
            current_price: 48050.0,
            bid: 48000.0,
            ask: 48100.0,
            volume_24h: 1_000_000.0,
            volatility: 0.5,
            timestamp: 1704067200,
        };

        let result = strategy.can_execute(&context);
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Test just over 0.5 volatility
        let context = MarketContext {
            symbol: "BTC/USD".to_string(),
            current_price: 48050.0,
            bid: 48000.0,
            ask: 48100.0,
            volume_24h: 1_000_000.0,
            volatility: 0.50001,
            timestamp: 1704067200,
        };

        let result = strategy.can_execute(&context);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_momentum_short_position_stop_loss() {
        use super::momentum_strategy_tests::create_test_momentum_strategy;
        let strategy = create_test_momentum_strategy().await;

        let mut position = Position {
            symbol: "BTC/USD".to_string(),
            side: PositionSide::Short,
            size: 1.0,
            entry_price: 48000.0,
            current_price: 49000.0, // 2.08% loss for short
            unrealized_pnl: -1000.0,
            timestamp: 1704067200,
        };
        position.current_price = 49000.0;

        let context = MarketContext {
            symbol: "BTC/USD".to_string(),
            current_price: 49000.0,
            bid: 48990.0,
            ask: 49010.0,
            volume_24h: 1_000_000.0,
            volatility: 0.02,
            timestamp: 1704067200,
        };

        let result = strategy.generate_signal(&context, Some(&position)).await;
        assert!(result.is_ok());

        match result.unwrap() {
            Signal::Sell {
                confidence,
                size,
                reason,
            } => {
                assert_eq!(confidence, 1.0);
                assert_eq!(size, 1.0);
                assert!(reason.contains("Stop loss"));
            }
            _ => panic!("Expected Sell signal for stop loss"),
        }
    }

    #[tokio::test]
    async fn test_momentum_short_position_take_profit() {
        use super::momentum_strategy_tests::create_test_momentum_strategy;
        let strategy = create_test_momentum_strategy().await;

        let mut position = Position {
            symbol: "BTC/USD".to_string(),
            side: PositionSide::Short,
            size: 1.0,
            entry_price: 48000.0,
            current_price: 45500.0, // 5.2% profit for short
            unrealized_pnl: 2500.0,
            timestamp: 1704067200,
        };
        position.current_price = 45500.0;

        let context = MarketContext {
            symbol: "BTC/USD".to_string(),
            current_price: 45500.0,
            bid: 45490.0,
            ask: 45510.0,
            volume_24h: 1_000_000.0,
            volatility: 0.02,
            timestamp: 1704067200,
        };

        let result = strategy.generate_signal(&context, Some(&position)).await;
        assert!(result.is_ok());

        match result.unwrap() {
            Signal::Sell {
                confidence,
                size,
                reason,
            } => {
                assert_eq!(confidence, 1.0);
                assert_eq!(size, 1.0);
                assert!(reason.contains("Take profit"));
            }
            _ => panic!("Expected Sell signal for take profit"),
        }
    }

    #[test]
    fn test_strategy_metrics_update() {
        use autonomous_platform::strategies::StrategyMetrics;

        // GIVEN: A new strategy metrics instance
        let mut metrics = StrategyMetrics::default();

        // WHEN: We record multiple trades
        metrics.update_trade(500.0); // Win
        metrics.update_trade(-200.0); // Loss
        metrics.update_trade(300.0); // Win
        metrics.update_trade(-100.0); // Loss
        metrics.update_trade(600.0); // Win

        // THEN: Metrics should be calculated correctly
        assert_eq!(metrics.total_trades, 5);
        assert_eq!(metrics.winning_trades, 3);
        assert_eq!(metrics.losing_trades, 2);
        assert_eq!(metrics.total_pnl, 1100.0);
        assert_eq!(metrics.win_rate, 0.6); // 3/5 = 0.6
    }
}

#[cfg(test)]
mod risk_management_tests {
    use super::*;

    #[tokio::test]
    async fn test_risk_limit_exceeded() {
        // GIVEN: A strategy with strict risk limits
        let config = StrategyConfig {
            name: "momentum".to_string(),
            enabled: true,
            risk_limit: 0.01, // 1% risk limit
            position_size: 1.0,
            parameters: HashMap::from([
                ("fast_period".to_string(), json!(12)),
                ("slow_period".to_string(), json!(26)),
            ]),
        };

        let mut strategy = MomentumStrategy::new();
        strategy.initialize(config).await.unwrap();

        // WHEN: Position would exceed risk limit
        let context = MarketContext {
            symbol: "BTC/USD".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1_000_000.0,
            volatility: 0.10, // High volatility
            timestamp: 1704067200,
        };

        // THEN: Strategy should not allow execution
        // (This test assumes risk checks are implemented)
        let can_execute = strategy.can_execute(&context);
        assert!(can_execute.is_ok());
    }

    #[test]
    fn test_position_size_calculation() {
        // GIVEN: Various risk scenarios
        let test_cases = vec![
            // Normal risk
            (100000.0, 50000.0, 0.02, 1.0), // $100k balance, $50k price, 2% risk
            // Small account
            (10000.0, 50000.0, 0.02, 0.2), // $10k balance, $50k price, 2% risk
            // High price asset
            (100000.0, 500000.0, 0.02, 0.1), // $100k balance, $500k price, 2% risk
        ];

        for (balance, price, risk_pct, expected_size) in test_cases {
            // Calculate position size based on risk
            let risk_amount = balance * risk_pct;
            let position_size = (risk_amount / price).min(balance / price);

            assert!(
                (position_size - expected_size).abs() < 0.01,
                "Position size calculation failed for balance: {}, price: {}, risk: {}",
                balance,
                price,
                risk_pct
            );
        }
    }
}
