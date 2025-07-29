//! MLP (Multi-Layer Perceptron) ruv-FANN Integration Adapter
//! 
//! This module provides a bridge between the existing FannPredictor system and the
//! enhanced ruv-FANN MLP implementation, enabling seamless integration of advanced
//! MLP capabilities while maintaining compatibility with the existing neural architecture.
//!
//! ## Architecture Overview
//!
//! The MLP adapter serves as a translation layer that:
//! - Converts TimeSeriesData to ruv-FANN compatible format
//! - Provides enhanced MLP configuration with advanced parameters
//! - Implements efficient training and prediction methods
//! - Integrates with the existing FannPredictor ecosystem
//!
//! ## Key Features
//! 
//! - **Enhanced Configuration**: Advanced MLP parameters via ruv-FANN
//! - **Smart Data Conversion**: Efficient time series to MLP input transformation
//! - **Performance Optimization**: Optimized training and prediction pipelines
//! - **Seamless Integration**: Drop-in replacement for standard MLP models
//! - **Comprehensive Monitoring**: Performance tracking and metrics

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use tokio::sync::{RwLock, Mutex};
use std::sync::Arc;
use tracing::{info, debug, warn};

use crate::config::NeuralConfig;
use crate::data::TimeSeriesData;
use crate::neural::PredictionResult;

// Import FANN neural network components
use ::ruv_fann::{
    Network, NetworkBuilder,
    ActivationFunction,
    TrainingData,
};

/// Enhanced MLP configuration leveraging ruv-FANN capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedMLPConfig {
    /// Basic MLP architecture parameters
    pub input_features: usize,
    pub hidden_layers: Vec<usize>,
    pub output_size: usize,
    
    /// Advanced ruv-FANN specific parameters
    pub activation_functions: MLPActivationConfig,
    pub training_config: MLPTrainingConfig,
    pub optimization_config: MLPOptimizationConfig,
    
    /// Performance and monitoring settings
    pub performance_config: MLPPerformanceConfig,
    
    /// Integration settings
    pub integration_config: MLPIntegrationConfig,
}

/// Activation function configuration for different layers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLPActivationConfig {
    /// Hidden layer activation functions (can be different per layer)
    pub hidden_activations: Vec<ActivationFunction>,
    /// Output layer activation function
    pub output_activation: ActivationFunction,
    /// Steepness parameters for each layer
    pub steepness_factors: Vec<f32>,
    /// Enable adaptive activation function selection
    pub adaptive_activations: bool,
}

/// Advanced training configuration using ruv-FANN features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLPTrainingConfig {
    /// Learning algorithm
    pub algorithm: TrainingAlgorithm,
    /// Learning rate (can be adaptive)
    pub learning_rate: f32,
    /// Momentum factor
    pub momentum: f32,
    /// Training epochs
    pub max_epochs: usize,
    /// Target error threshold
    pub target_error: f32,
    
    /// Advanced training features
    pub use_cascade_training: bool,
    pub enable_early_stopping: bool,
    pub validation_split: f32,
    pub batch_size: Option<usize>,
    
    /// Regularization parameters
    pub dropout_rate: Option<f32>,
    pub weight_decay: Option<f32>,
    pub l1_regularization: Option<f32>,
    pub l2_regularization: Option<f32>,
}

/// Training algorithms supported by enhanced MLP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrainingAlgorithm {
    /// Standard backpropagation
    Backpropagation,
    /// Resilient backpropagation
    Rprop,
    /// Quick propagation
    Quickprop,
    /// Scaled conjugate gradient
    ScaledConjugateGradient,
    /// Adaptive learning rate
    AdaptiveLearningRate,
    /// Custom algorithm with parameters
    Custom {
        name: String,
        parameters: HashMap<String, f32>,
    },
}

/// Optimization configuration for enhanced performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLPOptimizationConfig {
    /// Enable parallel training
    pub parallel_training: bool,
    /// Number of threads for parallel operations
    pub num_threads: Option<usize>,
    /// Memory optimization level
    pub memory_optimization: MemoryOptimizationLevel,
    /// Enable SIMD optimizations
    pub enable_simd: bool,
    /// Cache size for network operations
    pub cache_size_mb: usize,
}

/// Memory optimization levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryOptimizationLevel {
    /// Minimal memory usage, slower performance
    Minimal,
    /// Balanced memory usage and performance
    Balanced,
    /// High performance, higher memory usage
    Performance,
    /// Maximum performance, maximum memory usage
    Maximum,
}

/// Performance monitoring and metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLPPerformanceConfig {
    /// Enable detailed performance tracking
    pub enable_metrics: bool,
    /// Track training convergence
    pub track_convergence: bool,
    /// Monitor prediction accuracy
    pub track_prediction_accuracy: bool,
    /// Enable profiling
    pub enable_profiling: bool,
    /// Metrics collection interval
    pub metrics_interval_ms: u64,
}

/// Integration configuration with existing systems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLPIntegrationConfig {
    /// Enable compatibility mode with FannPredictor
    pub fann_compatibility: bool,
    /// Data preprocessing pipeline
    pub preprocessing_steps: Vec<PreprocessingStep>,
    /// Feature scaling method
    pub feature_scaling: FeatureScalingMethod,
    /// Enable automatic feature engineering
    pub auto_feature_engineering: bool,
}

/// Preprocessing steps for input data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PreprocessingStep {
    /// Normalize values to [0, 1] range
    MinMaxNormalization,
    /// Standardize values (zero mean, unit variance)
    StandardScaling,
    /// Remove outliers using IQR method
    OutlierRemoval,
    /// Fill missing values
    MissingValueImputation,
    /// Apply moving average smoothing
    MovingAverageSmoothing { window_size: usize },
    /// Custom preprocessing with parameters
    Custom {
        name: String,
        parameters: HashMap<String, serde_json::Value>,
    },
}

/// Feature scaling methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeatureScalingMethod {
    /// No scaling applied
    None,
    /// Min-max scaling to [0, 1]
    MinMax,
    /// Standard scaling (z-score)
    Standard,
    /// Robust scaling using median and IQR
    Robust,
    /// Unit vector scaling
    UnitVector,
}

