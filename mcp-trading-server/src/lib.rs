pub mod tools;
pub mod integrations;
pub mod models;
pub mod error;
pub mod config;

use mcp_sdk::{Server, Tool, ToolHandler};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error};

use crate::tools::{
    market_data::MarketDataTool,
    cache::CacheTool,
    neural::NeuralPredictionTool,
    trading::TradingDecisionTool,
    health::HealthMonitorTool,
};

use crate::integrations::{
    database::DatabaseClient,
    redis::RedisClient,
    neural::NeuralClient,
    agent::AgentClient,
    monitor::MonitorClient,
};

pub struct MCPTradingServer {
    server: Server,
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
        
        // Create MCP server
        let mut server = Server::new("mcp-trading-server", "0.1.0");
        
        // Register tools
        info!("Registering MCP tools...");
        
        // Market Data Tools
        server.register_tool(
            Tool::new("get_latest_price", "Get the latest price for a trading symbol")
                .with_parameter("symbol", "Trading symbol (e.g., BTC/USD)", true)
        );
        
        server.register_tool(
            Tool::new("get_historical_data", "Get historical price data")
                .with_parameter("symbol", "Trading symbol", true)
                .with_parameter("interval", "Time interval (1m, 5m, 1h, 1d)", true)
                .with_parameter("start_time", "Start time (ISO 8601)", true)
                .with_parameter("end_time", "End time (ISO 8601)", true)
        );
        
        server.register_tool(
            Tool::new("get_orderbook", "Get current orderbook data")
                .with_parameter("symbol", "Trading symbol", true)
                .with_parameter("depth", "Orderbook depth", false)
        );
        
        // Cache Tools
        server.register_tool(
            Tool::new("get_cached_price", "Get cached price data for fast access")
                .with_parameter("symbol", "Trading symbol", true)
        );
        
        server.register_tool(
            Tool::new("get_cached_indicators", "Get cached technical indicators")
                .with_parameter("symbol", "Trading symbol", true)
        );
        
        // Neural Prediction Tools
        server.register_tool(
            Tool::new("get_price_prediction", "Get neural network price predictions")
                .with_parameter("symbol", "Trading symbol", true)
                .with_parameter("timeframe", "Prediction timeframe", true)
                .with_parameter("periods", "Number of periods to predict", true)
        );
        
        server.register_tool(
            Tool::new("get_trend_analysis", "Get AI-powered trend analysis")
                .with_parameter("symbol", "Trading symbol", true)
        );
        
        server.register_tool(
            Tool::new("get_pattern_recognition", "Detect chart patterns using neural networks")
                .with_parameter("symbol", "Trading symbol", true)
                .with_parameter("timeframe", "Analysis timeframe", true)
        );
        
        // Trading Decision Tools
        server.register_tool(
            Tool::new("get_trading_signal", "Get trading signal from agent")
                .with_parameter("symbol", "Trading symbol", true)
        );
        
        server.register_tool(
            Tool::new("execute_trade", "Execute a trade order")
                .with_parameter("symbol", "Trading symbol", true)
                .with_parameter("side", "Order side (buy/sell)", true)
                .with_parameter("quantity", "Order quantity", true)
                .with_parameter("price", "Limit price (optional)", false)
                .with_parameter("take_profit", "Take profit price", false)
                .with_parameter("stop_loss", "Stop loss price", false)
        );
        
        server.register_tool(
            Tool::new("get_portfolio_status", "Get current portfolio status")
        );
        
        // Health Monitoring Tools
        server.register_tool(
            Tool::new("get_system_status", "Get overall system health status")
        );
        
        server.register_tool(
            Tool::new("get_component_health", "Get health status of specific component")
                .with_parameter("component", "Component name (database/redis/neural/agents)", true)
        );
        
        server.register_tool(
            Tool::new("get_performance_metrics", "Get system performance metrics")
                .with_parameter("timeframe", "Metrics timeframe (1m, 5m, 1h)", false)
        );
        
        Ok(Self {
            server,
            db_client,
            redis_client,
            neural_client,
            agent_client,
            monitor_client,
        })
    }
    
    pub async fn start(&self) -> Result<(), error::Error> {
        info!("Starting MCP Trading Server...");
        
        // Create tool handlers
        let handlers = self.create_handlers();
        
        // Start server with stdio transport
        self.server.start_stdio(handlers).await?;
        
        Ok(())
    }
    
    fn create_handlers(&self) -> Vec<Box<dyn ToolHandler>> {
        vec![
            // Market data handlers
            Box::new(MarketDataTool::new(self.db_client.clone())),
            
            // Cache handlers
            Box::new(CacheTool::new(self.redis_client.clone())),
            
            // Neural prediction handlers
            Box::new(NeuralPredictionTool::new(self.neural_client.clone())),
            
            // Trading decision handlers
            Box::new(TradingDecisionTool::new(self.agent_client.clone())),
            
            // Health monitoring handlers
            Box::new(HealthMonitorTool::new(self.monitor_client.clone())),
        ]
    }
}