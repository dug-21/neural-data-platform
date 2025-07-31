//! Monitoring module for comprehensive system observability
//!
//! This module provides health monitoring, metrics collection, and alerting
//! capabilities for the autonomous trading platform.

pub mod health;
pub mod resource_health_integration;

#[cfg(test)]
mod test_health;

pub use health::{
    Alert, AlertConfig, AlertManager, AlertSeverity, AlertType, ComponentHealth, ComponentType,
    HealthEndpoints, HealthMonitor, HealthStatus, MetricsCollector, PerformanceMetrics,
    SystemHealth,
};

pub use resource_health_integration::{
    ResourceHealthIntegration, HealthMonitorResourceExt,
};
