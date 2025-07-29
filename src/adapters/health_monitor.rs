//! Health monitoring system for neural model adapters
//!
//! Provides comprehensive health checks, performance monitoring, and automatic
//! recovery for production neural trading systems.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{Mutex, RwLock};
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};

use super::errors::{
    AdapterError, CircuitBreaker, CircuitBreakerConfig, CircuitBreakerState, ConsoleErrorReporter,
    ErrorMetrics, ErrorReporter, HealthCheckResult, HealthMetrics,
};
use crate::data::TimeSeriesData;

/// Health monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMonitorConfig {
    /// Health check interval
    pub check_interval: Duration,
    /// Health check timeout
    pub check_timeout: Duration,
    /// Number of health check results to keep in history
    pub history_size: usize,
    /// Unhealthy threshold (consecutive failures)
    pub unhealthy_threshold: u32,
    /// Recovery threshold (consecutive successes)
    pub recovery_threshold: u32,
    /// Performance metrics collection interval
    pub metrics_interval: Duration,
    /// Memory usage threshold for warnings (MB)
    pub memory_warning_threshold: u64,
    /// CPU usage threshold for warnings (%)
    pub cpu_warning_threshold: f32,
    /// Error rate threshold for warnings (%)
    pub error_rate_warning_threshold: f32,
    /// Response time threshold for warnings (ms)
    pub response_time_warning_threshold: u64,
}

impl Default for HealthMonitorConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(30),
            check_timeout: Duration::from_secs(10),
            history_size: 100,
            unhealthy_threshold: 3,
            recovery_threshold: 2,
            metrics_interval: Duration::from_secs(60),
            memory_warning_threshold: 500,         // 500 MB
            cpu_warning_threshold: 80.0,           // 80%
            error_rate_warning_threshold: 10.0,    // 10%
            response_time_warning_threshold: 5000, // 5 seconds
        }
    }
}

/// Model health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Health check trait for different model types
#[async_trait]
pub trait HealthChecker: Send + Sync {
    async fn check_health(&self, model_name: &str) -> HealthCheckResult;
    async fn get_metrics(&self, model_name: &str) -> HealthMetrics;
    fn get_model_type(&self) -> String;
}

/// Model performance tracker
#[derive(Debug)]
struct ModelPerformanceTracker {
    response_times: VecDeque<Duration>,
    error_count: u64,
    success_count: u64,
    last_error_time: Option<SystemTime>,
    last_success_time: Option<SystemTime>,
    memory_usage_history: VecDeque<u64>,
    cpu_usage_history: VecDeque<f32>,
}

impl ModelPerformanceTracker {
    fn new(history_size: usize) -> Self {
        Self {
            response_times: VecDeque::with_capacity(history_size),
            error_count: 0,
            success_count: 0,
            last_error_time: None,
            last_success_time: None,
            memory_usage_history: VecDeque::with_capacity(history_size),
            cpu_usage_history: VecDeque::with_capacity(history_size),
        }
    }

    fn record_response_time(&mut self, duration: Duration) {
        if self.response_times.len() >= self.response_times.capacity() {
            self.response_times.pop_front();
        }
        self.response_times.push_back(duration);
    }

    fn record_success(&mut self) {
        self.success_count += 1;
        self.last_success_time = Some(SystemTime::now());
    }

    fn record_error(&mut self) {
        self.error_count += 1;
        self.last_error_time = Some(SystemTime::now());
    }

    fn record_memory_usage(&mut self, memory_mb: u64) {
        if self.memory_usage_history.len() >= self.memory_usage_history.capacity() {
            self.memory_usage_history.pop_front();
        }
        self.memory_usage_history.push_back(memory_mb);
    }

    fn record_cpu_usage(&mut self, cpu_percent: f32) {
        if self.cpu_usage_history.len() >= self.cpu_usage_history.capacity() {
            self.cpu_usage_history.pop_front();
        }
        self.cpu_usage_history.push_back(cpu_percent);
    }

    fn get_average_response_time(&self) -> Duration {
        if self.response_times.is_empty() {
            Duration::from_millis(0)
        } else {
            let total: Duration = self.response_times.iter().sum();
            total / self.response_times.len() as u32
        }
    }

