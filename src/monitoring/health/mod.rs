//! Health Monitoring System for Autonomous Platform
//!
//! This module provides comprehensive health monitoring and observability for all
//! system components including database, cache, streaming, neural networks, and
//! DAA orchestrator agents.
//!
//! The health monitoring system has been refactored into modular components:
//! - `config`: Core types and configuration structures
//! - `checks`: Health check implementations and component monitoring
//! - `metrics`: Performance metrics collection and calculation
//! - `alerts`: Alert management and notification system
//! - `dashboard`: HTTP endpoints and reporting functionality

use anyhow::Result;
use chrono::Utc;
// Removed unused metrics imports to fix compilation
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

pub mod alerts;
pub mod checks;
pub mod config;
pub mod dashboard;
pub mod metrics;

// Re-export commonly used types and structs
pub use alerts::{Alert, AlertManager};
pub use checks::{HealthChecker};
pub use config::{
    AlertConfig, AlertSeverity, AlertType, ComponentHealth, ComponentType, HealthStatus,
    PerformanceMetrics, SystemHealth,
};
pub use dashboard::{HealthEndpoints, HealthMonitorInterface, HealthReporter};
pub use metrics::MetricsCollector;

/// Main health monitoring system
#[derive(Debug)]
pub struct HealthMonitor {
    component_health: Arc<RwLock<HashMap<ComponentType, ComponentHealth>>>,
    pub metrics_collector: MetricsCollector,
    pub alert_manager: AlertManager,
    health_checker: HealthChecker,
    start_time: Instant,
    monitoring_interval: Duration,
    is_monitoring: Arc<RwLock<bool>>,
}

impl HealthMonitor {
    /// Create a new health monitor
    pub async fn new() -> Result<Self> {
        let metrics_collector = MetricsCollector::new();
        let alert_manager = AlertManager::new();
        let health_checker = HealthChecker::new();

        let component_health = Arc::new(RwLock::new(HashMap::new()));
        let is_monitoring = Arc::new(RwLock::new(false));

        let monitor = Self {
            component_health: component_health.clone(),
            metrics_collector,
            alert_manager,
            health_checker,
            start_time: Instant::now(),
            monitoring_interval: Duration::from_secs(30),
            is_monitoring,
        };

        Ok(monitor)
    }

    /// Check health of a specific component
    pub async fn check_component_health(
        &self,
        component: ComponentType,
    ) -> Result<ComponentHealth> {
        let start_time = Instant::now();
        let mut health = ComponentHealth::new(component.clone());

        let result = match component {
            ComponentType::Database => self.check_database_health(&mut health).await,
            ComponentType::Redis => self.check_redis_health(&mut health).await,
            ComponentType::Streaming => self.check_streaming_health(&mut health).await,
            ComponentType::DAAOrchestrator => self.check_daa_health(&mut health).await,
            ComponentType::NeuralSystem => self.check_neural_health(&mut health).await,
            ComponentType::EventBus => self.check_event_bus_health(&mut health).await,
            ComponentType::DataPipeline => self.check_data_pipeline_health(&mut health).await,
            ComponentType::Cache => self.check_cache_health(&mut health).await,
        };

        let elapsed = start_time.elapsed();

        match result {
            Ok(()) => {
                health.update_status(HealthStatus::Healthy, Some(elapsed));
                self.metrics_collector
                    .record_latency(&component, elapsed)
                    .await;
                self.metrics_collector.record_throughput().await;
            }
            Err(e) => {
                health.set_error(e.to_string());
                self.metrics_collector
                    .record_error(&component, &e.to_string())
                    .await;
            }
        }

        // counter!("component_health_checks_total").increment(1);

        // Store in component health map
        self.component_health
            .write()
            .await
            .insert(component, health.clone());

        Ok(health)
    }

    /// Get overall system health
    pub async fn get_system_health(&self) -> Result<SystemHealth> {
        let components = vec![
            ComponentType::Database,
            ComponentType::Redis,
            ComponentType::Streaming,
            ComponentType::DAAOrchestrator,
            ComponentType::NeuralSystem,
            ComponentType::EventBus,
            ComponentType::DataPipeline,
            ComponentType::Cache,
        ];

        let mut component_health = HashMap::new();

        for component in components {
            let health = self.check_component_health(component).await?;
            component_health.insert(health.component_type.clone(), health);
        }

        let system_health = SystemHealth::from_components(component_health, self.start_time);

        // Record system health score
        // gauge!("system_health_score").set(system_health.health_score());

        Ok(system_health)
    }

    /// Start continuous monitoring
    pub async fn start_monitoring(&self) -> Result<()> {
        let mut is_monitoring = self.is_monitoring.write().await;
        if *is_monitoring {
            return Ok(());
        }
        *is_monitoring = true;

        info!("Starting health monitoring system");

        // Start monitoring loop
        let monitor = self.clone();
        tokio::spawn(async move {
            monitor.monitoring_loop().await;
        });

        Ok(())
    }

