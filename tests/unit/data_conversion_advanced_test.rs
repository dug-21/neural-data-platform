//! Advanced data conversion tests
//! 
//! This test suite covers complex data conversion scenarios including:
//! - Multi-format conversions
//! - Data preprocessing and normalization
//! - Feature engineering validation
//! - Memory-efficient transformations
//! - Streaming data conversion

use autonomous_platform::adapters::neural::data_converter::{
    DataConverter, ConversionFormat, ModelInput
};
use autonomous_platform::adapters::neural::type_converter::TypeConverter;
use autonomous_platform::adapters::neural::vendor_conversion::VendorConversion;
use autonomous_platform::adapters::neural::neuro_divergent_adapter::{
    NeuralModelConfig, NeuralAdapterError
};
use autonomous_platform::data::TimeSeriesData;
use chrono::{DateTime, Utc, TimeZone, Duration};
use std::collections::HashMap;
use polars::prelude::*;
use ndarray::{Array1, Array2, Array3};
use anyhow::Result;
use serde_json::json;

// Enhanced test data generation with more realistic patterns
fn create_realistic_market_data(count: usize, symbol: &str) -> Vec<TimeSeriesData> {
    let base_timestamp = Utc.ymd(2024, 1, 1).and_hms(0, 0, 0);
    let mut data = Vec::new();
    let mut price = 100.0;
    let mut volume_base = 1000000.0;
    
    for i in 0..count {
        // Simulate realistic price movements with trends and volatility
        let trend = 0.0001 * i as f64; // Slight upward trend
        let volatility = 0.02 * ((i as f64 * 0.1).sin() + 1.0); // Variable volatility
        let random_factor = (i as f64 * 0.7).sin() * volatility;
        
        price *= 1.0 + trend + random_factor;
        
        let open = price * (1.0 + (i as f64 * 0.3).cos() * 0.001);
        let high = price * (1.0 + (i as f64 * 0.5).sin().abs() * 0.002);
        let low = price * (1.0 - (i as f64 * 0.4).cos().abs() * 0.002);
        let close = price;
        
        // Volume patterns
        volume_base *= 1.0 + (i as f64 * 0.2).sin() * 0.1;
        let volume = volume_base * (1.0 + random_factor.abs());
        
        // Technical indicators
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 50.0 + 30.0 * (i as f64 * 0.1).sin());
        indicators.insert("macd".to_string(), 0.001 * (i as f64 * 0.15).sin());
        indicators.insert("macd_signal".to_string(), 0.0008 * (i as f64 * 0.12).sin());
        indicators.insert("bb_upper".to_string(), price * 1.02);
        indicators.insert("bb_lower".to_string(), price * 0.98);
        indicators.insert("bb_middle".to_string(), price);
        indicators.insert("sma_20".to_string(), price * 0.999);
        indicators.insert("ema_12".to_string(), price * 1.001);
        indicators.insert("volume_sma".to_string(), volume_base);
        indicators.insert("atr".to_string(), price * 0.015);
        indicators.insert("obv".to_string(), volume * (i + 1) as f64);
        
        // Market regime indicators
        indicators.insert("volatility_regime".to_string(), volatility * 100.0);
        indicators.insert("trend_strength".to_string(), trend.abs() * 1000.0);
        indicators.insert("momentum".to_string(), random_factor * 100.0);
        
        let ts = TimeSeriesData {
            symbol: symbol.to_string(),
            timestamp: base_timestamp + Duration::minutes(i as i64 * 5), // 5-minute intervals
            open,
            high,
            low,
            close,
            volume,
            indicators,
            source: Some("advanced_test".to_string()),
            entity: Some(format!("{}_entity", symbol)),
            value: Some(close),
            metadata: Some(json!({
                "market_session": if i % 288 < 195 { "market_hours" } else { "after_hours" },
                "day_of_week": (i / 288) % 7,
                "regime": if volatility > 0.015 { "high_vol" } else { "low_vol" }
            })),
        };
        data.push(ts);
    }
    
    data
}