    fn get_error_rate(&self) -> f32 {
        let total_requests = self.error_count + self.success_count;
        if total_requests == 0 {
            0.0
        } else {
            (self.error_count as f32 / total_requests as f32) * 100.0
        }
    }

    fn get_current_memory_usage(&self) -> u64 {
        self.memory_usage_history.back().copied().unwrap_or(0)
    }

    fn get_current_cpu_usage(&self) -> f32 {
        self.cpu_usage_history.back().copied().unwrap_or(0.0)
    }
}

/// Main health monitor for neural models
pub struct HealthMonitor {
    config: HealthMonitorConfig,
    health_checkers: HashMap<String, Arc<dyn HealthChecker>>,
    circuit_breakers: Arc<RwLock<HashMap<String, CircuitBreaker>>>,
    performance_trackers: Arc<RwLock<HashMap<String, ModelPerformanceTracker>>>,
    health_history: Arc<RwLock<HashMap<String, VecDeque<HealthCheckResult>>>>,
    error_metrics: Arc<RwLock<ErrorMetrics>>,
    error_reporter: Arc<dyn ErrorReporter>,
    monitoring_tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

impl HealthMonitor {
    pub fn new(config: HealthMonitorConfig) -> Self {
        Self {
            config,
            health_checkers: HashMap::new(),
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            performance_trackers: Arc::new(RwLock::new(HashMap::new())),
            health_history: Arc::new(RwLock::new(HashMap::new())),
            error_metrics: Arc::new(RwLock::new(ErrorMetrics::default())),
            error_reporter: Arc::new(ConsoleErrorReporter),
            monitoring_tasks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Register a health checker for a specific model
    pub fn register_health_checker(&mut self, model_name: String, checker: Arc<dyn HealthChecker>) {
        self.health_checkers.insert(model_name, checker);
    }

    /// Set custom error reporter
    pub fn set_error_reporter(&mut self, reporter: Arc<dyn ErrorReporter>) {
        self.error_reporter = reporter;
    }

    /// Start health monitoring for all registered models
    pub async fn start_monitoring(&self) -> Result<(), AdapterError> {
        let mut tasks = self.monitoring_tasks.lock().await;

        for model_name in self.health_checkers.keys() {
            // Initialize circuit breaker
            {
                let mut circuit_breakers = self.circuit_breakers.write().await;
                circuit_breakers.insert(
                    model_name.clone(),
                    CircuitBreaker::new(CircuitBreakerConfig::default()),
                );
            }

            // Initialize performance tracker
            {
                let mut trackers = self.performance_trackers.write().await;
                trackers.insert(
                    model_name.clone(),
                    ModelPerformanceTracker::new(self.config.history_size),
                );
            }

            // Initialize health history
            {
                let mut history = self.health_history.write().await;
                history.insert(
                    model_name.clone(),
                    VecDeque::with_capacity(self.config.history_size),
                );
            }

            // Start monitoring task for this model
            let task = self.start_model_monitoring(model_name.clone()).await;
            tasks.push(task);
        }

        // Start metrics collection task
        let metrics_task = self.start_metrics_collection().await;
        tasks.push(metrics_task);

        info!(
            "Health monitoring started for {} models",
            self.health_checkers.len()
        );
        Ok(())
    }

    /// Stop all monitoring tasks
    pub async fn stop_monitoring(&self) {
        let mut tasks = self.monitoring_tasks.lock().await;
        for task in tasks.drain(..) {
            task.abort();
        }
        info!("Health monitoring stopped");
    }

    /// Start monitoring for a specific model
    async fn start_model_monitoring(&self, model_name: String) -> tokio::task::JoinHandle<()> {
        let config = self.config.clone();
        let checker = self.health_checkers.get(&model_name).unwrap().clone();
        let circuit_breakers = Arc::clone(&self.circuit_breakers);
        let performance_trackers = Arc::clone(&self.performance_trackers);
        let health_history = Arc::clone(&self.health_history);
        let error_reporter = Arc::clone(&self.error_reporter);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.check_interval);
            let mut consecutive_failures = 0u32;
            let mut consecutive_successes = 0u32;

            loop {
                interval.tick().await;

                debug!("Performing health check for model: {}", model_name);
                let start_time = Instant::now();

                // Perform health check with timeout
                let health_result =
                    match timeout(config.check_timeout, checker.check_health(&model_name)).await {
                        Ok(result) => result,
                        Err(_) => HealthCheckResult {
                            model: model_name.clone(),
                            healthy: false,
                            response_time: config.check_timeout,
                            error: Some("Health check timeout".to_string()),
                            timestamp: SystemTime::now(),
                            metrics: HealthMetrics::default(),
                        },
                    };

                let response_time = start_time.elapsed();

                // Update performance tracker
                {
                    let mut trackers = performance_trackers.write().await;
                    if let Some(tracker) = trackers.get_mut(&model_name) {
                        tracker.record_response_time(response_time);
                        tracker.record_memory_usage(health_result.metrics.memory_usage_mb);
                        tracker.record_cpu_usage(health_result.metrics.cpu_usage_percent);

                        if health_result.healthy {
                            tracker.record_success();
                        } else {
                            tracker.record_error();
                        }
                    }
                }

                // Update circuit breaker
                {
                    let mut circuit_breakers = circuit_breakers.write().await;
                    if let Some(cb) = circuit_breakers.get_mut(&model_name) {
                        if health_result.healthy {
                            cb.record_success();
                            consecutive_successes += 1;
                            consecutive_failures = 0;
                        } else {
                            cb.record_failure();
                            consecutive_failures += 1;
                            consecutive_successes = 0;
                        }
                    }
                }

                // Update health history
                {
                    let mut history = health_history.write().await;
                    if let Some(model_history) = history.get_mut(&model_name) {
                        if model_history.len() >= config.history_size {
                            model_history.pop_front();
                        }
                        model_history.push_back(health_result.clone());
                    }
                }

                // Report health result
                error_reporter.report_health(health_result.clone()).await;

                // Check for state transitions
                if consecutive_failures >= config.unhealthy_threshold {
                    warn!(
                        "Model {} marked as unhealthy after {} consecutive failures",
                        model_name, consecutive_failures
                    );
                } else if consecutive_successes >= config.recovery_threshold {
                    info!(
                        "Model {} recovered after {} consecutive successes",
                        model_name, consecutive_successes
                    );
                }

                // Log warnings for concerning metrics
                if health_result.metrics.memory_usage_mb > config.memory_warning_threshold {
                    warn!(
                        "High memory usage for {}: {} MB",
                        model_name, health_result.metrics.memory_usage_mb
                    );
                }

                if health_result.metrics.cpu_usage_percent > config.cpu_warning_threshold {
                    warn!(
                        "High CPU usage for {}: {:.1}%",
                        model_name, health_result.metrics.cpu_usage_percent
                    );
                }

                if health_result.response_time.as_millis()
                    > config.response_time_warning_threshold as u128
                {
                    warn!(
                        "Slow response time for {}: {} ms",
                        model_name,
                        health_result.response_time.as_millis()
                    );
                }
            }
        })
    }

