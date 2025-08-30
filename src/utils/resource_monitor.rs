//! Resource monitoring and governance for training scheduler
//!
//! Provides real-time monitoring of CPU, memory, and system resources,
//! enforcing resource limits based on market hours to minimize trading impact.

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock, Semaphore};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

#[cfg(unix)]
use sysinfo::{System, Pid};

/// Resource usage snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSnapshot {
    pub timestamp: DateTime<Utc>,
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: u64,
    pub memory_percent: f64,
    pub disk_io_read_mbps: f64,
    pub disk_io_write_mbps: f64,
    pub network_rx_mbps: f64,
    pub network_tx_mbps: f64,
    pub process_count: usize,
    pub thread_count: usize,
    pub load_average: LoadAverage,
}

/// System load averages
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LoadAverage {
    pub one_minute: f64,
    pub five_minute: f64,
    pub fifteen_minute: f64,
}

/// Resource limits configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum CPU usage percentage
    pub max_cpu_percent: f64,
    /// Maximum memory usage in MB
    pub max_memory_mb: u64,
    /// Maximum memory usage percentage
    pub max_memory_percent: f64,
    /// Maximum disk I/O rate in MB/s
    pub max_disk_io_mbps: f64,
    /// Maximum network bandwidth in MB/s
    pub max_network_mbps: f64,
}

/// Resource governor configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernorConfig {
    /// Resource limits during different market windows
    pub market_hour_limits: ResourceLimits,
    pub off_hour_limits: ResourceLimits,
    /// Enforcement mode
    pub enforcement_mode: EnforcementMode,
    /// Grace period before enforcement
    pub grace_period_seconds: u64,
    /// Monitoring interval
    pub monitoring_interval_seconds: u64,
    /// History retention
    pub history_retention_minutes: u64,
}

/// Enforcement modes for resource governance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnforcementMode {
    /// Only monitor and alert
    Monitor,
    /// Throttle resource usage
    Throttle,
    /// Pause operations when limits exceeded
    Pause,
    /// Terminate operations when limits exceeded
    Terminate,
}

/// Resource violation event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceViolation {
    pub timestamp: DateTime<Utc>,
    pub resource_type: ResourceType,
    pub current_value: f64,
    pub limit_value: f64,
    pub severity: ViolationSeverity,
    pub action_taken: ViolationAction,
}

/// Types of resources being monitored
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceType {
    CPU,
    Memory,
    DiskIO,
    Network,
}

/// Severity levels for violations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViolationSeverity {
    Warning,
    Critical,
    Emergency,
}

/// Actions taken for violations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViolationAction {
    None,
    Alert,
    Throttled,
    Paused,
    Terminated,
}

/// Resource monitoring metrics
#[derive(Debug, Clone, Default, Serialize)]
pub struct ResourceMetrics {
    pub avg_cpu_usage: f64,
    pub peak_cpu_usage: f64,
    pub avg_memory_usage_mb: u64,
    pub peak_memory_usage_mb: u64,
    pub total_violations: usize,
    pub critical_violations: usize,
    pub enforcement_actions: usize,
}

/// Resource governor for enforcing limits
pub struct ResourceGovernor {
    config: Arc<GovernorConfig>,
    current_limits: Arc<RwLock<ResourceLimits>>,
    resource_monitor: Arc<ResourceMonitor>,
    violation_history: Arc<RwLock<VecDeque<ResourceViolation>>>,
    enforcement_semaphore: Arc<Semaphore>,
    shutdown_tx: mpsc::Sender<()>,
    shutdown_rx: Arc<RwLock<mpsc::Receiver<()>>>,
}

/// Resource monitoring system
pub struct ResourceMonitor {
    #[cfg(unix)]
    system: Arc<RwLock<System>>,
    snapshot_history: Arc<RwLock<VecDeque<ResourceSnapshot>>>,
    metrics: Arc<RwLock<ResourceMetrics>>,
    monitoring_interval: std::time::Duration,
}