fn create_multi_symbol_data() -> HashMap<String, Vec<TimeSeriesData>> {
    let symbols = vec!["BTC/USD", "ETH/USD", "ADA/USD", "SOL/USD", "MATIC/USD"];
    let mut multi_data = HashMap::new();
    
    for symbol in symbols {
        let data = create_realistic_market_data(200, symbol);
        multi_data.insert(symbol.to_string(), data);
    }
    
    multi_data
}

#[cfg(test)]
mod advanced_conversion_tests {
    use super::*;
    
    #[test]
    fn test_high_frequency_data_conversion() {
        let converter = DataConverter::new();
        
        // Create high-frequency data (1-second intervals)
        let base_time = Utc::now();
        let mut hf_data = Vec::new();
        
        for i in 0..3600 { // 1 hour of 1-second data
            let mut indicators = HashMap::new();
            indicators.insert("microstructure_signal".to_string(), (i as f64 * 0.01).sin());
            indicators.insert("order_flow".to_string(), (i as f64 * 0.005).cos() * 1000.0);
            indicators.insert("bid_ask_spread".to_string(), 0.001 + (i as f64 * 0.0001).sin().abs() * 0.0005);
            
            hf_data.push(TimeSeriesData {
                symbol: "BTC/USD".to_string(),
                timestamp: base_time + Duration::seconds(i),
                open: 50000.0 + (i as f64 * 0.01).sin() * 10.0,
                high: 50010.0 + (i as f64 * 0.01).sin() * 10.0,
                low: 49990.0 + (i as f64 * 0.01).sin() * 10.0,
                close: 50000.0 + (i as f64 * 0.01).sin() * 10.0,
                volume: 100.0 + (i as f64 * 0.001).cos().abs() * 50.0,
                indicators,
                source: Some("high_freq".to_string()),
                entity: Some("BTC/USD".to_string()),
                value: None,
                metadata: None,
            });
        }
        
        let config = NeuralModelConfig {
            lookback_window: 60, // 1 minute lookback
            ..NeuralModelConfig::default()
        };
        
        let result = converter.to_ndarray(&hf_data, &config);
        assert!(result.is_ok());
        
        if let ModelInput::Array2D(array) = result.unwrap() {
            let expected_samples = hf_data.len() - config.lookback_window + 1;
            assert_eq!(array.shape()[0], expected_samples);
            
            // High-frequency data should have additional microstructure features
            assert!(array.shape()[1] > config.lookback_window * 5); // More than OHLCV
        }
    }
    
    #[test]
    fn test_multi_timeframe_aggregation() {
        let converter = DataConverter::with_features(vec![
            "close".to_string(),
            "volume".to_string(),
            "rsi".to_string(),
            "macd".to_string(),
        ]);
        
        let minute_data = create_realistic_market_data(1440, "BTC/USD"); // 24 hours of minute data
        
        // Aggregate to different timeframes
        let timeframes = vec![5, 15, 60, 240]; // 5min, 15min, 1h, 4h
        
        for timeframe in timeframes {
            let aggregated = aggregate_timeframe(&minute_data, timeframe);
            assert_eq!(aggregated.len(), minute_data.len() / timeframe);
            
            let result = converter.to_dataframe(&aggregated);
            assert!(result.is_ok());
            
            if let ModelInput::DataFrame(df) = result.unwrap() {
                assert_eq!(df.height(), aggregated.len());
                
                // Verify OHLC aggregation is correct
                let opens = df.column("open").unwrap().f64().unwrap();
                let highs = df.column("high").unwrap().f64().unwrap();
                let lows = df.column("low").unwrap().f64().unwrap();
                let closes = df.column("y").unwrap().f64().unwrap(); // close is mapped to 'y'
                
                for i in 0..df.height() {
                    let open = opens.get(i).unwrap();
                    let high = highs.get(i).unwrap();
                    let low = lows.get(i).unwrap();
                    let close = closes.get(i).unwrap();
                    
                    assert!(high >= open && high >= close);
                    assert!(low <= open && low <= close);
                }
            }
        }
    }
    
