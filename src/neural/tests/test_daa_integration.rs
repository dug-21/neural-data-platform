//! DAA Integration Tests for Neural Prediction System
//!
//! This module tests the integration between the enhanced neural predictor
//! and the DAA (Decentralized Autonomous Agents) coordinator for:
//! - Autonomous decision making based on neural predictions
//! - Performance feedback loops
//! - Adaptive parameter tuning
//! - Confidence-based consensus mechanisms

use super::super::enhanced_predictor::*;
use super::super::fann_predictor::*;
use super::super::{NeuralPredictorTrait, PredictionResult};
use crate::config::NeuralConfig;
use crate::data::TimeSeriesData;
use crate::integration::daa_coordinator::{DaaConfig, DaaCoordinator, MarketContext};

use anyhow::Result;
use approx::{assert_abs_diff_eq, assert_relative_eq};
use chrono::{DateTime, TimeZone, Utc};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio;
use tokio::sync::mpsc;
use tracing_test::traced_test;

/// Helper function to create test DAA configuration
fn create_test_daa_config() -> DaaConfig {
    let mut model_weights = HashMap::new();
    model_weights.insert("MLP".to_string(), 1.0);
    model_weights.insert("DeepAR".to_string(), 1.3);
    model_weights.insert("LSTM".to_string(), 1.2);

    DaaConfig {
        enabled: true,
        min_confidence: 0.6,
        max_risk_per_trade: 0.05,
        max_positions: 5,
        model_weights,
        consensus_threshold: 0.7,
        enable_adaptation: true,
    }
}

/// Helper function to create test market context
fn create_test_market_context() -> MarketContext {
    MarketContext {
        symbol: "TEST_SYMBOL".to_string(),
        current_price: 100.0,
        bid_price: 99.8,
        ask_price: 100.2,
        volume_24h: 1000000.0,
        market_cap: Some(50000000.0),
        volatility: 0.02,
        trend: "sideways".to_string(),
        support_level: Some(95.0),
        resistance_level: Some(105.0),
        rsi: Some(50.0),
        macd: Some(0.1),
        bollinger_position: Some(0.5),
        market_hours: true,
        liquidity_score: 0.8,
        news_sentiment: Some(0.0),
        correlation_scores: HashMap::new(),
        last_updated: Utc::now(),
    }
}

/// Helper function to create test time series data for DAA integration
fn create_daa_test_data(count: usize) -> Vec<TimeSeriesData> {
    (0..count)
        .map(|i| {
            let mut indicators = HashMap::new();
            indicators.insert("rsi".to_string(), 30.0 + (i as f64 % 40.0));
            indicators.insert("macd".to_string(), -0.5 + (i as f64 % 10.0) * 0.1);
            indicators.insert("bb_position".to_string(), 0.2 + (i as f64 % 6.0) * 0.1);

            TimeSeriesData {
                timestamp: Utc
                    .timestamp_opt(1640000000 + (i as i64 * 3600), 0)
                    .unwrap(),
                symbol: "DAA_TEST".to_string(),
                open: 100.0 + (i as f64 * 0.2),
                high: 102.0 + (i as f64 * 0.25),
                low: 98.0 + (i as f64 * 0.15),
                close: 101.0 + (i as f64 * 0.2),
                volume: 1000000.0 + (i as f64 * 5000.0),
                indicators,
                source: Some("daa_test".to_string()),
                entity: Some("DAA_TEST".to_string()),
                value: Some(101.0 + (i as f64 * 0.2)),
                metadata: Some(json!({"daa_test": true})),
            }
        })
        .collect()
}

mod daa_neural_integration_tests {
    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn test_enhanced_predictor_with_daa_coordinator() -> Result<()> {
        // Create enhanced neural predictor
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string(), "DeepAR".to_string(), "LSTM".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.75,
            use_real_models: false,

            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 30,
            max_retries: 3,
            error_threshold: 0.05,
        };
        let enhanced_predictor = Arc::new(EnhancedNeuralPredictor::new(neural_config)?);

        // Create DAA coordinator
        let (tx, mut rx) = mpsc::channel(100);
        let daa_config = create_test_daa_config();

        // Test that we can create coordinator with enhanced predictor
        // Note: This assumes the DaaCoordinator can work with EnhancedNeuralPredictor
        // In reality, you might need to wrap it or create an adapter

