use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use crate::error::Result;
use crate::integrations::monitor::MonitorClient;
use crate::models::{ComponentHealth, PerformanceMetrics, SystemHealth};

#[derive(Debug, Clone)]
pub struct HealthMonitorTool {
    monitor: Arc<MonitorClient>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "operation")]
pub enum HealthRequest {
    CheckHealth,
    GetMetrics,
    CheckService { service: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HealthResponse {
    SystemHealth(SystemHealth),
    PerformanceMetrics(PerformanceMetrics),
    ComponentHealth(ComponentHealth),
}

impl HealthMonitorTool {
    pub fn new(monitor: Arc<MonitorClient>) -> Self {
        Self { monitor }
    }

    pub async fn execute(&self, request: HealthRequest) -> Result<HealthResponse> {
        match request {
            HealthRequest::CheckHealth => {
                info!("Checking system health");
                let status = self.monitor.get_system_health().await?;
                Ok(HealthResponse::SystemHealth(status))
            }
            HealthRequest::GetMetrics => {
                info!("Getting system metrics");
                let metrics = self.monitor.get_performance_metrics("1m").await?;
                Ok(HealthResponse::PerformanceMetrics(metrics))
            }
            HealthRequest::CheckService { service } => {
                info!("Checking health for service: {}", service);
                let health = self.monitor.get_component_health(&service).await?;
                Ok(HealthResponse::ComponentHealth(health))
            }
        }
    }
}
