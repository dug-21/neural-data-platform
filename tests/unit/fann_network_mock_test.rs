//! Mock FANN Network tests for neural predictor
//!
//! Tests mock FANN network behavior and integration with the predictor

use autonomous_platform::neural::fann_predictor::{FannPredictor, FannModelConfig};
use autonomous_platform::neural::NeuralPredictorTrait;
use autonomous_platform::config::NeuralConfig;
use autonomous_platform::data::TimeSeriesData;
use chrono::Utc;
use std::collections::HashMap;
use ::ruv_fann::{ActivationFunction, TrainingData};

/// Mock FANN Network builder for testing
struct MockNetworkBuilder {
    input_size: usize,
    hidden_layers: Vec<(usize, ActivationFunction)>,
    output_size: usize,
    output_activation: ActivationFunction,
}

impl MockNetworkBuilder {
    fn new() -> Self {
        Self {
            input_size: 0,
            hidden_layers: Vec::new(),
            output_size: 0,
            output_activation: ActivationFunction::Linear,
        }
    }
    
    fn input_layer(mut self, size: usize) -> Self {
        self.input_size = size;
        self
    }
    
    fn hidden_layer_with_activation(mut self, size: usize, activation: ActivationFunction, _steepness: f32) -> Self {
        self.hidden_layers.push((size, activation));
        self
    }
    
    fn output_layer_with_activation(mut self, size: usize, activation: ActivationFunction, _steepness: f32) -> Self {
        self.output_size = size;
        self.output_activation = activation;
        self
    }
    
    fn build(self) -> MockNetwork {
        MockNetwork {
            input_size: self.input_size,
            hidden_layers: self.hidden_layers,
            output_size: self.output_size,
            output_activation: self.output_activation,
            weights: vec![0.5; self.input_size * self.output_size], // Simplified
        }
    }
}

/// Mock FANN Network for testing
struct MockNetwork {
    input_size: usize,
    hidden_layers: Vec<(usize, ActivationFunction)>,
    output_size: usize,
    output_activation: ActivationFunction,
    weights: Vec<f32>,
}

impl MockNetwork {
    fn run(&self, inputs: &[f32]) -> Vec<f32> {
        let mut outputs = vec![0.0f32; self.output_size];
        
        // Simulate network forward pass
        for i in 0..self.output_size {
            let mut sum = 0.0f32;
            
            // Simple weighted sum
            for (j, &input) in inputs.iter().enumerate().take(self.input_size) {
                let weight_idx = i * self.input_size + j;
                if weight_idx < self.weights.len() {
                    sum += input * self.weights[weight_idx];
                }
            }
            
            // Apply activation function
            outputs[i] = match self.output_activation {
                ActivationFunction::Linear => sum,
                ActivationFunction::SigmoidSymmetric => sum.tanh(),
                ActivationFunction::ReLU => sum.max(0.0),
                ActivationFunction::Gaussian => (-sum * sum).exp(),
                ActivationFunction::Tanh => sum.tanh(),
                _ => sum,
            };
            
            // Add decay for multi-step predictions
            outputs[i] *= 1.0 - (0.05 * i as f32);
        }
        
        outputs
    }
    
    fn train(&mut self, _data: &TrainingData<f32>, _max_epochs: usize, _target_error: f32) {
        // Simulate training by slightly adjusting weights
        for weight in &mut self.weights {
            *weight *= 1.01; // Simple weight update
        }
    }
}

