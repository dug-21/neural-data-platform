//! Basic Usage Example
//!
//! This example demonstrates the fundamental usage patterns of the Neural Trader
//! Autonomous Platform, including configuration loading, data pipeline initialization,
//! and basic operations.

use autonomous_platform::{
    PlatformConfig, load_default_config, Result,
    data::{TimeSeriesData, QualityMetrics, PlatformMetrics},
    adapters::{ModelRegistry, ModelAdapter},
};
use chrono::Utc;
use std::collections::HashMap;
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting Basic Usage Example");

    // Step 1: Load Configuration
    let config = load_default_config().map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;
    
    info!("✓ Configuration loaded successfully");
    info!("  Platform: {} v{}", config.platform.name, config.platform.version);
    info!("  Database: {}", mask_connection_string(&config.database.url));
    info!("  Neural models: {:?}", config.neural.models);

    // Step 2: Create Sample Time Series Data
    let sample_data = create_sample_data();
    info!("✓ Created {} sample data points", sample_data.len());

    // Step 3: Initialize Model Registry
    let mut model_registry = ModelRegistry::new();
    info!("✓ Model registry initialized");

    // Step 4: Demonstrate Data Validation
    for data_point in &sample_data {
        match data_point.validate() {
            Ok(_) => info!("✓ Data point for {} is valid", data_point.symbol),
            Err(e) => warn!("✗ Data validation failed for {}: {}", data_point.symbol, e),
        }
    }

    // Step 5: Create Quality Metrics
    let quality_metrics = QualityMetrics::new(0.95, 150.0, 0.02);
    info!("✓ Quality metrics created:");
    info!("  Data completeness: {:.2}%", quality_metrics.data_completeness * 100.0);
    info!("  Latency: {:.0}ms", quality_metrics.latency_ms);
    info!("  Error rate: {:.2}%", quality_metrics.error_rate * 100.0);
    info!("  Overall quality: {:.2}%", quality_metrics.overall_quality * 100.0);

    // Step 6: Create Platform Metrics
    let platform_metrics = PlatformMetrics::new(
        1_000_000,  // total_records
        0.85,       // cache_hit_rate
        5000.0,     // processing_throughput
        2.5,        // storage_usage_gb
        15,         // active_connections
    );
    info!("✓ Platform metrics created:");
    info!("  Total records: {}", platform_metrics.total_records);
    info!("  Cache hit rate: {:.1}%", platform_metrics.cache_hit_rate * 100.0);
    info!("  Processing throughput: {:.0} records/sec", platform_metrics.processing_throughput);
    info!("  Storage usage: {:.1} GB", platform_metrics.storage_usage_gb);
    info!("  Active connections: {}", platform_metrics.active_connections);

    // Step 7: Demonstrate Error Handling
    info!("✓ Demonstrating error handling patterns");
    
    // Example of handling configuration errors
    match validate_configuration(&config) {
        Ok(_) => info!("  Configuration validation passed"),
        Err(e) => warn!("  Configuration validation failed: {}", e),
    }

    // Step 8: Simulate Processing Pipeline
    info!("✓ Simulating data processing pipeline");
    for (i, data_point) in sample_data.iter().enumerate() {
        if i % 100 == 0 {
            info!("  Processed {} data points", i + 1);
        }
        
        // Simulate processing delay
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
    }

    info!("✓ Basic usage example completed successfully");
    info!("Next steps:");
    info!("  1. Check out examples/trading_scenario.rs for trading workflows");
    info!("  2. See examples/performance_monitoring.rs for monitoring setup");
    info!("  3. Read docs/ARCHITECTURE.md for system design details");

    Ok(())
}

/// Create sample time series data for demonstration
fn create_sample_data() -> Vec<TimeSeriesData> {
    let symbols = vec!["BTCUSD", "ETHUSD", "ADAUSD"];
    let mut data = Vec::new();
    
    for symbol in symbols {
        for i in 0..10 {
            let base_price = match symbol {
                "BTCUSD" => 45000.0,
                "ETHUSD" => 3000.0,
                "ADAUSD" => 1.5,
                _ => 100.0,
            };
            
            let price_variation = (i as f64 * 0.01) - 0.05; // -5% to +5%
            let open = base_price * (1.0 + price_variation);
            let close = open * (1.0 + (i as f64 * 0.001)); // Small trend
            let high = open.max(close) * 1.02;
            let low = open.min(close) * 0.98;
            let volume = 1000.0 + (i as f64 * 100.0);
            
            let mut indicators = HashMap::new();
            indicators.insert("sma20".to_string(), (open + close) / 2.0);
            indicators.insert("rsi".to_string(), 50.0 + (i as f64 * 2.0));
            indicators.insert("volume_ma".to_string(), volume * 0.9);
            
            data.push(TimeSeriesData {
                symbol: symbol.to_string(),
                timestamp: Utc::now() - chrono::Duration::minutes(i as i64 * 5),
                open,
                high,
                low,
                close,
                volume,
                indicators,
            });
        }
    }
    
    data
}

/// Validate configuration settings
fn validate_configuration(config: &PlatformConfig) -> Result<()> {
    // Check neural model configuration
    if config.neural.models.is_empty() {
        anyhow::bail!("No neural models configured");
    }
    
    // Check memory allocation
    if config.neural.memory_gb < 0.5 {
        anyhow::bail!("Insufficient memory allocated for neural models");
    }
    
    // Check monitoring configuration
    if config.monitoring.quality_threshold < 0.8 {
        warn!("Quality threshold is quite low: {:.2}", config.monitoring.quality_threshold);
    }
    
    Ok(())
}

/// Mask sensitive parts of connection strings for logging
fn mask_connection_string(url: &str) -> String {
    if let Some(at_pos) = url.find('@') {
        if let Some(colon_pos) = url[..at_pos].rfind(':') {
            let mut masked = url.to_string();
            let password_start = colon_pos + 1;
            let password_end = at_pos;
            masked.replace_range(password_start..password_end, "***");
            return masked;
        }
    }
    url.to_string()
}