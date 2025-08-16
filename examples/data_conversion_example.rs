//! Data conversion example demonstrating the complete conversion pipeline
//!
//! This example shows how to convert TimeSeriesData to vendor formats,
//! handle predictions, and manage type conversions safely.

use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;

// Import neural-trader types
use autonomous_platform::adapters::neural::{
    BatchConverter, SafeF32Convert, StreamingConverter, VendorDataConverter, VendorFormatConverter,
};
use autonomous_platform::data::TimeSeriesData;

fn main() -> Result<()> {
    println!("🔄 Neural Trader Data Conversion Example");
    println!("==========================================");

    // Create sample data
    let sample_data = create_sample_data()?;
    println!("✅ Created {} sample data points", sample_data.len());

    // Example 1: Basic f64 to f32 conversion
    demonstrate_type_conversion()?;

    // Example 2: Convert to vendor format
    demonstrate_vendor_conversion(&sample_data)?;

    // Example 3: Batch conversion
    demonstrate_batch_conversion()?;

    // Example 4: Streaming conversion for large datasets
    demonstrate_streaming_conversion()?;

    // Example 5: Prediction result conversion
    demonstrate_prediction_conversion(&sample_data)?;

    println!("\n🎉 All conversion examples completed successfully!");

    Ok(())
}

fn create_sample_data() -> Result<Vec<TimeSeriesData>> {
    let mut data = Vec::new();
    let base_time = Utc::now();

    for i in 0..24 {
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 50.0 + (i as f64 * 2.0));
        indicators.insert("macd".to_string(), 0.001 * i as f64);
        indicators.insert("ema_20".to_string(), 51000.0 + i as f64 * 50.0);

        let point = TimeSeriesData {
            symbol: "BTC/USD".to_string(),
            timestamp: base_time + chrono::Duration::hours(i),
            open: 50000.0 + i as f64 * 100.0,
            high: 51000.0 + i as f64 * 120.0,
            low: 49500.0 + i as f64 * 80.0,
            close: 50500.0 + i as f64 * 110.0,
            volume: 1000.0 + i as f64 * 50.0,
            indicators,
            source: Some("example".to_string()),
            entity: Some("BTC/USD".to_string()),
            value: None,
            metadata: None,
        };

        data.push(point);
    }

    Ok(data)
}

fn demonstrate_type_conversion() -> Result<()> {
    println!("\n🔢 Type Conversion Examples:");

    // Safe f64 to f32 conversion
    let large_value = 123456789.123456789_f64;
    let converted = large_value.to_f32_safe()?;
    println!("  Large value: {} -> {}", large_value, converted);

    // Handle edge cases
    let extreme_values = vec![
        f64::MAX,
        f64::MIN,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ];

    for &value in &extreme_values {
        match value.to_f32_safe() {
            Ok(converted) => {
                println!(
                    "  Edge case: {} -> {}",
                    if value.is_nan() {
                        "NaN".to_string()
                    } else if value.is_infinite() {
                        if value.is_sign_positive() {
                            "∞".to_string()
                        } else {
                            "-∞".to_string()
                        }
                    } else {
                        value.to_string()
                    },
                    if converted.is_nan() {
                        "NaN".to_string()
                    } else if converted.is_infinite() {
                        if converted.is_sign_positive() {
                            "∞".to_string()
                        } else {
                            "-∞".to_string()
                        }
                    } else {
                        converted.to_string()
                    }
                );
            }
            Err(e) => println!("  Conversion error: {}", e),
        }
    }

    Ok(())
}

fn demonstrate_vendor_conversion(data: &[TimeSeriesData]) -> Result<()> {
    println!("\n🏭 Vendor Format Conversion:");

    let converter = VendorFormatConverter::new();

    // Convert to neuro-divergent format
    let vendor_data = converter.to_neuro_divergent_f32(data, "BTC/USD")?;

    println!("  ✅ Converted to neuro-divergent format:");
    println!("     Series ID: {}", vendor_data.series_id);
    println!("     Data points: {}", vendor_data.len());
    println!("     First value: {}", vendor_data.data_points[0].value);
    println!(
        "     Has exogenous: {}",
        vendor_data.data_points[0].exogenous.is_some()
    );

    if let Some(ref exog) = vendor_data.data_points[0].exogenous {
        println!("     Exogenous features: {}", exog.len());
    }

    // Validate conversion integrity
    converter.validate_conversion(data, &vendor_data)?;
    println!("  ✅ Conversion validation passed");

    Ok(())
}

