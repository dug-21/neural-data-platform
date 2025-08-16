//! AsyncHealthMonitor implementation for non-blocking health monitoring

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use super::{
    ComponentHealth, ComponentType, HealthCheckResult, HealthChecker, HealthMonitorConfig,
    HealthStatus, SystemHealth,
};

/// Non-blocking health monitor that runs checks in the background
pub struct AsyncHealthMonitor {
    /// Configuration for health monitoring
    config: HealthMonitorConfig,
    
    /// Shared state protected by RwLock for concurrent access
    state: Arc<RwLock<HealthMonitorState>>,
    
    /// Cancellation token for graceful shutdown
    shutdown_token: CancellationToken,
    
    /// Background task handle
    monitoring_task: Option<JoinHandle<()>>,
    
    /// Health checkers for each component
    health_checkers: Arc<HashMap<ComponentType, Box<dyn HealthChecker>>>,
}

/// Internal state of the health monitor
#[derive(Debug)]
struct HealthMonitorState {
    /// Current health status of each component
    component_health: HashMap<ComponentType, ComponentHealth>,
    
    /// System-wide health metrics
    system_health: SystemHealth,
    
    /// Whether monitoring is currently active
    is_monitoring: bool,
    
    /// Last update timestamp
    last_update: Instant,
}

impl AsyncHealthMonitor {
    /// Create a new async health monitor
    pub fn new(config: HealthMonitorConfig) -> Self {
        let initial_state = HealthMonitorState {
            component_health: HashMap::new(),
            system_health: SystemHealth::default(),
            is_monitoring: false,
            last_update: Instant::now(),
        };

        Self {
            config,
            state: Arc::new(RwLock::new(initial_state)),
            shutdown_token: CancellationToken::new(),
            monitoring_task: None,
            health_checkers: Arc::new(HashMap::new()),
        }
    }

    /// Start health monitoring in the background (non-blocking)
    pub async fn start(&mut self) -> Result<()> {
        // Check if already running
        {
            let state = self.state.read().await;
            if state.is_monitoring {
                warn!("Health monitoring is already running");
                return Ok(());
            }
        }

        info!("Starting async health monitoring");

        // Update state to indicate monitoring is starting
        {
            let mut state = self.state.write().await;
            state.is_monitoring = true;
        }

        // Clone necessary components for the background task
        let config = self.config.clone();
        let state = Arc::clone(&self.state);
        let shutdown_token = self.shutdown_token.clone();
        let health_checkers = Arc::clone(&self.health_checkers);

        // Spawn the monitoring task
        let monitoring_task = tokio::spawn(async move {
            Self::monitoring_loop(config, state, shutdown_token, health_checkers).await;
        });

        self.monitoring_task = Some(monitoring_task);

        debug!("Health monitoring started successfully");
        Ok(())
    }

    /// Stop health monitoring gracefully
    pub async fn stop(&mut self) {
        info!("Stopping health monitoring");

        // Signal shutdown
        self.shutdown_token.cancel();

        // Wait for the monitoring task to complete
        if let Some(task) = self.monitoring_task.take() {
            match tokio::time::timeout(Duration::from_secs(5), task).await {
                Ok(Ok(())) => info!("Health monitoring stopped successfully"),
                Ok(Err(e)) => error!("Error stopping health monitoring: {}", e),
                Err(_) => warn!("Health monitoring shutdown timed out"),
            }
        }

        // Update state
        {
            let mut state = self.state.write().await;
            state.is_monitoring = false;
        }
    }

    /// Check if monitoring is currently running
    pub fn is_running(&self) -> bool {
        self.monitoring_task.is_some()
    }

    /// Get current system health (non-blocking read)
    pub async fn get_system_health(&self) -> Result<SystemHealth> {
        let state = self.state.read().await;
        Ok(state.system_health.clone())
    }

    /// Get health status for a specific component
    pub async fn get_component_health(&self, component: ComponentType) -> Option<ComponentHealth> {
        let state = self.state.read().await;
        state.component_health.get(&component).cloned()
    }

    /// Register a new component for health monitoring
    pub async fn register_component(&mut self, component: ComponentType) -> Result<()> {
        let mut state = self.state.write().await;
        
        // Add component with unknown status initially
        state.component_health.insert(
            component.clone(),
            ComponentHealth {
                component_type: component,
                status: HealthStatus::Unknown,
                last_check: Instant::now(),
                response_time_ms: None,
                error_message: None,
                consecutive_failures: 0,
                metadata: HashMap::new(),
            },
        );

        // Update total component count
        state.system_health.total_components = state.component_health.len();

        Ok(())
    }

    /// Get detailed metrics about the health monitor
    pub async fn get_detailed_metrics(&self) -> Result<DetailedMetrics> {
        let state = self.state.read().await;
        
        Ok(DetailedMetrics {
            history_entries: state.component_health.len(),
            total_components: state.system_health.total_components,
            healthy_components: state.system_health.healthy_components,
            degraded_components: state.system_health.degraded_components,
            unhealthy_components: state.system_health.unhealthy_components,
            last_update_ms_ago: state.last_update.elapsed().as_millis() as u64,
        })
    }

