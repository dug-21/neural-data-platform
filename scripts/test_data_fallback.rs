use chrono::{DateTime, Utc, Duration};
use neural_trader::data::storage::TimescaleDBStorage;
use std::env;
use tokio;

/// Test script to verify that the data query fallback logic correctly accesses
/// the raw market_data table when hourly data is insufficient.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing data query fallback logic...");
    
    // Get database URL from environment
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://neural_trader:password@localhost/neural_trader_db".to_string());
    
    println!("Connecting to database: {}", database_url);
    
    // Initialize storage
    let storage = TimescaleDBStorage::new(&database_url).await?;
    
    // Test with a known symbol - should exist in the raw market_data table
    let symbol = "QQQ"; // or another symbol you know exists
    let end_time = Utc::now();
    let start_time = end_time - Duration::days(30); // Request 30 days of data
    
    println!("\nTesting query for symbol: {}", symbol);
    println!("Date range: {} to {}", start_time.format("%Y-%m-%d"), end_time.format("%Y-%m-%d"));
    
    // This should trigger the fallback logic
    let results = storage.query_range(symbol, start_time, end_time).await?;
    
    println!("\nResults:");
    println!("- Total records retrieved: {}", results.len());
    
    if !results.is_empty() {
        let first_record = &results[0];
        let last_record = &results[results.len() - 1];
        
        println!("- Data source: {}", first_record.source);
        println!("- Date range in data: {} to {}", 
                 first_record.timestamp.format("%Y-%m-%d %H:%M:%S"),
                 last_record.timestamp.format("%Y-%m-%d %H:%M:%S"));
        
        // Show sample data
        println!("\nFirst 3 records:");
        for (i, record) in results.iter().take(3).enumerate() {
            if let Some(metadata) = &record.metadata {
                println!("  {}. {} at {}: close={}, OHLCV data available", 
                        i + 1,
                        record.entity,
                        record.timestamp.format("%Y-%m-%d %H:%M:%S"),
                        record.value);
            }
        }
        
        // Verify the source indicates we're using raw data
        if first_record.source == "market_data" {
            println!("\n✅ SUCCESS: Data fallback correctly used raw 'market_data' table");
        } else if first_record.source == "market_data_1h" {
            println!("\n⚠️  INFO: Used hourly data from 'market_data_1h' table");
        } else {
            println!("\n❓ UNKNOWN: Used data from '{}' table", first_record.source);
        }
    } else {
        println!("❌ No data found for symbol {}", symbol);
    }
    
    println!("\nData fallback test completed!");
    Ok(())
}