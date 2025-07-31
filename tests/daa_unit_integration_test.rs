//! DAA Unit and Integration Tests
//!
//! Focused tests for DAA components that can run without external dependencies.
//! These tests ensure the integration between DAA coordinator, neural predictor,
//! and decision-making logic works correctly.

use anyhow::Result;
use async_trait::async_trait;
use autonomous_platform::{
    data::TimeSeriesData,
    integration::daa_coordinator::{DaaConfig, DaaCoordinator, RiskAssessment, TradingAction},
    neural::{NeuralConfig, NeuralPredictor},
    strategies::{
        MarketContext, Position, PositionSide, Signal, StrategyConfig, StrategyError,
        TradingStrategy,
    },
};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

/// Mock trading strategy for testing
struct MockStrategy {
    name: String,
    signal: Signal,
    should_fail: bool,
}

#[async_trait]
impl TradingStrategy for MockStrategy {
    fn name(&self) -> &str {
        &self.name
    }

    async fn initialize(&mut self, _config: StrategyConfig) -> Result<(), StrategyError> {
        if self.should_fail {
            Err(StrategyError::Initialization(
                "Mock initialization failure".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    async fn generate_signal(
        &self,
        _market_context: &MarketContext,
        _current_position: Option<&Position>,
    ) -> Result<Signal, StrategyError> {
        if self.should_fail {
            Err(StrategyError::Execution(
                "Mock signal generation failure".to_string(),
            ))
        } else {
            Ok(self.signal.clone())
        }
    }

    async fn update_parameters(
        &mut self,
        _parameters: HashMap<String, serde_json::Value>,
    ) -> Result<(), StrategyError> {
        Ok(())
    }

    fn get_metrics(&self) -> HashMap<String, f64> {
        let mut metrics = HashMap::new();
        metrics.insert("mock_metric".to_string(), 1.0);
        metrics
    }

    fn can_execute(&self, _context: &MarketContext) -> Result<bool, StrategyError> {
        Ok(!self.should_fail)
    }
}

/// Helper to create test time series data
fn create_test_time_series(symbol: &str, base_price: f64, count: usize) -> Vec<TimeSeriesData> {
    let mut data = Vec::new();
    let now = Utc::now();

    for i in 0..count {
        let price_variation = ((i as f64) * 0.1).sin() * 100.0;
        let price = base_price + price_variation;

        data.push(TimeSeriesData {
            symbol: symbol.to_string(),
            timestamp: now - chrono::Duration::minutes((count - i) as i64),
            open: price - 10.0,
            high: price + 15.0,
            low: price - 15.0,
            close: price,
            volume: 1000.0 + (i as f64 * 50.0),
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some(symbol.to_string()),
            value: Some(price),
            metadata: None,
        });
    }

    data
}

#[cfg(test)]
mod daa_unit_tests {
    use super::*;

    /// Test 1: DAA Coordinator Initialization
    #[tokio::test]
    async fn test_daa_coordinator_initialization() {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 5,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };

        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, _rx) = mpsc::channel(100);

        let config = DaaConfig::default();
        assert_eq!(config.enabled, true);
        assert_eq!(config.min_confidence, 0.75);
        assert_eq!(config.max_risk_per_trade, 0.02);
        assert_eq!(config.max_positions, 5);

        let coordinator = DaaCoordinator::new(config.clone(), neural_predictor, tx, create_test_market_hours());

        // Coordinator is ready to use
    }

    /// Test 2: Risk Assessment Logic
    #[tokio::test]
    async fn test_risk_assessment() {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 5,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };

        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, _rx) = mpsc::channel(100);
        let coordinator = DaaCoordinator::new(DaaConfig::default(), neural_predictor, tx, create_test_market_hours());

        // Test different market conditions
        let test_cases = vec![
            // Normal market
            (
                MarketContext {
                    symbol: "BTC/USDT".to_string(),
                    current_price: 50000.0,
                    bid: 49990.0,
                    ask: 50010.0,
                    volume_24h: 1000000.0,
                    volatility: 0.02,
                    timestamp: Utc::now().timestamp(),
                },
                "normal",
            ),
            // High volatility
            (
                MarketContext {
                    symbol: "BTC/USDT".to_string(),
                    current_price: 50000.0,
                    bid: 49900.0,
                    ask: 50100.0,
                    volume_24h: 1000000.0,
                    volatility: 0.10,
                    timestamp: Utc::now().timestamp(),
                },
                "high_volatility",
            ),
            // Low liquidity
            (
                MarketContext {
                    symbol: "BTC/USDT".to_string(),
                    current_price: 50000.0,
                    bid: 49500.0,
                    ask: 50500.0,
                    volume_24h: 10000.0,
                    volatility: 0.05,
                    timestamp: Utc::now().timestamp(),
                },
                "low_liquidity",
            ),
        ];

        for (market, scenario) in test_cases {
            let historical_data = create_test_time_series(&market.symbol, market.current_price, 20);
            let decision = coordinator
                .make_decision(&market, None, &historical_data)
                .await
                .unwrap();

            // Verify risk assessment makes sense
            assert!(decision.risk_assessment.market_risk >= 0.0);
            assert!(decision.risk_assessment.market_risk <= 1.0);
            assert!(decision.risk_assessment.volatility_adjusted_size > 0.0);

            // High volatility should reduce position size
            if scenario == "high_volatility" {
                assert!(decision.risk_assessment.volatility_adjusted_size < 0.02);
            }

            println!(
                "Risk assessment for {}: {:?}",
                scenario, decision.risk_assessment
            );
        }
    }

