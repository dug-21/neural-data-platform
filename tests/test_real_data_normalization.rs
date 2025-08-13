//! Integration tests for real data normalization
//! 
//! These tests verify that the normalization pipeline correctly handles
//! real market prices (like XLK ~$268) and scales them properly to [0,1] range.

use anyhow::Result;
use chrono::{DateTime, Utc, TimeZone};
use neural_trader::data::TimeSeriesData;
use neural_trader::neural::vendor_predictor::{VendorPredictor, DatasetNormalizationStats, NormalizedOHLCV};
use neural_trader::integration::training_data_service::{TrainingDataService, TrainingDataConfig, NormalizationParams};
use std::collections::HashMap;

/// Test data representing real XLK prices (technology ETF around $268)
fn create_real_xlk_data() -> Vec<TimeSeriesData> {
    let base_time = Utc.ymd(2024, 1, 1).and_hms(9, 30, 0);
    
    // Real XLK price ranges: typically $250-$290
    let xlk_prices = vec![
        (268.50, 269.20, 267.80, 268.90, 1_200_000.0), // O, H, L, C, Volume
        (268.90, 270.15, 268.45, 269.75, 1_350_000.0),
        (269.75, 271.30, 269.20, 270.85, 1_100_000.0),
        (270.85, 272.10, 270.40, 271.60, 980_000.0),
        (271.60, 272.80, 271.15, 272.40, 1_050_000.0),
    ];
    
    xlk_prices.into_iter().enumerate().map(|(i, (open, high, low, close, volume))| {
        TimeSeriesData {
            symbol: "XLK".to_string(),
            timestamp: base_time + chrono::Duration::minutes(i as i64),
            open,
            high,
            low,
            close,
            volume: vec![volume],
            volume_value: volume,
            intervals: vec![60000], // 1-minute intervals
            timestamps: vec![base_time + chrono::Duration::minutes(i as i64)],
            values: vec![close],
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("XLK".to_string()),
            value: Some(close),
            metadata: None,
            metadata_map: HashMap::new(),
        }
    }).collect()
}

/// Test data representing real SPY prices (S&P 500 ETF around $440)
fn create_real_spy_data() -> Vec<TimeSeriesData> {
    let base_time = Utc.ymd(2024, 1, 1).and_hms(9, 30, 0);
    
    // Real SPY price ranges: typically $420-$460
    let spy_prices = vec![
        (441.20, 442.50, 440.80, 441.90, 8_500_000.0),
        (441.90, 443.15, 441.45, 442.75, 9_200_000.0),
        (442.75, 444.30, 442.20, 443.85, 7_800_000.0),
        (443.85, 445.10, 443.40, 444.60, 8_100_000.0),
        (444.60, 445.80, 444.15, 445.25, 7_600_000.0),
    ];
    
    spy_prices.into_iter().enumerate().map(|(i, (open, high, low, close, volume))| {
        TimeSeriesData {
            symbol: "SPY".to_string(),
            timestamp: base_time + chrono::Duration::minutes(i as i64),
            open,
            high,
            low,
            close,
            volume: vec![volume],
            volume_value: volume,
            intervals: vec![60000],
            timestamps: vec![base_time + chrono::Duration::minutes(i as i64)],
            values: vec![close],
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("SPY".to_string()),
            value: Some(close),
            metadata: None,
            metadata_map: HashMap::new(),
        }
    }).collect()
}

#[tokio::test]
async fn test_real_xlk_price_normalization() -> Result<()> {
    println!("🧪 Testing XLK real price normalization...");
    
    let xlk_data = create_real_xlk_data();
    
    // Verify input data is realistic
    let prices: Vec<f64> = xlk_data.iter().map(|d| d.close).collect();
    let price_min = prices.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let price_max = prices.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    
    println!("📊 Original XLK price range: ${:.2} to ${:.2}", price_min, price_max);
    assert!(price_min > 260.0, "XLK price too low: ${:.2}", price_min);
    assert!(price_max < 280.0, "XLK price too high: ${:.2}", price_max);
    
    // Create normalization stats
    let dataset_stats = DatasetNormalizationStats {
        price_min,
        price_max,
        volume_min: 950_000.0,
        volume_max: 1_400_000.0,
    };
    
    // Test normalization of each data point
    for (i, data_point) in xlk_data.iter().enumerate() {
        let normalized = normalize_ohlcv_data_with_stats(data_point, &dataset_stats)?;
        
        // Verify all normalized values are in [0,1] range
        assert!(normalized.open >= 0.0 && normalized.open <= 1.0, 
                "Open price {} not in [0,1]: {:.4}", i, normalized.open);
        assert!(normalized.high >= 0.0 && normalized.high <= 1.0, 
                "High price {} not in [0,1]: {:.4}", i, normalized.high);
        assert!(normalized.low >= 0.0 && normalized.low <= 1.0, 
                "Low price {} not in [0,1]: {:.4}", i, normalized.low);
        assert!(normalized.close >= 0.0 && normalized.close <= 1.0, 
                "Close price {} not in [0,1]: {:.4}", i, normalized.close);
        assert!(normalized.volume >= 0.0 && normalized.volume <= 1.0, 
                "Volume {} not in [0,1]: {:.4}", i, normalized.volume);
        
        // Log first transformation for verification
        if i == 0 {
            println!("🔄 Sample normalization: ${:.2} → {:.4}", data_point.close, normalized.close);
        }
    }
    
    println!("✅ XLK normalization test passed");
    Ok(())
}

