//! Configuration Parsing Demo
//!
//! This example demonstrates how to use the configuration utility functions
//! to parse Redis and PostgreSQL URLs and convert them to adapter configurations.

use autonomous_platform::config::PlatformConfig;
use autonomous_platform::orchestration::{
    config_bridge::ConfigBridge,
    config_utils::{build_postgres_url, build_redis_url, parse_postgres_url, parse_redis_url},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Configuration Parsing Demo");
    println!("================================");

    // Example 1: Parse Redis URL
    println!("\n📡 Redis URL Parsing:");
    let redis_url = "redis://:mypassword@localhost:6379/1";
    println!("URL: {}", redis_url);

    match parse_redis_url(redis_url) {
        Ok(config) => {
            println!("✅ Parsed successfully:");
            println!("   Host: {}", config.host);
            println!("   Port: {}", config.port);
            println!("   Password: {:?}", config.password);
            println!("   Database: {}", config.db);
            println!("   Pool Size: {}", config.pool_size);
        }
        Err(e) => println!("❌ Failed to parse: {}", e),
    }

    // Example 2: Parse PostgreSQL URL
    println!("\n🐘 PostgreSQL URL Parsing:");
    let postgres_url = "postgres://user:secret@localhost:5432/trading_db";
    println!("URL: {}", postgres_url);

    match parse_postgres_url(postgres_url) {
        Ok(config) => {
            println!("✅ Parsed successfully:");
            println!("   Host: {}", config.host);
            println!("   Port: {}", config.port);
            println!("   Username: {}", config.username);
            println!("   Password: {}", config.password);
            println!("   Database: {}", config.database);
            println!("   Max Connections: {}", config.max_connections);
        }
        Err(e) => println!("❌ Failed to parse: {}", e),
    }

    // Example 3: Build URLs from components
    println!("\n🔨 URL Building:");
    let rebuilt_redis = build_redis_url("localhost", 6379, Some("mypassword"), 1);
    println!("Built Redis URL: {}", rebuilt_redis);

    let rebuilt_postgres = build_postgres_url("localhost", 5432, "user", "secret", "trading_db");
    println!("Built PostgreSQL URL: {}", rebuilt_postgres);

    // Example 4: Configuration Bridge Demo (if platform config exists)
    println!("\n🌉 Configuration Bridge Demo:");
    println!("The ConfigBridge can convert platform configuration URLs to adapter configs:");
    println!("  ConfigBridge::redis_config_from_platform(&platform_config)");
    println!("  ConfigBridge::timescale_config_from_platform(&platform_config)");
    println!("  ConfigBridge::validate_connection_urls(&platform_config)");

    // Example 5: Error handling
    println!("\n⚠️  Error Handling Demo:");
    let invalid_redis = "http://not-redis:6379";
    match parse_redis_url(invalid_redis) {
        Ok(_) => println!("❌ Should have failed!"),
        Err(e) => println!("✅ Correctly rejected invalid URL: {}", e),
    }

    println!("\n✨ Demo completed successfully!");
    Ok(())
}