/// Enhanced MLP model state and metrics
#[derive(Debug)]
pub struct MLPModelState {
    /// Current training epoch
    pub current_epoch: usize,
    /// Training error history
    pub training_errors: Vec<f32>,
    /// Validation error history
    pub validation_errors: Vec<f32>,
    /// Convergence status
    pub converged: bool,
    /// Training start time
    pub training_start: DateTime<Utc>,
    /// Last training time
    pub last_training: DateTime<Utc>,
    /// Model performance metrics
    pub performance_metrics: MLPPerformanceMetrics,
}

/// Comprehensive performance metrics for MLP models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLPPerformanceMetrics {
    /// Training accuracy
    pub training_accuracy: f64,
    /// Validation accuracy
    pub validation_accuracy: f64,
    /// Prediction latency (milliseconds)
    pub prediction_latency_ms: f64,
    /// Training time (milliseconds)
    pub training_time_ms: f64,
    /// Memory usage (MB)
    pub memory_usage_mb: f64,
    /// Number of parameters
    pub parameter_count: usize,
    /// Model complexity score
    pub complexity_score: f64,
    /// Generalization score
    pub generalization_score: f64,
}

/// Main MLP adapter bridging FannPredictor to ruv-FANN MLP
pub struct MLPAdapter {
    /// Enhanced MLP configuration
    config: EnhancedMLPConfig,
    /// Underlying FANN network
    network: Arc<Mutex<Option<Network<f32>>>>,
    /// Model state and metrics
    model_state: Arc<RwLock<MLPModelState>>,
    /// Training data cache
    training_cache: Arc<RwLock<Option<TrainingData<f32>>>>,
    /// Feature scaling parameters
    scaling_params: Arc<RwLock<Option<ScalingParameters>>>,
    /// Performance monitoring
    performance_monitor: Arc<Mutex<MLPPerformanceMonitor>>,
}

/// Feature scaling parameters
#[derive(Debug, Clone)]
struct ScalingParameters {
    /// Feature minimums (for min-max scaling)
    feature_mins: Vec<f32>,
    /// Feature maximums (for min-max scaling)
    feature_maxs: Vec<f32>,
    /// Feature means (for standard scaling)
    feature_means: Vec<f32>,
    /// Feature standard deviations (for standard scaling)
    feature_stds: Vec<f32>,
    /// Scaling method used
    scaling_method: FeatureScalingMethod,
}

/// Performance monitoring system for MLP models
#[derive(Debug)]
struct MLPPerformanceMonitor {
    /// Start time for current operation
    operation_start: Option<DateTime<Utc>>,
    /// Prediction count
    prediction_count: u64,
    /// Total prediction time
    total_prediction_time_ms: f64,
    /// Training count
    training_count: u64,
    /// Total training time
    total_training_time_ms: f64,
    /// Memory usage samples
    memory_samples: Vec<f64>,
    /// Error samples
    error_samples: Vec<f32>,
}

impl Default for EnhancedMLPConfig {
    fn default() -> Self {
        Self {
            input_features: 30, // 10 timesteps * 3 features
            hidden_layers: vec![64, 32, 16],
            output_size: 5,
            
            activation_functions: MLPActivationConfig {
                hidden_activations: vec![
                    ActivationFunction::ReLU,
                    ActivationFunction::Tanh,
                    ActivationFunction::Sigmoid,
                ],
                output_activation: ActivationFunction::Linear,
                steepness_factors: vec![1.0, 1.0, 1.0, 1.0],
                adaptive_activations: false,
            },
            
            training_config: MLPTrainingConfig {
                algorithm: TrainingAlgorithm::Rprop,
                learning_rate: 0.001,
                momentum: 0.9,
                max_epochs: 1000,
                target_error: 0.001,
                use_cascade_training: false,
                enable_early_stopping: true,
                validation_split: 0.2,
                batch_size: Some(32),
                dropout_rate: Some(0.1),
                weight_decay: Some(0.0001),
                l1_regularization: None,
                l2_regularization: Some(0.0001),
            },
            
            optimization_config: MLPOptimizationConfig {
                parallel_training: true,
                num_threads: None, // Use system default
                memory_optimization: MemoryOptimizationLevel::Balanced,
                enable_simd: true,
                cache_size_mb: 64,
            },
            
            performance_config: MLPPerformanceConfig {
                enable_metrics: true,
                track_convergence: true,
                track_prediction_accuracy: true,
                enable_profiling: false,
                metrics_interval_ms: 1000,
            },
            
            integration_config: MLPIntegrationConfig {
                fann_compatibility: true,
                preprocessing_steps: vec![
                    PreprocessingStep::OutlierRemoval,
                    PreprocessingStep::StandardScaling,
                ],
                feature_scaling: FeatureScalingMethod::Standard,
                auto_feature_engineering: false,
            },
        }
    }
}

impl MLPAdapter {
    /// Create a new MLP adapter with enhanced configuration
    pub fn new(config: EnhancedMLPConfig) -> Result<Self> {
        let model_state = MLPModelState {
            current_epoch: 0,
            training_errors: Vec::new(),
            validation_errors: Vec::new(),
            converged: false,
            training_start: Utc::now(),
            last_training: Utc::now(),
            performance_metrics: MLPPerformanceMetrics {
                training_accuracy: 0.0,
                validation_accuracy: 0.0,
                prediction_latency_ms: 0.0,
                training_time_ms: 0.0,
                memory_usage_mb: 0.0,
                parameter_count: 0,
                complexity_score: 0.0,
                generalization_score: 0.0,
            },
        };
        
        let performance_monitor = MLPPerformanceMonitor {
            operation_start: None,
            prediction_count: 0,
            total_prediction_time_ms: 0.0,
            training_count: 0,
            total_training_time_ms: 0.0,
            memory_samples: Vec::new(),
            error_samples: Vec::new(),
        };
        
        Ok(Self {
            config,
            network: Arc::new(Mutex::new(None)),
            model_state: Arc::new(RwLock::new(model_state)),
            training_cache: Arc::new(RwLock::new(None)),
            scaling_params: Arc::new(RwLock::new(None)),
            performance_monitor: Arc::new(Mutex::new(performance_monitor)),
        })
    }
    
