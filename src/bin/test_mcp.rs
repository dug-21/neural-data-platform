//! Simple test program for MCP functionality

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Neural Trader MCP Test");
    println!("=========================");
    
    // Test database connection
    println!("\n📊 Testing database connection...");
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://neural_trader:testpass123@localhost:5432/neural_trader_db".to_string());
    
    match sqlx::PgPool::connect(&database_url).await {
        Ok(pool) => {
            println!("✅ Database connected successfully!");
            
            // Query test data
            let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM market_data")
                .fetch_one(&pool)
                .await?;
            
            println!("📈 Found {} rows in market_data table", row.0);
        }
        Err(e) => {
            println!("❌ Database connection failed: {}", e);
        }
    }
    
    // Test Redis connection
    println!("\n💾 Testing Redis connection...");
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://:testredis123@localhost:6379".to_string());
    
    match redis::Client::open(redis_url) {
        Ok(client) => {
            match client.get_connection() {
                Ok(mut conn) => {
                    println!("✅ Redis connected successfully!");
                    
                    // Get test data
                    use redis::Commands;
                    let value: Option<String> = conn.get("market:btc:latest").ok();
                    if let Some(data) = value {
                        println!("📊 BTC latest data: {}", data);
                    }
                }
                Err(e) => {
                    println!("❌ Redis connection failed: {}", e);
                }
            }
        }
        Err(e) => {
            println!("❌ Redis client error: {}", e);
        }
    }
    
    println!("\n✨ MCP test complete!");
    println!("\nTo start the full MCP server, fix the compilation errors and run:");
    println!("  cargo run --bin mcp_server");
    
    Ok(())
}