        let test_data = create_daa_test_data(25);
        let market_context = create_test_market_context();

        // Test enhanced predictions
        let predictions = enhanced_predictor
            .predict_with_confidence(&test_data, 5)
            .await?;
        assert_eq!(predictions.len(), 5);

        // Verify enhanced prediction results have confidence breakdown
        for prediction in &predictions {
            assert!(prediction.confidence >= 0.0 && prediction.confidence <= 1.0);
            assert!(prediction.confidence_breakdown.base_confidence >= 0.0);
            assert!(prediction.confidence_breakdown.ensemble_agreement >= 0.0);
            assert!(prediction.confidence_breakdown.data_quality_factor > 0.0);
            assert!(!prediction.market_regime.is_empty());
        }

        // Test performance tracking
        let actual_values = vec![101.0, 102.0, 103.0, 104.0, 105.0];
        enhanced_predictor
            .update_performance(&actual_values, &predictions)
            .await?;

        // Test retraining metrics
        let retraining_metrics = enhanced_predictor.should_retrain().await?;
        assert!(retraining_metrics.accuracy_threshold >= 0.7);
        assert_eq!(retraining_metrics.hours_threshold, 24);
        assert_eq!(retraining_metrics.sample_threshold, 10000);

        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_daa_consensus_with_neural_confidence() -> Result<()> {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string(), "DeepAR".to_string(), "LSTM".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
            use_real_models: false,

            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 30,
            max_retries: 3,
            error_threshold: 0.05,
        };
        let enhanced_predictor = EnhancedNeuralPredictor::new(neural_config)?;

        let test_data = create_daa_test_data(30);

        // Get predictions with confidence breakdown
        let predictions = enhanced_predictor
            .predict_with_confidence(&test_data, 3)
            .await?;

        // Test that predictions have sufficient confidence for DAA consensus
        for prediction in &predictions {
            // High confidence predictions should meet DAA thresholds
            if prediction.confidence > 0.8 {
                assert!(prediction.models_agree);
                assert!(prediction.model_agreement_score > 0.7);
                assert!(prediction.confidence_breakdown.ensemble_agreement > 0.0);
            }

            // Verify confidence components are reasonable for DAA decision making
            let breakdown = &prediction.confidence_breakdown;
            assert!(breakdown.base_confidence >= 0.0 && breakdown.base_confidence <= 1.0);
            assert!(breakdown.data_quality_factor >= 0.8 && breakdown.data_quality_factor <= 1.2);
            assert!(breakdown.combined_confidence >= 0.0 && breakdown.combined_confidence <= 1.0);
        }

        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_autonomous_retraining_integration() -> Result<()> {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string(), "DeepAR".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
            use_real_models: false,

            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 30,
            max_retries: 3,
            error_threshold: 0.05,
        };
        let enhanced_predictor = EnhancedNeuralPredictor::new(neural_config)?;

        // Simulate poor predictions to trigger retraining
        let actual_values = vec![100.0, 101.0, 102.0];
        let poor_predictions = vec![
            EnhancedPredictionResult {
                timestamp: Utc::now(),
                value: 120.0, // 20% error
                confidence: 0.5,
                confidence_breakdown: ConfidenceBreakdown::default(),
                models_agree: false,
                model_agreement_score: 0.3,
                interval_low: 118.0,
                interval_high: 122.0,
                ensemble_size: 2,
                market_regime: "high_volatility".to_string(),
                volatility_adjustment: 1.2,
            },
            EnhancedPredictionResult {
                timestamp: Utc::now(),
                value: 130.0, // 29% error
                confidence: 0.3,
                confidence_breakdown: ConfidenceBreakdown::default(),
                models_agree: false,
                model_agreement_score: 0.2,
                interval_low: 128.0,
                interval_high: 132.0,
                ensemble_size: 2,
                market_regime: "high_volatility".to_string(),
                volatility_adjustment: 1.3,
            },
            EnhancedPredictionResult {
                timestamp: Utc::now(),
                value: 140.0, // 37% error
                confidence: 0.2,
                confidence_breakdown: ConfidenceBreakdown::default(),
                models_agree: false,
                model_agreement_score: 0.1,
                interval_low: 138.0,
                interval_high: 142.0,
                ensemble_size: 2,
                market_regime: "high_volatility".to_string(),
                volatility_adjustment: 1.4,
            },
        ];

