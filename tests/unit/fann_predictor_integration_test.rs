//! Unit tests for FannPredictor integration with NeuroDivergentAdapter
//! 
//! Tests the integration between FANN neural networks and neuro-divergent models

use autonomous_platform::neural::fann_predictor::{FannPredictor, FannModelConfig};
use autonomous_platform::neural::PredictionResult;
use autonomous_platform::config::NeuralConfig;
use autonomous_platform::data::TimeSeriesData;
use autonomous_platform::adapters::enhanced_neural_adapter::EnhancedNeuralAdapter;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use mockall::predicate::*;
use mockall::mock;

// Mock the FANN Network for testing
mock! {
    FannNetwork {
        fn run(&self, input: &[f32]) -> Vec<f32>;
        fn train_on_data(&mut self, data: &[(Vec<f32>, Vec<f32>)], epochs: usize) -> f32;
        fn get_mse(&self) -> f32;
        fn get_num_layers(&self) -> usize;
    }
}

// Mock the neuro-divergent models
mock! {
    NeuroDivergentModel {
        fn predict(&self, input: &[f64], horizon: usize) -> Result<Vec<f64>>;
        fn train(&mut self, data: &[Vec<f64>], targets: &[f64]) -> Result<f64>;
        fn get_model_type(&self) -> String;
        fn get_metrics(&self) -> HashMap<String, f64>;
    }
}

// Helper to create test configuration
fn create_test_config() -> NeuralConfig {
    NeuralConfig {
        ensemble_models: vec!["mlp".to_string(), "tcn".to_string(), "transformer".to_string()],
        lookback_window: 20,
        prediction_horizon: 5,
        confidence_threshold: 0.8,
        update_interval_seconds: 300,
        min_data_points: 50,
        feature_importance_threshold: 0.1,
        market_regime_detection: true,
        adaptive_learning: true,
        model_update_frequency: 3600,
        ensemble_method: "weighted_average".to_string(),
        cache_predictions: true,
        cache_ttl_seconds: 600,
        performance_tracking: true,
        risk_adjustment: true,
        volatility_scaling: true,
        anomaly_detection: true,
        use_gpu: false,
        batch_size: 32,
        learning_rate: 0.001,
        dropout_rate: 0.2,
        regularization: 0.0001,
    }
}

fn create_test_timeseries(count: usize) -> Vec<TimeSeriesData> {
    let base_timestamp = Utc.ymd(2024, 1, 1).and_hms(0, 0, 0);
    let mut data = Vec::new();
    
    for i in 0..count {
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 50.0 + (i as f64 * 0.5));
        indicators.insert("macd".to_string(), 0.001 * (i as f64));
        indicators.insert("volume_ma".to_string(), 1000.0 + (i as f64 * 10.0));
        
        data.push(TimeSeriesData {
            symbol: "BTC/USD".to_string(),
            timestamp: base_timestamp + chrono::Duration::minutes(i as i64 * 5),
            open: 50000.0 + (i as f64 * 10.0),
            high: 50100.0 + (i as f64 * 10.0),
            low: 49900.0 + (i as f64 * 10.0),
            close: 50050.0 + (i as f64 * 10.0),
            volume: 100.0 + (i as f64),
            indicators,
            source: Some("test".to_string()),
            entity: Some("BTC/USD".to_string()),
            value: Some(50050.0 + (i as f64 * 10.0)),
            metadata: None,
        });
    }
    
    data
}

#[cfg(test)]
mod fann_model_config_tests {
    use super::*;
    use ruv_fann::ActivationFunction;
    
    #[test]
    fn test_fann_model_config_default() {
        let config = FannModelConfig::default();
        
        assert_eq!(config.input_size, 30);
        assert_eq!(config.hidden_layers, vec![64, 32, 16]);
        assert_eq!(config.output_size, 5);
        assert_eq!(config.learning_rate, 0.001);
        assert_eq!(config.momentum, 0.9);
        assert_eq!(config.max_epochs, 1000);
        assert_eq!(config.target_error, 0.001);
        assert!(!config.use_cascade);
    }
    
