//! Configuration module for the autonomous platform
//! 
//! This module handles loading configuration from TOML files and environment variables.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::Path;

/// Main platform configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformConfig {
    pub platform: PlatformInfo,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub neural: NeuralConfig,
    pub monitoring: MonitoringConfig,
    #[serde(default)]
    pub observability: ObservabilityConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub performance: PerformanceConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub alerts: AlertsConfig,
    #[serde(default)]
    pub backup: BackupConfig,
    #[serde(default)]
    pub circuit_breaker: CircuitBreakerConfig,
    #[serde(default)]
    pub graceful_shutdown: GracefulShutdownConfig,
    #[serde(default)]
    pub development: DevelopmentConfig,
}

/// Platform metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformInfo {
    pub name: String,
    pub version: String,
    #[serde(default = "default_environment")]
    pub environment: String,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: u64,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout: u64,
    #[serde(default = "default_max_query_time")]
    pub max_query_time: u64,
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub max_connections: u32,
    pub default_ttl_seconds: u64,
    #[serde(default = "default_redis_connection_timeout_ms")]
    pub connection_timeout_ms: u64,
    #[serde(default = "default_false")]
    pub cluster_mode: bool,
    #[serde(default = "default_redis_pool_max_idle")]
    pub pool_max_idle: u32,
    #[serde(default = "default_redis_pool_timeout_seconds")]
    pub pool_timeout_seconds: u64,
}

/// Neural model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralConfig {
    pub memory_gb: f32,
    pub models: Vec<String>,
    pub prediction_cache_ttl: u64,
    #[serde(default = "default_model_load_timeout")]
    pub model_load_timeout: u64,
    #[serde(default = "default_max_concurrent_predictions")]
    pub max_concurrent_predictions: u32,
    #[serde(default = "default_true")]
    pub enable_model_monitoring: bool,
    #[serde(default = "default_accuracy_threshold")]
    pub accuracy_threshold: f64,
}

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

/// Security configuration for production deployment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(default = "default_enable_tls")]
    pub enable_tls: bool,
    
    #[serde(default = "default_tls_cert_path")]
    pub tls_cert_path: String,
    
    #[serde(default = "default_tls_key_path")]
    pub tls_key_path: String,
    
    #[serde(default = "default_rate_limit_per_minute")]
    pub rate_limit_per_minute: u64,
    
    #[serde(default = "default_max_request_size")]
    pub max_request_size: u64,
    
    #[serde(default = "default_allowed_origins")]
    pub allowed_origins: Vec<String>,
    
    // Additional existing fields for backward compatibility
    #[serde(default = "default_rate_limit_burst")]
    pub rate_limit_burst: u32,
    
    #[serde(default = "default_request_timeout")]
    pub request_timeout_seconds: u64,
    
    #[serde(default = "default_true")]
    pub enable_request_validation: bool,
    
    #[serde(default = "default_true")]
    pub enable_cors: bool,
    
    // Authentication & Authorization fields
    #[serde(default = "default_enable_auth")]
    pub enable_auth: bool,
    
    #[serde(default = "default_auth_token_expiry")]
    pub auth_token_expiry_seconds: u64,
    
    #[serde(default = "default_enable_api_keys")]
    pub enable_api_keys: bool,
    
    #[serde(default = "default_session_timeout")]
    pub session_timeout_seconds: u64,
}

/// Performance configuration for production optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    #[serde(default = "default_max_connections")]
    pub max_connections: u64,
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: u64,
    #[serde(default = "default_worker_threads")]
    pub worker_threads: usize,
    #[serde(default = "default_async_queue_size")]
    pub async_queue_size: usize,
    #[serde(default = "default_true")]
    pub enable_tcp_keepalive: bool,
    #[serde(default = "default_true")]
    pub enable_memory_compaction: bool,
    #[serde(default = "default_gc_interval_minutes")]
    pub gc_interval_minutes: u64,
    #[serde(default = "default_tcp_keepalive_time")]
    pub tcp_keepalive_time: u64,
    #[serde(default = "default_tcp_keepalive_interval")]
    pub tcp_keepalive_interval: u64,
    #[serde(default = "default_tcp_keepalive_probes")]
    pub tcp_keepalive_probes: u64,
    #[serde(default = "default_enable_profiling")]
    pub enable_profiling: bool,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_log_format")]
    pub format: String,
    #[serde(default = "default_false")]
    pub enable_file_logging: bool,
    #[serde(default)]
    pub log_file_path: Option<String>,
    #[serde(default = "default_log_file_max_size_mb")]
    pub log_file_max_size_mb: u32,
    #[serde(default = "default_log_file_max_files")]
    pub log_file_max_files: u32,
    #[serde(default = "default_false")]
    pub async_logging: bool,
    #[serde(default = "default_true")]
    pub filter_sensitive_data: bool,
}

/// Alerts configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertsConfig {
    #[serde(default = "default_false")]
    pub enable_email_alerts: bool,
    #[serde(default = "default_smtp_server")]
    pub email_smtp_server: String,
    #[serde(default = "default_smtp_port")]
    pub email_smtp_port: u16,
    #[serde(default)]
    pub email_from: Option<String>,
    #[serde(default = "default_empty_vec_string")]
    pub email_to: Vec<String>,
    #[serde(default = "default_false")]
    pub enable_slack_alerts: bool,
    #[serde(default)]
    pub slack_webhook_url: Option<String>,
    #[serde(default = "default_false")]
    pub enable_pagerduty: bool,
    #[serde(default)]
    pub pagerduty_service_key: Option<String>,
}