fn create_test_data_with_features(size: usize) -> Vec<TimeSeriesData> {
    let mut data = Vec::new();
    let base_time = Utc::now();
    
    for i in 0..size {
        let price = 50000.0 + (i as f64 * 100.0);
        let volume = 1000000.0 * (1.0 + 0.1 * (i as f64 * 0.1).sin());
        
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 50.0 + 20.0 * (i as f64 * 0.1).sin());
        indicators.insert("macd".to_string(), 0.5 * (i as f64 * 0.05).cos());
        indicators.insert("bb_width".to_string(), 200.0 + 50.0 * (i as f64 * 0.08).sin());
        indicators.insert("momentum".to_string(), 1.0 + 0.1 * (i as f64 * 0.15).cos());
        
        data.push(TimeSeriesData {
            timestamp: base_time + chrono::Duration::minutes(i as i64 * 5),
            entity: "test".to_string(),
            symbol: "TEST/USD".to_string(),
            open: price * 0.999,
            high: price * 1.001,
            low: price * 0.998,
            close: price,
            volume,
            source: "mock".to_string(),
            metadata: HashMap::new(),
            indicators,
        });
    }
    
    data
}

#[cfg(test)]
mod mock_network_tests {
    use super::*;

    #[test]
    fn test_mock_network_builder() {
        let network = MockNetworkBuilder::new()
            .input_layer(30)
            .hidden_layer_with_activation(64, ActivationFunction::SigmoidSymmetric, 1.0)
            .hidden_layer_with_activation(32, ActivationFunction::SigmoidSymmetric, 1.0)
            .output_layer_with_activation(5, ActivationFunction::Linear, 1.0)
            .build();
        
        assert_eq!(network.input_size, 30);
        assert_eq!(network.hidden_layers.len(), 2);
        assert_eq!(network.output_size, 5);
    }

    #[test]
    fn test_mock_network_forward_pass() {
        let network = MockNetworkBuilder::new()
            .input_layer(10)
            .hidden_layer_with_activation(20, ActivationFunction::ReLU, 1.0)
            .output_layer_with_activation(3, ActivationFunction::Linear, 1.0)
            .build();
        
        let inputs = vec![0.1f32; 10];
        let outputs = network.run(&inputs);
        
        assert_eq!(outputs.len(), 3);
        
        // Verify decay in multi-step predictions
        assert!(outputs[0] > outputs[1]);
        assert!(outputs[1] > outputs[2]);
    }

    #[test]
    fn test_activation_functions() {
        let activations = vec![
            ActivationFunction::Linear,
            ActivationFunction::SigmoidSymmetric,
            ActivationFunction::ReLU,
            ActivationFunction::Gaussian,
            ActivationFunction::Tanh,
        ];
        
        for activation in activations {
            let network = MockNetworkBuilder::new()
                .input_layer(5)
                .hidden_layer_with_activation(10, activation, 1.0)
                .output_layer_with_activation(2, activation, 1.0)
                .build();
            
            let inputs = vec![0.5f32, -0.5, 1.0, -1.0, 0.0];
            let outputs = network.run(&inputs);
            
            assert_eq!(outputs.len(), 2);
            
            // Verify activation function properties
            match activation {
                ActivationFunction::ReLU => {
                    for &output in &outputs {
                        assert!(output >= 0.0);
                    }
                },
                ActivationFunction::SigmoidSymmetric | ActivationFunction::Tanh => {
                    for &output in &outputs {
                        assert!(output >= -1.0 && output <= 1.0);
                    }
                },
                ActivationFunction::Gaussian => {
                    for &output in &outputs {
                        assert!(output >= 0.0 && output <= 1.0);
                    }
                },
                _ => {}
            }
        }
    }