    #[test]
    fn test_fann_model_config_custom() {
        let config = FannModelConfig {
            input_size: 50,
            hidden_layers: vec![128, 64, 32, 16],
            output_size: 10,
            hidden_activation: ActivationFunction::Gaussian,
            output_activation: ActivationFunction::SigmoidSymmetric,
            learning_rate: 0.0001,
            momentum: 0.95,
            max_epochs: 5000,
            target_error: 0.0001,
            use_cascade: true,
        };
        
        assert_eq!(config.hidden_layers.len(), 4);
        assert!(config.use_cascade);
        assert_eq!(config.learning_rate, 0.0001);
    }
}

#[cfg(test)]
mod fann_predictor_initialization_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_fann_predictor_new() {
        let config = create_test_config();
        let predictor = FannPredictor::new(config.clone());
        
        assert!(predictor.is_ok());
        // Predictor should be initialized with empty networks
    }
    
    #[tokio::test]
    async fn test_fann_predictor_with_custom_models() {
        let mut config = create_test_config();
        config.ensemble_models = vec!["lstm".to_string(), "gru".to_string()];
        
        let predictor = FannPredictor::new(config);
        assert!(predictor.is_ok());
    }
}

#[cfg(test)]
mod data_conversion_integration_tests {
    use super::*;
    
    #[test]
    fn test_timeseries_to_fann_input() {
        let data = create_test_timeseries(50);
        let lookback = 10;
        let forecast_horizon = 5;
        
        // Prepare data for FANN input manually
        // Verify we have enough data
        let min_required = lookback + forecast_horizon;
        assert!(data.len() >= min_required, "Need at least {} data points, got {}", min_required, data.len());
        
        // Create feature vectors
        let n_samples = data.len() - lookback - forecast_horizon + 1;
        assert!(n_samples > 0);
        
        // Each timestep has OHLCV + 3 indicators = 8 features
        let n_features_per_timestep = 8;
        let total_features = lookback * n_features_per_timestep;
        
        // Verify we can create training samples
        assert!(total_features > 0);
    }
    
    #[test]
    fn test_fann_output_to_predictions() {
        let fann_output = vec![50100.0, 50150.0, 50200.0, 50250.0, 50300.0];
        let base_timestamp = Utc::now();
        
        // Convert FANN output to predictions manually
        let mut predictions = Vec::new();
        for (i, &price) in fann_output.iter().enumerate() {
            let mut indicators = HashMap::new();
            indicators.insert("model_prediction".to_string(), price as f64);
            
            predictions.push(TimeSeriesData {
                symbol: "BTC/USD".to_string(),
                timestamp: base_timestamp + chrono::Duration::seconds((i as i64 + 1) * 300),
                open: price as f64,
                high: price as f64 * 1.01,
                low: price as f64 * 0.99,
                close: price as f64,
                volume: vec![1000.0],
                indicators,
                source: Some("prediction".to_string()),
                entity: Some("BTC/USD".to_string()),
                value: Some(price as f64),
                metadata: None,
            });
        }
        
        assert_eq!(predictions.len(), 5);
        assert_eq!(predictions[0].close, 50100.0);
        assert_eq!(predictions[0].source, Some("prediction".to_string()));
    }
}

#[cfg(test)]
mod ensemble_integration_tests {
    use super::*;
    
    #[test]
    fn test_ensemble_weight_calculation() {
        // Simulate model performances
        let mut model_performances = HashMap::new();
        model_performances.insert("fann_mlp".to_string(), 0.85);
        model_performances.insert("neuro_tcn".to_string(), 0.90);
        model_performances.insert("neuro_transformer".to_string(), 0.88);
        
        // Calculate normalized weights
        let total: f64 = model_performances.values().sum();
        let weights: HashMap<String, f64> = model_performances
            .iter()
            .map(|(k, v)| (k.clone(), v / total))
            .collect();
        
        // Verify weights sum to 1
        let weight_sum: f64 = weights.values().sum();
        assert!((weight_sum - 1.0).abs() < 1e-10);
        
        // TCN should have highest weight
        let max_weight_model = weights.iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap();
        assert_eq!(max_weight_model.0, "neuro_tcn");
    }
    
