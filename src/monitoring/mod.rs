//! Monitoring module for comprehensive system observability
//! 
//! This module provides health monitoring, metrics collection, and alerting
//! capabilities for the autonomous trading platform.

pub mod health;

pub use health::{
    HealthMonitor,
    ComponentType,
    ComponentHealth,
    SystemHealth,
    HealthStatus,
    PerformanceMetrics,
    AlertConfig,
    AlertType,
    Alert,
    AlertSeverity,
    MetricsCollector,
    AlertManager,
    HealthEndpoints,
};