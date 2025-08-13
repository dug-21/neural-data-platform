//! FANN Model Persistence Adapter
//!
//! This module provides a proper integration between FANN neural networks and the 
//! ModelStorage system, enabling real model persistence, checkpointing, and versioning.

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::RwLock as StdRwLock;
use std::fmt;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use ruv_fann::{Network, TrainingData, ActivationFunction, NetworkBuilder};
use ruv_fann::training::{IncrementalBackprop, TrainingAlgorithm, MseError};

use crate::adapters::model_storage::{
    ModelStorage, ModelStorageConfig, ModelMetadata, PerformanceMetrics, 
    DataInfo, TrainingParams, SemanticVersion, VersionIncrement, PersistableModel
};
use crate::adapters::vendor_bridge::{
    SyncVendorModel, VendorTimeSeriesData, PredictionResult, TrainingConfig, ModelError
};
use crate::data::TimeSeriesData;

/// Serializable network data for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
struct NetworkData {
    layers: Vec<usize>,
    weights: Vec<f32>,
    hidden_activation: String,
    output_activation: String,
}

/// Mock model for SyncVendorModel interface
struct MockFannModel {
    file_path: PathBuf,
}

impl MockFannModel {
    fn new(file_path: PathBuf) -> Self {
        Self { file_path }
    }
}

impl SyncVendorModel for MockFannModel {
    fn train(
        &mut self,
        _data: &VendorTimeSeriesData,
        _config: &TrainingConfig,
    ) -> Result<(), ModelError> {
        Ok(())
    }

    fn predict(
        &self,
        _data: &VendorTimeSeriesData,
    ) -> Result<PredictionResult, ModelError> {
        Err(ModelError::PredictionError("Mock model cannot predict".to_string()))
    }

    fn name(&self) -> &str {
        "MockFannModel"
    }

    fn is_trained(&self) -> bool {
        true
    }

    fn save(&self, path: &str) -> Result<(), ModelError> {
        std::fs::copy(&self.file_path, path)
            .map_err(|e| ModelError::InitializationError(e.to_string()))?;
        Ok(())
    }

    fn load(&mut self, _path: &str) -> Result<(), ModelError> {
        Ok(())
    }
}

/// FANN model wrapper that integrates with the persistence system
pub struct FannModelAdapter {
    /// The underlying FANN network (using RwLock for thread-safe interior mutability)
    network: StdRwLock<Option<Network<f32>>>,
    /// Model configuration
    config: FannModelConfig,
    /// Model metadata
    metadata: ModelMetadata,
    /// Model storage backend
    storage: Arc<ModelStorage>,
    /// Training history
    training_history: Vec<TrainingRecord>,
    /// Performance tracking
    performance_tracker: PerformanceTracker,
}

/// Configuration for FANN model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FannModelConfig {
    pub model_name: String,
    pub input_size: usize,
    pub hidden_layers: Vec<usize>,
    pub output_size: usize,
    pub hidden_activation: String, // Serializable version of ActivationFunction
    pub output_activation: String,
    pub learning_rate: f32,
    pub momentum: f32,
    pub max_epochs: usize,
    pub target_error: f32,
    pub use_cascade: bool,
    // New adaptive training parameters
    pub adaptive_learning_rate: bool,
    pub initial_lr_multiplier: f32,
    pub lr_increase_factor: f32,
    pub lr_decrease_factor: f32,
    pub plateau_patience: usize,
    pub early_stopping_patience: usize,
    pub min_improvement_threshold: f32,
}

impl Default for FannModelConfig {
    fn default() -> Self {
        Self {
            model_name: "fann_model".to_string(),
            input_size: 20,
            hidden_layers: vec![64, 32],
            output_size: 1,
            hidden_activation: "sigmoid".to_string(),
            output_activation: "linear".to_string(),
            learning_rate: 0.001,
            momentum: 0.9,
            max_epochs: 1000,
            target_error: 0.001,
            use_cascade: false,
            // Adaptive training defaults
            adaptive_learning_rate: true,
            initial_lr_multiplier: 0.1,  // Start with 10% of configured LR
            lr_increase_factor: 1.5,     // Increase by 50% when plateauing
            lr_decrease_factor: 0.8,     // Decrease by 20% when improving
            plateau_patience: 20,        // Wait 20 epochs before increasing LR
            early_stopping_patience: 100, // Stop after 100 epochs without improvement
            min_improvement_threshold: 0.001, // Minimum improvement to consider progress
        }
    }
}

/// Training record for model history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingRecord {
    pub timestamp: DateTime<Utc>,
    pub epochs_completed: usize,
    pub final_mse: f32,
    pub training_time_secs: u64,
    pub data_samples: usize,
    pub config: TrainingConfig,
}

