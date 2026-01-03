//! Application configuration for the Bronze MCP Server.
//!
//! Configuration follows the NDP pattern: Environment variables take precedence
//! with sensible defaults for local development.

use crate::error::{McpError, McpResult};

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
    ///
    /// # Errors
    ///
    /// Returns `McpError::Config` if environment parsing fails.
    pub fn from_env() -> McpResult<Self> {
        Ok(Self {
            listen_addr: std::env::var("NDP_MCP_LISTEN")
                .unwrap_or_else(|_| "0.0.0.0:9100".to_string()),

            etcd_endpoints: std::env::var("NDP_ETCD_ENDPOINTS")
                .unwrap_or_else(|_| "http://localhost:2379".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),

            raw_path: std::env::var("NDP_RAW_PATH")
                .unwrap_or_else(|_| "/data/raw".to_string()),

            log_level: std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "info".to_string()),
        })
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
            return Err(McpError::Config(
                "No etcd endpoints configured".to_string(),
            ));
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
        self.listen_addr
            .split(':')
            .next()
            .unwrap_or("0.0.0.0")
    }

    /// Get the port portion of the listen address.
    pub fn port(&self) -> u16 {
        self.listen_addr
            .split(':')
            .nth(1)
            .and_then(|p| p.parse().ok())
            .unwrap_or(9100)
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:9100".to_string(),
            etcd_endpoints: vec!["http://localhost:2379".to_string()],
            raw_path: "/data/raw".to_string(),
            log_level: "info".to_string(),
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
}