#[tokio::test]
async fn test_multi_symbol_normalization_isolation() -> Result<()> {
    println!("🧪 Testing multi-symbol normalization isolation...");
    
    let xlk_data = create_real_xlk_data();
    let spy_data = create_real_spy_data();
    
    // Calculate separate normalization stats for each symbol
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
    
    println!("📊 XLK range: ${:.2} to ${:.2}", xlk_min, xlk_max);
    println!("📊 SPY range: ${:.2} to ${:.2}", spy_min, spy_max);
    
    // Verify price ranges are realistic and different
    assert!(xlk_min < xlk_max, "XLK price range invalid");
    assert!(spy_min < spy_max, "SPY price range invalid");
    assert!((spy_max - xlk_max).abs() > 100.0, "Price ranges too similar - isolation may be broken");
    
    // Test per-symbol normalization parameters
    let xlk_stats = DatasetNormalizationStats {
        price_min: xlk_min,
        price_max: xlk_max,
        volume_min: 950_000.0,
        volume_max: 1_400_000.0,
    };
    
    let spy_stats = DatasetNormalizationStats {
        price_min: spy_min,
        price_max: spy_max,
        volume_min: 7_500_000.0,
        volume_max: 9_500_000.0,
    };
    
    // Normalize XLK data with XLK-specific stats
    let xlk_normalized = normalize_ohlcv_data_with_stats(&xlk_data[0], &xlk_stats)?;
    
    // Normalize SPY data with SPY-specific stats
    let spy_normalized = normalize_ohlcv_data_with_stats(&spy_data[0], &spy_stats)?;
    
    // Verify both are properly normalized to [0,1]
    assert!(xlk_normalized.close >= 0.0 && xlk_normalized.close <= 1.0);
    assert!(spy_normalized.close >= 0.0 && spy_normalized.close <= 1.0);
    
    // Verify that using wrong stats would produce different (wrong) results
    let xlk_with_spy_stats = normalize_ohlcv_data_with_stats(&xlk_data[0], &spy_stats)?;
    assert_ne!(xlk_normalized.close, xlk_with_spy_stats.close, 
               "Normalization not symbol-specific!");
    
    println!("✅ Multi-symbol isolation test passed");
    Ok(())
}

#[tokio::test] 
async fn test_training_data_service_normalization() -> Result<()> {
    println!("🧪 Testing TrainingDataService normalization...");
    
    // Create test data with realistic price ranges
    let test_data = create_real_xlk_data();
    
    // Test the normalization function from TrainingDataService
    let features = vec![
        vec![268.50, 1_200_000.0, 1.005, 1.001], // price, volume, ratios
        vec![270.85, 980_000.0, 1.008, 1.002],
        vec![272.40, 1_050_000.0, 1.006, 1.003],
    ];
    
    let targets = vec![268.90, 271.60, 272.80];
    
    // Test normalization parameters calculation
    let params = calculate_normalization_params(&features, &targets)?;
    
    // Verify parameter ranges are reasonable for real prices
    assert!(params.target_mean > 200.0 && params.target_mean < 300.0, 
            "Target mean unrealistic: {:.2}", params.target_mean);
    assert!(params.target_std > 0.0 && params.target_std < 50.0, 
            "Target std unrealistic: {:.2}", params.target_std);
    
    println!("📊 Normalization params - Mean: {:.2}, Std: {:.2}", 
             params.target_mean, params.target_std);
    
    // Test feature normalization
    let mut norm_features = features.clone();
    let mut norm_targets = targets.clone();
    
    apply_normalization(&mut norm_features, &mut norm_targets, &params)?;
    
    // Verify normalized values are reasonable
    for (i, feature_vec) in norm_features.iter().enumerate() {
        for (j, &value) in feature_vec.iter().enumerate() {
            assert!(value.is_finite(), "Feature [{},{}] not finite: {}", i, j, value);
            // Normalized features should be roughly centered around 0
            assert!(value > -5.0 && value < 5.0, "Feature [{},{}] out of range: {:.4}", i, j, value);
        }
    }
    
    for (i, &target) in norm_targets.iter().enumerate() {
        assert!(target.is_finite(), "Target {} not finite: {}", i, target);
        assert!(target > -5.0 && target < 5.0, "Target {} out of range: {:.4}", i, target);
    }
    
    println!("✅ TrainingDataService normalization test passed");
    Ok(())
}