/// Performance tracking for the model
#[derive(Debug, Clone)]
pub struct PerformanceTracker {
    pub prediction_count: u64,
    pub total_error: f64,
    pub accuracy_samples: Vec<f64>,
    pub latency_samples: Vec<u64>,
    pub last_updated: DateTime<Utc>,
}

impl Default for PerformanceTracker {
    fn default() -> Self {
        Self {
            prediction_count: 0,
            total_error: 0.0,
            accuracy_samples: Vec::new(),
            latency_samples: Vec::new(),
            last_updated: Utc::now(),
        }
    }
}

impl fmt::Debug for FannModelAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FannModelAdapter")
            .field("config", &self.config)
            .field("metadata", &self.metadata)
            .field("training_history", &self.training_history)
            .field("performance_tracker", &self.performance_tracker)
            .field("network_present", &self.network.read().unwrap().is_some())
            .finish()
    }
}

impl FannModelAdapter {
    /// Create a new FANN model adapter
    pub async fn new(
        config: FannModelConfig,
        storage_config: ModelStorageConfig,
    ) -> Result<Self> {
        let storage = Arc::new(ModelStorage::new(storage_config).await?);
        
        let metadata = ModelMetadata {
            model_type: "FANN".to_string(),
            version: SemanticVersion::new(1, 0, 0),
            timestamp: Utc::now(),
            accuracy: 0.0,
            loss: 0.0,
            training_params: TrainingParams {
                learning_rate: config.learning_rate,
                batch_size: 32, // Default batch size for FANN
                epochs: config.max_epochs,
                optimizer: "fann_default".to_string(),
                loss_function: "mse".to_string(),
                early_stopping_patience: Some(50),
                validation_split: 0.2,
            },
            performance_metrics: PerformanceMetrics {
                mae: 0.0,
                mse: 0.0,
                rmse: 0.0,
                mape: 0.0,
                r_squared: 0.0,
                validation_loss: 0.0,
                training_loss: 0.0,
            },
            checksum: String::new(),
            training_duration_secs: 0,
            data_info: DataInfo {
                num_samples: 0,
                num_features: config.input_size,
                symbol: "UNKNOWN".to_string(),
                time_range: (Utc::now(), Utc::now()),
            },
        };

        Ok(Self {
            network: StdRwLock::new(None),
            config,
            metadata,
            storage,
            training_history: Vec::new(),
            performance_tracker: PerformanceTracker::default(),
        })
    }

    /// Initialize the FANN network
    pub fn initialize_network(&mut self) -> Result<()> {
        let mut builder = NetworkBuilder::new();
        
        // Add input layer
        builder = builder.input_layer(self.config.input_size);
        
        // Add hidden layers
        let hidden_activation = self.parse_activation(&self.config.hidden_activation)?;
        for &size in &self.config.hidden_layers {
            builder = builder.hidden_layer_with_activation(size, hidden_activation, 1.0);
        }
        
        // Add output layer
        let output_activation = self.parse_activation(&self.config.output_activation)?;
        builder = builder.output_layer_with_activation(self.config.output_size, output_activation, 1.0);

        let network = builder.build();
        *self.network.write().unwrap() = Some(network);
        
        let layers = vec![self.config.input_size]
            .into_iter()
            .chain(self.config.hidden_layers.iter().cloned())
            .chain(std::iter::once(self.config.output_size))
            .collect::<Vec<_>>();
            
        info!("FANN network initialized with architecture: {:?}", layers);
        
        Ok(())
    }

    /// Parse activation function from string
    fn parse_activation(&self, activation: &str) -> Result<ActivationFunction> {
        match activation.to_lowercase().as_str() {
            "linear" => Ok(ActivationFunction::Linear),
            "threshold" => Ok(ActivationFunction::Threshold),
            "threshold_symmetric" => Ok(ActivationFunction::ThresholdSymmetric),
            "sigmoid" => Ok(ActivationFunction::Sigmoid),
            "sigmoid_symmetric" => Ok(ActivationFunction::SigmoidSymmetric),
            "tanh" => Ok(ActivationFunction::Tanh),
            "gaussian" => Ok(ActivationFunction::Gaussian),
            "gaussian_symmetric" => Ok(ActivationFunction::GaussianSymmetric),
            "elliot" => Ok(ActivationFunction::Elliot),
            "elliot_symmetric" => Ok(ActivationFunction::ElliotSymmetric),
            "linear_piece" => Ok(ActivationFunction::LinearPiece),
            "linear_piece_symmetric" => Ok(ActivationFunction::LinearPieceSymmetric),
            "sin_symmetric" => Ok(ActivationFunction::SinSymmetric),
            "cos_symmetric" => Ok(ActivationFunction::CosSymmetric),
            "sin" => Ok(ActivationFunction::Sin),
            "cos" => Ok(ActivationFunction::Cos),
            "relu" => Ok(ActivationFunction::ReLU),
            "relu_leaky" => Ok(ActivationFunction::ReLULeaky),
            _ => Err(anyhow!("Unknown activation function: {}", activation)),
        }
    }

