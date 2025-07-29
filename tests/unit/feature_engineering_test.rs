//! Unit tests for Feature Engineering module
//! Tests technical indicators, feature pipeline, and advanced features

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use autonomous_platform::features::{
    FeatureEngineeringPipeline, FeaturePipelineConfig, FeatureCategory,
    FeatureResult, ComputationStats, FeatureMetadata, FeatureError,
    technical_indicators::{TechnicalIndicatorEngine, IndicatorConfig},
};
use autonomous_platform::data::TimeSeriesData;

// Helper function to create test time series data
fn create_test_ohlcv_data(symbol: &str, timestamp: DateTime<Utc>, base_price: f64, volume: f64) -> TimeSeriesData {
    TimeSeriesData {
        symbol: symbol.to_string(),
        timestamp,
        open: base_price,
        high: base_price + 50.0,
        low: base_price - 50.0,
        close: base_price + 10.0,
        volume,
        indicators: HashMap::new(),
        source: Some("test".to_string()),
        entity: Some(symbol.to_string()),
        value: Some(base_price + 10.0),
        metadata: None,
    }
}

// Helper to create a sequence of realistic price data
fn create_price_sequence(symbol: &str, start_time: DateTime<Utc>, count: usize, base_price: f64) -> Vec<TimeSeriesData> {
    let mut data = Vec::new();
    let mut price = base_price;
    
    for i in 0..count {
        let timestamp = start_time + Duration::minutes(i as i64);
        
        // Create more realistic price movement
        let change = ((i as f64 * 0.1).sin() * 20.0) + (rand::random::<f64>() - 0.5) * 10.0;
        price += change;
        
        let volume = 1000.0 + (rand::random::<f64>() * 500.0);
        
        data.push(TimeSeriesData {
            symbol: symbol.to_string(),
            timestamp,
            open: price - 5.0,
            high: price + (rand::random::<f64>() * 20.0),
            low: price - (rand::random::<f64>() * 20.0),
            close: price,
            volume,
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some(symbol.to_string()),
            value: Some(price),
            metadata: None,
        });
    }
    
    data
}

#[tokio::test]
async fn test_technical_indicator_engine_creation() {
    let engine = TechnicalIndicatorEngine::new();
    
    // Test default configuration
    let default_engine = TechnicalIndicatorEngine::default();
    
    // Test custom configuration
    let custom_config = IndicatorConfig {
        ema_periods: vec![12, 26, 50],
        rsi_period: 21,
        macd_params: (12, 26, 9),
        bb_params: (20, 2.5),
        atr_period: 21,
        stoch_params: (14, 3),
        enable_volume_weighted: true,
        enable_custom: true,
    };
    
    let custom_engine = TechnicalIndicatorEngine::with_config(custom_config);
    
    // Just verify they can be created without panicking
    assert!(true); // If we get here, creation succeeded
}

#[tokio::test]
async fn test_compute_all_indicators() {
    let engine = TechnicalIndicatorEngine::new();
    let symbol = "BTC/USD";
    let start_time = Utc::now() - Duration::hours(24);
    
    // Create sufficient historical data for all indicators (200+ periods)
    let historical_data = create_price_sequence(symbol, start_time, 250, 50000.0);
    let current = create_test_ohlcv_data(symbol, Utc::now(), 50000.0, 1500.0);
    
    let result = engine.compute_all(&current, &historical_data).await;
    
    assert!(result.is_ok());
    let features = result.unwrap();
    
    // Verify basic price features
    assert!(features.contains_key("high_low_ratio"));
    assert!(features.contains_key("close_open_ratio"));
    assert!(features.contains_key("close_position_in_range"));
    
    // Verify momentum indicators
    assert!(features.contains_key("rsi"));
    assert!(features.contains_key("rsi_oversold"));
    assert!(features.contains_key("rsi_overbought"));
    assert!(features.contains_key("williams_r"));
    
    // Verify volatility indicators
    assert!(features.contains_key("atr"));
    assert!(features.contains_key("bb_middle"));
    assert!(features.contains_key("bb_upper"));
    assert!(features.contains_key("bb_lower"));
    
    // Verify trend indicators
    assert!(features.contains_key("ema_9"));
    assert!(features.contains_key("ema_21"));
    assert!(features.contains_key("macd_line"));
    assert!(features.contains_key("macd_signal"));
    
    // Verify value ranges are reasonable
    let rsi = features.get("rsi").unwrap();
    assert!(*rsi >= 0.0 && *rsi <= 100.0, "RSI should be between 0 and 100, got {}", rsi);
    
    let high_low_ratio = features.get("high_low_ratio").unwrap();
    assert!(*high_low_ratio > 1.0, "High/Low ratio should be > 1.0, got {}", high_low_ratio);
}