    /// Start metrics collection task
    async fn start_metrics_collection(&self) -> tokio::task::JoinHandle<()> {
        let config = self.config.clone();
        let performance_trackers = Arc::clone(&self.performance_trackers);
        let error_metrics = Arc::clone(&self.error_metrics);
        let error_reporter = Arc::clone(&self.error_reporter);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.metrics_interval);

            loop {
                interval.tick().await;

                // Collect and update error metrics
                let mut metrics = error_metrics.write().await;
                let trackers = performance_trackers.read().await;

                // Calculate aggregated metrics
                for (model_name, tracker) in trackers.iter() {
                    let error_rate = tracker.get_error_rate();
                    if error_rate > config.error_rate_warning_threshold {
                        warn!("High error rate for {}: {:.1}%", model_name, error_rate);
                    }

                    // Update model-specific error counts
                    metrics
                        .errors_by_model
                        .insert(model_name.clone(), tracker.error_count);
                }

                // Calculate overall statistics
                let total_errors: u64 = trackers.values().map(|t| t.error_count).sum();
                let total_successes: u64 = trackers.values().map(|t| t.success_count).sum();
                let total_requests = total_errors + total_successes;

                if total_requests > 0 {
                    metrics.recovery_success_rate =
                        (total_successes as f32 / total_requests as f32) * 100.0;
                }

                metrics.total_errors = total_errors;
                metrics.last_updated = SystemTime::now();

                // Calculate average response time across all models
                let response_times: Vec<Duration> = trackers
                    .values()
                    .map(|t| t.get_average_response_time())
                    .collect();

                if !response_times.is_empty() {
                    let total_time: Duration = response_times.iter().sum();
                    metrics.average_recovery_time = total_time / response_times.len() as u32;
                }

                let metrics_clone = metrics.clone();
                drop(metrics);
                drop(trackers);

                // Report metrics
                error_reporter.report_metrics(metrics_clone).await;
            }
        })
    }

    /// Check if a model is healthy and available
    pub async fn is_model_healthy(&self, model_name: &str) -> bool {
        // Check circuit breaker state
        {
            let circuit_breakers = self.circuit_breakers.read().await;
            if let Some(cb) = circuit_breakers.get(model_name) {
                if cb.state() == CircuitBreakerState::Open {
                    return false;
                }
            }
        }

        // Check recent health status
        let history = self.health_history.read().await;
        if let Some(model_history) = history.get(model_name) {
            if let Some(latest) = model_history.back() {
                return latest.healthy;
            }
        }

        // Default to healthy if no data (will be checked soon)
        true
    }

    /// Check if a model can accept new requests
    pub async fn can_execute(&self, model_name: &str) -> bool {
        let mut circuit_breakers = self.circuit_breakers.write().await;
        if let Some(cb) = circuit_breakers.get_mut(model_name) {
            cb.can_execute()
        } else {
            // If no circuit breaker exists, assume available
            true
        }
    }

    /// Record execution result for circuit breaker
    pub async fn record_execution_result(&self, model_name: &str, success: bool) {
        let mut circuit_breakers = self.circuit_breakers.write().await;
        if let Some(cb) = circuit_breakers.get_mut(model_name) {
            if success {
                cb.record_success();
            } else {
                cb.record_failure();
            }
        }
    }

    /// Get current health status for a model
    pub async fn get_health_status(&self, model_name: &str) -> HealthStatus {
        let history = self.health_history.read().await;
        if let Some(model_history) = history.get(model_name) {
            if model_history.is_empty() {
                return HealthStatus::Unknown;
            }

            // Count recent health checks
            let recent_checks = model_history
                .iter()
                .rev()
                .take(self.config.unhealthy_threshold as usize)
                .collect::<Vec<_>>();

            let healthy_count = recent_checks.iter().filter(|r| r.healthy).count();
            let total_count = recent_checks.len();

            if healthy_count == total_count {
                HealthStatus::Healthy
            } else if healthy_count > 0 {
                HealthStatus::Degraded
            } else {
                HealthStatus::Unhealthy
            }
        } else {
            HealthStatus::Unknown
        }
    }

    /// Get performance metrics for a model
    pub async fn get_performance_metrics(&self, model_name: &str) -> Option<HealthMetrics> {
        let trackers = self.performance_trackers.read().await;
        if let Some(tracker) = trackers.get(model_name) {
            Some(HealthMetrics {
                memory_usage_mb: tracker.get_current_memory_usage(),
                cpu_usage_percent: tracker.get_current_cpu_usage(),
                request_count: tracker.success_count + tracker.error_count,
                error_rate: tracker.get_error_rate(),
                average_response_time: tracker.get_average_response_time(),
            })
        } else {
            None
        }
    }

    /// Get overall system health summary
    pub async fn get_system_health_summary(&self) -> SystemHealthSummary {
        let mut healthy_models = 0;
        let mut degraded_models = 0;
        let mut unhealthy_models = 0;
        let mut unknown_models = 0;

        for model_name in self.health_checkers.keys() {
            match self.get_health_status(model_name).await {
                HealthStatus::Healthy => healthy_models += 1,
                HealthStatus::Degraded => degraded_models += 1,
                HealthStatus::Unhealthy => unhealthy_models += 1,
                HealthStatus::Unknown => unknown_models += 1,
            }
        }

        let error_metrics = self.error_metrics.read().await;
        let overall_status = if unhealthy_models > 0 {
            SystemStatus::Critical
        } else if degraded_models > 0 {
            SystemStatus::Degraded
        } else if healthy_models > 0 {
            SystemStatus::Healthy
        } else {
            SystemStatus::Unknown
        };

        SystemHealthSummary {
            overall_status,
            healthy_models,
            degraded_models,
            unhealthy_models,
            unknown_models,
            total_errors: error_metrics.total_errors,
            recovery_success_rate: error_metrics.recovery_success_rate,
            last_updated: SystemTime::now(),
        }
    }
}