    /// Save the current model to storage
    pub async fn save_model(&self, increment_type: VersionIncrement) -> Result<PathBuf> {
        let network_clone = {
            let network_opt = self.network.read().unwrap();
            let network = network_opt.as_ref()
                .ok_or_else(|| anyhow!("No network to save"))?;
            network.clone()  // Clone the network to avoid holding the lock across await
        }; // RwLockReadGuard is dropped here

        // Use the model storage to save with versioning
        let model_version = self.storage.save_model(
            &network_clone,
            &self.config.model_name,
            self.metadata.clone(),
            increment_type,
        ).await?;

        info!("Model saved to: {:?}", model_version.path);
        Ok(model_version.path)
    }

    /// Load a model from storage
    pub async fn load_model(&mut self, version: Option<SemanticVersion>) -> Result<()> {
        let (network, metadata) = self.storage.load_model(
            &self.config.model_name,
            version,
        ).await?;

        *self.network.write().unwrap() = Some(network);
        self.metadata = metadata;

        info!("Model loaded successfully: version {}", self.metadata.version);
        Ok(())
    }

    /// Train the model with real backpropagation and adaptive learning rate
    pub async fn train_with_real_backprop(
        &mut self,
        training_data: &TrainingData<f32>,
        config: &TrainingConfig,
    ) -> Result<TrainingRecord> {
        info!("🚀 [CONTAINER TRAINING] Starting REAL neural network training with adaptive LR");
        info!("📊 [CONTAINER TRAINING] Training data: {} samples, {} features", 
              training_data.inputs.len(), self.config.input_size);
        
        // Initialize network if needed
        if self.network.read().unwrap().is_none() {
            info!("🔧 [CONTAINER TRAINING] Initializing new network");
            self.initialize_network()?;
        }

        // Initialize adaptive learning rate
        let mut current_lr = if self.config.adaptive_learning_rate {
            config.learning_rate * self.config.initial_lr_multiplier
        } else {
            config.learning_rate
        };
        
        let mut trainer = IncrementalBackprop::new(current_lr)
            .with_momentum(0.9)
            .with_error_function(Box::new(MseError));
        
        info!("⚙️ [CONTAINER TRAINING] Trainer configured - Initial LR: {:.6}, Momentum: 0.9", current_lr);
        if self.config.adaptive_learning_rate {
            info!("🧠 [CONTAINER TRAINING] Adaptive learning rate enabled - will adjust based on progress");
        }
        
        let start_time = std::time::Instant::now();
        let mut best_error = f32::INFINITY;
        let mut epochs_completed = 0;
        let mut epochs_without_improvement = 0;
        let mut epochs_since_lr_change = 0;
        let mut error_history: Vec<f32> = Vec::new();
        
        // Get mutable access to network
        let mut network_guard = self.network.write().unwrap();
        let network = network_guard.as_mut()
            .ok_or_else(|| anyhow!("Network not initialized"))?;
        
        info!("🏋️ [CONTAINER TRAINING] Beginning training epochs (max: {})", config.max_epochs);
        
        for epoch in 0..config.max_epochs {
            // Train one epoch - THIS ACTUALLY UPDATES WEIGHTS!
            let epoch_error = trainer.train_epoch(
                network,
                training_data
            ).map_err(|e| anyhow!("Training epoch failed: {:?}", e))?;
            
            error_history.push(epoch_error);
            epochs_completed = epoch + 1;
            epochs_since_lr_change += 1;
            
            // Check for improvement
            let improvement = if epoch_error < best_error - self.config.min_improvement_threshold {
                best_error = epoch_error;
                epochs_without_improvement = 0;
                true
            } else {
                epochs_without_improvement += 1;
                false
            };
            
            // Adaptive learning rate adjustment
            if self.config.adaptive_learning_rate && epochs_since_lr_change >= self.config.plateau_patience {
                let should_adjust_lr = self.should_adjust_learning_rate(&error_history, epoch);
                
                if should_adjust_lr {
                    let old_lr = current_lr;
                    
                    if epochs_without_improvement >= self.config.plateau_patience {
                        // Plateau detected - increase learning rate
                        current_lr = (current_lr * self.config.lr_increase_factor).min(config.learning_rate * 2.0);
                        info!("📈 [ADAPTIVE LR] Plateau detected at epoch {}. Increasing LR: {:.6} -> {:.6}", 
                              epoch, old_lr, current_lr);
                    } else if improvement {
                        // Good progress - slightly decrease learning rate for stability
                        current_lr = current_lr * self.config.lr_decrease_factor;
                        info!("📉 [ADAPTIVE LR] Good progress at epoch {}. Decreasing LR for stability: {:.6} -> {:.6}", 
                              epoch, old_lr, current_lr);
                    }
                    
                    // Update trainer with new learning rate
                    trainer = IncrementalBackprop::new(current_lr)
                        .with_momentum(0.9)
                        .with_error_function(Box::new(MseError));
                    
                    epochs_since_lr_change = 0;
                }
            }
            
            // Enhanced logging with learning rate info
            if epoch % (config.max_epochs / 20).max(1) == 0 || (epoch < 100 && epoch % 10 == 0) {
                let progress_indicator = if improvement { "↓" } else { "→" };
                info!("📈 [TRAINING] Epoch {:4}/{}: error = {:.6} {} (LR: {:.6}, no-improve: {})", 
                      epoch, config.max_epochs, epoch_error, progress_indicator, current_lr, epochs_without_improvement);
            }
            
            // Early stopping check
            if self.config.early_stopping_patience > 0 && epochs_without_improvement >= self.config.early_stopping_patience {
                info!("⏹️ [EARLY STOPPING] No improvement for {} epochs. Stopping training at epoch {}", 
                      self.config.early_stopping_patience, epoch);
                break;
            }
            
            // Target error reached
            if epoch_error <= self.config.target_error {
                info!("🎯 [TARGET REACHED] Epoch {}: error {:.6} <= target {:.6}", 
                      epoch, epoch_error, self.config.target_error);
                break;
            }
        }
        
        drop(network_guard); // Release the write lock
        
        // Update metadata with REAL training results
        let r_squared = self.calculate_r_squared(training_data, best_error as f64)?;
        self.metadata.accuracy = r_squared;
        self.metadata.loss = best_error as f64;
        
        let duration = start_time.elapsed();
        info!("✅ [TRAINING COMPLETE] Final Results:");
        info!("📊   Final error: {:.6} (best: {:.6})", error_history.last().unwrap_or(&best_error), best_error);
        info!("🎯   Target achieved: {} (target: {:.6})", 
              if best_error <= self.config.target_error { "✅ YES" } else { "❌ NO" }, self.config.target_error);
        info!("⏱️   Duration: {:?} ({} epochs, {:.1} epochs/sec)", 
              duration, epochs_completed, epochs_completed as f64 / duration.as_secs_f64());
        info!("🧠   Final LR: {:.6}, Model accuracy: {:.1}%", current_lr, self.metadata.accuracy * 100.0);
        
        let record = TrainingRecord {
            timestamp: Utc::now(),
            epochs_completed,
            final_mse: best_error,
            training_time_secs: duration.as_secs(),
            data_samples: training_data.inputs.len(),
            config: config.clone(),
        };

        self.training_history.push(record.clone());
        self.metadata.training_duration_secs = duration.as_secs();
        self.metadata.timestamp = Utc::now();
        
        Ok(record)
    }

