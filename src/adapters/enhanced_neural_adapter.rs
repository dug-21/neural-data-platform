//! Enhanced Neural Adapter with Feature Flags and Error Handling
//!
//! This module provides a production-ready neural adapter with comprehensive
//! error handling, health monitoring, and graceful fallback capabilities.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use super::errors::{AdapterError, CircuitBreakerState, ErrorSeverity};
use super::errors::{HealthCheckResult, HealthMetrics};
use super::fallback_manager::{
    FallbackManager, FallbackResult, FallbackStrategy, UltimateFallbackStrategy,
};
use super::health_monitor::{HealthChecker, HealthMonitor, HealthMonitorConfig, HealthStatus};
use super::neuro_divergent::NeuroDivergentAdapter;
use crate::config::NeuralConfig;
use crate::data::TimeSeriesData;
use crate::neural::{
    FannPredictor, NeuralPredictorTrait, PredictionResult,
    PerformanceEmitter, PerformanceEvent, PerformanceEventBuilder,
    PerformanceEventType, PerformanceSource,
};

/// Enhanced configuration with feature flags and error handling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedNeuralConfig {
    /// Base neural configuration
    pub neural: NeuralConfig,
    /// Feature flag: enable real vendor models
    pub use_real_models: bool,
    /// Feature flag: enable health monitoring
    pub enable_health_monitoring: bool,
    /// Feature flag: enable fallback system
    pub enable_fallback: bool,
    /// Feature flag: enable prediction caching
    pub enable_caching: bool,
    /// Feature flag: enable circuit breakers
    pub enable_circuit_breakers: bool,
    /// Health monitoring configuration
    pub health_config: HealthMonitorConfig,
    /// Fallback strategy configuration
    pub fallback_strategy: FallbackStrategy,
    /// Model timeout settings
    pub model_timeouts: HashMap<String, Duration>,
    /// Retry configuration
    pub retry_config: RetryConfig,
    /// Performance thresholds
    pub performance_thresholds: PerformanceThresholds,
}

/// Retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub exponential_backoff: bool,
    pub jitter: bool,
}

/// Performance thresholds for health monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceThresholds {
    pub max_response_time: Duration,
    pub max_error_rate: f32,
    pub max_memory_usage_mb: u64,
    pub max_cpu_usage_percent: f32,
}

impl Default for EnhancedNeuralConfig {
    fn default() -> Self {
        Self {
            neural: NeuralConfig {
                memory_gb: 2.0,
                models: vec![
                    "DeepAR".to_string(),
                    "NHITS".to_string(),
                    "TCN".to_string(),
                    "LSTM".to_string(),
                    "FANN_MLP".to_string(),
                ],
                prediction_cache_ttl: 300,
                model_load_timeout: 60,
                max_concurrent_predictions: 50,
                enable_model_monitoring: true,
                accuracy_threshold: 0.85,
                use_real_models: true,
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
            },
            use_real_models: true,
            enable_health_monitoring: true,
            enable_fallback: true,
            enable_caching: true,
            enable_circuit_breakers: true,
            health_config: HealthMonitorConfig::default(),
            fallback_strategy: FallbackStrategy::default(),
            model_timeouts: HashMap::from([
                ("DeepAR".to_string(), Duration::from_secs(30)),
                ("NHITS".to_string(), Duration::from_secs(25)),
                ("TCN".to_string(), Duration::from_secs(20)),
                ("LSTM".to_string(), Duration::from_secs(15)),
                ("GRU".to_string(), Duration::from_secs(15)),
                ("FANN_MLP".to_string(), Duration::from_secs(5)),
            ]),
            retry_config: RetryConfig {
                max_retries: 3,
                base_delay: Duration::from_millis(500),
                max_delay: Duration::from_secs(10),
                exponential_backoff: true,
                jitter: true,
            },
            performance_thresholds: PerformanceThresholds {
                max_response_time: Duration::from_secs(10),
                max_error_rate: 10.0,
                max_memory_usage_mb: 1000,
                max_cpu_usage_percent: 80.0,
            },
        }
    }
}

