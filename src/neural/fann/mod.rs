//! FANN-based neural predictor modules
//!
//! This module provides a modularized implementation of the FANN predictor
//! with separate components for prediction, network management, training,
//! and data conversion.

pub mod predictor;
pub mod networks;
pub mod training;
pub mod conversion;

// Re-export main types for convenience
pub use predictor::{FannPredictor, ModelPerformance, MarketRegime, NeuralError, EnsembleManager, StreamingConfig};
pub use networks::{ModelConfig, ModelKey, FannModelConfig, TrainingResult, TrainingAlgorithm, NetworkArchitecture};
pub use training::{RecurrentState, OnlineTrainingConfig, TrainingMetrics, ConceptDriftDetector, PerformanceTrend};
pub use conversion::{ConversionConfig, NormalizationMethod, InputConverter, OutputConverter};

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::config::NeuralConfig;
use crate::data::TimeSeriesData;
use crate::neural::{PredictionResult, NeuralPredictorTrait};

/// Factory function to create a fully configured FANN predictor
pub async fn create_fann_predictor(config: NeuralConfig) -> Result<Arc<FannPredictor>> {
    info!("Creating FANN predictor with configuration: {:?}", config);

    let predictor = FannPredictor::new(config)?;
    let predictor = Arc::new(predictor);

    // Initialize enhanced adapter if real models are enabled
    if predictor.use_real_models() {
        info!("Initializing enhanced neural adapter for real models");
        if let Err(e) = predictor.init_enhanced_adapter().await {
            warn!("Failed to initialize enhanced adapter: {}. Falling back to FANN-only mode.", e);
        }
    }

    info!("FANN predictor created successfully");
    Ok(predictor)
}

/// Create a FANN predictor with custom model configurations
pub async fn create_fann_predictor_with_models(
    config: NeuralConfig,
    model_configs: std::collections::HashMap<String, FannModelConfig>,
) -> Result<Arc<FannPredictor>> {
    info!("Creating FANN predictor with {} custom model configurations", model_configs.len());

    // Create predictor with custom configurations
    let mut predictor_config = config.clone();
    // Here you would apply the custom model configurations
    // This is a placeholder for the actual implementation

    let predictor = FannPredictor::new(predictor_config)?;
    let predictor = Arc::new(predictor);

    // Initialize enhanced adapter if needed
    if predictor.use_real_models() {
        if let Err(e) = predictor.init_enhanced_adapter().await {
            warn!("Failed to initialize enhanced adapter: {}", e);
        }
    }

    info!("FANN predictor with custom models created successfully");
    Ok(predictor)
}

/// Utility function to validate predictor configuration
pub fn validate_fann_config(config: &NeuralConfig) -> Result<()> {
    // Basic validation
    if config.input_size == 0 {
        return Err(anyhow::anyhow!("Input size must be greater than 0"));
    }

    if config.output_size == 0 {
        return Err(anyhow::anyhow!("Output size must be greater than 0"));
    }

    if config.models.is_empty() {
        return Err(anyhow::anyhow!("At least one model must be specified"));
    }

    // Validate model names
    for model_name in &config.models {
        if let Err(_) = model_name.parse::<NetworkArchitecture>() {
            warn!("Unknown model architecture: {}. Will use default MLP configuration.", model_name);
        }
    }

    // Validate learning rate
    if config.learning_rate <= 0.0 || config.learning_rate > 1.0 {
        return Err(anyhow::anyhow!("Learning rate must be between 0 and 1, got: {}", config.learning_rate));
    }

    info!("FANN configuration validation passed");
    Ok(())
}

/// Create conversion components for FANN predictor
pub fn create_conversion_components(config: &NeuralConfig) -> (InputConverter, OutputConverter) {
    let conversion_config = conversion::ConversionConfig {
        normalization_method: match config.normalization_method.as_deref() {
            Some("minmax") => NormalizationMethod::MinMax,
            Some("zscore") => NormalizationMethod::ZScore,
            Some("robust") => NormalizationMethod::Robust,
            Some("none") => NormalizationMethod::None,
            _ => NormalizationMethod::MinMax, // Default
        },
        validate_data: true,
        ..Default::default()
    };

    let input_converter = InputConverter::new(conversion_config.clone());
    
    let output_config = conversion::output::OutputInterpretationConfig {
        prediction_horizon: config.prediction_horizon.unwrap_or(5),
        base_confidence: 0.7,
        ..Default::default()
    };
    
    let output_converter = OutputConverter::with_output_config(conversion_config, output_config);

    (input_converter, output_converter)
}

/// Get default model configurations for common architectures
pub fn get_default_model_configs(input_size: usize, output_size: usize) -> std::collections::HashMap<String, FannModelConfig> {
    let mut configs = std::collections::HashMap::new();

    let architectures = [
        NetworkArchitecture::MLP,
        NetworkArchitecture::LSTM,
        NetworkArchitecture::GRU,
        NetworkArchitecture::DeepAR,
        NetworkArchitecture::TCN,
        NetworkArchitecture::NHITS,
        NetworkArchitecture::Transformer,
    ];

    for arch in architectures {
        let config = arch.default_config(input_size, output_size);
        configs.insert(arch.to_string(), config);
    }

    configs
}

