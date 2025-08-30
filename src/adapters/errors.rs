//! Comprehensive error handling for neural model adapters
//!
//! This module provides detailed error types, fallback strategies, and health monitoring
//! for production-ready neural trading systems.

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Comprehensive error types for adapter failures
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum AdapterError {
    /// Model initialization failed
    #[error("Model initialization failed: {model} - {reason}")]
    ModelInitialization { model: String, reason: String },

    /// Model not found or not loaded
    #[error("Model not available: {model}")]
    ModelNotAvailable { model: String },

    /// Training failed with details
    #[error("Training failed for {model}: {reason}")]
    TrainingFailed { model: String, reason: String },

    /// Prediction failed with context
    #[error("Prediction failed for {model}: {reason} (retry_count: {retry_count})")]
    PredictionFailed {
        model: String,
        reason: String,
        retry_count: u32,
        recoverable: bool,
    },

    /// Data conversion/serialization error
    #[error("Data serialization error: {details}")]
    DataSerialization { details: String },

    /// Network/connectivity issues
    #[error("Network error for {model}: {details} (timeout: {timeout_ms}ms)")]
    NetworkError {
        model: String,
        details: String,
        timeout_ms: u64,
    },

    /// Resource exhaustion (memory, CPU, etc.)
    #[error("Resource exhaustion: {resource} - {details}")]
    ResourceExhaustion { resource: String, details: String },

    // Legacy variants for backward compatibility
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Query error: {0}")]
    Query(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Model creation error: {0}")]
    ModelCreation(String),

    #[error("Model not initialized: {0}")]
    ModelNotInitialized(String),

    #[error("Training error: {0}")]
    Training(String),

    #[error("Prediction error: {0}")]
    Prediction(String),

    /// Configuration issues
    #[error("Configuration error: {field} - {issue}")]
    ConfigurationError { field: String, issue: String },

    /// Vendor-specific errors
    #[error("Vendor model error ({vendor}): {error_code} - {message}")]
    VendorError {
        vendor: String,
        error_code: String,
        message: String,
        is_temporary: bool,
    },

    /// Circuit breaker activated
    #[error("Circuit breaker open for {model} - too many failures")]
    CircuitBreakerOpen { model: String },

    /// Health check failed
    #[error("Health check failed for {model}: {details}")]
    HealthCheckFailed { model: String, details: String },

    /// Fallback chain exhausted
    #[error("All fallback models failed: attempted {models:?}")]
    FallbackExhausted { models: Vec<String> },

    /// Generic adapter error
    #[error("Adapter error: {message}")]
    Generic { message: String },
}

// Implement From<anyhow::Error> for AdapterError
impl From<anyhow::Error> for AdapterError {
    fn from(err: anyhow::Error) -> Self {
        AdapterError::Generic {
            message: err.to_string(),
        }
    }
}

impl AdapterError {
    /// Check if error is recoverable through retry
    pub fn is_recoverable(&self) -> bool {
        match self {
            AdapterError::PredictionFailed { recoverable, .. } => *recoverable,
            AdapterError::NetworkError { .. } => true,
            AdapterError::VendorError { is_temporary, .. } => *is_temporary,
            AdapterError::ResourceExhaustion { .. } => true,
            AdapterError::CircuitBreakerOpen { .. } => false,
            AdapterError::FallbackExhausted { .. } => false,
            AdapterError::ModelNotAvailable { .. } => false,
            AdapterError::ConfigurationError { .. } => false,
            _ => true,
        }
    }

    /// Get suggested retry delay
    pub fn retry_delay(&self) -> Duration {
        match self {
            AdapterError::NetworkError { .. } => Duration::from_secs(5),
            AdapterError::VendorError {
                is_temporary: true, ..
            } => Duration::from_secs(10),
            AdapterError::ResourceExhaustion { .. } => Duration::from_secs(30),
            AdapterError::PredictionFailed { .. } => Duration::from_secs(2),
            _ => Duration::from_secs(1),
        }
    }

    /// Get error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            AdapterError::FallbackExhausted { .. } => ErrorSeverity::Critical,
            AdapterError::CircuitBreakerOpen { .. } => ErrorSeverity::High,
            AdapterError::ModelNotAvailable { .. } => ErrorSeverity::High,
            AdapterError::ConfigurationError { .. } => ErrorSeverity::High,
            AdapterError::ResourceExhaustion { .. } => ErrorSeverity::Medium,
            AdapterError::TrainingFailed { .. } => ErrorSeverity::Medium,
            AdapterError::PredictionFailed { .. } => ErrorSeverity::Low,
            AdapterError::NetworkError { .. } => ErrorSeverity::Low,
            _ => ErrorSeverity::Low,
        }
    }

    /// Convert to structured error for monitoring
    pub fn to_monitoring_event(&self) -> ErrorMonitoringEvent {
        ErrorMonitoringEvent {
            error_type: format!("{:?}", self),
            severity: self.severity(),
            recoverable: self.is_recoverable(),
            suggested_retry_delay: self.retry_delay(),
            timestamp: SystemTime::now(),
            context: self.get_context(),
        }
    }

    /// Get additional context for error analysis
    fn get_context(&self) -> ErrorContext {
        match self {
            AdapterError::PredictionFailed {
                model, retry_count, ..
            } => ErrorContext {
                model: Some(model.clone()),
                retry_count: Some(*retry_count),
                resource_info: None,
                vendor_info: None,
            },
            AdapterError::VendorError {
                vendor, error_code, ..
            } => ErrorContext {
                model: None,
                retry_count: None,
                resource_info: None,
                vendor_info: Some(VendorErrorInfo {
                    vendor: vendor.clone(),
                    error_code: error_code.clone(),
                }),
            },
            AdapterError::ResourceExhaustion { resource, .. } => ErrorContext {
                model: None,
                retry_count: None,
                resource_info: Some(ResourceInfo {
                    resource_type: resource.clone(),
                }),
                vendor_info: None,
            },
            _ => ErrorContext::default(),
        }
    }
}

