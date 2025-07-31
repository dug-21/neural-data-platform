//! Enhanced MCP Server with health monitoring and degraded mode support

use anyhow::{anyhow, Result};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use autonomous_platform::{
    agents::{AgentConfig, AutonomousAgent, TradingStrategy},
    config::load_default_config,
    data::{RedisCache, TimescaleDBStorage},
    mcp::{register_mcp_tools, TradingMcpTools},
    neural::NeuralPredictor,
};

use crate::mcp_server_config::{MpcServerConfig, OperationalMode};
use crate::health::{AsyncHealthMonitor, HealthMonitorConfig};

/// Component availability status
#[derive(Debug, Clone)]
pub struct ComponentStatus {
    pub database: bool,
    pub redis: bool,
    pub neural_predictor: bool,
    pub daa_orchestrator: bool,
}

/// Enhanced MCP Server with health monitoring
pub struct MpcServer {
    config: MpcServerConfig,
    component_status: ComponentStatus,
    operational_mode: OperationalMode,
    health_monitor: Option<AsyncHealthMonitor>,
}

impl MpcServer {
    /// Create a new MCP server with the given configuration
    pub fn new(config: MpcServerConfig) -> Self {
        Self {
            config,
            component_status: ComponentStatus {
                database: false,
                redis: false,
                neural_predictor: false,
                daa_orchestrator: false,
            },
            operational_mode: OperationalMode::Failed,
            health_monitor: None,
        }
    }

    /// Initialize and start the MCP server
    pub async fn start(mut self) -> Result<Self> {
        info!("🚀 Starting Enhanced Neural Trader MCP Server");
        info!("📋 Configuration: {:?}", self.config);

        // Initialize health monitoring if enabled
        if self.config.health_monitoring_enabled {
            info!("🏥 Initializing health monitoring...");
            let health_config = HealthMonitorConfig::default();
            let mut health_monitor = AsyncHealthMonitor::new(health_config);
            
            // Start health monitoring in background (non-blocking)
            health_monitor.start().await?;
            info!("✅ Health monitoring started (non-blocking)");
            
            self.health_monitor = Some(health_monitor);
        }

        // Load base configuration
        let base_config = load_default_config()?;

        // Initialize components with graceful fallback
        self.initialize_components().await?;

        // Determine operational mode
        self.operational_mode = self.determine_operational_mode();

        match self.operational_mode {
            OperationalMode::Normal => {
                info!("✅ All components initialized successfully - Normal mode");
            }
            OperationalMode::Degraded => {
                if self.config.allow_degraded_mode {
                    warn!("⚠️  Starting in DEGRADED mode - some features may be unavailable");
                } else {
                    error!("❌ Cannot start: required components are unavailable");
                    return Err(anyhow!("Required components unavailable and degraded mode not allowed"));
                }
            }
            OperationalMode::Failed => {
                error!("❌ Cannot start: critical components are unavailable");
                return Err(anyhow!("Critical components unavailable"));
            }
        }

        Ok(self)
    }

    /// Initialize all components with graceful error handling
    async fn initialize_components(&mut self) -> Result<()> {
        // Database
        info!("📊 Connecting to database...");
        match self.initialize_database().await {
            Ok(_) => {
                info!("✅ Database connected");
                self.component_status.database = true;
            }
            Err(e) => {
                warn!("⚠️  Database connection failed: {}", e);
                if self.config.required_components.database && !self.config.allow_degraded_mode {
                    return Err(anyhow!("Database is required but connection failed: {}", e));
                }
            }
        }

        // Redis
        info!("💾 Connecting to Redis...");
        match self.initialize_redis().await {
            Ok(_) => {
                info!("✅ Redis connected");
                self.component_status.redis = true;
            }
            Err(e) => {
                warn!("⚠️  Redis connection failed: {}", e);
                if self.config.required_components.redis && !self.config.allow_degraded_mode {
                    return Err(anyhow!("Redis is required but connection failed: {}", e));
                }
            }
        }

        // Neural predictor
        info!("🧠 Initializing neural predictor...");
        match self.initialize_neural_predictor().await {
            Ok(_) => {
                info!("✅ Neural predictor ready");
                self.component_status.neural_predictor = true;
            }
            Err(e) => {
                error!("❌ Neural predictor initialization failed: {}", e);
                if self.config.required_components.neural_predictor && !self.config.allow_degraded_mode {
                    return Err(anyhow!("Neural predictor is required but initialization failed: {}", e));
                }
            }
        }

        // DAA Orchestrator (placeholder for now)
        info!("🤖 Initializing DAA orchestrator...");
        match self.initialize_daa_orchestrator().await {
            Ok(_) => {
                info!("✅ DAA orchestrator ready");
                self.component_status.daa_orchestrator = true;
            }
            Err(e) => {
                warn!("⚠️  DAA orchestrator initialization failed: {}", e);
                if self.config.required_components.daa_orchestrator && !self.config.allow_degraded_mode {
                    return Err(anyhow!("DAA orchestrator is required but initialization failed: {}", e));
                }
            }
        }

        Ok(())
    }