    #[test]
    fn test_training_data_preparation() {
        let config = FannModelConfig {
            input_size: 30,  // 10 timesteps * 3 features
            hidden_layers: vec![20, 10],
            output_size: 5,
            hidden_activation: ActivationFunction::SigmoidSymmetric,
            output_activation: ActivationFunction::Linear,
            learning_rate: 0.001,
            momentum: 0.9,
            max_epochs: 100,
            target_error: 0.01,
            use_cascade: false,
        };
        
        let data = create_test_data_with_features(50);
        let window_size = config.input_size / 3;
        
        let mut training_inputs = Vec::new();
        let mut training_outputs = Vec::new();
        
        for i in window_size..(data.len() - config.output_size) {
            let mut input_vec = Vec::new();
            
            // Collect features
            for j in (i - window_size)..i {
                let price_norm = (data[j].close - data[i-1].close) / data[i-1].close;
                let volume_norm = (data[j].volume / 1_000_000.0).ln();
                let rsi = data[j].indicators.get("rsi").copied().unwrap_or(50.0) / 100.0;
                
                input_vec.push(price_norm as f32);
                input_vec.push(volume_norm as f32);
                input_vec.push(rsi as f32);
            }
            
            // Collect targets
            let mut output_vec = Vec::new();
            for j in 0..config.output_size {
                if i + j < data.len() {
                    let future_return = (data[i + j].close - data[i-1].close) / data[i-1].close;
                    output_vec.push(future_return as f32);
                }
            }
            
            if output_vec.len() == config.output_size {
                training_inputs.push(input_vec);
                training_outputs.push(output_vec);
            }
        }
        
        assert!(!training_inputs.is_empty());
        assert_eq!(training_inputs.len(), training_outputs.len());
        assert_eq!(training_inputs[0].len(), config.input_size);
        assert_eq!(training_outputs[0].len(), config.output_size);
    }

    #[test]
    fn test_cascade_training_simulation() {
        let mut network = MockNetworkBuilder::new()
            .input_layer(20)
            .hidden_layer_with_activation(30, ActivationFunction::SigmoidSymmetric, 1.0)
            .output_layer_with_activation(5, ActivationFunction::Linear, 1.0)
            .build();
        
        let training_data = TrainingData {
            inputs: vec![vec![0.1f32; 20]; 100],
            outputs: vec![vec![0.2f32; 5]; 100],
        };
        
        // Simulate cascade training
        let initial_weights = network.weights.clone();
        network.train(&training_data, 10, 0.001);
        
        // Weights should have changed
        for (i, (&initial, &trained)) in initial_weights.iter().zip(network.weights.iter()).enumerate() {
            assert_ne!(initial, trained, "Weight {} didn't change", i);
        }
    }
}