    #[test]
    fn test_feature_engineering_pipeline() {
        let converter = DataConverter::new();
        let data = create_realistic_market_data(500, "ETH/USD");
        
        // Apply feature engineering
        let engineered_data = apply_feature_engineering(&data);
        
        let result = converter.to_dataframe(&engineered_data);
        assert!(result.is_ok());
        
        if let ModelInput::DataFrame(df) = result.unwrap() {
            // Should have original features plus engineered ones
            let column_names = df.get_column_names();
            
            // Original indicators
            assert!(column_names.contains(&"rsi"));
            assert!(column_names.contains(&"macd"));
            
            // Engineered features should be present
            assert!(column_names.contains(&"returns"));
            assert!(column_names.contains(&"log_returns"));
            assert!(column_names.contains(&"volatility"));
            assert!(column_names.contains(&"price_change_pct"));
            assert!(column_names.contains(&"volume_change_pct"));
            
            // Verify engineered features have reasonable values
            let returns = df.column("returns").unwrap().f64().unwrap();
            let volatility = df.column("volatility").unwrap().f64().unwrap();
            
            // Returns should be small for realistic data
            for i in 1..df.height() {
                let ret = returns.get(i).unwrap();
                assert!(ret.abs() < 0.1); // Less than 10% per period
                
                let vol = volatility.get(i).unwrap();
                assert!(vol >= 0.0 && vol < 1.0); // Volatility should be positive and reasonable
            }
        }
    }
    
    #[test]
    fn test_normalization_and_scaling() {
        let converter = DataConverter::new();
        let data = create_realistic_market_data(300, "ADA/USD");
        
        let config = NeuralModelConfig {
            model_params: json!({
                "normalize": true,
                "scaling_method": "min_max",
                "feature_range": [0.0, 1.0]
            }),
            ..NeuralModelConfig::default()
        };
        
        let result = converter.to_ndarray(&data, &config);
        assert!(result.is_ok());
        
        if let ModelInput::Array2D(array) = result.unwrap() {
            // Check that values are in expected range after normalization
            let min_val = array.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let max_val = array.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            
            assert!(min_val >= -0.1); // Allow small margin for floating point
            assert!(max_val <= 1.1);
            assert!(min_val < max_val); // Should have some variation
        }
    }
    
    #[test]
    fn test_missing_data_handling() {
        let converter = DataConverter::new();
        let mut data = create_realistic_market_data(100, "SOL/USD");
        
        // Introduce missing data patterns
        for i in (10..20).step_by(2) {
            data[i].close = f64::NAN;
            data[i].volume = f64::NAN;
        }
        
        // Remove some indicators
        for i in 30..40 {
            data[i].indicators.remove("rsi");
            data[i].indicators.remove("macd");
        }
        
        // Add extreme outliers
        data[50].close = data[50].close * 10.0; // 10x price spike
        data[51].volume = data[51].volume * 100.0; // 100x volume spike
        
        let result = converter.to_dataframe(&data);
        assert!(result.is_ok());
        
        if let ModelInput::DataFrame(df) = result.unwrap() {
            assert_eq!(df.height(), data.len());
            
            // Check NaN handling
            let closes = df.column("y").unwrap().f64().unwrap();
            let nan_count = (0..df.height())
                .filter(|&i| closes.get(i).unwrap().is_nan())
                .count();
            assert!(nan_count > 0); // Should preserve NaN values
            
            // Missing indicators should be filled with 0.0 or NaN
            if df.get_column_names().contains(&"rsi") {
                let rsi = df.column("rsi").unwrap().f64().unwrap();
                let missing_count = (30..40)
                    .filter(|&i| rsi.get(i).unwrap().is_nan() || rsi.get(i).unwrap() == 0.0)
                    .count();
                assert!(missing_count > 0);
            }
        }
    }
}

#[cfg(test)]
mod vendor_format_tests {
    use super::*;
    
