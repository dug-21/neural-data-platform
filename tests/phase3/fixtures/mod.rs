//! Test Fixtures for Phase 3
//!
//! Shared test data and mock objects for consistent testing

use chrono::{DateTime, Utc};
use std::collections::HashMap;

use autonomous_platform::data::TimeSeriesData;
use autonomous_platform::config::NeuralConfig;

/// Standard test symbols for different market sectors
pub const TEST_SYMBOLS: &[&str] = &[
    "AAPL", "GOOGL", "MSFT", // Tech
    "JPM", "BAC", "WFC",     // Finance  
    "JNJ", "PFE", "UNH",     // Healthcare
    "XOM", "CVX", "COP",     // Energy
];

/// Create realistic test TimeSeriesData with Phase 3 structure
pub fn create_realistic_time_series_data(symbol: &str, base_price: f64, timestamp: DateTime<Utc>) -> TimeSeriesData {
    let volatility = 0.02; // 2% volatility
    let open = base_price * (1.0 + (rand::random::<f64>() - 0.5) * volatility);
    let close = open * (1.0 + (rand::random::<f64>() - 0.5) * volatility);
    let high = open.max(close) * (1.0 + rand::random::<f64>() * volatility * 0.5);
    let low = open.min(close) * (1.0 - rand::random::<f64>() * volatility * 0.5);
    let volume_base = match symbol {
        "AAPL" => 50_000_000.0,
        "GOOGL" => 25_000_000.0,
        "MSFT" => 40_000_000.0,
        _ => 10_000_000.0,
    };
    let volume_value = volume_base * (0.5 + rand::random::<f64>());
    
    let mut indicators = HashMap::new();
    indicators.insert("sma_20".to_string(), base_price * 0.98);
    indicators.insert("rsi".to_string(), 45.0 + rand::random::<f64>() * 10.0);
    indicators.insert("macd".to_string(), (rand::random::<f64>() - 0.5) * 2.0);
    indicators.insert("bb_upper".to_string(), high * 1.02);
    indicators.insert("bb_lower".to_string(), low * 0.98);
    
    TimeSeriesData {
        symbol: symbol.to_string(),
        timestamp,
        open,
        high,
        low,
        close,
        volume: vec![volume_value, volume_value * 1.1, volume_value * 0.9], // Vec for compatibility
        volume_value,
        indicators,
        source: Some("test_fixture".to_string()),
        entity: Some(format!("test_{}", symbol.to_lowercase())),
        value: Some(close),
        metadata: Some(serde_json::json!({
            "test_fixture": true,
            "sector": get_sector_for_symbol(symbol),
            "market_cap": get_market_cap_for_symbol(symbol)
        })),
        values: vec![open, high, low, close],
        intervals: vec![60, 300, 900], // 1min, 5min, 15min
        timestamps: vec![timestamp, timestamp, timestamp],
    }
}

/// Get sector for test symbol
pub fn get_sector_for_symbol(symbol: &str) -> &'static str {
    match symbol {
        "AAPL" | "GOOGL" | "MSFT" => "Technology",
        "JPM" | "BAC" | "WFC" => "Finance",
        "JNJ" | "PFE" | "UNH" => "Healthcare",
        "XOM" | "CVX" | "COP" => "Energy",
        _ => "Mixed",
    }
}

/// Get approximate market cap for test symbol (in billions)
pub fn get_market_cap_for_symbol(symbol: &str) -> f64 {
    match symbol {
        "AAPL" => 3000.0,
        "GOOGL" => 1800.0,
        "MSFT" => 2800.0,
        "JPM" => 450.0,
        "BAC" => 280.0,
        "WFC" => 180.0,
        "JNJ" => 400.0,
        "PFE" => 200.0,
        "UNH" => 500.0,
        "XOM" => 350.0,
        "CVX" => 280.0,
        "COP" => 150.0,
        _ => 100.0,
    }
}

/// Create test NeuralConfig with optimal Phase 3 settings
pub fn create_test_neural_config() -> NeuralConfig {
    NeuralConfig {
        model_path: "test_model".to_string(),
        learning_rate: 0.001,
        hidden_layers: vec![64, 32, 16],
        epochs: 100,
        batch_size: 32,
        ..Default::default()
    }
}

