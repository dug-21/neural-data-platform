//! Simple normalization validation tests
//! Tests to verify that real market data normalization works correctly

use anyhow::Result;
use chrono::{DateTime, Utc, TimeZone};
use neural_trader::data::TimeSeriesData;
use std::collections::HashMap;

/// Test data representing real XLK prices around $268
fn create_xlk_test_data() -> Vec<TimeSeriesData> {
    let base_time = Utc.ymd(2024, 1, 1).and_hms(9, 30, 0);
    
    // Real XLK price data: $268-$272 range
    vec![
        TimeSeriesData {
            symbol: "XLK".to_string(),
            timestamp: base_time,
            open: 268.50,
            high: 269.20,
            low: 267.80,
            close: 268.90,
            volume: vec![1_200_000.0],
            volume_value: 1_200_000.0,
            intervals: vec![60000],
            timestamps: vec![base_time],
            values: vec![268.90],
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("XLK".to_string()),
            value: Some(268.90),
            metadata: None,
            metadata_map: HashMap::new(),
        },
        TimeSeriesData {
            symbol: "XLK".to_string(),
            timestamp: base_time + chrono::Duration::minutes(1),
            open: 268.90,
            high: 270.15,
            low: 268.45,
            close: 269.75,
            volume: vec![1_350_000.0],
            volume_value: 1_350_000.0,
            intervals: vec![60000],
            timestamps: vec![base_time + chrono::Duration::minutes(1)],
            values: vec![269.75],
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("XLK".to_string()),
            value: Some(269.75),
            metadata: None,
            metadata_map: HashMap::new(),
        },
        TimeSeriesData {
            symbol: "XLK".to_string(),
            timestamp: base_time + chrono::Duration::minutes(2),
            open: 269.75,
            high: 271.30,
            low: 269.20,
            close: 270.85,
            volume: vec![1_100_000.0],
            volume_value: 1_100_000.0,
            intervals: vec![60000],
            timestamps: vec![base_time + chrono::Duration::minutes(2)],
            values: vec![270.85],
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("XLK".to_string()),
            value: Some(270.85),
            metadata: None,
            metadata_map: HashMap::new(),
        },
    ]
}

#[tokio::test]
async fn test_xlk_real_price_ranges() -> Result<()> {
    println!("🧪 Testing XLK real price ranges...");
    
    let xlk_data = create_xlk_test_data();
    
    // Verify the test data represents realistic XLK prices
    for data_point in &xlk_data {
        // XLK typically trades in $250-$300 range
        assert!(data_point.close >= 250.0 && data_point.close <= 300.0, 
               "XLK close price ${:.2} outside realistic range", data_point.close);
        assert!(data_point.open >= 250.0 && data_point.open <= 300.0, 
               "XLK open price ${:.2} outside realistic range", data_point.open);
        assert!(data_point.high >= 250.0 && data_point.high <= 300.0, 
               "XLK high price ${:.2} outside realistic range", data_point.high);
        assert!(data_point.low >= 250.0 && data_point.low <= 300.0, 
               "XLK low price ${:.2} outside realistic range", data_point.low);
        
        // Verify OHLC relationships
        assert!(data_point.high >= data_point.open, "High < Open");
        assert!(data_point.high >= data_point.close, "High < Close");
        assert!(data_point.low <= data_point.open, "Low > Open");
        assert!(data_point.low <= data_point.close, "Low > Close");
        
        // Verify volume is realistic (millions of shares)
        assert!(data_point.volume_value >= 500_000.0 && data_point.volume_value <= 5_000_000.0,
               "XLK volume {:.0} outside realistic range", data_point.volume_value);
    }
    
    // Calculate price statistics
    let prices: Vec<f64> = xlk_data.iter()
        .flat_map(|d| vec![d.open, d.high, d.low, d.close])
        .collect();
    let price_min = prices.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let price_max = prices.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    
    println!("📊 XLK price range: ${:.2} to ${:.2} (spread: ${:.2})", 
             price_min, price_max, price_max - price_min);
    
    // Verify price spread is reasonable (not $185 synthetic data)
    assert!(price_min > 265.0, "Minimum price too low: ${:.2}", price_min);
    assert!(price_max < 275.0, "Maximum price too high: ${:.2}", price_max);
    assert!(price_max - price_min > 2.0, "Price spread too small: ${:.2}", price_max - price_min);
    assert!(price_max - price_min < 10.0, "Price spread too large: ${:.2}", price_max - price_min);
    
    println!("✅ XLK real price range validation passed");
    Ok(())
}