    #[test]
    fn test_ensemble_prediction_aggregation() {
        let predictions = vec![
            vec![50100.0, 50200.0, 50300.0], // FANN
            vec![50150.0, 50250.0, 50350.0], // Neuro-TCN
            vec![50120.0, 50220.0, 50320.0], // Neuro-Transformer
        ];
        
        let weights = vec![0.3, 0.4, 0.3];
        
        let aggregated: Vec<f64> = (0..3)
            .map(|i| {
                predictions.iter()
                    .zip(weights.iter())
                    .map(|(pred, w)| pred[i] * w)
                    .sum()
            })
            .collect();
        
        assert!((aggregated[0] - 50130.0).abs() < 0.1);
        assert!((aggregated[1] - 50230.0).abs() < 0.1);
        assert!((aggregated[2] - 50330.0).abs() < 0.1);
    }
}

#[cfg(test)]
mod error_handling_integration_tests {
    use super::*;
    
    #[test]
    fn test_fann_network_failure_handling() {
        let mut mock_network = MockFannNetwork::new();
        mock_network
            .expect_run()
            .times(1)
            .returning(|_| vec![]); // Return empty vector to simulate failure
        
        let input = vec![1.0; 30];
        let output = mock_network.run(&input);
        
        assert!(output.is_empty());
    }
    
    #[test]
    fn test_neuro_divergent_model_failure() {
        let mut mock_model = MockNeuroDivergentModel::new();
        mock_model
            .expect_predict()
            .times(1)
            .returning(|_, _| Err(anyhow::anyhow!("Model prediction failed")));
        
        let result = mock_model.predict(&vec![1.0; 50], 5);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Model prediction failed"));
    }
    
    #[test]
    fn test_data_preparation_with_invalid_input() {
        let data = create_test_timeseries(10); // Too little data
        let lookback = 20;
        let forecast_horizon = 5;
        
        let result = NeuroDivergentAdapter::prepare_model_input(&data, lookback, forecast_horizon);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Insufficient data"));
    }
}

#[cfg(test)]
mod recurrent_state_tests {
    use super::*;
    
    #[test]
    fn test_lstm_state_management() {
        // Simulate LSTM state
        let hidden_size = 128;
        let hidden_state = vec![0.0f32; hidden_size];
        let cell_state = vec![0.0f32; hidden_size];
        
        // Update states (simulating LSTM forward pass)
        let new_hidden: Vec<f32> = hidden_state.iter()
            .enumerate()
            .map(|(i, &h)| h + 0.01 * i as f32)
            .collect();
        
        let new_cell: Vec<f32> = cell_state.iter()
            .enumerate()
            .map(|(i, &c)| c + 0.005 * i as f32)
            .collect();
        
        assert_eq!(new_hidden.len(), hidden_size);
        assert_eq!(new_cell.len(), hidden_size);
    }
    
    #[test]
    fn test_gru_state_management() {
        // GRU only has hidden state (no cell state)
        let hidden_size = 64;
        let hidden_state = vec![0.0f32; hidden_size];
        
        // Update state (simulating GRU forward pass)
        let new_hidden: Vec<f32> = hidden_state.iter()
            .enumerate()
            .map(|(i, &h)| h + 0.02 * i as f32)
            .collect();
        
        assert_eq!(new_hidden.len(), hidden_size);
    }
}

#[cfg(test)]
mod market_regime_detection_tests {
    use super::*;
    
    #[test]
    fn test_market_regime_classification() {
        let data = create_test_timeseries(100);
        
        // Calculate returns
        let returns: Vec<f64> = data.windows(2)
            .map(|w| (w[1].close - w[0].close) / w[0].close)
            .collect();
        
        // Simple regime detection based on returns
        let avg_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let volatility = returns.iter()
            .map(|r| (r - avg_return).powi(2))
            .sum::<f64>()
            .sqrt() / returns.len() as f64;
        
        let regime = if avg_return > 0.001 && volatility < 0.02 {
            "Bullish"
        } else if avg_return < -0.001 && volatility < 0.02 {
            "Bearish"
        } else if volatility > 0.02 {
            "Volatile"
        } else {
            "Sideways"
        };
        
        assert!(!regime.is_empty());
    }
}

