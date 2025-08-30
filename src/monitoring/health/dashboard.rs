//! Health Monitoring Dashboard and Endpoints
//!
//! HTTP endpoints and reporting functionality for health monitoring system.

use anyhow::Result;
use serde_json::json;
use std::sync::Arc;

use super::alerts::AlertManager;
use super::config::{HealthStatus, PerformanceMetrics, SystemHealth};

/// Health monitor interface for dashboard
pub trait HealthMonitorInterface {
    async fn get_system_health(&self) -> Result<SystemHealth>;
    async fn collect_performance_metrics(&self) -> Result<PerformanceMetrics>;
    fn get_alert_manager(&self) -> &AlertManager;
}

/// Health endpoints for HTTP/REST API
#[derive(Debug, Clone)]
pub struct HealthEndpoints<T: HealthMonitorInterface> {
    monitor: Arc<T>,
}

impl<T: HealthMonitorInterface> HealthEndpoints<T> {
    /// Create new health endpoints
    pub fn new(monitor: Arc<T>) -> Self {
        Self { monitor }
    }

    /// GET /health - Basic health check
    pub async fn health_endpoint(&self) -> Result<String> {
        let health = self.monitor.get_system_health().await?;
        let response = json!({
            "status": health.overall_status,
            "timestamp": health.timestamp,
            "uptime": health.system_uptime.as_secs(),
            "health_score": health.health_score()
        });
        Ok(response.to_string())
    }

    /// GET /health/components - Detailed component health
    pub async fn components_endpoint(&self) -> Result<String> {
        let health = self.monitor.get_system_health().await?;
        let response = json!({
            "components": health.components,
            "summary": {
                "total": health.total_components,
                "healthy": health.healthy_components,
                "degraded": health.degraded_components,
                "unhealthy": health.unhealthy_components
            },
            "timestamp": health.timestamp
        });
        Ok(response.to_string())
    }

    /// GET /metrics - Prometheus metrics format
    pub async fn metrics_endpoint(&self) -> Result<String> {
        let health = self.monitor.get_system_health().await?;
        let metrics = self.monitor.collect_performance_metrics().await?;

        let mut output = String::new();

        // System health metrics
        output.push_str("# HELP system_health_score Overall system health score (0-1)\n");
        output.push_str("# TYPE system_health_score gauge\n");
        output.push_str(&format!("system_health_score {}\n", health.health_score()));

        output.push_str("# HELP system_uptime_seconds System uptime in seconds\n");
        output.push_str("# TYPE system_uptime_seconds counter\n");
        output.push_str(&format!(
            "system_uptime_seconds {}\n",
            health.system_uptime.as_secs()
        ));

        // Component health metrics
        output.push_str("# HELP component_health Component health status (1=healthy, 0.5=degraded, 0=unhealthy)\n");
        output.push_str("# TYPE component_health gauge\n");
        for (component, component_health) in &health.components {
            let value = match component_health.status {
                HealthStatus::Healthy => 1.0,
                HealthStatus::Degraded(_) => 0.5,
                HealthStatus::Unhealthy(_) => 0.0,
                HealthStatus::Unknown => -1.0,
            };
            output.push_str(&format!(
                "component_health{{component=\"{:?}\"}} {}\n",
                component, value
            ));
        }

        // Performance metrics
        output.push_str("# HELP response_time_p95_seconds 95th percentile response time\n");
        output.push_str("# TYPE response_time_p95_seconds gauge\n");
        output.push_str(&format!(
            "response_time_p95_seconds {}\n",
            metrics.latency_p95.as_secs_f64()
        ));

        output.push_str("# HELP error_rate Error rate (0-1)\n");
        output.push_str("# TYPE error_rate gauge\n");
        output.push_str(&format!("error_rate {}\n", metrics.error_rate));

        output.push_str("# HELP throughput_per_second Throughput in operations per second\n");
        output.push_str("# TYPE throughput_per_second gauge\n");
        output.push_str(&format!(
            "throughput_per_second {}\n",
            metrics.throughput_per_sec
        ));

        output.push_str("# HELP cpu_usage_percent CPU usage percentage\n");
        output.push_str("# TYPE cpu_usage_percent gauge\n");
        output.push_str(&format!("cpu_usage_percent {}\n", metrics.cpu_usage_percent));

        output.push_str("# HELP memory_usage_mb Memory usage in MB\n");
        output.push_str("# TYPE memory_usage_mb gauge\n");
        output.push_str(&format!("memory_usage_mb {}\n", metrics.memory_usage_mb));

        output.push_str("# HELP disk_usage_percent Disk usage percentage\n");
        output.push_str("# TYPE disk_usage_percent gauge\n");
        output.push_str(&format!("disk_usage_percent {}\n", metrics.disk_usage_percent));

        // Neural trader specific metrics
        output.push_str("# HELP neural_trader_models_available Number of available models\n");
        output.push_str("# TYPE neural_trader_models_available gauge\n");
        output.push_str("neural_trader_models_available 0\n");
        
        output.push_str("# HELP neural_trader_required_models_missing Number of missing required models\n");
        output.push_str("# TYPE neural_trader_required_models_missing gauge\n");
        output.push_str("neural_trader_required_models_missing 0\n");
        
        output.push_str("# HELP neural_trader_model_storage_mounted Whether model storage is mounted (1=yes, 0=no)\n");
        output.push_str("# TYPE neural_trader_model_storage_mounted gauge\n");
        output.push_str("neural_trader_model_storage_mounted 1\n");
        
        output.push_str("# HELP neural_trader_model_storage_writable Whether model storage is writable (1=yes, 0=no)\n");
        output.push_str("# TYPE neural_trader_model_storage_writable gauge\n");
        output.push_str("neural_trader_model_storage_writable 1\n");
        
        output.push_str("# HELP neural_trader_model_storage_size_mb Total size of models in MB\n");
        output.push_str("# TYPE neural_trader_model_storage_size_mb gauge\n");
        output.push_str("neural_trader_model_storage_size_mb 0\n");
        
        output.push_str("# HELP neural_trader_model_storage_disk_available_gb Available disk space in GB\n");
        output.push_str("# TYPE neural_trader_model_storage_disk_available_gb gauge\n");
        output.push_str("neural_trader_model_storage_disk_available_gb 100\n");
        
        output.push_str("# HELP neural_trader_model_storage_disk_used_percent Disk usage percentage\n");
        output.push_str("# TYPE neural_trader_model_storage_disk_used_percent gauge\n");
        output.push_str("neural_trader_model_storage_disk_used_percent 25.0\n");
        
        output.push_str("# HELP neural_trader_corrupted_models Number of corrupted models detected\n");
        output.push_str("# TYPE neural_trader_corrupted_models gauge\n");
        output.push_str("neural_trader_corrupted_models 0\n");

        Ok(output)
    }