#[tokio::test]
async fn test_price_features_computation() {
    let engine = TechnicalIndicatorEngine::new();
    let current = create_test_ohlcv_data("BTC/USD", Utc::now(), 50000.0, 1000.0);
    let historical = create_price_sequence("BTC/USD", Utc::now() - Duration::hours(1), 60, 50000.0);
    
    let result = engine.compute_all(&current, &historical).await;
    assert!(result.is_ok());
    
    let features = result.unwrap();
    
    // Test high/low ratio
    let high_low_ratio = features.get("high_low_ratio").unwrap();
    let expected_ratio = current.high / current.low;
    assert!((high_low_ratio - expected_ratio).abs() < 0.001);
    
    // Test close/open ratio
    let close_open_ratio = features.get("close_open_ratio").unwrap();
    let expected_close_open = current.close / current.open;
    assert!((close_open_ratio - expected_close_open).abs() < 0.001);
    
    // Test position in range
    let position_in_range = features.get("close_position_in_range").unwrap();
    assert!(*position_in_range >= 0.0 && *position_in_range <= 1.0);
}

#[tokio::test]
async fn test_momentum_indicators() {
    let engine = TechnicalIndicatorEngine::new();
    let symbol = "BTC/USD";
    let start_time = Utc::now() - Duration::hours(2);
    
    // Create historical data with known pattern
    let mut historical = Vec::new();
    let mut price = 50000.0;
    
    // Create uptrend for first half, downtrend for second half
    for i in 0..100 {
        let timestamp = start_time + Duration::minutes(i);
        if i < 50 {
            price += 10.0; // Uptrend
        } else {
            price -= 5.0; // Downtrend
        }
        
        historical.push(TimeSeriesData {
            symbol: symbol.to_string(),
            timestamp,
            open: price - 2.0,
            high: price + 5.0,
            low: price - 5.0,
            close: price,
            volume: 1000.0,
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some(symbol.to_string()),
            value: Some(price),
            metadata: None,
        });
    }
    
    let current = create_test_ohlcv_data(symbol, Utc::now(), price, 1000.0);
    
    let result = engine.compute_all(&current, &historical).await;
    assert!(result.is_ok());
    
    let features = result.unwrap();
    
    // RSI should reflect the recent downtrend
    let rsi = features.get("rsi").unwrap();
    assert!(*rsi >= 0.0 && *rsi <= 100.0);
    
    // Williams %R should be between -100 and 0
    let williams_r = features.get("williams_r").unwrap();
    assert!(*williams_r >= -100.0 && *williams_r <= 0.0);
    
    // Rate of change should be available
    assert!(features.contains_key("roc_5"));
    assert!(features.contains_key("roc_10"));
    assert!(features.contains_key("roc_20"));
}

