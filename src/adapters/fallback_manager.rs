//! Fallback management system for neural model adapters
//!
//! Provides intelligent fallback strategies, automatic model switching,
//! and graceful degradation for production neural trading systems.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::errors::{AdapterError, DefaultErrorHandler, ErrorContext, ErrorHandler, FallbackConfig,
    RecoveryStrategy,
};
use super::health_monitor::{HealthMonitor, HealthStatus};
use crate::data::TimeSeriesData;
use crate::neural::PredictionResult;

/// Fallback execution result
#[derive(Debug, Clone)]
pub struct FallbackResult<T> {
    pub result: Result<T, AdapterError>,
    pub model_used: String,
    pub attempts: Vec<FallbackAttempt>,
    pub total_duration: Duration,
    pub fallback_triggered: bool,
}

/// Individual fallback attempt record
#[derive(Debug, Clone)]
pub struct FallbackAttempt {
    pub model: String,
    pub error: Option<AdapterError>,
    pub duration: Duration,
    pub recovery_strategy: RecoveryStrategy,
    pub retry_count: u32,
}

/// Fallback strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackStrategy {
    /// Primary model to try first
    pub primary_model: String,
    /// Ordered list of fallback models
    pub fallback_chain: Vec<String>,
    /// Maximum retries per model in chain
    pub max_retries_per_model: u32,
    /// Overall timeout for entire fallback chain
    pub total_timeout: Duration,
    /// Whether to cache successful fallback results
    pub cache_results: bool,
    /// Minimum confidence to accept from fallback models
    pub min_confidence_threshold: f64,
    /// Whether to enable parallel fallback for faster recovery
    pub enable_parallel_fallback: bool,
    /// Strategy for when all models fail
    pub ultimate_fallback: UltimateFallbackStrategy,
}

impl Default for FallbackStrategy {
    fn default() -> Self {
        Self {
            primary_model: "DeepAR".to_string(),
            fallback_chain: vec![
                "NHITS".to_string(),
                "TCN".to_string(),
                "LSTM".to_string(),
                "GRU".to_string(),
                "FANN_MLP".to_string(),
            ],
            max_retries_per_model: 2,
            total_timeout: Duration::from_secs(30),
            cache_results: true,
            min_confidence_threshold: 0.1,
            enable_parallel_fallback: false,
            ultimate_fallback: UltimateFallbackStrategy::UseLastKnownPrediction,
        }
    }
}

/// Strategy when all models fail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UltimateFallbackStrategy {
    /// Return error to caller
    FailFast,
    /// Use cached prediction if available
    UseLastKnownPrediction,
    /// Use simple statistical model (moving average, etc.)
    UseStatisticalFallback,
    /// Return conservative prediction with low confidence
    UseConservativePrediction,
}

/// Fallback performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackMetrics {
    pub total_fallback_attempts: u64,
    pub successful_fallbacks: u64,
    pub failed_fallbacks: u64,
    pub model_usage_stats: HashMap<String, ModelUsageStats>,
    pub average_fallback_time: Duration,
    pub ultimate_fallback_usage: u64,
    pub last_updated: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsageStats {
    pub attempts: u64,
    pub successes: u64,
    pub average_response_time: Duration,
    pub last_used: SystemTime,
    pub fallback_position: u32, // 0 for primary, 1+ for fallback chain
}

/// Cache entry for fallback results
#[derive(Debug, Clone)]
struct CacheEntry<T> {
    result: T,
    model_used: String,
    timestamp: SystemTime,
    confidence: f64,
}

/// Main fallback manager
pub struct FallbackManager {
    strategy: FallbackStrategy,
    health_monitor: Option<Arc<HealthMonitor>>,
    error_handler: Arc<dyn ErrorHandler>,
    metrics: Arc<RwLock<FallbackMetrics>>,
    prediction_cache: Arc<RwLock<HashMap<String, CacheEntry<Vec<PredictionResult>>>>>,
    cache_ttl: Duration,
}

impl FallbackManager {
    pub fn new(strategy: FallbackStrategy) -> Self {
        Self {
            strategy,
            health_monitor: None,
            error_handler: Arc::new(DefaultErrorHandler::default()),
            metrics: Arc::new(RwLock::new(FallbackMetrics {
                total_fallback_attempts: 0,
                successful_fallbacks: 0,
                failed_fallbacks: 0,
                model_usage_stats: HashMap::new(),
                average_fallback_time: Duration::from_millis(0),
                ultimate_fallback_usage: 0,
                last_updated: SystemTime::now(),
            })),
            prediction_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: Duration::from_secs(300), // 5 minutes
        }
    }

