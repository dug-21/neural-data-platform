//! Integration hooks between resource monitoring and health system
//!
//! Provides seamless integration between the ResourceGovernor and the
//! existing health monitoring infrastructure.

use crate::monitoring::health::{
    ComponentHealth, ComponentType, HealthStatus, HealthMonitor,
    AlertConfig, AlertType, AlertSeverity,
};
use crate::utils::resource_monitor::{
    ResourceGovernor, ResourceSnapshot, ResourceViolation, 
    ViolationSeverity, ResourceType,
};
use anyhow::{Context, Result};
use chrono::Duration;
use std::sync::Arc;
use tracing::{error, info};

/// Resource health integration component
pub struct ResourceHealthIntegration {
    resource_governor: Arc<ResourceGovernor>,
    health_monitor: Arc<HealthMonitor>,
    component_type: ComponentType,
}

impl ResourceHealthIntegration {
    /// Create new resource health integration
    pub async fn new(
        resource_governor: Arc<ResourceGovernor>,
        health_monitor: Arc<HealthMonitor>,
    ) -> Result<Self> {
        // Register resource monitoring as a component
        let component_type = ComponentType::Cache; // Using Cache as proxy for resources
        
        // Set up alerts for resource violations
        let alerts = vec![
            AlertConfig {
                id: "resource_cpu_warning".to_string(),
                component: component_type.clone(),
                metric_name: "cpu_usage".to_string(),
                threshold: 80.0,
                alert_type: AlertType::Threshold,
                enabled: true,
                cooldown_minutes: 5,
            },
            AlertConfig {
                id: "resource_cpu_critical".to_string(),
                component: component_type.clone(),
                metric_name: "cpu_usage".to_string(),
                threshold: 95.0,
                alert_type: AlertType::Threshold,
                enabled: true,
                cooldown_minutes: 2,
            },
            AlertConfig {
                id: "resource_memory_warning".to_string(),
                component: component_type.clone(),
                metric_name: "memory_usage".to_string(),
                threshold: 80.0,
                alert_type: AlertType::Threshold,
                enabled: true,
                cooldown_minutes: 5,
            },
            AlertConfig {
                id: "resource_violations".to_string(),
                component: component_type.clone(),
                metric_name: "violation_rate".to_string(),
                threshold: 0.1, // More than 10% violation rate
                alert_type: AlertType::PerformanceDegradation,
                enabled: true,
                cooldown_minutes: 10,
            },
        ];
        
        // Add alerts to health monitor
        for alert in alerts {
            health_monitor.add_alert_config(alert).await?;
        }
        
        Ok(Self {
            resource_governor,
            health_monitor,
            component_type,
        })
    }

    /// Update health status based on resource usage
    pub async fn update_health_status(&self) -> Result<()> {
        // Get current resource snapshot
        let snapshot = self.resource_governor
            .get_current_usage()
            .await
            .context("Failed to get resource usage")?;
        
        // Get recent violations
        let violations = self.resource_governor
            .get_violation_history(Duration::minutes(5))
            .await;
        
        // Create component health
        let mut health = ComponentHealth::new(self.component_type.clone());
        
        // Determine health status based on resource usage and violations
        let status = self.determine_health_status(&snapshot, &violations);
        health.update_status(status, None);
        
        // Add metadata
        self.add_health_metadata(&mut health, &snapshot, &violations).await;
        
        // Update health monitor
        Arc::clone(&self.health_monitor)
            .update_component_health(self.component_type.clone(), health)
            .await?;
        
        Ok(())
    }

    /// Determine health status from resource data
    fn determine_health_status(
        &self,
        snapshot: &ResourceSnapshot,
        violations: &[ResourceViolation],
    ) -> HealthStatus {
        // Count critical violations
        let critical_violations = violations
            .iter()
            .filter(|v| matches!(v.severity, ViolationSeverity::Critical | ViolationSeverity::Emergency))
            .count();
        
        // Check current resource usage
        let cpu_critical = snapshot.cpu_usage_percent > 90.0;
        let memory_critical = snapshot.memory_percent > 90.0;
        let load_critical = snapshot.load_average.one_minute > 10.0;
        
        if critical_violations > 5 || cpu_critical || memory_critical || load_critical {
            HealthStatus::Unhealthy(format!(
                "Critical resource constraints: {} violations, CPU: {:.1}%, Memory: {:.1}%",
                critical_violations,
                snapshot.cpu_usage_percent,
                snapshot.memory_percent
            ))
        } else if critical_violations > 0 || snapshot.cpu_usage_percent > 70.0 || snapshot.memory_percent > 70.0 {
            HealthStatus::Degraded(format!(
                "Elevated resource usage: CPU: {:.1}%, Memory: {:.1}%",
                snapshot.cpu_usage_percent,
                snapshot.memory_percent
            ))
        } else {
            HealthStatus::Healthy
        }
    }

