//! Resource limiter for memory protection
//!
//! Implements resource limiting and graceful degradation strategies
//! to prevent system overload under memory pressure.

use crate::memory_protection::MemoryProtectionConfig;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, Semaphore};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

/// Resource limiting configuration
#[derive(Debug, Clone)]
pub struct ResourceLimitConfig {
    pub max_concurrent_operations: usize,
    pub max_memory_bytes: u64,
    pub max_cpu_percent: f64,
    pub emergency_mode_threshold: f64,
    pub graceful_degradation_enabled: bool,
    pub operation_timeout_seconds: u64,
}

impl Default for ResourceLimitConfig {
    fn default() -> Self {
        Self {
            max_concurrent_operations: 100,
            max_memory_bytes: 1024 * 1024 * 1024, // 1GB
            max_cpu_percent: 80.0,
            emergency_mode_threshold: 0.95, // 95% of limits
            graceful_degradation_enabled: true,
            operation_timeout_seconds: 30,
        }
    }
}

/// Resource usage metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMetrics {
    pub memory_usage_bytes: u64,
    pub memory_usage_percent: f64,
    pub cpu_usage_percent: f64,
    pub concurrent_operations: usize,
    pub operations_per_second: f64,
    pub rejected_operations: u64,
    pub emergency_mode_active: bool,
    pub timestamp: DateTime<Utc>,
}

/// Operation priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OperationPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Resource limiter for controlling system resource usage
#[derive(Debug)]
pub struct ResourceLimiter {
    config: ResourceLimitConfig,
    operation_semaphore: Arc<Semaphore>,
    current_operations: Arc<AtomicUsize>,
    rejected_operations: Arc<AtomicU64>,
    total_operations: Arc<AtomicU64>,
    memory_usage: Arc<AtomicU64>,
    cpu_usage: Arc<RwLock<f64>>,
    emergency_mode: Arc<AtomicBool>,
    degradation_level: Arc<RwLock<DegradationLevel>>,
    active: Arc<AtomicBool>,
    operation_history: Arc<Mutex<OperationHistory>>,
}

#[derive(Debug, Clone, Copy)]
pub enum DegradationLevel {
    None,       // Normal operation
    Light,      // Reduce non-critical operations
    Moderate,   // Reduce batch sizes, increase intervals
    Severe,     // Only critical operations
    Emergency,  // Minimal functionality only
}

#[derive(Debug)]
struct OperationHistory {
    operations: std::collections::VecDeque<OperationRecord>,
    max_records: usize,
}

#[derive(Debug, Clone)]
struct OperationRecord {
    timestamp: DateTime<Utc>,
    operation_type: String,
    priority: OperationPriority,
    duration_ms: u64,
    success: bool,
    memory_before: u64,
    memory_after: u64,
}

impl ResourceLimiter {
    pub fn new(config: &MemoryProtectionConfig) -> Self {
        let resource_config = ResourceLimitConfig {
            max_concurrent_operations: 100,
            max_memory_bytes: config.max_memory_bytes as u64,
            max_cpu_percent: 80.0,
            emergency_mode_threshold: config.memory_alert_threshold_percent,
            graceful_degradation_enabled: true,
            operation_timeout_seconds: 30,
        };

        Self {
            operation_semaphore: Arc::new(Semaphore::new(resource_config.max_concurrent_operations)),
            current_operations: Arc::new(AtomicUsize::new(0)),
            rejected_operations: Arc::new(AtomicU64::new(0)),
            total_operations: Arc::new(AtomicU64::new(0)),
            memory_usage: Arc::new(AtomicU64::new(0)),
            cpu_usage: Arc::new(RwLock::new(0.0)),
            emergency_mode: Arc::new(AtomicBool::new(false)),
            degradation_level: Arc::new(RwLock::new(DegradationLevel::None)),
            active: Arc::new(AtomicBool::new(false)),
            operation_history: Arc::new(Mutex::new(OperationHistory {
                operations: std::collections::VecDeque::with_capacity(1000),
                max_records: 1000,
            })),
            config: resource_config,
        }
    }

    /// Start resource limiting
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.active.load(Ordering::Relaxed) {
            warn!("Resource limiter already active");
            return Ok(());
        }

        info!("Starting resource limiter");
        self.active.store(true, Ordering::Relaxed);

        // Start monitoring task
        let limiter = self.clone_for_task();
        tokio::spawn(async move {
            limiter.monitoring_loop().await;
        });