impl Default for GovernorConfig {
    fn default() -> Self {
        Self {
            market_hour_limits: ResourceLimits {
                max_cpu_percent: 25.0,      // 25% during market hours
                max_memory_mb: 2048,        // 2GB
                max_memory_percent: 25.0,   // 25% of system memory
                max_disk_io_mbps: 50.0,     // 50 MB/s
                max_network_mbps: 10.0,     // 10 MB/s
            },
            off_hour_limits: ResourceLimits {
                max_cpu_percent: 90.0,      // 90% during off hours
                max_memory_mb: 16384,       // 16GB
                max_memory_percent: 80.0,   // 80% of system memory
                max_disk_io_mbps: 500.0,    // 500 MB/s
                max_network_mbps: 100.0,    // 100 MB/s
            },
            enforcement_mode: EnforcementMode::Throttle,
            grace_period_seconds: 30,
            monitoring_interval_seconds: 5,
            history_retention_minutes: 60,
        }
    }
}

impl ResourceMonitor {
    /// Create a new resource monitor
    pub fn new(monitoring_interval_seconds: u64) -> Result<Self> {
        Ok(Self {
            #[cfg(unix)]
            system: Arc::new(RwLock::new(System::new_all())),
            snapshot_history: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            metrics: Arc::new(RwLock::new(ResourceMetrics::default())),
            monitoring_interval: std::time::Duration::from_secs(monitoring_interval_seconds),
        })
    }

    /// Take a resource usage snapshot
    pub async fn take_snapshot(&self) -> Result<ResourceSnapshot> {
        #[cfg(unix)]
        {
            let mut system = self.system.write().await;
            system.refresh_cpu_usage();

            let cpu_usage = system.global_cpu_info().cpu_usage() as f64;
            let total_memory = system.total_memory();
            let used_memory = system.used_memory();
            let memory_percent = (used_memory as f64 / total_memory as f64) * 100.0;

            let load_avg = System::load_average();
            let load_average = LoadAverage {
                one_minute: load_avg.one,
                five_minute: load_avg.five,
                fifteen_minute: load_avg.fifteen,
            };

            // Get process info
            let current_pid = Pid::from(std::process::id() as usize);
            let process = system.process(current_pid);
            let (process_count, thread_count) = if let Some(_proc) = process {
                // In sysinfo 0.30+, we can't access tasks() directly
                // Use system processes count and a reasonable thread estimate
                let total_processes = system.processes().len();
                let estimated_threads = total_processes * 2; // Conservative estimate
                (total_processes, estimated_threads)
            } else {
                // Fallback values
                let total_processes = system.processes().len().max(1);
                (total_processes, total_processes * 2)
            };

            // Disk and network I/O (simplified for now)
            let disk_io_read_mbps = 0.0;  // Would need iostat integration
            let disk_io_write_mbps = 0.0;
            let network_rx_mbps = 0.0;     // Would need network stats integration
            let network_tx_mbps = 0.0;

            Ok(ResourceSnapshot {
                timestamp: Utc::now(),
                cpu_usage_percent: cpu_usage,
                memory_usage_mb: used_memory / 1024, // Convert KB to MB
                memory_percent,
                disk_io_read_mbps,
                disk_io_write_mbps,
                network_rx_mbps,
                network_tx_mbps,
                process_count,
                thread_count,
                load_average,
            })
        }

        #[cfg(not(unix))]
        {
            // Fallback for non-Unix systems
            Ok(ResourceSnapshot {
                timestamp: Utc::now(),
                cpu_usage_percent: 0.0,
                memory_usage_mb: 0,
                memory_percent: 0.0,
                disk_io_read_mbps: 0.0,
                disk_io_write_mbps: 0.0,
                network_rx_mbps: 0.0,
                network_tx_mbps: 0.0,
                process_count: 1,
                thread_count: 1,
                load_average: LoadAverage {
                    one_minute: 0.0,
                    five_minute: 0.0,
                    fifteen_minute: 0.0,
                },
            })
        }
    }

    /// Start continuous monitoring
    pub async fn start_monitoring(&self) -> Result<()> {
        let monitor = self.clone();
        
        tokio::spawn(async move {
            let mut ticker = interval(monitor.monitoring_interval);
            
            loop {
                ticker.tick().await;
                
                match monitor.take_snapshot().await {
                    Ok(snapshot) => {
                        // Update history
                        {
                            let mut history = monitor.snapshot_history.write().await;
                            history.push_back(snapshot.clone());
                            
                            // Maintain history size
                            while history.len() > 720 { // ~1 hour at 5 second intervals
                                history.pop_front();
                            }
                        }
                        
                        // Update metrics
                        monitor.update_metrics(&snapshot).await;
                        
                        debug!(
                            "Resource snapshot: CPU: {:.1}%, Memory: {} MB ({:.1}%)",
                            snapshot.cpu_usage_percent,
                            snapshot.memory_usage_mb,
                            snapshot.memory_percent
                        );
                    }
                    Err(e) => {
                        error!("Failed to take resource snapshot: {}", e);
                    }
                }
            }
        });
        
        Ok(())
    }