/// Error severity levels for monitoring and alerting
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Structured error event for monitoring systems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMonitoringEvent {
    pub error_type: String,
    pub severity: ErrorSeverity,
    pub recoverable: bool,
    pub suggested_retry_delay: Duration,
    pub timestamp: SystemTime,
    pub context: ErrorContext,
}

/// Additional context for error analysis
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ErrorContext {
    pub model: Option<String>,
    pub retry_count: Option<u32>,
    pub resource_info: Option<ResourceInfo>,
    pub vendor_info: Option<VendorErrorInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInfo {
    pub resource_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorErrorInfo {
    pub vendor: String,
    pub error_code: String,
}

/// Circuit breaker states for model health management
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitBreakerState {
    Closed,   // Normal operation
    Open,     // Preventing calls due to failures
    HalfOpen, // Testing if service has recovered
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub timeout: Duration,
    pub half_open_max_calls: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            timeout: Duration::from_secs(60),
            half_open_max_calls: 3,
        }
    }
}

/// Circuit breaker for managing model health
#[derive(Debug)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: CircuitBreakerState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<SystemTime>,
    half_open_calls: u32,
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: CircuitBreakerState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure_time: None,
            half_open_calls: 0,
        }
    }

    /// Check if calls are allowed through the circuit breaker
    pub fn can_execute(&mut self) -> bool {
        match self.state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                if let Some(last_failure) = self.last_failure_time {
                    if last_failure.elapsed().unwrap_or(Duration::ZERO) >= self.config.timeout {
                        info!("Circuit breaker transitioning to half-open");
                        self.state = CircuitBreakerState::HalfOpen;
                        self.half_open_calls = 0;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitBreakerState::HalfOpen => self.half_open_calls < self.config.half_open_max_calls,
        }
    }

    /// Record a successful execution
    pub fn record_success(&mut self) {
        match self.state {
            CircuitBreakerState::Closed => {
                self.failure_count = 0;
            }
            CircuitBreakerState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= self.config.success_threshold {
                    info!("Circuit breaker closing - service recovered");
                    self.state = CircuitBreakerState::Closed;
                    self.failure_count = 0;
                    self.success_count = 0;
                    self.half_open_calls = 0;
                }
            }
            CircuitBreakerState::Open => {}
        }
    }

    /// Record a failed execution
    pub fn record_failure(&mut self) {
        self.last_failure_time = Some(SystemTime::now());

        match self.state {
            CircuitBreakerState::Closed => {
                self.failure_count += 1;
                if self.failure_count >= self.config.failure_threshold {
                    warn!(
                        "Circuit breaker opening due to failures: {}",
                        self.failure_count
                    );
                    self.state = CircuitBreakerState::Open;
                }
            }
            CircuitBreakerState::HalfOpen => {
                warn!("Circuit breaker opening again - service still failing");
                self.state = CircuitBreakerState::Open;
                self.success_count = 0;
                self.half_open_calls = 0;
            }
            CircuitBreakerState::Open => {}
        }
    }

    /// Record a call attempt in half-open state
    pub fn record_call_attempt(&mut self) {
        if self.state == CircuitBreakerState::HalfOpen {
            self.half_open_calls += 1;
        }
    }

    /// Get current state
    pub fn state(&self) -> CircuitBreakerState {
        self.state
    }

    /// Get failure count
    pub fn failure_count(&self) -> u32 {
        self.failure_count
    }
}

