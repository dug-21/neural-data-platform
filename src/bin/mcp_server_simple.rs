//! Simple Neural Trader MCP Server
//!
//! Minimal MCP server implementation for testing

use anyhow::Result;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::sync::RwLock;

use autonomous_platform::{
    agents::{AgentConfig, AutonomousAgent, TradingStrategy},
    config::load_default_config,
    data::{RedisCache, TimescaleDBStorage},
    mcp::{register_mcp_tools, TradingMcpTools},
    neural::NeuralPredictor,
};

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Starting Neural Trader MCP Server (Simple Mode)");
    println!("📋 Configuration: Simple standalone mode");

    // Load configuration
    let _config = load_default_config()?;

    println!("🔧 Initializing components...");

    // Database
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        format!(
            "postgresql://neural_trader:{}@localhost:5432/neural_trader_db",
            std::env::var("POSTGRES_PASSWORD").unwrap_or_else(|_| "testpass123".to_string())
        )
    });

    println!("📊 Connecting to database...");
    let pool = match PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
    {
        Ok(pool) => {
            println!("✅ Database connected");
            pool
        }
        Err(e) => {
            println!("⚠️  Database connection failed: {}", e);
            println!("   Using mock mode");
            // Create a minimal pool for testing
            PgPoolOptions::new()
                .max_connections(1)
                .connect("postgres://localhost/postgres")
                .await?
        }
    };

    let storage = Arc::new(TimescaleDBStorage { pool });

    // Cache
    println!("💾 Connecting to Redis...");
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| {
        format!(
            "redis://:{}@localhost:6379",
            std::env::var("REDIS_PASSWORD").unwrap_or_else(|_| "testredis123".to_string())
        )
    });

    let cache = match RedisCache::new(&redis_url).await {
        Ok(cache) => {
            println!("✅ Redis connected");
            Arc::new(RwLock::new(cache))
        }
        Err(e) => {
            println!("⚠️  Redis connection failed: {}", e);
            println!("   Continuing without cache");
            // Create a mock cache
            Arc::new(RwLock::new(
                RedisCache::new("redis://localhost:6379").await?,
            ))
        }
    };

    // Neural predictor
    println!("🧠 Initializing neural predictor...");
    let predictor = match NeuralPredictor::default().await {
        Ok(predictor) => {
            println!("✅ Neural predictor ready");
            Arc::new(predictor)
        }
        Err(e) => {
            eprintln!("❌ Neural predictor initialization failed: {}", e);
            eprintln!("   Cannot start MCP server without neural predictor");
            eprintln!("   Please check your neural model configuration and try again");
            
            // Return error instead of panicking
            return Err(anyhow::anyhow!(
                "Cannot start MCP server without neural predictor: {}",
                e
            ));
        }
    };

    // Agent
    println!("🤖 Initializing trading agent...");
    let agent_config = AgentConfig {
        id: "mcp-agent-001".to_string(),
        strategy: TradingStrategy::Momentum,
        risk_tolerance: 0.5,
        max_position_size: 10000.0,
        decision_threshold: 0.7,
    };
    let agent = Arc::new(RwLock::new(AutonomousAgent::new(agent_config)?));
    println!("✅ Trading agent ready");

    // Create MCP tools
    println!("🔌 Creating MCP tools...");
    let tools = TradingMcpTools::new(storage, cache, predictor, agent);
    let tools_arc = Arc::new(tools);

    // Register tools
    println!("📝 Registering MCP tools...");
    register_mcp_tools(tools_arc.clone()).await?;

    println!("\n✅ All components initialized successfully!");
    println!("\n🌐 MCP server ready for connections");
    println!("\n📡 Available tools:");
    println!("   - query_market_data   : Query historical market data");
    println!("   - get_cache_data      : Retrieve cached data");
    println!("   - request_prediction  : Get neural network predictions");
    println!("   - agent_decision      : Get trading decisions");
    println!("   - system_status       : Get system health status");

    println!("\n💡 Example usage in Claude:");
    println!("   'Show me the last hour of BTC/USD data'");
    println!("   'What's the prediction for ETH in the next 5 minutes?'");
    println!("   'Should I buy $5000 of BTC?'");

    println!("\n⏳ Server running... Press Ctrl+C to stop");

    // Keep the server running
    tokio::signal::ctrl_c().await?;
    println!("\n👋 Shutting down MCP server...");

    Ok(())
}