fn demonstrate_batch_conversion() -> Result<()> {
    println!("\n📦 Batch Conversion:");

    let mut data_batch = HashMap::new();

    // Create data for multiple symbols
    let symbols = vec!["BTC/USD", "ETH/USD", "ADA/USD"];
    for symbol in &symbols {
        let mut symbol_data = Vec::new();
        let base_time = Utc::now();

        for i in 0..12 {
            let mut indicators = HashMap::new();
            indicators.insert("rsi".to_string(), 45.0 + i as f64 * 3.0);

            let point = TimeSeriesData {
                symbol: symbol.to_string(),
                timestamp: base_time + chrono::Duration::hours(i),
                open: 1000.0 + i as f64 * 10.0,
                high: 1010.0 + i as f64 * 12.0,
                low: 990.0 + i as f64 * 8.0,
                close: 1005.0 + i as f64 * 11.0,
                volume: 500.0 + i as f64 * 25.0,
                indicators,
                source: Some("batch_example".to_string()),
                entity: Some(symbol.to_string()),
                value: None,
                metadata: None,
            };

            symbol_data.push(point);
        }

        data_batch.insert(symbol.to_string(), symbol_data);
    }

    // Convert batch
    let converter = VendorFormatConverter::new();
    let converted_batch = converter.convert_batch(&data_batch)?;

    println!("  ✅ Batch conversion completed:");
    for (symbol, vendor_data) in &converted_batch {
        println!("     {}: {} data points", symbol, vendor_data.len());
    }

    // Verify batch conversion
    BatchConverter::verify_conversions(&data_batch, &converted_batch)?;
    println!("  ✅ Batch verification passed");

    Ok(())
}

fn demonstrate_streaming_conversion() -> Result<()> {
    println!("\n🌊 Streaming Conversion:");

    // Create a large dataset iterator
    let large_dataset = create_large_dataset_iterator(1000);

    let streaming_converter = StreamingConverter::new(100); // 100 item chunks
    let converter = VendorFormatConverter::new();

    let result = converter.convert_streaming(large_dataset, "LARGE/DATASET", 100)?;

    println!("  ✅ Streaming conversion completed:");
    println!("     Series ID: {}", result.series_id);
    println!("     Total data points: {}", result.len());
    println!("     Memory efficient: Uses chunked processing");

    Ok(())
}

fn demonstrate_prediction_conversion(base_data: &[TimeSeriesData]) -> Result<()> {
    println!("\n🔮 Prediction Result Conversion:");

    // Simulate model predictions
    let predictions = vec![
        52000.0_f32,
        52500.0_f32,
        53000.0_f32,
        53200.0_f32,
        53800.0_f32,
        54100.0_f32,
    ];

    let converter = VendorFormatConverter::new();
    let prediction_results = converter.from_vendor_predictions_f32(
        &predictions,
        &base_data[0],
        6, // forecast horizon
    )?;

    println!("  ✅ Prediction conversion completed:");
    println!("     Forecast steps: {}", prediction_results.len());

    for (i, pred) in prediction_results.iter().enumerate() {
        println!(
            "     Step {}: ${:.2} at {}",
            i + 1,
            pred.close,
            pred.timestamp.format("%Y-%m-%d %H:%M")
        );

        // Show metadata
        if let Some(ref metadata) = pred.metadata {
            println!("       Forecast step: {}", metadata["forecast_step"]);
        }
    }

    Ok(())
}

fn create_large_dataset_iterator(size: usize) -> impl Iterator<Item = TimeSeriesData> {
    let base_time = Utc::now();

    (0..size).map(move |i| {
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 30.0 + (i % 70) as f64);
        indicators.insert("macd".to_string(), -0.001 + (i as f64 * 0.0001));

        TimeSeriesData {
            symbol: "STREAM/DATA".to_string(),
            timestamp: base_time + chrono::Duration::minutes(i as i64),
            open: 1000.0 + (i as f64 * 0.5),
            high: 1005.0 + (i as f64 * 0.6),
            low: 995.0 + (i as f64 * 0.4),
            close: 1002.0 + (i as f64 * 0.55),
            volume: 100.0 + (i as f64 * 2.0),
            indicators,
            source: Some("streaming".to_string()),
            entity: Some("STREAM/DATA".to_string()),
            value: None,
            metadata: None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example_conversion_pipeline() {
        let result = std::panic::catch_unwind(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                main().unwrap();
            });
        });

        assert!(
            result.is_ok(),
            "Conversion pipeline example should run without panics"
        );
    }
}