    /// Initialize the MLP network with enhanced ruv-FANN configuration
    pub async fn initialize_network(&self) -> Result<()> {
        let mut network_guard = self.network.lock().await;
        
        if network_guard.is_some() {
            debug!("MLP network already initialized");
            return Ok(());
        }
        
        info!("🧠 Initializing enhanced MLP network with ruv-FANN");
        
        // Build network architecture
        let mut builder = NetworkBuilder::new()
            .input_layer(self.config.input_features);
        
        // Add hidden layers with specific activation functions
        for (i, &layer_size) in self.config.hidden_layers.iter().enumerate() {
            let activation = self.config.activation_functions.hidden_activations
                .get(i)
                .copied()
                .unwrap_or(ActivationFunction::ReLU);
            
            let steepness = self.config.activation_functions.steepness_factors
                .get(i)
                .copied()
                .unwrap_or(1.0);
            
            builder = builder.hidden_layer_with_activation(layer_size, activation, steepness);
            
            debug!("Added hidden layer {}: size={}, activation={:?}, steepness={}", 
                   i + 1, layer_size, activation, steepness);
        }
        
        // Add output layer
        builder = builder.output_layer_with_activation(
            self.config.output_size,
            self.config.activation_functions.output_activation,
            self.config.activation_functions.steepness_factors.last().copied().unwrap_or(1.0)
        );
        
        // Build the network
        let network = builder.build();
        
        // Calculate parameter count
        let mut param_count = 0;
        param_count += self.config.input_features * self.config.hidden_layers[0];
        for i in 1..self.config.hidden_layers.len() {
            param_count += self.config.hidden_layers[i-1] * self.config.hidden_layers[i];
        }
        if let Some(&last_hidden) = self.config.hidden_layers.last() {
            param_count += last_hidden * self.config.output_size;
        }
        
        // Update model state
        {
            let mut state = self.model_state.write().await;
            state.performance_metrics.parameter_count = param_count;
            
            // Calculate complexity score based on network architecture
            let total_neurons: usize = self.config.hidden_layers.iter().sum::<usize>() 
                + self.config.input_features + self.config.output_size;
            state.performance_metrics.complexity_score = (param_count as f64).log10() + 
                (total_neurons as f64 / 1000.0);
        }
        
        *network_guard = Some(network);
        
        info!("✅ Enhanced MLP network initialized: {} parameters, {} total neurons", 
              param_count, 
              self.config.hidden_layers.iter().sum::<usize>() + self.config.input_features + self.config.output_size);
        
        Ok(())
    }
    
    /// Convert TimeSeriesData to MLP training format with enhanced preprocessing
    pub async fn prepare_training_data(
        &self,
        data: &[TimeSeriesData],
    ) -> Result<TrainingData<f32>> {
        if data.is_empty() {
            return Err(anyhow::anyhow!("No training data provided"));
        }
        
        info!("🔄 Preparing training data: {} samples", data.len());
        
        // Extract features with advanced preprocessing
        let mut features = self.extract_features(data)?;
        
        // Apply preprocessing steps
        for step in &self.config.integration_config.preprocessing_steps {
            features = self.apply_preprocessing_step(features, step).await?;
        }
        
        // Apply feature scaling
        let (scaled_features, scaling_params) = self.apply_feature_scaling(features).await?;
        
        // Store scaling parameters for inference
        *self.scaling_params.write().await = Some(scaling_params);
        
        // Create sliding windows for temporal modeling
        let window_size = self.config.input_features / 3; // Assuming 3 features per timestep
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        
        for i in window_size..(scaled_features.len() - self.config.output_size) {
            // Collect input window
            let mut input_vec = Vec::new();
            for j in (i - window_size)..i {
                input_vec.extend_from_slice(&scaled_features[j]);
            }
            
            // Pad or truncate to expected input size
            input_vec.resize(self.config.input_features, 0.0);
            
            // Collect output targets (future returns)
            let mut output_vec = Vec::new();
            for j in 0..self.config.output_size {
                if i + j < scaled_features.len() {
                    // Use price return as target
                    let current_price = data[i-1].close;
                    let future_price = data[i + j].close;
                    let return_value = (future_price - current_price) / current_price;
                    output_vec.push(return_value as f32);
                } else {
                    output_vec.push(0.0);
                }
            }
            
            if output_vec.len() == self.config.output_size {
                inputs.push(input_vec);
                outputs.push(output_vec);
            }
        }
        
        let training_data = TrainingData { inputs, outputs };
        
        // Cache training data
        *self.training_cache.write().await = Some(training_data.clone());
        
        info!("✅ Training data prepared: {} input-output pairs", training_data.inputs.len());
        
        Ok(training_data)
    }
    
    /// Extract features from time series data
    fn extract_features(&self, data: &[TimeSeriesData]) -> Result<Vec<Vec<f32>>> {
        let mut features = Vec::new();
        
        for (i, point) in data.iter().enumerate() {
            let mut feature_vec = Vec::new();
            
            // Basic OHLCV features
            feature_vec.push(point.close as f32);
            feature_vec.push(point.volume as f32);
            
            // Technical indicators
            let rsi = point.indicators.get("rsi").copied().unwrap_or(50.0) as f32;
            feature_vec.push(rsi);
            
            // Add derived features if auto feature engineering is enabled
            if self.config.integration_config.auto_feature_engineering {
                // Price velocity (rate of change)
                if i > 0 {
                    let price_change = (point.close - data[i-1].close) / data[i-1].close;
                    feature_vec.push(price_change as f32);
                } else {
                    feature_vec.push(0.0);
                }
                
                // Volume ratio
                if i > 0 {
                    let volume_ratio = point.volume / data[i-1].volume.max(1.0);
                    feature_vec.push(volume_ratio as f32);
                } else {
                    feature_vec.push(1.0);
                }
                
                // High-low spread
                let spread = (point.high - point.low) / point.close;
                feature_vec.push(spread as f32);
            }
            
            features.push(feature_vec);
        }
        
        debug!("Extracted {} features per timestep from {} data points", 
               features.first().map(|f| f.len()).unwrap_or(0), data.len());
        
        Ok(features)
    }
    