    /// Add metadata to health component
    async fn add_health_metadata(
        &self,
        health: &mut ComponentHealth,
        snapshot: &ResourceSnapshot,
        violations: &[ResourceViolation],
    ) {
        // Resource usage metrics
        health.add_metadata("cpu_usage_percent".to_string(), format!("{:.1}", snapshot.cpu_usage_percent));
        health.add_metadata("memory_usage_mb".to_string(), snapshot.memory_usage_mb.to_string());
        health.add_metadata("memory_percent".to_string(), format!("{:.1}", snapshot.memory_percent));
        health.add_metadata("load_1m".to_string(), format!("{:.2}", snapshot.load_average.one_minute));
        health.add_metadata("load_5m".to_string(), format!("{:.2}", snapshot.load_average.five_minute));
        health.add_metadata("load_15m".to_string(), format!("{:.2}", snapshot.load_average.fifteen_minute));
        
        // Violation statistics
        health.add_metadata("violations_5m".to_string(), violations.len().to_string());
        health.add_metadata("critical_violations_5m".to_string(), 
            violations.iter()
                .filter(|v| matches!(v.severity, ViolationSeverity::Critical | ViolationSeverity::Emergency))
                .count()
                .to_string()
        );
        
        // Current limits
        let limits = self.resource_governor.get_current_limits().await;
        health.add_metadata("cpu_limit_percent".to_string(), format!("{:.1}", limits.max_cpu_percent));
        health.add_metadata("memory_limit_mb".to_string(), limits.max_memory_mb.to_string());
        
        // Governor status
        let governor_status = self.resource_governor.get_status().await;
        if let Some(mode) = governor_status.get("enforcement_mode").and_then(|v| v.as_str()) {
            health.add_metadata("enforcement_mode".to_string(), mode.to_string());
        }
        
        // Resource metrics
        let metrics = self.resource_governor.get_metrics().await;
        health.add_metadata("avg_cpu_usage".to_string(), format!("{:.1}", metrics.avg_cpu_usage));
        health.add_metadata("peak_cpu_usage".to_string(), format!("{:.1}", metrics.peak_cpu_usage));
        health.add_metadata("total_violations".to_string(), metrics.total_violations.to_string());
        health.add_metadata("enforcement_actions".to_string(), metrics.enforcement_actions.to_string());
    }

    /// Start continuous health monitoring integration
    pub async fn start_monitoring(&self) -> Result<()> {
        let integration = Arc::new(self.clone());
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                if let Err(e) = integration.update_health_status().await {
                    error!("Failed to update resource health status: {}", e);
                }
            }
        });
        
        info!("Started resource health monitoring integration");
        Ok(())
    }

    /// Convert resource violation to health alert
    pub fn violation_to_alert_severity(violation: &ResourceViolation) -> AlertSeverity {
        match violation.severity {
            ViolationSeverity::Emergency => AlertSeverity::Critical,
            ViolationSeverity::Critical => AlertSeverity::Critical,
            ViolationSeverity::Warning => AlertSeverity::Warning,
        }
    }

    /// Get resource health summary
    pub async fn get_health_summary(&self) -> Result<serde_json::Value> {
        let snapshot = self.resource_governor.get_current_usage().await?;
        let violations = self.resource_governor.get_violation_history(Duration::minutes(60)).await;
        let metrics = self.resource_governor.get_metrics().await;
        let limits = self.resource_governor.get_current_limits().await;
        
        Ok(serde_json::json!({
            "current_usage": {
                "cpu_percent": snapshot.cpu_usage_percent,
                "memory_mb": snapshot.memory_usage_mb,
                "memory_percent": snapshot.memory_percent,
                "load_average": snapshot.load_average,
            },
            "current_limits": {
                "cpu_percent": limits.max_cpu_percent,
                "memory_mb": limits.max_memory_mb,
                "memory_percent": limits.max_memory_percent,
            },
            "violations_summary": {
                "last_hour": violations.len(),
                "critical": violations.iter()
                    .filter(|v| matches!(v.severity, ViolationSeverity::Critical | ViolationSeverity::Emergency))
                    .count(),
                "by_type": {
                    "cpu": violations.iter().filter(|v| v.resource_type == ResourceType::CPU).count(),
                    "memory": violations.iter().filter(|v| v.resource_type == ResourceType::Memory).count(),
                    "disk_io": violations.iter().filter(|v| v.resource_type == ResourceType::DiskIO).count(),
                    "network": violations.iter().filter(|v| v.resource_type == ResourceType::Network).count(),
                }
            },
            "performance_metrics": {
                "avg_cpu": metrics.avg_cpu_usage,
                "peak_cpu": metrics.peak_cpu_usage,
                "avg_memory_mb": metrics.avg_memory_usage_mb,
                "peak_memory_mb": metrics.peak_memory_usage_mb,
                "total_violations": metrics.total_violations,
                "enforcement_actions": metrics.enforcement_actions,
            },
            "health_score": self.calculate_health_score(&snapshot, &violations, &metrics),
        }))
    }

    /// Calculate overall resource health score (0.0 - 1.0)
    fn calculate_health_score(
        &self,
        snapshot: &ResourceSnapshot,
        violations: &[ResourceViolation],
        metrics: &crate::utils::resource_monitor::ResourceMetrics,
    ) -> f64 {
        let mut score = 1.0;
        
        // Deduct for current usage
        score -= (snapshot.cpu_usage_percent / 100.0) * 0.3;
        score -= (snapshot.memory_percent / 100.0) * 0.2;
        
        // Deduct for violations
        let violation_rate = violations.len() as f64 / 60.0; // violations per minute
        score -= violation_rate.min(0.3);
        
        // Deduct for enforcement actions
        if metrics.enforcement_actions > 0 {
            score -= 0.1;
        }
        
        // Ensure score is in valid range
        score.max(0.0).min(1.0)
    }
}