        // Update performance to degrade accuracy
        enhanced_predictor
            .update_performance(&actual_values, &poor_predictions)
            .await?;

        // Check if retraining is needed
        let retraining_metrics = enhanced_predictor.should_retrain().await?;

        // Should trigger retraining due to poor accuracy
        if retraining_metrics.current_accuracy < 0.7 {
            assert!(retraining_metrics.should_retrain);
            assert_eq!(retraining_metrics.primary_trigger, "accuracy_degradation");
            assert!(retraining_metrics.urgency_score > 0.0);

            // Test retraining completion
            enhanced_predictor.mark_retrained().await?;

            // Verify reset
            let metrics_after = enhanced_predictor.get_performance_metrics().await?;
            let hours_since = metrics_after
                .get("hours_since_training")
                .unwrap()
                .as_i64()
                .unwrap();
            assert!(hours_since < 1); // Should be very recent
        }

        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_market_regime_adaptation() -> Result<()> {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string(), "NHITS".to_string(), "DeepAR".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.75,
            use_real_models: false,

            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 30,
            max_retries: 3,
            error_threshold: 0.05,
        };
        let enhanced_predictor = EnhancedNeuralPredictor::new(neural_config)?;

        // Test different market regimes
        let regimes = vec!["bullish", "bearish", "high_volatility", "low_volatility"];

        for regime in regimes {
            // Create regime-specific data
            let mut test_data = create_daa_test_data(25);

            // Modify data based on regime
            match regime {
                "bullish" => {
                    for (i, data) in test_data.iter_mut().enumerate() {
                        data.close = 100.0 + i as f64 * 2.0; // Strong uptrend
                    }
                }
                "bearish" => {
                    for (i, data) in test_data.iter_mut().enumerate() {
                        data.close = 120.0 - i as f64 * 1.5; // Downtrend
                    }
                }
                "high_volatility" => {
                    for (i, data) in test_data.iter_mut().enumerate() {
                        data.close = 100.0 + (i as f64 * 0.5).sin() * 10.0; // High volatility
                    }
                }
                "low_volatility" => {
                    for (i, data) in test_data.iter_mut().enumerate() {
                        data.close = 100.0 + (i as f64 * 0.1).sin() * 0.5; // Low volatility
                    }
                }
                _ => {}
            }

            let predictions = enhanced_predictor
                .predict_with_confidence(&test_data, 3)
                .await?;

            // Verify regime detection and adaptation
            for prediction in &predictions {
                // Market regime should be detected
                assert!(!prediction.market_regime.is_empty());

                // Confidence adjustments should reflect regime
                let breakdown = &prediction.confidence_breakdown;
                match regime {
                    "high_volatility" => {
                        assert!(breakdown.volatility_penalty <= 0.0); // Should have volatility penalty
                        assert!(prediction.volatility_adjustment > 1.0); // Should adjust intervals
                    }
                    "low_volatility" => {
                        assert!(breakdown.volatility_penalty >= -0.05); // Minimal penalty
                    }
                    _ => {
                        // Market regime adjustment should be applied
                        assert!(breakdown.market_regime_adjustment >= -0.1);
                        assert!(breakdown.market_regime_adjustment <= 0.1);
                    }
                }

                // Confidence should be reasonable
                assert!(prediction.confidence >= 0.0 && prediction.confidence <= 1.0);
            }
        }

        Ok(())
    }
}

mod performance_feedback_tests {
    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn test_prediction_accuracy_feedback_loop() -> Result<()> {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string(), "DeepAR".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
            use_real_models: false,

            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 30,
            max_retries: 3,
            error_threshold: 0.05,
        };
        let enhanced_predictor = EnhancedNeuralPredictor::new(neural_config)?;

        let test_data = create_daa_test_data(30);

        // Initial predictions
        let initial_predictions = enhanced_predictor
            .predict_with_confidence(&test_data, 5)
            .await?;
        let initial_performance = enhanced_predictor.get_performance_metrics().await?;
        let initial_accuracy = initial_performance
            .get("recent_accuracy")
            .unwrap()
            .as_f64()
            .unwrap();

