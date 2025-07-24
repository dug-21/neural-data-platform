//! Comprehensive tests for technical indicators module
//! 
//! Tests cover Elliott Wave detection, Harmonic patterns, and all technical indicators

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::TimeSeriesData;
    use crate::features::technical_indicators::{TechnicalIndicatorEngine, IndicatorConfig};
    use chrono::{DateTime, Utc, TimeZone};
    use std::collections::HashMap;
    use approx::assert_relative_eq;

    /// Helper function to create test time series data
    fn create_test_data(prices: Vec<(f64, f64, f64, f64, f64)>) -> Vec<TimeSeriesData> {
        prices.iter().enumerate().map(|(i, &(open, high, low, close, volume))| {
            TimeSeriesData {
                timestamp: Utc.timestamp_opt(1640000000 + (i as i64 * 3600), 0).unwrap(),
                symbol: "TEST".to_string(),
                open,
                high,
                low,
                close,
                volume,
            }
        }).collect()
    }

    /// Create Elliott Wave pattern data
    fn create_elliott_wave_data() -> Vec<TimeSeriesData> {
        // Create a 5-wave impulsive pattern
        let mut prices = vec![];
        
        // Wave 1: Up from 100 to 120
        for i in 0..20 {
            let price = 100.0 + (i as f64);
            prices.push((price - 0.5, price + 0.5, price - 1.0, price, 1000.0 + (i as f64 * 10.0)));
        }
        
        // Wave 2: Down to 110 (50% retracement)
        for i in 0..10 {
            let price = 120.0 - (i as f64);
            prices.push((price + 0.5, price + 1.0, price - 0.5, price, 900.0 - (i as f64 * 10.0)));
        }
        
        // Wave 3: Up to 145 (1.618 extension)
        for i in 0..35 {
            let price = 110.0 + (i as f64);
            prices.push((price - 0.5, price + 0.5, price - 1.0, price, 1200.0 + (i as f64 * 20.0)));
        }
        
        // Wave 4: Down to 135 (38.2% retracement)
        for i in 0..10 {
            let price = 145.0 - (i as f64);
            prices.push((price + 0.5, price + 1.0, price - 0.5, price, 800.0 - (i as f64 * 5.0)));
        }
        
        // Wave 5: Up to 155 (equal to wave 1)
        for i in 0..20 {
            let price = 135.0 + (i as f64);
            prices.push((price - 0.5, price + 0.5, price - 1.0, price, 1000.0 + (i as f64 * 15.0)));
        }
        
        // Add more historical data for pattern detection
        for i in 0..150 {
            let price = 100.0 + (i as f64 * 0.1).sin() * 5.0;
            prices.push((price - 0.5, price + 0.5, price - 1.0, price, 1000.0));
        }
        
        create_test_data(prices)
    }

    /// Create Gartley pattern data
    fn create_gartley_pattern_data() -> Vec<TimeSeriesData> {
        let mut prices = vec![];
        
        // X point: Start at 100
        for _ in 0..10 {
            prices.push((100.0, 100.5, 99.5, 100.0, 1000.0));
        }
        
        // X to A: Move up to 130
        for i in 0..30 {
            let price = 100.0 + (i as f64);
            prices.push((price - 0.5, price + 0.5, price - 1.0, price, 1100.0));
        }
        
        // A to B: Retrace 61.8% to 111.46
        for i in 0..19 {
            let price = 130.0 - (i as f64);
            prices.push((price + 0.5, price + 1.0, price - 0.5, price, 900.0));
        }
        
        // B to C: Move up 38.2-88.6% of AB
        for i in 0..15 {
            let price = 111.46 + (i as f64 * 0.8);
            prices.push((price - 0.5, price + 0.5, price - 1.0, price, 1000.0));
        }
        
        // C to D: Move down to complete pattern at 78.6% of XA
        for i in 0..25 {
            let price = 123.5 - (i as f64 * 0.5);
            prices.push((price + 0.5, price + 1.0, price - 0.5, price, 950.0));
        }
        
        create_test_data(prices)
    }

    #[tokio::test]
    async fn test_elliott_wave_detection() {
        let engine = TechnicalIndicatorEngine::new();
        let data = create_elliott_wave_data();
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = engine.compute_all(current, historical).await.unwrap();
        
        // Check for Elliott Wave features
        assert!(features.contains_key("elliott_wave_type"));
        assert!(features.contains_key("elliott_wave_position"));
        assert!(features.contains_key("elliott_wave_strength"));
        assert!(features.contains_key("elliott_wave_completion"));
        
        // Verify wave type detected (1.0 for impulsive, -1.0 for corrective)
        let wave_type = features.get("elliott_wave_type").unwrap();
        assert!(*wave_type == 1.0 || *wave_type == -1.0);
        
        // Check wave position is between 1 and 5
        if *wave_type == 1.0 {
            let wave_position = features.get("elliott_wave_position").unwrap();
            assert!(*wave_position >= 1.0 && *wave_position <= 5.0);
        }
        
        // Check wave strength is between 0 and 1
        let wave_strength = features.get("elliott_wave_strength").unwrap();
        assert!(*wave_strength >= 0.0 && *wave_strength <= 1.0);
    }

    #[tokio::test]
    async fn test_elliott_wave_fibonacci_relationships() {
        let engine = TechnicalIndicatorEngine::new();
        let data = create_elliott_wave_data();
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = engine.compute_all(current, historical).await.unwrap();
        
        // Check for wave 3 to wave 1 ratio (should be around 1.618)
        if let Some(ratio) = features.get("elliott_wave3_to_wave1_ratio") {
            assert!(*ratio > 1.4 && *ratio < 1.8, "Wave 3/1 ratio should be near 1.618");
        }
        
        // Check for wave targets
        if features.get("elliott_wave_type") == Some(&1.0) {
            assert!(features.contains_key("elliott_wave_target"));
            assert!(features.contains_key("elliott_wave_target_distance"));
        }
    }

    #[tokio::test]
    async fn test_elliott_wave_degree_analysis() {
        let engine = TechnicalIndicatorEngine::new();
        let data = create_elliott_wave_data();
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = engine.compute_all(current, historical).await.unwrap();
        
        // Check for multiple degree trends
        let degrees = vec![21, 55, 89, 144];
        for degree in degrees {
            let key = format!("elliott_degree_{}_trend", degree);
            if features.contains_key(&key) {
                let trend = features.get(&key).unwrap();
                assert!(*trend == -1.0 || *trend == 0.0 || *trend == 1.0);
            }
        }
    }

    #[tokio::test]
    async fn test_harmonic_pattern_gartley() {
        let engine = TechnicalIndicatorEngine::new();
        let data = create_gartley_pattern_data();
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = engine.compute_all(current, historical).await.unwrap();
        
        // Check for Gartley pattern detection
        if let Some(gartley) = features.get("harmonic_pattern_gartley") {
            assert!(*gartley == 1.0 || *gartley == -1.0);
            
            // If Gartley detected, check completion and target
            if features.get("harmonic_gartley_completion") == Some(&1.0) {
                assert!(features.contains_key("harmonic_gartley_target"));
            }
        }
        
        // Check harmonic ratios
        assert!(features.contains_key("harmonic_ab_xa_ratio"));
        assert!(features.contains_key("harmonic_bc_ab_ratio"));
        assert!(features.contains_key("harmonic_cd_bc_ratio"));
        assert!(features.contains_key("harmonic_ad_xa_ratio"));
    }

    #[tokio::test]
    async fn test_harmonic_pattern_ratios() {
        let engine = TechnicalIndicatorEngine::new();
        let data = create_gartley_pattern_data();
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = engine.compute_all(current, historical).await.unwrap();
        
        // Verify ratio ranges
        if let Some(ab_xa) = features.get("harmonic_ab_xa_ratio") {
            assert!(*ab_xa > 0.0 && *ab_xa < 2.0, "AB/XA ratio should be positive");
        }
        
        if let Some(bc_ab) = features.get("harmonic_bc_ab_ratio") {
            assert!(*bc_ab > 0.0 && *bc_ab < 2.0, "BC/AB ratio should be positive");
        }
        
        // Check pattern potential score
        assert!(features.contains_key("harmonic_pattern_potential"));
        let potential = features.get("harmonic_pattern_potential").unwrap();
        assert!(*potential >= 0.0 && *potential <= 1.0);
    }

    #[tokio::test]
    async fn test_all_harmonic_patterns() {
        let engine = TechnicalIndicatorEngine::new();
        let data = create_gartley_pattern_data();
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = engine.compute_all(current, historical).await.unwrap();
        
        // Check for all harmonic pattern types
        let patterns = vec!["gartley", "bat", "butterfly", "crab", "abcd"];
        
        let mut pattern_found = false;
        for pattern in patterns {
            let key = format!("harmonic_pattern_{}", pattern);
            if features.contains_key(&key) {
                pattern_found = true;
                let value = features.get(&key).unwrap();
                assert!(*value == -1.0 || *value == 0.0 || *value == 1.0);
            }
        }
        
        // At least one pattern or pattern potential should be detected
        assert!(pattern_found || features.get("harmonic_pattern_potential").unwrap() > &0.0);
    }

    #[tokio::test]
    async fn test_technical_indicators_basic() {
        let engine = TechnicalIndicatorEngine::new();
        let prices = vec![
            (100.0, 102.0, 99.0, 101.0, 1000.0),
            (101.0, 103.0, 100.0, 102.0, 1100.0),
            (102.0, 104.0, 101.0, 103.0, 1200.0),
            (103.0, 105.0, 102.0, 104.0, 1300.0),
            (104.0, 106.0, 103.0, 105.0, 1400.0),
        ];
        let data = create_test_data(prices);
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = engine.compute_all(current, historical).await.unwrap();
        
        // Check basic price features
        assert!(features.contains_key("high_low_ratio"));
        assert!(features.contains_key("close_open_ratio"));
        assert!(features.contains_key("close_position_in_range"));
        
        let high_low_ratio = features.get("high_low_ratio").unwrap();
        assert_relative_eq!(*high_low_ratio, 106.0 / 103.0, epsilon = 0.0001);
    }

    #[tokio::test]
    async fn test_momentum_indicators() {
        let engine = TechnicalIndicatorEngine::new();
        let mut prices = vec![];
        
        // Create trending data for RSI testing
        for i in 0..20 {
            let price = 100.0 + (i as f64);
            prices.push((price - 0.5, price + 0.5, price - 1.0, price, 1000.0));
        }
        
        let data = create_test_data(prices);
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = engine.compute_all(current, historical).await.unwrap();
        
        // Check momentum indicators
        assert!(features.contains_key("rsi"));
        assert!(features.contains_key("rsi_oversold"));
        assert!(features.contains_key("rsi_overbought"));
        assert!(features.contains_key("williams_r"));
        assert!(features.contains_key("cci"));
        
        // RSI should be high for uptrend
        let rsi = features.get("rsi").unwrap();
        assert!(*rsi > 50.0, "RSI should be above 50 for uptrend");
    }

    #[tokio::test]
    async fn test_volatility_indicators() {
        let engine = TechnicalIndicatorEngine::new();
        let mut prices = vec![];
        
        // Create volatile data
        for i in 0..30 {
            let base = 100.0;
            let volatility = (i as f64 * 0.5).sin() * 5.0;
            let price = base + volatility;
            prices.push((price - 2.0, price + 2.0, price - 3.0, price, 1000.0));
        }
        
        let data = create_test_data(prices);
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = engine.compute_all(current, historical).await.unwrap();
        
        // Check volatility indicators
        assert!(features.contains_key("atr"));
        assert!(features.contains_key("atr_percentage"));
        assert!(features.contains_key("bb_middle"));
        assert!(features.contains_key("bb_upper"));
        assert!(features.contains_key("bb_lower"));
        assert!(features.contains_key("bb_width"));
        assert!(features.contains_key("bb_position"));
        
        // ATR should be positive
        let atr = features.get("atr").unwrap();
        assert!(*atr > 0.0, "ATR should be positive");
        
        // Bollinger Bands should be ordered correctly
        let bb_upper = features.get("bb_upper").unwrap();
        let bb_middle = features.get("bb_middle").unwrap();
        let bb_lower = features.get("bb_lower").unwrap();
        assert!(*bb_upper > *bb_middle && *bb_middle > *bb_lower);
    }

    #[tokio::test]
    async fn test_volume_indicators() {
        let engine = TechnicalIndicatorEngine::new();
        let mut prices = vec![];
        
        // Create data with increasing volume
        for i in 0..20 {
            let price = 100.0 + (i as f64 * 0.5);
            let volume = 1000.0 + (i as f64 * 100.0);
            prices.push((price - 0.5, price + 0.5, price - 1.0, price, volume));
        }
        
        let data = create_test_data(prices);
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = engine.compute_all(current, historical).await.unwrap();
        
        // Check volume indicators
        assert!(features.contains_key("volume_roc"));
        assert!(features.contains_key("obv_trend"));
        assert!(features.contains_key("vwap"));
        assert!(features.contains_key("price_to_vwap_ratio"));
        assert!(features.contains_key("mfi"));
        assert!(features.contains_key("ad_line_slope"));
        
        // Volume ROC should be positive for increasing volume
        let volume_roc = features.get("volume_roc").unwrap();
        assert!(*volume_roc > 0.0, "Volume ROC should be positive");
    }

    #[tokio::test]
    async fn test_trend_indicators() {
        let engine = TechnicalIndicatorEngine::new();
        let mut prices = vec![];
        
        // Create trending data
        for i in 0..100 {
            let price = 100.0 + (i as f64 * 0.2);
            prices.push((price - 0.5, price + 0.5, price - 1.0, price, 1000.0));
        }
        
        let data = create_test_data(prices);
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = engine.compute_all(current, historical).await.unwrap();
        
        // Check EMAs
        for period in &[9, 21, 50] {
            assert!(features.contains_key(&format!("ema_{}", period)));
            assert!(features.contains_key(&format!("price_to_ema_{}_ratio", period)));
        }
        
        // Check MACD
        assert!(features.contains_key("macd_line"));
        assert!(features.contains_key("macd_signal"));
        assert!(features.contains_key("macd_histogram"));
        assert!(features.contains_key("macd_crossover"));
        
        // Check ADX
        assert!(features.contains_key("adx"));
        assert!(features.contains_key("trending_market"));
        
        // In strong trend, shorter EMA > longer EMA
        let ema_9 = features.get("ema_9").unwrap();
        let ema_21 = features.get("ema_21").unwrap();
        assert!(*ema_9 > *ema_21, "In uptrend, EMA9 should be above EMA21");
    }

    #[tokio::test]
    async fn test_custom_indicators() {
        let engine = TechnicalIndicatorEngine::new();
        let prices = vec![
            (100.0, 105.0, 98.0, 103.0, 1000.0),
            (103.0, 107.0, 101.0, 105.0, 1200.0),
            (105.0, 108.0, 103.0, 104.0, 1100.0),
            (104.0, 106.0, 102.0, 105.0, 1300.0),
            (105.0, 107.0, 104.0, 106.0, 1400.0),
        ];
        let data = create_test_data(prices);
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = engine.compute_all(current, historical).await.unwrap();
        
        // Check Heikin-Ashi
        assert!(features.contains_key("ha_body_size"));
        assert!(features.contains_key("ha_upper_shadow"));
        assert!(features.contains_key("ha_lower_shadow"));
        assert!(features.contains_key("ha_trend"));
        
        // Check pivot points
        assert!(features.contains_key("pivot_point"));
        assert!(features.contains_key("resistance_1"));
        assert!(features.contains_key("resistance_2"));
        assert!(features.contains_key("support_1"));
        assert!(features.contains_key("support_2"));
    }

    #[tokio::test]
    async fn test_fibonacci_levels() {
        let engine = TechnicalIndicatorEngine::new();
        let mut prices = vec![];
        
        // Create data with clear high/low for Fibonacci
        for i in 0..50 {
            let price = 100.0 + (i as f64);
            prices.push((price - 0.5, price + 0.5, price - 1.0, price, 1000.0));
        }
        for i in 0..50 {
            let price = 150.0 - (i as f64);
            prices.push((price + 0.5, price + 1.0, price - 0.5, price, 900.0));
        }
        
        let data = create_test_data(prices);
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = engine.compute_all(current, historical).await.unwrap();
        
        // Check Fibonacci levels
        let fib_levels = vec!["0", "236", "382", "500", "618", "786", "100"];
        for level in fib_levels {
            assert!(features.contains_key(&format!("fib_{}_level", level)));
            assert!(features.contains_key(&format!("fib_{}_distance", level)));
        }
        
        // Verify Fibonacci levels are ordered correctly
        let fib_0 = features.get("fib_0_level").unwrap();
        let fib_100 = features.get("fib_100_level").unwrap();
        assert!(*fib_0 < *fib_100, "Fib 0% should be below 100%");
    }

    #[tokio::test]
    async fn test_edge_cases() {
        let engine = TechnicalIndicatorEngine::new();
        
        // Test with minimal data
        let prices = vec![
            (100.0, 101.0, 99.0, 100.0, 1000.0),
            (100.0, 101.0, 99.0, 100.0, 1000.0),
        ];
        let data = create_test_data(prices);
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = engine.compute_all(current, historical).await.unwrap();
        assert!(!features.is_empty(), "Should return some features even with minimal data");
        
        // Test with zero volume
        let prices_zero_vol = vec![
            (100.0, 101.0, 99.0, 100.0, 0.0),
            (100.0, 101.0, 99.0, 100.0, 0.0),
        ];
        let data_zero_vol = create_test_data(prices_zero_vol);
        let current_zero = data_zero_vol.last().unwrap();
        let historical_zero = &data_zero_vol[..data_zero_vol.len() - 1];
        
        let features_zero = engine.compute_all(current_zero, historical_zero).await.unwrap();
        assert!(!features_zero.is_empty(), "Should handle zero volume gracefully");
        
        // Test with identical prices
        let prices_flat = vec![
            (100.0, 100.0, 100.0, 100.0, 1000.0),
            (100.0, 100.0, 100.0, 100.0, 1000.0),
            (100.0, 100.0, 100.0, 100.0, 1000.0),
        ];
        let data_flat = create_test_data(prices_flat);
        let current_flat = data_flat.last().unwrap();
        let historical_flat = &data_flat[..data_flat.len() - 1];
        
        let features_flat = engine.compute_all(current_flat, historical_flat).await.unwrap();
        assert!(!features_flat.is_empty(), "Should handle flat prices gracefully");
    }

    #[tokio::test]
    async fn test_performance_benchmark() {
        use std::time::Instant;
        
        let engine = TechnicalIndicatorEngine::new();
        let mut prices = vec![];
        
        // Create large dataset
        for i in 0..1000 {
            let price = 100.0 + (i as f64 * 0.1).sin() * 10.0;
            let volume = 1000.0 + (i as f64 * 0.2).cos() * 200.0;
            prices.push((price - 1.0, price + 1.0, price - 1.5, price, volume));
        }
        
        let data = create_test_data(prices);
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let start = Instant::now();
        let features = engine.compute_all(current, historical).await.unwrap();
        let duration = start.elapsed();
        
        println!("Computed {} features in {:?}", features.len(), duration);
        assert!(features.len() > 100, "Should compute many features");
        assert!(duration.as_millis() < 1000, "Should complete within 1 second");
    }

    #[tokio::test]
    async fn test_custom_config() {
        let config = IndicatorConfig {
            ema_periods: vec![5, 10, 20],
            rsi_period: 10,
            macd_params: (8, 17, 9),
            bb_params: (10, 1.5),
            atr_period: 10,
            stoch_params: (10, 3),
            enable_volume_weighted: false,
            enable_custom: false,
        };
        
        let engine = TechnicalIndicatorEngine::with_config(config);
        let prices = vec![
            (100.0, 102.0, 99.0, 101.0, 1000.0),
            (101.0, 103.0, 100.0, 102.0, 1100.0),
            (102.0, 104.0, 101.0, 103.0, 1200.0),
            (103.0, 105.0, 102.0, 104.0, 1300.0),
            (104.0, 106.0, 103.0, 105.0, 1400.0),
        ];
        let data = create_test_data(prices);
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = engine.compute_all(current, historical).await.unwrap();
        
        // Check custom EMA periods
        assert!(features.contains_key("ema_5"));
        assert!(features.contains_key("ema_10"));
        assert!(features.contains_key("ema_20"));
        assert!(!features.contains_key("ema_50")); // Not in custom config
        
        // Volume indicators should be disabled
        assert!(!features.contains_key("vwap"));
        assert!(!features.contains_key("mfi"));
        
        // Custom indicators should be disabled
        assert!(!features.contains_key("elliott_wave_type"));
        assert!(!features.contains_key("harmonic_pattern_gartley"));
    }
}