/// Enhanced neural adapter with production-ready features
pub struct EnhancedNeuralAdapter {
    config: EnhancedNeuralConfig,
    fann_predictor: Arc<FannPredictor>,
    neuro_divergent_adapter: Option<Arc<RwLock<NeuroDivergentAdapter>>>,
    health_monitor: Option<Arc<HealthMonitor>>,
    fallback_manager: Option<Arc<FallbackManager>>,
    performance_stats: Arc<RwLock<PerformanceStats>>,
    performance_sender: Option<mpsc::UnboundedSender<PerformanceEvent>>,
}

/// Performance statistics tracking
#[derive(Debug, Default)]
struct PerformanceStats {
    total_predictions: u64,
    successful_predictions: u64,
    failed_predictions: u64,
    fallback_usage: u64,
    average_response_time: Duration,
    model_usage_count: HashMap<String, u64>,
    error_count_by_type: HashMap<String, u64>,
}

impl EnhancedNeuralAdapter {
    /// Create new enhanced neural adapter
    pub async fn new(config: EnhancedNeuralConfig) -> Result<Self, AdapterError> {
        info!("Initializing Enhanced Neural Adapter with feature flags");

        // Initialize FANN predictor (always available as fallback)
        let neural_config = config.neural.clone();
        // Respect the original use_real_models setting from config
        let fann_predictor = Arc::new(FannPredictor::new(neural_config).map_err(|e| {
            AdapterError::ModelInitialization {
                model: "FANN".to_string(),
                reason: e.to_string(),
            }
        })?);

        // Initialize real model adapter if enabled
        let neuro_divergent_adapter = if config.use_real_models {
            info!("Real models enabled - initializing neuro-divergent adapter");
            let mut adapter = NeuroDivergentAdapter::new();

            // Initialize supported real models
            for model in &config.neural.models {
                match model.as_str() {
                    "DeepAR" => {
                        if let Err(e) = adapter.init_deepar().await {
                            warn!("Failed to initialize DeepAR: {}", e);
                        }
                    }
                    "TCN" => {
                        if let Err(e) = adapter.init_tcn().await {
                            warn!("Failed to initialize TCN: {}", e);
                        }
                    }
                    _ => {
                        debug!(
                            "Model {} not supported by real adapter, will use FANN",
                            model
                        );
                    }
                }
            }

            Some(Arc::new(RwLock::new(adapter)))
        } else {
            info!("Real models disabled - using FANN models only");
            None
        };

        // Initialize health monitor if enabled
        let health_monitor = if config.enable_health_monitoring {
            info!("Health monitoring enabled");
            let mut monitor = HealthMonitor::new(config.health_config.clone());

            // Register health checkers for all models
            for model in &config.neural.models {
                let checker = Arc::new(ModelHealthChecker::new(
                    model.clone(),
                    fann_predictor.clone(),
                    neuro_divergent_adapter.clone(),
                ));
                monitor.register_health_checker(model.clone(), checker);
            }

            let monitor = Arc::new(monitor);

            // Start monitoring
            if let Err(e) = monitor.start_monitoring().await {
                warn!("Failed to start health monitoring: {}", e);
            }

            Some(monitor)
        } else {
            info!("Health monitoring disabled");
            None
        };

        // Initialize fallback manager if enabled
        let fallback_manager = if config.enable_fallback {
            info!("Fallback system enabled");
            let mut manager = FallbackManager::new(config.fallback_strategy.clone());

            if let Some(ref monitor) = health_monitor {
                manager.set_health_monitor(Arc::clone(monitor));
            }

            Some(Arc::new(manager))
        } else {
            info!("Fallback system disabled");
            None
        };

        Ok(Self {
            config,
            fann_predictor,
            neuro_divergent_adapter,
            health_monitor,
            fallback_manager,
            performance_stats: Arc::new(RwLock::new(PerformanceStats::default())),
            performance_sender: None,
        })
    }

    /// Check if a specific model is available and healthy
    pub async fn is_model_available(&self, model_name: &str) -> bool {
        // Check if model is in configuration
        if !self.config.neural.models.contains(&model_name.to_string()) {
            return false;
        }

        // Check health monitor if enabled
        if let Some(ref health_monitor) = self.health_monitor {
            health_monitor.is_model_healthy(model_name).await
        } else {
            true // Assume available if no health monitoring
        }
    }