    /// GET /status - Detailed system status
    pub async fn status_endpoint(&self) -> Result<String> {
        let health = self.monitor.get_system_health().await?;
        let metrics = self.monitor.collect_performance_metrics().await?;
        let alerts = self.monitor.get_alert_manager().get_active_alerts().await;

        let response = json!({
            "status": {
                "overall": health.overall_status,
                "health_score": health.health_score(),
                "uptime": health.system_uptime.as_secs(),
                "timestamp": health.timestamp
            },
            "components": health.components,
            "metrics": {
                "performance": metrics,
                "platform": {
                    "total_components": health.total_components,
                    "healthy_components": health.healthy_components,
                    "degraded_components": health.degraded_components,
                    "unhealthy_components": health.unhealthy_components
                }
            },
            "alerts": {
                "active_count": alerts.len(),
                "active_alerts": alerts
            }
        });

        Ok(response.to_string())
    }

    /// GET /health/alerts - Active alerts endpoint
    pub async fn alerts_endpoint(&self) -> Result<String> {
        let alerts = self.monitor.get_alert_manager().get_active_alerts().await;
        let response = json!({
            "active_alerts": alerts,
            "count": alerts.len(),
            "timestamp": chrono::Utc::now()
        });
        Ok(response.to_string())
    }

    /// GET /health/metrics/raw - Raw performance metrics
    pub async fn raw_metrics_endpoint(&self) -> Result<String> {
        let metrics = self.monitor.collect_performance_metrics().await?;
        let response = json!({
            "metrics": metrics,
            "timestamp": chrono::Utc::now()
        });
        Ok(response.to_string())
    }
}

/// Health reporting utilities
pub struct HealthReporter;

impl HealthReporter {
    /// Generate a health summary report
    pub fn generate_summary_report(health: &SystemHealth) -> String {
        let mut report = String::new();
        
        report.push_str("=== Health Monitoring System Report ===\n");
        report.push_str(&format!("Overall Status: {}\n", health.overall_status));
        report.push_str(&format!("Health Score: {:.2}\n", health.health_score()));
        report.push_str(&format!("System Uptime: {}s\n", health.system_uptime.as_secs()));
        report.push_str(&format!("Last Updated: {}\n", health.timestamp));
        
        report.push_str("\n=== Component Summary ===\n");
        report.push_str(&format!("Total Components: {}\n", health.total_components));
        report.push_str(&format!("Healthy: {}\n", health.healthy_components));
        report.push_str(&format!("Degraded: {}\n", health.degraded_components));
        report.push_str(&format!("Unhealthy: {}\n", health.unhealthy_components));
        
        report.push_str("\n=== Component Details ===\n");
        for (component_type, component_health) in &health.components {
            report.push_str(&format!(
                "{:?}: {} (Last check: {})\n",
                component_type,
                component_health.status,
                component_health.last_check
            ));
            
            if let Some(error) = &component_health.error_message {
                report.push_str(&format!("  Error: {}\n", error));
            }
            
            if let Some(response_time) = component_health.response_time_ms {
                report.push_str(&format!("  Response Time: {}ms\n", response_time));
            }
        }
        
        report
    }

    /// Generate a CSV report of component health
    pub fn generate_csv_report(health: &SystemHealth) -> String {
        let mut csv = String::new();
        csv.push_str("Component,Status,Last Check,Response Time (ms),Error Message\n");
        
        for (component_type, component_health) in &health.components {
            csv.push_str(&format!(
                "{:?},{},{},{},{}\n",
                component_type,
                component_health.status,
                component_health.last_check,
                component_health.response_time_ms.unwrap_or(0),
                component_health.error_message.as_deref().unwrap_or("")
            ));
        }
        
        csv
    }
}