/// Health check result for model monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub model: String,
    pub healthy: bool,
    pub response_time: Duration,
    pub error: Option<String>,
    pub timestamp: SystemTime,
    pub metrics: HealthMetrics,
}

/// Health metrics for model monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMetrics {
    pub memory_usage_mb: u64,
    pub cpu_usage_percent: f32,
    pub request_count: u64,
    pub error_rate: f32,
    pub average_response_time: Duration,
}

impl Default for HealthMetrics {
    fn default() -> Self {
        Self {
            memory_usage_mb: 0,
            cpu_usage_percent: 0.0,
            request_count: 0,
            error_rate: 0.0,
            average_response_time: Duration::from_millis(0),
        }
    }
}

/// Fallback strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackConfig {
    /// Ordered list of fallback models
    pub fallback_chain: Vec<String>,
    /// Maximum retry attempts per model
    pub max_retries_per_model: u32,
    /// Overall timeout for fallback chain
    pub total_timeout: Duration,
    /// Whether to enable caching of fallback results
    pub cache_fallback_results: bool,
    /// Minimum confidence threshold to accept predictions
    pub min_confidence_threshold: f64,
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self {
            fallback_chain: vec![
                "DeepAR".to_string(),
                "NHITS".to_string(),
                "TCN".to_string(),
                "LSTM".to_string(),
                "FANN_MLP".to_string(),
            ],
            max_retries_per_model: 3,
            total_timeout: Duration::from_secs(30),
            cache_fallback_results: true,
            min_confidence_threshold: 0.1,
        }
    }
}

/// Error recovery strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryStrategy {
    /// Immediate retry
    ImmediateRetry,
    /// Retry with exponential backoff
    ExponentialBackoff,
    /// Try next model in fallback chain
    FallbackToNext,
    /// Skip to FANN models immediately
    FallbackToFANN,
    /// Fail the entire prediction
    FailFast,
}

/// Error handler trait for extensible error handling
pub trait ErrorHandler: Send + Sync {
    fn handle_error(&self, error: &AdapterError, context: &ErrorContext) -> RecoveryStrategy;
    fn should_report(&self, error: &AdapterError) -> bool;
}

/// Default error handler implementation
pub struct DefaultErrorHandler {
    pub max_retries: u32,
    pub enable_fallback: bool,
}

impl Default for DefaultErrorHandler {
    fn default() -> Self {
        Self {
            max_retries: 3,
            enable_fallback: true,
        }
    }
}

impl ErrorHandler for DefaultErrorHandler {
    fn handle_error(&self, error: &AdapterError, context: &ErrorContext) -> RecoveryStrategy {
        match error {
            AdapterError::NetworkError { .. } => {
                if context.retry_count.unwrap_or(0) < self.max_retries {
                    RecoveryStrategy::ExponentialBackoff
                } else if self.enable_fallback {
                    RecoveryStrategy::FallbackToNext
                } else {
                    RecoveryStrategy::FailFast
                }
            }
            AdapterError::VendorError {
                is_temporary: true, ..
            } => RecoveryStrategy::ExponentialBackoff,
            AdapterError::VendorError {
                is_temporary: false,
                ..
            } => RecoveryStrategy::FallbackToNext,
            AdapterError::ResourceExhaustion { .. } => RecoveryStrategy::FallbackToFANN,
            AdapterError::ModelNotAvailable { .. } => RecoveryStrategy::FallbackToNext,
            AdapterError::CircuitBreakerOpen { .. } => RecoveryStrategy::FallbackToNext,
            AdapterError::PredictionFailed {
                recoverable: true, ..
            } => {
                if context.retry_count.unwrap_or(0) < self.max_retries {
                    RecoveryStrategy::ImmediateRetry
                } else {
                    RecoveryStrategy::FallbackToNext
                }
            }
            _ => RecoveryStrategy::FailFast,
        }
    }

    fn should_report(&self, error: &AdapterError) -> bool {
        error.severity() >= ErrorSeverity::Medium
    }
}

/// Error metrics for monitoring and alerting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMetrics {
    pub total_errors: u64,
    pub errors_by_type: std::collections::HashMap<String, u64>,
    pub errors_by_model: std::collections::HashMap<String, u64>,
    pub recovery_success_rate: f32,
    pub fallback_usage_rate: f32,
    pub average_recovery_time: Duration,
    pub last_updated: SystemTime,
}

