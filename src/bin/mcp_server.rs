//! Neural Trader MCP Server
//! 
//! Standalone MCP server for the Neural Trader platform

use anyhow::Result;
// use clap::{Arg, Command}; // Commented out - add clap to Cargo.toml if needed
use std::sync::Arc;
use tokio::sync::RwLock;

use autonomous_platform::{
    neural::NeuralPredictor,
    agents::AutonomousAgent,
    monitoring::HealthMonitor,
    mcp::{TradingMcpTools, register_mcp_tools},
};

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Starting Neural Trader MCP Server");
    
    // Initialize components
    println!("🔧 Initializing components...");
    
    // Health monitor
    let monitor = Arc::new(HealthMonitor::new().await?);
    monitor.start_monitoring().await?;
    
    // Create MCP tools
    let tools = TradingMcpTools::with_monitor(monitor).await?;
    let tools_arc = Arc::new(tools);
    
    // Register tools
    register_mcp_tools(tools_arc.clone()).await?;
    
    println!("✅ All components initialized successfully");
    println!("🌐 MCP server ready for connections");
    println!("📡 Available tools:");
    println!("   - query_market_data");
    println!("   - get_cache_data");
    println!("   - request_prediction");
    println!("   - agent_decision");
    println!("   - system_status");
    
    // Keep the server running
    tokio::signal::ctrl_c().await?;
    println!("\n👋 Shutting down MCP server...");
    
    Ok(())
}