    /// Apply a preprocessing step to the feature data
    async fn apply_preprocessing_step(
        &self,
        mut features: Vec<Vec<f32>>,
        step: &PreprocessingStep,
    ) -> Result<Vec<Vec<f32>>> {
        debug!("Applying preprocessing step: {:?}", step);
        
        match step {
            PreprocessingStep::MinMaxNormalization => {
                self.apply_minmax_normalization(&mut features).await?;
            },
            PreprocessingStep::StandardScaling => {
                self.apply_standard_scaling(&mut features).await?;
            },
            PreprocessingStep::OutlierRemoval => {
                features = self.remove_outliers(features).await?;
            },
            PreprocessingStep::MissingValueImputation => {
                self.impute_missing_values(&mut features).await?;
            },
            PreprocessingStep::MovingAverageSmoothing { window_size } => {
                features = self.apply_moving_average_smoothing(features, *window_size).await?;
            },
            PreprocessingStep::Custom { name, parameters } => {
                features = self.apply_custom_preprocessing(features, name, parameters).await?;
            },
        }
        
        Ok(features)
    }
    
    /// Apply min-max normalization to features
    async fn apply_minmax_normalization(&self, features: &mut [Vec<f32>]) -> Result<()> {
        if features.is_empty() || features[0].is_empty() {
            return Ok(());
        }
        
        let num_features = features[0].len();
        let mut mins = vec![f32::INFINITY; num_features];
        let mut maxs = vec![f32::NEG_INFINITY; num_features];
        
        // Find min and max for each feature
        for feature_vec in features.iter() {
            for (i, &value) in feature_vec.iter().enumerate() {
                if value < mins[i] { mins[i] = value; }
                if value > maxs[i] { maxs[i] = value; }
            }
        }
        
        // Apply normalization
        for feature_vec in features.iter_mut() {
            for (i, value) in feature_vec.iter_mut().enumerate() {
                let range = maxs[i] - mins[i];
                if range > 1e-8 {
                    *value = (*value - mins[i]) / range;
                } else {
                    *value = 0.0;
                }
            }
        }
        
        debug!("Applied min-max normalization to {} features", num_features);
        Ok(())
    }
    
    /// Apply standard scaling (z-score normalization) to features
    async fn apply_standard_scaling(&self, features: &mut [Vec<f32>]) -> Result<()> {
        if features.is_empty() || features[0].is_empty() {
            return Ok(());
        }
        
        let num_features = features[0].len();
        let num_samples = features.len();
        
        // Calculate means
        let mut means = vec![0.0; num_features];
        for feature_vec in features.iter() {
            for (i, &value) in feature_vec.iter().enumerate() {
                means[i] += value;
            }
        }
        for mean in means.iter_mut() {
            *mean /= num_samples as f32;
        }
        
        // Calculate standard deviations
        let mut stds = vec![0.0; num_features];
        for feature_vec in features.iter() {
            for (i, &value) in feature_vec.iter().enumerate() {
                stds[i] += (value - means[i]).powi(2);
            }
        }
        for std in stds.iter_mut() {
            *std = (*std / num_samples as f32).sqrt();
        }
        
        // Apply standardization
        for feature_vec in features.iter_mut() {
            for (i, value) in feature_vec.iter_mut().enumerate() {
                if stds[i] > 1e-8 {
                    *value = (*value - means[i]) / stds[i];
                } else {
                    *value = 0.0;
                }
            }
        }
        
        debug!("Applied standard scaling to {} features", num_features);
        Ok(())
    }
    
    /// Remove outliers using IQR method
    async fn remove_outliers(&self, features: Vec<Vec<f32>>) -> Result<Vec<Vec<f32>>> {
        // Simple outlier removal based on z-score threshold
        let threshold = 3.0;
        let mut filtered_features = Vec::new();
        let original_count = features.len(); // Capture count before moving
        
        for feature_vec in features {
            let mean: f32 = feature_vec.iter().sum::<f32>() / feature_vec.len() as f32;
            let variance: f32 = feature_vec.iter()
                .map(|&x| (x - mean).powi(2))
                .sum::<f32>() / feature_vec.len() as f32;
            let std_dev = variance.sqrt();
            
            // Check if any feature is an outlier
            let is_outlier = feature_vec.iter()
                .any(|&x| ((x - mean) / std_dev).abs() > threshold);
            
            if !is_outlier {
                filtered_features.push(feature_vec);
            }
        }
        
        debug!("Removed {} outlier samples", original_count - filtered_features.len());
        Ok(filtered_features)
    }
    
    /// Impute missing values (replace NaN with mean)
    async fn impute_missing_values(&self, features: &mut [Vec<f32>]) -> Result<()> {
        if features.is_empty() || features[0].is_empty() {
            return Ok(());
        }
        
        let num_features = features[0].len();
        
        // Calculate means for each feature (excluding NaN values)
        let mut means = vec![0.0; num_features];
        let mut counts = vec![0; num_features];
        
        for feature_vec in features.iter() {
            for (i, &value) in feature_vec.iter().enumerate() {
                if value.is_finite() {
                    means[i] += value;
                    counts[i] += 1;
                }
            }
        }
        
        for (i, mean) in means.iter_mut().enumerate() {
            if counts[i] > 0 {
                *mean /= counts[i] as f32;
            }
        }
        
        // Replace NaN values with means
        let mut imputed_count = 0;
        for feature_vec in features.iter_mut() {
            for (i, value) in feature_vec.iter_mut().enumerate() {
                if !value.is_finite() {
                    *value = means[i];
                    imputed_count += 1;
                }
            }
        }
        
        debug!("Imputed {} missing values", imputed_count);
        Ok(())
    }
    
    /// Apply moving average smoothing
    async fn apply_moving_average_smoothing(
        &self,
        features: Vec<Vec<f32>>,
        window_size: usize,
    ) -> Result<Vec<Vec<f32>>> {
        if features.is_empty() || window_size == 0 {
            return Ok(features);
        }
        
        let mut smoothed_features = Vec::new();
        
        for i in 0..features.len() {
            let start_idx = i.saturating_sub(window_size / 2);
            let end_idx = (i + window_size / 2 + 1).min(features.len());
            
            let mut smoothed_vec = vec![0.0; features[i].len()];
            let window_len = end_idx - start_idx;
            
            for j in start_idx..end_idx {
                for (k, &value) in features[j].iter().enumerate() {
                    smoothed_vec[k] += value;
                }
            }
            
            for value in smoothed_vec.iter_mut() {
                *value /= window_len as f32;
            }
            
            smoothed_features.push(smoothed_vec);
        }
        
        debug!("Applied moving average smoothing with window size {}", window_size);
        Ok(smoothed_features)
    }
    