/// Backup configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    #[serde(default = "default_false")]
    pub enable_automatic_backup: bool,
    #[serde(default = "default_backup_interval_hours")]
    pub backup_interval_hours: u64,
    #[serde(default = "default_backup_retention_days")]
    pub backup_retention_days: u64,
    #[serde(default = "default_backup_storage_path")]
    pub backup_storage_path: String,
    #[serde(default = "default_false")]
    pub enable_cloud_backup: bool,
    #[serde(default = "default_cloud_backup_provider")]
    pub cloud_backup_provider: String,
    #[serde(default)]
    pub cloud_backup_bucket: Option<String>,
    #[serde(default)]
    pub cloud_backup_region: Option<String>,
}

/// Circuit breaker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    #[serde(default = "default_true")]
    pub enable_circuit_breaker: bool,
    #[serde(default = "default_circuit_breaker_failure_threshold")]
    pub failure_threshold: u32,
    #[serde(default = "default_circuit_breaker_recovery_timeout_seconds")]
    pub recovery_timeout_seconds: u64,
    #[serde(default = "default_circuit_breaker_half_open_max_calls")]
    pub half_open_max_calls: u32,
}

/// Graceful shutdown configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GracefulShutdownConfig {
    #[serde(default = "default_shutdown_timeout_secs")]
    pub shutdown_timeout_secs: u64,
    #[serde(default = "default_force_shutdown_after_secs")]
    pub force_shutdown_after_secs: u64,
}

/// Development-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevelopmentConfig {
    #[serde(default = "default_false")]
    pub enable_hot_reload: bool,
    #[serde(default = "default_false")]
    pub enable_debug_endpoints: bool,
    #[serde(default = "default_false")]
    pub mock_external_services: bool,
    #[serde(default = "default_false")]
    pub enable_test_data_generation: bool,
    #[serde(default = "default_false")]
    pub enable_profiling: bool,
    #[serde(default = "default_false")]
    pub seed_test_data: bool,
    #[serde(default = "default_false")]
    pub reset_database_on_startup: bool,
}

// Default value functions
fn default_max_connections() -> u64 { 1000 }
fn default_enable_profiling() -> bool { false }
fn default_shutdown_timeout_secs() -> u64 { 30 }
fn default_force_shutdown_after_secs() -> u64 { 60 }
fn default_prometheus_port() -> Option<u16> { Some(8080) }
fn default_prometheus_path() -> String { "/metrics".to_string() }
fn default_log_level() -> String { "info".to_string() }
fn default_log_format() -> String { "json".to_string() }
fn default_environment() -> String { "development".to_string() }
fn default_connection_timeout() -> u64 { 30 }
fn default_idle_timeout() -> u64 { 600 }
fn default_max_query_time() -> u64 { 30 }
fn default_redis_connection_timeout_ms() -> u64 { 5000 }
fn default_redis_pool_max_idle() -> u32 { 10 }
fn default_redis_pool_timeout_seconds() -> u64 { 30 }
fn default_true() -> bool { true }
fn default_false() -> bool { false }
fn default_trace_sample_rate() -> f64 { 1.0 }
fn default_rate_limit() -> u32 { 1000 }
fn default_rate_limit_burst() -> u32 { 100 }
fn default_request_timeout() -> u64 { 30 }
fn default_max_request_body_mb() -> u32 { 10 }
fn default_worker_threads() -> usize { 8 }
fn default_async_queue_size() -> usize { 10000 }
fn default_gc_interval_minutes() -> u64 { 30 }
fn default_model_load_timeout() -> u64 { 300 }
fn default_max_concurrent_predictions() -> u32 { 50 }
fn default_accuracy_threshold() -> f64 { 0.85 }
fn default_cpu_usage_threshold() -> f64 { 80.0 }
fn default_memory_usage_threshold() -> f64 { 85.0 }
fn default_error_rate_threshold() -> f64 { 0.05 }
fn default_allowed_origins() -> Vec<String> { vec!["https://localhost:3000".to_string()] }
fn default_tcp_keepalive_time() -> u64 { 7200 }
fn default_tcp_keepalive_interval() -> u64 { 75 }
fn default_tcp_keepalive_probes() -> u64 { 9 }
fn default_log_file_max_size_mb() -> u32 { 100 }
fn default_log_file_max_files() -> u32 { 10 }
fn default_smtp_server() -> String { "localhost".to_string() }
fn default_smtp_port() -> u16 { 587 }
fn default_empty_vec_string() -> Vec<String> { Vec::new() }
fn default_backup_interval_hours() -> u64 { 6 }
fn default_backup_retention_days() -> u64 { 30 }
fn default_backup_storage_path() -> String { "./backups".to_string() }
fn default_cloud_backup_provider() -> String { "s3".to_string() }
fn default_circuit_breaker_failure_threshold() -> u32 { 5 }
fn default_circuit_breaker_recovery_timeout_seconds() -> u64 { 60 }
fn default_circuit_breaker_half_open_max_calls() -> u32 { 10 }
fn default_shutdown_timeout_seconds() -> u64 { 30 }
fn default_drain_timeout_seconds() -> u64 { 10 }
fn default_api_version() -> String { "v1".to_string() }

// Security-related default functions
fn default_enable_tls() -> bool { false }
fn default_tls_cert_path() -> String { "/etc/ssl/certs/server.crt".to_string() }
fn default_tls_key_path() -> String { "/etc/ssl/private/server.key".to_string() }
fn default_rate_limit_per_minute() -> u64 { 1000 }
fn default_max_request_size() -> u64 { 10 * 1024 * 1024 } // 10MB
fn default_enable_auth() -> bool { false }
fn default_auth_token_expiry() -> u64 { 3600 } // 1 hour
fn default_enable_api_keys() -> bool { false }
fn default_session_timeout() -> u64 { 1800 } // 30 minutes