#[tokio::test]
async fn test_volatility_indicators() {
    let engine = TechnicalIndicatorEngine::new();
    let symbol = "BTC/USD";
    let start_time = Utc::now() - Duration::hours(1);
    
    // Create data with varying volatility
    let mut historical = Vec::new();
    let base_price = 50000.0;
    
    for i in 0..60 {
        let timestamp = start_time + Duration::minutes(i);
        let volatility = if i < 30 { 10.0 } else { 50.0 }; // Low then high volatility
        
        let price = base_price + ((i as f64 * 0.2).sin() * volatility);
        
        historical.push(TimeSeriesData {
            symbol: symbol.to_string(),
            timestamp,
            open: price,
            high: price + volatility,
            low: price - volatility,
            close: price + (volatility * 0.1),
            volume: 1000.0,
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some(symbol.to_string()),
            value: Some(price),
            metadata: None,
        });
    }
    
    let current = create_test_ohlcv_data(symbol, Utc::now(), base_price, 1000.0);
    
    let result = engine.compute_all(&current, &historical).await;
    assert!(result.is_ok());
    
    let features = result.unwrap();
    
    // ATR should be positive
    let atr = features.get("atr").unwrap();
    assert!(*atr > 0.0);
    
    // Bollinger Bands should be in correct order
    let bb_upper = features.get("bb_upper").unwrap();
    let bb_middle = features.get("bb_middle").unwrap();
    let bb_lower = features.get("bb_lower").unwrap();
    
    assert!(*bb_upper > *bb_middle);
    assert!(*bb_middle > *bb_lower);
    
    // BB width should be positive
    let bb_width = features.get("bb_width").unwrap();
    assert!(*bb_width > 0.0);
    
    // Historical volatility should be available
    assert!(features.contains_key("volatility_10"));
    assert!(features.contains_key("volatility_20"));
}

#[tokio::test]
async fn test_volume_indicators() {
    let engine = TechnicalIndicatorEngine::with_config(IndicatorConfig {
        enable_volume_weighted: true,
        ..Default::default()
    });
    
    let symbol = "BTC/USD";
    let start_time = Utc::now() - Duration::hours(1);
    
    // Create data with varying volume
    let mut historical = Vec::new();
    let mut price = 50000.0;
    
    for i in 0..60 {
        let timestamp = start_time + Duration::minutes(i);
        price += ((i as f64 * 0.1).sin() * 20.0);
        let volume = 1000.0 + (i as f64 * 50.0); // Increasing volume
        
        historical.push(TimeSeriesData {
            symbol: symbol.to_string(),
            timestamp,
            open: price - 5.0,
            high: price + 10.0,
            low: price - 10.0,
            close: price,
            volume,
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some(symbol.to_string()),
            value: Some(price),
            metadata: None,
        });
    }
    
    let current = create_test_ohlcv_data(symbol, Utc::now(), price, 5000.0);
    
    let result = engine.compute_all(&current, &historical).await;
    assert!(result.is_ok());
    
    let features = result.unwrap();
    
    // Volume-based indicators should be present
    assert!(features.contains_key("volume_roc"));
    assert!(features.contains_key("vwap"));
    assert!(features.contains_key("price_to_vwap_ratio"));
    assert!(features.contains_key("obv_trend"));
    
    // VWAP should be positive
    let vwap = features.get("vwap").unwrap();
    assert!(*vwap > 0.0);
    
    // Price to VWAP ratio should be reasonable
    let price_vwap_ratio = features.get("price_to_vwap_ratio").unwrap();
    assert!(*price_vwap_ratio > 0.0);
}

#[tokio::test]
async fn test_custom_indicators() {
    let engine = TechnicalIndicatorEngine::with_config(IndicatorConfig {
        enable_custom: true,
        ..Default::default()
    });
    
    let symbol = "BTC/USD";
    let start_time = Utc::now() - Duration::hours(2);
    let historical = create_price_sequence(symbol, start_time, 120, 50000.0);
    let current = create_test_ohlcv_data(symbol, Utc::now(), 50000.0, 1000.0);
    
    let result = engine.compute_all(&current, &historical).await;
    assert!(result.is_ok());
    
    let features = result.unwrap();
    
    // Heikin-Ashi features
    assert!(features.contains_key("ha_body_size"));
    assert!(features.contains_key("ha_upper_shadow"));
    assert!(features.contains_key("ha_lower_shadow"));
    assert!(features.contains_key("ha_trend"));
    
    // Pivot points
    assert!(features.contains_key("pivot_point"));
    assert!(features.contains_key("resistance_1"));
    assert!(features.contains_key("support_1"));
    
    // Fibonacci levels (if enough historical data)
    if historical.len() >= 100 {
        assert!(features.contains_key("fib_236_level"));
        assert!(features.contains_key("fib_382_level"));
        assert!(features.contains_key("fib_618_level"));
    }
}

