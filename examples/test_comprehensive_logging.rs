use std::collections::HashMap;
use chrono::{DateTime, Utc};
use neural_trader::data::TimeSeriesData;
use neural_trader::neural::vendor_predictor::{VendorPredictor, VendorPredictorConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    println!("🚀 Testing Comprehensive Symbol Data Loading Logging");
    println!("=====================================================");
    
    // Create VendorPredictor instance
    let config = VendorPredictorConfig {
        enable_performance_tracking: true,
        enable_sector_routing: true,
        ..Default::default()
    };
    
    let predictor = VendorPredictor::new(config).await?;
    
    // Test different symbol types with comprehensive logging
    let test_symbols = vec![
        ("AAPL", "Individual Stock"),
        ("SPY", "ETF"),
        ("XLK", "Sector ETF"),
        ("BTCUSD", "Cryptocurrency"),
        ("TECH_SECTOR", "Custom Sector"),
    ];
    
    for (symbol, expected_type) in test_symbols {
        println!("\n🔍 Testing symbol: {} (Expected: {})", symbol, expected_type);
        println!("================================================");
        
        // Create sample training data
        let mut training_data = Vec::new();
        let base_time = Utc::now();
        
        for i in 0..100 {
            let timestamp = base_time - chrono::Duration::hours(i as i64);
            let base_price = 150.0 + (i as f64 * 0.5) + (i as f64 / 10.0).sin() * 10.0;
            
            let data_point = TimeSeriesData {
                timestamp,
                symbol: symbol.to_string(),
                open: base_price,
                high: base_price + 2.0,
                low: base_price - 1.5,
                close: base_price + 0.5,
                volume: vec![1000000.0 + (i * 10000) as f64],
                volume_value: 1000000.0 + (i * 10000) as f64,
                indicators: HashMap::new(),
                source: Some("test".to_string()),
                entity: Some(symbol.to_string()),
                value: Some(base_price + 0.5),
                metadata: None,
                values: vec![base_price + 0.5],
                intervals: vec![3600], // 1 hour intervals
                timestamps: vec![timestamp],
                metadata_map: HashMap::new(),
            };
            training_data.push(data_point);
        }
        
        // Call train_model to trigger comprehensive logging
        match predictor.train_model(symbol, &training_data).await {
            Ok(_) => println!("✅ Training completed successfully for {}", symbol),
            Err(e) => println!("⚠️ Training failed for {}: {}", symbol, e),
        }
        
        println!("\n" + &"=".repeat(60));
    }
    
    println!("\n🎉 Comprehensive logging test completed!");
    println!("Check the output above to see detailed symbol data loading information.");
    
    Ok(())
}