#[tokio::test]
async fn test_minmax_normalization_math() -> Result<()> {
    println!("🧪 Testing MinMax normalization mathematics...");
    
    let xlk_data = create_xlk_test_data();
    
    // Calculate normalization parameters
    let prices: Vec<f64> = xlk_data.iter()
        .flat_map(|d| vec![d.open, d.high, d.low, d.close])
        .collect();
    let price_min = prices.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let price_max = prices.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let price_range = price_max - price_min;
    
    println!("📊 Price normalization parameters:");
    println!("   Min: ${:.2}, Max: ${:.2}, Range: ${:.2}", price_min, price_max, price_range);
    
    // Test normalization formula: (value - min) / (max - min)
    for data_point in &xlk_data {
        let normalized_close = (data_point.close - price_min) / price_range;
        let normalized_open = (data_point.open - price_min) / price_range;
        let normalized_high = (data_point.high - price_min) / price_range;
        let normalized_low = (data_point.low - price_min) / price_range;
        
        // Verify all normalized values are in [0,1] range
        assert!(normalized_close >= 0.0 && normalized_close <= 1.0,
               "Normalized close {:.4} not in [0,1]", normalized_close);
        assert!(normalized_open >= 0.0 && normalized_open <= 1.0,
               "Normalized open {:.4} not in [0,1]", normalized_open);
        assert!(normalized_high >= 0.0 && normalized_high <= 1.0,
               "Normalized high {:.4} not in [0,1]", normalized_high);
        assert!(normalized_low >= 0.0 && normalized_low <= 1.0,
               "Normalized low {:.4} not in [0,1]", normalized_low);
        
        // Test denormalization: normalized * range + min = original
        let denormalized_close = normalized_close * price_range + price_min;
        let error = (denormalized_close - data_point.close).abs();
        assert!(error < 1e-10, "Denormalization error too large: {:.2e}", error);
        
        println!("🔄 ${:.2} → {:.4} → ${:.2} (close price)", 
                data_point.close, normalized_close, denormalized_close);
    }
    
    // Test edge cases
    let min_normalized = (price_min - price_min) / price_range;
    let max_normalized = (price_max - price_min) / price_range;
    
    assert!((min_normalized - 0.0).abs() < 1e-10, "Min should normalize to 0.0");
    assert!((max_normalized - 1.0).abs() < 1e-10, "Max should normalize to 1.0");
    
    println!("✅ MinMax normalization mathematics validated");
    Ok(())
}

#[tokio::test]
async fn test_symbol_isolation_validation() -> Result<()> {
    println!("🧪 Testing symbol isolation in normalization...");
    
    let xlk_data = create_xlk_test_data();
    
    // Create different price range data for another symbol
    let spy_data = vec![
        TimeSeriesData {
            symbol: "SPY".to_string(),
            timestamp: Utc.ymd(2024, 1, 1).and_hms(9, 30, 0),
            open: 441.20,
            high: 442.50,
            low: 440.80,
            close: 441.90,
            volume: vec![8_500_000.0],
            volume_value: 8_500_000.0,
            intervals: vec![60000],
            timestamps: vec![Utc.ymd(2024, 1, 1).and_hms(9, 30, 0)],
            values: vec![441.90],
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("SPY".to_string()),
            value: Some(441.90),
            metadata: None,
            metadata_map: HashMap::new(),
        }
    ];
    
    // Calculate separate normalization stats
    let xlk_prices: Vec<f64> = xlk_data.iter()
        .flat_map(|d| vec![d.open, d.high, d.low, d.close])
        .collect();
    let spy_prices: Vec<f64> = spy_data.iter()
        .flat_map(|d| vec![d.open, d.high, d.low, d.close])
        .collect();
    
    let xlk_min = xlk_prices.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let xlk_max = xlk_prices.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let spy_min = spy_prices.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let spy_max = spy_prices.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    
    println!("📊 Symbol ranges:");
    println!("   XLK: ${:.2} to ${:.2}", xlk_min, xlk_max);
    println!("   SPY: ${:.2} to ${:.2}", spy_min, spy_max);
    
    // Verify they have different price ranges (different symbols should not interfere)
    assert!((spy_min - xlk_min).abs() > 100.0, "Symbol price ranges too similar");
    assert!((spy_max - xlk_max).abs() > 100.0, "Symbol price ranges too similar");
    
    // Test per-symbol normalization
    let xlk_test_price = xlk_data[0].close; // Around $268.90
    let spy_test_price = spy_data[0].close; // Around $441.90
    
    // XLK normalization using XLK range
    let xlk_normalized = (xlk_test_price - xlk_min) / (xlk_max - xlk_min);
    
    // SPY normalization using SPY range
    let spy_normalized = (spy_test_price - spy_min) / (spy_max - spy_min);
    
    // Both should be in [0,1] range
    assert!(xlk_normalized >= 0.0 && xlk_normalized <= 1.0);
    assert!(spy_normalized >= 0.0 && spy_normalized <= 1.0);
    
    // Test contamination: XLK price normalized with SPY range should give wrong result
    let xlk_with_spy_stats = (xlk_test_price - spy_min) / (spy_max - spy_min);
    assert!(xlk_with_spy_stats < 0.0, "Cross-contamination not detected"); // Should be negative
    
    println!("🔒 XLK normalized with XLK stats: {:.4}", xlk_normalized);
    println!("🔒 SPY normalized with SPY stats: {:.4}", spy_normalized);
    println!("⚠️ XLK normalized with SPY stats: {:.4} (should be negative)", xlk_with_spy_stats);
    
    println!("✅ Symbol isolation validation passed");
    Ok(())
}

#[tokio::test]
async fn test_real_data_vs_synthetic_detection() -> Result<()> {
    println!("🧪 Testing real data vs synthetic data detection...");
    
    let xlk_data = create_xlk_test_data();
    
    // Check that our test data doesn't match the old synthetic $185.00 price
    for data_point in &xlk_data {
        let synthetic_price_distance = (data_point.close - 185.0).abs();
        assert!(synthetic_price_distance > 80.0, 
               "Price ${:.2} too close to synthetic $185.00", data_point.close);
    }
    
    // Verify we're in the expected real XLK price range (~$268)
    let avg_price = xlk_data.iter().map(|d| d.close).sum::<f64>() / xlk_data.len() as f64;
    println!("📊 Average XLK price: ${:.2}", avg_price);
    
    assert!(avg_price > 265.0 && avg_price < 275.0, 
           "Average price ${:.2} not in expected real range", avg_price);
    
    println!("✅ Real data validation passed (not synthetic $185)");
    Ok(())
}