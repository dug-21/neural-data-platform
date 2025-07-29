//! Performance Regression Tests for ruv-FANN Integration
//!
//! Ensures performance optimizations maintain acceptable latency and throughput

use anyhow::Result;
use std::sync::Arc;
use tokio::time::{Duration, Instant};

use crate::config::NeuralConfig;
use crate::data::TimeSeriesData;
use crate::neural::{FannPredictor, OptimizedFannPredictor};

/// Maximum acceptable latency for single predictions (ms)
const MAX_SINGLE_PREDICTION_LATENCY_MS: u64 = 50;

/// Minimum required throughput for batch predictions (predictions/sec)
const MIN_BATCH_THROUGHPUT: f64 = 1000.0;

/// Maximum memory growth allowed during sustained operation (MB)
const MAX_MEMORY_GROWTH_MB: f64 = 100.0;

/// Generate test data
fn generate_test_data(size: usize) -> Vec<TimeSeriesData> {
    (0..size)
        .map(|i| {
            let base_price = 100.0 + (i as f64 * 0.1).sin() * 10.0;
            TimeSeriesData {
                timestamp: chrono::Utc::now() - chrono::Duration::hours(i as i64),
                entity: Some("TEST".to_string()),
                symbol: "TEST".to_string(),
                open: base_price - 0.5,
                high: base_price + 1.0,
                low: base_price - 1.0,
                close: base_price,
                volume: 1_000_000.0,
                source: Some("test".to_string()),
                value: Some(base_price),
                metadata: None,
                indicators: std::collections::HashMap::from([
                    ("rsi".to_string(), 50.0),
                    ("macd".to_string(), 0.0),
                ]),
            }
        })
        .collect()
}

#[tokio::test]
async fn test_single_prediction_latency() -> Result<()> {
    let config = NeuralConfig::default();
    let base_predictor = Arc::new(FannPredictor::new(config)?);
    let optimized = OptimizedFannPredictor::new(base_predictor).await?;

    let data = generate_test_data(100);

    // Warmup
    for _ in 0..10 {
        let _ = optimized.predict_batch("MLP", vec![&data], 5).await?;
    }

    // Measure latency
    let mut latencies = Vec::new();
    for _ in 0..100 {
        let start = Instant::now();
        let _ = optimized.predict_batch("MLP", vec![&data], 5).await?;
        let latency = start.elapsed().as_millis() as u64;
        latencies.push(latency);
    }

    // Calculate statistics
    latencies.sort();
    let p50 = latencies[latencies.len() / 2];
    let p95 = latencies[latencies.len() * 95 / 100];
    let p99 = latencies[latencies.len() * 99 / 100];

    println!(
        "Single prediction latencies - P50: {}ms, P95: {}ms, P99: {}ms",
        p50, p95, p99
    );

    // Assert performance requirements
    assert!(
        p50 <= MAX_SINGLE_PREDICTION_LATENCY_MS,
        "P50 latency {}ms exceeds max {}ms",
        p50,
        MAX_SINGLE_PREDICTION_LATENCY_MS
    );
    assert!(
        p95 <= MAX_SINGLE_PREDICTION_LATENCY_MS * 2,
        "P95 latency {}ms exceeds 2x max",
        p95
    );

    Ok(())
}

#[tokio::test]
async fn test_batch_throughput() -> Result<()> {
    let config = NeuralConfig::default();
    let base_predictor = Arc::new(FannPredictor::new(config)?);
    let optimized = OptimizedFannPredictor::new(base_predictor).await?;

    let data = generate_test_data(100);
    let batch_size = 64;
    let batch: Vec<_> = (0..batch_size).map(|_| data.as_slice()).collect();

    // Warmup
    for _ in 0..5 {
        let _ = optimized.predict_batch("MLP", batch.clone(), 5).await?;
    }

    // Measure throughput
    let start = Instant::now();
    let iterations = 50;

    for _ in 0..iterations {
        let _ = optimized.predict_batch("MLP", batch.clone(), 5).await?;
    }

    let elapsed_secs = start.elapsed().as_secs_f64();
    let total_predictions = (batch_size * iterations) as f64;
    let throughput = total_predictions / elapsed_secs;

    println!("Batch throughput: {:.2} predictions/sec", throughput);

    assert!(
        throughput >= MIN_BATCH_THROUGHPUT,
        "Throughput {:.2} below minimum {:.2}",
        throughput,
        MIN_BATCH_THROUGHPUT
    );

    Ok(())
}

#[tokio::test]
async fn test_memory_efficiency() -> Result<()> {
    let config = NeuralConfig::default();
    let base_predictor = Arc::new(FannPredictor::new(config)?);
    let optimized = OptimizedFannPredictor::new(base_predictor).await?;

    let data = generate_test_data(1000);

    // Get initial memory usage
    let initial_memory = get_process_memory_mb();

    // Run many predictions
    for _ in 0..100 {
        let batch: Vec<_> = (0..32).map(|_| data.as_slice()).collect();
        let _ = optimized.predict_batch("MLP", batch, 5).await?;
    }

    // Force garbage collection by dropping and recreating
    drop(optimized);
    tokio::time::sleep(Duration::from_millis(100)).await;

    let final_memory = get_process_memory_mb();
    let memory_growth = final_memory - initial_memory;

    println!("Memory growth: {:.2} MB", memory_growth);

    assert!(
        memory_growth <= MAX_MEMORY_GROWTH_MB,
        "Memory growth {:.2}MB exceeds max {:.2}MB",
        memory_growth,
        MAX_MEMORY_GROWTH_MB
    );

    Ok(())
}