    /// Set health monitor for intelligent fallback decisions
    pub fn set_health_monitor(&mut self, monitor: Arc<HealthMonitor>) {
        self.health_monitor = Some(monitor);
    }

    /// Set custom error handler
    pub fn set_error_handler(&mut self, handler: Arc<dyn ErrorHandler>) {
        self.error_handler = handler;
    }

    /// Execute with fallback strategy
    pub async fn execute_with_fallback<F, T, Fut>(&self, operation: F) -> FallbackResult<T>
    where
        F: Fn(String) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<T, AdapterError>> + Send,
        T: Clone + Send + Sync,
    {
        let start_time = Instant::now();
        let mut attempts = Vec::new();
        let mut fallback_triggered = false;

        // Build execution order based on health status
        let execution_order = self.build_execution_order().await;
        debug!("Fallback execution order: {:?}", execution_order);

        // Try each model in order
        for (position, model_name) in execution_order.iter().enumerate() {
            if position > 0 {
                fallback_triggered = true;
            }

            let model_result = self
                .try_model_with_retries(&operation, model_name.clone(), position as u32)
                .await;

            attempts.extend(model_result.attempts);

            match model_result.result {
                Ok(result) => {
                    // Success! Update metrics and return
                    self.record_success(model_name, &attempts).await;

                    return FallbackResult {
                        result: Ok(result),
                        model_used: model_name.clone(),
                        attempts,
                        total_duration: start_time.elapsed(),
                        fallback_triggered,
                    };
                }
                Err(error) => {
                    warn!("Model {} failed: {}", model_name, error);

                    // Check if we should continue trying other models
                    if !self
                        .should_continue_fallback(&error, position, &execution_order)
                        .await
                    {
                        self.record_failure(&attempts).await;
                        return FallbackResult {
                            result: Err(error),
                            model_used: model_name.clone(),
                            attempts,
                            total_duration: start_time.elapsed(),
                            fallback_triggered,
                        };
                    }
                }
            }

            // Check overall timeout
            if start_time.elapsed() >= self.strategy.total_timeout {
                warn!("Fallback chain timeout reached");
                break;
            }
        }

        // All models failed - try ultimate fallback
        match self.try_ultimate_fallback(&attempts).await {
            Ok(result) => {
                self.record_ultimate_fallback_success().await;
                FallbackResult {
                    result: Ok(result),
                    model_used: "ultimate_fallback".to_string(),
                    attempts,
                    total_duration: start_time.elapsed(),
                    fallback_triggered: true,
                }
            }
            Err(error) => {
                self.record_failure(&attempts).await;
                FallbackResult {
                    result: Err(AdapterError::FallbackExhausted {
                        models: execution_order,
                    }),
                    model_used: "none".to_string(),
                    attempts,
                    total_duration: start_time.elapsed(),
                    fallback_triggered: true,
                }
            }
        }
    }

    /// Build optimal execution order based on health and performance
    async fn build_execution_order(&self) -> Vec<String> {
        let mut order = vec![self.strategy.primary_model.clone()];

        if let Some(health_monitor) = &self.health_monitor {
            // Filter out unhealthy models and reorder based on health status
            let mut healthy_fallbacks = Vec::new();
            let mut degraded_fallbacks = Vec::new();

            for model in &self.strategy.fallback_chain {
                match health_monitor.get_health_status(model).await {
                    HealthStatus::Healthy => healthy_fallbacks.push(model.clone()),
                    HealthStatus::Degraded => degraded_fallbacks.push(model.clone()),
                    HealthStatus::Unhealthy | HealthStatus::Unknown => {
                        debug!("Skipping unhealthy model: {}", model);
                    }
                }
            }

            // Prioritize healthy models first, then degraded ones
            order.extend(healthy_fallbacks);
            order.extend(degraded_fallbacks);
        } else {
            // No health monitor - use configured order
            order.extend(self.strategy.fallback_chain.clone());
        }

        // Remove duplicates while preserving order
        let mut seen = std::collections::HashSet::new();
        order.retain(|model| seen.insert(model.clone()));

        order
    }