    #[test]
    fn test_neuralprophet_format_conversion() {
        let data = create_realistic_market_data(200, "BTC/USD");
        let vendor_converter = VendorConversion::new();
        
        let result = vendor_converter.to_neuralprophet_format(&data);
        assert!(result.is_ok());
        
        let df = result.unwrap();
        
        // NeuralProphet expects specific column names
        assert!(df.get_column_names().contains(&"ds")); // timestamp
        assert!(df.get_column_names().contains(&"y"));  // target
        
        // Should have additional regressors
        assert!(df.get_column_names().contains(&"volume"));
        assert!(df.get_column_names().contains(&"rsi"));
        
        // Timestamps should be sorted
        let timestamps = df.column("ds").unwrap().datetime().unwrap();
        for i in 1..df.height() {
            let prev = timestamps.get(i - 1).unwrap();
            let curr = timestamps.get(i).unwrap();
            assert!(curr >= prev);
        }
    }
    
    #[test]
    fn test_statsforecast_format_conversion() {
        let multi_data = create_multi_symbol_data();
        let vendor_converter = VendorConversion::new();
        
        let result = vendor_converter.to_statsforecast_format(&multi_data);
        assert!(result.is_ok());
        
        let df = result.unwrap();
        
        // StatsForecast expects hierarchical format
        assert!(df.get_column_names().contains(&"unique_id"));
        assert!(df.get_column_names().contains(&"ds"));
        assert!(df.get_column_names().contains(&"y"));
        
        // Should have data for all symbols
        let unique_ids = df.column("unique_id").unwrap().utf8().unwrap();
        let symbols: std::collections::HashSet<_> = unique_ids.into_iter().flatten().collect();
        assert_eq!(symbols.len(), 5); // BTC, ETH, ADA, SOL, MATIC
        
        // Each symbol should have same number of observations
        for symbol in symbols {
            let symbol_data: Vec<_> = unique_ids.into_iter()
                .enumerate()
                .filter(|(_, id)| id == &Some(symbol))
                .collect();
            assert_eq!(symbol_data.len(), 200); // Each symbol has 200 observations
        }
    }
    
    #[test]
    fn test_timesfm_format_conversion() {
        let data = create_realistic_market_data(300, "ETH/USD");
        let vendor_converter = VendorConversion::new();
        
        let result = vendor_converter.to_timesfm_format(&data, 48, 12); // 48 lookback, 12 horizon
        assert!(result.is_ok());
        
        let tensor = result.unwrap();
        
        // TimesFM expects 3D tensor: [batch, sequence, features]
        assert_eq!(tensor.ndim(), 3);
        assert_eq!(tensor.shape()[1], 48); // Sequence length
        assert!(tensor.shape()[2] > 0); // Feature count
        
        // Values should be normalized for TimesFM
        let min_val = tensor.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_val = tensor.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        assert!(min_val >= -3.0 && max_val <= 3.0); // Reasonable normalization range
    }
    
    #[test]
    fn test_chronos_format_conversion() {
        let data = create_realistic_market_data(1000, "MATIC/USD");
        let vendor_converter = VendorConversion::new();
        
        let result = vendor_converter.to_chronos_format(&data, 512); // 512 context length
        assert!(result.is_ok());
        
        let tokenized = result.unwrap();
        
        // Chronos uses tokenized time series
        assert_eq!(tokenized.len(), 512);
        
        // Tokens should be in valid range
        for &token in &tokenized {
            assert!(token >= 0 && token < 4096); // Typical vocab size
        }
        
        // Should preserve some structure of original data
        let unique_tokens: std::collections::HashSet<_> = tokenized.iter().collect();
        assert!(unique_tokens.len() > 10); // Should have reasonable diversity
    }
}

#[cfg(test)]
mod type_conversion_tests {
    use super::*;
    
    #[test]
    fn test_polars_to_ndarray_conversion() {
        let converter = TypeConverter::new();
        
        // Create test DataFrame
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let df = DataFrame::new(vec![
            Series::new("col1", &values),
            Series::new("col2", values.iter().map(|x| x * 2.0).collect::<Vec<_>>()),
            Series::new("col3", values.iter().map(|x| x * 3.0).collect::<Vec<_>>()),
        ]).unwrap();
        
        let result = converter.dataframe_to_ndarray(&df);
        assert!(result.is_ok());
        
        let array = result.unwrap();
        assert_eq!(array.shape(), &[5, 3]); // 5 rows, 3 columns
        
        // Verify values
        assert_eq!(array[[0, 0]], 1.0);
        assert_eq!(array[[0, 1]], 2.0);
        assert_eq!(array[[0, 2]], 3.0);
    }
    
