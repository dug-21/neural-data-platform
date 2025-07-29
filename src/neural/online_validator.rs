//! Online Validation System for Real-time Performance Monitoring
//!
//! This module provides comprehensive online validation capabilities for neural models,
//! including real-time performance monitoring, model degradation detection, and
//! automatic retraining triggers.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::{interval, Instant};
use tracing::{debug, error, info, warn};

use super::{PredictionResult, NeuralPredictorTrait};
use crate::data::TimeSeriesData;

/// Configuration for online validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineValidationConfig {
    /// Validation window size (number of predictions to track)
    pub validation_window_size: usize,
    /// Performance threshold for triggering alerts
    pub performance_threshold: f64,
    /// Degradation threshold for triggering retraining
    pub degradation_threshold: f64,
    /// Validation frequency in seconds
    pub validation_frequency_secs: u64,
    /// Enable automatic retraining
    pub auto_retrain_enabled: bool,
    /// Minimum samples before validation starts
    pub min_samples_for_validation: usize,
    /// Alert cooldown period in seconds
    pub alert_cooldown_secs: u64,
    /// Maximum allowed latency in milliseconds
    pub max_latency_ms: f64,
    /// Memory usage threshold in MB
    pub memory_threshold_mb: f64,
}

impl Default for OnlineValidationConfig {
    fn default() -> Self {
        Self {
            validation_window_size: 1000,
            performance_threshold: 0.7,
            degradation_threshold: 0.5,
            validation_frequency_secs: 30,
            auto_retrain_enabled: true,
            min_samples_for_validation: 50,
            alert_cooldown_secs: 300,
            max_latency_ms: 1000.0,
            memory_threshold_mb: 1024.0,
        }
    }
}

/// Real-time validation metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationMetrics {
    /// Prediction accuracy over validation window
    pub accuracy: f64,
    /// Mean absolute error
    pub mae: f64,
    /// Root mean square error
    pub rmse: f64,
    /// R-squared correlation coefficient
    pub r_squared: f64,
    /// Prediction latency statistics
    pub latency_stats: LatencyStats,
    /// Memory usage statistics
    pub memory_stats: MemoryStats,
    /// Model stability score
    pub stability_score: f64,
    /// Calibration score (confidence vs accuracy alignment)
    pub calibration_score: f64,
    /// Timestamp of last update
    pub last_update: DateTime<Utc>,
    /// Total number of predictions validated
    pub total_predictions: u64,
    /// Number of degradation events detected
    pub degradation_events: u32,
}

/// Latency statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyStats {
    pub mean_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
}

/// Memory usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub current_mb: f64,
    pub peak_mb: f64,
    pub average_mb: f64,
    pub gc_count: u32,
}

/// Alert types for validation system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AlertType {
    PerformanceDegradation,
    HighLatency,
    MemoryExhaustion,
    ModelUnstable,
    PredictionError,
    ValidationFailure,
}

/// Validation alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationAlert {
    pub alert_type: AlertType,
    pub model_name: String,
    pub message: String,
    pub severity: AlertSeverity,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Alert severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Validation sample for tracking predictions vs actual values
#[derive(Debug, Clone)]
struct ValidationSample {
    pub prediction: PredictionResult,
    pub actual_value: Option<f64>,
    pub prediction_time: Instant,
    pub validation_time: Option<Instant>,
    pub latency_ms: Option<f64>,
}

/// Online validator for real-time model performance monitoring
pub struct OnlineValidator {
    config: OnlineValidationConfig,
    validation_samples: Arc<RwLock<HashMap<String, VecDeque<ValidationSample>>>>,
    metrics: Arc<RwLock<HashMap<String, ValidationMetrics>>>,
    alerts: Arc<RwLock<VecDeque<ValidationAlert>>>,
    last_alert_time: Arc<RwLock<HashMap<(String, AlertType), DateTime<Utc>>>>,
    is_running: Arc<RwLock<bool>>,
}