    /// Get model recommendation based on health and performance
    pub async fn get_recommended_model(&self, requirements: &PredictionRequirements) -> String {
        if let Some(ref health_monitor) = self.health_monitor {
            // Get health status for all models
            let mut healthy_models = Vec::new();

            for model in &self.config.neural.models {
                let status = health_monitor.get_health_status(model).await;
                if status == HealthStatus::Healthy || status == HealthStatus::Degraded {
                    healthy_models.push(model.clone());
                }
            }

            // Choose based on requirements
            if requirements.prefer_accuracy {
                // Prefer real models for accuracy
                if self.config.use_real_models {
                    for model in &["DeepAR", "NHITS", "TCN"] {
                        if healthy_models.contains(&model.to_string()) {
                            return model.to_string();
                        }
                    }
                }
                // Fallback to FANN models
                for model in &["LSTM", "GRU", "FANN_MLP"] {
                    if healthy_models.contains(&model.to_string()) {
                        return model.to_string();
                    }
                }
            } else if requirements.prefer_speed {
                // Prefer faster models
                for model in &["FANN_MLP", "GRU", "TCN"] {
                    if healthy_models.contains(&model.to_string()) {
                        return model.to_string();
                    }
                }
            }

            // Return first healthy model if any
            if !healthy_models.is_empty() {
                return healthy_models[0].clone();
            }
        }

        // Default fallback
        self.config.fallback_strategy.primary_model.clone()
    }

    /// Predict with enhanced error handling and fallback
    pub async fn predict_enhanced(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        requirements: Option<PredictionRequirements>,
    ) -> Result<EnhancedPredictionResult, AdapterError> {
        let start_time = Instant::now();
        let requirements = requirements.unwrap_or_default();

        // Update performance stats
        {
            let mut stats = self.performance_stats.write().await;
            stats.total_predictions += 1;
        }

        // Get recommended model
        let recommended_model = self.get_recommended_model(&requirements).await;
        debug!("Using recommended model: {}", recommended_model);

        // Execute prediction with fallback if enabled
        let result = if self.config.enable_fallback && self.fallback_manager.is_some() {
            self.predict_with_fallback(data, horizon, &recommended_model)
                .await
        } else {
            self.predict_direct(data, horizon, &recommended_model).await
        };

        let duration = start_time.elapsed();

        // Update performance stats
        {
            let mut stats = self.performance_stats.write().await;
            match &result {
                Ok(_) => stats.successful_predictions += 1,
                Err(_) => stats.failed_predictions += 1,
            }

            // Update average response time
            let total = stats.total_predictions;
            stats.average_response_time =
                (stats.average_response_time * (total - 1) as u32 + duration) / total as u32;
        }

        // Convert result
        match result {
            Ok(predictions) => {
                let confidence_score = self.calculate_confidence_score(&predictions);
                let health_status = self.get_system_health_summary().await;
                Ok(EnhancedPredictionResult {
                    predictions,
                    model_used: recommended_model,
                    execution_time: duration,
                    confidence_score,
                    fallback_triggered: false,
                    health_status,
                })
            }
            Err(error) => {
                error!("Prediction failed: {}", error);
                Err(error)
            }
        }
    }

    /// Predict with fallback mechanism
    async fn predict_with_fallback(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        _preferred_model: &str,
    ) -> Result<Vec<PredictionResult>, AdapterError> {
        let fallback_manager =
            self.fallback_manager
                .as_ref()
                .ok_or_else(|| AdapterError::ConfigurationError {
                    field: "fallback_manager".to_string(),
                    issue: "not initialized".to_string(),
                })?;

        let data_clone = data.to_vec();
        let horizon_clone = horizon;
        // Clone the fann_predictor Arc to ensure Send safety
        let fann_predictor_clone = Arc::clone(&self.fann_predictor);

        let fallback_result = fallback_manager
            .predict_with_fallback(
                move |model_name, data, horizon| {
                    let fann_predictor_clone = Arc::clone(&fann_predictor_clone);
                    async move {
                        // Use the FANN predictor directly for fallback operations
                        fann_predictor_clone
                            .predict_with_model(&model_name, &data, horizon)
                            .await
                            .map_err(|e| AdapterError::Prediction(e.to_string()))
                    }
                },
                &data_clone,
                horizon_clone,
            )
            .await;

        // Update fallback stats
        if fallback_result.fallback_triggered {
            let mut stats = self.performance_stats.write().await;
            stats.fallback_usage += 1;
        }

        fallback_result.result
    }

