// Test to validate config-store can handle market hours configuration

use config_store::{ConfigValue, ConfigStore, InMemoryConfigStore};
use std::collections::HashMap;

#[tokio::test]
async fn test_market_hours_storage() {
    // Create an in-memory config store
    let store = InMemoryConfigStore::new();
    
    // Create a complex market hours configuration
    let mut market_hours = HashMap::new();
    
    // NYSE configuration
    let mut nyse = HashMap::new();
    nyse.insert("timezone".to_string(), ConfigValue::String("America/New_York".to_string()));
    nyse.insert("dst_observed".to_string(), ConfigValue::Boolean(true));
    
    // Regular hours
    let mut regular_hours = HashMap::new();
    regular_hours.insert("open".to_string(), ConfigValue::String("09:30".to_string()));
    regular_hours.insert("close".to_string(), ConfigValue::String("16:00".to_string()));
    nyse.insert("regular_hours".to_string(), ConfigValue::Object(regular_hours));
    
    // Extended hours
    let mut extended_hours = HashMap::new();
    extended_hours.insert("pre_market_open".to_string(), ConfigValue::String("04:00".to_string()));
    extended_hours.insert("pre_market_close".to_string(), ConfigValue::String("09:30".to_string()));
    extended_hours.insert("after_hours_open".to_string(), ConfigValue::String("16:00".to_string()));
    extended_hours.insert("after_hours_close".to_string(), ConfigValue::String("20:00".to_string()));
    nyse.insert("extended_hours".to_string(), ConfigValue::Object(extended_hours));
    
    // Trading days
    let trading_days = vec![
        ConfigValue::String("Monday".to_string()),
        ConfigValue::String("Tuesday".to_string()),
        ConfigValue::String("Wednesday".to_string()),
        ConfigValue::String("Thursday".to_string()),
        ConfigValue::String("Friday".to_string()),
    ];
    nyse.insert("trading_days".to_string(), ConfigValue::Array(trading_days));
    
    // Holidays
    let mut holidays = HashMap::new();
    
    // Fixed holidays
    let fixed_holidays = vec![
        ConfigValue::Object({
            let mut holiday = HashMap::new();
            holiday.insert("date".to_string(), ConfigValue::String("2024-01-01".to_string()));
            holiday.insert("name".to_string(), ConfigValue::String("New Year's Day".to_string()));
            holiday
        }),
        ConfigValue::Object({
            let mut holiday = HashMap::new();
            holiday.insert("date".to_string(), ConfigValue::String("2024-07-04".to_string()));
            holiday.insert("name".to_string(), ConfigValue::String("Independence Day".to_string()));
            holiday
        }),
        ConfigValue::Object({
            let mut holiday = HashMap::new();
            holiday.insert("date".to_string(), ConfigValue::String("2024-12-25".to_string()));
            holiday.insert("name".to_string(), ConfigValue::String("Christmas".to_string()));
            holiday
        }),
    ];
    holidays.insert("fixed".to_string(), ConfigValue::Array(fixed_holidays));
    
    // Moveable holidays (calculated in code)
    let moveable_holidays = vec![
        ConfigValue::String("GOOD_FRIDAY".to_string()),
        ConfigValue::String("THANKSGIVING".to_string()),
    ];
    holidays.insert("moveable".to_string(), ConfigValue::Array(moveable_holidays));
    
    nyse.insert("holidays".to_string(), ConfigValue::Object(holidays));
    
    // Add NYSE to market_hours
    market_hours.insert("NYSE".to_string(), ConfigValue::Object(nyse));
    
    // Store the configuration (paths must start with /)
    let config = ConfigValue::Object(market_hours);
    store.set("/market_hours", config.clone()).await.unwrap();
    
    // Retrieve and verify
    let retrieved = store.get("/market_hours").await.unwrap();
    assert_eq!(retrieved, config);
    
    // Test nested access
    let nyse_config = store.get("/market_hours/NYSE").await.unwrap();
    assert!(nyse_config.is_object());
    
    let timezone = store.get("/market_hours/NYSE/timezone").await.unwrap();
    assert_eq!(timezone.as_string(), Some("America/New_York"));
    
    let open_time = store.get("/market_hours/NYSE/regular_hours/open").await.unwrap();
    assert_eq!(open_time.as_string(), Some("09:30"));
    
    println!("✅ Config-store successfully handles complex market hours configuration!");
}

#[tokio::test]
async fn test_json_serialization() {
    let store = InMemoryConfigStore::new();
    
    // Test that config can be serialized to/from JSON
    let mut config = HashMap::new();
    let mut exchange = HashMap::new();
    
    exchange.insert("timezone".to_string(), ConfigValue::String("UTC-5".to_string()));
    exchange.insert("open_time".to_string(), ConfigValue::String("09:30".to_string()));
    exchange.insert("close_time".to_string(), ConfigValue::String("16:00".to_string()));
    exchange.insert("dst_observed".to_string(), ConfigValue::Boolean(true));
    exchange.insert("trading_days_per_year".to_string(), ConfigValue::Integer(252));
    exchange.insert("average_volume".to_string(), ConfigValue::Float(1_000_000_000.0));
    
    config.insert("NYSE".to_string(), ConfigValue::Object(exchange));
    
    let value = ConfigValue::Object(config);
    
    // Store and retrieve (paths must start with /)
    store.set("/test_exchange", value.clone()).await.unwrap();
    let retrieved = store.get("/test_exchange").await.unwrap();
    
    // Serialize to JSON string
    let json = serde_json::to_string_pretty(&retrieved).unwrap();
    println!("JSON representation:\n{}", json);
    
    // Parse back from JSON
    let parsed: ConfigValue = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, value);
    
    println!("✅ Config-store handles JSON serialization perfectly!");
}

fn main() {
    println!("Testing config-store capability for market hours data...");
}