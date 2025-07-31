pub mod config;
pub mod error;
pub mod handlers;
pub mod integrations;
pub mod models;
pub mod tools;

use std::sync::Arc;
use tracing::info;

use crate::integrations::{
    agent::AgentClient, database::DatabaseClient, monitor::MonitorClient, neural::NeuralClient,
    redis::RedisClient,
};

pub struct MCPTradingServer {
    db_client: Arc<DatabaseClient>,
    redis_client: Arc<RedisClient>,
    neural_client: Arc<NeuralClient>,
    agent_client: Arc<AgentClient>,
    monitor_client: Arc<MonitorClient>,
}

impl MCPTradingServer {
    pub async fn new() -> Result<Self, error::Error> {
        // Initialize logging
        tracing_subscriber::fmt::init();

        // Load configuration
        let config = config::Config::from_env()?;

        // Initialize clients
        info!("Initializing database client...");
        let db_client = Arc::new(DatabaseClient::new(&config.database_url).await?);

        info!("Initializing Redis client...");
        let redis_client = Arc::new(RedisClient::new(&config.redis_url).await?);

        info!("Initializing neural client...");
        let neural_client = Arc::new(NeuralClient::new(&config.neural_service_url).await?);

        info!("Initializing agent client...");
        let agent_client = Arc::new(AgentClient::new(&config.agent_service_url).await?);

        info!("Initializing monitor client...");
        let monitor_client = Arc::new(MonitorClient::new());

        // TODO: Integrate MCP SDK 0.0.3 once proper documentation is available
        // The examples found online are for rust-mcp-sdk which has a different API
        info!("MCP server integration pending - handlers are ready");

        Ok(Self {
            db_client,
            redis_client,
            neural_client,
            agent_client,
            monitor_client,
        })
    }

    pub async fn start(&self) -> Result<(), error::Error> {
        info!("Starting MCP Trading Server...");

        // TODO: Properly start MCP server with stdio transport
        // For now, the handlers are available for direct use

        Ok(())
    }

    // Direct handler methods for testing
    pub fn get_db_client(&self) -> &Arc<DatabaseClient> {
        &self.db_client
    }

    pub fn get_redis_client(&self) -> &Arc<RedisClient> {
        &self.redis_client
    }

    pub fn get_neural_client(&self) -> &Arc<NeuralClient> {
        &self.neural_client
    }

    pub fn get_agent_client(&self) -> &Arc<AgentClient> {
        &self.agent_client
    }

    pub fn get_monitor_client(&self) -> &Arc<MonitorClient> {
        &self.monitor_client
    }
}
