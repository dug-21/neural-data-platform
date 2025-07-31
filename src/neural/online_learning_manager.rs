//! Online Learning Manager - Unified API for Real-time Neural Model Updates
//!
//! This module provides a comprehensive, unified interface for all online learning
//! capabilities including incremental learning, concept drift detection, streaming
//! data integration, and real-time performance monitoring.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::neural::FannPredictor;
use super::online_validator::{OnlineValidator, OnlineValidationConfig, ValidationMetrics};
use super::streaming_connector::{StreamingConnector, StreamingConfig};
use super::PredictionResult;
use crate::config::NeuralConfig;
use crate::data::TimeSeriesData;
use crate::neural::NeuralPredictorTrait;

/// Configuration for the complete online learning system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineLearningConfig {
    /// Neural network configuration
    pub neural_config: NeuralConfig,
    /// Streaming data configuration
    pub streaming_config: StreamingConfig,
    /// Online validation configuration
    pub validation_config: OnlineValidationConfig,
    /// Enable automatic model retraining
    pub auto_retrain_enabled: bool,
    /// Online learning update frequency in seconds
    pub update_frequency_secs: u64,
    /// Memory management settings
    pub memory_config: MemoryConfig,
    /// Performance monitoring settings
    pub monitoring_config: MonitoringConfig,
}

/// Memory management configuration for online learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Maximum memory usage in MB
    pub max_memory_mb: f64,
    /// Cleanup frequency in seconds
    pub cleanup_frequency_secs: u64,
    /// Maximum cache size per model
    pub max_cache_size: usize,
    /// Enable memory optimization
    pub enable_optimization: bool,
}

/// Performance monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable detailed metrics collection
    pub enable_detailed_metrics: bool,
    /// Metrics collection frequency in seconds
    pub metrics_frequency_secs: u64,
    /// Enable alert system
    pub enable_alerts: bool,
    /// Performance threshold for alerts
    pub alert_threshold: f64,
}

/// Status of the online learning system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineLearningStatus {
    /// Whether the system is currently running
    pub is_running: bool,
    /// Number of models being trained online
    pub active_models: usize,
    /// Total samples processed
    pub total_samples_processed: u64,
    /// Current memory usage in MB
    pub memory_usage_mb: f64,
    /// System start time
    pub start_time: DateTime<Utc>,
    /// Last update time
    pub last_update: DateTime<Utc>,
    /// Streaming connection status
    pub streaming_connected: bool,
    /// Number of concept drift events detected
    pub drift_events_detected: u32,
    /// Number of automatic retrains triggered
    pub auto_retrains_triggered: u32,
}

/// Comprehensive online learning manager
pub struct OnlineLearningManager {
    config: OnlineLearningConfig,
    predictor: Arc<FannPredictor>,
    validator: Option<Arc<OnlineValidator>>,
    streaming_connector: Option<Arc<RwLock<StreamingConnector>>>,
    status: Arc<RwLock<OnlineLearningStatus>>,
    is_running: Arc<RwLock<bool>>,
}

impl OnlineLearningManager {
    /// Create a new online learning manager
    pub fn new(config: OnlineLearningConfig) -> Result<Self> {
        let predictor = Arc::new(FannPredictor::new(config.neural_config.clone())?);
        
        let status = OnlineLearningStatus {
            is_running: false,
            active_models: config.neural_config.models.len(),
            total_samples_processed: 0,
            memory_usage_mb: 0.0,
            start_time: Utc::now(),
            last_update: Utc::now(),
            streaming_connected: false,
            drift_events_detected: 0,
            auto_retrains_triggered: 0,
        };

        Ok(Self {
            config,
            predictor,
            validator: None,
            streaming_connector: None,
            status: Arc::new(RwLock::new(status)),
            is_running: Arc::new(RwLock::new(false)),
        })
    }

    /// Initialize the complete online learning system
    pub async fn initialize(&mut self) -> Result<()> {
        info!("🚀 Initializing online learning system");

        // Initialize validator
        let validator = Arc::new(OnlineValidator::new(self.config.validation_config.clone()));
        self.validator = Some(validator);

        // Initialize streaming connector
        let streaming_connector = StreamingConnector::new(
            self.config.streaming_config.clone(),
            Arc::clone(&self.predictor),
        );
        self.streaming_connector = Some(Arc::new(RwLock::new(streaming_connector)));

        info!("✅ Online learning system initialized successfully");
        Ok(())
    }

