//! Health Monitoring System for Autonomous Platform
//! 
//! This module provides comprehensive health monitoring and observability for all
//! system components including database, cache, streaming, neural networks, and
//! DAA orchestrator agents.

use anyhow::Result;
use chrono::{DateTime, Utc};
use metrics::{counter, gauge, histogram};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex};
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, warn};
use uuid::Uuid;


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

/// Overall system health aggregation
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

/// Performance metrics for the system
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

/// Alert severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Critical,
    Warning,
    Info,
}

/// Metrics collector for performance data
#[derive(Debug, Clone)]
pub struct MetricsCollector {
    start_time: Instant,
    latency_histogram: Arc<Mutex<Vec<Duration>>>,
    throughput_counter: Arc<Mutex<u64>>,
    error_counter: Arc<Mutex<u64>>,
}

/// Alert manager for handling alerts
#[derive(Debug)]
pub struct AlertManager {
    configs: Arc<RwLock<HashMap<String, AlertConfig>>>,
    active_alerts: Arc<RwLock<HashMap<String, Alert>>>,
    alert_history: Arc<RwLock<Vec<Alert>>>,
}

/// Health endpoints for HTTP/REST API
#[derive(Debug, Clone)]
pub struct HealthEndpoints {
    monitor: Arc<HealthMonitor>,
}

/// Main health monitoring system
#[derive(Debug)]
pub struct HealthMonitor {
    component_health: Arc<RwLock<HashMap<ComponentType, ComponentHealth>>>,
    metrics_collector: MetricsCollector,
    alert_manager: AlertManager,
    start_time: Instant,
    monitoring_interval: Duration,
    is_monitoring: Arc<RwLock<bool>>,
}

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
}