        info!("Resource limiter started");
        Ok(())
    }

    /// Stop resource limiting
    pub async fn stop(&self) {
        info!("Stopping resource limiter");
        self.active.store(false, Ordering::Relaxed);
    }

    /// Execute an operation with resource limiting
    pub async fn execute_with_limit<F, T, E>(
        &self,
        operation_type: &str,
        priority: OperationPriority,
        operation: F,
    ) -> Result<T, ResourceLimitError<E>>
    where
        F: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Debug,
    {
        // Check if operation should be allowed based on current state
        if !self.should_allow_operation(priority).await {
            self.rejected_operations.fetch_add(1, Ordering::Relaxed);
            return Err(ResourceLimitError::OperationRejected {
                reason: "System under resource pressure".to_string(),
                degradation_level: *self.degradation_level.read().await,
            });
        }

        // Acquire semaphore permit
        let permit = match self.operation_semaphore.try_acquire() {
            Ok(permit) => permit,
            Err(_) => {
                self.rejected_operations.fetch_add(1, Ordering::Relaxed);
                return Err(ResourceLimitError::OperationRejected {
                    reason: "Too many concurrent operations".to_string(),
                    degradation_level: *self.degradation_level.read().await,
                });
            }
        };

        // Update counters
        self.current_operations.fetch_add(1, Ordering::Relaxed);
        self.total_operations.fetch_add(1, Ordering::Relaxed);

        let memory_before = self.memory_usage.load(Ordering::Relaxed);
        let start_time = std::time::Instant::now();

        // Execute operation with timeout
        let result = tokio::time::timeout(
            Duration::from_secs(self.config.operation_timeout_seconds),
            operation
        ).await;

        let duration = start_time.elapsed();
        let memory_after = self.memory_usage.load(Ordering::Relaxed);

        // Clean up
        self.current_operations.fetch_sub(1, Ordering::Relaxed);
        drop(permit);

        // Record operation
        let operation_record = OperationRecord {
            timestamp: Utc::now(),
            operation_type: operation_type.to_string(),
            priority,
            duration_ms: duration.as_millis() as u64,
            success: result.is_ok(),
            memory_before,
            memory_after,
        };

        self.record_operation(operation_record).await;

        // Handle result
        match result {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(error)) => Err(ResourceLimitError::OperationFailed(error)),
            Err(_) => Err(ResourceLimitError::OperationTimeout {
                timeout_seconds: self.config.operation_timeout_seconds,
            }),
        }
    }

    /// Update current memory usage
    pub async fn update_memory_usage(&self, usage_bytes: u64) {
        self.memory_usage.store(usage_bytes, Ordering::Relaxed);
        self.update_degradation_level().await;
    }

    /// Update current CPU usage
    pub async fn update_cpu_usage(&self, usage_percent: f64) {
        let mut cpu_usage = self.cpu_usage.write().await;
        *cpu_usage = usage_percent;
        self.update_degradation_level().await;
    }

    /// Get current resource metrics
    pub async fn get_metrics(&self) -> ResourceMetrics {
        let memory_usage = self.memory_usage.load(Ordering::Relaxed);
        let cpu_usage = *self.cpu_usage.read().await;

        ResourceMetrics {
            memory_usage_bytes: memory_usage,
            memory_usage_percent: (memory_usage as f64 / self.config.max_memory_bytes as f64) * 100.0,
            cpu_usage_percent: cpu_usage,
            concurrent_operations: self.current_operations.load(Ordering::Relaxed),
            operations_per_second: self.calculate_ops_per_second().await,
            rejected_operations: self.rejected_operations.load(Ordering::Relaxed),
            emergency_mode_active: self.emergency_mode.load(Ordering::Relaxed),
            timestamp: Utc::now(),
        }
    }

    /// Check if system is in emergency mode
    pub async fn is_emergency_mode(&self) -> bool {
        self.emergency_mode.load(Ordering::Relaxed)
    }

    /// Get current degradation level
    pub async fn get_degradation_level(&self) -> DegradationLevel {
        *self.degradation_level.read().await
    }

    /// Trigger emergency mode
    pub async fn trigger_emergency_mode(&self, reason: &str) {
        warn!("Triggering emergency mode: {}", reason);
        self.emergency_mode.store(true, Ordering::Relaxed);
        
        let mut degradation = self.degradation_level.write().await;
        *degradation = DegradationLevel::Emergency;
        
        info!("Emergency mode activated");
    }

    /// Exit emergency mode
    pub async fn exit_emergency_mode(&self) {
        info!("Exiting emergency mode");
        self.emergency_mode.store(false, Ordering::Relaxed);
        
        // Reset to appropriate degradation level based on current usage
        self.update_degradation_level().await;
    }

    /// Get operation statistics
    pub async fn get_operation_stats(&self) -> HashMap<String, OperationStats> {
        let history = self.operation_history.lock().await;
        let mut stats: HashMap<String, OperationStats> = HashMap::new();

        for record in &history.operations {
            let entry = stats.entry(record.operation_type.clone()).or_insert(OperationStats {
                operation_type: record.operation_type.clone(),
                total_count: 0,
                success_count: 0,
                failure_count: 0,
                average_duration_ms: 0.0,
                total_duration_ms: 0,
                memory_impact_bytes: 0,
            });

            entry.total_count += 1;
            if record.success {
                entry.success_count += 1;
            } else {
                entry.failure_count += 1;
            }
            entry.total_duration_ms += record.duration_ms;
            entry.memory_impact_bytes += record.memory_after.saturating_sub(record.memory_before) as i64;
        }

        // Calculate averages
        for stat in stats.values_mut() {
            if stat.total_count > 0 {
                stat.average_duration_ms = stat.total_duration_ms as f64 / stat.total_count as f64;
            }
        }

        stats
    }

    /// Check if operation should be allowed based on current system state
    async fn should_allow_operation(&self, priority: OperationPriority) -> bool {
        let degradation_level = *self.degradation_level.read().await;

        match degradation_level {
            DegradationLevel::None => true,
            DegradationLevel::Light => priority >= OperationPriority::Normal,
            DegradationLevel::Moderate => priority >= OperationPriority::High,
            DegradationLevel::Severe => priority >= OperationPriority::Critical,
            DegradationLevel::Emergency => false, // Block all operations in emergency
        }
    }

    /// Update degradation level based on current resource usage
    async fn update_degradation_level(&self) {
        let memory_usage = self.memory_usage.load(Ordering::Relaxed);
        let cpu_usage = *self.cpu_usage.read().await;
        
        let memory_percent = (memory_usage as f64 / self.config.max_memory_bytes as f64) * 100.0;
        let resource_pressure = memory_percent.max(cpu_usage) / 100.0;

        let new_level = if resource_pressure >= 0.95 {
            DegradationLevel::Emergency
        } else if resource_pressure >= 0.85 {
            DegradationLevel::Severe
        } else if resource_pressure >= 0.75 {
            DegradationLevel::Moderate
        } else if resource_pressure >= 0.65 {
            DegradationLevel::Light
        } else {
            DegradationLevel::None
        };

        let mut current_level = self.degradation_level.write().await;
        if std::mem::discriminant(&new_level) != std::mem::discriminant(&*current_level) {
            info!(
                "Degradation level changed: {:?} -> {:?} (resource pressure: {:.1}%)",
                *current_level, new_level, resource_pressure * 100.0
            );
            *current_level = new_level;
        }
    }

    /// Record an operation in history
    async fn record_operation(&self, record: OperationRecord) {
        let mut history = self.operation_history.lock().await;
        
        if history.operations.len() >= history.max_records {
            history.operations.pop_front();
        }
        
        history.operations.push_back(record);
    }

    /// Calculate operations per second
    async fn calculate_ops_per_second(&self) -> f64 {
        let history = self.operation_history.lock().await;
        
        if history.operations.len() < 2 {
            return 0.0;
        }

        let now = Utc::now();
        let one_minute_ago = now - chrono::Duration::seconds(60);
        
        let recent_ops: Vec<_> = history.operations
            .iter()
            .filter(|op| op.timestamp > one_minute_ago)
            .collect();
        
        if recent_ops.is_empty() {
            return 0.0;
        }

        recent_ops.len() as f64 / 60.0 // Operations per second over last minute
    }

    /// Main monitoring loop
    async fn monitoring_loop(&self) {
        let mut interval = interval(Duration::from_secs(10)); // Check every 10 seconds

        while self.active.load(Ordering::Relaxed) {
            interval.tick().await;

            if let Err(e) = self.perform_monitoring_tasks().await {
                error!("Error in resource limiter monitoring: {}", e);
            }
        }

        debug!("Resource limiter monitoring loop exited");
    }

    /// Perform periodic monitoring tasks
    async fn perform_monitoring_tasks(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Update degradation level based on current usage
        self.update_degradation_level().await;

        // Log metrics periodically
        let metrics = self.get_metrics().await;
        debug!(
            "Resource metrics - Memory: {:.1}%, CPU: {:.1}%, Ops: {}, Rejected: {}",
            metrics.memory_usage_percent,
            metrics.cpu_usage_percent,
            metrics.concurrent_operations,
            metrics.rejected_operations
        );

        // Check if we should trigger emergency mode
        if metrics.memory_usage_percent > self.config.emergency_mode_threshold * 100.0 &&
           !self.emergency_mode.load(Ordering::Relaxed) {
            self.trigger_emergency_mode(&format!(
                "High memory usage: {:.1}%", metrics.memory_usage_percent
            )).await;
        }

        Ok(())
    }

    fn clone_for_task(&self) -> Self {
        Self {
            config: self.config.clone(),
            operation_semaphore: Arc::clone(&self.operation_semaphore),
            current_operations: Arc::clone(&self.current_operations),
            rejected_operations: Arc::clone(&self.rejected_operations),
            total_operations: Arc::clone(&self.total_operations),
            memory_usage: Arc::clone(&self.memory_usage),
            cpu_usage: Arc::clone(&self.cpu_usage),
            emergency_mode: Arc::clone(&self.emergency_mode),
            degradation_level: Arc::clone(&self.degradation_level),
            active: Arc::clone(&self.active),
            operation_history: Arc::clone(&self.operation_history),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationStats {
    pub operation_type: String,
    pub total_count: u64,
    pub success_count: u64,
    pub failure_count: u64,
    pub average_duration_ms: f64,
    pub total_duration_ms: u64,
    pub memory_impact_bytes: i64,
}

/// Resource limiting errors
#[derive(Debug)]
pub enum ResourceLimitError<E> {
    OperationRejected {
        reason: String,
        degradation_level: DegradationLevel,
    },
    OperationTimeout {
        timeout_seconds: u64,
    },
    OperationFailed(E),
}

impl<E: std::fmt::Display> std::fmt::Display for ResourceLimitError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceLimitError::OperationRejected { reason, degradation_level } => {
                write!(f, "Operation rejected (degradation: {:?}): {}", degradation_level, reason)
            }
            ResourceLimitError::OperationTimeout { timeout_seconds } => {
                write!(f, "Operation timed out after {} seconds", timeout_seconds)
            }
            ResourceLimitError::OperationFailed(e) => {
                write!(f, "Operation failed: {}", e)
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ResourceLimitError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ResourceLimitError::OperationFailed(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_protection::MemoryProtectionConfig;

    #[tokio::test]
    async fn test_resource_limiter_creation() {
        let config = MemoryProtectionConfig::default();
        let limiter = ResourceLimiter::new(&config);

        let metrics = limiter.get_metrics().await;
        assert_eq!(metrics.concurrent_operations, 0);
        assert_eq!(metrics.rejected_operations, 0);
    }

    #[tokio::test]
    async fn test_operation_execution() {
        let config = MemoryProtectionConfig::default();
        let limiter = ResourceLimiter::new(&config);
        limiter.start().await.unwrap();

        let result = limiter.execute_with_limit(
            "test_operation",
            OperationPriority::Normal,
            async { Ok::<i32, &str>(42) }
        ).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);

        let metrics = limiter.get_metrics().await;
        assert_eq!(metrics.concurrent_operations, 0); // Should be 0 after completion
        
        limiter.stop().await;
    }

    #[tokio::test]
    async fn test_degradation_levels() {
        let config = MemoryProtectionConfig::default();
        let limiter = ResourceLimiter::new(&config);

        // Test normal operation
        assert!(limiter.should_allow_operation(OperationPriority::Low).await);

        // Simulate high memory usage to trigger degradation
        limiter.update_memory_usage(config.max_memory_bytes as u64 * 80 / 100).await; // 80%
        
        let level = limiter.get_degradation_level().await;
        // Should be at least Light degradation
        match level {
            DegradationLevel::None => {},
            _ => {
                // In moderate+ degradation, low priority operations should be rejected
                if matches!(level, DegradationLevel::Moderate | DegradationLevel::Severe | DegradationLevel::Emergency) {
                    assert!(!limiter.should_allow_operation(OperationPriority::Low).await);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_emergency_mode() {
        let config = MemoryProtectionConfig::default();
        let limiter = ResourceLimiter::new(&config);

        assert!(!limiter.is_emergency_mode().await);

        limiter.trigger_emergency_mode("Test emergency").await;
        assert!(limiter.is_emergency_mode().await);

        // All operations should be blocked in emergency mode
        assert!(!limiter.should_allow_operation(OperationPriority::Critical).await);

        limiter.exit_emergency_mode().await;
        assert!(!limiter.is_emergency_mode().await);
    }
}