    /// Update metrics based on snapshot
    async fn update_metrics(&self, snapshot: &ResourceSnapshot) {
        let mut metrics = self.metrics.write().await;
        
        // Update peak values
        if snapshot.cpu_usage_percent > metrics.peak_cpu_usage {
            metrics.peak_cpu_usage = snapshot.cpu_usage_percent;
        }
        if snapshot.memory_usage_mb > metrics.peak_memory_usage_mb {
            metrics.peak_memory_usage_mb = snapshot.memory_usage_mb;
        }
        
        // Update averages (simplified for now)
        let history = self.snapshot_history.read().await;
        if !history.is_empty() {
            let sum_cpu: f64 = history.iter().map(|s| s.cpu_usage_percent).sum();
            let sum_memory: u64 = history.iter().map(|s| s.memory_usage_mb).sum();
            
            metrics.avg_cpu_usage = sum_cpu / history.len() as f64;
            metrics.avg_memory_usage_mb = sum_memory / history.len() as u64;
        }
    }

    /// Get current resource usage
    pub async fn get_current_usage(&self) -> Result<ResourceSnapshot> {
        self.take_snapshot().await
    }

    /// Get resource history
    pub async fn get_history(&self, duration: Duration) -> Vec<ResourceSnapshot> {
        let history = self.snapshot_history.read().await;
        let cutoff = Utc::now() - duration;
        
        history
            .iter()
            .filter(|s| s.timestamp > cutoff)
            .cloned()
            .collect()
    }

    /// Get resource metrics
    pub async fn get_metrics(&self) -> ResourceMetrics {
        (*self.metrics.read().await).clone()
    }

    /// Update violation metrics
    pub async fn update_violation_metrics(&self, is_critical: bool, action_taken: bool) {
        let mut metrics = self.metrics.write().await;
        metrics.total_violations += 1;
        if is_critical {
            metrics.critical_violations += 1;
        }
        if action_taken {
            metrics.enforcement_actions += 1;
        }
    }
}

impl ResourceGovernor {
    /// Create a new resource governor
    pub async fn new(
        config: GovernorConfig,
        market_hours: Arc<crate::utils::MarketHours>,
    ) -> Result<Self> {
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
        
        let resource_monitor = Arc::new(
            ResourceMonitor::new(config.monitoring_interval_seconds)
                .context("Failed to create resource monitor")?
        );
        
        // Start resource monitoring
        resource_monitor.start_monitoring().await?;
        
        let governor = Self {
            config: Arc::new(config.clone()),
            current_limits: Arc::new(RwLock::new(config.off_hour_limits.clone())),
            resource_monitor,
            violation_history: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            enforcement_semaphore: Arc::new(Semaphore::new(100)), // Percentage units
            shutdown_tx,
            shutdown_rx: Arc::new(RwLock::new(shutdown_rx)),
        };
        
        // Start governance loop
        governor.start_governance(market_hours).await?;
        
        Ok(governor)
    }