    /// Test 3: Strategy Integration
    #[tokio::test]
    async fn test_strategy_integration() {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 5,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };

        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, mut rx) = mpsc::channel(100);
        let coordinator = DaaCoordinator::new(DaaConfig::default(), neural_predictor, tx, create_test_market_hours());

        // Register multiple strategies with different signals
        let strategies = vec![
            (
                "bullish",
                Signal::Buy {
                    confidence: 0.8,
                    size: Some(0.1),
                    reason: "Strong uptrend".to_string(),
                },
                false,
            ),
            (
                "bearish",
                Signal::Sell {
                    confidence: 0.7,
                    size: Some(0.05),
                    reason: "Resistance hit".to_string(),
                },
                false,
            ),
            (
                "neutral",
                Signal::Hold {
                    reason: "Waiting for confirmation".to_string(),
                },
                false,
            ),
        ];

        for (name, signal, should_fail) in strategies {
            let strategy = Box::new(MockStrategy {
                name: name.to_string(),
                signal,
                should_fail,
            });
            coordinator
                .register_strategy(name.to_string(), strategy)
                .await;
        }

        let market = MarketContext {
            symbol: "BTC/USDT".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1000000.0,
            volatility: 0.02,
            timestamp: Utc::now().timestamp(),
        };

        let historical_data = create_test_time_series(&market.symbol, market.current_price, 50);
        let decision = coordinator
            .make_decision(&market, None, &historical_data)
            .await
            .unwrap();

        // Verify decision reasoning includes strategy signals
        assert!(decision
            .reasoning
            .iter()
            .any(|r| r.contains("bullish votes BUY")));
        assert!(decision
            .reasoning
            .iter()
            .any(|r| r.contains("bearish votes SELL")));
        assert!(decision
            .reasoning
            .iter()
            .any(|r| r.contains("neutral votes HOLD")));

        // Verify decision was sent through channel
        let received = timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(received.is_ok());
        assert!(received.unwrap().is_some());
    }

    /// Test 4: Decision Making with Positions
    #[tokio::test]
    async fn test_decision_with_positions() {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 5,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };

        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, mut rx) = mpsc::channel(100);
        let coordinator = DaaCoordinator::new(DaaConfig::default(), neural_predictor, tx, create_test_market_hours());

        // Test different position scenarios
        let positions = vec![
            // Profitable position
            Position {
                symbol: "BTC/USDT".to_string(),
                side: PositionSide::Long,
                size: 0.1,
                entry_price: 48000.0,
                current_price: 50000.0,
                unrealized_pnl: 200.0,
                timestamp: Utc::now().timestamp() - 3600,
            },
            // Losing position
            Position {
                symbol: "BTC/USDT".to_string(),
                side: PositionSide::Long,
                size: 0.1,
                entry_price: 52000.0,
                current_price: 50000.0,
                unrealized_pnl: -200.0,
                timestamp: Utc::now().timestamp() - 7200,
            },
        ];

        let market = MarketContext {
            symbol: "BTC/USDT".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1000000.0,
            volatility: 0.02,
            timestamp: Utc::now().timestamp(),
        };

        for position in positions {
            let historical_data = create_test_time_series(&market.symbol, market.current_price, 30);
            let decision = coordinator
                .make_decision(&market, Some(&position), &historical_data)
                .await
                .unwrap();

            println!(
                "Decision for position with PnL {}: {:?}",
                position.unrealized_pnl, decision.action
            );

            // Verify position-aware decisions
            match &decision.action {
                TradingAction::Sell { size, .. } => {
                    // Should match position size
                    assert_eq!(*size, position.size);
                }
                TradingAction::AdjustPosition { .. } => {
                    // Valid for risk management
                }
                TradingAction::Hold { reason } => {
                    assert!(!reason.is_empty());
                }
                _ => {}
            }

            // Verify decision was sent
            let _ = rx.recv().await;
        }
    }

    /// Test 5: Adaptation Mechanism
    #[tokio::test]
    async fn test_adaptation_mechanism() {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 5,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };

        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, _rx) = mpsc::channel(100);

        let mut config = DaaConfig::default();
        config.enable_adaptation = true;
        let coordinator = DaaCoordinator::new(config, neural_predictor, tx, create_test_market_hours());

        let market = MarketContext {
            symbol: "BTC/USDT".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1000000.0,
            volatility: 0.02,
            timestamp: Utc::now().timestamp(),
        };

        let historical_data = create_test_time_series(&market.symbol, market.current_price, 50);

        // Make enough decisions to trigger adaptation
        for i in 0..15 {
            let mut market_variation = market.clone();
            market_variation.current_price += (i as f64 * 100.0);

            let decision = coordinator
                .make_decision(&market_variation, None, &historical_data)
                .await
                .unwrap();

            // After 10 decisions, adaptation should occur
            if i > 10 {
                assert!(decision.adapted_parameters.is_some());
                let params = decision.adapted_parameters.as_ref().unwrap();
                assert!(params.contains_key("min_confidence"));
            }
        }

        // Verify metrics have been updated
        let metrics = coordinator.get_metrics().await;
        println!("Metrics after adaptation: {:?}", metrics);
    }

    /// Test 6: Concurrent Decision Making
    #[tokio::test]
    async fn test_concurrent_decisions() {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };

        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, mut rx) = mpsc::channel(1000);
        let coordinator = Arc::new(DaaCoordinator::new(
            DaaConfig::default(),
            neural_predictor,
            tx,
        ));

        // Spawn multiple concurrent tasks
        let mut handles = vec![];
        let symbols = vec!["BTC/USDT", "ETH/USDT", "SOL/USDT", "ADA/USDT", "DOT/USDT"];

        for (i, symbol) in symbols.iter().enumerate() {
            let coordinator_clone = coordinator.clone();
            let symbol = symbol.to_string();

            let handle = tokio::spawn(async move {
                let market = MarketContext {
                    symbol: symbol.clone(),
                    current_price: 1000.0 * (i + 1) as f64,
                    bid: 1000.0 * (i + 1) as f64 - 5.0,
                    ask: 1000.0 * (i + 1) as f64 + 5.0,
                    volume_24h: 100000.0,
                    volatility: 0.02 + (i as f64 * 0.01),
                    timestamp: Utc::now().timestamp(),
                };

                let historical_data = create_test_time_series(&symbol, market.current_price, 20);

                coordinator_clone
                    .make_decision(&market, None, &historical_data)
                    .await
            });

            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }

        // Verify all decisions were received
        let mut received_count = 0;
        while let Ok(Some(_)) = timeout(Duration::from_millis(100), rx.recv()).await {
            received_count += 1;
        }

        assert_eq!(received_count, symbols.len());
    }

    /// Test 7: Error Handling and Recovery
    #[tokio::test]
    async fn test_error_handling() {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 5,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };

        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, _rx) = mpsc::channel(100);
        let coordinator = DaaCoordinator::new(DaaConfig::default(), neural_predictor, tx, create_test_market_hours());

        // Register a failing strategy
        let failing_strategy = Box::new(MockStrategy {
            name: "failing".to_string(),
            signal: Signal::Buy {
                confidence: 0.8,
                size: Some(0.1),
                reason: "Test".to_string(),
            },
            should_fail: true,
        });

        coordinator
            .register_strategy("failing".to_string(), failing_strategy)
            .await;

        // Also register a working strategy
        let working_strategy = Box::new(MockStrategy {
            name: "working".to_string(),
            signal: Signal::Hold {
                reason: "Normal operation".to_string(),
            },
            should_fail: false,
        });

        coordinator
            .register_strategy("working".to_string(), working_strategy)
            .await;

        let market = MarketContext {
            symbol: "BTC/USDT".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1000000.0,
            volatility: 0.02,
            timestamp: Utc::now().timestamp(),
        };

        let historical_data = create_test_time_series(&market.symbol, market.current_price, 20);

        // Decision should still be made despite one failing strategy
        let decision = coordinator
            .make_decision(&market, None, &historical_data)
            .await
            .unwrap();

        // Verify the working strategy's signal is included
        assert!(decision
            .reasoning
            .iter()
            .any(|r| r.contains("working votes HOLD")));
    }

    /// Test 8: Neural Consensus Calculation
    #[tokio::test]
    async fn test_neural_consensus() {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string(), "TCN".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 5,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };

        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, _rx) = mpsc::channel(100);

        let mut config = DaaConfig::default();
        // Set specific model weights
        config.model_weights.insert("MLP".to_string(), 1.0);
        config.model_weights.insert("TCN".to_string(), 1.5);

        let coordinator = DaaCoordinator::new(config, neural_predictor, tx, create_test_market_hours());

        let market = MarketContext {
            symbol: "BTC/USDT".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1000000.0,
            volatility: 0.02,
            timestamp: Utc::now().timestamp(),
        };

        let historical_data = create_test_time_series(&market.symbol, market.current_price, 100);
        let decision = coordinator
            .make_decision(&market, None, &historical_data)
            .await
            .unwrap();

        // Verify neural consensus was calculated
        assert!(!decision.neural_consensus.is_empty());

        // Check that model weights were applied
        for (model, _signal) in &decision.neural_consensus {
            println!(
                "Neural consensus for {}: {:?}",
                model,
                decision.neural_consensus.get(model)
            );
        }
    }

    /// Test 9: Decision Reasoning
    #[tokio::test]
    async fn test_decision_reasoning() {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 5,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };

        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, _rx) = mpsc::channel(100);
        let coordinator = DaaCoordinator::new(DaaConfig::default(), neural_predictor, tx, create_test_market_hours());

        // Register strategies for comprehensive reasoning
        let buy_strategy = Box::new(MockStrategy {
            name: "momentum".to_string(),
            signal: Signal::Buy {
                confidence: 0.85,
                size: Some(0.1),
                reason: "Strong momentum".to_string(),
            },
            should_fail: false,
        });

        let hold_strategy = Box::new(MockStrategy {
            name: "mean_reversion".to_string(),
            signal: Signal::Hold {
                reason: "Waiting for reversion".to_string(),
            },
            should_fail: false,
        });

        coordinator
            .register_strategy("momentum".to_string(), buy_strategy)
            .await;
        coordinator
            .register_strategy("mean_reversion".to_string(), hold_strategy)
            .await;

        let market = MarketContext {
            symbol: "BTC/USDT".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1000000.0,
            volatility: 0.02,
            timestamp: Utc::now().timestamp(),
        };

        let historical_data = create_test_time_series(&market.symbol, market.current_price, 50);
        let decision = coordinator
            .make_decision(&market, None, &historical_data)
            .await
            .unwrap();

        // Verify comprehensive reasoning
        assert!(!decision.reasoning.is_empty());
        assert!(decision
            .reasoning
            .iter()
            .any(|r| r.contains("Neural consensus signal")));
        assert!(decision
            .reasoning
            .iter()
            .any(|r| r.contains("momentum votes BUY")));
        assert!(decision
            .reasoning
            .iter()
            .any(|r| r.contains("mean_reversion votes HOLD")));
        assert!(decision
            .reasoning
            .iter()
            .any(|r| r.contains("Risk assessment")));

        println!("Decision reasoning:");
        for reason in &decision.reasoning {
            println!("  - {}", reason);
        }
    }

    /// Test 10: Performance Metrics Tracking
    #[tokio::test]
    async fn test_performance_metrics() {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 5,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };

        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, _rx) = mpsc::channel(100);
        let coordinator = DaaCoordinator::new(DaaConfig::default(), neural_predictor, tx, create_test_market_hours());

        let market = MarketContext {
            symbol: "BTC/USDT".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1000000.0,
            volatility: 0.02,
            timestamp: Utc::now().timestamp(),
        };

        let historical_data = create_test_time_series(&market.symbol, market.current_price, 50);

        // Make multiple decisions to accumulate metrics
        for i in 0..5 {
            let mut market_var = market.clone();
            market_var.current_price += (i as f64 * 100.0);

            let _ = coordinator
                .make_decision(&market_var, None, &historical_data)
                .await
                .unwrap();
        }

        // Check accumulated metrics
        let metrics = coordinator.get_metrics().await;
        println!("Performance metrics after 5 decisions: {:?}", metrics);
    }
}