    /// Stop monitoring
    pub async fn stop_monitoring(&self) -> Result<()> {
        let mut is_monitoring = self.is_monitoring.write().await;
        *is_monitoring = false;
        info!("Stopping health monitoring system");
        Ok(())
    }

    /// Collect performance metrics
    pub async fn collect_performance_metrics(&self) -> Result<PerformanceMetrics> {
        Ok(self.metrics_collector.calculate_metrics().await?)
    }

    /// Add alert configuration
    pub async fn add_alert_config(&self, config: AlertConfig) -> Result<()> {
        self.alert_manager.add_config(config).await
    }

    /// Check for alerts
    pub async fn check_alerts(&self) -> Result<Vec<Alert>> {
        let metrics = self.collect_performance_metrics().await?;
        let health = self.get_system_health().await?;
        self.alert_manager.check_alerts(&metrics, &health).await
    }

    /// Register a component for monitoring
    pub async fn register_component(&self, component: ComponentType) -> Result<()> {
        let health = ComponentHealth::new(component.clone());
        self.component_health
            .write()
            .await
            .insert(component.clone(), health);
        info!("Registered component for monitoring: {:?}", component);
        Ok(())
    }

    /// Update component health directly
    pub async fn update_component_health(
        &self,
        component: ComponentType,
        health: ComponentHealth,
    ) -> Result<()> {
        self.component_health
            .write()
            .await
            .insert(component, health);
        Ok(())
    }

    /// Get health endpoints for HTTP/REST API
    pub fn get_endpoints(&self) -> HealthEndpoints<Self> {
        HealthEndpoints::new(Arc::new(self.clone()))
    }

    /// Main monitoring loop
    async fn monitoring_loop(&self) {
        let mut interval = interval(self.monitoring_interval);

        loop {
            interval.tick().await;

            // Check if monitoring should continue
            if !*self.is_monitoring.read().await {
                break;
            }

            // Perform health checks
            match self.get_system_health().await {
                Ok(health) => {
                    debug!(
                        "System health check completed: {} components checked",
                        health.total_components
                    );

                    // Check for alerts
                    match self.check_alerts().await {
                        Ok(alerts) => {
                            if !alerts.is_empty() {
                                warn!("Generated {} new alerts", alerts.len());
                                for alert in alerts {
                                    warn!("Alert: {} - {}", alert.severity, alert.message);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to check alerts: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to get system health: {}", e);
                }
            }
        }

        info!("Health monitoring loop stopped");
    }

    /// Component-specific health check implementations
    async fn check_database_health(&self, _health: &mut ComponentHealth) -> Result<()> {
        // Placeholder implementation - in real system would check database connectivity
        Ok(())
    }

    async fn check_redis_health(&self, _health: &mut ComponentHealth) -> Result<()> {
        // Placeholder implementation - in real system would check Redis connectivity
        Ok(())
    }

    async fn check_streaming_health(&self, _health: &mut ComponentHealth) -> Result<()> {
        // Placeholder implementation - in real system would check streaming system
        Ok(())
    }

    async fn check_daa_health(&self, _health: &mut ComponentHealth) -> Result<()> {
        // Placeholder implementation - in real system would check DAA orchestrator
        Ok(())
    }

    async fn check_neural_health(&self, _health: &mut ComponentHealth) -> Result<()> {
        // Placeholder implementation - in real system would check neural network status
        Ok(())
    }

    async fn check_event_bus_health(&self, _health: &mut ComponentHealth) -> Result<()> {
        // Placeholder implementation - in real system would check event bus
        Ok(())
    }

    async fn check_data_pipeline_health(&self, _health: &mut ComponentHealth) -> Result<()> {
        // Placeholder implementation - in real system would check data pipeline
        Ok(())
    }

    async fn check_cache_health(&self, _health: &mut ComponentHealth) -> Result<()> {
        // Placeholder implementation - in real system would check cache
        Ok(())
    }
}

impl HealthMonitorInterface for HealthMonitor {
    async fn get_system_health(&self) -> Result<SystemHealth> {
        self.get_system_health().await
    }

    async fn collect_performance_metrics(&self) -> Result<PerformanceMetrics> {
        self.collect_performance_metrics().await
    }

    fn get_alert_manager(&self) -> &AlertManager {
        &self.alert_manager
    }
}

impl Clone for HealthMonitor {
    fn clone(&self) -> Self {
        Self {
            component_health: self.component_health.clone(),
            metrics_collector: self.metrics_collector.clone(),
            alert_manager: AlertManager::new(), // Create new alert manager for cloned instance
            health_checker: HealthChecker::new(),
            start_time: self.start_time,
            monitoring_interval: self.monitoring_interval,
            is_monitoring: self.is_monitoring.clone(),
        }
    }
}