impl Default for ErrorMetrics {
    fn default() -> Self {
        Self {
            total_errors: 0,
            errors_by_type: std::collections::HashMap::new(),
            errors_by_model: std::collections::HashMap::new(),
            recovery_success_rate: 0.0,
            fallback_usage_rate: 0.0,
            average_recovery_time: Duration::from_millis(0),
            last_updated: SystemTime::now(),
        }
    }
}

/// Error reporting interface for external monitoring systems
use std::future::Future;
use std::pin::Pin;

pub trait ErrorReporter: Send + Sync {
    fn report_error(&self, event: ErrorMonitoringEvent)
        -> Pin<Box<dyn Future<Output = ()> + Send>>;
    fn report_metrics(&self, metrics: ErrorMetrics) -> Pin<Box<dyn Future<Output = ()> + Send>>;
    fn report_health(&self, health: HealthCheckResult) -> Pin<Box<dyn Future<Output = ()> + Send>>;
}

/// Console error reporter for development
pub struct ConsoleErrorReporter;

impl ErrorReporter for ConsoleErrorReporter {
    fn report_error(
        &self,
        event: ErrorMonitoringEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async move {
            match event.severity {
                ErrorSeverity::Critical => error!("CRITICAL ERROR: {:?}", event),
                ErrorSeverity::High => error!("HIGH SEVERITY: {:?}", event),
                ErrorSeverity::Medium => warn!("MEDIUM SEVERITY: {:?}", event),
                ErrorSeverity::Low => debug!("LOW SEVERITY: {:?}", event),
            }
        })
    }

    fn report_metrics(&self, metrics: ErrorMetrics) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async move {
            info!("Error metrics: {:?}", metrics);
        })
    }

    fn report_health(&self, health: HealthCheckResult) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async move {
            if health.healthy {
                debug!(
                    "Health check passed for {}: {:?}",
                    health.model, health.metrics
                );
            } else {
                warn!(
                    "Health check failed for {}: {:?}",
                    health.model, health.error
                );
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_error_recoverable() {
        let recoverable = AdapterError::NetworkError {
            model: "test".to_string(),
            details: "timeout".to_string(),
            timeout_ms: 5000,
        };
        assert!(recoverable.is_recoverable());

        let non_recoverable = AdapterError::ConfigurationError {
            field: "api_key".to_string(),
            issue: "missing".to_string(),
        };
        assert!(!non_recoverable.is_recoverable());
    }

    #[test]
    fn test_circuit_breaker() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 1,
            timeout: Duration::from_millis(100),
            half_open_max_calls: 1,
        };

        let mut cb = CircuitBreaker::new(config);

        // Initially closed
        assert_eq!(cb.state(), CircuitBreakerState::Closed);
        assert!(cb.can_execute());

        // Record failures to open circuit
        cb.record_failure();
        assert_eq!(cb.state(), CircuitBreakerState::Closed);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitBreakerState::Open);
        assert!(!cb.can_execute());

        // Wait for timeout and check half-open
        std::thread::sleep(Duration::from_millis(150));
        assert!(cb.can_execute());
        assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);

        // Record success to close circuit
        cb.record_call_attempt();
        cb.record_success();
        assert_eq!(cb.state(), CircuitBreakerState::Closed);
    }

    #[test]
    fn test_error_severity() {
        let critical = AdapterError::FallbackExhausted {
            models: vec!["model1".to_string()],
        };
        assert_eq!(critical.severity(), ErrorSeverity::Critical);

        let low = AdapterError::NetworkError {
            model: "test".to_string(),
            details: "timeout".to_string(),
            timeout_ms: 1000,
        };
        assert_eq!(low.severity(), ErrorSeverity::Low);
    }

    #[test]
    fn test_error_handler() {
        let handler = DefaultErrorHandler::default();
        let context = ErrorContext::default();

        let network_error = AdapterError::NetworkError {
            model: "test".to_string(),
            details: "timeout".to_string(),
            timeout_ms: 1000,
        };

        let strategy = handler.handle_error(&network_error, &context);
        assert_eq!(strategy, RecoveryStrategy::ExponentialBackoff);

        let config_error = AdapterError::ConfigurationError {
            field: "api_key".to_string(),
            issue: "missing".to_string(),
        };

        let strategy = handler.handle_error(&config_error, &context);
        assert_eq!(strategy, RecoveryStrategy::FailFast);
    }
}