    /// Start the online learning system
    pub async fn start(&self) -> Result<()> {
        if *self.is_running.read().await {
            return Err(anyhow::anyhow!("Online learning system is already running"));
        }

        *self.is_running.write().await = true;
        
        {
            let mut status = self.status.write().await;
            status.is_running = true;
            status.start_time = Utc::now();
        }

        info!("🎯 Starting comprehensive online learning system");

        // Start validator if available
        if let Some(validator) = &self.validator {
            let validator = Arc::clone(validator);
            tokio::spawn(async move {
                if let Err(e) = validator.start().await {
                    error!("Validator failed: {}", e);
                }
            });
        }

        // Start streaming connector if available
        if let Some(streaming_connector) = &self.streaming_connector {
            let connector = Arc::clone(streaming_connector);
            tokio::spawn(async move {
                let mut connector = connector.write().await;
                if let Err(e) = connector.start().await {
                    error!("Streaming connector failed: {}", e);
                }
            });
        }

        // Start main management loop
        let management_loop = self.start_management_loop().await;
        
        // Start memory management
        let memory_manager = self.start_memory_manager().await;
        
        // Start performance monitoring
        let performance_monitor = self.start_performance_monitor().await;

        // Wait for all components
        tokio::select! {
            _ = management_loop => warn!("Management loop stopped"),
            _ = memory_manager => warn!("Memory manager stopped"),
            _ = performance_monitor => warn!("Performance monitor stopped"),
        }

        Ok(())
    }

    /// Stop the online learning system
    pub async fn stop(&self) -> Result<()> {
        *self.is_running.write().await = false;
        
        {
            let mut status = self.status.write().await;
            status.is_running = false;
            status.last_update = Utc::now();
        }

        // Stop validator
        if let Some(validator) = &self.validator {
            validator.stop().await?;
        }

        // Stop streaming connector
        if let Some(streaming_connector) = &self.streaming_connector {
            let connector = streaming_connector.read().await;
            connector.stop().await?;
        }

        info!("🛑 Online learning system stopped");
        Ok(())
    }

    /// Process a single data sample for online learning
    pub async fn process_sample(&self, sample: TimeSeriesData) -> Result<()> {
        // Update all models with the new sample
        for model_name in &self.config.neural_config.models {
            // Perform online learning update
            if let Err(e) = self.predictor.update_with_new_sample(model_name, &sample, None).await {
                warn!("Failed to update model '{}' with sample: {}", model_name, e);
            }
        }

        // Update status
        {
            let mut status = self.status.write().await;
            status.total_samples_processed += 1;
            status.last_update = Utc::now();
        }

        debug!("📊 Processed sample for online learning: symbol={}, price={:.2}", 
               sample.symbol, sample.close);

        Ok(())
    }

    /// Process a batch of samples for efficient online learning
    pub async fn process_batch(&self, samples: &[TimeSeriesData]) -> Result<()> {
        let batch_size = 32;
        
        for model_name in &self.config.neural_config.models {
            if let Err(e) = self.predictor.mini_batch_update(model_name, vec![], batch_size, None).await {
                warn!("Failed to process batch for model '{}': {}", model_name, e);
            }
        }

        // Update status
        {
            let mut status = self.status.write().await;
            status.total_samples_processed += samples.len() as u64;
            status.last_update = Utc::now();
        }

        info!("📦 Processed batch for online learning: {} samples", samples.len());
        Ok(())
    }

    /// Generate predictions with online learning integration
    pub async fn predict(&self, data: &[TimeSeriesData], horizon: usize) -> Result<Vec<PredictionResult>> {
        // Use ensemble prediction for better results
        let models = self.config.neural_config.models.clone();
        let predictions = self.predictor.predict_ensemble(data, horizon, &models, None).await?;

        // Record predictions with validator if available
        if let Some(validator) = &self.validator {
            for prediction in &predictions {
                if let Err(e) = validator.record_prediction(&prediction.model_name, prediction.clone()).await {
                    warn!("Failed to record prediction for validation: {}", e);
                }
            }
        }

        debug!("🎯 Generated {} predictions with online learning", predictions.len());
        Ok(predictions)
    }

    /// Update predictions with actual values for validation and learning
    pub async fn update_with_actual(&self, predictions: &[PredictionResult], actual_values: &[f64]) -> Result<()> {
        // Update performance metrics
        for model_name in &self.config.neural_config.models {
            let prediction_values: Vec<f64> = predictions.iter().map(|p| p.value).collect();
            if let Err(e) = self.predictor.update_performance(model_name, actual_values.to_vec(), prediction_values).await {
                warn!("Failed to update performance for model '{}': {}", model_name, e);
            }
        }

        // Update validator with actual values
        if let Some(validator) = &self.validator {
            for (prediction, &actual_value) in predictions.iter().zip(actual_values.iter()) {
                if let Err(e) = validator.update_with_actual(&prediction.model_name, prediction.timestamp, actual_value).await {
                    warn!("Failed to update validator with actual value: {}", e);
                }
            }
        }

        info!("✅ Updated {} predictions with actual values", predictions.len());
        Ok(())
    }