    /// Try a specific model with retry logic
    async fn try_model_with_retries<F, T, Fut>(
        &self,
        operation: &F,
        model_name: String,
        position: u32,
    ) -> ModelAttemptResult<T>
    where
        F: Fn(String) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<T, AdapterError>> + Send,
        T: Clone + Send + Sync,
    {
        let mut attempts = Vec::new();
        let mut retry_count = 0;

        loop {
            let attempt_start = Instant::now();

            // Check if model is available before attempting
            if let Some(health_monitor) = &self.health_monitor {
                if !health_monitor.can_execute(&model_name).await {
                    let error = AdapterError::CircuitBreakerOpen {
                        model: model_name.clone(),
                    };

                    attempts.push(FallbackAttempt {
                        model: model_name.clone(),
                        error: Some(error.clone()),
                        duration: attempt_start.elapsed(),
                        recovery_strategy: RecoveryStrategy::FallbackToNext,
                        retry_count,
                    });

                    return ModelAttemptResult {
                        result: Err(error),
                        attempts,
                    };
                }
            }

            // Execute the operation
            match operation(model_name.clone()).await {
                Ok(result) => {
                    // Success!
                    attempts.push(FallbackAttempt {
                        model: model_name.clone(),
                        error: None,
                        duration: attempt_start.elapsed(),
                        recovery_strategy: RecoveryStrategy::ImmediateRetry,
                        retry_count,
                    });

                    // Record success in health monitor
                    if let Some(health_monitor) = &self.health_monitor {
                        health_monitor
                            .record_execution_result(&model_name, true)
                            .await;
                    }

                    return ModelAttemptResult {
                        result: Ok(result),
                        attempts,
                    };
                }
                Err(error) => {
                    // Record failure in health monitor
                    if let Some(health_monitor) = &self.health_monitor {
                        health_monitor
                            .record_execution_result(&model_name, false)
                            .await;
                    }

                    let context = ErrorContext {
                        model: Some(model_name.clone()),
                        retry_count: Some(retry_count),
                        resource_info: None,
                        vendor_info: None,
                    };

                    let recovery_strategy = self.error_handler.handle_error(&error, &context);

                    attempts.push(FallbackAttempt {
                        model: model_name.clone(),
                        error: Some(error.clone()),
                        duration: attempt_start.elapsed(),
                        recovery_strategy,
                        retry_count,
                    });

                    match recovery_strategy {
                        RecoveryStrategy::ImmediateRetry | RecoveryStrategy::ExponentialBackoff => {
                            retry_count += 1;
                            if retry_count >= self.strategy.max_retries_per_model {
                                return ModelAttemptResult {
                                    result: Err(error),
                                    attempts,
                                };
                            }

                            // Apply backoff delay if needed
                            if recovery_strategy == RecoveryStrategy::ExponentialBackoff {
                                let delay = error.retry_delay() * 2_u32.pow(retry_count.min(5));
                                tokio::time::sleep(delay).await;
                            }
                        }
                        _ => {
                            // Stop retrying for this model
                            return ModelAttemptResult {
                                result: Err(error),
                                attempts,
                            };
                        }
                    }
                }
            }
        }
    }

    /// Check if fallback should continue after an error
    async fn should_continue_fallback(
        &self,
        error: &AdapterError,
        current_position: usize,
        execution_order: &[String],
    ) -> bool {
        match error {
            AdapterError::FallbackExhausted { .. } => false,
            AdapterError::ConfigurationError { .. } => false,
            _ => {
                // Continue if we have more models to try
                current_position + 1 < execution_order.len()
            }
        }
    }

    /// Try ultimate fallback strategies when all models fail
    async fn try_ultimate_fallback<T>(
        &self,
        _attempts: &[FallbackAttempt],
    ) -> Result<T, AdapterError>
    where
        T: Clone,
    {
        match self.strategy.ultimate_fallback {
            UltimateFallbackStrategy::FailFast => Err(AdapterError::FallbackExhausted {
                models: vec!["all".to_string()],
            }),
            UltimateFallbackStrategy::UseLastKnownPrediction => {
                // This would need to be implemented specifically for the prediction type
                // For now, return an error
                Err(AdapterError::Generic {
                    message: "Ultimate fallback not implemented for this type".to_string(),
                })
            }
            UltimateFallbackStrategy::UseStatisticalFallback => {
                // This would implement a simple statistical model
                Err(AdapterError::Generic {
                    message: "Statistical fallback not implemented".to_string(),
                })
            }
            UltimateFallbackStrategy::UseConservativePrediction => {
                // This would return a conservative prediction
                Err(AdapterError::Generic {
                    message: "Conservative prediction fallback not implemented".to_string(),
                })
            }
        }
    }

