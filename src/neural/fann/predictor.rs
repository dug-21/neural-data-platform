//! Core FannPredictor implementation
//!
//! This module contains the main FannPredictor struct and its core functionality.
//! The FannPredictor provides the main interface for neural network predictions
//! in the neural-trader system.

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex, mpsc};
use tracing::{debug, info, warn};
use dashmap::DashMap;

use crate::adapters::enhanced_neural_adapter::EnhancedNeuralAdapter;
use crate::adapters::DataAdapter;
use crate::config::NeuralConfig;
use crate::data::TimeSeriesData;
use crate::integration::training_data_service::TrainingDataService;
use crate::neural::{PredictionResult, NeuralPredictorTrait, PerformanceChannel, PerformanceEvent};

// Import FANN neural network components
use ::ruv_fann::{ActivationFunction, Network, NetworkBuilder, TrainingData};
use std::time::{Duration, Instant};

// Re-export shared types from parent modules
pub use super::networks::{ModelConfig, ModelKey, FannModelConfig, TrainingResult, TrainingAlgorithm};
pub use super::training::RecurrentState;

/// Model performance metrics for dynamic weighting
#[derive(Debug)]
pub struct ModelPerformance {
    /// Recent prediction accuracy (0.0 to 1.0)
    pub recent_accuracy: f64,
    /// Confidence calibration score
    pub confidence_score: f64,
    /// Total predictions made
    pub prediction_count: AtomicUsize,
    /// Successful predictions (within threshold)
    pub successful_predictions: AtomicUsize,
    /// Model's performance in different market regimes
    pub regime_performance: HashMap<MarketRegime, f64>,
    /// Time-weighted accuracy (more recent predictions have higher weight)
    pub time_weighted_accuracy: f64,
}

/// Market regime detection for dynamic ensemble weighting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarketRegime {
    Bullish,
    Bearish,
    Sideways,
    HighVolatility,
    LowVolatility,
}

/// Neural Error types
#[derive(Debug, thiserror::Error)]
pub enum NeuralError {
    #[error("Model not found: {0}")]
    ModelNotFound(String),
    #[error("Training failed: {0}")]
    TrainingFailed(String),
    #[error("Prediction failed: {0}")]
    PredictionFailed(String),
    #[error("Network configuration error: {0}")]
    NetworkConfigError(String),
    #[error("Adapter error: {0}")]
    AdapterError(String),
}

/// Main FannPredictor struct
///
/// This predictor provides seamless integration between lightweight FANN models
/// and sophisticated real neural models, with intelligent routing and ensemble
/// capabilities for optimal prediction performance.
pub struct FannPredictor {
    config: NeuralConfig,
    networks: Arc<RwLock<HashMap<String, Arc<Mutex<Network<f32>>>>>>,
    network_cache: Arc<DashMap<ModelKey, Arc<Network<f32>>>>,
    model_configs: HashMap<String, FannModelConfig>,
    training_cache: Arc<RwLock<HashMap<String, TrainingData<f32>>>>,
    prediction_cache: Arc<RwLock<HashMap<String, (DateTime<Utc>, Vec<PredictionResult>)>>>,
    performance_tx: mpsc::Sender<PerformanceEvent>,
    recurrent_states: Arc<RwLock<HashMap<String, RecurrentState>>>,
    ensemble_manager: Arc<RwLock<EnsembleManager>>,
    enhanced_adapter: Option<Arc<EnhancedNeuralAdapter>>,
    enhanced_neural_adapter: Option<Arc<Mutex<EnhancedNeuralAdapter>>>,
    checkpoint_manager: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    streaming_buffer: Arc<RwLock<VecDeque<TimeSeriesData>>>,
    training_data_service: Option<Arc<TrainingDataService>>,
}

