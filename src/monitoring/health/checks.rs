//! Health Check Implementations
//!
//! Component health checks and system health aggregation.

use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, warn};

use super::config::{ComponentHealth, ComponentType, HealthStatus, SystemHealth};

impl ComponentHealth {
    /// Create a new component health record
    pub fn new(component_type: ComponentType) -> Self {
        Self {
            component_type,
            status: HealthStatus::Unknown,
            last_check: Utc::now(),
            response_time_ms: None,
            error_message: None,
            metadata: HashMap::new(),
            uptime: Duration::from_secs(0),
            last_restart: None,
        }
    }

    /// Update health status with timing information
    pub fn update_status(&mut self, status: HealthStatus, response_time: Option<Duration>) {
        // Clear error message if now healthy
        if matches!(status, HealthStatus::Healthy) {
            self.error_message = None;
        }

        self.status = status;
        self.last_check = Utc::now();
        self.response_time_ms = response_time.map(|d| d.as_millis() as u64);
    }

    /// Set error message and mark as unhealthy
    pub fn set_error(&mut self, error: String) {
        self.status = HealthStatus::Unhealthy("Health check failed".to_string());
        self.error_message = Some(error);
        self.last_check = Utc::now();
    }

    /// Add metadata
    pub fn add_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    /// Check if component is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self.status, HealthStatus::Healthy)
    }

    /// Check if component is degraded
    pub fn is_degraded(&self) -> bool {
        matches!(self.status, HealthStatus::Degraded(_))
    }

    /// Check if component is unhealthy
    pub fn is_unhealthy(&self) -> bool {
        matches!(self.status, HealthStatus::Unhealthy(_))
    }

    /// Set restart timestamp
    pub fn mark_restart(&mut self) {
        self.last_restart = Some(Utc::now());
        self.uptime = Duration::from_secs(0);
    }

    /// Update uptime
    pub fn update_uptime(&mut self, uptime: Duration) {
        self.uptime = uptime;
    }
}

impl SystemHealth {
    /// Create system health from component health map
    pub fn from_components(
        components: HashMap<ComponentType, ComponentHealth>,
        start_time: Instant,
    ) -> Self {
        let total_components = components.len();
        let healthy_components = components.values().filter(|c| c.is_healthy()).count();
        let degraded_components = components.values().filter(|c| c.is_degraded()).count();
        let unhealthy_components = components.values().filter(|c| c.is_unhealthy()).count();

        // Determine overall status
        let overall_status = if unhealthy_components > 0 {
            HealthStatus::Unhealthy(format!("{} unhealthy components", unhealthy_components))
        } else if degraded_components > 0 {
            HealthStatus::Degraded(format!("{} degraded components", degraded_components))
        } else if healthy_components == total_components {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unknown
        };

        Self {
            overall_status,
            components,
            timestamp: Utc::now(),
            system_uptime: start_time.elapsed(),
            total_components,
            healthy_components,
            degraded_components,
            unhealthy_components,
        }
    }

    /// Get health score (0.0 to 1.0)
    pub fn health_score(&self) -> f64 {
        if self.total_components == 0 {
            return 1.0;
        }

        let healthy_weight = 1.0;
        let degraded_weight = 0.5;
        let unhealthy_weight = 0.0;

        let score = (self.healthy_components as f64 * healthy_weight
            + self.degraded_components as f64 * degraded_weight
            + self.unhealthy_components as f64 * unhealthy_weight)
            / self.total_components as f64;

        score
    }

    /// Check if system is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self.overall_status, HealthStatus::Healthy)
    }

    /// Check if system is degraded
    pub fn is_degraded(&self) -> bool {
        matches!(self.overall_status, HealthStatus::Degraded(_))
    }

    /// Check if system is unhealthy
    pub fn is_unhealthy(&self) -> bool {
        matches!(self.overall_status, HealthStatus::Unhealthy(_))
    }

    /// Get components by status
    pub fn get_components_by_status(&self, status: &HealthStatus) -> Vec<&ComponentHealth> {
        self.components
            .values()
            .filter(|c| std::mem::discriminant(&c.status) == std::mem::discriminant(status))
            .collect()
    }

    /// Get unhealthy components
    pub fn get_unhealthy_components(&self) -> Vec<&ComponentHealth> {
        self.components
            .values()
            .filter(|c| c.is_unhealthy())
            .collect()
    }

    /// Get degraded components
    pub fn get_degraded_components(&self) -> Vec<&ComponentHealth> {
        self.components
            .values()
            .filter(|c| c.is_degraded())
            .collect()
    }
}