    #[test]
    fn test_ndarray_to_polars_conversion() {
        let converter = TypeConverter::new();
        
        // Create test array
        let array = Array2::from_shape_vec((3, 4), (0..12).map(|x| x as f64).collect()).unwrap();
        let column_names = vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string()];
        
        let result = converter.ndarray_to_dataframe(&array, &column_names);
        assert!(result.is_ok());
        
        let df = result.unwrap();
        assert_eq!(df.shape(), (3, 4));
        
        // Verify values
        let col_a = df.column("a").unwrap().f64().unwrap();
        assert_eq!(col_a.get(0).unwrap(), 0.0);
        assert_eq!(col_a.get(1).unwrap(), 4.0);
        assert_eq!(col_a.get(2).unwrap(), 8.0);
    }
    
    #[test]
    fn test_timeseries_to_tensor_conversion() {
        let converter = TypeConverter::new();
        let data = create_realistic_market_data(100, "BTC/USD");
        
        let result = converter.timeseries_to_tensor(&data, 20, 5);
        assert!(result.is_ok());
        
        let tensor = result.unwrap();
        
        // Should create 3D tensor: [samples, sequence_length, features]
        assert_eq!(tensor.ndim(), 3);
        assert_eq!(tensor.shape()[1], 20); // Sequence length
        assert!(tensor.shape()[2] > 5); // At least OHLCV features
        
        // Verify temporal ordering
        for sample in 0..tensor.shape()[0] {
            for seq in 1..tensor.shape()[1] {
                // Later timestamps should generally have higher values (due to trend)
                let curr_close = tensor[[sample, seq, 3]]; // Close is typically index 3
                let prev_close = tensor[[sample, seq - 1, 3]];
                // Allow for some variation but check general trend
                assert!((curr_close - prev_close).abs() < prev_close * 0.1);
            }
        }
    }
    
    #[test]
    fn test_batch_processing() {
        let converter = TypeConverter::new();
        let large_data = create_realistic_market_data(5000, "ETH/USD");
        
        let batch_size = 128;
        let batches = converter.create_batches(&large_data, batch_size);
        
        assert!(!batches.is_empty());
        
        // All batches except last should be full size
        for i in 0..(batches.len() - 1) {
            assert_eq!(batches[i].len(), batch_size);
        }
        
        // Last batch can be smaller
        assert!(batches.last().unwrap().len() <= batch_size);
        
        // Total elements should equal original
        let total_elements: usize = batches.iter().map(|b| b.len()).sum();
        assert_eq!(total_elements, large_data.len());
    }
}