#[tokio::test]
async fn test_ichimoku_cloud() {
    let engine = TechnicalIndicatorEngine::new();
    let symbol = "BTC/USD";
    let start_time = Utc::now() - Duration::hours(4);
    
    // Need at least 52 periods for Ichimoku
    let historical = create_price_sequence(symbol, start_time, 60, 50000.0);
    let current = create_test_ohlcv_data(symbol, Utc::now(), 50000.0, 1000.0);
    
    let result = engine.compute_all(&current, &historical).await;
    assert!(result.is_ok());
    
    let features = result.unwrap();
    
    // Ichimoku components
    assert!(features.contains_key("ichimoku_tenkan"));
    assert!(features.contains_key("ichimoku_kijun"));
    assert!(features.contains_key("ichimoku_senkou_a"));
    assert!(features.contains_key("ichimoku_senkou_b"));
    assert!(features.contains_key("ichimoku_cloud_thickness"));
    assert!(features.contains_key("ichimoku_position"));
    assert!(features.contains_key("ichimoku_tk_cross"));
    
    // Cloud thickness should be positive
    let cloud_thickness = features.get("ichimoku_cloud_thickness").unwrap();
    assert!(*cloud_thickness >= 0.0);
    
    // Position should be -1, 0, or 1
    let position = features.get("ichimoku_position").unwrap();
    assert!(*position >= -1.0 && *position <= 1.0);
}

#[tokio::test]
async fn test_feature_pipeline_config() {
    let config = FeaturePipelineConfig::default();
    
    assert!(config.enable_realtime);
    assert_eq!(config.max_features, 500);
    assert_eq!(config.importance_threshold, 0.01);
    assert!(config.enable_caching);
    assert_eq!(config.cache_ttl_seconds, 300);
    assert!(config.enable_parallel);
    assert_eq!(config.num_workers, 4);
    assert_eq!(config.memory_limit_mb, 1024.0);
    assert!(config.enable_adaptive_selection);
    assert_eq!(config.update_frequency_seconds, 60);
}

#[tokio::test]
async fn test_feature_category_enum() {
    let categories = vec![
        FeatureCategory::Price,
        FeatureCategory::Volume,
        FeatureCategory::Volatility,
        FeatureCategory::Momentum,
        FeatureCategory::MeanReversion,
        FeatureCategory::MarketMicrostructure,
        FeatureCategory::OrderFlow,
        FeatureCategory::CrossAsset,
        FeatureCategory::Sentiment,
        FeatureCategory::Regime,
        FeatureCategory::Custom,
    ];
    
    // Just verify all variants exist
    assert_eq!(categories.len(), 11);
}

#[tokio::test]
async fn test_feature_metadata() {
    let metadata = FeatureMetadata {
        name: "rsi_14".to_string(),
        category: FeatureCategory::Momentum,
        computation_time_ms: 1.5,
        memory_usage_mb: 0.1,
        importance_score: Some(0.85),
        dependencies: vec!["close_price".to_string()],
        version: "1.0.0".to_string(),
        last_updated: Utc::now(),
    };
    
    assert_eq!(metadata.name, "rsi_14");
    assert_eq!(metadata.category, FeatureCategory::Momentum);
    assert_eq!(metadata.computation_time_ms, 1.5);
    assert_eq!(metadata.importance_score, Some(0.85));
    assert_eq!(metadata.dependencies.len(), 1);
}

#[tokio::test]
async fn test_computation_stats() {
    let start_time = Utc::now();
    let end_time = start_time + Duration::milliseconds(150);
    
    let stats = ComputationStats {
        start_time,
        end_time,
        records_processed: 100,
        errors: vec!["Warning: Low data quality".to_string()],
        warnings: vec!["Gap detected in data".to_string()],
    };
    
    assert_eq!(stats.records_processed, 100);
    assert_eq!(stats.errors.len(), 1);
    assert_eq!(stats.warnings.len(), 1);
    assert!(stats.end_time > stats.start_time);
}