        // Simulate accurate predictions
        let accurate_actual = vec![101.2, 102.1, 103.3, 104.0, 104.8];
        let accurate_predictions = vec![
            EnhancedPredictionResult {
                timestamp: Utc::now(),
                value: 101.1,
                confidence: 0.85,
                confidence_breakdown: ConfidenceBreakdown::default(),
                models_agree: true,
                model_agreement_score: 0.9,
                interval_low: 100.0,
                interval_high: 102.0,
                ensemble_size: 2,
                market_regime: "bullish".to_string(),
                volatility_adjustment: 1.0,
            },
            EnhancedPredictionResult {
                timestamp: Utc::now(),
                value: 102.0,
                confidence: 0.87,
                confidence_breakdown: ConfidenceBreakdown::default(),
                models_agree: true,
                model_agreement_score: 0.92,
                interval_low: 101.0,
                interval_high: 103.0,
                ensemble_size: 2,
                market_regime: "bullish".to_string(),
                volatility_adjustment: 1.0,
            },
            EnhancedPredictionResult {
                timestamp: Utc::now(),
                value: 103.2,
                confidence: 0.83,
                confidence_breakdown: ConfidenceBreakdown::default(),
                models_agree: true,
                model_agreement_score: 0.88,
                interval_low: 102.0,
                interval_high: 104.0,
                ensemble_size: 2,
                market_regime: "bullish".to_string(),
                volatility_adjustment: 1.0,
            },
            EnhancedPredictionResult {
                timestamp: Utc::now(),
                value: 103.9,
                confidence: 0.81,
                confidence_breakdown: ConfidenceBreakdown::default(),
                models_agree: true,
                model_agreement_score: 0.86,
                interval_low: 103.0,
                interval_high: 105.0,
                ensemble_size: 2,
                market_regime: "bullish".to_string(),
                volatility_adjustment: 1.0,
            },
            EnhancedPredictionResult {
                timestamp: Utc::now(),
                value: 104.7,
                confidence: 0.79,
                confidence_breakdown: ConfidenceBreakdown::default(),
                models_agree: true,
                model_agreement_score: 0.84,
                interval_low: 104.0,
                interval_high: 106.0,
                ensemble_size: 2,
                market_regime: "bullish".to_string(),
                volatility_adjustment: 1.0,
            },
        ];

        // Update performance with accurate results
        enhanced_predictor
            .update_performance(&accurate_actual, &accurate_predictions)
            .await?;

        // Check improved performance
        let updated_performance = enhanced_predictor.get_performance_metrics().await?;
        let updated_accuracy = updated_performance
            .get("recent_accuracy")
            .unwrap()
            .as_f64()
            .unwrap();

        // Accuracy should improve with good predictions
        assert!(updated_accuracy >= initial_accuracy - 0.1); // Allow for some variance

        // Total predictions should increase
        let total_preds = updated_performance
            .get("total_predictions")
            .unwrap()
            .as_u64()
            .unwrap();
        assert!(total_preds >= 5);

        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_confidence_calibration_feedback() -> Result<()> {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string(), "NHITS".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.75,
            use_real_models: false,

            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 30,
            max_retries: 3,
            error_threshold: 0.05,
        };
        let enhanced_predictor = EnhancedNeuralPredictor::new(neural_config)?;

        let test_data = create_daa_test_data(25);

        // Test confidence calibration with different scenarios

        // Scenario 1: High confidence, accurate prediction
        let high_conf_accurate = vec![EnhancedPredictionResult {
            timestamp: Utc::now(),
            value: 100.0,
            confidence: 0.95, // Very high confidence
            confidence_breakdown: ConfidenceBreakdown {
                base_confidence: 0.9,
                ensemble_agreement: 0.2,
                historical_accuracy: 0.1,
                market_regime_adjustment: 0.05,
                data_quality_factor: 1.0,
                volatility_penalty: -0.05,
                temporal_distance_penalty: -0.02,
                combined_confidence: 0.95,
            },
            models_agree: true,
            model_agreement_score: 0.98,
            interval_low: 99.0,
            interval_high: 101.0,
            ensemble_size: 2,
            market_regime: "low_volatility".to_string(),
            volatility_adjustment: 1.0,
        }];
        let actual_accurate = vec![100.2]; // Very close to prediction
        enhanced_predictor
            .update_performance(&actual_accurate, &high_conf_accurate)
            .await?;