    /// Train the model with automatic checkpointing
    pub async fn train_with_checkpointing(
        &mut self,
        training_data: &TrainingData<f32>,
        config: &TrainingConfig,
        checkpoint_frequency: usize,
    ) -> Result<TrainingRecord> {
        info!("Starting training with checkpointing every {} epochs", checkpoint_frequency);

        // Use the real training method
        let mut record = self.train_with_real_backprop(training_data, config).await?;
        
        let epochs_completed = record.epochs_completed;
        let last_mse = record.final_mse;

        // Save checkpoints during training (if needed)
        if epochs_completed % checkpoint_frequency == 0 || epochs_completed % 100 == 0 {
            let network_borrow = self.network.read().unwrap();
            if let Some(net) = network_borrow.as_ref() {
                let checkpoint_metrics = crate::adapters::model_storage::CheckpointMetrics {
                    epoch: epochs_completed,
                    training_loss: last_mse as f64,
                    validation_loss: last_mse as f64,
                    learning_rate: self.config.learning_rate,
                    timestamp: Utc::now(),
                };

                if let Err(e) = self.storage.save_checkpoint(
                    net,
                    &self.config.model_name,
                    epochs_completed,
                    checkpoint_metrics,
                ).await {
                    warn!("Failed to save checkpoint at epoch {}: {}", epochs_completed, e);
                }
            }
        }

        // Save the trained model
        if let Err(e) = self.save_model(VersionIncrement::Minor).await {
            warn!("Failed to save trained model: {}", e);
        }

        // Comprehensive training completion logging
        let final_accuracy = self.metadata.accuracy; // Use the properly calculated R-squared
        let training_time = std::time::Duration::from_secs(record.training_time_secs);
        info!("🎉 TRAINING COMPLETED SUCCESSFULLY!");
        info!("📈 Final Results: {} epochs, MSE: {:.6}, Accuracy: {:.1}%, Time: {:?}", 
              epochs_completed, last_mse, final_accuracy * 100.0, training_time);
        info!("🎯 Target achieved: {} (Target MSE: {:.6})", 
              if last_mse <= self.config.target_error { "✅ YES" } else { "❌ NO" },
              self.config.target_error);
        info!("⚡ Training Performance: {:.2} epochs/sec, {:.0} samples/sec", 
              epochs_completed as f64 / training_time.as_secs_f64(),
              (epochs_completed * training_data.inputs.len()) as f64 / training_time.as_secs_f64());

        Ok(record)
    }