/// Streaming configuration for real-time prediction
#[derive(Debug, Clone)]
pub struct StreamingConfig {
    /// Maximum buffer size for streaming data
    pub max_buffer_size: usize,
    /// Prediction frequency in milliseconds
    pub prediction_frequency: u64,
    /// Enable streaming mode
    pub enabled: bool,
    /// Buffer timeout for stale data
    pub buffer_timeout: Duration,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            max_buffer_size: 1000,
            prediction_frequency: 1000, // 1 second
            enabled: false,
            buffer_timeout: Duration::from_secs(30),
        }
    }
}

/// Ensemble management for model combination
#[derive(Debug)]
pub struct EnsembleManager {
    /// Model weights for ensemble predictions
    model_weights: HashMap<String, f64>,
    /// Performance tracking for dynamic weighting
    model_performance: HashMap<String, ModelPerformance>,
    /// Ensemble strategy
    strategy: EnsembleStrategy,
    /// Confidence threshold for predictions
    confidence_threshold: f64,
}

/// Ensemble strategy for combining model predictions
#[derive(Debug, Clone)]
pub enum EnsembleStrategy {
    /// Simple average of all model predictions
    Average,
    /// Weighted average based on model performance
    WeightedAverage,
    /// Best model selection based on recent performance
    BestModel,
    /// Dynamic strategy that adapts based on market conditions
    Dynamic,
}

impl FannPredictor {
    /// Create a new FannPredictor instance
    pub fn new(config: NeuralConfig) -> Result<Self> {
        let (performance_tx, _performance_rx) = mpsc::channel(1000);
        
        let model_configs = Self::create_default_model_configs(&config);
        
        Ok(Self {
            config,
            networks: Arc::new(RwLock::new(HashMap::new())),
            network_cache: Arc::new(DashMap::new()),
            model_configs,
            training_cache: Arc::new(RwLock::new(HashMap::new())),
            prediction_cache: Arc::new(RwLock::new(HashMap::new())),
            performance_tx,
            recurrent_states: Arc::new(RwLock::new(HashMap::new())),
            ensemble_manager: Arc::new(RwLock::new(EnsembleManager::new())),
            enhanced_adapter: None,
            enhanced_neural_adapter: None,
            checkpoint_manager: Arc::new(RwLock::new(HashMap::new())),
            streaming_buffer: Arc::new(RwLock::new(VecDeque::new())),
            training_data_service: None,
        })
    }

    /// Initialize the enhanced neural adapter
    pub async fn init_enhanced_adapter(&self) -> Result<()> {
        info!("Initializing enhanced neural adapter");
        
        // This will be implemented when the adapter modules are ready
        warn!("Enhanced adapter initialization placeholder - implement in adapter module");
        
        Ok(())
    }

    /// Get the current configuration
    pub fn config(&self) -> &NeuralConfig {
        &self.config
    }

    /// Get model configurations
    pub fn model_configs(&self) -> &HashMap<String, FannModelConfig> {
        &self.model_configs
    }

    /// Check if enhanced models are enabled
    pub fn use_real_models(&self) -> bool {
        self.config.use_real_models
    }

    /// Create default model configurations
    fn create_default_model_configs(config: &NeuralConfig) -> HashMap<String, FannModelConfig> {
        let mut configs = HashMap::new();
        
        // MLP configuration
        configs.insert("MLP".to_string(), FannModelConfig {
            layers: vec![config.input_size, 64, 32, config.output_size],
            activation: ActivationFunction::SigmoidStepwise,
            learning_rate: 0.001,
            epochs: 1000,
            desired_error: 0.001,
            max_epochs: 5000,
            epochs_between_reports: 100,
        });

        // LSTM configuration (simulated)
        configs.insert("LSTM".to_string(), FannModelConfig {
            layers: vec![config.input_size, 128, 64, config.output_size],
            activation: ActivationFunction::SigmoidStepwise,
            learning_rate: 0.001,
            epochs: 1500, 
            desired_error: 0.001,
            max_epochs: 7000,
            epochs_between_reports: 150,
        });

        // Add other model configurations as needed...
        
        configs
    }