// Clone implementation
impl Clone for ResourceHealthIntegration {
    fn clone(&self) -> Self {
        Self {
            resource_governor: self.resource_governor.clone(),
            health_monitor: self.health_monitor.clone(),
            component_type: self.component_type.clone(),
        }
    }
}

/// Extension trait for HealthMonitor to add resource monitoring
pub trait HealthMonitorResourceExt {
    /// Add resource monitoring to health monitor
    async fn add_resource_monitoring(
        &self,
        resource_governor: Arc<ResourceGovernor>,
    ) -> Result<Arc<ResourceHealthIntegration>>;
}

impl HealthMonitorResourceExt for HealthMonitor {
    async fn add_resource_monitoring(
        &self,
        resource_governor: Arc<ResourceGovernor>,
    ) -> Result<Arc<ResourceHealthIntegration>> {
        let integration = Arc::new(
            ResourceHealthIntegration::new(
                resource_governor,
                Arc::new(self.clone()),
            ).await?
        );
        
        integration.start_monitoring().await?;
        
        Ok(integration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::resource_monitor::GovernorConfig;
    use crate::utils::MarketHours;

    #[tokio::test]
    async fn test_health_integration_creation() {
        let config = GovernorConfig::default();
        let market_hours = Arc::new(MarketHours::new());
        let resource_governor = Arc::new(
            ResourceGovernor::new(config, market_hours).await.unwrap()
        );
        let health_monitor = Arc::new(HealthMonitor::new().await.unwrap());
        
        let integration = ResourceHealthIntegration::new(
            resource_governor,
            health_monitor,
        ).await.unwrap();
        
        assert_eq!(integration.component_type, ComponentType::Cache);
    }

    #[tokio::test]
    async fn test_health_score_calculation() {
        let config = GovernorConfig::default();
        let market_hours = Arc::new(MarketHours::new());
        let resource_governor = Arc::new(
            ResourceGovernor::new(config, market_hours).await.unwrap()
        );
        let health_monitor = Arc::new(HealthMonitor::new().await.unwrap());
        
        let integration = ResourceHealthIntegration::new(
            resource_governor,
            health_monitor,
        ).await.unwrap();
        
        let snapshot = ResourceSnapshot {
            timestamp: Utc::now(),
            cpu_usage_percent: 50.0,
            memory_usage_mb: 1000,
            memory_percent: 25.0,
            disk_io_read_mbps: 0.0,
            disk_io_write_mbps: 0.0,
            network_rx_mbps: 0.0,
            network_tx_mbps: 0.0,
            process_count: 1,
            thread_count: 1,
            load_average: crate::utils::resource_monitor::LoadAverage {
                one_minute: 1.0,
                five_minute: 1.0,
                fifteen_minute: 1.0,
            },
        };
        
        let violations = vec![];
        let metrics = crate::utils::resource_monitor::ResourceMetrics::default();
        
        let score = integration.calculate_health_score(&snapshot, &violations, &metrics);
        
        // With 50% CPU and 25% memory, score should be reduced
        assert!(score > 0.5 && score < 0.8);
    }
}