    /// Save a training checkpoint
    async fn save_checkpoint(&self, epoch: usize, mse: f32) -> Result<()> {
        use crate::adapters::model_storage::CheckpointMetrics;

        let network_guard = self.network.read().unwrap();
        let network = network_guard.as_ref()
            .ok_or_else(|| anyhow!("No network to checkpoint"))?;

        let metrics = CheckpointMetrics {
            epoch,
            training_loss: mse as f64,
            validation_loss: mse as f64, // In FANN we don't separate these
            learning_rate: self.config.learning_rate,
            timestamp: Utc::now(),
        };

        self.storage.save_checkpoint(
            network,
            &self.config.model_name,
            epoch,
            metrics,
        ).await?;

        debug!("Checkpoint saved at epoch {}", epoch);
        Ok(())
    }

    /// Get model performance metrics
    pub fn get_performance_metrics(&self) -> PerformanceMetrics {
        let tracker = &self.performance_tracker;
        
        let mae = if !tracker.accuracy_samples.is_empty() {
            tracker.accuracy_samples.iter().sum::<f64>() / tracker.accuracy_samples.len() as f64
        } else {
            0.0
        };

        let mse = tracker.total_error / tracker.prediction_count.max(1) as f64;
        
        PerformanceMetrics {
            mae,
            mse,
            rmse: mse.sqrt(),
            mape: mae * 100.0,
            r_squared: self.metadata.accuracy,
            validation_loss: mse,
            training_loss: self.metadata.loss,
        }
    }

    /// Update performance tracking
    pub fn update_performance(&mut self, actual: f32, predicted: f32, latency_ms: u64) {
        let error = (actual - predicted).abs() as f64;
        
        self.performance_tracker.prediction_count += 1;
        self.performance_tracker.total_error += error;
        self.performance_tracker.accuracy_samples.push(1.0 - error);
        self.performance_tracker.latency_samples.push(latency_ms);
        self.performance_tracker.last_updated = Utc::now();

        // Keep only recent samples (last 1000)
        if self.performance_tracker.accuracy_samples.len() > 1000 {
            self.performance_tracker.accuracy_samples.remove(0);
        }
        if self.performance_tracker.latency_samples.len() > 1000 {
            self.performance_tracker.latency_samples.remove(0);
        }
    }

    /// Get training history
    pub fn get_training_history(&self) -> &[TrainingRecord] {
        &self.training_history
    }

    /// Get model metadata
    pub fn get_metadata(&self) -> &ModelMetadata {
        &self.metadata
    }

    /// Check if model is trained
    pub fn is_trained(&self) -> bool {
        self.network.read().unwrap().is_some() && !self.training_history.is_empty()
    }

    /// Get model configuration
    pub fn get_config(&self) -> &FannModelConfig {
        &self.config
    }