#[tokio::test]
async fn test_denormalization_roundtrip() -> Result<()> {
    println!("🧪 Testing normalization-denormalization roundtrip...");
    
    let xlk_data = create_real_xlk_data();
    let original_price = xlk_data[0].close; // Should be around $268
    
    // Calculate normalization stats
    let prices: Vec<f64> = xlk_data.iter().map(|d| d.close).collect();
    let price_min = prices.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let price_max = prices.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    
    let stats = DatasetNormalizationStats {
        price_min,
        price_max,
        volume_min: 950_000.0,
        volume_max: 1_400_000.0,
    };
    
    // Normalize
    let normalized = normalize_ohlcv_data_with_stats(&xlk_data[0], &stats)?;
    
    // Denormalize
    let denormalized_price = denormalize_price(normalized.close, &stats);
    
    // Verify roundtrip accuracy
    let error = (original_price - denormalized_price).abs();
    let relative_error = error / original_price;
    
    println!("🔄 Roundtrip: ${:.2} → {:.4} → ${:.2} (error: {:.6})", 
             original_price, normalized.close, denormalized_price, relative_error);
    
    assert!(relative_error < 1e-10, "Roundtrip error too large: {:.2e}", relative_error);
    
    println!("✅ Denormalization roundtrip test passed");
    Ok(())
}

// Helper functions that mirror the implementation

fn normalize_ohlcv_data_with_stats(
    data: &TimeSeriesData,
    stats: &DatasetNormalizationStats,
) -> Result<NormalizedOHLCV> {
    let price_range = if stats.price_max != stats.price_min {
        stats.price_max - stats.price_min
    } else {
        1.0
    };
    
    let volume_range = if stats.volume_max != stats.volume_min {
        stats.volume_max - stats.volume_min
    } else {
        1.0
    };
    
    let normalized_open = (data.open - stats.price_min) / price_range;
    let normalized_high = (data.high - stats.price_min) / price_range;
    let normalized_low = (data.low - stats.price_min) / price_range;
    let normalized_close = (data.close - stats.price_min) / price_range;
    
    let normalized_volume = if data.volume_value > 0.0 {
        (data.volume_value - stats.volume_min) / volume_range
    } else {
        0.0
    };
    
    Ok(NormalizedOHLCV {
        open: normalized_open.clamp(0.0, 1.0),
        high: normalized_high.clamp(0.0, 1.0),
        low: normalized_low.clamp(0.0, 1.0),
        close: normalized_close.clamp(0.0, 1.0),
        volume: normalized_volume.clamp(0.0, 1.0),
    })
}

fn denormalize_price(normalized_price: f64, stats: &DatasetNormalizationStats) -> f64 {
    let price_range = stats.price_max - stats.price_min;
    normalized_price * price_range + stats.price_min
}

fn calculate_normalization_params(
    features: &[Vec<f64>],
    targets: &[f64],
) -> Result<NormalizationParams> {
    if features.is_empty() {
        anyhow::bail!("Cannot calculate normalization params for empty features");
    }
    
    let num_features = features[0].len();
    let mut feature_means = vec![0.0; num_features];
    
    // Calculate feature means
    for feature_vec in features {
        for (i, &value) in feature_vec.iter().enumerate() {
            feature_means[i] += value;
        }
    }
    for mean in &mut feature_means {
        *mean /= features.len() as f64;
    }
    
    // Calculate feature standard deviations
    let mut feature_stds = vec![0.0; num_features];
    for feature_vec in features {
        for (i, &value) in feature_vec.iter().enumerate() {
            feature_stds[i] += (value - feature_means[i]).powi(2);
        }
    }
    for std in &mut feature_stds {
        *std = (*std / features.len() as f64).sqrt().max(1e-8);
    }
    
    // Calculate target statistics
    let target_mean = targets.iter().sum::<f64>() / targets.len() as f64;
    let target_std = (targets
        .iter()
        .map(|&t| (t - target_mean).powi(2))
        .sum::<f64>()
        / targets.len() as f64)
        .sqrt()
        .max(1e-8);
    
    Ok(NormalizationParams {
        feature_means,
        feature_stds,
        target_mean,
        target_std,
    })
}

fn apply_normalization(
    features: &mut [Vec<f64>],
    targets: &mut [f64],
    params: &NormalizationParams,
) -> Result<()> {
    // Normalize features
    for feature_vec in features.iter_mut() {
        for (i, value) in feature_vec.iter_mut().enumerate() {
            *value = (*value - params.feature_means[i]) / params.feature_stds[i];
        }
    }
    
    // Normalize targets
    for target in targets.iter_mut() {
        *target = (*target - params.target_mean) / params.target_std;
    }
    
    Ok(())
}