#[tokio::test]
async fn test_parallel_scaling() -> Result<()> {
    let config = NeuralConfig::default();
    let base_predictor = Arc::new(FannPredictor::new(config)?);
    let optimized = OptimizedFannPredictor::new(base_predictor).await?;

    let data = generate_test_data(100);

    // Test scaling with different batch sizes
    let batch_sizes = vec![1, 4, 16, 64];
    let mut efficiencies = Vec::new();

    for &batch_size in &batch_sizes {
        let batch: Vec<_> = (0..batch_size).map(|_| data.as_slice()).collect();

        // Warmup
        for _ in 0..5 {
            let _ = optimized.predict_batch("MLP", batch.clone(), 5).await?;
        }

        // Measure
        let start = Instant::now();
        let iterations = 20;

        for _ in 0..iterations {
            let _ = optimized.predict_batch("MLP", batch.clone(), 5).await?;
        }

        let elapsed = start.elapsed().as_secs_f64();
        let predictions_per_sec = (batch_size * iterations) as f64 / elapsed;
        let efficiency = predictions_per_sec / batch_size as f64;

        efficiencies.push(efficiency);
        println!(
            "Batch size {}: {:.2} predictions/sec, efficiency: {:.2}",
            batch_size, predictions_per_sec, efficiency
        );
    }

    // Verify scaling efficiency
    let avg_efficiency = efficiencies.iter().sum::<f64>() / efficiencies.len() as f64;
    assert!(
        avg_efficiency >= 0.7,
        "Poor parallel scaling efficiency: {:.2}",
        avg_efficiency
    );

    Ok(())
}

#[tokio::test]
async fn test_cache_effectiveness() -> Result<()> {
    let config = NeuralConfig::default();
    let base_predictor = Arc::new(FannPredictor::new(config)?);
    let optimized = OptimizedFannPredictor::new(base_predictor).await?;

    let data = generate_test_data(100);

    // Clear cache
    optimized.clear_caches();

    // First run (cache misses)
    let start_cold = Instant::now();
    for _ in 0..50 {
        let _ = optimized.predict_batch("MLP", vec![&data], 5).await?;
    }
    let cold_time = start_cold.elapsed();

    // Second run (cache hits)
    let start_warm = Instant::now();
    for _ in 0..50 {
        let _ = optimized.predict_batch("MLP", vec![&data], 5).await?;
    }
    let warm_time = start_warm.elapsed();

    let speedup = cold_time.as_secs_f64() / warm_time.as_secs_f64();
    println!(
        "Cache speedup: {:.2}x (cold: {:?}, warm: {:?})",
        speedup, cold_time, warm_time
    );

    assert!(
        speedup >= 2.0,
        "Insufficient cache speedup: {:.2}x",
        speedup
    );

    // Check cache hit rate
    let metrics = optimized.get_metrics();
    assert!(
        metrics.cache_hit_rate >= 0.8,
        "Low cache hit rate: {:.2}",
        metrics.cache_hit_rate
    );

    Ok(())
}

#[tokio::test]
async fn test_model_preloading() -> Result<()> {
    let config = NeuralConfig::default();

    // Measure time with preloading
    let start_preload = Instant::now();
    let base_predictor = Arc::new(FannPredictor::new(config.clone())?);
    let optimized = OptimizedFannPredictor::new(base_predictor).await?;
    let preload_time = start_preload.elapsed();

    // Measure first prediction time
    let data = generate_test_data(100);
    let start_predict = Instant::now();
    let _ = optimized.predict_batch("MLP", vec![&data], 5).await?;
    let first_predict_time = start_predict.elapsed();

    println!(
        "Preload time: {:?}, First prediction: {:?}",
        preload_time, first_predict_time
    );

    // First prediction should be fast due to preloading
    assert!(
        first_predict_time.as_millis() < 100,
        "First prediction too slow: {:?}",
        first_predict_time
    );

    Ok(())
}

/// Get process memory usage in MB
fn get_process_memory_mb() -> f64 {
    use sysinfo::{ProcessExt, System, SystemExt};

    let mut system = System::new();
    system.refresh_processes();

    let pid = std::process::id();
    system
        .process(pid.into())
        .map(|p| p.memory() as f64 / 1024.0 / 1024.0)
        .unwrap_or(0.0)
}

#[tokio::test]
async fn test_concurrent_predictions() -> Result<()> {
    let config = NeuralConfig::default();
    let base_predictor = Arc::new(FannPredictor::new(config)?);
    let optimized = Arc::new(OptimizedFannPredictor::new(base_predictor).await?);

    let data = Arc::new(generate_test_data(100));
    let concurrent_tasks = 16;

    let start = Instant::now();
    let mut handles = Vec::new();

    for _ in 0..concurrent_tasks {
        let optimized_clone = optimized.clone();
        let data_clone = data.clone();

        let handle = tokio::spawn(async move {
            for _ in 0..10 {
                let _ = optimized_clone
                    .predict_batch("MLP", vec![data_clone.as_slice()], 5)
                    .await?;
            }
            Result::<()>::Ok(())
        });

        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await??;
    }

    let elapsed = start.elapsed();
    let total_predictions = concurrent_tasks * 10;
    let predictions_per_sec = total_predictions as f64 / elapsed.as_secs_f64();

    println!(
        "Concurrent predictions: {} tasks, {:.2} predictions/sec",
        concurrent_tasks, predictions_per_sec
    );

    assert!(
        predictions_per_sec >= MIN_BATCH_THROUGHPUT / 2.0,
        "Poor concurrent performance: {:.2}",
        predictions_per_sec
    );

    Ok(())
}
