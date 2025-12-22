//! MCP Server main entry point for air-quality-mcp binary

use air_quality_app::mcp::McpServer;
use tracing_subscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    // Create and start MCP server
    let server = McpServer::new().await?;
    server.start().await?;

    // Keep server running
    tokio::signal::ctrl_c().await?;

    Ok(())
}