/// Create a series of time series data for backtesting
pub fn create_time_series_sequence(
    symbol: &str,
    start_price: f64,
    start_time: DateTime<Utc>,
    count: usize,
    interval_minutes: i64,
) -> Vec<TimeSeriesData> {
    let mut data = Vec::new();
    let mut current_price = start_price;
    let mut current_time = start_time;
    
    for _ in 0..count {
        let ts_data = create_realistic_time_series_data(symbol, current_price, current_time);
        current_price = ts_data.close; // Use close as next open
        current_time = current_time + chrono::Duration::minutes(interval_minutes);
        data.push(ts_data);
    }
    
    data
}

/// Market condition scenarios for testing
#[derive(Debug, Clone)]
pub enum MarketCondition {
    Bullish,
    Bearish,
    Sideways,
    Volatile,
    LowVolume,
}

/// Create time series data with specific market conditions
pub fn create_market_condition_data(
    symbol: &str,
    condition: MarketCondition,
    base_price: f64,
    timestamp: DateTime<Utc>,
) -> TimeSeriesData {
    let mut data = create_realistic_time_series_data(symbol, base_price, timestamp);
    
    match condition {
        MarketCondition::Bullish => {
            data.close = data.open * 1.02; // 2% gain
            data.high = data.close * 1.005;
            data.low = data.open * 0.998;
            data.volume_value *= 1.5; // Higher volume
        },
        MarketCondition::Bearish => {
            data.close = data.open * 0.98; // 2% loss
            data.high = data.open * 1.002;
            data.low = data.close * 0.995;
            data.volume_value *= 1.3; // Higher volume
        },
        MarketCondition::Sideways => {
            data.close = data.open * (0.999 + rand::random::<f64>() * 0.002); // ±0.1%
            data.high = data.open * 1.001;
            data.low = data.open * 0.999;
            data.volume_value *= 0.8; // Lower volume
        },
        MarketCondition::Volatile => {
            data.high = data.open * 1.05; // 5% range
            data.low = data.open * 0.95;
            data.close = data.low + (data.high - data.low) * rand::random::<f64>();
            data.volume_value *= 2.0; // Much higher volume
        },
        MarketCondition::LowVolume => {
            data.volume_value *= 0.3; // 30% of normal volume
            data.close = data.open * (0.9995 + rand::random::<f64>() * 0.001); // Minimal movement
        },
    }
    
    // Update volume vector to match volume_value
    data.volume = vec![data.volume_value, data.volume_value * 1.1, data.volume_value * 0.9];
    data.values = vec![data.open, data.high, data.low, data.close];
    
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_realistic_data_creation() {
        let timestamp = Utc::now();
        let data = create_realistic_time_series_data("AAPL", 150.0, timestamp);
        
        assert_eq!(data.symbol, "AAPL");
        assert_eq!(data.timestamp, timestamp);
        assert!(data.open > 140.0 && data.open < 160.0);
        assert!(data.volume_value > 20_000_000.0);
        assert!(!data.indicators.is_empty());
        assert_eq!(data.values.len(), 4);
        assert_eq!(data.volume.len(), 3);
    }

    #[test]
    fn test_market_conditions() {
        let timestamp = Utc::now();
        let base_price = 100.0;
        
        let bullish = create_market_condition_data("AAPL", MarketCondition::Bullish, base_price, timestamp);
        assert!(bullish.close > bullish.open);
        
        let bearish = create_market_condition_data("AAPL", MarketCondition::Bearish, base_price, timestamp);
        assert!(bearish.close < bearish.open);
        
        let volatile = create_market_condition_data("AAPL", MarketCondition::Volatile, base_price, timestamp);
        assert!((volatile.high - volatile.low) / volatile.open > 0.08); // >8% range
    }

    #[test]
    fn test_time_series_sequence() {
        let start_time = Utc::now();
        let sequence = create_time_series_sequence("AAPL", 150.0, start_time, 10, 5);
        
        assert_eq!(sequence.len(), 10);
        
        // Check time progression
        for i in 1..sequence.len() {
            let time_diff = sequence[i].timestamp - sequence[i-1].timestamp;
            assert_eq!(time_diff.num_minutes(), 5);
        }
    }
}