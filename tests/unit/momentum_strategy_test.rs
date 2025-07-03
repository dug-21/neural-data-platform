//! Unit tests for Momentum trading strategy

use neural_trader::strategies::{
    momentum::{MomentumConfig, MomentumStrategy},
    MarketContext, Position, PositionSide, Signal, StrategyConfig, StrategyError, TradingStrategy,
};
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> StrategyConfig {
        let mut parameters = HashMap::new();
        parameters.insert("fast_period".to_string(), serde_json::json!(10));
        parameters.insert("slow_period".to_string(), serde_json::json!(20));
        parameters.insert("rsi_period".to_string(), serde_json::json!(14));
        parameters.insert("momentum_threshold".to_string(), serde_json::json!(0.02));
        
        StrategyConfig {
            name: "test_momentum".to_string(),
            strategy_type: "momentum".to_string(),
            parameters,
            risk_limit: 0.1,
            max_positions: 5,
        }
    }

    fn create_market_context(bid: f64, ask: f64, volume: f64, volatility: f64) -> MarketContext {
        MarketContext {
            symbol: "BTC/USD".to_string(),
            timestamp: 1640995200,
            bid,
            ask,
            volume_24h: volume,
            volatility,
        }
    }

    fn create_position(entry_price: f64, current_price: f64, side: PositionSide) -> Position {
        Position {
            id: "test_position".to_string(),
            symbol: "BTC/USD".to_string(),
            side,
            size: 1.0,
            entry_price,
            current_price,
            timestamp: 1640995200,
            pnl: (current_price - entry_price) * 1.0,
        }
    }

    #[test]
    fn test_momentum_config_default() {
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
    fn test_momentum_strategy_creation() {
        let strategy = MomentumStrategy::new();
        assert_eq!(strategy.name(), "Momentum Strategy");
        assert!(strategy.get_metrics().is_empty());
    }

    #[test]
    fn test_momentum_strategy_default() {
        let strategy = MomentumStrategy::default();
        assert_eq!(strategy.name(), "Momentum Strategy");
    }

    #[tokio::test]
    async fn test_initialize_valid_config() {
        let mut strategy = MomentumStrategy::new();
        let config = create_test_config();
        
        let result = strategy.initialize(config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_initialize_invalid_fast_slow_period() {
        let mut strategy = MomentumStrategy::new();
        let mut config = create_test_config();
        config.parameters.insert("fast_period".to_string(), serde_json::json!(30));
        config.parameters.insert("slow_period".to_string(), serde_json::json!(20));
        
        let result = strategy.initialize(config).await;
        assert!(result.is_err());
        
        match result.unwrap_err() {
            StrategyError::Configuration(msg) => {
                assert_eq!(msg, "Fast period must be less than slow period");
            }
            _ => panic!("Expected Configuration error"),
        }
    }

    #[tokio::test]
    async fn test_initialize_invalid_parameter_type() {
        let mut strategy = MomentumStrategy::new();
        let mut config = create_test_config();
        config.parameters.insert("fast_period".to_string(), serde_json::json!("invalid"));
        
        let result = strategy.initialize(config).await;
        assert!(result.is_err());
        
        match result.unwrap_err() {
            StrategyError::Configuration(msg) => {
                assert_eq!(msg, "Invalid fast_period");
            }
            _ => panic!("Expected Configuration error"),
        }
    }

    #[tokio::test]
    async fn test_update_parameters_valid() {
        let mut strategy = MomentumStrategy::new();
        let config = create_test_config();
        strategy.initialize(config).await.unwrap();
        
        let mut new_params = HashMap::new();
        new_params.insert("fast_period".to_string(), serde_json::json!(15));
        new_params.insert("slow_period".to_string(), serde_json::json!(30));
        new_params.insert("momentum_threshold".to_string(), serde_json::json!(0.03));
        
        let result = strategy.update_parameters(new_params).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_update_parameters_invalid_fast_slow() {
        let mut strategy = MomentumStrategy::new();
        let config = create_test_config();
        strategy.initialize(config).await.unwrap();
        
        let mut new_params = HashMap::new();
        new_params.insert("fast_period".to_string(), serde_json::json!(40));
        new_params.insert("slow_period".to_string(), serde_json::json!(30));
        
        let result = strategy.update_parameters(new_params).await;
        assert!(result.is_err());
        
        match result.unwrap_err() {
            StrategyError::Configuration(msg) => {
                assert_eq!(msg, "Fast period must be less than slow period");
            }
            _ => panic!("Expected Configuration error"),
        }
    }

    #[tokio::test]
    async fn test_update_parameters_unknown_parameter() {
        let mut strategy = MomentumStrategy::new();
        let config = create_test_config();
        strategy.initialize(config).await.unwrap();
        
        let mut new_params = HashMap::new();
        new_params.insert("unknown_param".to_string(), serde_json::json!(123));
        
        let result = strategy.update_parameters(new_params).await;
        assert!(result.is_err());
        
        match result.unwrap_err() {
            StrategyError::Configuration(msg) => {
                assert!(msg.contains("Unknown parameter"));
            }
            _ => panic!("Expected Configuration error"),
        }
    }

    #[test]
    fn test_can_execute_valid_market() {
        let strategy = MomentumStrategy::new();
        let context = create_market_context(48000.0, 48100.0, 1000000.0, 0.2);
        
        let result = strategy.can_execute(&context);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_can_execute_zero_volume() {
        let strategy = MomentumStrategy::new();
        let context = create_market_context(48000.0, 48100.0, 0.0, 0.2);
        
        let result = strategy.can_execute(&context);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_can_execute_high_spread() {
        let strategy = MomentumStrategy::new();
        let context = create_market_context(48000.0, 49000.0, 1000000.0, 0.2);
        
        let result = strategy.can_execute(&context);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_can_execute_high_volatility() {
        let strategy = MomentumStrategy::new();
        let context = create_market_context(48000.0, 48100.0, 1000000.0, 0.6);
        
        let result = strategy.can_execute(&context);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_generate_signal_insufficient_data() {
        let strategy = MomentumStrategy::new();
        let context = create_market_context(48000.0, 48100.0, 1000000.0, 0.2);
        
        let result = strategy.generate_signal(&context, None).await;
        assert!(result.is_ok());
        
        match result.unwrap() {
            Signal::Hold { reason } => {
                assert_eq!(reason, "Insufficient data for analysis");
            }
            _ => panic!("Expected Hold signal"),
        }
    }

    #[tokio::test]
    async fn test_stop_loss_long_position() {
        let mut strategy = MomentumStrategy::new();
        let config = create_test_config();
        strategy.initialize(config).await.unwrap();
        
        let context = create_market_context(47000.0, 47100.0, 1000000.0, 0.2);
        let position = create_position(48000.0, 47000.0, PositionSide::Long);
        
        let result = strategy.generate_signal(&context, Some(&position)).await;
        assert!(result.is_ok());
        
        match result.unwrap() {
            Signal::Sell { confidence, size, reason } => {
                assert_eq!(confidence, 1.0);
                assert_eq!(size, 1.0);
                assert_eq!(reason, "Stop loss triggered");
            }
            _ => panic!("Expected Sell signal"),
        }
    }

    #[tokio::test]
    async fn test_stop_loss_short_position() {
        let mut strategy = MomentumStrategy::new();
        let config = create_test_config();
        strategy.initialize(config).await.unwrap();
        
        let context = create_market_context(49000.0, 49100.0, 1000000.0, 0.2);
        let position = create_position(48000.0, 49000.0, PositionSide::Short);
        
        let result = strategy.generate_signal(&context, Some(&position)).await;
        assert!(result.is_ok());
        
        match result.unwrap() {
            Signal::Sell { confidence, size, reason } => {
                assert_eq!(confidence, 1.0);
                assert_eq!(size, 1.0);
                assert_eq!(reason, "Stop loss triggered");
            }
            _ => panic!("Expected Sell signal"),
        }
    }

    #[tokio::test]
    async fn test_take_profit_long_position() {
        let mut strategy = MomentumStrategy::new();
        let config = create_test_config();
        strategy.initialize(config).await.unwrap();
        
        let context = create_market_context(50500.0, 50600.0, 1000000.0, 0.2);
        let position = create_position(48000.0, 50500.0, PositionSide::Long);
        
        let result = strategy.generate_signal(&context, Some(&position)).await;
        assert!(result.is_ok());
        
        match result.unwrap() {
            Signal::Sell { confidence, size, reason } => {
                assert_eq!(confidence, 1.0);
                assert_eq!(size, 1.0);
                assert_eq!(reason, "Take profit reached");
            }
            _ => panic!("Expected Sell signal"),
        }
    }

    #[tokio::test]
    async fn test_take_profit_short_position() {
        let mut strategy = MomentumStrategy::new();
        let config = create_test_config();
        strategy.initialize(config).await.unwrap();
        
        let context = create_market_context(45500.0, 45600.0, 1000000.0, 0.2);
        let position = create_position(48000.0, 45500.0, PositionSide::Short);
        
        let result = strategy.generate_signal(&context, Some(&position)).await;
        assert!(result.is_ok());
        
        match result.unwrap() {
            Signal::Sell { confidence, size, reason } => {
                assert_eq!(confidence, 1.0);
                assert_eq!(size, 1.0);
                assert_eq!(reason, "Take profit reached");
            }
            _ => panic!("Expected Sell signal"),
        }
    }

    #[test]
    fn test_get_metrics_empty() {
        let strategy = MomentumStrategy::new();
        let metrics = strategy.get_metrics();
        assert!(metrics.is_empty());
    }

    #[test]
    fn test_momentum_config_validation() {
        let config = MomentumConfig {
            fast_period: 10,
            slow_period: 20,
            rsi_period: 14,
            rsi_overbought: 70.0,
            rsi_oversold: 30.0,
            momentum_threshold: 0.02,
            stop_loss_pct: 0.02,
            take_profit_pct: 0.05,
        };
        
        // Validate config values
        assert!(config.fast_period < config.slow_period);
        assert!(config.rsi_overbought > config.rsi_oversold);
        assert!(config.momentum_threshold > 0.0);
        assert!(config.stop_loss_pct > 0.0);
        assert!(config.take_profit_pct > 0.0);
    }

    #[tokio::test]
    async fn test_multiple_parameter_updates() {
        let mut strategy = MomentumStrategy::new();
        let config = create_test_config();
        strategy.initialize(config).await.unwrap();
        
        // First update
        let mut params1 = HashMap::new();
        params1.insert("fast_period".to_string(), serde_json::json!(8));
        let result = strategy.update_parameters(params1).await;
        assert!(result.is_ok());
        
        // Second update
        let mut params2 = HashMap::new();
        params2.insert("slow_period".to_string(), serde_json::json!(30));
        let result = strategy.update_parameters(params2).await;
        assert!(result.is_ok());
        
        // Third update
        let mut params3 = HashMap::new();
        params3.insert("momentum_threshold".to_string(), serde_json::json!(0.025));
        let result = strategy.update_parameters(params3).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_edge_case_spread_calculation() {
        let strategy = MomentumStrategy::new();
        
        // Test exact 1% spread (boundary)
        let context = create_market_context(48000.0, 48480.0, 1000000.0, 0.2);
        let result = strategy.can_execute(&context);
        assert!(result.is_ok());
        assert!(result.unwrap());
        
        // Test just over 1% spread
        let context = create_market_context(48000.0, 48481.0, 1000000.0, 0.2);
        let result = strategy.can_execute(&context);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_edge_case_volatility() {
        let strategy = MomentumStrategy::new();
        
        // Test exact 0.5 volatility (boundary)
        let context = create_market_context(48000.0, 48100.0, 1000000.0, 0.5);
        let result = strategy.can_execute(&context);
        assert!(result.is_ok());
        assert!(result.unwrap());
        
        // Test just over 0.5 volatility
        let context = create_market_context(48000.0, 48100.0, 1000000.0, 0.50001);
        let result = strategy.can_execute(&context);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }
}