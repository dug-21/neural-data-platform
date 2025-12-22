//! Monitoring and observability configuration module
//!
//! Handles metrics, logging, and performance monitoring configuration.

use serde::{Deserialize, Serialize};

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub metrics_interval_secs: u64,
    pub quality_threshold: f64,
    #[serde(default = "default_prometheus_port")]
    pub prometheus_port: Option<u16>,
    #[serde(default = "default_prometheus_path")]
    pub prometheus_path: String,
    #[serde(default = "default_true")]
    pub enable_performance_metrics: bool,
    #[serde(default = "default_true")]
    pub enable_memory_monitoring: bool,
    #[serde(default = "default_true")]
    pub enable_error_monitoring: bool,
    #[serde(default = "default_cpu_usage_threshold")]
    pub cpu_usage_threshold: f64,
    #[serde(default = "default_memory_usage_threshold")]
    pub memory_usage_threshold: f64,
    #[serde(default = "default_error_rate_threshold")]
    pub error_rate_threshold: f64,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            metrics_interval_secs: 60,
            quality_threshold: 0.95,
            prometheus_port: default_prometheus_port(),
            prometheus_path: default_prometheus_path(),
            enable_performance_metrics: default_true(),
            enable_memory_monitoring: default_true(),
            enable_error_monitoring: default_true(),
            cpu_usage_threshold: default_cpu_usage_threshold(),
            memory_usage_threshold: default_memory_usage_threshold(),
            error_rate_threshold: default_error_rate_threshold(),
        }
    }
}

/// Observability configuration for production monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_log_format")]
    pub log_format: String,
    #[serde(default = "default_true")]
    pub enable_tracing: bool,
    #[serde(default = "default_trace_sample_rate")]
    pub trace_sample_rate: f64,
    #[serde(default = "default_true")]
    pub enable_metrics: bool,
    #[serde(default = "default_false")]
    pub enable_file_logging: bool,
    #[serde(default)]
    pub log_file_path: Option<String>,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            log_format: default_log_format(),
            enable_tracing: default_true(),
            trace_sample_rate: default_trace_sample_rate(),
            enable_metrics: default_true(),
            enable_file_logging: default_false(),
            log_file_path: None,
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_log_format")]
    pub format: String,
    #[serde(default = "default_false")]
    pub enable_file_output: bool,
    #[serde(default = "default_log_file_path")]
    pub file_path: String,
    #[serde(default = "default_log_rotation_size")]
    pub rotation_size_mb: u64,
    #[serde(default = "default_log_rotation_count")]
    pub rotation_count: u32,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
            enable_file_output: default_false(),
            file_path: default_log_file_path(),
            rotation_size_mb: default_log_rotation_size(),
            rotation_count: default_log_rotation_count(),
        }
    }
}

/// Alerts configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_alert_interval_seconds")]
    pub alert_interval_seconds: u64,
    #[serde(default = "default_critical_threshold")]
    pub critical_threshold: f64,
    #[serde(default = "default_warning_threshold")]
    pub warning_threshold: f64,
    #[serde(default = "default_false")]
    pub enable_email_alerts: bool,
    #[serde(default = "default_false")]
    pub enable_slack_alerts: bool,
    #[serde(default)]
    pub email_recipients: Vec<String>,
    #[serde(default)]
    pub slack_webhook_url: Option<String>,
}

impl Default for AlertsConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            alert_interval_seconds: default_alert_interval_seconds(),
            critical_threshold: default_critical_threshold(),
            warning_threshold: default_warning_threshold(),
            enable_email_alerts: default_false(),
            enable_slack_alerts: default_false(),
            email_recipients: Vec::new(),
            slack_webhook_url: None,
        }
    }
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    #[serde(default = "default_max_memory_usage_mb")]
    pub max_memory_usage_mb: u64,
    #[serde(default = "default_max_cpu_usage_percent")]
    pub max_cpu_usage_percent: f64,
    #[serde(default = "default_gc_interval_seconds")]
    pub gc_interval_seconds: u64,
    #[serde(default = "default_true")]
    pub enable_performance_profiling: bool,
    #[serde(default = "default_performance_sample_rate")]
    pub performance_sample_rate: f64,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            max_memory_usage_mb: default_max_memory_usage_mb(),
            max_cpu_usage_percent: default_max_cpu_usage_percent(),
            gc_interval_seconds: default_gc_interval_seconds(),
            enable_performance_profiling: default_true(),
            performance_sample_rate: default_performance_sample_rate(),
        }
    }
}

// Default value functions
fn default_prometheus_port() -> Option<u16> {
    Some(9092)
}
fn default_prometheus_path() -> String {
    "/metrics".to_string()
}
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}
fn default_cpu_usage_threshold() -> f64 {
    80.0
}
fn default_memory_usage_threshold() -> f64 {
    85.0
}
fn default_error_rate_threshold() -> f64 {
    5.0
}
fn default_log_level() -> String {
    "info".to_string()
}
fn default_log_format() -> String {
    "json".to_string()
}
fn default_trace_sample_rate() -> f64 {
    0.1
}
fn default_log_file_path() -> String {
    "logs/neural-trader.log".to_string()
}
fn default_log_rotation_size() -> u64 {
    100
}
fn default_log_rotation_count() -> u32 {
    10
}
fn default_alert_interval_seconds() -> u64 {
    300
}
fn default_critical_threshold() -> f64 {
    95.0
}
fn default_warning_threshold() -> f64 {
    80.0
}
fn default_max_memory_usage_mb() -> u64 {
    2048
}
fn default_max_cpu_usage_percent() -> f64 {
    80.0
}
fn default_gc_interval_seconds() -> u64 {
    300
}
fn default_performance_sample_rate() -> f64 {
    0.01
}
