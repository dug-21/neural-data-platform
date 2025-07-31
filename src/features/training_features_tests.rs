//! Comprehensive tests for training features module
//! 
//! Tests all feature extraction capabilities including technical indicators,
//! price transformations, market microstructure features, and normalization.

use super::training_features::*;
use crate::data::TimeSeriesData;
use chrono::{TimeZone, Utc};
use std::collections::HashMap;

/// Create synthetic test data for multiple symbols
fn create_test_data_multi_symbol() -> HashMap<String, Vec<TimeSeriesData>> {
    let mut data = HashMap::new();
    
    let symbols = vec!["AAPL", "GOOGL", "MSFT", "TSLA", "NVDA"];
    
    for symbol in symbols {
        let mut symbol_data = Vec::new();
        let base_time = Utc.ymd_opt(2024, 1, 1).unwrap().and_hms_opt(9, 30, 0).unwrap();
        let base_price = match symbol {
            "AAPL" => 150.0,
            "GOOGL" => 2800.0,
            "MSFT" => 350.0,
            "TSLA" => 200.0,
            "NVDA" => 800.0,
            _ => 100.0,
        };
        
        for i in 0..200 {
            let time_offset = chrono::Duration::minutes(i * 5);
            let price_variation = (i as f64 * 0.1).sin() * 10.0 + (i as f64 * 0.05).cos() * 5.0;
            let price = base_price + price_variation;
            
            // Add some realistic market behavior
            let volatility = if i % 20 == 0 { 2.0 } else { 1.0 };
            let volume_multiplier = if i % 30 == 0 { 2.5 } else { 1.0 };
            
            symbol_data.push(TimeSeriesData {
                symbol: symbol.to_string(),
                timestamp: base_time + time_offset,
                open: price - 0.5 + (i as f64 * 0.02).sin(),
                high: price + volatility,
                low: price - volatility,
                close: price,
                volume: 1000000.0 * volume_multiplier * (1.0 + (i as f64 * 0.1).sin() * 0.5),
                indicators: HashMap::new(),
                source: Some("test".to_string()),
                entity: Some(symbol.to_string()),
                value: Some(price),
                metadata: None,
            });
        }
        
        data.insert(symbol.to_string(), symbol_data);
    }
    
    data
}