    /// The main monitoring loop that runs in the background
    async fn monitoring_loop(
        config: HealthMonitorConfig,
        state: Arc<RwLock<HealthMonitorState>>,
        shutdown_token: CancellationToken,
        health_checkers: Arc<HashMap<ComponentType, Box<dyn HealthChecker>>>,
    ) {
        let mut interval = tokio::time::interval(config.check_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        info!("Health monitoring loop started");

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Perform health checks
                    Self::perform_health_checks(&config, &state, &health_checkers).await;
                }
                _ = shutdown_token.cancelled() => {
                    info!("Health monitoring loop received shutdown signal");
                    break;
                }
            }
        }

        info!("Health monitoring loop ended");
    }

    /// Perform health checks for all registered components
    async fn perform_health_checks(
        config: &HealthMonitorConfig,
        state: &Arc<RwLock<HealthMonitorState>>,
        health_checkers: &Arc<HashMap<ComponentType, Box<dyn HealthChecker>>>,
    ) {
        debug!("Performing health checks");

        // Get list of components to check
        let components_to_check: Vec<ComponentType> = {
            let state_read = state.read().await;
            state_read.component_health.keys().cloned().collect()
        };

        // Perform health checks concurrently
        let mut check_futures = Vec::new();

        for component in components_to_check {
            if let Some(checker) = health_checkers.get(&component) {
                let checker = checker.clone();
                let timeout_duration = config.check_timeout;
                
                let future = async move {
                    let start = Instant::now();
                    
                    // Perform health check with timeout
                    let result = match tokio::time::timeout(
                        timeout_duration,
                        checker.check_health(),
                    )
                    .await
                    {
                        Ok(Ok(health)) => health,
                        Ok(Err(e)) => HealthCheckResult {
                            component_type: component.clone(),
                            is_healthy: false,
                            response_time_ms: Some(start.elapsed().as_millis() as u64),
                            error_message: Some(format!("Health check failed: {}", e)),
                            metadata: HashMap::new(),
                        },
                        Err(_) => HealthCheckResult {
                            component_type: component.clone(),
                            is_healthy: false,
                            response_time_ms: Some(timeout_duration.as_millis() as u64),
                            error_message: Some("Health check timeout".to_string()),
                            metadata: HashMap::new(),
                        },
                    };

                    (component, result)
                };

                check_futures.push(future);
            }
        }

        // Wait for all health checks to complete
        let results = futures::future::join_all(check_futures).await;

        // Update state with results
        let mut state_write = state.write().await;
        
        for (component, result) in results {
            // Update component health
            if let Some(health) = state_write.component_health.get_mut(&component) {
                health.status = if result.is_healthy {
                    HealthStatus::Healthy
                } else {
                    HealthStatus::Unhealthy
                };
                health.last_check = Instant::now();
                health.response_time_ms = result.response_time_ms;
                health.error_message = result.error_message;
                
                // Update consecutive failure count
                if result.is_healthy {
                    health.consecutive_failures = 0;
                } else {
                    health.consecutive_failures += 1;
                }
                
                health.metadata = result.metadata;
            }
        }

        // Update system health summary
        Self::update_system_health(&mut state_write);
        state_write.last_update = Instant::now();
    }

    /// Update the overall system health based on component health
    fn update_system_health(state: &mut HealthMonitorState) {
        let mut healthy_count = 0;
        let mut degraded_count = 0;
        let mut unhealthy_count = 0;
        let mut unknown_count = 0;

        for health in state.component_health.values() {
            match health.status {
                HealthStatus::Healthy => healthy_count += 1,
                HealthStatus::Degraded => degraded_count += 1,
                HealthStatus::Unhealthy => unhealthy_count += 1,
                HealthStatus::Unknown => unknown_count += 1,
            }
        }

        state.system_health.healthy_components = healthy_count;
        state.system_health.degraded_components = degraded_count;
        state.system_health.unhealthy_components = unhealthy_count;
        state.system_health.total_components = state.component_health.len();

        // Calculate health score (0.0 to 1.0)
        if state.system_health.total_components > 0 {
            state.system_health.health_score = 
                healthy_count as f64 / state.system_health.total_components as f64;
        } else {
            state.system_health.health_score = 0.0;
        }

        // Determine overall status
        state.system_health.status = if unhealthy_count > 0 {
            "unhealthy"
        } else if degraded_count > 0 {
            "degraded"
        } else if healthy_count > 0 {
            "healthy"
        } else {
            "unknown"
        };

        debug!(
            "System health updated: {} healthy, {} degraded, {} unhealthy, score: {:.2}",
            healthy_count, degraded_count, unhealthy_count, state.system_health.health_score
        );
    }
}

/// Detailed metrics about the health monitor
#[derive(Debug, Clone)]
pub struct DetailedMetrics {
    pub history_entries: usize,
    pub total_components: usize,
    pub healthy_components: usize,
    pub degraded_components: usize,
    pub unhealthy_components: usize,
    pub last_update_ms_ago: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_async_health_monitor_starts_quickly() {
        let mut monitor = AsyncHealthMonitor::new(HealthMonitorConfig::default());
        
        let start = Instant::now();
        let result = monitor.start().await;
        let elapsed = start.elapsed();

        assert!(result.is_ok());
        assert!(elapsed < Duration::from_millis(100));
        assert!(monitor.is_running());

        monitor.stop().await;
    }

    #[tokio::test]
    async fn test_health_monitor_non_blocking() {
        let mut monitor = AsyncHealthMonitor::new(HealthMonitorConfig::default());
        monitor.start().await.unwrap();

        // Should be able to get health immediately
        let health = monitor.get_system_health().await.unwrap();
        assert_eq!(health.total_components, 0); // No components registered yet

        monitor.stop().await;
    }

    #[tokio::test]
    async fn test_register_component() {
        let mut monitor = AsyncHealthMonitor::new(HealthMonitorConfig::default());
        monitor.start().await.unwrap();

        // Register a component
        monitor.register_component(ComponentType::Database).await.unwrap();

        // Should reflect in system health
        let health = monitor.get_system_health().await.unwrap();
        assert_eq!(health.total_components, 1);

        monitor.stop().await;
    }
}