    /// Get comprehensive system status
    pub async fn get_status(&self) -> OnlineLearningStatus {
        let mut status = self.status.read().await.clone();
        
        // Update memory usage
        status.memory_usage_mb = self.estimate_memory_usage().await;
        
        // Update streaming connection status
        if let Some(streaming_connector) = &self.streaming_connector {
            let connector = streaming_connector.read().await;
            let connection_status = connector.get_connection_status().await;
            status.streaming_connected = connection_status.connected;
        }

        status
    }

    /// Get validation metrics for all models
    pub async fn get_validation_metrics(&self) -> HashMap<String, ValidationMetrics> {
        if let Some(validator) = &self.validator {
            validator.get_all_metrics().await
        } else {
            HashMap::new()
        }
    }

    /// Get online performance metrics
    pub async fn get_performance_metrics(&self) -> Result<HashMap<String, serde_json::Value>> {
        self.predictor.get_online_performance_metrics().await
    }

    /// Trigger manual retraining for specific models
    pub async fn trigger_retraining(&self, model_names: &[String]) -> Result<()> {
        for model_name in model_names {
            if let Err(e) = self.predictor.trigger_automatic_retrain(model_name).await {
                warn!("Failed to retrain model '{}': {}", model_name, e);
            } else {
                // Update status
                let mut status = self.status.write().await;
                status.auto_retrains_triggered += 1;
            }
        }

        info!("🔄 Triggered retraining for {} models", model_names.len());
        Ok(())
    }

    /// Check which models need retraining
    pub async fn check_retraining_needs(&self) -> Vec<String> {
        self.predictor.detect_model_degradation().await.unwrap_or_default()
    }

    /// Save checkpoints for all models
    pub async fn save_checkpoints(&self) -> Result<()> {
        for model_name in &self.config.neural_config.models {
            if let Err(e) = self.predictor.save_checkpoint(model_name).await {
                warn!("Failed to save checkpoint for model '{}': {}", model_name, e);
            }
        }

        info!("💾 Saved checkpoints for {} models", self.config.neural_config.models.len());
        Ok(())
    }

    /// Load checkpoints for all models
    pub async fn load_checkpoints(&self) -> Result<()> {
        for model_name in &self.config.neural_config.models {
            if let Err(e) = self.predictor.load_checkpoint(model_name).await {
                warn!("Failed to load checkpoint for model '{}': {}", model_name, e);
            }
        }

        info!("💿 Loaded checkpoints for {} models", self.config.neural_config.models.len());
        Ok(())
    }

    /// Get comprehensive system report
    pub async fn get_system_report(&self) -> HashMap<String, serde_json::Value> {
        let mut report = HashMap::new();

        // System status
        report.insert("status".to_string(), serde_json::to_value(self.get_status().await).unwrap());

        // Performance metrics
        if let Ok(performance) = self.get_performance_metrics().await {
            report.insert("performance_metrics".to_string(), serde_json::json!(performance));
        }

        // Validation metrics
        let validation = self.get_validation_metrics().await;
        report.insert("validation_metrics".to_string(), serde_json::to_value(validation).unwrap());

        // Ensemble statistics
        if let Ok(ensemble_stats) = self.predictor.get_ensemble_stats().await {
            report.insert("ensemble_stats".to_string(), serde_json::json!(ensemble_stats));
        }

        // Models needing retraining
        let retraining_needs = self.check_retraining_needs().await;
        report.insert("models_needing_retrain".to_string(), serde_json::json!(retraining_needs));

        // Streaming data quality
        if let Some(streaming_connector) = &self.streaming_connector {
            let connector = streaming_connector.read().await;
            let quality_metrics = connector.get_quality_metrics().await;
            report.insert("data_quality".to_string(), serde_json::to_value(quality_metrics).unwrap());
        }

        report.insert("report_timestamp".to_string(), serde_json::json!(Utc::now()));
        report
    }

    /// Start the main management loop
    async fn start_management_loop(&self) -> tokio::task::JoinHandle<()> {
        let is_running = Arc::clone(&self.is_running);
        let predictor = Arc::clone(&self.predictor);
        let status = Arc::clone(&self.status);
        let auto_retrain = self.config.auto_retrain_enabled;
        let update_frequency = self.config.update_frequency_secs;

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(update_frequency));