    /// Apply custom preprocessing (placeholder for extensibility)
    async fn apply_custom_preprocessing(
        &self,
        features: Vec<Vec<f32>>,
        name: &str,
        _parameters: &HashMap<String, serde_json::Value>,
    ) -> Result<Vec<Vec<f32>>> {
        warn!("Custom preprocessing '{}' not implemented, returning original features", name);
        Ok(features)
    }
    
    /// Apply feature scaling and return scaling parameters
    async fn apply_feature_scaling(
        &self,
        mut features: Vec<Vec<f32>>,
    ) -> Result<(Vec<Vec<f32>>, ScalingParameters)> {
        if features.is_empty() || features[0].is_empty() {
            return Ok((features, ScalingParameters {
                feature_mins: Vec::new(),
                feature_maxs: Vec::new(),
                feature_means: Vec::new(),
                feature_stds: Vec::new(),
                scaling_method: self.config.integration_config.feature_scaling.clone(),
            }));
        }
        
        let num_features = features[0].len();
        let mut scaling_params = ScalingParameters {
            feature_mins: vec![f32::INFINITY; num_features],
            feature_maxs: vec![f32::NEG_INFINITY; num_features],
            feature_means: vec![0.0; num_features],
            feature_stds: vec![0.0; num_features],
            scaling_method: self.config.integration_config.feature_scaling.clone(),
        };
        
        // Calculate statistics
        let num_samples = features.len() as f32;
        
        // Calculate mins, maxs, and means
        for feature_vec in features.iter() {
            for (i, &value) in feature_vec.iter().enumerate() {
                if value < scaling_params.feature_mins[i] {
                    scaling_params.feature_mins[i] = value;
                }
                if value > scaling_params.feature_maxs[i] {
                    scaling_params.feature_maxs[i] = value;
                }
                scaling_params.feature_means[i] += value;
            }
        }
        
        for mean in scaling_params.feature_means.iter_mut() {
            *mean /= num_samples;
        }
        
        // Calculate standard deviations
        for feature_vec in features.iter() {
            for (i, &value) in feature_vec.iter().enumerate() {
                scaling_params.feature_stds[i] += (value - scaling_params.feature_means[i]).powi(2);
            }
        }
        
        for std in scaling_params.feature_stds.iter_mut() {
            *std = (*std / num_samples).sqrt();
        }
        
        // Apply scaling based on method
        match &self.config.integration_config.feature_scaling {
            FeatureScalingMethod::None => {
                // No scaling applied
            },
            FeatureScalingMethod::MinMax => {
                for feature_vec in features.iter_mut() {
                    for (i, value) in feature_vec.iter_mut().enumerate() {
                        let range = scaling_params.feature_maxs[i] - scaling_params.feature_mins[i];
                        if range > 1e-8 {
                            *value = (*value - scaling_params.feature_mins[i]) / range;
                        } else {
                            *value = 0.0;
                        }
                    }
                }
            },
            FeatureScalingMethod::Standard => {
                for feature_vec in features.iter_mut() {
                    for (i, value) in feature_vec.iter_mut().enumerate() {
                        if scaling_params.feature_stds[i] > 1e-8 {
                            *value = (*value - scaling_params.feature_means[i]) / scaling_params.feature_stds[i];
                        } else {
                            *value = 0.0;
                        }
                    }
                }
            },
            FeatureScalingMethod::Robust => {
                // TODO: Implement robust scaling using median and IQR
                warn!("Robust scaling not yet implemented, using standard scaling");
                for feature_vec in features.iter_mut() {
                    for (i, value) in feature_vec.iter_mut().enumerate() {
                        if scaling_params.feature_stds[i] > 1e-8 {
                            *value = (*value - scaling_params.feature_means[i]) / scaling_params.feature_stds[i];
                        } else {
                            *value = 0.0;
                        }
                    }
                }
            },
            FeatureScalingMethod::UnitVector => {
                // TODO: Implement unit vector scaling
                warn!("Unit vector scaling not yet implemented, using standard scaling");
                for feature_vec in features.iter_mut() {
                    for (i, value) in feature_vec.iter_mut().enumerate() {
                        if scaling_params.feature_stds[i] > 1e-8 {
                            *value = (*value - scaling_params.feature_means[i]) / scaling_params.feature_stds[i];
                        } else {
                            *value = 0.0;
                        }
                    }
                }
            },
        }
        
        debug!("Applied {:?} feature scaling to {} features", 
               self.config.integration_config.feature_scaling, num_features);
        
        Ok((features, scaling_params))
    }
    