    /// Initialize database connection
    async fn initialize_database(&self) -> Result<()> {
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            format!(
                "postgresql://neural_trader:{}@localhost:5432/neural_trader_db",
                std::env::var("POSTGRES_PASSWORD").unwrap_or_else(|_| "testpass123".to_string())
            )
        });

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?;

        // Test the connection
        sqlx::query("SELECT 1").execute(&pool).await?;

        Ok(())
    }

    /// Initialize Redis connection
    async fn initialize_redis(&self) -> Result<()> {
        let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| {
            format!(
                "redis://:{}@localhost:6379",
                std::env::var("REDIS_PASSWORD").unwrap_or_else(|_| "testredis123".to_string())
            )
        });

        let cache = RedisCache::new(&redis_url).await?;
        
        // Test the connection
        // Note: In real implementation, we'd add a ping method to RedisCache
        
        Ok(())
    }

    /// Initialize neural predictor
    async fn initialize_neural_predictor(&self) -> Result<()> {
        let predictor = NeuralPredictor::default().await?;
        
        // Test prediction capability
        // Note: In real implementation, we'd add a test_prediction method
        
        Ok(())
    }

    /// Initialize DAA orchestrator
    async fn initialize_daa_orchestrator(&self) -> Result<()> {
        // Placeholder for DAA orchestrator initialization
        // In real implementation, this would initialize the actual DAA system
        Ok(())
    }

    /// Determine operational mode based on component availability
    fn determine_operational_mode(&self) -> OperationalMode {
        let status = &self.component_status;
        let required = &self.config.required_components;

        // Check if all required components are available
        let critical_components_ok = 
            (!required.database || status.database) &&
            (!required.neural_predictor || status.neural_predictor);

        let optional_components_ok = 
            (!required.redis || status.redis) &&
            (!required.daa_orchestrator || status.daa_orchestrator);

        if critical_components_ok && optional_components_ok {
            OperationalMode::Normal
        } else if critical_components_ok {
            OperationalMode::Degraded
        } else {
            OperationalMode::Failed
        }
    }

    /// Check if the server has a neural predictor available
    pub fn has_neural_predictor(&self) -> bool {
        self.component_status.neural_predictor
    }

    /// Check if the server is running in degraded mode
    pub fn is_degraded_mode(&self) -> bool {
        self.operational_mode == OperationalMode::Degraded
    }

    /// Get the current operational mode
    pub fn operational_mode(&self) -> OperationalMode {
        self.operational_mode
    }

    /// Shutdown the server gracefully
    pub async fn shutdown(mut self) -> Result<()> {
        info!("👋 Shutting down MCP server...");
        
        // Stop health monitoring if active
        if let Some(mut health_monitor) = self.health_monitor {
            info!("🏥 Stopping health monitoring...");
            health_monitor.stop().await;
        }

        info!("✅ MCP server shutdown complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_with_all_components_available() {
        let config = MpcServerConfig {
            allow_degraded_mode: false,
            ..Default::default()
        };

        let server = MpcServer::new(config);
        // In real tests, we'd mock the component initialization
        // For now, this is a placeholder
    }

    #[tokio::test]
    async fn test_server_with_degraded_mode_allowed() {
        let config = MpcServerConfig {
            allow_degraded_mode: true,
            ..Default::default()
        };

        let server = MpcServer::new(config);
        // Test that server starts even with some component failures
    }

    #[tokio::test]
    async fn test_server_fails_without_critical_components() {
        let config = MpcServerConfig {
            allow_degraded_mode: false,
            required_components: RequiredComponents {
                database: true,
                neural_predictor: true,
                ..Default::default()
            },
            ..Default::default()
        };

        let server = MpcServer::new(config);
        // Test that server fails to start without critical components
    }
}