    /// Start the governance loop
    async fn start_governance(&self, market_hours: Arc<crate::utils::MarketHours>) -> Result<()> {
        let governor = Arc::new(self.clone());
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let mut ticker = interval(std::time::Duration::from_secs(
                config.monitoring_interval_seconds
            ));
            
            loop {
                ticker.tick().await;
                
                // Update limits based on market hours
                let now = Utc::now();
                let resource_limit = market_hours.get_resource_limit(now).await;
                
                // Update current limits
                {
                    let mut current = governor.current_limits.write().await;
                    if resource_limit < 0.5 {
                        // Market hours - enforce strict limits
                        *current = config.market_hour_limits.clone();
                    } else {
                        // Off hours - relaxed limits
                        *current = config.off_hour_limits.clone();
                    }
                }
                
                // Check for violations
                if let Err(e) = governor.check_and_enforce().await {
                    error!("Resource enforcement error: {}", e);
                }
            }
        });
        
        Ok(())
    }

    /// Check current usage and enforce limits
    async fn check_and_enforce(&self) -> Result<()> {
        let snapshot = self.resource_monitor.get_current_usage().await?;
        let limits = self.current_limits.read().await.clone();
        
        let mut violations = Vec::new();
        
        // Check CPU usage
        if snapshot.cpu_usage_percent > limits.max_cpu_percent {
            violations.push(self.create_violation(
                ResourceType::CPU,
                snapshot.cpu_usage_percent,
                limits.max_cpu_percent,
            ));
        }
        
        // Check memory usage
        if snapshot.memory_usage_mb > limits.max_memory_mb ||
           snapshot.memory_percent > limits.max_memory_percent {
            violations.push(self.create_violation(
                ResourceType::Memory,
                snapshot.memory_percent,
                limits.max_memory_percent,
            ));
        }
        
        // Check disk I/O
        let total_disk_io = snapshot.disk_io_read_mbps + snapshot.disk_io_write_mbps;
        if total_disk_io > limits.max_disk_io_mbps {
            violations.push(self.create_violation(
                ResourceType::DiskIO,
                total_disk_io,
                limits.max_disk_io_mbps,
            ));
        }
        
        // Check network I/O
        let total_network = snapshot.network_rx_mbps + snapshot.network_tx_mbps;
        if total_network > limits.max_network_mbps {
            violations.push(self.create_violation(
                ResourceType::Network,
                total_network,
                limits.max_network_mbps,
            ));
        }
        
        // Process violations
        for mut violation in violations {
            violation.action_taken = self.enforce_violation(&violation).await?;
            
            // Store violation history
            {
                let mut history = self.violation_history.write().await;
                history.push_back(violation.clone());
                
                // Maintain history size
                let retention = Duration::minutes(self.config.history_retention_minutes as i64);
                let cutoff = Utc::now() - retention;
                while let Some(front) = history.front() {
                    if front.timestamp < cutoff {
                        history.pop_front();
                    } else {
                        break;
                    }
                }
            }
            
            // Update metrics
            let is_critical = violation.severity == ViolationSeverity::Critical;
            let action_taken = !matches!(violation.action_taken, ViolationAction::None | ViolationAction::Alert);
            self.resource_monitor.update_violation_metrics(is_critical, action_taken).await;
            
            warn!(
                "Resource violation: {:?} usage {:.1}% exceeds limit {:.1}% - Action: {:?}",
                violation.resource_type,
                violation.current_value,
                violation.limit_value,
                violation.action_taken
            );
        }
        
        Ok(())
    }

    /// Create a violation record
    fn create_violation(
        &self,
        resource_type: ResourceType,
        current_value: f64,
        limit_value: f64,
    ) -> ResourceViolation {
        let excess_percent = ((current_value - limit_value) / limit_value) * 100.0;
        
        let severity = if excess_percent > 50.0 {
            ViolationSeverity::Emergency
        } else if excess_percent > 20.0 {
            ViolationSeverity::Critical
        } else {
            ViolationSeverity::Warning
        };
        
        ResourceViolation {
            timestamp: Utc::now(),
            resource_type,
            current_value,
            limit_value,
            severity,
            action_taken: ViolationAction::None,
        }
    }

    /// Enforce a resource violation
    async fn enforce_violation(&self, violation: &ResourceViolation) -> Result<ViolationAction> {
        match self.config.enforcement_mode {
            EnforcementMode::Monitor => {
                // Just alert, no enforcement
                Ok(ViolationAction::Alert)
            }
            EnforcementMode::Throttle => {
                // Throttle resource usage
                match violation.resource_type {
                    ResourceType::CPU => {
                        // Reduce available CPU permits
                        let permits_to_acquire = 
                            ((violation.current_value - violation.limit_value) / 100.0 * 100.0) as u32;
                        let _ = self.enforcement_semaphore.try_acquire_many(permits_to_acquire);
                    }
                    _ => {
                        // For other resources, we'd need specific throttling mechanisms
                    }
                }
                Ok(ViolationAction::Throttled)
            }
            EnforcementMode::Pause => {
                // Would pause operations - implementation depends on integration
                warn!("Pausing operations due to resource violation");
                Ok(ViolationAction::Paused)
            }
            EnforcementMode::Terminate => {
                // Would terminate operations - implementation depends on integration
                error!("Terminating operations due to resource violation");
                Ok(ViolationAction::Terminated)
            }
        }
    }

    /// Get current resource limits
    pub async fn get_current_limits(&self) -> ResourceLimits {
        self.current_limits.read().await.clone()
    }

    /// Get violation history
    pub async fn get_violation_history(&self, duration: Duration) -> Vec<ResourceViolation> {
        let history = self.violation_history.read().await;
        let cutoff = Utc::now() - duration;
        
        history
            .iter()
            .filter(|v| v.timestamp > cutoff)
            .cloned()
            .collect()
    }

    /// Get governor status
    pub async fn get_status(&self) -> serde_json::Value {
        let current_usage = self.resource_monitor.get_current_usage().await.ok();
        let limits = self.current_limits.read().await.clone();
        let metrics = self.resource_monitor.get_metrics().await;
        let recent_violations = self.get_violation_history(Duration::minutes(5)).await;
        
        serde_json::json!({
            "current_usage": current_usage,
            "current_limits": limits,
            "enforcement_mode": self.config.enforcement_mode,
            "metrics": metrics,
            "recent_violations": recent_violations.len(),
            "health": {
                "monitoring": "active",
                "enforcement": match self.config.enforcement_mode {
                    EnforcementMode::Monitor => "monitoring_only",
                    _ => "active"
                }
            }
        })
    }

    /// Check if resource usage is within limits
    pub async fn check_resource_availability(
        &self,
        required_cpu: f64,
        required_memory_mb: u64,
    ) -> Result<bool> {
        let snapshot = self.resource_monitor.get_current_usage().await?;
        let limits = self.current_limits.read().await;
        
        let cpu_available = limits.max_cpu_percent - snapshot.cpu_usage_percent;
        let memory_available = limits.max_memory_mb.saturating_sub(snapshot.memory_usage_mb);
        
        Ok(cpu_available >= required_cpu && memory_available >= required_memory_mb)
    }

    /// Acquire resource permits (for throttling)
    pub async fn acquire_resources(&self, cpu_percent: f64) -> Result<()> {
        let permits = (cpu_percent / 100.0 * 100.0) as u32;
        let _ = self.enforcement_semaphore.acquire_many(permits).await?;
        Ok(())
    }

    /// Release resource permits
    pub fn release_resources(&self, cpu_percent: f64) {
        let permits = (cpu_percent / 100.0 * 100.0) as u32;
        self.enforcement_semaphore.add_permits(permits as usize);
    }

    /// Get current resource usage from the monitor
    pub async fn get_current_usage(&self) -> Result<ResourceSnapshot> {
        self.resource_monitor.get_current_usage().await
    }

    /// Get resource metrics from the monitor
    pub async fn get_metrics(&self) -> ResourceMetrics {
        self.resource_monitor.get_metrics().await
    }
}