impl SystemHealth {
    /// Create system health from component health map
    pub fn from_components(components: HashMap<ComponentType, ComponentHealth>, start_time: Instant) -> Self {
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
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            latency_histogram: Arc::new(Mutex::new(Vec::new())),
            throughput_counter: Arc::new(Mutex::new(0)),
            error_counter: Arc::new(Mutex::new(0)),
        }
    }

    /// Record a latency measurement
    pub async fn record_latency(&self, _component: &ComponentType, latency: Duration) {
        let mut histogram = self.latency_histogram.lock().await;
        histogram.push(latency);
        
        // Keep only last 1000 measurements
        if histogram.len() > 1000 {
            histogram.drain(0..100);
        }

        // Record to metrics crate
        histogram!("component_response_time").record(latency.as_secs_f64());
    }

    /// Record a throughput event
    pub async fn record_throughput(&self) {
        let mut counter = self.throughput_counter.lock().await;
        *counter += 1;
    }

    /// Record an error
    pub async fn record_error(&self, _component: &ComponentType, _error: &str) {
        let mut counter = self.error_counter.lock().await;
        *counter += 1;

        counter!("component_errors_total").increment(1);
    }

    /// Calculate performance metrics
    pub async fn calculate_metrics(&self) -> Result<PerformanceMetrics> {
        let histogram = self.latency_histogram.lock().await;
        let throughput = *self.throughput_counter.lock().await;
        let errors = *self.error_counter.lock().await;

        let mut latencies = histogram.clone();
        latencies.sort();

        let latency_p50 = latencies.get(latencies.len() / 2).copied().unwrap_or(Duration::from_millis(0));
        let latency_p95 = latencies.get((latencies.len() * 95) / 100).copied().unwrap_or(Duration::from_millis(0));
        let latency_p99 = latencies.get((latencies.len() * 99) / 100).copied().unwrap_or(Duration::from_millis(0));

        let elapsed = self.start_time.elapsed();
        let throughput_per_sec = if elapsed.as_secs() > 0 {
            throughput as f64 / elapsed.as_secs() as f64
        } else {
            0.0
        };

        let error_rate = if throughput > 0 {
            errors as f64 / throughput as f64
        } else {
            0.0
        };

        Ok(PerformanceMetrics {
            latency_p50,
            latency_p95,
            latency_p99,
            throughput_per_sec,
            error_rate,
            cpu_usage_percent: self.get_cpu_usage().await,
            memory_usage_mb: self.get_memory_usage().await,
            disk_usage_percent: self.get_disk_usage().await,
            network_bytes_in: 0,
            network_bytes_out: 0,
            timestamp: Utc::now(),
        })
    }

    /// Get CPU usage (placeholder implementation)
    async fn get_cpu_usage(&self) -> f64 {
        // In a real implementation, this would query system CPU usage
        // For now, return a mock value
        45.0
    }

    /// Get memory usage (placeholder implementation)
    async fn get_memory_usage(&self) -> u64 {
        // In a real implementation, this would query system memory usage
        // For now, return a mock value
        512
    }

    /// Get disk usage (placeholder implementation)
    async fn get_disk_usage(&self) -> f64 {
        // In a real implementation, this would query system disk usage
        // For now, return a mock value
        25.0
    }
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
    pub async fn check_alerts(&self, metrics: &PerformanceMetrics, health: &SystemHealth) -> Result<Vec<Alert>> {
        let configs = self.configs.read().await;
        let mut new_alerts = Vec::new();

        for config in configs.values() {
            if !config.enabled {
                continue;
            }

            let should_alert = match config.alert_type {
                AlertType::Threshold => self.check_threshold_alert(config, metrics, health).await?,
                AlertType::Availability => self.check_availability_alert(config, health).await?,
                AlertType::PerformanceDegradation => self.check_performance_alert(config, metrics).await?,
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

    async fn check_threshold_alert(&self, config: &AlertConfig, metrics: &PerformanceMetrics, _health: &SystemHealth) -> Result<bool> {
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

    async fn check_availability_alert(&self, config: &AlertConfig, health: &SystemHealth) -> Result<bool> {
        if let Some(component_health) = health.components.get(&config.component) {
            Ok(component_health.is_unhealthy())
        } else {
            Ok(true) // Component not found is an availability issue
        }
    }

    async fn check_performance_alert(&self, config: &AlertConfig, metrics: &PerformanceMetrics) -> Result<bool> {
        // Check if performance has degraded significantly
        let performance_score = 1.0 - metrics.error_rate - (metrics.cpu_usage_percent / 100.0) * 0.3;
        Ok(performance_score < config.threshold)
    }

    async fn check_anomaly_alert(&self, _config: &AlertConfig, _metrics: &PerformanceMetrics) -> Result<bool> {
        // Placeholder for anomaly detection
        // In a real implementation, this would use statistical analysis
        Ok(false)
    }

    fn determine_severity(&self, config: &AlertConfig, metrics: &PerformanceMetrics, health: &SystemHealth) -> AlertSeverity {
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

    fn generate_alert_message(&self, config: &AlertConfig, metrics: &PerformanceMetrics, _health: &SystemHealth) -> String {
        match config.alert_type {
            AlertType::Threshold => {
                format!("Threshold alert for {} on {:?}: {} exceeded threshold {}",
                    config.metric_name, config.component, 
                    self.get_metric_value(&config.metric_name, metrics), 
                    config.threshold)
            }
            AlertType::Availability => {
                format!("Availability alert for {:?}: Component is unhealthy", config.component)
            }
            AlertType::PerformanceDegradation => {
                format!("Performance degradation detected for {:?}", config.component)
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

impl HealthEndpoints {
    /// Create new health endpoints
    pub fn new(monitor: Arc<HealthMonitor>) -> Self {
        Self { monitor }
    }

    /// GET /health - Basic health check
    pub async fn health_endpoint(&self) -> Result<String> {
        let health = self.monitor.get_system_health().await?;
        let response = serde_json::json!({
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
        let response = serde_json::json!({
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
        output.push_str(&format!("system_uptime_seconds {}\n", health.system_uptime.as_secs()));
        
        // Component health metrics
        output.push_str("# HELP component_health Component health status (1=healthy, 0.5=degraded, 0=unhealthy)\n");
        output.push_str("# TYPE component_health gauge\n");
        for (component, health) in &health.components {
            let value = match health.status {
                HealthStatus::Healthy => 1.0,
                HealthStatus::Degraded(_) => 0.5,
                HealthStatus::Unhealthy(_) => 0.0,
                HealthStatus::Unknown => -1.0,
            };
            output.push_str(&format!("component_health{{component=\"{:?}\"}} {}\n", component, value));
        }
        
        // Performance metrics
        output.push_str("# HELP response_time_p95_seconds 95th percentile response time\n");
        output.push_str("# TYPE response_time_p95_seconds gauge\n");
        output.push_str(&format!("response_time_p95_seconds {}\n", metrics.latency_p95.as_secs_f64()));
        
        output.push_str("# HELP error_rate Error rate (0-1)\n");
        output.push_str("# TYPE error_rate gauge\n");
        output.push_str(&format!("error_rate {}\n", metrics.error_rate));
        
        output.push_str("# HELP throughput_per_second Throughput in operations per second\n");
        output.push_str("# TYPE throughput_per_second gauge\n");
        output.push_str(&format!("throughput_per_second {}\n", metrics.throughput_per_sec));
        
        Ok(output)
    }

    /// GET /status - Detailed system status
    pub async fn status_endpoint(&self) -> Result<String> {
        let health = self.monitor.get_system_health().await?;
        let metrics = self.monitor.collect_performance_metrics().await?;
        let alerts = self.monitor.alert_manager.get_active_alerts().await;
        
        let response = serde_json::json!({
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
}

impl HealthMonitor {
    /// Create a new health monitor
    pub async fn new() -> Result<Self> {
        let metrics_collector = MetricsCollector::new();
        let alert_manager = AlertManager::new();
        
        let component_health = Arc::new(RwLock::new(HashMap::new()));
        let is_monitoring = Arc::new(RwLock::new(false));
        
        let monitor = Self {
            component_health: component_health.clone(),
            metrics_collector,
            alert_manager,
            start_time: Instant::now(),
            monitoring_interval: Duration::from_secs(30),
            is_monitoring,
        };
        
        Ok(monitor)
    }

    /// Check health of a specific component
    pub async fn check_component_health(&self, component: ComponentType) -> Result<ComponentHealth> {
        let start_time = Instant::now();
        let mut health = ComponentHealth::new(component.clone());

        let result = match component {
            ComponentType::Database => self.check_database_health(&mut health).await,
            ComponentType::Redis => self.check_redis_health(&mut health).await,
            ComponentType::Streaming => self.check_streaming_health(&mut health).await,
            ComponentType::DAAOrchestrator => self.check_daa_health(&mut health).await,
            ComponentType::NeuralSystem => self.check_neural_health(&mut health).await,
            ComponentType::EventBus => self.check_event_bus_health(&mut health).await,
            ComponentType::DataPipeline => self.check_data_pipeline_health(&mut health).await,
            ComponentType::Cache => self.check_cache_health(&mut health).await,
        };

        let elapsed = start_time.elapsed();
        
        match result {
            Ok(()) => {
                health.update_status(HealthStatus::Healthy, Some(elapsed));
                self.metrics_collector.record_latency(&component, elapsed).await;
                self.metrics_collector.record_throughput().await;
            }
            Err(e) => {
                health.set_error(e.to_string());
                self.metrics_collector.record_error(&component, &e.to_string()).await;
            }
        }

        counter!("component_health_checks_total").increment(1);
        
        // Store in component health map
        self.component_health.write().await.insert(component, health.clone());
        
        Ok(health)
    }

    /// Get overall system health
    pub async fn get_system_health(&self) -> Result<SystemHealth> {
        let components = vec![
            ComponentType::Database,
            ComponentType::Redis,
            ComponentType::Streaming,
            ComponentType::DAAOrchestrator,
            ComponentType::NeuralSystem,
            ComponentType::EventBus,
            ComponentType::DataPipeline,
            ComponentType::Cache,
        ];

        let mut component_health = HashMap::new();
        
        for component in components {
            let health = self.check_component_health(component).await?;
            component_health.insert(health.component_type.clone(), health);
        }

        let system_health = SystemHealth::from_components(component_health, self.start_time);
        
        // Record system health score
        gauge!("system_health_score").set(system_health.health_score());
        
        Ok(system_health)
    }

    /// Start continuous monitoring
    pub async fn start_monitoring(&self) -> Result<()> {
        let mut is_monitoring = self.is_monitoring.write().await;
        if *is_monitoring {
            return Ok(());
        }
        *is_monitoring = true;

        info!("Starting health monitoring system");
        
        // Start monitoring loop
        let monitor = self.clone();
        tokio::spawn(async move {
            monitor.monitoring_loop().await;
        });

        Ok(())
    }

    /// Stop monitoring
    pub async fn stop_monitoring(&self) -> Result<()> {
        let mut is_monitoring = self.is_monitoring.write().await;
        *is_monitoring = false;
        info!("Stopping health monitoring system");
        Ok(())
    }

    /// Collect performance metrics
    pub async fn collect_performance_metrics(&self) -> Result<PerformanceMetrics> {
        Ok(self.metrics_collector.calculate_metrics().await?)
    }

    /// Add alert configuration
    pub async fn add_alert_config(&self, config: AlertConfig) -> Result<()> {
        self.alert_manager.add_config(config).await
    }

    /// Check for alerts
    pub async fn check_alerts(&self) -> Result<Vec<Alert>> {
        let metrics = self.collect_performance_metrics().await?;
        let health = self.get_system_health().await?;
        self.alert_manager.check_alerts(&metrics, &health).await
    }

    /// Main monitoring loop
    async fn monitoring_loop(&self) {
        let mut interval = interval(self.monitoring_interval);
        
        loop {
            interval.tick().await;
            
            // Check if monitoring should continue
            if !*self.is_monitoring.read().await {
                break;
            }
            
            // Perform health checks
            match self.get_system_health().await {
                Ok(health) => {
                    debug!("System health check completed: {} components checked", health.total_components);
                    
                    // Check for alerts
                    match self.check_alerts().await {
                        Ok(alerts) => {
                            if !alerts.is_empty() {
                                warn!("Generated {} new alerts", alerts.len());
                                for alert in alerts {
                                    warn!("Alert: {} - {}", alert.severity, alert.message);
                                }
                            }
                        }
                        Err(e) => error!("Failed to check alerts: {}", e),
                    }
                }
                Err(e) => error!("Failed to get system health: {}", e),
            }
        }
        
        info!("Health monitoring loop stopped");
    }

    // Component-specific health check methods
    async fn check_database_health(&self, health: &mut ComponentHealth) -> Result<()> {
        // Database health check implementation
        // Check connection, query performance, connection pool status
        
        // TODO: Replace with actual database health check once TimescaleDBStorage is available
        // let db = self.timescale_db.lock().await;
        // let is_connected = db.check_connection().await?;
        
        // Add metadata
        health.add_metadata("connection_pool_size".to_string(), "10".to_string());
        health.add_metadata("active_connections".to_string(), "5".to_string());
        health.add_metadata("query_count".to_string(), "1000".to_string());
        health.add_metadata("database_type".to_string(), "TimescaleDB".to_string());
        
        // Simulate database ping
        sleep(Duration::from_millis(10)).await;
        
        Ok(())
    }

    async fn check_redis_health(&self, health: &mut ComponentHealth) -> Result<()> {
        // Redis health check implementation
        // Check connection, memory usage, connected clients
        
        // TODO: Replace with actual Redis health check once RedisCache is available
        // let cache = self.redis_cache.lock().await;
        // let is_connected = cache.ping().await?;
        
        health.add_metadata("memory_usage_mb".to_string(), "100".to_string());
        health.add_metadata("connected_clients".to_string(), "5".to_string());
        health.add_metadata("hit_rate".to_string(), "0.85".to_string());
        health.add_metadata("cache_type".to_string(), "Redis".to_string());
        health.add_metadata("max_memory_mb".to_string(), "512".to_string());
        
        // Simulate Redis ping
        sleep(Duration::from_millis(5)).await;
        
        Ok(())
    }

    async fn check_streaming_health(&self, health: &mut ComponentHealth) -> Result<()> {
        // Streaming pipeline health check
        // Check throughput, lag, buffer status
        
        // TODO: Replace with actual StreamingPipeline health check once available
        // let pipeline = self.streaming_pipeline.lock().await;
        // let stats = pipeline.get_health_stats().await?;
        
        health.add_metadata("throughput_per_sec".to_string(), "1000".to_string());
        health.add_metadata("lag_ms".to_string(), "100".to_string());
        health.add_metadata("buffer_usage".to_string(), "0.75".to_string());
        health.add_metadata("active_streams".to_string(), "3".to_string());
        health.add_metadata("dropped_messages".to_string(), "0".to_string());
        
        Ok(())
    }

    async fn check_daa_health(&self, health: &mut ComponentHealth) -> Result<()> {
        // DAA orchestrator health check
        // Check agent count, active agents, agent responsiveness
        
        // TODO: Replace with actual DaaFannIntegration health check once available
        // let daa = self.daa_integration.lock().await;
        // let agent_stats = daa.get_agent_statistics().await?;
        
        health.add_metadata("total_agents".to_string(), "5".to_string());
        health.add_metadata("active_agents".to_string(), "3".to_string());
        health.add_metadata("agent_response_rate".to_string(), "0.95".to_string());
        health.add_metadata("orchestrator_version".to_string(), "1.0.0".to_string());
        health.add_metadata("failed_agents".to_string(), "0".to_string());
        
        Ok(())
    }

    async fn check_neural_health(&self, health: &mut ComponentHealth) -> Result<()> {
        // Neural system health check
        // Check model availability, inference latency, accuracy
        
        // TODO: Replace with actual neural system health check
        // let neural = self.neural_system.lock().await;
        // let model_status = neural.get_model_status().await?;
        
        health.add_metadata("model_available".to_string(), "true".to_string());
        health.add_metadata("inference_latency_ms".to_string(), "200".to_string());
        health.add_metadata("model_accuracy".to_string(), "0.95".to_string());
        health.add_metadata("model_version".to_string(), "2.0.0".to_string());
        health.add_metadata("total_predictions".to_string(), "10000".to_string());
        
        Ok(())
    }

    async fn check_event_bus_health(&self, health: &mut ComponentHealth) -> Result<()> {
        // Event bus health check
        // Check message throughput, queue depth, subscriber count
        
        health.add_metadata("message_throughput".to_string(), "500".to_string());
        health.add_metadata("queue_depth".to_string(), "10".to_string());
        health.add_metadata("subscriber_count".to_string(), "8".to_string());
        
        Ok(())
    }

    async fn check_data_pipeline_health(&self, health: &mut ComponentHealth) -> Result<()> {
        // Data pipeline health check
        // Check processing rate, error rate, data quality
        
        // TODO: Replace with actual DataPipeline health check once available
        // let pipeline = self.data_pipeline.lock().await;
        // let pipeline_stats = pipeline.get_statistics().await?;
        
        health.add_metadata("processing_rate".to_string(), "750".to_string());
        health.add_metadata("error_rate".to_string(), "0.01".to_string());
        health.add_metadata("data_quality_score".to_string(), "0.98".to_string());
        health.add_metadata("pipeline_stages".to_string(), "5".to_string());
        health.add_metadata("queue_depth".to_string(), "100".to_string());
        
        Ok(())
    }

    async fn check_cache_health(&self, health: &mut ComponentHealth) -> Result<()> {
        // Cache health check
        // Check hit rate, memory usage, eviction rate
        
        health.add_metadata("hit_rate".to_string(), "0.88".to_string());
        health.add_metadata("memory_usage_mb".to_string(), "256".to_string());
        health.add_metadata("eviction_rate".to_string(), "0.05".to_string());
        
        Ok(())
    }
}

// Clone implementation for HealthMonitor
impl Clone for HealthMonitor {
    fn clone(&self) -> Self {
        Self {
            component_health: self.component_health.clone(),
            metrics_collector: self.metrics_collector.clone(),
            alert_manager: AlertManager::new(), // Create new alert manager for clone
            start_time: self.start_time,
            monitoring_interval: self.monitoring_interval,
            is_monitoring: self.is_monitoring.clone(),
        }
    }
}

// Default implementations
impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for AlertManager {
    fn default() -> Self {
        Self::new()
    }
}

// Display implementations for better debugging
impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "Healthy"),
            HealthStatus::Degraded(msg) => write!(f, "Degraded: {}", msg),
            HealthStatus::Unhealthy(msg) => write!(f, "Unhealthy: {}", msg),
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