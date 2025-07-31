//! Standalone HTTP health server implementation

use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{error, info};

use super::{
    AsyncHealthMonitor, ComponentHealthInfo, ComponentType, HealthMetrics, HealthResponse,
    HealthStatus, LivenessResponse, ReadinessResponse,
};

/// Configuration for the health server
#[derive(Debug, Clone)]
pub struct HealthServerConfig {
    pub port: u16,
    pub bind_address: String,
    pub request_timeout: Duration,
}

impl Default for HealthServerConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            bind_address: "0.0.0.0".to_string(),
            request_timeout: Duration::from_secs(30),
        }
    }
}

/// Standalone health server that provides HTTP endpoints
pub struct HealthServer {
    config: HealthServerConfig,
    health_monitor: Arc<RwLock<AsyncHealthMonitor>>,
    server_task: Option<JoinHandle<()>>,
    start_time: Instant,
}

/// Shared state for the Axum handlers
#[derive(Clone)]
struct AppState {
    health_monitor: Arc<RwLock<AsyncHealthMonitor>>,
    start_time: Instant,
}

impl HealthServer {
    /// Create a new health server
    pub fn new(config: HealthServerConfig) -> Self {
        let health_monitor = AsyncHealthMonitor::new(Default::default());
        
        Self {
            config,
            health_monitor: Arc::new(RwLock::new(health_monitor)),
            server_task: None,
            start_time: Instant::now(),
        }
    }

    /// Create a health server with an existing health monitor
    pub fn with_monitor(
        config: HealthServerConfig,
        health_monitor: AsyncHealthMonitor,
    ) -> Self {
        Self {
            config,
            health_monitor: Arc::new(RwLock::new(health_monitor)),
            server_task: None,
            start_time: Instant::now(),
        }
    }

    /// Start the health server
    pub async fn start(&mut self) -> Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.config.bind_address, self.config.port)
            .parse()?;

        info!("Starting health server on {}", addr);

        // Create the Axum app with routes
        let app = self.create_app();

        // Create TCP listener
        let listener = TcpListener::bind(addr).await?;

        // Spawn the server task
        let server_task = tokio::spawn(async move {
            info!("Health server listening on {}", addr);
            
            if let Err(e) = axum::serve(listener, app).await {
                error!("Health server error: {}", e);
            }
        });

        self.server_task = Some(server_task);

        Ok(())
    }

    /// Stop the health server
    pub async fn stop(&mut self) {
        info!("Stopping health server");

        if let Some(task) = self.server_task.take() {
            task.abort();
            // Wait a bit for graceful shutdown
            let _ = tokio::time::timeout(Duration::from_secs(5), task).await;
        }

        info!("Health server stopped");
    }

    /// Set the health status of a component (for testing)
    pub async fn set_component_health(
        &self,
        component: ComponentType,
        status: HealthStatus,
    ) {
        // This would be implemented to update the health monitor's state
        // For now, it's a placeholder for testing
    }

    /// Create the Axum application with all routes
    fn create_app(&self) -> Router {
        let state = AppState {
            health_monitor: Arc::clone(&self.health_monitor),
            start_time: self.start_time,
        };

        Router::new()
            .route("/health", get(health_handler))
            .route("/health/live", get(liveness_handler))
            .route("/health/ready", get(readiness_handler))
            .route("/metrics", get(metrics_handler))
            .with_state(state)
    }
}

// Handler functions