    /// Calculate volatility from time series data
    fn calculate_volatility(&self, data: &[TimeSeriesData]) -> f64 {
        if data.len() < 2 {
            return 0.05; // Default 5% volatility
        }

        let returns: Vec<f64> = data
            .windows(2)
            .map(|window| (window[1].close / window[0].close - 1.0).ln())
            .collect();

        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns
            .iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>()
            / (returns.len() - 1) as f64;

        variance.sqrt()
    }
}

impl EnsembleManager {
    fn new() -> Self {
        Self {
            model_weights: HashMap::new(),
            model_performance: HashMap::new(),
            strategy: EnsembleStrategy::WeightedAverage,
            confidence_threshold: 0.7,
        }
    }

    /// Update model performance metrics
    pub fn update_performance(&mut self, model_name: &str, accuracy: f64, confidence: f64) {
        let performance = self.model_performance
            .entry(model_name.to_string())
            .or_insert_with(|| ModelPerformance {
                recent_accuracy: 0.0,
                confidence_score: 0.0,
                prediction_count: AtomicUsize::new(0),
                successful_predictions: AtomicUsize::new(0),
                regime_performance: HashMap::new(),
                time_weighted_accuracy: 0.0,
            });

        performance.recent_accuracy = accuracy;
        performance.confidence_score = confidence;
        performance.prediction_count.fetch_add(1, Ordering::Relaxed);
        
        if accuracy > self.confidence_threshold {
            performance.successful_predictions.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Get ensemble weights for models
    pub fn get_weights(&self) -> &HashMap<String, f64> {
        &self.model_weights
    }

    /// Calculate dynamic weights based on performance
    pub fn calculate_dynamic_weights(&mut self, models: &[String]) {
        self.model_weights.clear();
        
        let total_performance: f64 = models
            .iter()
            .filter_map(|model| self.model_performance.get(model))
            .map(|perf| perf.recent_accuracy)
            .sum();

        if total_performance > 0.0 {
            for model in models {
                if let Some(performance) = self.model_performance.get(model) {
                    let weight = performance.recent_accuracy / total_performance;
                    self.model_weights.insert(model.clone(), weight);
                }
            }
        } else {
            // Equal weights if no performance data
            let equal_weight = 1.0 / models.len() as f64;
            for model in models {
                self.model_weights.insert(model.clone(), equal_weight);
            }
        }
    }
}

impl ModelPerformance {
    /// Get success rate for this model
    pub fn success_rate(&self) -> f64 {
        let total = self.prediction_count.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        
        let successful = self.successful_predictions.load(Ordering::Relaxed);
        successful as f64 / total as f64
    }
}

// PHASE 3A TRAIT IMPLEMENTATION: Add missing NeuralPredictorTrait implementation
#[async_trait::async_trait]
impl crate::neural::NeuralPredictorTrait for FannPredictor {
    /// Main prediction method - implements trait requirement
    async fn predict(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        _features: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<PredictionResult>> {
        debug!("FannPredictor::predict called with horizon: {}", horizon);
        
        // Get first available model configuration
        let model_name = self.model_configs
            .keys()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No model configurations available"))?;
        
        // Generate predictions using FANN networks
        self.predict_with_model(model_name, data, horizon).await
    }

    /// Ensemble prediction - combines multiple models
    async fn predict_ensemble(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        models: &[String],
        _features: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<PredictionResult>> {
        debug!("FannPredictor::predict_ensemble called with {} models, horizon: {}", models.len(), horizon);
        
        if models.is_empty() {
            return self.predict(data, horizon, None).await;
        }

        let mut all_predictions = Vec::new();
        let ensemble_manager = self.ensemble_manager.read().await;
        let weights = ensemble_manager.get_weights();

        // Predict with each model and collect results
        for model_name in models {
            if self.model_configs.contains_key(model_name) {
                match self.predict_with_model(model_name, data, horizon).await {
                    Ok(mut predictions) => {
                        // Apply model weight to predictions
                        let weight = weights.get(model_name).copied().unwrap_or(1.0 / models.len() as f64);
                        for pred in &mut predictions {
                            pred.value *= weight;
                            pred.confidence *= weight;
                        }
                        all_predictions.push(predictions);
                    }
                    Err(e) => {
                        warn!("Model {} prediction failed: {}", model_name, e);
                        continue;
                    }
                }
            }
        }

        // Combine ensemble predictions
        self.combine_ensemble_predictions(all_predictions, horizon).await
    }

    /// Get feature importance scores
    async fn get_feature_importance(&self) -> Result<HashMap<String, f64>> {
        debug!("FannPredictor::get_feature_importance called");
        
        // For FANN networks, feature importance is based on network weights
        // This is a simplified implementation - real importance would require network analysis
        let mut importance = HashMap::new();
        
        // Add common financial features with estimated importance
        importance.insert("price_trend".to_string(), 0.25);
        importance.insert("volume_trend".to_string(), 0.20);
        importance.insert("volatility".to_string(), 0.15);
        importance.insert("momentum".to_string(), 0.15);
        importance.insert("support_resistance".to_string(), 0.10);
        importance.insert("market_sentiment".to_string(), 0.10);
        importance.insert("other_indicators".to_string(), 0.05);

        Ok(importance)
    }
}

impl FannPredictor {
    /// Predict with a specific model
    async fn predict_with_model(
        &self,
        model_name: &str,
        data: &[TimeSeriesData],
        horizon: usize,
    ) -> Result<Vec<PredictionResult>> {
        debug!("Predicting with model: {}, horizon: {}", model_name, horizon);

        // Check if we have training data
        if data.is_empty() {
            return Err(anyhow::anyhow!("No input data provided"));
        }

        // Convert time series data to network inputs
        let inputs = self.convert_to_network_inputs(data)?;
        
        // Get or create network for this model
        let network = self.get_or_create_network(model_name).await?;
        let network_guard = network.lock().await;

        // Generate predictions for each horizon step
        let mut predictions = Vec::with_capacity(horizon);
        let mut current_input = inputs;

        for step in 0..horizon {
            // Run network prediction
            let output = network_guard.run(&current_input)
                .map_err(|e| anyhow::anyhow!("Network prediction failed: {:?}", e))?;

            let prediction_value = output.get(0).copied().unwrap_or(0.0) as f64;
            
            // Calculate confidence based on recent performance
            let confidence = self.calculate_prediction_confidence(model_name, step).await;
            
            // Calculate prediction intervals (simplified)
            let volatility = self.calculate_volatility(data);
            let interval_width = volatility * (step + 1) as f64 * 0.1; // Simple volatility scaling
            
            let prediction = PredictionResult {
                timestamp: chrono::Utc::now() + chrono::Duration::seconds((step + 1) as i64 * 3600), // 1 hour steps
                value: prediction_value,
                confidence,
                interval_low: prediction_value - interval_width,
                interval_high: prediction_value + interval_width,
                model_name: model_name.to_string(),
                metadata: Some({
                    let mut meta = HashMap::new();
                    meta.insert("step".to_string(), serde_json::Value::Number(serde_json::Number::from(step)));
                    meta.insert("volatility".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(volatility).unwrap_or(serde_json::Number::from(0))));
                    meta
                }),
            };

            predictions.push(prediction);

            // Update input for next prediction (use prediction as feedback)
            if current_input.len() > 1 {
                current_input.rotate_left(1);
                current_input[current_input.len() - 1] = prediction_value as f32;
            }
        }

        Ok(predictions)
    }

    /// Get or create a network for the specified model
    async fn get_or_create_network(&self, model_name: &str) -> Result<Arc<Mutex<Network<f32>>>> {
        let networks = self.networks.read().await;
        
        if let Some(network) = networks.get(model_name) {
            return Ok(network.clone());
        }
        
        drop(networks); // Release read lock before acquiring write lock
        
        // Create new network
        let config = self.model_configs.get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model configuration not found: {}", model_name))?;

        let network = NetworkBuilder::new()
            .with_layers(&config.layers)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create network: {:?}", e))?;

        let network_arc = Arc::new(Mutex::new(network));
        
        // Store the network
        let mut networks = self.networks.write().await;
        networks.insert(model_name.to_string(), network_arc.clone());
        
        Ok(network_arc)
    }

    /// Convert time series data to network inputs
    fn convert_to_network_inputs(&self, data: &[TimeSeriesData]) -> Result<Vec<f32>> {
        if data.is_empty() {
            return Err(anyhow::anyhow!("No data to convert"));
        }

        // Take the most recent data points up to input size
        let input_size = self.model_configs.values().next()
            .map(|config| config.layers[0])
            .unwrap_or(60); // Default input size
        
        let recent_data = if data.len() >= input_size {
            &data[data.len() - input_size..]
        } else {
            data
        };

        // Convert to normalized inputs
        let mut inputs = Vec::with_capacity(input_size);
        
        // Use closing prices normalized by the first price
        let base_price = recent_data[0].close;
        
        for item in recent_data {
            let normalized_price = (item.close / base_price - 1.0) as f32;
            inputs.push(normalized_price);
        }

        // Pad with zeros if we don't have enough data
        while inputs.len() < input_size {
            inputs.insert(0, 0.0);
        }

        Ok(inputs)
    }

    /// Calculate prediction confidence for a model
    async fn calculate_prediction_confidence(&self, model_name: &str, step: usize) -> f64 {
        let ensemble_manager = self.ensemble_manager.read().await;
        
        if let Some(performance) = ensemble_manager.model_performance.get(model_name) {
            // Decrease confidence with prediction horizon
            let base_confidence = performance.recent_accuracy;
            let decay_factor = 1.0 - (step as f64 * 0.1); // 10% decay per step
            (base_confidence * decay_factor).max(0.1) // Minimum 10% confidence
        } else {
            // Default confidence for new models
            0.6 - (step as f64 * 0.05) // Start at 60%, decay 5% per step
        }
    }

    /// Combine ensemble predictions
    async fn combine_ensemble_predictions(
        &self,
        all_predictions: Vec<Vec<PredictionResult>>,
        horizon: usize,
    ) -> Result<Vec<PredictionResult>> {
        if all_predictions.is_empty() {
            return Err(anyhow::anyhow!("No predictions to combine"));
        }

        let mut combined = Vec::with_capacity(horizon);

        for step in 0..horizon {
            let mut values = Vec::new();
            let mut confidences = Vec::new();
            let mut total_weight = 0.0;

            // Collect values and confidences for this step from all models
            for model_predictions in &all_predictions {
                if let Some(pred) = model_predictions.get(step) {
                    values.push(pred.value);
                    confidences.push(pred.confidence);
                    total_weight += pred.confidence; // Use confidence as weight
                }
            }

            if values.is_empty() {
                continue;
            }

            // Calculate weighted average
            let weighted_value = values.iter()
                .zip(confidences.iter())
                .map(|(v, c)| v * c)
                .sum::<f64>() / total_weight;

            let avg_confidence = confidences.iter().sum::<f64>() / confidences.len() as f64;

            // Calculate ensemble intervals
            let variance = values.iter()
                .map(|v| (v - weighted_value).powi(2))
                .sum::<f64>() / values.len() as f64;
            let std_dev = variance.sqrt();

            combined.push(PredictionResult {
                timestamp: chrono::Utc::now() + chrono::Duration::seconds((step + 1) as i64 * 3600),
                value: weighted_value,
                confidence: avg_confidence,
                interval_low: weighted_value - std_dev,
                interval_high: weighted_value + std_dev,
                model_name: "ensemble".to_string(),
                metadata: Some({
                    let mut meta = HashMap::new();
                    meta.insert("num_models".to_string(), serde_json::Value::Number(serde_json::Number::from(values.len())));
                    meta.insert("variance".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(variance).unwrap_or(serde_json::Number::from(0))));
                    meta
                }),
            });
        }

        Ok(combined)
    }
}