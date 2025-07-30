//! Health Monitoring Alert System
//!
//! Alert management, processing, and notification system for health monitoring.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::config::{AlertConfig, AlertSeverity, AlertType, ComponentType, PerformanceMetrics, SystemHealth};

/// Alert instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub config_id: String,
    pub component: ComponentType,
    pub severity: AlertSeverity,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub resolved: bool,
    pub resolved_at: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, String>,
}

/// Alert manager for handling alerts
#[derive(Debug)]
pub struct AlertManager {
    configs: Arc<RwLock<HashMap<String, AlertConfig>>>,
    active_alerts: Arc<RwLock<HashMap<String, Alert>>>,
    alert_history: Arc<RwLock<Vec<Alert>>>,
}

impl AlertManager {
    /// Create a new alert manager
    pub fn new() -> Self {
        Self {
            configs: Arc::new(RwLock::new(HashMap::new())),
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            alert_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Add an alert configuration
    pub async fn add_config(&self, config: AlertConfig) -> Result<()> {
        let mut configs = self.configs.write().await;
        configs.insert(config.id.clone(), config);
        Ok(())
    }

    /// Remove an alert configuration
    pub async fn remove_config(&self, config_id: &str) -> Result<()> {
        let mut configs = self.configs.write().await;
        configs.remove(config_id);
        Ok(())
    }

    /// Check alerts based on current metrics
    pub async fn check_alerts(
        &self,
        metrics: &PerformanceMetrics,
        health: &SystemHealth,
    ) -> Result<Vec<Alert>> {
        let configs = self.configs.read().await;
        let mut new_alerts = Vec::new();

        for config in configs.values() {
            if !config.enabled {
                continue;
            }

            let should_alert = match config.alert_type {
                AlertType::Threshold => self.check_threshold_alert(config, metrics, health).await?,
                AlertType::Availability => self.check_availability_alert(config, health).await?,
                AlertType::PerformanceDegradation => {
                    self.check_performance_alert(config, metrics).await?
                }
                AlertType::Anomaly => self.check_anomaly_alert(config, metrics).await?,
            };

            if should_alert {
                let alert = Alert {
                    id: Uuid::new_v4().to_string(),
                    config_id: config.id.clone(),
                    component: config.component.clone(),
                    severity: self.determine_severity(config, metrics, health),
                    message: self.generate_alert_message(config, metrics, health),
                    timestamp: Utc::now(),
                    resolved: false,
                    resolved_at: None,
                    metadata: HashMap::new(),
                };

                new_alerts.push(alert);
            }
        }

        // Store new alerts
        if !new_alerts.is_empty() {
            let mut active_alerts = self.active_alerts.write().await;
            let mut history = self.alert_history.write().await;

            for alert in &new_alerts {
                active_alerts.insert(alert.id.clone(), alert.clone());
                history.push(alert.clone());
            }
        }

        Ok(new_alerts)
    }

    /// Resolve an alert
    pub async fn resolve_alert(&self, alert_id: &str) -> Result<()> {
        let mut active_alerts = self.active_alerts.write().await;

        if let Some(alert) = active_alerts.get_mut(alert_id) {
            alert.resolved = true;
            alert.resolved_at = Some(Utc::now());
        }

        Ok(())
    }

    /// Get active alerts
    pub async fn get_active_alerts(&self) -> Vec<Alert> {
        let active_alerts = self.active_alerts.read().await;
        active_alerts.values().cloned().collect()
    }

    /// Get alert history
    pub async fn get_alert_history(&self) -> Vec<Alert> {
        let history = self.alert_history.read().await;
        history.clone()
    }

    /// Get alert configurations
    pub async fn get_configs(&self) -> Vec<AlertConfig> {
        let configs = self.configs.read().await;
        configs.values().cloned().collect()
    }

    async fn check_threshold_alert(
        &self,
        config: &AlertConfig,
        metrics: &PerformanceMetrics,
        _health: &SystemHealth,
    ) -> Result<bool> {
        let value = match config.metric_name.as_str() {
            "error_rate" => metrics.error_rate,
            "cpu_usage" => metrics.cpu_usage_percent,
            "memory_usage" => metrics.memory_usage_mb as f64,
            "disk_usage" => metrics.disk_usage_percent,
            "latency_p95" => metrics.latency_p95.as_millis() as f64,
            _ => return Ok(false),
        };

        Ok(value > config.threshold)
    }

    async fn check_availability_alert(
        &self,
        config: &AlertConfig,
        health: &SystemHealth,
    ) -> Result<bool> {
        if let Some(component_health) = health.components.get(&config.component) {
            Ok(matches!(component_health.status, super::config::HealthStatus::Unhealthy(_)))
        } else {
            Ok(true) // Component not found is an availability issue
        }
    }

    async fn check_performance_alert(
        &self,
        config: &AlertConfig,
        metrics: &PerformanceMetrics,
    ) -> Result<bool> {
        // Check if performance has degraded significantly
        let performance_score =
            1.0 - metrics.error_rate - (metrics.cpu_usage_percent / 100.0) * 0.3;
        Ok(performance_score < config.threshold)
    }

    async fn check_anomaly_alert(
        &self,
        _config: &AlertConfig,
        _metrics: &PerformanceMetrics,
    ) -> Result<bool> {
        // Placeholder for anomaly detection
        // In a real implementation, this would use statistical analysis
        Ok(false)
    }

    fn determine_severity(
        &self,
        config: &AlertConfig,
        metrics: &PerformanceMetrics,
        health: &SystemHealth,
    ) -> AlertSeverity {
        match config.alert_type {
            AlertType::Availability => {
                if health.unhealthy_components > 0 {
                    AlertSeverity::Critical
                } else {
                    AlertSeverity::Warning
                }
            }
            AlertType::Threshold => {
                if metrics.error_rate > 0.1 || metrics.cpu_usage_percent > 90.0 {
                    AlertSeverity::Critical
                } else {
                    AlertSeverity::Warning
                }
            }
            _ => AlertSeverity::Info,
        }
    }

    fn generate_alert_message(
        &self,
        config: &AlertConfig,
        metrics: &PerformanceMetrics,
        _health: &SystemHealth,
    ) -> String {
        match config.alert_type {
            AlertType::Threshold => {
                format!(
                    "Threshold alert for {} on {:?}: {} exceeded threshold {}",
                    config.metric_name,
                    config.component,
                    self.get_metric_value(&config.metric_name, metrics),
                    config.threshold
                )
            }
            AlertType::Availability => {
                format!(
                    "Availability alert for {:?}: Component is unhealthy",
                    config.component
                )
            }
            AlertType::PerformanceDegradation => {
                format!(
                    "Performance degradation detected for {:?}",
                    config.component
                )
            }
            AlertType::Anomaly => {
                format!("Anomaly detected for {:?}", config.component)
            }
        }
    }

    fn get_metric_value(&self, metric_name: &str, metrics: &PerformanceMetrics) -> String {
        match metric_name {
            "error_rate" => format!("{:.2}%", metrics.error_rate * 100.0),
            "cpu_usage" => format!("{:.1}%", metrics.cpu_usage_percent),
            "memory_usage" => format!("{} MB", metrics.memory_usage_mb),
            "disk_usage" => format!("{:.1}%", metrics.disk_usage_percent),
            "latency_p95" => format!("{} ms", metrics.latency_p95.as_millis()),
            _ => "unknown".to_string(),
        }
    }
}

impl Default for AlertManager {
    fn default() -> Self {
        Self::new()
    }
}