    /// Record successful execution metrics
    async fn record_success(&self, model_name: &str, attempts: &[FallbackAttempt]) {
        let mut metrics = self.metrics.write().await;

        if attempts.len() > 1 {
            metrics.successful_fallbacks += 1;
        }

        // Update model usage stats
        let stats = metrics
            .model_usage_stats
            .entry(model_name.to_string())
            .or_insert_with(|| ModelUsageStats {
                attempts: 0,
                successes: 0,
                average_response_time: Duration::from_millis(0),
                last_used: SystemTime::now(),
                fallback_position: 0,
            });

        stats.attempts += 1;
        stats.successes += 1;
        stats.last_used = SystemTime::now();

        // Update average response time
        let total_time = attempts.iter().map(|a| a.duration).sum::<Duration>();
        let total_attempts = stats.attempts;
        stats.average_response_time = (stats.average_response_time * (total_attempts - 1) as u32
            + total_time)
            / total_attempts as u32;

        metrics.last_updated = SystemTime::now();
    }

    /// Record failed execution metrics
    async fn record_failure(&self, attempts: &[FallbackAttempt]) {
        let mut metrics = self.metrics.write().await;

        if attempts.len() > 1 {
            metrics.failed_fallbacks += 1;
        }

        metrics.total_fallback_attempts += 1;
        metrics.last_updated = SystemTime::now();
    }

    /// Record ultimate fallback usage
    async fn record_ultimate_fallback_success(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.ultimate_fallback_usage += 1;
        metrics.successful_fallbacks += 1;
        metrics.last_updated = SystemTime::now();
    }

    /// Get current fallback metrics
    pub async fn get_metrics(&self) -> FallbackMetrics {
        self.metrics.read().await.clone()
    }

    /// Clear metrics (useful for testing)
    pub async fn reset_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        *metrics = FallbackMetrics {
            total_fallback_attempts: 0,
            successful_fallbacks: 0,
            failed_fallbacks: 0,
            model_usage_stats: HashMap::new(),
            average_fallback_time: Duration::from_millis(0),
            ultimate_fallback_usage: 0,
            last_updated: SystemTime::now(),
        };
    }

    /// Update fallback strategy at runtime
    pub fn update_strategy(&mut self, new_strategy: FallbackStrategy) {
        self.strategy = new_strategy;
        info!("Fallback strategy updated");
    }
}

/// Helper struct for model attempt results
struct ModelAttemptResult<T> {
    result: Result<T, AdapterError>,
    attempts: Vec<FallbackAttempt>,
}

/// Prediction fallback implementation for neural models
impl FallbackManager {
    /// Execute prediction with fallback for neural models
    pub async fn predict_with_fallback<F, Fut>(
        &self,
        predictor_fn: F,
        data: &[TimeSeriesData],
        horizon: usize,
    ) -> FallbackResult<Vec<PredictionResult>>
    where
        F: Fn(String, Vec<TimeSeriesData>, usize) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<Vec<PredictionResult>, AdapterError>> + Send,
    {
        let cache_key = format!(
            "{}_{}_{}",
            data.last().map(|d| d.symbol.clone()).unwrap_or_default(),
            data.len(),
            horizon
        );

        // Check cache first
        if self.strategy.cache_results {
            if let Some(cached) = self.get_cached_prediction(&cache_key).await {
                debug!("Using cached prediction for key: {}", cache_key);
                return FallbackResult {
                    result: Ok(cached.result),
                    model_used: cached.model_used,
                    attempts: vec![],
                    total_duration: Duration::from_millis(0),
                    fallback_triggered: false,
                };
            }
        }

        let data_clone = data.to_vec();
        let result = self
            .execute_with_fallback(|model_name| {
                let data = data_clone.clone();
                predictor_fn(model_name, data, horizon)
            })
            .await;

        // Cache successful results
        if let Ok(ref predictions) = result.result {
            if self.strategy.cache_results {
                self.cache_prediction(&cache_key, predictions.clone(), &result.model_used)
                    .await;
            }
        }

        result
    }

    /// Get cached prediction if available and valid
    async fn get_cached_prediction(
        &self,
        cache_key: &str,
    ) -> Option<CacheEntry<Vec<PredictionResult>>> {
        let cache = self.prediction_cache.read().await;
        if let Some(entry) = cache.get(cache_key) {
            if entry.timestamp.elapsed().unwrap_or(Duration::MAX) <= self.cache_ttl {
                return Some(entry.clone());
            }
        }
        None
    }