impl PlatformConfig {
    /// Loads platform configuration from a TOML file with environment variable overrides.
    ///
    /// This function reads configuration from a TOML file, applies environment variable
    /// overrides, and validates the resulting configuration. Environment variables
    /// follow the pattern `SECTION_FIELD` (e.g., `DATABASE_URL`, `NEURAL_MEMORY_GB`).
    ///
    /// # Arguments
    ///
    /// * `config_path` - Path to the TOML configuration file
    ///
    /// # Returns
    ///
    /// Returns a validated `PlatformConfig` instance, or an error if loading or
    /// validation fails.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The configuration file doesn't exist or can't be read
    /// - The TOML syntax is invalid
    /// - Environment variable values are invalid (e.g., non-numeric for numeric fields)
    /// - Configuration validation fails (e.g., invalid ranges, missing required values)
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use autonomous_platform::config::PlatformConfig;
    ///
    /// // Load from default location
    /// let config = PlatformConfig::load("config/platform.toml")?;
    ///
    /// // Environment variables override file values
    /// std::env::set_var("DATABASE_MAX_CONNECTIONS", "50");
    /// let config = PlatformConfig::load("config/platform.toml")?;
    /// assert_eq!(config.database.max_connections, 50);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn load(config_path: impl AsRef<Path>) -> Result<Self> {
        // Load from TOML file
        let config_content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {:?}", config_path.as_ref()))?;
        
        let mut config: PlatformConfig = toml::from_str(&config_content)
            .context("Failed to parse configuration TOML")?;
        
        // Apply environment variable overrides
        config.apply_env_overrides()?;
        
        // Validate configuration
        config.validate()?;
        