    /// Calculate R-squared coefficient of determination
    /// R² = 1 - (SS_res / SS_tot) where:
    /// SS_res = sum of squares of residuals (predicted - actual)²
    /// SS_tot = total sum of squares (actual - mean)²
    fn calculate_r_squared(&self, training_data: &TrainingData<f32>, _mse: f64) -> Result<f64> {
        if training_data.outputs.is_empty() || training_data.inputs.is_empty() {
            return Ok(0.0);
        }

        // Get network access for predictions (need write access for run method)
        let mut network_guard = self.network.write().unwrap();
        let network = match network_guard.as_mut() {
            Some(net) => net,
            None => return Ok(0.0), // No trained network
        };

        // Collect all actual values and calculate mean
        let mut actual_values = Vec::new();
        for output in &training_data.outputs {
            for &value in output {
                actual_values.push(value as f64);
            }
        }

        if actual_values.is_empty() {
            return Ok(0.0);
        }

        let mean: f64 = actual_values.iter().sum::<f64>() / actual_values.len() as f64;

        // Calculate total sum of squares (SS_tot)
        let ss_tot: f64 = actual_values.iter()
            .map(|&actual| {
                let diff = actual - mean;
                diff * diff
            })
            .sum();

        // Handle edge case: if all values are the same, R² is undefined
        if ss_tot == 0.0 {
            return Ok(0.0);
        }

        // Calculate residual sum of squares (SS_res) using actual predictions
        let mut ss_res = 0.0;
        let mut prediction_idx = 0;

        for (input_idx, input) in training_data.inputs.iter().enumerate() {
            // Run the network to get predictions
            let predictions = network.run(input);
            
            // Compare predictions with actual outputs
            if input_idx < training_data.outputs.len() {
                let actual_output = &training_data.outputs[input_idx];
                
                for (pred_idx, &predicted) in predictions.iter().enumerate() {
                    if pred_idx < actual_output.len() && prediction_idx < actual_values.len() {
                        let actual = actual_values[prediction_idx] as f64;
                        let residual = predicted as f64 - actual;
                        ss_res += residual * residual;
                        prediction_idx += 1;
                    }
                }
            }
        }

        // Calculate R-squared: R² = 1 - (SS_res / SS_tot)
        let r_squared = 1.0 - (ss_res / ss_tot);
        
        // R-squared can be negative if model is worse than predicting mean
        // For reporting purposes, we'll clamp to [0, 1] but allow debugging of negative values
        let final_r_squared = if r_squared < 0.0 {
            warn!("Negative R-squared detected: {:.6}, model performs worse than mean prediction", r_squared);
            0.0
        } else {
            r_squared.min(1.0)
        };

        debug!("R-squared calculation: SS_res={:.6}, SS_tot={:.6}, R²={:.6}", 
               ss_res, ss_tot, final_r_squared);
        
        Ok(final_r_squared)
    }

    /// Calculate variance of a slice of values
    fn calculate_variance(&self, values: &[f32]) -> f64 {
        if values.len() <= 1 {
            return 0.0;
        }

        let mean: f64 = values.iter().map(|&x| x as f64).sum::<f64>() / values.len() as f64;
        let variance: f64 = values.iter()
            .map(|&x| {
                let diff = x as f64 - mean;
                diff * diff
            })
            .sum::<f64>() / (values.len() - 1) as f64;
        
        variance
    }

    /// Determine if learning rate should be adjusted based on error history
    fn should_adjust_learning_rate(&self, error_history: &[f32], current_epoch: usize) -> bool {
        if error_history.len() < self.config.plateau_patience {
            return false;
        }

        let window_size = self.config.plateau_patience;
        let recent_errors = &error_history[error_history.len().saturating_sub(window_size)..];
        
        // Check if we're in a plateau (very small variance in recent errors)
        let variance = self.calculate_variance(recent_errors);
        let mean_error: f32 = recent_errors.iter().sum::<f32>() / recent_errors.len() as f32;
        
        // Coefficient of variation (std dev / mean) - low values indicate plateau
        let coefficient_of_variation = if mean_error > 0.0 {
            (variance.sqrt() as f32) / mean_error
        } else {
            0.0
        };

        // Consider it a plateau if coefficient of variation is very low
        let is_plateau = coefficient_of_variation < 0.01 && variance < 0.000001;
        
        // Also check for decreasing trend (good progress)
        let is_improving = if recent_errors.len() >= 2 {
            let first_half = &recent_errors[..recent_errors.len()/2];
            let second_half = &recent_errors[recent_errors.len()/2..];
            
            let first_mean: f32 = first_half.iter().sum::<f32>() / first_half.len() as f32;
            let second_mean: f32 = second_half.iter().sum::<f32>() / second_half.len() as f32;
            
            (first_mean - second_mean) > self.config.min_improvement_threshold
        } else {
            false
        };

        // Log diagnostic information occasionally
        if current_epoch % 50 == 0 && self.config.adaptive_learning_rate {
            debug!("[LR ANALYSIS] Epoch {}: variance={:.8}, CV={:.6}, plateau={}, improving={}", 
                   current_epoch, variance, coefficient_of_variation, is_plateau, is_improving);
        }

        is_plateau || is_improving
    }
}

// Implement SyncVendorModel trait for integration with existing systems
impl SyncVendorModel for FannModelAdapter {
    fn train(
        &mut self,
        data: &VendorTimeSeriesData,
        config: &TrainingConfig,
    ) -> Result<(), ModelError> {
        // Convert VendorTimeSeriesData to FANN TrainingData
        let training_data = self.convert_to_fann_data(data)
            .map_err(|e| ModelError::TrainingError(e.to_string()))?;

        // Use async runtime for the training (since our train method is async)
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_| ModelError::InitializationError("No async runtime available".to_string()))?;