    /// Cache prediction result
    async fn cache_prediction(
        &self,
        cache_key: &str,
        predictions: Vec<PredictionResult>,
        model_used: &str,
    ) {
        let mut cache = self.prediction_cache.write().await;

        // Calculate average confidence
        let avg_confidence = if predictions.is_empty() {
            0.0
        } else {
            predictions.iter().map(|p| p.confidence).sum::<f64>() / predictions.len() as f64
        };

        // Only cache if confidence meets threshold
        if avg_confidence >= self.strategy.min_confidence_threshold {
            cache.insert(
                cache_key.to_string(),
                CacheEntry {
                    result: predictions,
                    model_used: model_used.to_string(),
                    timestamp: SystemTime::now(),
                    confidence: avg_confidence,
                },
            );
        }

        // Cleanup old entries (simple LRU-like behavior)
        if cache.len() > 1000 {
            let cutoff = SystemTime::now() - self.cache_ttl;
            cache.retain(|_, entry| entry.timestamp > cutoff);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    // Mock predictor function for testing
    async fn mock_predictor(
        model_name: String,
        _data: Vec<TimeSeriesData>,
        horizon: usize,
    ) -> Result<Vec<PredictionResult>, AdapterError> {
        match model_name.as_str() {
            "failing_model" => Err(AdapterError::NetworkError {
                model: model_name,
                details: "Mock network error".to_string(),
                timeout_ms: 1000,
            }),
            "slow_model" => {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok(vec![])
            }
            _ => {
                // Return mock predictions
                let mut predictions = Vec::new();
                for i in 0..horizon {
                    predictions.push(PredictionResult {
                        timestamp: chrono::Utc::now() + chrono::Duration::minutes(i as i64),
                        value: 100.0 + i as f64,
                        confidence: 0.8,
                        interval_low: 95.0 + i as f64,
                        interval_high: 105.0 + i as f64,
                        model_name: model_name.clone(),
                        metadata: None,
                    });
                }
                Ok(predictions)
            }
        }
    }

    #[tokio::test]
    async fn test_fallback_manager_success() {
        let strategy = FallbackStrategy {
            primary_model: "good_model".to_string(),
            fallback_chain: vec!["backup_model".to_string()],
            ..Default::default()
        };

        let manager = FallbackManager::new(strategy);

        let result = manager
            .execute_with_fallback(
                |model_name| async move { Ok(format!("success_{}", model_name)) },
            )
            .await;

        assert!(result.result.is_ok());
        assert_eq!(result.model_used, "good_model");
        assert!(!result.fallback_triggered);
    }

    #[tokio::test]
    async fn test_fallback_manager_fallback() {
        let strategy = FallbackStrategy {
            primary_model: "failing_model".to_string(),
            fallback_chain: vec!["good_model".to_string()],
            max_retries_per_model: 1,
            ..Default::default()
        };

        let manager = FallbackManager::new(strategy);

        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = Arc::clone(&call_count);

        let result = manager
            .execute_with_fallback(|model_name| {
                let count = Arc::clone(&call_count_clone);
                async move {
                    let current_count = count.fetch_add(1, Ordering::SeqCst);
                    if model_name == "failing_model" {
                        Err(AdapterError::NetworkError {
                            model: model_name,
                            details: "Mock failure".to_string(),
                            timeout_ms: 1000,
                        })
                    } else {
                        Ok(format!("success_{}_attempt_{}", model_name, current_count))
                    }
                }
            })
            .await;

        assert!(result.result.is_ok());
        assert_eq!(result.model_used, "good_model");
        assert!(result.fallback_triggered);
        assert!(result.attempts.len() > 1);
    }

    #[tokio::test]
    async fn test_prediction_fallback() {
        let strategy = FallbackStrategy {
            primary_model: "failing_model".to_string(),
            fallback_chain: vec!["good_model".to_string()],
            cache_results: true,
            ..Default::default()
        };

        let manager = FallbackManager::new(strategy);
        let test_data = vec![];

        let result = manager
            .predict_with_fallback(mock_predictor, &test_data, 5)
            .await;

        assert!(result.result.is_ok());
        assert_eq!(result.model_used, "good_model");
        assert!(result.fallback_triggered);

        let predictions = result.result.unwrap();
        assert_eq!(predictions.len(), 5);
    }

    #[tokio::test]
    async fn test_metrics_collection() {
        let strategy = FallbackStrategy {
            primary_model: "failing_model".to_string(),
            fallback_chain: vec!["good_model".to_string()],
            ..Default::default()
        };

        let manager = FallbackManager::new(strategy);

        // Execute a fallback scenario
        let _result = manager
            .execute_with_fallback(|model_name| async move {
                if model_name == "failing_model" {
                    Err(AdapterError::NetworkError {
                        model: model_name,
                        details: "Test error".to_string(),
                        timeout_ms: 1000,
                    })
                } else {
                    Ok("success".to_string())
                }
            })
            .await;

        let metrics = manager.get_metrics().await;
        assert!(metrics.successful_fallbacks > 0);
        assert!(metrics.model_usage_stats.contains_key("good_model"));
    }
}