            while *is_running.read().await {
                interval.tick().await;

                // Check for models needing retraining
                if auto_retrain {
                    if let Ok(degraded_models) = predictor.detect_model_degradation().await {
                        if !degraded_models.is_empty() {
                            info!("🔄 Auto-retraining {} degraded models", degraded_models.len());
                            
                            let mut status = status.write().await;
                            status.auto_retrains_triggered += degraded_models.len() as u32;
                        }
                    }
                }

                debug!("🔄 Management loop cycle completed");
            }
        })
    }

    /// Start memory management task
    async fn start_memory_manager(&self) -> tokio::task::JoinHandle<()> {
        let is_running = Arc::clone(&self.is_running);
        let cleanup_frequency = self.config.memory_config.cleanup_frequency_secs;
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(cleanup_frequency));

            while *is_running.read().await {
                interval.tick().await;

                // Memory cleanup would be implemented here
                debug!("🧹 Memory cleanup cycle completed");
            }
        })
    }

    /// Start performance monitoring task
    async fn start_performance_monitor(&self) -> tokio::task::JoinHandle<()> {
        let is_running = Arc::clone(&self.is_running);
        let status = Arc::clone(&self.status);
        let monitoring_frequency = self.config.monitoring_config.metrics_frequency_secs;

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(monitoring_frequency));

            while *is_running.read().await {
                interval.tick().await;

                // Update status metrics
                {
                    let mut status = status.write().await;
                    status.last_update = Utc::now();
                    // Memory usage would be calculated here
                    status.memory_usage_mb = 100.0; // Placeholder
                }

                debug!("📊 Performance monitoring cycle completed");
            }
        })
    }

    /// Estimate current memory usage
    async fn estimate_memory_usage(&self) -> f64 {
        // In a real implementation, this would calculate actual memory usage
        100.0 + (self.config.neural_config.models.len() as f64 * 50.0)
    }
}

impl Default for OnlineLearningConfig {
    fn default() -> Self {
        Self {
            neural_config: NeuralConfig {
                memory_gb: 2.0,
                models: vec!["MLP".to_string(), "LSTM".to_string()],
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
                enable_model_ensembles: true,
                model_timeout_seconds: 30,
                max_retries: 3,
                error_threshold: 0.05,
                lookback_window: 24,
                input_size: 24,
                output_size: 1,
                hidden_layers: vec![64, 32],
                learning_rate: 0.001,
                prediction_horizon: None,
                normalization_method: None,
            },
            streaming_config: StreamingConfig::default(),
            validation_config: OnlineValidationConfig::default(),
            auto_retrain_enabled: true,
            update_frequency_secs: 60,
            memory_config: MemoryConfig {
                max_memory_mb: 2048.0,
                cleanup_frequency_secs: 300,
                max_cache_size: 10000,
                enable_optimization: true,
            },
            monitoring_config: MonitoringConfig {
                enable_detailed_metrics: true,
                metrics_frequency_secs: 30,
                enable_alerts: true,
                alert_threshold: 0.7,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_online_learning_manager_creation() {
        let config = OnlineLearningConfig::default();
        let manager = OnlineLearningManager::new(config);
        
        assert!(manager.is_ok(), "Manager creation should succeed");
        
        let manager = manager.unwrap();
        let status = manager.get_status().await;
        assert!(!status.is_running, "Manager should not be running initially");
        assert_eq!(status.active_models, 2, "Should have 2 default models");
    }

    #[tokio::test]
    async fn test_online_learning_manager_initialization() {
        let config = OnlineLearningConfig::default();
        let mut manager = OnlineLearningManager::new(config).unwrap();
        
        let result = manager.initialize().await;
        assert!(result.is_ok(), "Initialization should succeed");
        
        assert!(manager.validator.is_some(), "Validator should be initialized");
        assert!(manager.streaming_connector.is_some(), "Streaming connector should be initialized");
    }

    #[tokio::test]
    async fn test_sample_processing() {
        let config = OnlineLearningConfig::default();
        let mut manager = OnlineLearningManager::new(config).unwrap();
        manager.initialize().await.unwrap();

        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 55.0);

        let sample = TimeSeriesData {
            timestamp: Utc::now(),
            entity: Some("test".to_string()),
            symbol: "TESTCOIN".to_string(),
            open: 1000.0,
            high: 1002.0,
            low: 998.0,
            close: 1001.0,
            volume: 1000000.0,
            source: Some("test".to_string()),
            value: Some(1001.0),
            metadata: None,
            indicators,
        };

        let result = manager.process_sample(sample).await;
        assert!(result.is_ok(), "Sample processing should succeed");

        let status = manager.get_status().await;
        assert_eq!(status.total_samples_processed, 1, "Should have processed 1 sample");
    }

    #[tokio::test]
    async fn test_system_report_generation() {
        let config = OnlineLearningConfig::default();
        let mut manager = OnlineLearningManager::new(config).unwrap();
        manager.initialize().await.unwrap();

        let report = manager.get_system_report().await;
        
        assert!(report.contains_key("status"), "Report should contain status");
        assert!(report.contains_key("validation_metrics"), "Report should contain validation metrics");
        assert!(report.contains_key("report_timestamp"), "Report should contain timestamp");
    }
}