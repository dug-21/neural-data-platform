use mcp_trading_server::MCPTradingServer;
use tracing::error;

#[tokio::main]
async fn main() {
    match MCPTradingServer::new().await {
        Ok(server) => {
            if let Err(e) = server.start().await {
                error!("Failed to start MCP server: {}", e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            error!("Failed to initialize MCP server: {}", e);
            std::process::exit(1);
        }
    }
}