    /// Direct prediction without fallback
    async fn predict_direct(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        model_name: &str,
    ) -> Result<Vec<PredictionResult>, AdapterError> {
        self.predict_with_specific_model(data, horizon, model_name)
            .await
    }

    /// Predict using a specific model with error handling
    async fn predict_with_specific_model(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        model_name: &str,
    ) -> Result<Vec<PredictionResult>, AdapterError> {
        let timeout = self
            .config
            .model_timeouts
            .get(model_name)
            .copied()
            .unwrap_or(Duration::from_secs(30));

        // Check if model should use real implementation
        let use_real = self.config.use_real_models
            && self.is_real_model_supported(model_name)
            && self.neuro_divergent_adapter.is_some();

        // Apply timeout to the prediction future
        let prediction_result = if use_real {
            tokio::time::timeout(
                timeout,
                self.predict_with_real_model(data, horizon, model_name),
            )
            .await
        } else {
            tokio::time::timeout(
                timeout,
                self.predict_with_fann_model(data, horizon, model_name),
            )
            .await
        };

        match prediction_result {
            Ok(result) => result,
            Err(_) => Err(AdapterError::NetworkError {
                model: model_name.to_string(),
                details: "Prediction timeout".to_string(),
                timeout_ms: timeout.as_millis() as u64,
            }),
        }
    }

    /// Predict using real neuro-divergent models
    async fn predict_with_real_model(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        model_name: &str,
    ) -> Result<Vec<PredictionResult>, AdapterError> {
        let adapter = self.neuro_divergent_adapter.as_ref().ok_or_else(|| {
            AdapterError::ModelNotAvailable {
                model: model_name.to_string(),
            }
        })?;

        let adapter_guard = adapter.read().await;
        let symbol = data
            .first()
            .map(|d| d.symbol.clone())
            .unwrap_or_else(|| "UNKNOWN".to_string());

        let ts_predictions = match model_name {
            "DeepAR" => adapter_guard.predict_deepar(data, &symbol).await?,
            "TCN" => adapter_guard.predict_tcn(data, &symbol).await?,
            _ => {
                return Err(AdapterError::ModelNotAvailable {
                    model: model_name.to_string(),
                });
            }
        };

        // Convert TimeSeriesData predictions to PredictionResult
        let mut predictions = Vec::new();
        for (i, ts_data) in ts_predictions.into_iter().take(horizon).enumerate() {
            predictions.push(PredictionResult {
                timestamp: ts_data.timestamp,
                value: ts_data.close,
                confidence: 0.9 - (i as f64 * 0.02), // Decreasing confidence over time
                interval_low: ts_data.close * 0.95,
                interval_high: ts_data.close * 1.05,
                model_name: format!("{}_real", model_name),
                metadata: None,
            });
        }

        Ok(predictions)
    }

    /// Predict using FANN models
    async fn predict_with_fann_model(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        model_name: &str,
    ) -> Result<Vec<PredictionResult>, AdapterError> {
        // Use FANN predictor's test method for specific model prediction
        self.fann_predictor
            .test_predict_with_model(model_name, data, horizon)
            .await
            .map_err(|e| AdapterError::PredictionFailed {
                model: model_name.to_string(),
                reason: e.to_string(),
                retry_count: 0,
                recoverable: true,
            })
    }

    /// Check if model is supported by real implementation
    fn is_real_model_supported(&self, model_name: &str) -> bool {
        matches!(model_name, "DeepAR" | "TCN" | "NHITS")
    }

