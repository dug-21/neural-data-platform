//! Example demonstrating config-store usage in neural-trader

use config_store::{ConfigStoreBuilder, ConfigStore, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
struct DatabaseConfig {
    host: String,
    port: u16,
    username: String,
    password: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TradingConfig {
    max_position_size: f64,
    risk_tolerance: f64,
    enable_paper_trading: bool,
}

fn main() -> Result<()> {
    println!("Neural Trader - Config Store Usage Example");

    // Create default configuration values
    let mut defaults = HashMap::new();
    defaults.insert(
        "database.host".to_string(),
        serde_json::Value::String("localhost".to_string()),
    );
    defaults.insert(
        "database.port".to_string(),
        serde_json::Value::Number(5432.into()),
    );
    defaults.insert(
        "trading.max_position_size".to_string(),
        serde_json::Value::Number(serde_json::Number::from_f64(10000.0).unwrap()),
    );
    defaults.insert(
        "trading.risk_tolerance".to_string(),
        serde_json::Value::Number(serde_json::Number::from_f64(0.02).unwrap()),
    );
    defaults.insert(
        "trading.enable_paper_trading".to_string(),
        serde_json::Value::Bool(true),
    );

    // Build configuration store with defaults and environment variables
    let mut store = ConfigStoreBuilder::new()
        .add_defaults(defaults)
        .add_env("NEURAL_TRADER") // Will read NEURAL_TRADER_* environment variables
        .build()?;

    // Store some runtime configuration
    store.store("runtime.startup_time", &chrono::Utc::now().timestamp())?;
    store.store("runtime.version", &"0.1.0")?;

    // Load individual configuration values
    let db_host: String = store.load("database.host")?;
    let db_port: u16 = store.load("database.port")?;
    let max_position: f64 = store.load("trading.max_position_size")?;
    let risk_tolerance: f64 = store.load("trading.risk_tolerance")?;
    let paper_trading: bool = store.load("trading.enable_paper_trading")?;

    println!("Configuration loaded:");
    println!("  Database: {}:{}", db_host, db_port);
    println!("  Max Position Size: ${:.2}", max_position);
    println!("  Risk Tolerance: {:.2}%", risk_tolerance * 100.0);
    println!("  Paper Trading: {}", if paper_trading { "enabled" } else { "disabled" });

    // Load structured configuration (if all fields are present)
    if store.exists("database.username") && store.exists("database.password") {
        let db_config = DatabaseConfig {
            host: store.load("database.host")?,
            port: store.load("database.port")?,
            username: store.load("database.username")?,
            password: store.load("database.password")?,
        };
        println!("  Database config: {:?}", db_config);
    } else {
        println!("  Database username/password not configured");
    }

    // Show all configuration keys
    println!("\nAll configuration keys:");
    for key in store.keys() {
        if let Some(value) = store.get_raw(&key) {
            println!("  {}: {:?}", key, value);
        }
    }

    // Get nested configuration as JSON object
    let nested_config = store.as_nested_object();
    println!("\nNested configuration:");
    println!("{}", serde_json::to_string_pretty(&nested_config)?);

    println!("\nExample completed successfully!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_store_basic_usage() {
        let store = ConfigStoreBuilder::new().build().unwrap();
        assert_eq!(store.keys().len(), 0);
    }

    #[test]
    fn test_config_store_with_defaults() {
        let mut defaults = HashMap::new();
        defaults.insert("test.value".to_string(), serde_json::Value::String("default".to_string()));

        let store = ConfigStoreBuilder::new()
            .add_defaults(defaults)
            .build()
            .unwrap();

        let value: String = store.load("test.value").unwrap();
        assert_eq!(value, "default");
    }
}