/// Performance monitoring utilities
pub mod monitoring {
    use super::*;
    use std::time::{Duration, Instant};

    /// Performance monitor for FANN predictor
    pub struct FannPerformanceMonitor {
        start_time: Instant,
        prediction_count: usize,
        error_count: usize,
        total_prediction_time: Duration,
    }

    impl FannPerformanceMonitor {
        pub fn new() -> Self {
            Self {
                start_time: Instant::now(),
                prediction_count: 0,
                error_count: 0,
                total_prediction_time: Duration::from_secs(0),
            }
        }

        pub fn record_prediction(&mut self, duration: Duration, success: bool) {
            self.prediction_count += 1;
            self.total_prediction_time += duration;
            
            if !success {
                self.error_count += 1;
            }
        }

        pub fn get_stats(&self) -> PerformanceStats {
            let uptime = self.start_time.elapsed();
            let avg_prediction_time = if self.prediction_count > 0 {
                self.total_prediction_time / self.prediction_count as u32
            } else {
                Duration::from_secs(0)
            };

            let success_rate = if self.prediction_count > 0 {
                (self.prediction_count - self.error_count) as f64 / self.prediction_count as f64
            } else {
                0.0
            };

            PerformanceStats {
                uptime,
                total_predictions: self.prediction_count,
                error_count: self.error_count,
                success_rate,
                average_prediction_time: avg_prediction_time,
                predictions_per_second: if uptime.as_secs() > 0 {
                    self.prediction_count as f64 / uptime.as_secs() as f64
                } else {
                    0.0
                },
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct PerformanceStats {
        pub uptime: Duration,
        pub total_predictions: usize,
        pub error_count: usize,
        pub success_rate: f64,
        pub average_prediction_time: Duration,
        pub predictions_per_second: f64,
    }

    impl Default for FannPerformanceMonitor {
        fn default() -> Self {
            Self::new()
        }
    }
}

/// Testing utilities for FANN predictor
#[cfg(test)]
pub mod test_utils {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;

    /// Create test time series data
    pub fn create_test_data(count: usize) -> Vec<TimeSeriesData> {
        (0..count)
            .map(|i| {
                let base_price = 100.0 + i as f64;
                TimeSeriesData {
                    timestamp: Utc::now(),
                    open: base_price,
                    high: base_price * 1.02,
                    low: base_price * 0.98,
                    close: base_price * 1.01,
                    volume: 1000.0 + i as f64 * 10.0,
                    indicators: {
                        let mut indicators = HashMap::new();
                        indicators.insert("rsi".to_string(), 50.0 + (i as f64 % 50.0));
                        indicators.insert("macd".to_string(), (i as f64 % 2.0) - 1.0);
                        indicators
                    },
                }
            })
            .collect()
    }

    /// Create test neural config
    pub fn create_test_config() -> NeuralConfig {
        NeuralConfig {
            input_size: 24,
            output_size: 1,
            hidden_layers: vec![64, 32],
            learning_rate: 0.001,
            models: vec!["MLP".to_string(), "LSTM".to_string()],
            use_real_models: false,
            prediction_horizon: Some(5),
            normalization_method: Some("minmax".to_string()),
            ..Default::default()
        }
    }

    /// Validate prediction results
    pub fn validate_predictions(predictions: &[PredictionResult]) -> bool {
        predictions.iter().all(|p| {
            p.value > 0.0 &&
            p.confidence >= 0.0 &&
            p.confidence <= 1.0 &&
            p.interval_low <= p.value &&
            p.value <= p.interval_high &&
            !p.model_name.is_empty()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::neural::fann::test_utils::*;

    #[tokio::test]
    async fn test_create_fann_predictor() {
        let config = create_test_config();
        let result = create_fann_predictor(config).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_fann_config() {
        let config = create_test_config();
        assert!(validate_fann_config(&config).is_ok());

        let mut invalid_config = config.clone();
        invalid_config.input_size = 0;
        assert!(validate_fann_config(&invalid_config).is_err());
    }

    #[test]
    fn test_create_conversion_components() {
        let config = create_test_config();
        let (input_converter, output_converter) = create_conversion_components(&config);
        
        assert!(input_converter.feature_count() > 0);
        assert!(output_converter.config().validate_data);
    }

    #[test]
    fn test_get_default_model_configs() {
        let configs = get_default_model_configs(24, 1);
        
        assert!(!configs.is_empty());
        assert!(configs.contains_key("MLP"));
        assert!(configs.contains_key("LSTM"));
        assert!(configs.contains_key("Transformer"));
    }

    #[test]
    fn test_performance_monitor() {
        let mut monitor = monitoring::FannPerformanceMonitor::new();
        
        monitor.record_prediction(std::time::Duration::from_millis(100), true);
        monitor.record_prediction(std::time::Duration::from_millis(150), false);
        
        let stats = monitor.get_stats();
        assert_eq!(stats.total_predictions, 2);
        assert_eq!(stats.error_count, 1);
        assert_eq!(stats.success_rate, 0.5);
    }
}