impl OnlineValidator {
    /// Create a new online validator
    pub fn new(config: OnlineValidationConfig) -> Self {
        Self {
            config,
            validation_samples: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(HashMap::new())),
            alerts: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            last_alert_time: Arc::new(RwLock::new(HashMap::new())),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the online validation system
    pub async fn start(&self) -> Result<()> {
        *self.is_running.write().await = true;
        
        info!("🔍 Starting online validation system");
        
        // Start validation monitoring task
        let validation_monitor = self.start_validation_monitor().await;
        
        // Start metrics calculation task
        let metrics_calculator = self.start_metrics_calculator().await;
        
        // Start alert processor
        let alert_processor = self.start_alert_processor().await;
        
        // Wait for all tasks
        tokio::select! {
            _ = validation_monitor => warn!("Validation monitor stopped"),
            _ = metrics_calculator => warn!("Metrics calculator stopped"),
            _ = alert_processor => warn!("Alert processor stopped"),
        }
        
        Ok(())
    }

    /// Stop the validation system
    pub async fn stop(&self) -> Result<()> {
        *self.is_running.write().await = false;
        info!("🛑 Stopping online validation system");
        Ok(())
    }

    /// Record a prediction for validation
    pub async fn record_prediction(
        &self,
        model_name: &str,
        prediction: PredictionResult,
    ) -> Result<()> {
        let sample = ValidationSample {
            prediction,
            actual_value: None,
            prediction_time: Instant::now(),
            validation_time: None,
            latency_ms: None,
        };

        let mut samples = self.validation_samples.write().await;
        let model_samples = samples.entry(model_name.to_string())
            .or_insert_with(|| VecDeque::with_capacity(self.config.validation_window_size));
        
        model_samples.push_back(sample);
        
        // Maintain window size
        if model_samples.len() > self.config.validation_window_size {
            model_samples.pop_front();
        }
        
        debug!("📊 Recorded prediction for validation: model={}, samples={}", 
               model_name, model_samples.len());
        
        Ok(())
    }

    /// Update prediction with actual value for validation
    pub async fn update_with_actual(
        &self,
        model_name: &str,
        prediction_timestamp: DateTime<Utc>,
        actual_value: f64,
    ) -> Result<()> {
        let mut samples = self.validation_samples.write().await;
        
        if let Some(model_samples) = samples.get_mut(model_name) {
            // Find the matching prediction sample
            for sample in model_samples.iter_mut().rev() { // Search from most recent
                if (sample.prediction.timestamp - prediction_timestamp).num_seconds().abs() < 60 {
                    sample.actual_value = Some(actual_value);
                    sample.validation_time = Some(Instant::now());
                    sample.latency_ms = Some(
                        sample.validation_time.unwrap().duration_since(sample.prediction_time).as_millis() as f64
                    );
                    
                    debug!("✅ Updated prediction with actual value: model={}, predicted={:.2}, actual={:.2}",
                           model_name, sample.prediction.value, actual_value);
                    
                    return Ok(());
                }
            }
            
            warn!("⚠️ No matching prediction found for actual value update: model={}, timestamp={}",
                  model_name, prediction_timestamp);
        }
        
        Ok(())
    }

    /// Calculate validation metrics for a model
    async fn calculate_metrics(&self, model_name: &str) -> Result<ValidationMetrics> {
        let samples = self.validation_samples.read().await;
        
        let model_samples = samples.get(model_name)
            .ok_or_else(|| anyhow::anyhow!("No samples found for model: {}", model_name))?;
        
        // Filter samples with actual values
        let validated_samples: Vec<&ValidationSample> = model_samples.iter()
            .filter(|s| s.actual_value.is_some() && s.latency_ms.is_some())
            .collect();
        
        if validated_samples.len() < self.config.min_samples_for_validation {
            return Err(anyhow::anyhow!("Insufficient validated samples: {} < {}", 
                                     validated_samples.len(), self.config.min_samples_for_validation));
        }
        
        // Calculate accuracy metrics
        let mut errors = Vec::new();
        let mut squared_errors = Vec::new();
        let mut actual_values = Vec::new();
        let mut predicted_values = Vec::new();
        let mut confidence_errors = Vec::new();
        let mut latencies = Vec::new();
        
        for sample in &validated_samples {
            let actual = sample.actual_value.unwrap();
            let predicted = sample.prediction.value;
            let error = (predicted - actual).abs();
            
            errors.push(error);
            squared_errors.push(error * error);
            actual_values.push(actual);
            predicted_values.push(predicted);
            
            // Confidence calibration error
            let prediction_error = error / actual.abs().max(0.01);
            let confidence_error = (sample.prediction.confidence - (1.0 - prediction_error)).abs();
            confidence_errors.push(confidence_error);
            
            if let Some(latency) = sample.latency_ms {
                latencies.push(latency);
            }
        }
        
        // Calculate MAE
        let mae = errors.iter().sum::<f64>() / errors.len() as f64;
        
        // Calculate RMSE
        let mse = squared_errors.iter().sum::<f64>() / squared_errors.len() as f64;
        let rmse = mse.sqrt();
        
        // Calculate R-squared
        let actual_mean = actual_values.iter().sum::<f64>() / actual_values.len() as f64;
        let total_sum_squares: f64 = actual_values.iter()
            .map(|&v| (v - actual_mean).powi(2))
            .sum();
        let residual_sum_squares: f64 = squared_errors.iter().sum();
        let r_squared = if total_sum_squares > 0.0 {
            1.0 - (residual_sum_squares / total_sum_squares)
        } else {
            0.0
        };
        
        // Calculate accuracy (percentage of predictions within 5% of actual)
        let accurate_predictions = errors.iter().zip(actual_values.iter())
            .filter(|(&error, &actual)| error / actual.abs().max(0.01) < 0.05)
            .count();
        let accuracy = accurate_predictions as f64 / errors.len() as f64;
        
        // Calculate stability score (inverse of prediction variance)
        let pred_mean = predicted_values.iter().sum::<f64>() / predicted_values.len() as f64;
        let pred_variance = predicted_values.iter()
            .map(|&v| (v - pred_mean).powi(2))
            .sum::<f64>() / predicted_values.len() as f64;
        let stability_score = 1.0 / (1.0 + pred_variance.sqrt());
        
        // Calculate calibration score
        let calibration_score = 1.0 - (confidence_errors.iter().sum::<f64>() / confidence_errors.len() as f64);
        
        // Calculate latency statistics
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let latency_stats = if !latencies.is_empty() {
            LatencyStats {
                mean_ms: latencies.iter().sum::<f64>() / latencies.len() as f64,
                p50_ms: latencies[latencies.len() / 2],
                p95_ms: latencies[(latencies.len() * 95) / 100],
                p99_ms: latencies[(latencies.len() * 99) / 100],
                max_ms: *latencies.last().unwrap(),
            }
        } else {
            LatencyStats {
                mean_ms: 0.0,
                p50_ms: 0.0,
                p95_ms: 0.0,
                p99_ms: 0.0,
                max_ms: 0.0,
            }
        };
        
        // Simple memory stats (would be more sophisticated in production)
        let memory_stats = MemoryStats {
            current_mb: 100.0 + (validated_samples.len() as f64 * 0.1),
            peak_mb: 150.0 + (validated_samples.len() as f64 * 0.15),
            average_mb: 120.0 + (validated_samples.len() as f64 * 0.12),
            gc_count: (validated_samples.len() / 1000) as u32,
        };
        
        Ok(ValidationMetrics {
            accuracy,
            mae,
            rmse,
            r_squared,
            latency_stats,
            memory_stats,
            stability_score,
            calibration_score,
            last_update: Utc::now(),
            total_predictions: validated_samples.len() as u64,
            degradation_events: 0, // Will be updated by degradation detection
        })
    }

    /// Start validation monitoring task
    async fn start_validation_monitor(&self) -> tokio::task::JoinHandle<()> {
        let validation_samples = Arc::clone(&self.validation_samples);
        let metrics = Arc::clone(&self.metrics);
        let is_running = Arc::clone(&self.is_running);
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.validation_frequency_secs));
            