#[tokio::test]
async fn test_insufficient_data_handling() {
    let engine = TechnicalIndicatorEngine::new();
    let symbol = "BTC/USD";
    
    // Create minimal data (insufficient for most indicators)
    let historical = vec![
        create_test_ohlcv_data(symbol, Utc::now() - Duration::minutes(2), 50000.0, 1000.0),
        create_test_ohlcv_data(symbol, Utc::now() - Duration::minutes(1), 50100.0, 1100.0),
    ];
    let current = create_test_ohlcv_data(symbol, Utc::now(), 50200.0, 1200.0);
    
    let result = engine.compute_all(&current, &historical).await;
    assert!(result.is_ok());
    
    let features = result.unwrap();
    
    // Should still have basic price features
    assert!(features.contains_key("high_low_ratio"));
    assert!(features.contains_key("close_open_ratio"));
    
    // Should have gap features since we have historical data
    assert!(features.contains_key("gap_percentage"));
    
    // Complex indicators might not be present or have default values
    // This tests graceful degradation
}

#[tokio::test]
async fn test_elliott_wave_pattern_detection() {
    let engine = TechnicalIndicatorEngine::with_config(IndicatorConfig {
        enable_custom: true,
        ..Default::default()
    });
    
    let symbol = "BTC/USD";
    let start_time = Utc::now() - Duration::hours(10);
    
    // Create data with wave-like pattern (need 240+ periods for Elliott Wave)
    let mut historical = Vec::new();
    let base_price = 50000.0;
    
    for i in 0..250 {
        let timestamp = start_time + Duration::minutes(i * 2);
        
        // Create 5-wave pattern
        let wave_progress = (i as f64) / 250.0;
        let price_movement = if wave_progress < 0.2 {
            // Wave 1: Up
            (wave_progress * 5.0) * 200.0
        } else if wave_progress < 0.35 {
            // Wave 2: Down (retracement)
            200.0 - ((wave_progress - 0.2) * 6.67) * 120.0
        } else if wave_progress < 0.65 {
            // Wave 3: Up (strongest)
            80.0 + ((wave_progress - 0.35) * 3.33) * 300.0
        } else if wave_progress < 0.8 {
            // Wave 4: Down (retracement)
            380.0 - ((wave_progress - 0.65) * 6.67) * 100.0
        } else {
            // Wave 5: Up (final)
            280.0 + ((wave_progress - 0.8) * 5.0) * 150.0
        };
        
        let price = base_price + price_movement;
        
        historical.push(TimeSeriesData {
            symbol: symbol.to_string(),
            timestamp,
            open: price - 10.0,
            high: price + 20.0,
            low: price - 20.0,
            close: price,
            volume: 1000.0,
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some(symbol.to_string()),
            value: Some(price),
            metadata: None,
        });
    }
    
    let current = create_test_ohlcv_data(symbol, Utc::now(), base_price + 430.0, 1000.0);
    
    let result = engine.compute_all(&current, &historical).await;
    assert!(result.is_ok());
    
    let features = result.unwrap();
    
    // Elliott Wave features might be present
    if features.contains_key("elliott_wave_type") {
        let wave_type = features.get("elliott_wave_type").unwrap();
        assert!(*wave_type == 1.0 || *wave_type == -1.0); // Impulsive or Corrective
        
        if features.contains_key("elliott_wave_position") {
            let position = features.get("elliott_wave_position").unwrap();
            assert!(*position >= 1.0 && *position <= 5.0); // Wave position
        }
    }
}