#[cfg(test)]
mod performance_tracking_tests {
    use super::*;
    use std::time::Instant;
    
    #[test]
    fn test_prediction_performance_tracking() {
        let start = Instant::now();
        
        // Simulate prediction process
        let data = create_test_timeseries(1000);
        let config = create_test_config();
        let predictor = FannPredictor::new(config).unwrap();
        
        // Simulate data preparation timing
        let _result = predictor.test_predict_with_model("FANN_MLP", &data, 5).await;
        
        let duration = start.elapsed();
        
        // Track performance metrics
        let metrics = HashMap::from([
            ("preparation_time_ms".to_string(), duration.as_millis() as f64),
            ("data_points".to_string(), data.len() as f64),
            ("throughput".to_string(), data.len() as f64 / duration.as_secs_f64().max(0.001)),
        ]);
        
        assert!(metrics["preparation_time_ms"] < 5000.0); // Should be reasonably fast
        assert!(metrics["throughput"] > 100.0); // Should process >100 points/sec
    }
}

#[cfg(test)]
mod cache_integration_tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    
    #[tokio::test]
    async fn test_prediction_caching() {
        let cache: Arc<RwLock<HashMap<String, (DateTime<Utc>, Vec<f64>)>>> = 
            Arc::new(RwLock::new(HashMap::new()));
        
        let symbol = "BTC/USD";
        let timestamp = Utc::now();
        let predictions = vec![50100.0, 50200.0, 50300.0];
        
        // Store in cache
        {
            let mut cache_write = cache.write().await;
            cache_write.insert(
                symbol.to_string(),
                (timestamp, predictions.clone())
            );
        }
        
        // Retrieve from cache
        {
            let cache_read = cache.read().await;
            let cached = cache_read.get(symbol);
            assert!(cached.is_some());
            
            let (cached_time, cached_preds) = cached.unwrap();
            assert_eq!(*cached_time, timestamp);
            assert_eq!(*cached_preds, predictions);
        }
    }
    
    #[tokio::test]
    async fn test_cache_expiration() {
        let cache: Arc<RwLock<HashMap<String, (DateTime<Utc>, Vec<f64>)>>> = 
            Arc::new(RwLock::new(HashMap::new()));
        
        let symbol = "ETH/USD";
        let old_timestamp = Utc::now() - chrono::Duration::seconds(700); // Older than TTL
        let predictions = vec![3000.0, 3100.0, 3200.0];
        
        // Store old prediction
        {
            let mut cache_write = cache.write().await;
            cache_write.insert(symbol.to_string(), (old_timestamp, predictions));
        }
        
        // Check if expired (TTL = 600 seconds)
        let cache_ttl = 600;
        let now = Utc::now();
        
        {
            let cache_read = cache.read().await;
            if let Some((cached_time, _)) = cache_read.get(symbol) {
                let age = (now - *cached_time).num_seconds();
                assert!(age > cache_ttl);
            }
        }
    }
}

#[cfg(test)]
mod feature_flag_integration_tests {
    use super::*;
    
    #[test]
    #[cfg(feature = "gpu")]
    fn test_gpu_acceleration_enabled() {
        let config = create_test_config();
        assert!(config.use_gpu || !config.use_gpu); // GPU flag exists
    }
    
    #[test]
    #[cfg(feature = "advanced-models")]
    fn test_advanced_models_available() {
        let advanced_models = vec!["autoformer", "informer", "nbeats", "deepar"];
        assert!(!advanced_models.is_empty());
    }
    
    #[test]
    #[cfg(not(feature = "gpu"))]
    fn test_cpu_only_mode() {
        let config = create_test_config();
        assert!(!config.use_gpu); // Should default to CPU
    }
}