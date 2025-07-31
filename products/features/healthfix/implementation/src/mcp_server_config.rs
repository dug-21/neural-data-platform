//! Configuration for MCP server with health monitoring support

use serde::{Deserialize, Serialize};

/// MCP Server configuration with degraded mode support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MpcServerConfig {
    /// Allow server to start in degraded mode if some components fail
    pub allow_degraded_mode: bool,
    
    /// Required components for normal operation
    pub required_components: RequiredComponents,
    
    /// Health monitoring configuration
    pub health_monitoring_enabled: bool,
    
    /// Health server port
    pub health_server_port: u16,
}

/// Components required for normal operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredComponents {
    /// Database is required
    pub database: bool,
    
    /// Redis cache is required
    pub redis: bool,
    
    /// Neural predictor is required
    pub neural_predictor: bool,
    
    /// DAA orchestrator is required
    pub daa_orchestrator: bool,
}

impl Default for MpcServerConfig {
    fn default() -> Self {
        Self {
            allow_degraded_mode: false,
            required_components: RequiredComponents::default(),
            health_monitoring_enabled: true,
            health_server_port: 8080,
        }
    }
}

impl Default for RequiredComponents {
    fn default() -> Self {
        Self {
            database: true,
            redis: false, // Cache is optional by default
            neural_predictor: true,
            daa_orchestrator: false, // DAA is optional by default
        }
    }
}

/// Server operational mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationalMode {
    /// All components are healthy
    Normal,
    /// Some non-critical components are unavailable
    Degraded,
    /// Critical components are unavailable (server should not start)
    Failed,
}

impl MpcServerConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            allow_degraded_mode: std::env::var("MCP_ALLOW_DEGRADED_MODE")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            required_components: RequiredComponents {
                database: std::env::var("MCP_REQUIRE_DATABASE")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
                redis: std::env::var("MCP_REQUIRE_REDIS")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()
                    .unwrap_or(false),
                neural_predictor: std::env::var("MCP_REQUIRE_NEURAL")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
                daa_orchestrator: std::env::var("MCP_REQUIRE_DAA")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()
                    .unwrap_or(false),
            },
            health_monitoring_enabled: std::env::var("HEALTH_MONITORING_ENABLED")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            health_server_port: std::env::var("HEALTH_SERVER_PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .unwrap_or(8080),
        }
    }
}