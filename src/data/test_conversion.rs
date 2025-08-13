//! Test data conversion between database and TimeSeriesData

use super::*;
use chrono::{DateTime, Utc};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ohlcv_to_time_series_conversion() {
        // Test conversion from OHLCV data to TimeSeriesData
        let timestamp = Utc::now();
        
        let market_data = crate::adapters::MarketData {
            symbol: "BTCUSD".to_string(),
            timestamp: timestamp.timestamp(),
            open: 50000.0,
            high: 51000.0,
            low: 49000.0,
            close: 50500.0,
            volume: 1000.0,
        };

        // Convert to TimeSeriesData
        let ts_data = TimeSeriesData::from_market_data(&market_data).unwrap();
        
        // Validate the conversion
        assert_eq!(ts_data.symbol, "BTCUSD");
        assert_eq!(ts_data.open, 50000.0);
        assert_eq!(ts_data.high, 51000.0);
        assert_eq!(ts_data.low, 49000.0);
        assert_eq!(ts_data.close, 50500.0);
        assert_eq!(ts_data.volume_value, 1000.0);
        assert_eq!(ts_data.volume, vec![1000.0]);

        // Test validation
        ts_data.validate().unwrap();
        
        // Convert back to MarketData
        let back_to_market = ts_data.to_market_data();
        assert_eq!(back_to_market.symbol, market_data.symbol);
        assert_eq!(back_to_market.open, market_data.open);
        assert_eq!(back_to_market.high, market_data.high);
        assert_eq!(back_to_market.low, market_data.low);
        assert_eq!(back_to_market.close, market_data.close);
        assert_eq!(back_to_market.volume, market_data.volume);
    }

    #[test]
    fn test_storage_format_conversion() {
        let timestamp = Utc::now();
        
        // Create TimeSeriesData with OHLCV data
        let mut ts_data = TimeSeriesData::new("ETHUSD".to_string(), timestamp);
        ts_data.open = 3000.0;
        ts_data.high = 3100.0;
        ts_data.low = 2900.0;
        ts_data.close = 3050.0;
        ts_data.volume = vec![500.0];
        ts_data.volume_value = 500.0;

        // Convert to storage format
        let storage_data = ts_data.to_storage_format();
        
        // Verify metadata contains OHLCV data
        let metadata = storage_data.metadata.unwrap();
        assert_eq!(metadata["symbol"], "ETHUSD");
        assert_eq!(metadata["open"], 3000.0);
        assert_eq!(metadata["high"], 3100.0);
        assert_eq!(metadata["low"], 2900.0);
        assert_eq!(metadata["close"], 3050.0);
        assert_eq!(metadata["volume"], 500.0);
        
        // Convert back from storage format
        let back_to_ts = TimeSeriesData::from_storage_format(&storage_data);
        assert_eq!(back_to_ts.symbol, "ETHUSD");
        assert_eq!(back_to_ts.open, 3000.0);
        assert_eq!(back_to_ts.high, 3100.0);
        assert_eq!(back_to_ts.low, 2900.0);
        assert_eq!(back_to_ts.close, 3050.0);
        assert_eq!(back_to_ts.volume_value, 500.0);
    }

    #[test]
    fn test_ohlcv_validation() {
        let timestamp = Utc::now();
        
        // Test valid OHLCV data
        let valid_data = TimeSeriesData::from_ohlcv(
            "AAPL".to_string(),
            timestamp,
            150.0,  // open
            155.0,  // high
            149.0,  // low
            152.0,  // close
            1000.0, // volume
        ).unwrap();
        
        valid_data.validate().unwrap();

        // Test invalid OHLCV (high < low)
        let result = TimeSeriesData::from_ohlcv(
            "AAPL".to_string(),
            timestamp,
            150.0,  // open
            145.0,  // high (invalid: less than low)
            149.0,  // low
            152.0,  // close
            1000.0, // volume
        );
        
        assert!(result.is_err());

        // Test invalid open price (outside high-low range)
        let result = TimeSeriesData::from_ohlcv(
            "AAPL".to_string(),
            timestamp,
            160.0,  // open (invalid: higher than high)
            155.0,  // high
            149.0,  // low
            152.0,  // close
            1000.0, // volume
        );
        
        assert!(result.is_err());
    }

    #[test]
    fn test_database_row_simulation() {
        // Simulate what would come from database query
        let timestamp = Utc::now();
        
        // Simulate storage::TimeSeriesData from database with OHLCV metadata
        let storage_data = storage::TimeSeriesData {
            timestamp,
            source: "market_data_1h".to_string(),
            entity: "BTCUSD".to_string(),
            value: 50500.0, // close price as main value
            metadata: Some(serde_json::json!({
                "symbol": "BTCUSD",
                "open": 50000.0,
                "high": 51000.0,
                "low": 49000.0,
                "close": 50500.0,
                "volume": 1000.0
            })),
        };

        // Convert to enhanced TimeSeriesData
        let ts_data = TimeSeriesData::from_storage_format(&storage_data);
        
        // Verify all OHLCV fields are properly extracted
        assert_eq!(ts_data.symbol, "BTCUSD");
        assert_eq!(ts_data.open, 50000.0);
        assert_eq!(ts_data.high, 51000.0);
        assert_eq!(ts_data.low, 49000.0);
        assert_eq!(ts_data.close, 50500.0);
        assert_eq!(ts_data.volume_value, 1000.0);
        assert_eq!(ts_data.value, Some(50500.0));

        // Validate the data
        ts_data.validate().unwrap();
    }
}