/// System health summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthSummary {
    pub overall_status: SystemStatus,
    pub healthy_models: u32,
    pub degraded_models: u32,
    pub unhealthy_models: u32,
    pub unknown_models: u32,
    pub total_errors: u64,
    pub recovery_success_rate: f32,
    pub last_updated: SystemTime,
}

/// Overall system status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemStatus {
    Healthy,
    Degraded,
    Critical,
    Unknown,
}

/// Basic health checker implementation for testing
pub struct BasicHealthChecker;

#[async_trait]
impl HealthChecker for BasicHealthChecker {
    async fn check_health(&self, model_name: &str) -> HealthCheckResult {
        let start = Instant::now();

        // Simulate health check work
        sleep(Duration::from_millis(10)).await;

        let response_time = start.elapsed();

        // For testing, consider model healthy if name doesn't contain "failed"
        let healthy = !model_name.contains("failed");

        HealthCheckResult {
            model: model_name.to_string(),
            healthy,
            response_time,
            error: if healthy {
                None
            } else {
                Some("Simulated failure".to_string())
            },
            timestamp: SystemTime::now(),
            metrics: self.get_metrics(model_name).await,
        }
    }

    async fn get_metrics(&self, _model_name: &str) -> HealthMetrics {
        // Simulate metrics collection
        HealthMetrics {
            memory_usage_mb: 100,
            cpu_usage_percent: 25.0,
            request_count: 1000,
            error_rate: 1.5,
            average_response_time: Duration::from_millis(150),
        }
    }

