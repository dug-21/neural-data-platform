//! Test the NeuroDivergentAdapter independently

use std::collections::HashMap;
use chrono::Utc;
use autonomous_platform::data::TimeSeriesData;
use autonomous_platform::adapters::neuro_divergent::{NeuroDivergentAdapter, AdapterConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing NeuroDivergentAdapter...");

    // Create test data
    let mut indicators = HashMap::new();
    indicators.insert("rsi".to_string(), 65.5);
    indicators.insert("macd".to_string(), 0.0012);

    let data = vec![
        TimeSeriesData {
            symbol: "BTC/USD".to_string(),
            timestamp: Utc::now(),
            open: 50000.0,
            high: 51000.0,
            low: 49500.0,
            close: 50500.0,
            volume: 1000.0,
            indicators,
            source: None,
            entity: None,
            value: None,
            metadata: None,
        }
    ];

    // Test DataFrame conversion
    println!("Testing DataFrame conversion...");
    let df = NeuroDivergentAdapter::to_neuro_divergent_df(&data)?;
    println!("DataFrame height: {}", df.height());
    println!("DataFrame columns: {:?}", df.get_column_names());

    // Test adapter creation
    println!("Testing adapter initialization...");
    let mut adapter = NeuroDivergentAdapter::new();

    // Test DeepAR initialization
    match adapter.init_deepar().await {
        Ok(_) => println!("✓ DeepAR initialized successfully"),
        Err(e) => println!("✗ DeepAR initialization failed: {}", e),
    }

    // Test TCN initialization
    match adapter.init_tcn().await {
        Ok(_) => println!("✓ TCN initialized successfully"),
        Err(e) => println!("✗ TCN initialization failed: {}", e),
    }

    println!("Testing complete!");
    Ok(())
}