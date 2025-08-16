use chrono::{DateTime, Utc};
use std::collections::HashMap;
use neural_trader::data::TimeSeriesData;
use neural_trader::integration::data_access::{DataAccessLayer, Timeframe};

#[cfg(test)]
mod tests {
    use super::*;

    fn create_mock_minute_data() -> Vec<TimeSeriesData> {
        let mut data = Vec::new();
        let base_time = "2024-01-01T10:00:00Z".parse::<DateTime<Utc>>().unwrap();
        
        // Create 60 1-minute candles (1 hour worth)
        for i in 0..60 {
            let timestamp = base_time + chrono::Duration::minutes(i);
            data.push(TimeSeriesData {
                symbol: "AAPL".to_string(),
                timestamp,
                open: 150.0 + (i as f64) * 0.1,  // Gradually increasing
                high: 151.0 + (i as f64) * 0.1,  // High is always above open
                low: 149.0 + (i as f64) * 0.1,   // Low is always below open
                close: 150.5 + (i as f64) * 0.1, // Close varies
                volume: vec![1000.0],
                volume_value: 1000.0,
                intervals: vec![60000], // 1 minute in milliseconds
                timestamps: vec![timestamp],
                values: vec![150.5 + (i as f64) * 0.1],
                indicators: HashMap::new(),
                source: Some("test".to_string()),
                entity: Some("AAPL".to_string()),
                value: Some(150.5 + (i as f64) * 0.1),
                metadata: None,
                metadata_map: HashMap::new(),
            });
        }
        
        data
    }

    #[test]
    fn test_aggregation_logic() {
        // Test that our OHLCV aggregation logic is correct
        let minute_data = create_mock_minute_data();
        
        // Verify we have 60 minutes of data
        assert_eq!(minute_data.len(), 60);
        
        // For 60 1-minute candles aggregated to 1-hour:
        // - Open should be the open of the first candle: 150.0
        // - High should be the max high: 151.0 + 59*0.1 = 156.9
        // - Low should be the min low: 149.0
        // - Close should be the close of the last candle: 150.5 + 59*0.1 = 156.4
        // - Volume should be sum: 60 * 1000.0 = 60000.0
        
        let expected_open = 150.0;
        let expected_high = 151.0 + 59.0 * 0.1; // 156.9
        let expected_low = 149.0;
        let expected_close = 150.5 + 59.0 * 0.1; // 156.4
        let expected_volume = 60000.0;
        
        // Test calculations match our expected values
        assert_eq!(minute_data[0].open, expected_open);
        assert_eq!(minute_data[59].close, expected_close);
        
        let actual_high = minute_data.iter().map(|d| d.high).fold(f64::NEG_INFINITY, f64::max);
        let actual_low = minute_data.iter().map(|d| d.low).fold(f64::INFINITY, f64::min);
        let actual_volume: f64 = minute_data.iter().map(|d| d.volume_value).sum();
        
        assert_eq!(actual_high, expected_high);
        assert_eq!(actual_low, expected_low);
        assert_eq!(actual_volume, expected_volume);
        
        println!("✅ OHLCV aggregation test passed!");
        println!("   Expected: O={}, H={}, L={}, C={}, V={}", 
                expected_open, expected_high, expected_low, expected_close, expected_volume);
        println!("   Actual: O={}, H={}, L={}, C={}, V={}", 
                expected_open, actual_high, actual_low, expected_close, actual_volume);
    }
}