// Clone implementation for ResourceGovernor
impl Clone for ResourceGovernor {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            current_limits: self.current_limits.clone(),
            resource_monitor: self.resource_monitor.clone(),
            violation_history: self.violation_history.clone(),
            enforcement_semaphore: self.enforcement_semaphore.clone(),
            shutdown_tx: self.shutdown_tx.clone(),
            shutdown_rx: self.shutdown_rx.clone(),
        }
    }
}

// Clone implementation for ResourceMonitor
impl Clone for ResourceMonitor {
    fn clone(&self) -> Self {
        Self {
            #[cfg(unix)]
            system: self.system.clone(),
            snapshot_history: self.snapshot_history.clone(),
            metrics: self.metrics.clone(),
            monitoring_interval: self.monitoring_interval,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resource_monitor_creation() {
        let monitor = ResourceMonitor::new(5).unwrap();
        let snapshot = monitor.take_snapshot().await.unwrap();
        assert!(snapshot.cpu_usage_percent >= 0.0);
        assert!(snapshot.memory_usage_mb > 0);
    }

    #[tokio::test]
    async fn test_governor_config_defaults() {
        let config = GovernorConfig::default();
        assert_eq!(config.market_hour_limits.max_cpu_percent, 25.0);
        assert_eq!(config.off_hour_limits.max_cpu_percent, 90.0);
    }

    #[tokio::test]
    async fn test_violation_creation() {
        let config = GovernorConfig::default();
        let market_hours = Arc::new(crate::utils::MarketHours::new());
        let governor = ResourceGovernor::new(config, market_hours).await.unwrap();
        
        let violation = governor.create_violation(
            ResourceType::CPU,
            50.0,  // Current value
            25.0,  // Limit value
        );
        
        assert_eq!(violation.resource_type, ResourceType::CPU);
        assert_eq!(violation.severity, ViolationSeverity::Critical);
    }
}