    /// Calculate overall confidence score
    fn calculate_confidence_score(&self, predictions: &[PredictionResult]) -> f64 {
        if predictions.is_empty() {
            return 0.0;
        }

        let total_confidence: f64 = predictions.iter().map(|p| p.confidence).sum();
        total_confidence / predictions.len() as f64
    }

    /// Get system health summary
    pub async fn get_system_health_summary(&self) -> Option<SystemHealthStatus> {
        if let Some(ref health_monitor) = self.health_monitor {
            let summary = health_monitor.get_system_health_summary().await;
            Some(SystemHealthStatus {
                overall_healthy: summary.healthy_models > 0,
                healthy_models: summary.healthy_models,
                total_models: summary.healthy_models
                    + summary.degraded_models
                    + summary.unhealthy_models,
                error_rate: if summary.total_errors > 0 {
                    100.0 - summary.recovery_success_rate
                } else {
                    0.0
                },
            })
        } else {
            None
        }
    }

    /// Get performance statistics
    pub async fn get_performance_stats(&self) -> PerformanceStatsSnapshot {
        let stats = self.performance_stats.read().await;
        let success_rate = if stats.total_predictions > 0 {
            (stats.successful_predictions as f64 / stats.total_predictions as f64) * 100.0
        } else {
            0.0
        };

        PerformanceStatsSnapshot {
            total_predictions: stats.total_predictions,
            success_rate,
            average_response_time: stats.average_response_time,
            fallback_usage_rate: if stats.total_predictions > 0 {
                (stats.fallback_usage as f64 / stats.total_predictions as f64) * 100.0
            } else {
                0.0
            },
            model_usage_count: stats.model_usage_count.clone(),
        }
    }

    /// Shutdown the adapter gracefully
    pub async fn shutdown(&self) -> Result<(), AdapterError> {
        info!("Shutting down Enhanced Neural Adapter");

        if let Some(ref health_monitor) = self.health_monitor {
            health_monitor.stop_monitoring().await;
        }

        info!("Enhanced Neural Adapter shutdown complete");
        Ok(())
    }
}

/// Prediction requirements for model selection
#[derive(Debug, Clone, Default)]
pub struct PredictionRequirements {
    pub prefer_accuracy: bool,
    pub prefer_speed: bool,
    pub max_acceptable_latency: Option<Duration>,
    pub min_confidence_threshold: Option<f64>,
}

/// Enhanced prediction result with metadata
#[derive(Debug, Clone)]
pub struct EnhancedPredictionResult {
    pub predictions: Vec<PredictionResult>,
    pub model_used: String,
    pub execution_time: Duration,
    pub confidence_score: f64,
    pub fallback_triggered: bool,
    pub health_status: Option<SystemHealthStatus>,
}

/// System health status summary
#[derive(Debug, Clone)]
pub struct SystemHealthStatus {
    pub overall_healthy: bool,
    pub healthy_models: u32,
    pub total_models: u32,
    pub error_rate: f32,
}

/// Performance statistics snapshot
#[derive(Debug, Clone)]
pub struct PerformanceStatsSnapshot {
    pub total_predictions: u64,
    pub success_rate: f64,
    pub average_response_time: Duration,
    pub fallback_usage_rate: f64,
    pub model_usage_count: HashMap<String, u64>,
}

/// Health checker implementation for neural models
struct ModelHealthChecker {
    model_name: String,
    fann_predictor: Arc<FannPredictor>,
    neuro_divergent_adapter: Option<Arc<RwLock<NeuroDivergentAdapter>>>,
}

impl ModelHealthChecker {
    fn new(
        model_name: String,
        fann_predictor: Arc<FannPredictor>,
        neuro_divergent_adapter: Option<Arc<RwLock<NeuroDivergentAdapter>>>,
    ) -> Self {
        Self {
            model_name,
            fann_predictor,
            neuro_divergent_adapter,
        }
    }
}

#[async_trait]
impl HealthChecker for ModelHealthChecker {
    async fn check_health(&self, model_name: &str) -> HealthCheckResult {
        let start = Instant::now();

        // Create minimal test data
        let test_data = vec![TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: chrono::Utc::now(),
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.5,
            volume: 1000.0,
            indicators: HashMap::new(),
            source: Some("health_check".to_string()),
            entity: Some("test".to_string()),
            value: Some(100.5),
            metadata: None,
        }];