#[cfg(test)]
mod fann_integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_model_specific_network_configs() {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec![
                "NHITS".to_string(),
                "TCN".to_string(),
                "DeepAR".to_string(),
                "Transformer".to_string(),
                "LSTM".to_string(),
                "GRU".to_string(),
            ],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };
        
        let predictor = FannPredictor::new(config).unwrap();
        
        // Verify model-specific configurations are applied
        let data = create_test_data_with_features(150);
        
        // Test each model type
        for model in &["NHITS", "TCN", "DeepAR", "Transformer", "LSTM", "GRU"] {
            let result = predictor.predict_with_model(model, &data, 5).await;
            
            match model {
                &"NHITS" => {
                    // NHITS should produce multi-horizon outputs
                    assert!(result.is_ok());
                    let predictions = result.unwrap();
                    assert!(predictions.len() >= 5);
                },
                &"TCN" => {
                    // TCN with temporal convolutions
                    assert!(result.is_ok());
                    let predictions = result.unwrap();
                    assert_eq!(predictions.len(), 5);
                },
                &"DeepAR" => {
                    // DeepAR with probabilistic outputs
                    assert!(result.is_ok());
                    let predictions = result.unwrap();
                    for pred in &predictions {
                        assert!(pred.interval_high > pred.interval_low);
                    }
                },
                &"Transformer" => {
                    // Transformer with attention
                    assert!(result.is_ok());
                    let predictions = result.unwrap();
                    assert_eq!(predictions.len(), 5);
                },
                &"LSTM" | &"GRU" => {
                    // Recurrent models
                    assert!(result.is_ok());
                    let predictions = result.unwrap();
                    assert_eq!(predictions.len(), 5);
                },
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn test_training_with_different_data_patterns() {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string(), "LSTM".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };
        
        let predictor = FannPredictor::new(config).unwrap();
        
        // Create different data patterns
        let patterns = vec![
            ("trending", vec![50000.0, 51000.0, 52000.0, 53000.0, 54000.0]),
            ("cyclic", vec![50000.0, 52000.0, 50000.0, 52000.0, 50000.0]),
            ("noisy", vec![50000.0, 49500.0, 50500.0, 49800.0, 50200.0]),
        ];
        
        for (pattern_name, prices) in patterns {
            let mut data = Vec::new();
            let base_time = Utc::now();
            
            for (i, &price) in prices.iter().enumerate() {
                let mut indicators = HashMap::new();
                indicators.insert("rsi".to_string(), 50.0);
                
                // Extend data with more samples
                for j in 0..30 {
                    let idx = i * 30 + j;
                    let price_var = price + (j as f64 - 15.0) * 10.0;
                    
                    data.push(TimeSeriesData {
                        timestamp: base_time + chrono::Duration::minutes(idx as i64),
                        entity: "test".to_string(),
                        symbol: "TEST/USD".to_string(),
                        open: price_var * 0.999,
                        high: price_var * 1.001,
                        low: price_var * 0.998,
                        close: price_var,
                        volume: 1000000.0,
                        source: "test".to_string(),
                        metadata: HashMap::new(),
                        indicators: indicators.clone(),
                    });
                }
            }
            
            // Test predictions
            let predictions = predictor.predict(&data, 3, None).await.unwrap();
            assert_eq!(predictions.len(), 3);
            
            // Verify pattern-specific behavior
            match pattern_name {
                "trending" => {
                    // Should predict continuation of trend
                    assert!(predictions[0].value > data.last().unwrap().close);
                },
                "cyclic" => {
                    // Should recognize cycle
                    let last_price = data.last().unwrap().close;
                    let predicted = predictions[0].value;
                    assert!((predicted - last_price).abs() / last_price < 0.1);
                },
                "noisy" => {
                    // Should have wider intervals due to noise
                    let interval_width = predictions[0].interval_high - predictions[0].interval_low;
                    assert!(interval_width > 0.01 * predictions[0].value);
                },
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn test_online_learning_effectiveness() {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };
        
        let predictor = FannPredictor::new(config).unwrap();
        
        // Initial training data
        let initial_data = create_test_data_with_features(100);
        
        // Get baseline predictions
        let baseline_preds = predictor.predict(&initial_data, 5, None).await.unwrap();
        
        // Create new data with different pattern
        let mut new_data = Vec::new();
        let base_time = initial_data.last().unwrap().timestamp;
        let new_base_price = 60000.0; // Higher price level
        
        for i in 0..50 {
            let price = new_base_price + (i as f64 * 50.0);
            let mut indicators = HashMap::new();
            indicators.insert("rsi".to_string(), 70.0); // Bullish RSI
            
            new_data.push(TimeSeriesData {
                timestamp: base_time + chrono::Duration::minutes((i + 1) as i64 * 5),
                entity: "test".to_string(),
                symbol: "TEST/USD".to_string(),
                open: price * 0.999,
                high: price * 1.002,
                low: price * 0.998,
                close: price,
                volume: 2000000.0, // Higher volume
                source: "test".to_string(),
                metadata: HashMap::new(),
                indicators,
            });
        }
        
        // Update with new data
        predictor.update_with_new_data("MLP", &new_data).await.unwrap();
        
        // Combine all data
        let mut all_data = initial_data.clone();
        all_data.extend(new_data);
        
        // Get updated predictions
        let updated_preds = predictor.predict(&all_data, 5, None).await.unwrap();
        
        // Predictions should reflect new pattern
        assert!(updated_preds[0].value > baseline_preds[0].value);
        
        // Confidence might be different due to pattern change
        for i in 0..5 {
            assert_ne!(baseline_preds[i].confidence, updated_preds[i].confidence);
        }
    }
}