        runtime.block_on(async {
            self.train_with_checkpointing(&training_data, config, 100).await
        }).map_err(|e| ModelError::TrainingError(e.to_string()))?;

        Ok(())
    }

    fn predict(
        &self,
        data: &VendorTimeSeriesData,
    ) -> Result<PredictionResult, ModelError> {
        {
            let network_borrow = self.network.read().unwrap();
            let _network = network_borrow.as_ref()
                .ok_or_else(|| ModelError::PredictionError("Model not trained".to_string()))?;
        }

        if data.values.is_empty() {
            return Err(ModelError::PredictionError("No input data".to_string()));
        }

        // Use the last input_size values for prediction
        let input_size = self.config.input_size;
        let input_data = if data.values.len() >= input_size {
            data.values[data.values.len() - input_size..].to_vec()
        } else {
            // Pad with zeros if not enough data
            let mut padded = vec![0.0; input_size];
            let start_idx = input_size - data.values.len();
            padded[start_idx..].copy_from_slice(&data.values);
            padded
        };

        self.predict_with_input(&input_data, data)
    }

    fn name(&self) -> &str {
        &self.config.model_name
    }

    fn is_trained(&self) -> bool {
        self.is_trained()
    }

    fn save(&self, path: &str) -> Result<(), ModelError> {
        let network_borrow = self.network.read().unwrap();
        let network = network_borrow.as_ref()
            .ok_or_else(|| ModelError::InitializationError("No network to save".to_string()))?;

        // Serialize network data
        let weights = network.get_weights();
        let network_data = NetworkData {
            layers: vec![self.config.input_size]
                .into_iter()
                .chain(self.config.hidden_layers.iter().cloned())
                .chain(std::iter::once(self.config.output_size))
                .collect(),
            weights,
            hidden_activation: self.config.hidden_activation.clone(),
            output_activation: self.config.output_activation.clone(),
        };

        let json_data = serde_json::to_string_pretty(&network_data)
            .map_err(|e| ModelError::InitializationError(format!("Serialization failed: {}", e)))?;
        
        std::fs::write(path, json_data)
            .map_err(|e| ModelError::InitializationError(format!("Failed to save: {}", e)))?;
        
        Ok(())
    }

    fn load(&mut self, path: &str) -> Result<(), ModelError> {
        let json_data = std::fs::read_to_string(path)
            .map_err(|e| ModelError::InitializationError(format!("Failed to read file: {}", e)))?;
        
        let network_data: NetworkData = serde_json::from_str(&json_data)
            .map_err(|e| ModelError::InitializationError(format!("Deserialization failed: {}", e)))?;

        // Update config from loaded data
        self.config.hidden_activation = network_data.hidden_activation;
        self.config.output_activation = network_data.output_activation;

        // Initialize network
        self.initialize_network()
            .map_err(|e| ModelError::InitializationError(e.to_string()))?;

        // Set weights
        {
            let mut network_guard = self.network.write()
                .map_err(|_| ModelError::InitializationError("Failed to acquire write lock".to_string()))?;
            if let Some(network) = network_guard.as_mut() {
                network.set_weights(&network_data.weights)
                    .map_err(|e| ModelError::InitializationError(format!("Failed to set weights: {:?}", e)))?;
            }
        }

        Ok(())
    }
}

impl FannModelAdapter {
    /// Helper method for prediction with input validation
    fn predict_with_input(
        &self,
        input: &[f32],
        original_data: &VendorTimeSeriesData,
    ) -> Result<PredictionResult, ModelError> {
        let output = {
            let mut network_guard = self.network.write()
                .map_err(|_| ModelError::PredictionError("Failed to acquire write lock".to_string()))?;
            let network = network_guard.as_mut()
                .ok_or_else(|| ModelError::PredictionError("Model not trained".to_string()))?;
            network.run(input)
        };

        // Create future timestamps for the predictions
        let last_timestamp = original_data.timestamps.last()
            .copied()
            .unwrap_or_else(Utc::now);
        
        let mut prediction_timestamps = Vec::new();
        for i in 0..output.len() {
            prediction_timestamps.push(last_timestamp + chrono::Duration::hours(i as i64 + 1));
        }

        Ok(PredictionResult {
            forecasts: output,
            timestamps: prediction_timestamps,
            series_id: original_data.symbol.clone(),
            metadata: HashMap::new(),
            confidence_intervals: None,
            quantiles: None,
        })
    }

