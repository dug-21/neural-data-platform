//! Example usage of the neural adapter infrastructure

use chrono::Utc;
use neural_trader::adapters::neural::{
    ConversionFormat, DataConverter, NeuralModelConfig, NeuroDivergentAdapter,
};
use neural_trader::adapters::DataAdapter;
use neural_trader::data::TimeSeriesData;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create configuration for the neural model
    let config = NeuralModelConfig {
        model_type: "TimeMixer".to_string(),
        lookback_window: 48,
        forecast_horizon: 12,
        batch_size: 64,
        use_gpu: false,
        model_params: serde_json::json!({
            "n_layers": 4,
            "d_model": 256,
            "dropout": 0.1,
        }),
    };

    // Initialize the adapter
    let mut adapter = NeuroDivergentAdapter::new(config);

    // Connect to the model
    println!("Connecting to neural model...");
    adapter.connect().await?;
    println!("Connected: {}", adapter.is_connected());

    // Create sample data
    let data = create_sample_data(100);

    // Make predictions
    println!("\nMaking predictions...");
    match adapter.predict(&data).await {
        Ok(predictions) => {
            println!("Received {} predictions", predictions.len());
            for (i, pred) in predictions.iter().enumerate() {
                println!("  t+{}: {:.2}", i + 1, pred);
            }
        }
        Err(e) => {
            println!("Prediction failed: {}", e);
        }
    }

    // Demonstrate data conversion
    println!("\nDemonstrating data conversion...");
    let converter = DataConverter::new();

    // Convert to different formats
    let formats = vec![
        ("DataFrame", ConversionFormat::DataFrame),
        ("NdArray", ConversionFormat::NdArray),
        ("Tensor", ConversionFormat::Tensor),
        ("DictArray", ConversionFormat::DictArray),
    ];

    for (name, _format) in formats {
        println!("  Converting to {} format...", name);
        // The actual conversion would happen inside the adapter
    }

    // Disconnect
    adapter.disconnect().await?;
    println!("\nDisconnected from neural model");

    Ok(())
}

fn create_sample_data(points: usize) -> Vec<TimeSeriesData> {
    let mut data = Vec::new();
    let base_time = Utc::now();

    for i in 0..points {
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 50.0 + (i as f64 * 0.5).sin() * 20.0);
        indicators.insert("macd".to_string(), 0.001 * (i as f64 * 0.1).cos());
        indicators.insert("volume_ma".to_string(), 1000.0 + (i as f64 * 10.0));

        data.push(TimeSeriesData {
            symbol: "BTC/USD".to_string(),
            timestamp: base_time + chrono::Duration::minutes(i as i64 * 5),
            open: 50000.0 + (i as f64 * 0.1).sin() * 1000.0,
            high: 50500.0 + (i as f64 * 0.1).sin() * 1000.0,
            low: 49500.0 + (i as f64 * 0.1).sin() * 1000.0,
            close: 50000.0 + (i as f64 * 0.1).sin() * 1000.0,
            volume: 1000.0 + (i as f64 * 0.05).sin() * 500.0,
            indicators,
            source: Some("example".to_string()),
            entity: Some("crypto".to_string()),
            value: None,
            metadata: None,
        });
    }

    data
}
