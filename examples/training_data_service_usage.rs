//! Example usage of the TrainingDataService
//!
//! This example demonstrates how to use the TrainingDataService to prepare
//! training data for different neural network model types.

use autonomous_platform::config::{RedisConfig};
use autonomous_platform::data::{RedisCache, TimescaleDBStorage};
use autonomous_platform::integration::{TrainingDataConfig, TrainingDataService};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🧠 TrainingDataService Usage Example");
    println!("=====================================");

    // Configuration
    let data_config = DataConfig {
        timescale_url: std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://localhost/neural_trader".to_string()),
        retention_days: 90,
        aggregate_intervals: vec!["1 hour".to_string(), "1 day".to_string()],
        enable_data_validation: true,
        enable_data_monitoring: true,
    };

    let redis_config = RedisConfig {
        url: std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string()),
        pool_size: 4,
        cache_ttl: 300,
        key_prefix: "neural_trader".to_string(),
    };

    // Initialize storage and cache
    println!("📦 Initializing storage and cache...");
    let storage = Arc::new(TimescaleDBStorage::new(&data_config).await?);
    let cache = Arc::new(RedisCache::new(&redis_config).await?);

    // Create TrainingDataService
    let service = TrainingDataService::new(storage, cache).await?;
    println!("✅ TrainingDataService initialized successfully");

    // Example 1: Load MLP training data
    println!("\n📊 Example 1: Loading MLP Training Data");
    println!("----------------------------------------");

    let mlp_config = TrainingDataConfig {
        batch_size: 64,
        sequence_length: 30, // Not used for MLP
        feature_window: 20,
        normalize: true,
        include_volume: true,
        include_indicators: true,
        cache_enabled: true,
        cache_ttl_seconds: 1800, // 30 minutes
    };

    match service
        .load_training_batch(ModelType::MLP, "BTC/USD", mlp_config.clone())
        .await
    {
        Ok(data) => {
            println!("✅ MLP data loaded successfully:");
            println!("   - Symbol: {}", data.symbol);
            println!("   - Samples: {}", data.features.len());
            println!(
                "   - Features per sample: {}",
                data.features.get(0).map(|f| f.len()).unwrap_or(0)
            );
            println!("   - Feature names: {:?}", data.feature_names);
            println!("   - Normalized: {}", data.normalization_params.is_some());
        }
        Err(e) => println!("❌ Failed to load MLP data: {}", e),
    }

    // Example 2: Load LSTM training data
    println!("\n📈 Example 2: Loading LSTM Training Data");
    println!("----------------------------------------");

    let lstm_config = TrainingDataConfig {
        batch_size: 32,
        sequence_length: 50, // Important for LSTM
        feature_window: 15,
        normalize: true,
        include_volume: true,
        include_indicators: false, // Simpler features for LSTM
        cache_enabled: true,
        cache_ttl_seconds: 1800,
    };

    match service
        .load_training_batch(ModelType::LSTM, "ETH/USD", lstm_config)
        .await
    {
        Ok(data) => {
            println!("✅ LSTM data loaded successfully:");
            println!("   - Symbol: {}", data.symbol);
            println!("   - Sequences: {}", data.features.len());
            println!(
                "   - Sequence length: {}",
                data.metadata
                    .get("sequence_length")
                    .unwrap_or(&serde_json::json!(0))
            );
            println!(
                "   - Features per timestep: {}",
                data.metadata
                    .get("features_per_step")
                    .unwrap_or(&serde_json::json!(0))
            );

            // LSTM data is flattened - calculate actual shape
            if let Some(first_seq) = data.features.get(0) {
                let seq_len = data
                    .metadata
                    .get("sequence_length")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(50) as usize;
                let features_per_step = first_seq.len() / seq_len;
                println!(
                    "   - Actual shape: {} sequences × {} timesteps × {} features",
                    data.features.len(),
                    seq_len,
                    features_per_step
                );
            }
        }
        Err(e) => println!("❌ Failed to load LSTM data: {}", e),
    }

    // Example 3: Prepare online data for real-time predictions
    println!("\n⚡ Example 3: Preparing Online Data");
    println!("-----------------------------------");

    match service.prepare_online_data("BTC/USD", 30).await {
        Ok(data) => {
            println!("✅ Online data prepared successfully:");
            println!("   - Symbol: {}", data.symbol);
            println!("   - Timestamp: {}", data.timestamp);
            println!("   - Close price: ${:.2}", data.close);
            println!("   - Volume: {:.0}", data.volume);
            println!("   - Indicators calculated: {}", data.indicators.len());

            // Display some indicators
            for (name, value) in data.indicators.iter().take(5) {
                println!("     - {}: {:.4}", name, value);
            }
        }
        Err(e) => println!("❌ Failed to prepare online data: {}", e),
    }

    // Example 4: Data validation demonstration
    println!("\n🔍 Example 4: Data Validation");
    println!("------------------------------");

    // Create sample data for validation
    use chrono::{Duration, Utc};
    use autonomous_platform::data::TimeSeriesData;
    use std::collections::HashMap;

    let mut test_data = Vec::new();
    let base_time = Utc::now() - Duration::hours(200);

    // Create valid data
    for i in 0..150 {
        test_data.push(TimeSeriesData {
            symbol: "TEST/USD".to_string(),
            timestamp: base_time + Duration::hours(i),
            open: 100.0 + (i as f64 * 0.1),
            high: 101.0 + (i as f64 * 0.1),
            low: 99.0 + (i as f64 * 0.1),
            close: 100.5 + (i as f64 * 0.1),
            volume: 1000.0 + (i as f64 * 10.0),
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("TEST/USD".to_string()),
            value: Some(100.5),
            metadata: None,
        });
    }

    match service.validate_training_data(&test_data) {
        Ok(()) => println!(
            "✅ Data validation passed: {} data points are valid",
            test_data.len()
        ),
        Err(e) => println!("❌ Data validation failed: {}", e),
    }

    // Test with insufficient data
    let small_data = test_data.into_iter().take(50).collect::<Vec<_>>();
    match service.validate_training_data(&small_data) {
        Ok(()) => println!("✅ Small data validation passed unexpectedly"),
        Err(e) => println!("✅ Expected validation failure: {}", e),
    }

    // Example 5: Service metrics
    println!("\n📊 Example 5: Service Metrics");
    println!("------------------------------");

    let metrics = service.get_metrics().await;
    println!("Service Performance Metrics:");
    println!(
        "   - Total batches loaded: {}",
        metrics.total_batches_loaded
    );
    println!("   - Cache hits: {}", metrics.cache_hits);
    println!("   - Cache misses: {}", metrics.cache_misses);
    println!(
        "   - Total data points processed: {}",
        metrics.total_data_points
    );
    println!(
        "   - Average preparation time: {:.2}ms",
        metrics.average_preparation_time_ms
    );

    if metrics.cache_hits + metrics.cache_misses > 0 {
        let cache_hit_rate =
            metrics.cache_hits as f64 / (metrics.cache_hits + metrics.cache_misses) as f64 * 100.0;
        println!("   - Cache hit rate: {:.1}%", cache_hit_rate);
    }

    // Example 6: Different model type configurations
    println!("\n🎛️  Example 6: Model Type Configurations");
    println!("------------------------------------------");

    let model_configs = vec![
        (
            "MLP",
            ModelType::MLP,
            TrainingDataConfig {
                batch_size: 128,
                sequence_length: 1, // Not relevant for MLP
                feature_window: 25, // Larger window for more features
                normalize: true,
                include_volume: true,
                include_indicators: true,
                cache_enabled: true,
                cache_ttl_seconds: 3600,
            },
        ),
        (
            "LSTM",
            ModelType::LSTM,
            TrainingDataConfig {
                batch_size: 32,
                sequence_length: 60, // Longer sequences for better patterns
                feature_window: 10,  // Smaller window, rely on sequence
                normalize: true,
                include_volume: true,
                include_indicators: false,
                cache_enabled: true,
                cache_ttl_seconds: 1800,
            },
        ),
        (
            "GRU",
            ModelType::GRU,
            TrainingDataConfig {
                batch_size: 48,
                sequence_length: 40, // Medium sequence length
                feature_window: 15,
                normalize: true,
                include_volume: true,
                include_indicators: true,
                cache_enabled: true,
                cache_ttl_seconds: 2400,
            },
        ),
    ];

    for (name, model_type, config) in model_configs {
        println!("\n{} Configuration:", name);
        println!("   - Batch size: {}", config.batch_size);
        println!("   - Sequence length: {}", config.sequence_length);
        println!("   - Feature window: {}", config.feature_window);
        println!("   - Include volume: {}", config.include_volume);
        println!("   - Include indicators: {}", config.include_indicators);
        println!("   - Cache TTL: {}s", config.cache_ttl_seconds);

        // Note: In a real application, you would load data for each model type
        // For this example, we just show the configuration
    }

    println!("\n🎉 TrainingDataService example completed!");
    println!("\nKey Features Demonstrated:");
    println!("   ✓ Multi-model support (MLP, LSTM, GRU, CNN, Ensemble)");
    println!("   ✓ Efficient data loading and caching");
    println!("   ✓ Automatic feature engineering and normalization");
    println!("   ✓ Real-time data preparation");
    println!("   ✓ Data validation and quality checks");
    println!("   ✓ Performance monitoring and metrics");
    println!("   ✓ Flexible configuration for different use cases");

    Ok(())
}

// Helper function to format numbers
fn format_number(num: f64) -> String {
    if num >= 1_000_000.0 {
        format!("{:.1}M", num / 1_000_000.0)
    } else if num >= 1_000.0 {
        format!("{:.1}K", num / 1_000.0)
    } else {
        format!("{:.1}", num)
    }
}
