//! Application configuration for the Bronze MCP Server.
//!
//! Configuration follows the NDP pattern: Environment variables take precedence
//! with sensible defaults for local development. When available, configuration
//! is also read from etcd via the config-client crate.
//!
//! # Configuration Hierarchy
//!
//! 1. Environment variables (highest priority)
//! 2. etcd values (via ConfigClient)
//! 3. Defaults (lowest priority)

use crate::error::{McpError, McpResult};
use config_client::{ConfigClient, StreamRegistry};
use tracing::{debug, info, warn};

/// Application configuration loaded from environment variables.
///
/// All configuration is environment-driven for cloud portability.
/// No hardcoded values - defaults are for local development only.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Server listen address (host:port)
    pub listen_addr: String,

    /// etcd endpoints (comma-separated for HA)
    pub etcd_endpoints: Vec<String>,

    /// Path to Bronze layer raw data directory
    pub raw_path: String,

    /// Log level for tracing subscriber
    pub log_level: String,

    /// TimescaleDB connection URL (optional - Silver layer features)
    ///
    /// When set, enables Silver, Dictionary, and ETL MCP tools.
    /// Format: postgresql://user:password@host:port/database
    pub timescale_url: Option<String>,

    /// Maximum connections in TimescaleDB pool (default: 5)
    pub timescale_max_connections: u32,

    /// Connection timeout in seconds (default: 10)
    pub timescale_connect_timeout_secs: u64,
}