        Ok(config)
    }
    
    /// Apply comprehensive environment variable overrides
    /// All configuration values can be overridden using environment variables
    /// with the format: SECTION_FIELD_NAME (e.g., DATABASE_URL, REDIS_MAX_CONNECTIONS)
    fn apply_env_overrides(&mut self) -> Result<()> {
        // Platform overrides
        if let Ok(name) = env::var("PLATFORM_NAME") {
            self.platform.name = name;
        }
        if let Ok(version) = env::var("PLATFORM_VERSION") {
            self.platform.version = version;
        }
        if let Ok(env_val) = env::var("PLATFORM_ENVIRONMENT") {
            self.platform.environment = env_val;
        }
        if let Ok(log_level) = env::var("PLATFORM_LOG_LEVEL") {
            self.platform.log_level = log_level;
        }

        // Database overrides
        if let Ok(url) = env::var("DATABASE_URL") {
            self.database.url = url;
        }
        if let Ok(max_conn) = env::var("DATABASE_MAX_CONNECTIONS") {
            self.database.max_connections = max_conn.parse()
                .context("Invalid DATABASE_MAX_CONNECTIONS")?;
        }
        if let Ok(min_conn) = env::var("DATABASE_MIN_CONNECTIONS") {
            self.database.min_connections = min_conn.parse()
                .context("Invalid DATABASE_MIN_CONNECTIONS")?;
        }
        if let Ok(timeout) = env::var("DATABASE_CONNECTION_TIMEOUT") {
            self.database.connection_timeout = timeout.parse()
                .context("Invalid DATABASE_CONNECTION_TIMEOUT")?;
        }
        if let Ok(idle_timeout) = env::var("DATABASE_IDLE_TIMEOUT") {
            self.database.idle_timeout = idle_timeout.parse()
                .context("Invalid DATABASE_IDLE_TIMEOUT")?;
        }
        if let Ok(max_query_time) = env::var("DATABASE_MAX_QUERY_TIME") {
            self.database.max_query_time = max_query_time.parse()
                .context("Invalid DATABASE_MAX_QUERY_TIME")?;
        }
        
        // Redis overrides
        if let Ok(url) = env::var("REDIS_URL") {
            self.redis.url = url;
        }
        if let Ok(max_conn) = env::var("REDIS_MAX_CONNECTIONS") {
            self.redis.max_connections = max_conn.parse()
                .context("Invalid REDIS_MAX_CONNECTIONS")?;
        }
        if let Ok(ttl) = env::var("REDIS_DEFAULT_TTL_SECONDS") {
            self.redis.default_ttl_seconds = ttl.parse()
                .context("Invalid REDIS_DEFAULT_TTL_SECONDS")?;
        }
        if let Ok(timeout) = env::var("REDIS_CONNECTION_TIMEOUT_MS") {
            self.redis.connection_timeout_ms = timeout.parse()
                .context("Invalid REDIS_CONNECTION_TIMEOUT_MS")?;
        }
        if let Ok(cluster_mode) = env::var("REDIS_CLUSTER_MODE") {
            self.redis.cluster_mode = cluster_mode.parse()
                .context("Invalid REDIS_CLUSTER_MODE")?;
        }
        
        // Neural overrides
        if let Ok(memory) = env::var("NEURAL_MEMORY_GB") {
            self.neural.memory_gb = memory.parse()
                .context("Invalid NEURAL_MEMORY_GB")?;
        }
        if let Ok(models) = env::var("NEURAL_MODELS") {
            self.neural.models = models.split(',').map(String::from).collect();
        }
        if let Ok(ttl) = env::var("NEURAL_PREDICTION_CACHE_TTL") {
            self.neural.prediction_cache_ttl = ttl.parse()
                .context("Invalid NEURAL_PREDICTION_CACHE_TTL")?;
        }
        if let Ok(timeout) = env::var("NEURAL_MODEL_LOAD_TIMEOUT") {
            self.neural.model_load_timeout = timeout.parse()
                .context("Invalid NEURAL_MODEL_LOAD_TIMEOUT")?;
        }
        if let Ok(max_predictions) = env::var("NEURAL_MAX_CONCURRENT_PREDICTIONS") {
            self.neural.max_concurrent_predictions = max_predictions.parse()
                .context("Invalid NEURAL_MAX_CONCURRENT_PREDICTIONS")?;
        }
        if let Ok(enable_monitoring) = env::var("NEURAL_ENABLE_MODEL_MONITORING") {
            self.neural.enable_model_monitoring = enable_monitoring.parse()
                .context("Invalid NEURAL_ENABLE_MODEL_MONITORING")?;
        }
        if let Ok(threshold) = env::var("NEURAL_ACCURACY_THRESHOLD") {
            self.neural.accuracy_threshold = threshold.parse()
                .context("Invalid NEURAL_ACCURACY_THRESHOLD")?;
        }
        
        // Monitoring overrides
        if let Ok(interval) = env::var("MONITORING_METRICS_INTERVAL_SECS") {
            self.monitoring.metrics_interval_secs = interval.parse()
                .context("Invalid MONITORING_METRICS_INTERVAL_SECS")?;
        }
        if let Ok(threshold) = env::var("MONITORING_QUALITY_THRESHOLD") {
            self.monitoring.quality_threshold = threshold.parse()
                .context("Invalid MONITORING_QUALITY_THRESHOLD")?;
        }
        if let Ok(port) = env::var("MONITORING_PROMETHEUS_PORT") {
            self.monitoring.prometheus_port = Some(port.parse()
                .context("Invalid MONITORING_PROMETHEUS_PORT")?);
        }
        if let Ok(path) = env::var("MONITORING_PROMETHEUS_PATH") {
            self.monitoring.prometheus_path = path;
        }
        if let Ok(enable_perf) = env::var("MONITORING_ENABLE_PERFORMANCE_METRICS") {
            self.monitoring.enable_performance_metrics = enable_perf.parse()
                .context("Invalid MONITORING_ENABLE_PERFORMANCE_METRICS")?;
        }
        if let Ok(cpu_threshold) = env::var("MONITORING_CPU_USAGE_THRESHOLD") {
            self.monitoring.cpu_usage_threshold = cpu_threshold.parse()
                .context("Invalid MONITORING_CPU_USAGE_THRESHOLD")?;
        }
        if let Ok(memory_threshold) = env::var("MONITORING_MEMORY_USAGE_THRESHOLD") {
            self.monitoring.memory_usage_threshold = memory_threshold.parse()
                .context("Invalid MONITORING_MEMORY_USAGE_THRESHOLD")?;
        }
        if let Ok(error_threshold) = env::var("MONITORING_ERROR_RATE_THRESHOLD") {
            self.monitoring.error_rate_threshold = error_threshold.parse()
                .context("Invalid MONITORING_ERROR_RATE_THRESHOLD")?;
        }

        // Observability overrides
        if let Ok(log_level) = env::var("OBSERVABILITY_LOG_LEVEL") {
            self.observability.log_level = log_level;
        }
        if let Ok(log_format) = env::var("OBSERVABILITY_LOG_FORMAT") {
            self.observability.log_format = log_format;
        }
        if let Ok(enable_tracing) = env::var("OBSERVABILITY_ENABLE_TRACING") {
            self.observability.enable_tracing = enable_tracing.parse()
                .context("Invalid OBSERVABILITY_ENABLE_TRACING")?;
        }
        if let Ok(sample_rate) = env::var("OBSERVABILITY_TRACE_SAMPLE_RATE") {
            self.observability.trace_sample_rate = sample_rate.parse()
                .context("Invalid OBSERVABILITY_TRACE_SAMPLE_RATE")?;
        }

        // Security overrides
        if let Ok(enable_tls) = env::var("SECURITY_ENABLE_TLS") {
            self.security.enable_tls = enable_tls.parse()
                .context("Invalid SECURITY_ENABLE_TLS")?;
        }
        if let Ok(cert_path) = env::var("SECURITY_TLS_CERT_PATH") {
            self.security.tls_cert_path = cert_path;
        }
        if let Ok(key_path) = env::var("SECURITY_TLS_KEY_PATH") {
            self.security.tls_key_path = key_path;
        }
        if let Ok(rate_limit) = env::var("SECURITY_RATE_LIMIT_PER_MINUTE") {
            self.security.rate_limit_per_minute = rate_limit.parse()
                .context("Invalid SECURITY_RATE_LIMIT_PER_MINUTE")?;
        }
        if let Ok(max_size) = env::var("SECURITY_MAX_REQUEST_SIZE") {
            self.security.max_request_size = max_size.parse()
                .context("Invalid SECURITY_MAX_REQUEST_SIZE")?;
        }
        if let Ok(burst) = env::var("SECURITY_RATE_LIMIT_BURST") {
            self.security.rate_limit_burst = burst.parse()
                .context("Invalid SECURITY_RATE_LIMIT_BURST")?;
        }
        if let Ok(timeout) = env::var("SECURITY_REQUEST_TIMEOUT_SECONDS") {
            self.security.request_timeout_seconds = timeout.parse()
                .context("Invalid SECURITY_REQUEST_TIMEOUT_SECONDS")?;
        }
        if let Ok(enable_cors) = env::var("SECURITY_ENABLE_CORS") {
            self.security.enable_cors = enable_cors.parse()
                .context("Invalid SECURITY_ENABLE_CORS")?;
        }
        if let Ok(origins) = env::var("SECURITY_ALLOWED_ORIGINS") {
            self.security.allowed_origins = origins.split(',').map(String::from).collect();
        }
        if let Ok(enable_auth) = env::var("SECURITY_ENABLE_AUTH") {
            self.security.enable_auth = enable_auth.parse()
                .context("Invalid SECURITY_ENABLE_AUTH")?;
        }
        if let Ok(token_expiry) = env::var("SECURITY_AUTH_TOKEN_EXPIRY_SECONDS") {
            self.security.auth_token_expiry_seconds = token_expiry.parse()
                .context("Invalid SECURITY_AUTH_TOKEN_EXPIRY_SECONDS")?;
        }
        if let Ok(enable_api_keys) = env::var("SECURITY_ENABLE_API_KEYS") {
            self.security.enable_api_keys = enable_api_keys.parse()
                .context("Invalid SECURITY_ENABLE_API_KEYS")?;
        }
        if let Ok(session_timeout) = env::var("SECURITY_SESSION_TIMEOUT_SECONDS") {
            self.security.session_timeout_seconds = session_timeout.parse()
                .context("Invalid SECURITY_SESSION_TIMEOUT_SECONDS")?;
        }

        // Performance overrides
        if let Ok(max_connections) = env::var("PERFORMANCE_MAX_CONNECTIONS") {
            self.performance.max_connections = max_connections.parse()
                .context("Invalid PERFORMANCE_MAX_CONNECTIONS")?;
        }
        if let Ok(conn_timeout) = env::var("PERFORMANCE_CONNECTION_TIMEOUT") {
            self.performance.connection_timeout = conn_timeout.parse()
                .context("Invalid PERFORMANCE_CONNECTION_TIMEOUT")?;
        }
        if let Ok(threads) = env::var("PERFORMANCE_WORKER_THREADS") {
            self.performance.worker_threads = threads.parse()
                .context("Invalid PERFORMANCE_WORKER_THREADS")?;
        }
        if let Ok(queue_size) = env::var("PERFORMANCE_ASYNC_QUEUE_SIZE") {
            self.performance.async_queue_size = queue_size.parse()
                .context("Invalid PERFORMANCE_ASYNC_QUEUE_SIZE")?;
        }
        if let Ok(enable_keepalive) = env::var("PERFORMANCE_ENABLE_TCP_KEEPALIVE") {
            self.performance.enable_tcp_keepalive = enable_keepalive.parse()
                .context("Invalid PERFORMANCE_ENABLE_TCP_KEEPALIVE")?;
        }
        if let Ok(keepalive_time) = env::var("PERFORMANCE_TCP_KEEPALIVE_TIME") {
            self.performance.tcp_keepalive_time = keepalive_time.parse()
                .context("Invalid PERFORMANCE_TCP_KEEPALIVE_TIME")?;
        }
        if let Ok(keepalive_interval) = env::var("PERFORMANCE_TCP_KEEPALIVE_INTERVAL") {
            self.performance.tcp_keepalive_interval = keepalive_interval.parse()
                .context("Invalid PERFORMANCE_TCP_KEEPALIVE_INTERVAL")?;
        }
        if let Ok(keepalive_probes) = env::var("PERFORMANCE_TCP_KEEPALIVE_PROBES") {
            self.performance.tcp_keepalive_probes = keepalive_probes.parse()
                .context("Invalid PERFORMANCE_TCP_KEEPALIVE_PROBES")?;
        }
        if let Ok(enable_profiling) = env::var("PERFORMANCE_ENABLE_PROFILING") {
            self.performance.enable_profiling = enable_profiling.parse()
                .context("Invalid PERFORMANCE_ENABLE_PROFILING")?;
        }

        // Logging overrides
        if let Ok(level) = env::var("LOGGING_LEVEL") {
            self.logging.level = level;
        }
        if let Ok(format) = env::var("LOGGING_FORMAT") {
            self.logging.format = format;
        }
        if let Ok(enable_file) = env::var("LOGGING_ENABLE_FILE_LOGGING") {
            self.logging.enable_file_logging = enable_file.parse()
                .context("Invalid LOGGING_ENABLE_FILE_LOGGING")?;
        }
        if let Ok(file_path) = env::var("LOGGING_LOG_FILE_PATH") {
            self.logging.log_file_path = Some(file_path);
        }
        if let Ok(async_logging) = env::var("LOGGING_ASYNC_LOGGING") {
            self.logging.async_logging = async_logging.parse()
                .context("Invalid LOGGING_ASYNC_LOGGING")?;
        }

        // Circuit breaker overrides
        if let Ok(enable_cb) = env::var("CIRCUIT_BREAKER_ENABLE_CIRCUIT_BREAKER") {
            self.circuit_breaker.enable_circuit_breaker = enable_cb.parse()
                .context("Invalid CIRCUIT_BREAKER_ENABLE_CIRCUIT_BREAKER")?;
        }
        if let Ok(failure_threshold) = env::var("CIRCUIT_BREAKER_FAILURE_THRESHOLD") {
            self.circuit_breaker.failure_threshold = failure_threshold.parse()
                .context("Invalid CIRCUIT_BREAKER_FAILURE_THRESHOLD")?;
        }
        if let Ok(recovery_timeout) = env::var("CIRCUIT_BREAKER_RECOVERY_TIMEOUT_SECONDS") {
            self.circuit_breaker.recovery_timeout_seconds = recovery_timeout.parse()
                .context("Invalid CIRCUIT_BREAKER_RECOVERY_TIMEOUT_SECONDS")?;
        }

        // Graceful shutdown overrides
        if let Ok(shutdown_timeout) = env::var("GRACEFUL_SHUTDOWN_SHUTDOWN_TIMEOUT_SECS") {
            self.graceful_shutdown.shutdown_timeout_secs = shutdown_timeout.parse()
                .context("Invalid GRACEFUL_SHUTDOWN_SHUTDOWN_TIMEOUT_SECS")?;
        }
        if let Ok(force_shutdown) = env::var("GRACEFUL_SHUTDOWN_FORCE_SHUTDOWN_AFTER_SECS") {
            self.graceful_shutdown.force_shutdown_after_secs = force_shutdown.parse()
                .context("Invalid GRACEFUL_SHUTDOWN_FORCE_SHUTDOWN_AFTER_SECS")?;
        }

        // Development overrides
        if let Ok(enable_hot_reload) = env::var("DEVELOPMENT_ENABLE_HOT_RELOAD") {
            self.development.enable_hot_reload = enable_hot_reload.parse()
                .context("Invalid DEVELOPMENT_ENABLE_HOT_RELOAD")?;
        }
        if let Ok(enable_debug) = env::var("DEVELOPMENT_ENABLE_DEBUG_ENDPOINTS") {
            self.development.enable_debug_endpoints = enable_debug.parse()
                .context("Invalid DEVELOPMENT_ENABLE_DEBUG_ENDPOINTS")?;
        }
        if let Ok(mock_services) = env::var("DEVELOPMENT_MOCK_EXTERNAL_SERVICES") {
            self.development.mock_external_services = mock_services.parse()
                .context("Invalid DEVELOPMENT_MOCK_EXTERNAL_SERVICES")?;
        }
        
        Ok(())
    }
    
    /// Validate all configuration settings comprehensively
    fn validate(&self) -> Result<()> {
        // Validate platform settings
        if self.platform.name.is_empty() {
            anyhow::bail!("Platform name cannot be empty");
        }
        if self.platform.version.is_empty() {
            anyhow::bail!("Platform version cannot be empty");
        }
        let valid_environments = ["development", "staging", "production"];
        if !valid_environments.contains(&self.platform.environment.as_str()) {
            anyhow::bail!("Platform environment must be one of: {:?}", valid_environments);
        }

        // Validate database settings
        if self.database.url.is_empty() {
            anyhow::bail!("Database URL cannot be empty");
        }
        if self.database.min_connections > self.database.max_connections {
            anyhow::bail!("Database min_connections cannot exceed max_connections");
        }
        if self.database.max_connections == 0 {
            anyhow::bail!("Database max_connections must be greater than 0");
        }
        if self.database.connection_timeout == 0 {
            anyhow::bail!("Database connection_timeout must be greater than 0");
        }
        if self.database.max_query_time == 0 {
            anyhow::bail!("Database max_query_time must be greater than 0");
        }
        
        // Validate Redis settings
        if self.redis.url.is_empty() {
            anyhow::bail!("Redis URL cannot be empty");
        }
        if self.redis.max_connections == 0 {
            anyhow::bail!("Redis max_connections must be greater than 0");
        }
        if self.redis.default_ttl_seconds == 0 {
            anyhow::bail!("Redis default_ttl_seconds must be greater than 0");
        }
        if self.redis.connection_timeout_ms == 0 {
            anyhow::bail!("Redis connection_timeout_ms must be greater than 0");
        }
        
        // Validate neural settings
        if self.neural.memory_gb <= 0.0 {
            anyhow::bail!("Neural memory_gb must be positive");
        }
        if self.neural.models.is_empty() {
            anyhow::bail!("At least one neural model must be configured");
        }
        if self.neural.prediction_cache_ttl == 0 {
            anyhow::bail!("Neural prediction_cache_ttl must be greater than 0");
        }
        if self.neural.model_load_timeout == 0 {
            anyhow::bail!("Neural model_load_timeout must be greater than 0");
        }
        if self.neural.max_concurrent_predictions == 0 {
            anyhow::bail!("Neural max_concurrent_predictions must be greater than 0");
        }
        if !(0.0..=1.0).contains(&self.neural.accuracy_threshold) {
            anyhow::bail!("Neural accuracy_threshold must be between 0 and 1");
        }
        
        // Validate monitoring settings
        if self.monitoring.metrics_interval_secs == 0 {
            anyhow::bail!("Monitoring metrics_interval_secs must be greater than 0");
        }
        if !(0.0..=1.0).contains(&self.monitoring.quality_threshold) {
            anyhow::bail!("Monitoring quality_threshold must be between 0 and 1");
        }
        if let Some(port) = self.monitoring.prometheus_port {
            if port == 0 {
                anyhow::bail!("Monitoring prometheus_port must be greater than 0");
            }
        }
        if !(0.0..=100.0).contains(&self.monitoring.cpu_usage_threshold) {
            anyhow::bail!("Monitoring cpu_usage_threshold must be between 0 and 100");
        }
        if !(0.0..=100.0).contains(&self.monitoring.memory_usage_threshold) {
            anyhow::bail!("Monitoring memory_usage_threshold must be between 0 and 100");
        }
        if !(0.0..=1.0).contains(&self.monitoring.error_rate_threshold) {
            anyhow::bail!("Monitoring error_rate_threshold must be between 0 and 1");
        }

        // Validate observability settings
        let valid_log_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_log_levels.contains(&self.observability.log_level.as_str()) {
            anyhow::bail!("Observability log_level must be one of: {:?}", valid_log_levels);
        }
        let valid_log_formats = ["json", "pretty", "compact"];
        if !valid_log_formats.contains(&self.observability.log_format.as_str()) {
            anyhow::bail!("Observability log_format must be one of: {:?}", valid_log_formats);
        }
        if !(0.0..=1.0).contains(&self.observability.trace_sample_rate) {
            anyhow::bail!("Observability trace_sample_rate must be between 0 and 1");
        }

        // Validate security settings
        if self.security.enable_tls && (self.security.tls_cert_path.is_empty() || self.security.tls_key_path.is_empty()) {
            anyhow::bail!("TLS cert and key paths must be provided when TLS is enabled");
        }
        if self.security.rate_limit_per_minute == 0 {
            anyhow::bail!("Security rate_limit_per_minute must be greater than 0");
        }
        if self.security.max_request_size == 0 {
            anyhow::bail!("Security max_request_size must be greater than 0");
        }
        if self.security.rate_limit_burst == 0 {
            anyhow::bail!("Security rate_limit_burst must be greater than 0");
        }
        if self.security.request_timeout_seconds == 0 {
            anyhow::bail!("Security request_timeout_seconds must be greater than 0");
        }
        if self.security.auth_token_expiry_seconds == 0 {
            anyhow::bail!("Security auth_token_expiry_seconds must be greater than 0");
        }
        if self.security.session_timeout_seconds == 0 {
            anyhow::bail!("Security session_timeout_seconds must be greater than 0");
        }

        // Validate performance settings
        if self.performance.max_connections == 0 {
            anyhow::bail!("Performance max_connections must be greater than 0");
        }
        if self.performance.connection_timeout == 0 {
            anyhow::bail!("Performance connection_timeout must be greater than 0");
        }
        if self.performance.worker_threads == 0 {
            anyhow::bail!("Performance worker_threads must be greater than 0");
        }
        if self.performance.async_queue_size == 0 {
            anyhow::bail!("Performance async_queue_size must be greater than 0");
        }
        if self.performance.tcp_keepalive_time == 0 {
            anyhow::bail!("Performance tcp_keepalive_time must be greater than 0");
        }
        if self.performance.tcp_keepalive_interval == 0 {
            anyhow::bail!("Performance tcp_keepalive_interval must be greater than 0");
        }
        if self.performance.tcp_keepalive_probes == 0 {
            anyhow::bail!("Performance tcp_keepalive_probes must be greater than 0");
        }

        // Validate logging settings
        let valid_logging_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_logging_levels.contains(&self.logging.level.as_str()) {
            anyhow::bail!("Logging level must be one of: {:?}", valid_logging_levels);
        }
        let valid_logging_formats = ["json", "pretty", "compact"];
        if !valid_logging_formats.contains(&self.logging.format.as_str()) {
            anyhow::bail!("Logging format must be one of: {:?}", valid_logging_formats);
        }
        if self.logging.log_file_max_size_mb == 0 {
            anyhow::bail!("Logging log_file_max_size_mb must be greater than 0");
        }
        if self.logging.log_file_max_files == 0 {
            anyhow::bail!("Logging log_file_max_files must be greater than 0");
        }

        // Validate alerts settings
        if self.alerts.enable_email_alerts && self.alerts.email_from.is_none() {
            anyhow::bail!("Email from address must be provided when email alerts are enabled");
        }
        if self.alerts.enable_slack_alerts && self.alerts.slack_webhook_url.is_none() {
            anyhow::bail!("Slack webhook URL must be provided when Slack alerts are enabled");
        }
        if self.alerts.enable_pagerduty && self.alerts.pagerduty_service_key.is_none() {
            anyhow::bail!("PagerDuty service key must be provided when PagerDuty alerts are enabled");
        }

        // Validate backup settings
        if self.backup.backup_interval_hours == 0 {
            anyhow::bail!("Backup backup_interval_hours must be greater than 0");
        }
        if self.backup.backup_retention_days == 0 {
            anyhow::bail!("Backup backup_retention_days must be greater than 0");
        }
        if self.backup.backup_storage_path.is_empty() {
            anyhow::bail!("Backup storage path cannot be empty");
        }
        if self.backup.enable_cloud_backup && self.backup.cloud_backup_bucket.is_none() {
            anyhow::bail!("Cloud backup bucket must be provided when cloud backup is enabled");
        }

        // Validate circuit breaker settings
        if self.circuit_breaker.failure_threshold == 0 {
            anyhow::bail!("Circuit breaker failure_threshold must be greater than 0");
        }
        if self.circuit_breaker.recovery_timeout_seconds == 0 {
            anyhow::bail!("Circuit breaker recovery_timeout_seconds must be greater than 0");
        }
        if self.circuit_breaker.half_open_max_calls == 0 {
            anyhow::bail!("Circuit breaker half_open_max_calls must be greater than 0");
        }

        // Validate graceful shutdown settings
        if self.graceful_shutdown.shutdown_timeout_secs == 0 {
            anyhow::bail!("Graceful shutdown timeout_secs must be greater than 0");
        }
        if self.graceful_shutdown.force_shutdown_after_secs == 0 {
            anyhow::bail!("Graceful shutdown force_shutdown_after_secs must be greater than 0");
        }
        
        Ok(())
    }
}