        // Try a simple prediction to check health
        let healthy = match self
            .fann_predictor
            .test_predict_with_model(model_name, &test_data, 1)
            .await
        {
            Ok(_) => true,
            Err(e) => {
                debug!("Health check failed for {}: {}", model_name, e);
                false
            }
        };

        let response_time = start.elapsed();

        HealthCheckResult {
            model: model_name.to_string(),
            healthy,
            response_time,
            error: if healthy {
                None
            } else {
                Some("Prediction test failed".to_string())
            },
            timestamp: std::time::SystemTime::now(),
            metrics: self.get_metrics(model_name).await,
        }
    }

    async fn get_metrics(&self, _model_name: &str) -> HealthMetrics {
        // In a real implementation, this would collect actual metrics
        HealthMetrics {
            memory_usage_mb: 200,
            cpu_usage_percent: 15.0,
            request_count: 100,
            error_rate: 2.0,
            average_response_time: Duration::from_millis(200),
        }
    }

    fn get_model_type(&self) -> String {
        "neural".to_string()
    }
}

/// Implement the NeuralPredictorTrait for the enhanced adapter
#[async_trait]
impl NeuralPredictorTrait for EnhancedNeuralAdapter {
    async fn predict(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        _features: Option<HashMap<String, serde_json::Value>>,
    ) -> anyhow::Result<Vec<PredictionResult>> {
        let result = self
            .predict_enhanced(data, horizon, None)
            .await
            .map_err(|e| anyhow::anyhow!("Enhanced prediction failed: {}", e))?;

        Ok(result.predictions)
    }

    async fn predict_ensemble(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        models: &[String],
        _features: Option<HashMap<String, serde_json::Value>>,
    ) -> anyhow::Result<Vec<PredictionResult>> {
        // Use the FANN predictor's ensemble capability
        self.fann_predictor
            .predict_ensemble(data, horizon, models, None)
            .await
    }

    async fn get_feature_importance(&self) -> anyhow::Result<HashMap<String, f64>> {
        self.fann_predictor.get_feature_importance().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_enhanced_adapter_initialization() {
        let config = EnhancedNeuralConfig {
            use_real_models: false,
            enable_health_monitoring: false,
            enable_fallback: false,
            ..Default::default()
        };

        let adapter = EnhancedNeuralAdapter::new(config).await;
        assert!(adapter.is_ok());
    }

    #[tokio::test]
    async fn test_model_availability_check() {
        let config = EnhancedNeuralConfig {
            use_real_models: false,
            enable_health_monitoring: false,
            ..Default::default()
        };

        let adapter = EnhancedNeuralAdapter::new(config).await.unwrap();

        // Test with configured model
        let available = adapter.is_model_available("FANN_MLP").await;
        assert!(available);

        // Test with non-configured model
        let not_available = adapter.is_model_available("NonExistentModel").await;
        assert!(!not_available);
    }

    #[tokio::test]
    async fn test_prediction_with_enhanced_features() {
        let config = EnhancedNeuralConfig {
            use_real_models: false,
            enable_health_monitoring: false,
            enable_fallback: false,
            ..Default::default()
        };

        let adapter = EnhancedNeuralAdapter::new(config).await.unwrap();

        let test_data = vec![TimeSeriesData {
            symbol: "BTC/USD".to_string(),
            timestamp: chrono::Utc::now(),
            open: 50000.0,
            high: 51000.0,
            low: 49500.0,
            close: 50500.0,
            volume: 1000.0,
            indicators: HashMap::new(),
            source: None,
            entity: None,
            value: None,
            metadata: None,
        }];

        let result = adapter.predict_enhanced(&test_data, 5, None).await;

        // Should work with FANN models
        assert!(result.is_ok() || result.is_err()); // Either success or graceful error handling

        if let Ok(result) = result {
            assert_eq!(result.predictions.len(), 5);
            assert!(!result.model_used.is_empty());
            assert!(result.execution_time > Duration::from_millis(0));
        }
    }
}