/// Create test data with missing values
fn create_test_data_with_gaps() -> Vec<TimeSeriesData> {
    let mut data = Vec::new();
    let base_time = Utc.ymd_opt(2024, 1, 1).unwrap().and_hms_opt(9, 30, 0).unwrap();
    
    for i in 0..100 {
        let price = if i % 10 == 5 {
            // Introduce some NaN values
            f64::NAN
        } else {
            100.0 + (i as f64 * 0.1).sin() * 10.0
        };
        
        let volume = if i % 15 == 7 {
            f64::NAN
        } else {
            1000000.0 * (1.0 + (i as f64 * 0.1).cos())
        };
        
        data.push(TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: base_time + chrono::Duration::minutes(i * 5),
            open: if price.is_nan() { 100.0 } else { price - 0.5 },
            high: if price.is_nan() { 101.0 } else { price + 1.0 },
            low: if price.is_nan() { 99.0 } else { price - 1.0 },
            close: if price.is_nan() { 100.0 } else { price },
            volume: if volume.is_nan() { 1000000.0 } else { volume },
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("TEST".to_string()),
            value: Some(if price.is_nan() { 100.0 } else { price }),
            metadata: None,
        });
    }
    
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_feature_extraction_comprehensive() {
        let mut engine = TrainingFeatureEngine::default();
        let test_data = create_test_data_multi_symbol();
        
        for (symbol, data) in test_data {
            let result = engine.extract_features(&symbol, &data).await;
            assert!(result.is_ok(), "Feature extraction failed for symbol: {}", symbol);
            
            let features = result.unwrap();
            assert!(!features.features.is_empty(), "No features extracted for {}", symbol);
            assert_eq!(features.timestamps.len(), data.len());
            assert_eq!(features.symbols.len(), data.len());
            
            // Verify all symbols are correct
            assert!(features.symbols.iter().all(|s| s == &symbol));
            
            println!("Symbol: {} - Extracted {} features for {} data points", 
                symbol, features.features.len(), data.len());
        }
    }

    #[tokio::test]
    async fn test_technical_indicators() {
        let engine = TrainingFeatureEngine::default();
        let test_data = create_test_data_multi_symbol();
        let aapl_data = &test_data["AAPL"];
        
        let technical_features = engine.compute_technical_indicators(aapl_data).unwrap();
        
        // Test RSI features
        assert!(technical_features.contains_key("rsi_5"));
        assert!(technical_features.contains_key("rsi_10"));
        assert!(technical_features.contains_key("rsi_20"));
        
        // Verify RSI values are in valid range
        let rsi_values = &technical_features["rsi_14"];
        for &rsi in rsi_values {
            assert!(rsi >= 0.0 && rsi <= 100.0, "RSI value {} out of range", rsi);
        }
        
        // Test MACD features
        assert!(technical_features.contains_key("macd_line"));
        assert!(technical_features.contains_key("macd_signal"));
        assert!(technical_features.contains_key("macd_histogram"));
        
        // Test Bollinger Bands
        assert!(technical_features.contains_key("bb_upper_20"));
        assert!(technical_features.contains_key("bb_middle_20"));
        assert!(technical_features.contains_key("bb_lower_20"));
        assert!(technical_features.contains_key("bb_position_20"));
        
        // Verify Bollinger Band ordering (upper > middle > lower)
        let upper = &technical_features["bb_upper_20"];
        let middle = &technical_features["bb_middle_20"];
        let lower = &technical_features["bb_lower_20"];
        
        for i in 20..upper.len() {
            assert!(upper[i] >= middle[i], "Upper band should be >= middle at index {}", i);
            assert!(middle[i] >= lower[i], "Middle band should be >= lower at index {}", i);
        }
        
        // Test ATR
        assert!(technical_features.contains_key("atr_14"));
        let atr_values = &technical_features["atr_14"];
        for &atr in atr_values {
            assert!(atr >= 0.0, "ATR should be non-negative, got {}", atr);
        }
        
        // Test Stochastic
        assert!(technical_features.contains_key("stoch_k"));
        assert!(technical_features.contains_key("stoch_d"));
        
        let stoch_k = &technical_features["stoch_k"];
        for &k in stoch_k {
            assert!(k >= 0.0 && k <= 100.0, "Stochastic %K value {} out of range", k);
        }
        
        // Test OBV
        assert!(technical_features.contains_key("obv"));
        
        // Test MFI
        assert!(technical_features.contains_key("mfi"));
        let mfi_values = &technical_features["mfi"];
        for &mfi in mfi_values {
            assert!(mfi >= 0.0 && mfi <= 100.0, "MFI value {} out of range", mfi);
        }
    }

    #[tokio::test]
    async fn test_price_transformations() {
        let engine = TrainingFeatureEngine::default();
        let test_data = create_test_data_multi_symbol();
        let tsla_data = &test_data["TSLA"];
        
        let price_features = engine.compute_price_transformations(tsla_data).unwrap();
        
        // Test returns
        assert!(price_features.contains_key("return_1"));
        assert!(price_features.contains_key("return_5"));
        assert!(price_features.contains_key("return_10"));
        assert!(price_features.contains_key("return_20"));
        
        // Test log returns
        assert!(price_features.contains_key("log_return_1"));
        assert!(price_features.contains_key("log_return_5"));
        
        // Test price ratios
        assert!(price_features.contains_key("close_open_ratio"));
        let ratios = &price_features["close_open_ratio"];
        for &ratio in ratios {
            assert!(ratio > 0.0, "Price ratio should be positive, got {}", ratio);
        }
        
        // Test high-low spread
        assert!(price_features.contains_key("hl_spread"));
        let spreads = &price_features["hl_spread"];
        for &spread in spreads {
            assert!(spread >= 0.0, "High-low spread should be non-negative, got {}", spread);
        }
        
        // Test price position
        assert!(price_features.contains_key("price_position"));
        let positions = &price_features["price_position"];
        for &pos in positions {
            assert!(pos >= 0.0 && pos <= 1.0, "Price position {} should be in [0,1]", pos);
        }
    }

    #[tokio::test]
    async fn test_market_microstructure_features() {
        let engine = TrainingFeatureEngine::default();
        let test_data = create_test_data_multi_symbol();
        let nvda_data = &test_data["NVDA"];
        
        let microstructure_features = engine.compute_microstructure_features(nvda_data).unwrap();
        
        // Test spread proxy
        assert!(microstructure_features.contains_key("spread_proxy"));
        let spreads = &microstructure_features["spread_proxy"];
        for &spread in spreads {
            assert!(spread >= 0.0, "Spread proxy should be non-negative, got {}", spread);
        }
        
        // Test volume ratio
        assert!(microstructure_features.contains_key("volume_ratio"));
        let vol_ratios = &microstructure_features["volume_ratio"];
        for &ratio in vol_ratios {
            assert!(ratio > 0.0, "Volume ratio should be positive, got {}", ratio);
        }
        
        // Test Kyle's lambda
        assert!(microstructure_features.contains_key("kyles_lambda"));
        
        // Test Amihud illiquidity
        assert!(microstructure_features.contains_key("amihud_illiquidity"));
        
        // Test Roll's spread
        assert!(microstructure_features.contains_key("roll_spread"));
    }

    #[tokio::test]
    async fn test_rolling_statistics() {
        let engine = TrainingFeatureEngine::default();
        let test_data = create_test_data_multi_symbol();
        let googl_data = &test_data["GOOGL"];
        
        let rolling_features = engine.compute_rolling_statistics(googl_data).unwrap();
        
        // Test rolling means
        assert!(rolling_features.contains_key("rolling_mean_5"));
        assert!(rolling_features.contains_key("rolling_mean_10"));
        assert!(rolling_features.contains_key("rolling_mean_20"));
        assert!(rolling_features.contains_key("rolling_mean_50"));
        
        // Test rolling standard deviations
        assert!(rolling_features.contains_key("rolling_std_5"));
        assert!(rolling_features.contains_key("rolling_std_10"));
        
        let stds = &rolling_features["rolling_std_10"];
        for &std in stds {
            assert!(std >= 0.0, "Standard deviation should be non-negative, got {}", std);
        }
        
        // Test rolling skewness
        assert!(rolling_features.contains_key("rolling_skew_10"));
        
        // Test rolling kurtosis  
        assert!(rolling_features.contains_key("rolling_kurtosis_20"));
        
        // Test price-volume correlation
        assert!(rolling_features.contains_key("price_volume_corr_20"));
        let correlations = &rolling_features["price_volume_corr_20"];
        for &corr in correlations {
            assert!(corr >= -1.0 && corr <= 1.0, "Correlation {} should be in [-1,1]", corr);
        }
    }

    #[tokio::test]
    async fn test_volatility_features() {
        let engine = TrainingFeatureEngine::default();
        let test_data = create_test_data_multi_symbol();
        let msft_data = &test_data["MSFT"];
        
        let volatility_features = engine.compute_volatility_features(msft_data).unwrap();
        
        // Test historical volatility
        assert!(volatility_features.contains_key("hist_vol_10"));
        assert!(volatility_features.contains_key("hist_vol_20"));
        assert!(volatility_features.contains_key("hist_vol_30"));
        
        let hist_vols = &volatility_features["hist_vol_20"];
        for &vol in hist_vols {
            assert!(vol >= 0.0, "Historical volatility should be non-negative, got {}", vol);
        }
        
        // Test Parkinson volatility
        assert!(volatility_features.contains_key("parkinson_vol_20"));
        
        // Test Garman-Klass volatility
        assert!(volatility_features.contains_key("garman_klass_vol_20"));
        
        // Test Rogers-Satchell volatility
        assert!(volatility_features.contains_key("rogers_satchell_vol_20"));
        
        // Test volatility regime
        assert!(volatility_features.contains_key("volatility_regime"));
        let regimes = &volatility_features["volatility_regime"];
        for &regime in regimes {
            assert!(regime >= 0.0 && regime <= 2.0, "Volatility regime {} should be in [0,2]", regime);
        }
    }

    #[tokio::test]
    async fn test_time_features() {
        let engine = TrainingFeatureEngine::default();
        let test_data = create_test_data_multi_symbol();
        let aapl_data = &test_data["AAPL"];
        
        let time_features = engine.compute_time_features(aapl_data).unwrap();
        
        // Test time-based features
        assert!(time_features.contains_key("hour_of_day"));
        assert!(time_features.contains_key("day_of_week"));
        assert!(time_features.contains_key("day_of_month"));
        assert!(time_features.contains_key("month_of_year"));
        assert!(time_features.contains_key("quarter"));
        assert!(time_features.contains_key("trading_session"));
        
        // Verify hour of day is normalized
        let hours = &time_features["hour_of_day"];
        for &hour in hours {
            assert!(hour >= 0.0 && hour <= 1.0, "Hour of day {} should be in [0,1]", hour);
        }
        
        // Verify day of week is normalized
        let days = &time_features["day_of_week"];
        for &day in days {
            assert!(day >= 0.0 && day <= 1.0, "Day of week {} should be in [0,1]", day);
        }
        
        // Verify trading session indicators
        let sessions = &time_features["trading_session"];
        for &session in sessions {
            assert!(session >= 0.0 && session <= 1.0, "Trading session {} should be in [0,1]", session);
        }
    }

    #[tokio::test]
    async fn test_normalization_methods() {
        let mut engine = TrainingFeatureEngine::new(FeatureConfig {
            normalization: NormalizationMethod::MinMax,
            ..Default::default()
        });
        
        let test_data = create_test_data_multi_symbol();
        let aapl_data = &test_data["AAPL"];
        
        let features = engine.extract_features("AAPL", aapl_data).await.unwrap();
        
        // Test that features are properly normalized
        for (name, values) in &features.features {
            if !name.starts_with("_") {  // Skip metadata features
                let min_val = values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
                let max_val = values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
                
                // For MinMax normalization, values should be in [0, 1] range
                // (with some tolerance for edge cases)
                assert!(min_val >= -0.1, "Feature {} min value {} below expected range", name, min_val);
                assert!(max_val <= 1.1, "Feature {} max value {} above expected range", name, max_val);
            }
        }
        
        // Test Z-Score normalization
        let mut zscore_engine = TrainingFeatureEngine::new(FeatureConfig {
            normalization: NormalizationMethod::ZScore,
            ..Default::default()
        });
        
        let zscore_features = zscore_engine.extract_features("AAPL", aapl_data).await.unwrap();
        
        // For Z-Score normalization, features should have approximately zero mean
        for (name, values) in &zscore_features.features {
            if !name.starts_with("_") && values.len() > 10 {
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                assert!(mean.abs() < 0.5, "Feature {} mean {} too far from zero", name, mean);
            }
        }
    }

    #[tokio::test]
    async fn test_missing_data_handling() {
        let mut engine = TrainingFeatureEngine::new(FeatureConfig {
            handle_missing: MissingDataStrategy::Forward,
            ..Default::default()
        });
        
        let test_data = create_test_data_with_gaps();
        let features = engine.extract_features("TEST", &test_data).await.unwrap();
        
        // Verify no NaN values remain after forward filling
        for (name, values) in &features.features {
            for (i, &value) in values.iter().enumerate() {
                assert!(!value.is_nan(), 
                    "Feature {} has NaN at index {} after missing data handling", name, i);
                assert!(!value.is_infinite(), 
                    "Feature {} has infinite value at index {} after missing data handling", name, i);
            }
        }
        
        // Test backward filling strategy
        let mut backward_engine = TrainingFeatureEngine::new(FeatureConfig {
            handle_missing: MissingDataStrategy::Backward,
            ..Default::default()
        });
        
        let backward_features = backward_engine.extract_features("TEST", &test_data).await.unwrap();
        
        // Verify no NaN values remain
        for (name, values) in &backward_features.features {
            assert!(!values.iter().any(|v| v.is_nan()), 
                "Feature {} contains NaN values after backward filling", name);
        }
    }

    #[tokio::test]
    async fn test_feature_validation() {
        let engine = TrainingFeatureEngine::default();
        let test_data = create_test_data_multi_symbol();
        let aapl_data = &test_data["AAPL"];
        
        let technical_features = engine.compute_technical_indicators(aapl_data).unwrap();
        
        // This should not panic - validation should pass
        let validation_result = engine.validate_features(&technical_features);
        assert!(validation_result.is_ok(), "Feature validation failed");
        
        // Test with features that have extreme values
        let mut extreme_features = HashMap::new();
        extreme_features.insert("extreme_feature".to_string(), vec![1e10, -1e10, 0.0]);
        
        // Validation should still succeed but might print warnings
        let extreme_validation = engine.validate_features(&extreme_features);
        assert!(extreme_validation.is_ok(), "Extreme feature validation failed");
    }

    #[tokio::test]
    async fn test_feature_metadata() {
        let mut engine = TrainingFeatureEngine::default();
        let test_data = create_test_data_multi_symbol();
        let aapl_data = &test_data["AAPL"];
        
        let features = engine.extract_features("AAPL", aapl_data).await.unwrap();
        
        // Verify metadata is generated
        assert!(!features.metadata.is_empty(), "Feature metadata should not be empty");
        
        // Test metadata fields
        for (name, metadata) in &features.metadata {
            assert_eq!(metadata.name, *name);
            assert!(!metadata.category.is_empty());
            assert!(metadata.variance >= 0.0);
            assert!(metadata.missing_ratio >= 0.0 && metadata.missing_ratio <= 1.0);
        }
        
        // Test feature importance updates
        let mut importance_scores = HashMap::new();
        importance_scores.insert("rsi_14".to_string(), 0.85);
        importance_scores.insert("macd_line".to_string(), 0.72);
        
        let update_result = engine.update_importances(importance_scores.clone());
        assert!(update_result.is_ok(), "Feature importance update failed");
        
        // Verify importance was updated
        if let Some(rsi_metadata) = engine.feature_metadata.get("rsi_14") {
            assert_eq!(rsi_metadata.importance, 0.85);
        }
        
        // Test getting top features
        let top_features = engine.get_top_features(5);
        assert!(!top_features.is_empty(), "Top features list should not be empty");
    }

    #[tokio::test]
    async fn test_incremental_updates() {
        let mut engine = TrainingFeatureEngine::new(FeatureConfig {
            incremental_updates: true,
            ..Default::default()
        });
        
        let test_data = create_test_data_multi_symbol();
        let aapl_data = &test_data["AAPL"];
        
        // Test incremental update with new data point
        let new_data = &aapl_data[aapl_data.len() - 1];
        let incremental_features = engine.update_features_incremental("AAPL", new_data, 20).await;
        
        assert!(incremental_features.is_ok(), "Incremental update failed");
        let features = incremental_features.unwrap();
        assert!(!features.is_empty(), "Incremental features should not be empty");
        
        // Verify basic features are present
        assert!(features.contains_key("close"));
        assert!(features.contains_key("volume"));
        assert!(features.contains_key("high_low_ratio"));
    }

    #[test]
    fn test_individual_calculations() {
        let engine = TrainingFeatureEngine::default();
        
        // Test RSI calculation
        let closes = vec![44.0, 44.25, 44.5, 43.75, 44.5, 44.0, 44.25, 44.75, 44.5, 44.0];
        let rsi = engine.calculate_rsi(&closes, 5).unwrap();
        assert_eq!(rsi.len(), closes.len());
        
        // Test EMA calculation
        let ema = engine.calculate_ema(&closes, 3).unwrap();
        assert_eq!(ema.len(), closes.len());
        assert_eq!(ema[0], closes[0]);  // First value should equal first close
        
        // Test returns calculation
        let returns = engine.calculate_returns(&closes, 1).unwrap();
        assert_eq!(returns.len(), closes.len());
        assert_eq!(returns[0], 0.0);  // First return should be 0
        
        // Test rolling statistics
        let rolling_mean = engine.calculate_rolling_mean(&closes, 3).unwrap();
        assert_eq!(rolling_mean.len(), closes.len());
        
        let rolling_std = engine.calculate_rolling_std(&closes, 3).unwrap();
        assert_eq!(rolling_std.len(), closes.len());
        
        // Verify rolling mean calculation for specific values
        let expected_mean_2 = (closes[0] + closes[1] + closes[2]) / 3.0;
        assert!((rolling_mean[2] - expected_mean_2).abs() < 1e-10);
    }

    #[test]
    fn test_feature_categorization() {
        let engine = TrainingFeatureEngine::default();
        
        assert_eq!(engine.infer_category("rsi_14"), "momentum");
        assert_eq!(engine.infer_category("volatility_20"), "volatility");
        assert_eq!(engine.infer_category("return_5"), "returns");
        assert_eq!(engine.infer_category("bb_upper_20"), "trend");
        assert_eq!(engine.infer_category("volume_ratio"), "volume");
        assert_eq!(engine.infer_category("kyles_lambda"), "microstructure");
        assert_eq!(engine.infer_category("hour_of_day"), "time");
        assert_eq!(engine.infer_category("rolling_skew_10"), "statistics");
        assert_eq!(engine.infer_category("unknown_feature"), "other");
    }

    #[tokio::test]
    async fn test_performance_with_large_dataset() {
        let mut engine = TrainingFeatureEngine::default();
        
        // Create larger dataset
        let mut large_data = Vec::new();
        let base_time = Utc.ymd_opt(2024, 1, 1).unwrap().and_hms_opt(9, 30, 0).unwrap();
        
        for i in 0..1000 {  // 1000 data points
            let price = 100.0 + (i as f64 * 0.1).sin() * 10.0;
            large_data.push(TimeSeriesData {
                symbol: "PERF_TEST".to_string(),
                timestamp: base_time + chrono::Duration::minutes(i * 5),
                open: price - 0.5,
                high: price + 1.0,
                low: price - 1.0,
                close: price,
                volume: 1000000.0 * (1.0 + (i as f64 * 0.1).sin()),
                indicators: HashMap::new(),
                source: Some("test".to_string()),
                entity: Some("PERF_TEST".to_string()),
                value: Some(price),
                metadata: None,
            });
        }
        
        let start_time = std::time::Instant::now();
        let result = engine.extract_features("PERF_TEST", &large_data).await;
        let duration = start_time.elapsed();
        
        assert!(result.is_ok(), "Large dataset feature extraction failed");
        let features = result.unwrap();
        
        println!("Extracted {} features from {} data points in {:?}", 
            features.features.len(), large_data.len(), duration);
        
        // Performance should be reasonable (less than 5 seconds for 1000 points)
        assert!(duration.as_secs() < 5, "Feature extraction took too long: {:?}", duration);
        
        // Verify computation time is recorded
        assert!(features.features.contains_key("_computation_time_ms"));
    }
}