#[cfg(test)]
mod streaming_conversion_tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration as StdDuration;
    
    #[test]
    fn test_streaming_data_conversion() {
        let converter = DataConverter::new();
        let (tx, rx) = mpsc::channel();
        
        // Simulate streaming data
        thread::spawn(move || {
            let data = create_realistic_market_data(100, "BTC/USD");
            for item in data {
                tx.send(item).unwrap();
                thread::sleep(StdDuration::from_millis(1)); // Simulate real-time arrival
            }
        });
        
        let mut buffer = Vec::new();
        let window_size = 20;
        let mut conversion_count = 0;
        
        while let Ok(data_point) = rx.recv_timeout(StdDuration::from_millis(100)) {
            buffer.push(data_point);
            
            if buffer.len() >= window_size {
                // Convert sliding window
                let window = &buffer[buffer.len() - window_size..];
                let result = converter.to_dataframe(window);
                
                assert!(result.is_ok());
                conversion_count += 1;
                
                if conversion_count >= 50 { // Stop after 50 conversions
                    break;
                }
            }
        }
        
        assert!(conversion_count >= 50);
    }
    
    #[test]
    fn test_incremental_feature_calculation() {
        let converter = DataConverter::new();
        let mut data_stream = Vec::new();
        
        // Simulate incremental data arrival
        let base_data = create_realistic_market_data(200, "ETH/USD");
        
        for (i, new_point) in base_data.iter().enumerate() {
            data_stream.push(new_point.clone());
            
            if i >= 20 && i % 10 == 0 { // Every 10 points after initial 20
                // Calculate features incrementally
                let window = &data_stream[data_stream.len().saturating_sub(50)..];
                let result = converter.to_dataframe(window);
                
                assert!(result.is_ok());
                
                if let ModelInput::DataFrame(df) = result.unwrap() {
                    // Verify incremental indicators are reasonable
                    if df.get_column_names().contains(&"rsi") {
                        let rsi = df.column("rsi").unwrap().f64().unwrap();
                        let last_rsi = rsi.get(df.height() - 1).unwrap();
                        assert!(last_rsi >= 0.0 && last_rsi <= 100.0);
                    }
                }
            }
        }
    }
}

// Helper functions for complex test scenarios
fn aggregate_timeframe(data: &[TimeSeriesData], timeframe: usize) -> Vec<TimeSeriesData> {
    let mut aggregated = Vec::new();
    
    for chunk in data.chunks(timeframe) {
        if chunk.is_empty() { continue; }
        
        let first = &chunk[0];
        let last = &chunk[chunk.len() - 1];
        
        let open = first.open;
        let close = last.close;
        let high = chunk.iter().map(|d| d.high).fold(f64::NEG_INFINITY, f64::max);
        let low = chunk.iter().map(|d| d.low).fold(f64::INFINITY, f64::min);
        let volume: f64 = chunk.iter().map(|d| d.volume).sum();
        
        // Average indicators
        let mut indicators = HashMap::new();
        for key in first.indicators.keys() {
            let avg = chunk.iter()
                .filter_map(|d| d.indicators.get(key))
                .sum::<f64>() / chunk.len() as f64;
            indicators.insert(key.clone(), avg);
        }
        
        aggregated.push(TimeSeriesData {
            symbol: first.symbol.clone(),
            timestamp: last.timestamp,
            open,
            high,
            low,
            close,
            volume,
            indicators,
            source: first.source.clone(),
            entity: first.entity.clone(),
            value: Some(close),
            metadata: first.metadata.clone(),
        });
    }
    
    aggregated
}

fn apply_feature_engineering(data: &[TimeSeriesData]) -> Vec<TimeSeriesData> {
    let mut engineered = data.to_vec();
    
    for i in 1..engineered.len() {
        let curr = &data[i];
        let prev = &data[i - 1];
        
        // Calculate additional features
        let returns = (curr.close - prev.close) / prev.close;
        let log_returns = (curr.close / prev.close).ln();
        let price_change_pct = (curr.close - prev.close) / prev.close * 100.0;
        let volume_change_pct = (curr.volume - prev.volume) / prev.volume * 100.0;
        
        // Rolling volatility (simplified)
        let volatility = if i >= 20 {
            let window = &data[i-19..=i];
            let mean_return = window.windows(2)
                .map(|w| (w[1].close - w[0].close) / w[0].close)
                .sum::<f64>() / 19.0;
            
            let variance = window.windows(2)
                .map(|w| {
                    let ret = (w[1].close - w[0].close) / w[0].close;
                    (ret - mean_return).powi(2)
                })
                .sum::<f64>() / 19.0;
            
            variance.sqrt()
        } else {
            0.01 // Default volatility
        };
        
        // Add engineered features
        engineered[i].indicators.insert("returns".to_string(), returns);
        engineered[i].indicators.insert("log_returns".to_string(), log_returns);
        engineered[i].indicators.insert("volatility".to_string(), volatility);
        engineered[i].indicators.insert("price_change_pct".to_string(), price_change_pct);
        engineered[i].indicators.insert("volume_change_pct".to_string(), volume_change_pct);
    }
    
    engineered
}