#[tokio::test]
async fn test_harmonic_pattern_detection() {
    let engine = TechnicalIndicatorEngine::with_config(IndicatorConfig {
        enable_custom: true,
        ..Default::default()
    });
    
    let symbol = "BTC/USD";
    let start_time = Utc::now() - Duration::hours(5);
    
    // Create data for harmonic pattern (need 100+ periods)
    let historical = create_price_sequence(symbol, start_time, 120, 50000.0);
    let current = create_test_ohlcv_data(symbol, Utc::now(), 50000.0, 1000.0);
    
    let result = engine.compute_all(&current, &historical).await;
    assert!(result.is_ok());
    
    let features = result.unwrap();
    
    // Harmonic pattern ratios should be present
    assert!(features.contains_key("harmonic_ab_xa_ratio"));
    assert!(features.contains_key("harmonic_bc_ab_ratio"));
    assert!(features.contains_key("harmonic_cd_bc_ratio"));
    assert!(features.contains_key("harmonic_ad_xa_ratio"));
    
    // Pattern potential should be between 0 and 1
    if let Some(potential) = features.get("harmonic_pattern_potential") {
        assert!(*potential >= 0.0 && *potential <= 1.0);
    }
}

#[tokio::test]
async fn test_feature_error_types() {
    // Test the error enum variants
    let errors = vec![
        FeatureError::InsufficientData("Need more data".to_string()),
        FeatureError::ComputationError("Division by zero".to_string()),
        FeatureError::StorageError("Database connection failed".to_string()),
        FeatureError::ConfigurationError("Invalid parameter".to_string()),
        FeatureError::MemoryLimitExceeded("Out of memory".to_string()),
    ];
    
    for error in errors {
        let error_string = error.to_string();
        assert!(!error_string.is_empty());
    }
}

#[tokio::test]
async fn test_rsi_boundary_conditions() {
    let engine = TechnicalIndicatorEngine::new();
    let symbol = "BTC/USD";
    
    // Test RSI with all gains (should approach 100)
    let mut historical_all_gains = Vec::new();
    let mut price = 50000.0;
    
    for i in 0..30 {
        price += 100.0; // Only gains
        let timestamp = Utc::now() - Duration::minutes((30 - i) as i64);
        
        historical_all_gains.push(TimeSeriesData {
            symbol: symbol.to_string(),
            timestamp,
            open: price - 100.0,
            high: price + 50.0,
            low: price - 50.0,
            close: price,
            volume: 1000.0,
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some(symbol.to_string()),
            value: Some(price),
            metadata: None,
        });
    }
    
    let current_gain = create_test_ohlcv_data(symbol, Utc::now(), price + 100.0, 1000.0);
    
    let result = engine.compute_all(&current_gain, &historical_all_gains).await;
    assert!(result.is_ok());
    
    let features = result.unwrap();
    let rsi = features.get("rsi").unwrap();
    
    // RSI should be high (but not necessarily 100 due to calculation method)
    assert!(*rsi > 70.0, "RSI should be high with all gains, got {}", rsi);
    assert!(*rsi <= 100.0, "RSI should not exceed 100, got {}", rsi);
}

// Property-based test helper
#[tokio::test]
async fn test_indicator_properties() {
    let engine = TechnicalIndicatorEngine::new();
    let symbol = "BTC/USD";
    
    // Test with various data sizes
    for data_size in vec![20, 50, 100, 200] {
        let historical = create_price_sequence(symbol, Utc::now() - Duration::hours(data_size), data_size, 50000.0);
        let current = create_test_ohlcv_data(symbol, Utc::now(), 50000.0, 1000.0);
        
        let result = engine.compute_all(&current, &historical).await;
        assert!(result.is_ok(), "Failed with data size {}", data_size);
        
        let features = result.unwrap();
        
        // All features should have finite values
        for (name, value) in features.iter() {
            assert!(value.is_finite(), "Feature {} has non-finite value: {}", name, value);
        }
        
        // RSI should always be between 0 and 100
        if let Some(rsi) = features.get("rsi") {
            assert!(*rsi >= 0.0 && *rsi <= 100.0, "RSI out of bounds: {}", rsi);
        }
        
        // Ratios should be positive
        if let Some(ratio) = features.get("high_low_ratio") {
            assert!(*ratio > 0.0, "High/Low ratio should be positive: {}", ratio);
        }
    }
}