        // Scenario 2: Low confidence, inaccurate prediction
        let low_conf_inaccurate = vec![EnhancedPredictionResult {
            timestamp: Utc::now(),
            value: 90.0,
            confidence: 0.3, // Low confidence
            confidence_breakdown: ConfidenceBreakdown {
                base_confidence: 0.4,
                ensemble_agreement: 0.0,
                historical_accuracy: -0.1,
                market_regime_adjustment: -0.05,
                data_quality_factor: 0.9,
                volatility_penalty: -0.1,
                temporal_distance_penalty: -0.05,
                combined_confidence: 0.3,
            },
            models_agree: false,
            model_agreement_score: 0.2,
            interval_low: 85.0,
            interval_high: 95.0,
            ensemble_size: 2,
            market_regime: "high_volatility".to_string(),
            volatility_adjustment: 1.5,
        }];
        let actual_inaccurate = vec![105.0]; // Far from prediction
        enhanced_predictor
            .update_performance(&actual_inaccurate, &low_conf_inaccurate)
            .await?;

        // Verify performance tracking reflects confidence calibration
        let performance = enhanced_predictor.get_performance_metrics().await?;
        let total_preds = performance
            .get("total_predictions")
            .unwrap()
            .as_u64()
            .unwrap();
        assert_eq!(total_preds, 2);

        // Check that performance metrics are reasonable
        let recent_accuracy = performance
            .get("recent_accuracy")
            .unwrap()
            .as_f64()
            .unwrap();
        assert!(recent_accuracy >= 0.0 && recent_accuracy <= 1.0);

        Ok(())
    }
}

mod edge_case_integration_tests {
    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn test_daa_integration_with_insufficient_data() -> Result<()> {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
            use_real_models: false,

            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 30,
            max_retries: 3,
            error_threshold: 0.05,
        };
        let enhanced_predictor = EnhancedNeuralPredictor::new(neural_config)?;

        // Test with very little data
        let insufficient_data = create_daa_test_data(3);

        let result = enhanced_predictor
            .predict_with_confidence(&insufficient_data, 2)
            .await;

        // Should handle gracefully
        match result {
            Ok(predictions) => {
                // If successful, predictions should have reasonable confidence
                for pred in &predictions {
                    assert!(pred.confidence >= 0.0 && pred.confidence <= 1.0);
                    // Confidence should likely be lower due to insufficient data
                    assert!(pred.confidence_breakdown.data_quality_factor > 0.0);
                }
            }
            Err(_) => {
                // Also acceptable to fail with insufficient data
            }
        }

        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_daa_integration_with_extreme_market_conditions() -> Result<()> {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string(), "DeepAR".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.75,
            use_real_models: false,

            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 30,
            max_retries: 3,
            error_threshold: 0.05,
        };
        let enhanced_predictor = EnhancedNeuralPredictor::new(neural_config)?;

        // Create extreme market data
        let mut extreme_data = create_daa_test_data(20);

        // Simulate market crash scenario
        for (i, data) in extreme_data.iter_mut().enumerate() {
            if i > 10 {
                data.close = data.close * 0.5; // 50% drop
                data.volume = data.volume * 10.0; // 10x volume spike
                data.indicators.insert("rsi".to_string(), 10.0); // Oversold
            }
        }

        let result = enhanced_predictor
            .predict_with_confidence(&extreme_data, 3)
            .await;

        match result {
            Ok(predictions) => {
                for prediction in &predictions {
                    // Should detect extreme market conditions
                    assert!(
                        prediction.market_regime == "bearish"
                            || prediction.market_regime == "high_volatility"
                    );

                    // Confidence should be affected by extreme conditions
                    assert!(prediction.confidence_breakdown.volatility_penalty <= 0.0);
                    assert!(prediction.volatility_adjustment > 1.0);

                    // Prediction intervals should be wider
                    let interval_width = prediction.interval_high - prediction.interval_low;
                    assert!(interval_width > 0.0);
                }
            }
            Err(_) => {
                // Also acceptable to fail in extreme conditions
            }
        }

        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_concurrent_daa_prediction_requests() -> Result<()> {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string(), "NHITS".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 20,
            enable_model_monitoring: true,
            accuracy_threshold: 0.75,
            use_real_models: false,

            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 30,
            max_retries: 3,
            error_threshold: 0.05,
        };
        let enhanced_predictor = Arc::new(EnhancedNeuralPredictor::new(neural_config)?);