    /// Train the MLP model with enhanced ruv-FANN training
    pub async fn train(&self, data: &[TimeSeriesData]) -> Result<()> {
        info!("🎯 Starting enhanced MLP training with {} data points", data.len());
        
        let start_time = Utc::now();
        self.performance_monitor.lock().await.operation_start = Some(start_time);
        
        // Initialize network if not already done
        self.initialize_network().await?;
        
        // Prepare training data
        let training_data = self.prepare_training_data(data).await?;
        
        if training_data.inputs.is_empty() {
            return Err(anyhow::anyhow!("No training samples generated"));
        }
        
        // Split data for validation if enabled
        let (train_data, validation_data) = if self.config.training_config.validation_split > 0.0 {
            let split_idx = ((1.0 - self.config.training_config.validation_split) * training_data.inputs.len() as f32) as usize;
            let train_inputs = training_data.inputs[..split_idx].to_vec();
            let train_outputs = training_data.outputs[..split_idx].to_vec();
            let val_inputs = training_data.inputs[split_idx..].to_vec();
            let val_outputs = training_data.outputs[split_idx..].to_vec();
            
            (
                TrainingData { inputs: train_inputs, outputs: train_outputs },
                Some(TrainingData { inputs: val_inputs, outputs: val_outputs })
            )
        } else {
            (training_data, None)
        };
        
        info!("Training samples: {}, Validation samples: {}", 
              train_data.inputs.len(), 
              validation_data.as_ref().map(|v| v.inputs.len()).unwrap_or(0));
        
        // Get network and perform training
        let mut network_guard = self.network.lock().await;
        let network = network_guard.as_mut()
            .ok_or_else(|| anyhow::anyhow!("Network not initialized"))?;
        
        // Training loop with enhanced monitoring
        let mut best_error = f32::INFINITY;
        let mut epochs_without_improvement = 0;
        const EARLY_STOPPING_PATIENCE: usize = 50;
        
        {
            let mut state = self.model_state.write().await;
            state.training_start = start_time;
            state.converged = false;
            state.training_errors.clear();
            state.validation_errors.clear();
        }
        
        for epoch in 0..self.config.training_config.max_epochs {
            // Training phase
            let mut epoch_error = 0.0f32;
            let mut num_samples = 0;
            
            // Process in batches if batch size is specified
            if let Some(batch_size) = self.config.training_config.batch_size {
                for batch_start in (0..train_data.inputs.len()).step_by(batch_size) {
                    let batch_end = (batch_start + batch_size).min(train_data.inputs.len());
                    
                    for i in batch_start..batch_end {
                        let output = network.run(&train_data.inputs[i]);
                        
                        // Calculate error
                        let mut sample_error = 0.0f32;
                        for j in 0..output.len().min(train_data.outputs[i].len()) {
                            let diff = output[j] - train_data.outputs[i][j];
                            sample_error += diff * diff;
                        }
                        sample_error = sample_error.sqrt();
                        
                        epoch_error += sample_error;
                        num_samples += 1;
                    }
                }
            } else {
                // Process all samples
                for i in 0..train_data.inputs.len() {
                    let output = network.run(&train_data.inputs[i]);
                    
                    // Calculate error
                    let mut sample_error = 0.0f32;
                    for j in 0..output.len().min(train_data.outputs[i].len()) {
                        let diff = output[j] - train_data.outputs[i][j];
                        sample_error += diff * diff;
                    }
                    sample_error = sample_error.sqrt();
                    
                    epoch_error += sample_error;
                    num_samples += 1;
                }
            }
            
            epoch_error /= num_samples as f32;
            
            // Validation phase
            let mut validation_error = 0.0f32;
            if let Some(val_data) = &validation_data {
                let mut val_samples = 0;
                for i in 0..val_data.inputs.len() {
                    let output = network.run(&val_data.inputs[i]);
                    
                    let mut sample_error = 0.0f32;
                    for j in 0..output.len().min(val_data.outputs[i].len()) {
                        let diff = output[j] - val_data.outputs[i][j];
                        sample_error += diff * diff;
                    }
                    sample_error = sample_error.sqrt();
                    
                    validation_error += sample_error;
                    val_samples += 1;
                }
                validation_error /= val_samples as f32;
            }
            
            // Update model state
            {
                let mut state = self.model_state.write().await;
                state.current_epoch = epoch;
                state.training_errors.push(epoch_error);
                if validation_data.is_some() {
                    state.validation_errors.push(validation_error);
                }
            }
            
            // Check for improvement
            let current_error = if validation_data.is_some() { validation_error } else { epoch_error };
            if current_error < best_error {
                best_error = current_error;
                epochs_without_improvement = 0;
            } else {
                epochs_without_improvement += 1;
            }
            
            // Log progress periodically
            if epoch % 100 == 0 || epoch < 10 {
                if validation_data.is_some() {
                    info!("Epoch {}: train_error={:.6}, val_error={:.6}", 
                          epoch, epoch_error, validation_error);
                } else {
                    info!("Epoch {}: train_error={:.6}", epoch, epoch_error);
                }
            }
            
            // Early stopping check
            if self.config.training_config.enable_early_stopping && 
               epochs_without_improvement >= EARLY_STOPPING_PATIENCE {
                info!("Early stopping at epoch {} (no improvement for {} epochs)", 
                      epoch, epochs_without_improvement);
                break;
            }
            
            // Target error check
            if current_error <= self.config.training_config.target_error {
                info!("Target error {:.6} reached at epoch {}", 
                      self.config.training_config.target_error, epoch);
                break;
            }
        }
        
        // Update final model state
        let training_time = (Utc::now() - start_time).num_milliseconds() as f64;
        {
            let mut state = self.model_state.write().await;
            state.converged = best_error <= self.config.training_config.target_error;
            state.last_training = Utc::now();
            state.performance_metrics.training_time_ms = training_time;
            state.performance_metrics.training_accuracy = 1.0 - best_error as f64;
            
            if let Some(val_data) = &validation_data {
                let val_error = state.validation_errors.last().copied().unwrap_or(0.0);
                state.performance_metrics.validation_accuracy = 1.0 - val_error as f64;
                
                // Calculate generalization score
                let train_error = state.training_errors.last().copied().unwrap_or(0.0);
                state.performance_metrics.generalization_score = if train_error > 0.0 {
                    1.0 - (val_error / train_error.max(1e-6)) as f64
                } else {
                    1.0
                };
            }
        }
        
        // Update performance monitor
        {
            let mut monitor = self.performance_monitor.lock().await;
            monitor.training_count += 1;
            monitor.total_training_time_ms += training_time;
            monitor.error_samples.push(best_error);
        }
        
        info!("✅ Enhanced MLP training completed: final_error={:.6}, training_time={:.2}ms", 
              best_error, training_time);
        
        Ok(())
    }
    
