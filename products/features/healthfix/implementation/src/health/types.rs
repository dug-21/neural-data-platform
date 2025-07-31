//! Core types and traits for the health monitoring system

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Component types in the system
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum ComponentType {
    Database,
    Redis,
    NeuralSystem,
    DAAOrchestrator,
    Custom(String),
}

impl ComponentType {
    pub fn as_str(&self) -> &str {
        match self {
            ComponentType::Database => "database",
            ComponentType::Redis => "redis",
            ComponentType::NeuralSystem => "neural_system",
            ComponentType::DAAOrchestrator => "daa_orchestrator",
            ComponentType::Custom(name) => name,
        }
    }
}

impl std::fmt::Display for ComponentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Health status of a component
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Component is functioning normally
    Healthy,
    /// Component is functioning but with issues
    Degraded,
    /// Component is not functioning
    Unhealthy,
    /// Health status is unknown
    Unknown,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Degraded => write!(f, "degraded"),
            HealthStatus::Unhealthy => write!(f, "unhealthy"),
            HealthStatus::Unknown => write!(f, "unknown"),
        }
    }
}

/// Health check result for a component
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub component_type: ComponentType,
    pub is_healthy: bool,
    pub response_time_ms: Option<u64>,
    pub error_message: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// Health information for a component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub component_type: ComponentType,
    pub status: HealthStatus,
    pub last_check: Instant,
    pub response_time_ms: Option<u64>,
    pub error_message: Option<String>,
    pub consecutive_failures: u32,
    pub metadata: HashMap<String, String>,
}

/// System-wide health information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemHealth {
    pub status: String,
    pub total_components: usize,
    pub healthy_components: usize,
    pub degraded_components: usize,
    pub unhealthy_components: usize,
    pub health_score: f64,
    pub system_uptime: Duration,
    pub timestamp: std::time::SystemTime,
}

/// Configuration for health monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMonitorConfig {
    /// Interval between health checks
    pub check_interval: Duration,
    /// Timeout for individual health checks
    pub check_timeout: Duration,
    /// Number of health check results to keep in history
    pub history_size: usize,
    /// Number of consecutive failures before marking unhealthy
    pub unhealthy_threshold: u32,
    /// Number of consecutive successes before marking healthy
    pub recovery_threshold: u32,
}

impl Default for HealthMonitorConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(30),
            check_timeout: Duration::from_secs(5),
            history_size: 100,
            unhealthy_threshold: 3,
            recovery_threshold: 2,
        }
    }
}

/// Trait for components that can be health checked
#[async_trait]
pub trait HealthChecker: Send + Sync {
    /// Perform a health check
    async fn check_health(&self) -> Result<HealthCheckResult>;
    
    /// Get the component type this checker is for
    fn component_type(&self) -> ComponentType;
}

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitBreakerState {
    /// Circuit is closed, requests are allowed
    Closed,
    /// Circuit is open, requests are blocked
    Open,
    /// Circuit is half-open, limited requests allowed for testing
    HalfOpen,
}

/// Circuit breaker for preventing cascading failures
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    pub state: CircuitBreakerState,
    pub failure_count: u32,
    pub last_failure_time: Option<Instant>,
    pub success_count: u32,
    pub config: CircuitBreakerConfig,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub recovery_timeout: Duration,
    pub success_threshold: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            recovery_timeout: Duration::from_secs(30),
            success_threshold: 2,
        }
    }
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: CircuitBreakerState::Closed,
            failure_count: 0,
            last_failure_time: None,
            success_count: 0,
            config,
        }
    }

    pub fn record_success(&mut self) {
        self.success_count += 1;
        self.failure_count = 0;

        match self.state {
            CircuitBreakerState::HalfOpen => {
                if self.success_count >= self.config.success_threshold {
                    self.state = CircuitBreakerState::Closed;
                    self.success_count = 0;
                }
            }
            _ => {}
        }
    }

    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.success_count = 0;
        self.last_failure_time = Some(Instant::now());

        if self.failure_count >= self.config.failure_threshold {
            self.state = CircuitBreakerState::Open;
        }
    }

    pub fn can_execute(&mut self) -> bool {
        match self.state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                if let Some(last_failure) = self.last_failure_time {
                    if last_failure.elapsed() >= self.config.recovery_timeout {
                        self.state = CircuitBreakerState::HalfOpen;
                        self.failure_count = 0;
                        self.success_count = 0;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitBreakerState::HalfOpen => true,
        }
    }
}

/// Health endpoint response types
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: String,
    pub system_uptime: String,
    pub components: HashMap<String, ComponentHealthInfo>,
    pub metrics: HealthMetrics,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComponentHealthInfo {
    pub status: String,
    pub response_time_ms: Option<u64>,
    pub last_check: String,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthMetrics {
    pub total_components: usize,
    pub healthy_components: usize,
    pub degraded_components: usize,
    pub unhealthy_components: usize,
    pub health_score: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LivenessResponse {
    pub status: String,
    pub timestamp: String,
    pub uptime: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadinessResponse {
    pub status: String,
    pub timestamp: String,
    pub critical_components: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_transitions() {
        let mut cb = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(100),
            success_threshold: 2,
        });

        // Initially closed
        assert_eq!(cb.state, CircuitBreakerState::Closed);
        assert!(cb.can_execute());

        // Record failures to open circuit
        cb.record_failure();
        assert_eq!(cb.state, CircuitBreakerState::Closed);
        cb.record_failure();
        assert_eq!(cb.state, CircuitBreakerState::Open);
        assert!(!cb.can_execute());

        // Wait for recovery timeout
        std::thread::sleep(Duration::from_millis(150));
        assert!(cb.can_execute());
        assert_eq!(cb.state, CircuitBreakerState::HalfOpen);

        // Success in half-open moves to closed
        cb.record_success();
        cb.record_success();
        assert_eq!(cb.state, CircuitBreakerState::Closed);
    }
}