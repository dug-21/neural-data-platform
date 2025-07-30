//! Health Monitoring Configuration
//!
//! Configuration structures and types for the health monitoring system.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Component types in the autonomous platform
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ComponentType {
    Database,
    Redis,
    Streaming,
    DAAOrchestrator,
    NeuralSystem,
    EventBus,
    DataPipeline,
    Cache,
}

/// Health status of a component
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded(String),
    Unhealthy(String),
    Unknown,
}

/// Alert configuration for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    pub id: String,
    pub component: ComponentType,
    pub metric_name: String,
    pub threshold: f64,
    pub alert_type: AlertType,
    pub enabled: bool,
    pub cooldown_minutes: u32,
}

/// Types of alerts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    Threshold,
    Anomaly,
    Availability,
    PerformanceDegradation,
}

/// Alert severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Critical,
    Warning,
    Info,
}

/// Detailed health information for a component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub component_type: ComponentType,
    pub status: HealthStatus,
    pub last_check: DateTime<Utc>,
    pub response_time_ms: Option<u64>,
    pub error_message: Option<String>,
    pub metadata: HashMap<String, String>,
    pub uptime: Duration,
    pub last_restart: Option<DateTime<Utc>>,
}

/// Overall system health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub overall_status: HealthStatus,
    pub components: HashMap<ComponentType, ComponentHealth>,
    pub timestamp: DateTime<Utc>,
    pub system_uptime: Duration,
    pub total_components: usize,
    pub healthy_components: usize,
    pub degraded_components: usize,
    pub unhealthy_components: usize,
}

/// Performance metrics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub latency_p50: Duration,
    pub latency_p95: Duration,
    pub latency_p99: Duration,
    pub throughput_per_sec: f64,
    pub error_rate: f64,
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: u64,
    pub disk_usage_percent: f64,
    pub network_bytes_in: u64,
    pub network_bytes_out: u64,
    pub timestamp: DateTime<Utc>,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "Healthy"),
            HealthStatus::Degraded(reason) => write!(f, "Degraded: {}", reason),
            HealthStatus::Unhealthy(reason) => write!(f, "Unhealthy: {}", reason),
            HealthStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

impl std::fmt::Display for ComponentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComponentType::Database => write!(f, "Database"),
            ComponentType::Redis => write!(f, "Redis"),
            ComponentType::Streaming => write!(f, "Streaming"),
            ComponentType::DAAOrchestrator => write!(f, "DAA Orchestrator"),
            ComponentType::NeuralSystem => write!(f, "Neural System"),
            ComponentType::EventBus => write!(f, "Event Bus"),
            ComponentType::DataPipeline => write!(f, "Data Pipeline"),
            ComponentType::Cache => write!(f, "Cache"),
        }
    }
}

impl std::fmt::Display for AlertSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertSeverity::Critical => write!(f, "Critical"),
            AlertSeverity::Warning => write!(f, "Warning"),
            AlertSeverity::Info => write!(f, "Info"),
        }
    }
}