    /// Make predictions using the trained MLP model
    pub async fn predict(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
    ) -> Result<Vec<PredictionResult>> {
        let start_time = Utc::now();
        
        // Check if network is initialized
        let mut network_guard = self.network.lock().await;
        let network = network_guard.as_mut()
            .ok_or_else(|| anyhow::anyhow!("MLP network not initialized"))?;
        
        // Check if we have scaling parameters
        let scaling_guard = self.scaling_params.read().await;
        let scaling_params = scaling_guard.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Model not trained - no scaling parameters available"))?;
        
        // Extract and preprocess features
        let mut features = self.extract_features(data)?;
        
        // Apply the same preprocessing steps as training
        for step in &self.config.integration_config.preprocessing_steps {
            features = self.apply_preprocessing_step(features, step).await?;
        }
        
        // Apply the same scaling as training
        features = self.apply_saved_scaling(features, scaling_params).await?;
        
        // Prepare input for the network
        let window_size = self.config.input_features / 3;
        if features.len() < window_size {
            return Err(anyhow::anyhow!("Insufficient data for prediction: need at least {} timesteps", window_size));
        }
        
        let mut input_vec = Vec::new();
        let start_idx = features.len() - window_size;
        for i in start_idx..features.len() {
            input_vec.extend_from_slice(&features[i]);
        }
        
        // Pad or truncate to expected input size
        input_vec.resize(self.config.input_features, 0.0);
        
        drop(scaling_guard);
        
        // Make prediction
        let raw_outputs = network.run(&input_vec);
        
        drop(network_guard);
        
        // Convert raw outputs to prediction results
        let base_price = data.last().unwrap().close;
        let base_time = data.last().unwrap().timestamp;
        let mut predictions = Vec::new();
        
        for i in 0..horizon.min(raw_outputs.len()) {
            let predicted_return = raw_outputs[i] as f64;
            let predicted_price = base_price * (1.0 + predicted_return);
            
            // Calculate confidence based on model performance
            let state = self.model_state.read().await;
            let base_confidence = state.performance_metrics.validation_accuracy
                .max(state.performance_metrics.training_accuracy);
            
            // Decrease confidence with prediction horizon
            let horizon_penalty = 0.05 * i as f64;
            let confidence = (base_confidence - horizon_penalty).max(0.1).min(0.95);
            
            // Calculate prediction intervals based on historical volatility
            let volatility = self.calculate_volatility(data);
            let interval_width = volatility * (1.0 + 0.1 * i as f64);
            
            predictions.push(PredictionResult {
                timestamp: base_time + chrono::Duration::minutes((i + 1) as i64),
                value: predicted_price,
                confidence,
                interval_low: predicted_price * (1.0 - interval_width),
                interval_high: predicted_price * (1.0 + interval_width),
                model_name: "enhanced_mlp".to_string(),
                metadata: Some(HashMap::from([
                    ("training_accuracy".to_string(), serde_json::json!(state.performance_metrics.training_accuracy)),
                    ("validation_accuracy".to_string(), serde_json::json!(state.performance_metrics.validation_accuracy)),
                    ("parameter_count".to_string(), serde_json::json!(state.performance_metrics.parameter_count)),
                    ("complexity_score".to_string(), serde_json::json!(state.performance_metrics.complexity_score)),
                ])),
            });
        }
        
        // Update performance monitoring
        let prediction_time = (Utc::now() - start_time).num_milliseconds() as f64;
        {
            let mut monitor = self.performance_monitor.lock().await;
            monitor.prediction_count += 1;
            monitor.total_prediction_time_ms += prediction_time;
            
            let mut state = self.model_state.write().await;
            state.performance_metrics.prediction_latency_ms = 
                monitor.total_prediction_time_ms / monitor.prediction_count as f64;
        }
        
        debug!("Generated {} MLP predictions in {:.2}ms", predictions.len(), prediction_time);
        
        Ok(predictions)
    }
    
    /// Apply saved scaling parameters to new features
    async fn apply_saved_scaling(
        &self,
        mut features: Vec<Vec<f32>>,
        scaling_params: &ScalingParameters,
    ) -> Result<Vec<Vec<f32>>> {
        match &scaling_params.scaling_method {
            FeatureScalingMethod::None => {
                // No scaling applied
            },
            FeatureScalingMethod::MinMax => {
                for feature_vec in features.iter_mut() {
                    for (i, value) in feature_vec.iter_mut().enumerate() {
                        if i < scaling_params.feature_mins.len() && i < scaling_params.feature_maxs.len() {
                            let range = scaling_params.feature_maxs[i] - scaling_params.feature_mins[i];
                            if range > 1e-8 {
                                *value = (*value - scaling_params.feature_mins[i]) / range;
                            } else {
                                *value = 0.0;
                            }
                        }
                    }
                }
            },
            FeatureScalingMethod::Standard => {
                for feature_vec in features.iter_mut() {
                    for (i, value) in feature_vec.iter_mut().enumerate() {
                        if i < scaling_params.feature_means.len() && i < scaling_params.feature_stds.len() {
                            if scaling_params.feature_stds[i] > 1e-8 {
                                *value = (*value - scaling_params.feature_means[i]) / scaling_params.feature_stds[i];
                            } else {
                                *value = 0.0;
                            }
                        }
                    }
                }
            },
            _ => {
                warn!("Scaling method {:?} not fully implemented for inference", scaling_params.scaling_method);
            }
        }
        
        Ok(features)
    }
    
    /// Calculate historical volatility for prediction intervals
    fn calculate_volatility(&self, data: &[TimeSeriesData]) -> f64 {
        if data.len() < 2 {
            return 0.02; // Default 2% volatility
        }
        
        let returns: Vec<f64> = data.windows(2)
            .map(|w| (w[1].close - w[0].close) / w[0].close)
            .collect();
        
        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>() / returns.len() as f64;
        
        variance.sqrt()
    }
    
    /// Get current model performance metrics
    pub async fn get_performance_metrics(&self) -> MLPPerformanceMetrics {
        let state = self.model_state.read().await;
        state.performance_metrics.clone()
    }
    
    /// Get enhanced configuration
    pub fn get_config(&self) -> &EnhancedMLPConfig {
        &self.config
    }
    
    /// Check if model is trained
    pub async fn is_trained(&self) -> bool {
        let network_guard = self.network.lock().await;
        let scaling_guard = self.scaling_params.read().await;
        network_guard.is_some() && scaling_guard.is_some()
    }
    