/// Health check runner for performing health checks on components
#[derive(Debug)]
pub struct HealthChecker {
    component_health: Arc<RwLock<HashMap<ComponentType, ComponentHealth>>>,
}

impl HealthChecker {
    /// Create a new health checker
    pub fn new() -> Self {
        Self {
            component_health: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a component for health checking
    pub async fn register_component(&self, component_type: ComponentType) {
        let mut health_map = self.component_health.write().await;
        health_map.insert(component_type.clone(), ComponentHealth::new(component_type));
    }

    /// Perform health check on a specific component
    pub async fn check_component(&self, component_type: &ComponentType) -> Result<()> {
        let start_time = Instant::now();
        
        let health_result = match component_type {
            ComponentType::Database => self.check_database().await,
            ComponentType::Redis => self.check_redis().await,
            ComponentType::Streaming => self.check_streaming().await,
            ComponentType::DAAOrchestrator => self.check_daa_orchestrator().await,
            ComponentType::NeuralSystem => self.check_neural_system().await,
            ComponentType::EventBus => self.check_event_bus().await,
            ComponentType::DataPipeline => self.check_data_pipeline().await,
            ComponentType::Cache => self.check_cache().await,
        };

        let response_time = start_time.elapsed();
        let mut health_map = self.component_health.write().await;

        if let Some(component_health) = health_map.get_mut(component_type) {
            match health_result {
                Ok(status) => {
                    component_health.update_status(status, Some(response_time));
                    debug!("Health check passed for {:?}: {:?}", component_type, response_time);
                }
                Err(e) => {
                    component_health.set_error(e.to_string());
                    warn!("Health check failed for {:?}: {}", component_type, e);
                }
            }
        }

        Ok(())
    }

    /// Get current system health
    pub async fn get_system_health(&self, start_time: Instant) -> SystemHealth {
        let health_map = self.component_health.read().await;
        SystemHealth::from_components(health_map.clone(), start_time)
    }

    /// Check database health
    async fn check_database(&self) -> Result<HealthStatus> {
        // Placeholder implementation - in real system would check database connectivity
        // For now, simulate a healthy database
        Ok(HealthStatus::Healthy)
    }

    /// Check Redis health
    async fn check_redis(&self) -> Result<HealthStatus> {
        // Placeholder implementation - in real system would check Redis connectivity
        Ok(HealthStatus::Healthy)
    }

    /// Check streaming system health
    async fn check_streaming(&self) -> Result<HealthStatus> {
        // Placeholder implementation - in real system would check streaming system
        Ok(HealthStatus::Healthy)
    }

    /// Check DAA orchestrator health
    async fn check_daa_orchestrator(&self) -> Result<HealthStatus> {
        // Placeholder implementation - in real system would check DAA orchestrator
        Ok(HealthStatus::Healthy)
    }

    /// Check neural system health
    async fn check_neural_system(&self) -> Result<HealthStatus> {
        // Placeholder implementation - in real system would check neural network status
        Ok(HealthStatus::Healthy)
    }

    /// Check event bus health
    async fn check_event_bus(&self) -> Result<HealthStatus> {
        // Placeholder implementation - in real system would check event bus
        Ok(HealthStatus::Healthy)
    }

    /// Check data pipeline health
    async fn check_data_pipeline(&self) -> Result<HealthStatus> {
        // Placeholder implementation - in real system would check data pipeline
        Ok(HealthStatus::Healthy)
    }

    /// Check cache health
    async fn check_cache(&self) -> Result<HealthStatus> {
        // Placeholder implementation - in real system would check cache
        Ok(HealthStatus::Healthy)
    }

    /// Run health checks for all registered components
    pub async fn run_all_checks(&self) -> Result<()> {
        let health_map = self.component_health.read().await;
        let components: Vec<ComponentType> = health_map.keys().cloned().collect();
        drop(health_map); // Release the read lock

        for component in components {
            if let Err(e) = self.check_component(&component).await {
                error!("Failed to check component {:?}: {}", component, e);
            }
        }

        Ok(())
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}