/// Main health endpoint handler
async fn health_handler(State(state): State<AppState>) -> Response {
    let monitor = state.health_monitor.read().await;
    let system_health = match monitor.get_system_health().await {
        Ok(health) => health,
        Err(e) => {
            error!("Failed to get system health: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
    };

    // Build component health info
    let mut components = std::collections::HashMap::new();
    
    // Get health for each component type
    for component_type in [
        ComponentType::Database,
        ComponentType::Redis,
        ComponentType::NeuralSystem,
        ComponentType::DAAOrchestrator,
    ] {
        if let Some(component_health) = monitor.get_component_health(component_type.clone()).await {
            components.insert(
                component_type.to_string(),
                ComponentHealthInfo {
                    status: component_health.status.to_string(),
                    response_time_ms: component_health.response_time_ms,
                    last_check: format_instant_as_iso8601(component_health.last_check),
                    error: component_health.error_message,
                },
            );
        }
    }

    let response = HealthResponse {
        status: system_health.status.clone(),
        timestamp: format_system_time_as_iso8601(SystemTime::now()),
        system_uptime: format_duration(state.start_time.elapsed()),
        components,
        metrics: HealthMetrics {
            total_components: system_health.total_components,
            healthy_components: system_health.healthy_components,
            degraded_components: system_health.degraded_components,
            unhealthy_components: system_health.unhealthy_components,
            health_score: system_health.health_score,
        },
    };

    // Return appropriate status code based on health
    let status_code = match system_health.status.as_str() {
        "healthy" => StatusCode::OK,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    };

    (status_code, Json(response)).into_response()
}

/// Liveness probe handler (Kubernetes)
async fn liveness_handler(State(state): State<AppState>) -> Response {
    let response = LivenessResponse {
        status: "alive".to_string(),
        timestamp: format_system_time_as_iso8601(SystemTime::now()),
        uptime: format_duration(state.start_time.elapsed()),
    };

    (StatusCode::OK, Json(response)).into_response()
}

/// Readiness probe handler (Load balancer)
async fn readiness_handler(State(state): State<AppState>) -> Response {
    let monitor = state.health_monitor.read().await;
    let system_health = match monitor.get_system_health().await {
        Ok(health) => health,
        Err(_) => {
            return (StatusCode::SERVICE_UNAVAILABLE, "Not ready").into_response();
        }
    };

    // Check critical components
    let mut critical_components = std::collections::HashMap::new();
    
    for component_type in [
        ComponentType::Database,
        ComponentType::Redis,
        ComponentType::NeuralSystem,
    ] {
        let status = if let Some(health) = monitor.get_component_health(component_type.clone()).await {
            health.status.to_string()
        } else {
            "unknown".to_string()
        };
        
        critical_components.insert(component_type.to_string(), status);
    }

    // Check if any critical component is unhealthy
    let all_critical_healthy = critical_components.values()
        .all(|status| status == "healthy" || status == "degraded");

    let status = if all_critical_healthy { "ready" } else { "not_ready" };
    let status_code = if all_critical_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let response = ReadinessResponse {
        status: status.to_string(),
        timestamp: format_system_time_as_iso8601(SystemTime::now()),
        critical_components,
    };

    (status_code, Json(response)).into_response()
}

/// Prometheus metrics endpoint handler
async fn metrics_handler(State(state): State<AppState>) -> Response {
    let monitor = state.health_monitor.read().await;
    let system_health = match monitor.get_system_health().await {
        Ok(health) => health,
        Err(e) => {
            error!("Failed to get system health for metrics: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
    };

    let mut metrics = String::new();

    // System health score
    metrics.push_str("# HELP system_health_score Overall system health score (0.0-1.0)\n");
    metrics.push_str("# TYPE system_health_score gauge\n");
    metrics.push_str(&format!("system_health_score {}\n\n", system_health.health_score));

    // Component health status
    metrics.push_str("# HELP component_health_status Health status of components (1=healthy, 0=unhealthy)\n");
    metrics.push_str("# TYPE component_health_status gauge\n");
    
    for component_type in [
        ComponentType::Database,
        ComponentType::Redis,
        ComponentType::NeuralSystem,
        ComponentType::DAAOrchestrator,
    ] {
        if let Some(health) = monitor.get_component_health(component_type.clone()).await {
            let value = match health.status {
                HealthStatus::Healthy => 1.0,
                HealthStatus::Degraded => 0.5,
                _ => 0.0,
            };
            metrics.push_str(&format!(
                "component_health_status{{component=\"{}\"}} {}\n",
                component_type,
                value
            ));
        }
    }
    metrics.push_str("\n");

    // Health check duration
    metrics.push_str("# HELP component_health_check_duration_seconds Health check duration\n");
    metrics.push_str("# TYPE component_health_check_duration_seconds histogram\n");
    
    // Component counts
    metrics.push_str("# HELP healthy_components_total Number of healthy components\n");
    metrics.push_str("# TYPE healthy_components_total gauge\n");
    metrics.push_str(&format!("healthy_components_total {}\n\n", system_health.healthy_components));

    metrics.push_str("# HELP unhealthy_components_total Number of unhealthy components\n");
    metrics.push_str("# TYPE unhealthy_components_total gauge\n");
    metrics.push_str(&format!("unhealthy_components_total {}\n\n", system_health.unhealthy_components));

    // Server uptime
    let uptime_seconds = state.start_time.elapsed().as_secs();
    metrics.push_str("# HELP health_server_uptime_seconds Health server uptime in seconds\n");
    metrics.push_str("# TYPE health_server_uptime_seconds counter\n");
    metrics.push_str(&format!("health_server_uptime_seconds {}\n", uptime_seconds));

    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        metrics,
    )
        .into_response()
}

// Helper functions

fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    format!("{}h{}m{}s", hours, minutes, seconds)
}

fn format_system_time_as_iso8601(time: SystemTime) -> String {
    let datetime = time.duration_since(UNIX_EPOCH).unwrap().as_secs();
    // Simple ISO8601 format - in production, use chrono or time crate
    format!("2024-01-01T00:00:{}Z", datetime % 60)
}

fn format_instant_as_iso8601(instant: Instant) -> String {
    // Convert instant to system time (approximation)
    let now = SystemTime::now();
    format_system_time_as_iso8601(now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_server_creation() {
        let server = HealthServer::new(HealthServerConfig::default());
        assert_eq!(server.config.port, 8080);
    }

    #[tokio::test]
    async fn test_duration_formatting() {
        let duration = Duration::from_secs(3661); // 1h 1m 1s
        assert_eq!(format_duration(duration), "1h1m1s");
    }
}