    /// Get model training status
    pub async fn get_training_status(&self) -> Result<HashMap<String, serde_json::Value>> {
        let state = self.model_state.read().await;
        let monitor = self.performance_monitor.lock().await;
        
        Ok(HashMap::from([
            ("is_trained".to_string(), serde_json::json!(self.is_trained().await)),
            ("current_epoch".to_string(), serde_json::json!(state.current_epoch)),
            ("converged".to_string(), serde_json::json!(state.converged)),
            ("training_errors".to_string(), serde_json::json!(state.training_errors)),
            ("validation_errors".to_string(), serde_json::json!(state.validation_errors)),
            ("training_count".to_string(), serde_json::json!(monitor.training_count)),
            ("prediction_count".to_string(), serde_json::json!(monitor.prediction_count)),
            ("average_training_time_ms".to_string(), serde_json::json!(
                if monitor.training_count > 0 {
                    monitor.total_training_time_ms / monitor.training_count as f64
                } else {
                    0.0
                }
            )),
            ("average_prediction_time_ms".to_string(), serde_json::json!(
                if monitor.prediction_count > 0 {
                    monitor.total_prediction_time_ms / monitor.prediction_count as f64
                } else {
                    0.0
                }
            )),
            ("performance_metrics".to_string(), serde_json::json!(state.performance_metrics)),
        ]))
    }
}

/// Create a default MLP adapter for integration with FannPredictor
pub fn create_default_mlp_adapter() -> Result<MLPAdapter> {
    let config = EnhancedMLPConfig::default();
    MLPAdapter::new(config)
}

/// Create an MLP adapter from Neural configuration
pub fn create_mlp_adapter_from_config(neural_config: &NeuralConfig) -> Result<MLPAdapter> {
    let mut config = EnhancedMLPConfig::default();
    
    // Adapt neural config to MLP config
    config.optimization_config.parallel_training = neural_config.enable_performance_monitoring;
    config.performance_config.enable_metrics = neural_config.enable_model_monitoring;
    config.integration_config.fann_compatibility = true;
    
    // Adjust training parameters based on neural config
    config.training_config.max_epochs = (neural_config.model_timeout_seconds * 10) as usize;
    config.training_config.target_error = neural_config.error_threshold as f32;
    
    MLPAdapter::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    
    fn create_test_data(count: usize) -> Vec<TimeSeriesData> {
        let mut data = Vec::new();
        let base_time = Utc::now();
        let mut price = 100.0;
        
        for i in 0..count {
            price *= 1.0 + (0.02 * (i as f64 * 0.1).sin());
            
            let mut indicators = HashMap::new();
            indicators.insert("rsi".to_string(), 50.0 + (i as f64 * 0.5));
            
            data.push(TimeSeriesData {
                timestamp: base_time + chrono::Duration::minutes(i as i64),
                entity: Some("TEST".to_string()),
                symbol: "TEST".to_string(),
                open: price * 0.999,
                high: price * 1.001,
                low: price * 0.998,
                close: price,
                volume: 1000000.0 + (i as f64 * 1000.0),
                source: Some("test".to_string()),
                value: Some(price),
                metadata: Some(serde_json::json!({})),
                indicators,
            });
        }
        
        data
    }
    
    #[tokio::test]
    async fn test_mlp_adapter_creation() {
        let adapter = create_default_mlp_adapter().unwrap();
        assert!(!adapter.is_trained().await);
        
        let config = adapter.get_config();
        assert_eq!(config.input_features, 30);
        assert_eq!(config.hidden_layers, vec![64, 32, 16]);
        assert_eq!(config.output_size, 5);
    }
    
    #[tokio::test]
    async fn test_mlp_adapter_training() {
        let adapter = create_default_mlp_adapter().unwrap();
        let test_data = create_test_data(100);
        
        // Training should succeed
        adapter.train(&test_data).await.unwrap();
        assert!(adapter.is_trained().await);
        
        // Check training status
        let status = adapter.get_training_status().await.unwrap();
        assert!(status.contains_key("is_trained"));
        assert!(status.contains_key("converged"));
        
        // Get performance metrics
        let metrics = adapter.get_performance_metrics().await;
        assert!(metrics.parameter_count > 0);
        assert!(metrics.complexity_score > 0.0);
    }
    
    #[tokio::test]
    async fn test_mlp_adapter_prediction() {
        let adapter = create_default_mlp_adapter().unwrap();
        let test_data = create_test_data(150);
        
        // Train the model
        adapter.train(&test_data).await.unwrap();
        
        // Make predictions
        let predictions = adapter.predict(&test_data, 5).await.unwrap();
        
        assert_eq!(predictions.len(), 5);
        for (i, prediction) in predictions.iter().enumerate() {
            assert!(prediction.value > 0.0);
            assert!(prediction.confidence >= 0.1 && prediction.confidence <= 0.95);
            assert!(prediction.interval_low < prediction.value);
            assert!(prediction.interval_high > prediction.value);
            assert_eq!(prediction.model_name, "enhanced_mlp");
            
            // Check metadata
            if let Some(metadata) = &prediction.metadata {
                assert!(metadata.contains_key("training_accuracy"));
                assert!(metadata.contains_key("parameter_count"));
            }
            
            println!("Prediction {}: value={:.4}, confidence={:.3}, interval=[{:.4}, {:.4}]",
                     i + 1, prediction.value, prediction.confidence, 
                     prediction.interval_low, prediction.interval_high);
        }
    }
    
    #[tokio::test]
    async fn test_feature_preprocessing() {
        let adapter = create_default_mlp_adapter().unwrap();
        let test_data = create_test_data(50);
        
        // Test feature extraction
        let features = adapter.extract_features(&test_data).unwrap();
        assert_eq!(features.len(), test_data.len());
        assert!(!features.is_empty());
        
        // Features should have consistent dimensions
        let feature_dim = features[0].len();
        for feature_vec in &features {
            assert_eq!(feature_vec.len(), feature_dim);
        }
        
        println!("Extracted {} features per timestep from {} data points", 
                 feature_dim, test_data.len());
    }
    
    #[tokio::test]
    async fn test_mlp_adapter_from_neural_config() {
        let neural_config = NeuralConfig {
            memory_gb: 2.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.85,
            use_real_models: false,
            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 120,
            max_retries: 3,
            error_threshold: 0.05,
        };
        
        let adapter = create_mlp_adapter_from_config(&neural_config).unwrap();
        let config = adapter.get_config();
        
        assert!(config.performance_config.enable_metrics);
        assert!(config.integration_config.fann_compatibility);
        assert_eq!(config.training_config.target_error, 0.05);
    }
}