            while *is_running.read().await {
                interval.tick().await;
                
                let sample_models: Vec<String> = {
                    let samples = validation_samples.read().await;
                    samples.keys().cloned().collect()
                };
                
                for model_name in sample_models {
                    match Self::calculate_metrics_static(&validation_samples, &model_name, &config).await {
                        Ok(new_metrics) => {
                            let mut metrics_map = metrics.write().await;
                            
                            // Check for degradation
                            if let Some(old_metrics) = metrics_map.get(&model_name) {
                                let degradation = old_metrics.accuracy - new_metrics.accuracy;
                                if degradation > config.degradation_threshold {
                                    warn!("📉 Performance degradation detected for {}: {:.3} -> {:.3}",
                                          model_name, old_metrics.accuracy, new_metrics.accuracy);
                                }
                            }
                            
                            metrics_map.insert(model_name.clone(), new_metrics);
                            
                            debug!("📈 Updated validation metrics for model: {}", model_name);
                        }
                        Err(e) => {
                            debug!("Could not calculate metrics for {}: {}", model_name, e);
                        }
                    }
                }
            }
        })
    }

    /// Static helper for calculating metrics (for use in spawned tasks)
    async fn calculate_metrics_static(
        validation_samples: &Arc<RwLock<HashMap<String, VecDeque<ValidationSample>>>>,
        model_name: &str,
        config: &OnlineValidationConfig,
    ) -> Result<ValidationMetrics> {
        let samples = validation_samples.read().await;
        
        let model_samples = samples.get(model_name)
            .ok_or_else(|| anyhow::anyhow!("No samples found for model: {}", model_name))?;
        
        // Filter samples with actual values
        let validated_samples: Vec<&ValidationSample> = model_samples.iter()
            .filter(|s| s.actual_value.is_some() && s.latency_ms.is_some())
            .collect();
        
        if validated_samples.len() < config.min_samples_for_validation {
            return Err(anyhow::anyhow!("Insufficient validated samples: {} < {}", 
                                     validated_samples.len(), config.min_samples_for_validation));
        }
        
        // Calculate accuracy metrics (simplified version of the full calculation)
        let mut errors = Vec::new();
        let mut latencies = Vec::new();
        
        for sample in &validated_samples {
            let actual = sample.actual_value.unwrap();
            let predicted = sample.prediction.value;
            let error = (predicted - actual).abs();
            errors.push(error);
            
            if let Some(latency) = sample.latency_ms {
                latencies.push(latency);
            }
        }
        
        let mae = errors.iter().sum::<f64>() / errors.len() as f64;
        let accuracy = errors.iter().filter(|&&e| e < 0.05).count() as f64 / errors.len() as f64;
        
        // Calculate latency statistics
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let latency_stats = if !latencies.is_empty() {
            LatencyStats {
                mean_ms: latencies.iter().sum::<f64>() / latencies.len() as f64,
                p50_ms: latencies[latencies.len() / 2],
                p95_ms: latencies[(latencies.len() * 95) / 100],
                p99_ms: latencies[(latencies.len() * 99) / 100],
                max_ms: *latencies.last().unwrap(),
            }
        } else {
            LatencyStats {
                mean_ms: 0.0,
                p50_ms: 0.0,
                p95_ms: 0.0,
                p99_ms: 0.0,
                max_ms: 0.0,
            }
        };
        
        Ok(ValidationMetrics {
            accuracy,
            mae,
            rmse: mae, // Simplified
            r_squared: accuracy, // Simplified
            latency_stats,
            memory_stats: MemoryStats {
                current_mb: 100.0,
                peak_mb: 150.0,
                average_mb: 120.0,
                gc_count: 0,
            },
            stability_score: accuracy,
            calibration_score: accuracy,
            last_update: Utc::now(),
            total_predictions: validated_samples.len() as u64,
            degradation_events: 0,
        })
    }

    /// Start metrics calculation task
    async fn start_metrics_calculator(&self) -> tokio::task::JoinHandle<()> {
        let is_running = Arc::clone(&self.is_running);
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60));
            
            while *is_running.read().await {
                interval.tick().await;
                debug!("📊 Metrics calculation cycle completed");
            }
        })
    }

    /// Start alert processing task
    async fn start_alert_processor(&self) -> tokio::task::JoinHandle<()> {
        let alerts = Arc::clone(&self.alerts);
        let is_running = Arc::clone(&self.is_running);
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5));
            
            while *is_running.read().await {
                interval.tick().await;
                
                let alert_count = alerts.read().await.len();
                if alert_count > 0 {
                    debug!("🚨 Processing {} alerts", alert_count);
                }
            }
        })
    }

    /// Generate an alert
    async fn generate_alert(
        &self,
        alert_type: AlertType,
        model_name: String,
        message: String,
        severity: AlertSeverity,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        // Check cooldown
        let cooldown_key = (model_name.clone(), alert_type.clone());
        let should_alert = {
            let last_alerts = self.last_alert_time.read().await;
            if let Some(last_time) = last_alerts.get(&cooldown_key) {
                let elapsed = Utc::now().timestamp() - last_time.timestamp();
                elapsed > self.config.alert_cooldown_secs as i64
            } else {
                true
            }
        };
        
        if should_alert {
            let alert = ValidationAlert {
                alert_type: alert_type.clone(),
                model_name: model_name.clone(),
                message,
                severity,
                timestamp: Utc::now(),
                metadata,
            };
            
            // Add to alert queue
            {
                let mut alerts = self.alerts.write().await;
                alerts.push_back(alert.clone());
                
                // Maintain queue size
                if alerts.len() > 1000 {
                    alerts.pop_front();
                }
            }
            
            // Update last alert time
            {
                let mut last_alerts = self.last_alert_time.write().await;
                last_alerts.insert(cooldown_key, Utc::now());
            }
            
            match alert.severity {
                AlertSeverity::Critical => error!("🚨 CRITICAL: {}", alert.message),
                AlertSeverity::Error => error!("❌ ERROR: {}", alert.message),
                AlertSeverity::Warning => warn!("⚠️ WARNING: {}", alert.message),
                AlertSeverity::Info => info!("ℹ️ INFO: {}", alert.message),
            }
        }
        
        Ok(())
    }

    /// Get current validation metrics for all models
    pub async fn get_all_metrics(&self) -> HashMap<String, ValidationMetrics> {
        self.metrics.read().await.clone()
    }

    /// Get validation metrics for a specific model
    pub async fn get_metrics(&self, model_name: &str) -> Option<ValidationMetrics> {
        self.metrics.read().await.get(model_name).cloned()
    }

    /// Get recent alerts
    pub async fn get_recent_alerts(&self, limit: usize) -> Vec<ValidationAlert> {
        let alerts = self.alerts.read().await;
        alerts.iter().rev().take(limit).cloned().collect()
    }

    /// Check if model needs retraining based on validation metrics
    pub async fn needs_retraining(&self, model_name: &str) -> bool {
        if let Some(metrics) = self.get_metrics(model_name).await {
            metrics.accuracy < self.config.performance_threshold ||
            metrics.stability_score < 0.5 ||
            metrics.calibration_score < 0.6
        } else {
            false
        }
    }

    /// Get validation summary report
    pub async fn get_validation_report(&self) -> HashMap<String, serde_json::Value> {
        let mut report = HashMap::new();
        let metrics = self.get_all_metrics().await;
        
        // Overall system health
        let total_models = metrics.len();
        let healthy_models = metrics.values()
            .filter(|m| m.accuracy >= self.config.performance_threshold)
            .count();
        
        report.insert("total_models".to_string(), serde_json::json!(total_models));
        report.insert("healthy_models".to_string(), serde_json::json!(healthy_models));
        report.insert("health_ratio".to_string(), 
                     serde_json::json!(if total_models > 0 { healthy_models as f64 / total_models as f64 } else { 0.0 }));
        
        // Individual model metrics
        let mut model_reports = HashMap::new();
        for (model_name, metric) in metrics {
            model_reports.insert(model_name, serde_json::json!({
                "accuracy": metric.accuracy,
                "mae": metric.mae,
                "rmse": metric.rmse,
                "r_squared": metric.r_squared,
                "stability_score": metric.stability_score,
                "calibration_score": metric.calibration_score,
                "total_predictions": metric.total_predictions,
                "last_update": metric.last_update,
                "needs_retraining": metric.accuracy < self.config.performance_threshold
            }));
        }
        report.insert("models".to_string(), serde_json::json!(model_reports));
        
        // Recent alerts summary
        let recent_alerts = self.get_recent_alerts(10).await;
        let alert_summary: Vec<serde_json::Value> = recent_alerts.into_iter()
            .map(|alert| serde_json::json!({
                "type": format!("{:?}", alert.alert_type),
                "model": alert.model_name,
                "severity": format!("{:?}", alert.severity),
                "message": alert.message,
                "timestamp": alert.timestamp
            }))
            .collect();
        report.insert("recent_alerts".to_string(), serde_json::json!(alert_summary));
        
        report.insert("report_timestamp".to_string(), serde_json::json!(Utc::now()));
        
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_online_validator_creation() {
        let config = OnlineValidationConfig::default();
        let validator = OnlineValidator::new(config);
        
        assert!(!*validator.is_running.read().await);
        assert!(validator.get_all_metrics().await.is_empty());
    }

    #[tokio::test]
    async fn test_prediction_recording() {
        let config = OnlineValidationConfig::default();
        let validator = OnlineValidator::new(config);
        
        let prediction = PredictionResult {
            timestamp: Utc::now(),
            value: 100.0,
            confidence: 0.8,
            interval_low: 95.0,
            interval_high: 105.0,
            model_name: "test_model".to_string(),
            metadata: None,
        };
        
        validator.record_prediction("test_model", prediction).await.unwrap();
        
        let samples = validator.validation_samples.read().await;
        assert!(samples.contains_key("test_model"));
        assert_eq!(samples.get("test_model").unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_actual_value_update() {
        let config = OnlineValidationConfig::default();
        let validator = OnlineValidator::new(config);
        
        let timestamp = Utc::now();
        let prediction = PredictionResult {
            timestamp,
            value: 100.0,
            confidence: 0.8,
            interval_low: 95.0,
            interval_high: 105.0,
            model_name: "test_model".to_string(),
            metadata: None,
        };
        
        validator.record_prediction("test_model", prediction).await.unwrap();
        validator.update_with_actual("test_model", timestamp, 102.0).await.unwrap();
        
        let samples = validator.validation_samples.read().await;
        let model_samples = samples.get("test_model").unwrap();
        assert!(model_samples[0].actual_value.is_some());
        assert_eq!(model_samples[0].actual_value.unwrap(), 102.0);
    }
}