/// Default configuration path
pub const DEFAULT_CONFIG_PATH: &str = "config/platform.toml";

/// Load default configuration
pub fn load_default_config() -> Result<PlatformConfig> {
    PlatformConfig::load(DEFAULT_CONFIG_PATH)
}

/// Load production configuration
pub fn load_production_config() -> Result<PlatformConfig> {
    PlatformConfig::load("config/production.toml")
}

/// Load development configuration
pub fn load_development_config() -> Result<PlatformConfig> {
    PlatformConfig::load("config/development.toml")
}

/// Get configuration based on environment
pub fn load_config_for_environment(environment: &str) -> Result<PlatformConfig> {
    match environment {
        "production" => load_production_config(),
        "development" => load_development_config(),
        _ => load_default_config(),
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
            enable_file_logging: default_false(),
            log_file_path: None,
            log_file_max_size_mb: default_log_file_max_size_mb(),
            log_file_max_files: default_log_file_max_files(),
            async_logging: default_false(),
            filter_sensitive_data: default_true(),
        }
    }
}

impl Default for AlertsConfig {
    fn default() -> Self {
        Self {
            enable_email_alerts: default_false(),
            email_smtp_server: default_smtp_server(),
            email_smtp_port: default_smtp_port(),
            email_from: None,
            email_to: default_empty_vec_string(),
            enable_slack_alerts: default_false(),
            slack_webhook_url: None,
            enable_pagerduty: default_false(),
            pagerduty_service_key: None,
        }
    }
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            enable_automatic_backup: default_false(),
            backup_interval_hours: default_backup_interval_hours(),
            backup_retention_days: default_backup_retention_days(),
            backup_storage_path: default_backup_storage_path(),
            enable_cloud_backup: default_false(),
            cloud_backup_provider: default_cloud_backup_provider(),
            cloud_backup_bucket: None,
            cloud_backup_region: None,
        }
    }
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            enable_circuit_breaker: default_true(),
            failure_threshold: default_circuit_breaker_failure_threshold(),
            recovery_timeout_seconds: default_circuit_breaker_recovery_timeout_seconds(),
            half_open_max_calls: default_circuit_breaker_half_open_max_calls(),
        }
    }
}