    fn get_model_type(&self) -> String {
        "basic".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_health_monitor_basic() {
        let config = HealthMonitorConfig {
            check_interval: Duration::from_millis(100),
            ..Default::default()
        };

        let mut monitor = HealthMonitor::new(config);
        monitor.register_health_checker("test_model".to_string(), Arc::new(BasicHealthChecker));

        // Start monitoring briefly
        monitor.start_monitoring().await.unwrap();

        // Wait for a few health checks
        tokio::time::sleep(Duration::from_millis(250)).await;

        // Check health status
        let status = monitor.get_health_status("test_model").await;
        assert_eq!(status, HealthStatus::Healthy);

        let is_healthy = monitor.is_model_healthy("test_model").await;
        assert!(is_healthy);

        let can_execute = monitor.can_execute("test_model").await;
        assert!(can_execute);

        // Get performance metrics
        let metrics = monitor.get_performance_metrics("test_model").await;
        assert!(metrics.is_some());

        // Get system summary
        let summary = monitor.get_system_health_summary().await;
        assert_eq!(summary.healthy_models, 1);
        assert_eq!(summary.overall_status, SystemStatus::Healthy);

        monitor.stop_monitoring().await;
    }

    #[tokio::test]
    async fn test_circuit_breaker_integration() {
        let config = HealthMonitorConfig {
            check_interval: Duration::from_millis(50),
            unhealthy_threshold: 2,
            ..Default::default()
        };

        let mut monitor = HealthMonitor::new(config);
        monitor.register_health_checker(
            "failed_model".to_string(), // Will be considered unhealthy
            Arc::new(BasicHealthChecker),
        );

        monitor.start_monitoring().await.unwrap();

        // Wait for health checks to detect failure
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Check that model is marked as unhealthy
        let status = monitor.get_health_status("failed_model").await;
        assert_eq!(status, HealthStatus::Unhealthy);

        monitor.stop_monitoring().await;
    }
}