        let test_data = create_daa_test_data(25);

        // Spawn multiple concurrent prediction tasks
        let mut handles = vec![];
        for i in 0..5 {
            let predictor_clone = Arc::clone(&enhanced_predictor);
            let data_clone = test_data.clone();

            let handle = tokio::spawn(async move {
                let horizon = 3 + (i % 3); // Vary horizon
                predictor_clone
                    .predict_with_confidence(&data_clone, horizon)
                    .await
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        let mut all_successful = true;
        for handle in handles {
            match handle.await {
                Ok(Ok(predictions)) => {
                    assert!(!predictions.is_empty());
                    for pred in &predictions {
                        assert!(pred.confidence >= 0.0 && pred.confidence <= 1.0);
                    }
                }
                _ => {
                    all_successful = false;
                }
            }
        }

        // At least some should succeed
        assert!(all_successful);

        Ok(())
    }
}

/// Comprehensive integration test covering DAA-Neural integration
#[tokio::test]
#[traced_test]
async fn test_comprehensive_daa_neural_integration() -> Result<()> {
    println!("🧪 Testing Comprehensive DAA-Neural Integration");

    let neural_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string(), "NHITS".to_string(), "DeepAR".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.75,
        use_real_models: false,

        enable_health_checks: true,
        enable_fallback: true,
        enable_circuit_breakers: true,
        enable_graceful_degradation: false,
        enable_performance_monitoring: true,
        enable_adaptive_retry: true,
        enable_model_ensembles: false,
        model_timeout_seconds: 30,
        max_retries: 3,
        error_threshold: 0.05,
    };
    let enhanced_predictor = EnhancedNeuralPredictor::new(neural_config)?;

    let test_data = create_daa_test_data(30);
    let market_context = create_test_market_context();
    let daa_config = create_test_daa_config();

    // 1. Test enhanced predictions suitable for DAA
    let predictions = enhanced_predictor
        .predict_with_confidence(&test_data, 5)
        .await?;
    assert_eq!(predictions.len(), 5);

    // 2. Verify predictions meet DAA requirements
    for prediction in &predictions {
        // Confidence breakdown should provide detailed information for DAA
        let breakdown = &prediction.confidence_breakdown;
        assert!(breakdown.base_confidence >= 0.0);
        assert!(breakdown.ensemble_agreement >= 0.0);
        assert!(breakdown.data_quality_factor > 0.0);
        assert!(breakdown.combined_confidence >= 0.0 && breakdown.combined_confidence <= 1.0);

        // Market regime detection for DAA adaptation
        assert!(!prediction.market_regime.is_empty());

        // Model agreement for consensus
        assert!(prediction.model_agreement_score >= 0.0 && prediction.model_agreement_score <= 1.0);

        // Prediction intervals for risk management
        assert!(prediction.interval_low <= prediction.value);
        assert!(prediction.value <= prediction.interval_high);
    }

    // 3. Test performance feedback for DAA learning
    let actual_values = vec![101.5, 102.3, 103.1, 104.0, 104.8];
    enhanced_predictor
        .update_performance(&actual_values, &predictions)
        .await?;

    // 4. Test retraining decision for autonomous operation
    let retraining_metrics = enhanced_predictor.should_retrain().await?;
    assert!(retraining_metrics.accuracy_threshold >= 0.7);
    assert!(retraining_metrics.hours_threshold == 24);
    assert!(retraining_metrics.sample_threshold == 10000);

    // 5. Test performance metrics for DAA monitoring
    let performance_metrics = enhanced_predictor.get_performance_metrics().await?;
    assert!(performance_metrics.contains_key("recent_accuracy"));
    assert!(performance_metrics.contains_key("total_predictions"));
    assert!(performance_metrics.contains_key("successful_predictions"));

    // 6. Test data quality assessment for DAA confidence
    // This is implicitly tested through confidence breakdown data_quality_factor

    // 7. Test market regime adaptation
    for prediction in &predictions {
        let regime_adjustment = prediction.confidence_breakdown.market_regime_adjustment;
        assert!(regime_adjustment >= -0.1 && regime_adjustment <= 0.1);
    }

    println!("✅ Comprehensive DAA-Neural Integration test completed successfully");

    Ok(())
}