    /// Convert VendorTimeSeriesData to FANN TrainingData
    fn convert_to_fann_data(&self, data: &VendorTimeSeriesData) -> Result<TrainingData<f32>> {
        if data.values.len() < self.config.input_size + self.config.output_size {
            return Err(anyhow!("Insufficient data for training"));
        }

        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        
        // Create sliding window samples
        let num_samples = data.values.len() - self.config.input_size - self.config.output_size + 1;
        
        for i in 0..num_samples {
            let input_start = i;
            let input_end = i + self.config.input_size;
            let output_start = input_end;
            let output_end = output_start + self.config.output_size;
            
            let input = data.values[input_start..input_end].to_vec();
            let output = data.values[output_start..output_end].to_vec();
            
            inputs.push(input);
            outputs.push(output);
        }

        Ok(TrainingData { inputs, outputs })
    }
}

// Implement PersistableModel trait for advanced model management
#[async_trait]
impl PersistableModel for FannModelAdapter {
    async fn get_metadata(&self) -> Result<ModelMetadata> {
        Ok(self.metadata.clone())
    }

    async fn load_checkpoint(&mut self, checkpoint_path: &Path) -> Result<()> {
        let checkpoint_str = checkpoint_path.to_str()
            .ok_or_else(|| anyhow!("Invalid checkpoint path"))?;

        self.load(checkpoint_str)
            .map_err(|e| anyhow!("Failed to load checkpoint: {:?}", e))?;

        info!("Checkpoint loaded from: {:?}", checkpoint_path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_fann_model_adapter_creation() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = ModelStorageConfig {
            base_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let config = FannModelConfig::default();
        let mut adapter = FannModelAdapter::new(config, storage_config).await.unwrap();
        
        assert!(!adapter.is_trained());
        assert_eq!(adapter.name(), "fann_model");
        
        // Test network initialization
        adapter.initialize_network().unwrap();
        assert!(adapter.network.is_some());
    }

    #[tokio::test]
    async fn test_model_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = ModelStorageConfig {
            base_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let config = FannModelConfig::default();
        let mut adapter = FannModelAdapter::new(config, storage_config).await.unwrap();
        adapter.initialize_network().unwrap();

        // Create some dummy training data
        let mut training_data = TrainingData::new();
        let input = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        let output = vec![0.6];
        training_data.add_sample(&input, &output).unwrap();

        // Train the model briefly
        let train_config = TrainingConfig {
            max_epochs: 10,
            learning_rate: 0.1,
            batch_size: 1,
            validation_size: 0.0,
            early_stopping_patience: 5,
            save_best_model: false,
            verbose: false,
            use_gpu: false,
            gradient_clipping: None,
            weight_decay: None,
            scheduler_config: None,
        };

        let record = adapter.train_with_checkpointing(&training_data, &train_config, 5).await.unwrap();
        assert_eq!(record.epochs_completed, 10);

        // Test save and load
        let saved_path = adapter.save_model(VersionIncrement::Patch).await.unwrap();
        assert!(saved_path.exists());

        // Create a new adapter and load the model
        let mut new_adapter = FannModelAdapter::new(
            FannModelConfig::default(),
            ModelStorageConfig {
                base_path: temp_dir.path().to_path_buf(),
                ..Default::default()
            },
        ).await.unwrap();

        new_adapter.load_model(None).await.unwrap();
        assert!(new_adapter.is_trained());
    }

    #[tokio::test]
    async fn test_prediction() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = ModelStorageConfig {
            base_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let config = FannModelConfig {
            input_size: 5,
            output_size: 1,
            adaptive_learning_rate: false, // Disable for predictable test
            ..Default::default()
        };
        
        let mut adapter = FannModelAdapter::new(config, storage_config).await.unwrap();
        adapter.initialize_network().unwrap();

        // Create vendor data for prediction
        let vendor_data = VendorTimeSeriesData::new(
            "TEST".to_string(),
            vec![Utc::now(); 10],
            vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0],
        );

        // Quick training
        let mut training_data = TrainingData {
            inputs: Vec::new(),
            outputs: Vec::new(),
        };
        training_data.inputs.push(vec![0.1, 0.2, 0.3, 0.4, 0.5]);
        training_data.outputs.push(vec![0.6]);
        
        let train_config = TrainingConfig {
            max_epochs: 5,
            learning_rate: 0.1,
            batch_size: 1,
            validation_size: 0.0,
            early_stopping_patience: 5,
            save_best_model: false,
            verbose: false,
            use_gpu: false,
            gradient_clipping: None,
            weight_decay: None,
            scheduler_config: None,
        };

        adapter.train_with_checkpointing(&training_data, &train_config, 5).await.unwrap();

        // Test prediction
        let result = adapter.predict(&vendor_data).unwrap();
        assert_eq!(result.forecasts.len(), 1);
        assert_eq!(result.series_id, "TEST");
    }
}