#[cfg(test)]
mod integration_stress_tests {
    use super::*;

    /// Stress test: Rapid decision making
    #[tokio::test]
    async fn test_rapid_decisions() {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 20,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };

        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, mut rx) = mpsc::channel(10000);
        let coordinator = Arc::new(DaaCoordinator::new(
            DaaConfig::default(),
            neural_predictor,
            tx,
        ));

        let start = std::time::Instant::now();
        let num_decisions = 50;

        // Fire off rapid decisions
        let mut handles = vec![];
        for i in 0..num_decisions {
            let coordinator_clone = coordinator.clone();

            let handle = tokio::spawn(async move {
                let market = MarketContext {
                    symbol: format!("TEST{}/USDT", i % 5),
                    current_price: 1000.0 + (i as f64),
                    bid: 999.0 + (i as f64),
                    ask: 1001.0 + (i as f64),
                    volume_24h: 100000.0,
                    volatility: 0.02,
                    timestamp: Utc::now().timestamp(),
                };

                let data = create_test_time_series(&market.symbol, market.current_price, 10);

                coordinator_clone.make_decision(&market, None, &data).await
            });

            handles.push(handle);
        }

        // Wait for all decisions
        for handle in handles {
            assert!(handle.await.unwrap().is_ok());
        }

        let elapsed = start.elapsed();

        // Verify all decisions were received
        let mut received = 0;
        while let Ok(Some(_)) = timeout(Duration::from_millis(10), rx.recv()).await {
            received += 1;
        }

        assert_eq!(received, num_decisions);

        let decisions_per_second = num_decisions as f64 / elapsed.as_secs_f64();
        println!("Performance: {:.2} decisions/second", decisions_per_second);
        println!("Total time: {:?} for {} decisions", elapsed, num_decisions);

        // Should handle at least 10 decisions per second
        assert!(decisions_per_second > 10.0);
    }
}