impl Default for GracefulShutdownConfig {
    fn default() -> Self {
        Self {
            shutdown_timeout_secs: default_shutdown_timeout_secs(),
            force_shutdown_after_secs: default_force_shutdown_after_secs(),
        }
    }
}

impl Default for DevelopmentConfig {
    fn default() -> Self {
        Self {
            enable_hot_reload: default_false(),
            enable_debug_endpoints: default_false(),
            mock_external_services: default_false(),
            enable_test_data_generation: default_false(),
            enable_profiling: default_false(),
            seed_test_data: default_false(),
            reset_database_on_startup: default_false(),
        }
    }
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

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_tls: default_enable_tls(),
            tls_cert_path: default_tls_cert_path(),
            tls_key_path: default_tls_key_path(),
            rate_limit_per_minute: default_rate_limit_per_minute(),
            max_request_size: default_max_request_size(),
            allowed_origins: default_allowed_origins(),
            rate_limit_burst: default_rate_limit_burst(),
            request_timeout_seconds: default_request_timeout(),
            enable_request_validation: default_true(),
            enable_cors: default_true(),
            enable_auth: default_enable_auth(),
            auth_token_expiry_seconds: default_auth_token_expiry(),
            enable_api_keys: default_enable_api_keys(),
            session_timeout_seconds: default_session_timeout(),
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            max_connections: default_max_connections(),
            connection_timeout: default_connection_timeout(),
            worker_threads: default_worker_threads(),
            async_queue_size: default_async_queue_size(),
            enable_tcp_keepalive: default_true(),
            enable_memory_compaction: default_true(),
            gc_interval_minutes: default_gc_interval_minutes(),
            tcp_keepalive_time: default_tcp_keepalive_time(),
            tcp_keepalive_interval: default_tcp_keepalive_interval(),
            tcp_keepalive_probes: default_tcp_keepalive_probes(),
            enable_profiling: default_enable_profiling(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;
    use serial_test::serial;
    
    #[test]
    #[serial]
    fn test_load_valid_config() {
        // Clear any environment variables that might interfere from other tests
        std::env::remove_var("DATABASE_MAX_CONNECTIONS");
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("REDIS_URL");
        std::env::remove_var("REDIS_MAX_CONNECTIONS");
        
        let config_content = r#"
[platform]
name = "test-platform"
version = "0.1.0"

[database]
url = "postgres://test@localhost/test"
max_connections = 10
min_connections = 2

[redis]
url = "redis://localhost:6379"
max_connections = 5
default_ttl_seconds = 300

[neural]
memory_gb = 2.0
models = ["NHITS", "DeepAR"]
prediction_cache_ttl = 600

[monitoring]
metrics_interval_secs = 30
quality_threshold = 0.9
"#;
        
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();
        
        let config = PlatformConfig::load(temp_file.path()).unwrap();
        assert_eq!(config.platform.name, "test-platform");
        assert_eq!(config.database.max_connections, 10);
        assert_eq!(config.neural.models.len(), 2);
    }
    
    #[test]
    #[serial]
    fn test_env_override() {
        let config_content = r#"
[platform]
name = "test-platform"
version = "0.1.0"

[database]
url = "postgres://test@localhost/test"
max_connections = 10
min_connections = 2

[redis]
url = "redis://localhost:6379"
max_connections = 5
default_ttl_seconds = 300

[neural]
memory_gb = 2.0
models = ["NHITS"]
prediction_cache_ttl = 600

[monitoring]
metrics_interval_secs = 30
quality_threshold = 0.9
"#;
        
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();
        
        // Set environment variable
        env::set_var("DATABASE_MAX_CONNECTIONS", "20");
        
        let config = PlatformConfig::load(temp_file.path()).unwrap();
        assert_eq!(config.database.max_connections, 20);
        
        // Clean up
        env::remove_var("DATABASE_MAX_CONNECTIONS");
    }
    
    #[test]
    #[serial]
    fn test_validation_errors() {
        // Clear any environment variables that might interfere
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("DATABASE_MAX_CONNECTIONS");
        std::env::remove_var("DATABASE_MIN_CONNECTIONS");
        let invalid_configs = vec![
            // Empty database URL
            r#"
[platform]
name = "test"
version = "1.0"
[database]
url = ""
max_connections = 10
min_connections = 2
[redis]
url = "redis://localhost"
max_connections = 5
default_ttl_seconds = 300
[neural]
memory_gb = 1.0
models = ["test"]
prediction_cache_ttl = 300
[monitoring]
metrics_interval_secs = 30
quality_threshold = 0.9
"#,
            // Invalid connection settings
            r#"
[platform]
name = "test"
version = "1.0"
[database]
url = "postgres://test"
max_connections = 5
min_connections = 10
[redis]
url = "redis://localhost"
max_connections = 5
default_ttl_seconds = 300
[neural]
memory_gb = 1.0
models = ["test"]
prediction_cache_ttl = 300
[monitoring]
metrics_interval_secs = 30
quality_threshold = 0.9
"#,
        ];
        
        for config_content in invalid_configs {
            let mut temp_file = NamedTempFile::new().unwrap();
            temp_file.write_all(config_content.as_bytes()).unwrap();
            
            assert!(PlatformConfig::load(temp_file.path()).is_err());
        }
    }
}