impl AppConfig {
    /// Load configuration from environment variables.
    ///
    /// # Environment Variables
    ///
    /// - `NDP_MCP_LISTEN`: Server bind address (default: "0.0.0.0:9100")
    /// - `NDP_ETCD_ENDPOINTS`: Comma-separated etcd endpoints (default: "http://localhost:2379")
    /// - `NDP_RAW_PATH`: Bronze layer data directory (default: "/data/raw")
    /// - `RUST_LOG`: Log level filter (default: "info")
    /// - `NDP_TIMESCALE_URL`: TimescaleDB connection URL (optional)
    /// - `NDP_TIMESCALE_MAX_CONNECTIONS`: Max pool connections (default: 5)
    /// - `NDP_TIMESCALE_CONNECT_TIMEOUT_SECS`: Connection timeout (default: 10)
    ///
    /// # Errors
    ///
    /// Returns `McpError::Config` if environment parsing fails.
    pub fn from_env() -> McpResult<Self> {
        // Parse optional TimescaleDB max connections
        let timescale_max_connections = std::env::var("NDP_TIMESCALE_MAX_CONNECTIONS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);

        // Parse optional TimescaleDB connect timeout
        let timescale_connect_timeout_secs = std::env::var("NDP_TIMESCALE_CONNECT_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);

        Ok(Self {
            listen_addr: std::env::var("NDP_MCP_LISTEN")
                .unwrap_or_else(|_| "0.0.0.0:9100".to_string()),

            etcd_endpoints: std::env::var("NDP_ETCD_ENDPOINTS")
                .unwrap_or_else(|_| "http://localhost:2379".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),

            raw_path: std::env::var("NDP_RAW_PATH").unwrap_or_else(|_| "/data/raw".to_string()),

            log_level: std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),

            // TimescaleDB configuration (optional - enables Silver layer)
            timescale_url: std::env::var("NDP_TIMESCALE_URL").ok(),
            timescale_max_connections,
            timescale_connect_timeout_secs,
        })
    }

    /// Check if TimescaleDB is configured.
    ///
    /// Returns true if NDP_TIMESCALE_URL is set, enabling Silver layer features.
    pub fn has_timescale(&self) -> bool {
        self.timescale_url.is_some()
    }

    /// Validate configuration values.
    ///
    /// # Errors
    ///
    /// Returns `McpError::Config` if:
    /// - No etcd endpoints configured
    /// - Listen address is empty
    pub fn validate(&self) -> McpResult<()> {
        if self.etcd_endpoints.is_empty() {
            return Err(McpError::Config("No etcd endpoints configured".to_string()));
        }

        if self.listen_addr.is_empty() {
            return Err(McpError::Config(
                "Listen address cannot be empty".to_string(),
            ));
        }

        Ok(())
    }

    /// Get the host portion of the listen address.
    pub fn host(&self) -> &str {
        self.listen_addr.split(':').next().unwrap_or("0.0.0.0")
    }

    /// Get the port portion of the listen address.
    pub fn port(&self) -> u16 {
        self.listen_addr
            .split(':')
            .nth(1)
            .and_then(|p| p.parse().ok())
            .unwrap_or(9100)
    }

    /// Create a ConfigClient from the configured etcd endpoints.
    ///
    /// # Errors
    ///
    /// Returns `McpError::EtcdUnavailable` if connection fails.
    pub async fn create_config_client(&self) -> McpResult<ConfigClient> {
        let endpoints: Vec<&str> = self.etcd_endpoints.iter().map(|s| s.as_str()).collect();
        info!(endpoints = ?endpoints, "Creating ConfigClient");

        ConfigClient::new(&endpoints)
            .await
            .map_err(|e| McpError::EtcdUnavailable(format!("Failed to create ConfigClient: {}", e)))
    }

    /// Create a StreamRegistry from the configured etcd endpoints.
    ///
    /// StreamRegistry provides cached access to stream configurations.
    ///
    /// # Errors
    ///
    /// Returns `McpError::EtcdUnavailable` if connection fails.
    pub async fn create_stream_registry(&self) -> McpResult<StreamRegistry> {
        let endpoints: Vec<&str> = self.etcd_endpoints.iter().map(|s| s.as_str()).collect();
        info!(endpoints = ?endpoints, "Creating StreamRegistry");

        StreamRegistry::new(&endpoints).await.map_err(|e| {
            McpError::EtcdUnavailable(format!("Failed to create StreamRegistry: {}", e))
        })
    }

    /// Get the storage base path, with etcd fallback.
    ///
    /// Order of precedence:
    /// 1. NDP_RAW_PATH environment variable
    /// 2. etcd `/storage/base_path` key (if ConfigClient provided)
    /// 3. Default value "/data/raw"
    ///
    /// # Arguments
    ///
    /// * `config_client` - Optional ConfigClient for etcd lookup
    pub async fn get_raw_path_with_etcd(&self, config_client: Option<&ConfigClient>) -> String {
        // Environment variable takes precedence
        if let Ok(env_path) = std::env::var("NDP_RAW_PATH") {
            debug!(path = %env_path, "Using NDP_RAW_PATH from environment");
            return env_path;
        }

        // Try etcd if client provided
        if let Some(client) = config_client {
            match client.get::<String>("/storage/base_path").await {
                Ok(etcd_path) => {
                    info!(path = %etcd_path, "Using base_path from etcd");
                    return etcd_path;
                }
                Err(e) => {
                    warn!(error = %e, "Failed to read /storage/base_path from etcd, using default");
                }
            }
        }

        // Fall back to default
        debug!(path = %self.raw_path, "Using default raw_path");
        self.raw_path.clone()
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:9100".to_string(),
            etcd_endpoints: vec!["http://localhost:2379".to_string()],
            raw_path: "/data/raw".to_string(),
            log_level: "info".to_string(),
            timescale_url: None,
            timescale_max_connections: 5,
            timescale_connect_timeout_secs: 10,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.listen_addr, "0.0.0.0:9100");
        assert_eq!(config.etcd_endpoints, vec!["http://localhost:2379"]);
        assert_eq!(config.raw_path, "/data/raw");
        assert_eq!(config.log_level, "info");
        assert!(config.timescale_url.is_none());
        assert_eq!(config.timescale_max_connections, 5);
        assert_eq!(config.timescale_connect_timeout_secs, 10);
    }

    #[test]
    fn test_host_and_port() {
        let config = AppConfig {
            listen_addr: "127.0.0.1:8080".to_string(),
            ..Default::default()
        };
        assert_eq!(config.host(), "127.0.0.1");
        assert_eq!(config.port(), 8080);
    }

    #[test]
    fn test_validate_empty_endpoints() {
        let config = AppConfig {
            etcd_endpoints: vec![],
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_success() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_has_timescale_false_when_not_configured() {
        let config = AppConfig::default();
        assert!(!config.has_timescale());
    }

    #[test]
    fn test_has_timescale_true_when_configured() {
        let config = AppConfig {
            timescale_url: Some("postgresql://user:pass@localhost:5432/ndp".to_string()),
            ..Default::default()
        };
        assert!(config.has_timescale());
    }

    #[test]
    fn test_timescale_defaults() {
        let config = AppConfig::default();
        assert_eq!(config.timescale_max_connections, 5);
        assert_eq!(config.timescale_connect_timeout_secs, 10);
    }
}
