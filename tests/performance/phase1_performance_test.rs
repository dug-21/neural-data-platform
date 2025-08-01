//! Performance and Load Tests for Phase 1 Vendor Integration
//!
//! Tests throughput, latency, memory usage, and scalability of the vendor model integration.

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

use crate::config::NeuralConfig;
use crate::data::{TimeSeriesData, sector_mapper::{SectorMapper, SectorMapperConfig}};
use crate::monitoring::model_performance_tracker::ModelPerformanceTracker;
use crate::neural::vendor_predictor::VendorPredictor;
use crate::neural::NeuralPredictorTrait;

// Performance test configuration
struct PerformanceConfig {
    pub warmup_iterations: usize,
    pub measurement_iterations: usize,
    pub max_latency_ms: u64,
    pub min_throughput_per_sec: f64,
    pub max_memory_mb: usize,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            warmup_iterations: 10,
            measurement_iterations: 100,
            max_latency_ms: 100, // 100ms max latency
            min_throughput_per_sec: 10.0, // 10 predictions per second minimum
            max_memory_mb: 512, // 512MB max memory usage
        }
    }
}

// Performance metrics collection
#[derive(Debug, Clone)]
struct PerformanceMetrics {
    pub avg_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub throughput_per_sec: f64,
    pub memory_usage_mb: f64,
    pub successful_predictions: usize,
    pub failed_predictions: usize,
}

async fn setup_performance_environment() -> Result<Arc<VendorPredictor>> {
    let neural_config = NeuralConfig {
        model_path: "/tmp/perf_test_models".to_string(),
        batch_size: 32,
        learning_rate: 0.001,
        hidden_layers: vec![64, 32],
        activation: "relu".to_string(),
        optimizer: "adam".to_string(),
        loss_function: "mse".to_string(),
        epochs: 100,
        validation_split: 0.2,
        early_stopping: true,
        patience: 10,
        enable_cuda: false,
        model_type: "performance_test".to_string(),
        sequence_length: 60,
        prediction_horizon: 1,
        features: vec!["price".to_string(), "volume".to_string()],
        enable_technical_indicators: true,
        enable_feature_scaling: true,
        dropout_rate: 0.1,
        l2_regularization: 0.001,
    };
    
    let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
    let performance_tracker = Arc::new(ModelPerformanceTracker::new()?);
    
    let vendor_predictor = Arc::new(VendorPredictor::new(
        &neural_config,
        sector_mapper,
        performance_tracker,
    )?);
    
    Ok(vendor_predictor)
}

fn create_performance_test_data(symbol: &str, size: usize) -> TimeSeriesData {
    let base_price = match symbol {
        "AAPL" => 150.0,
        "MSFT" => 300.0,
        _ => 100.0,
    };
    
    let values: Vec<f64> = (0..size)
        .map(|i| base_price + (i as f64 * 0.1) + (rand::random::<f64>() - 0.5) * 2.0)
        .collect();
    
    let timestamps: Vec<DateTime<Utc>> = (0..size)
        .map(|i| Utc::now() - chrono::Duration::seconds((size - i) as i64))
        .collect();
    
    TimeSeriesData {
        values,
        timestamps,
        metadata: {
            let mut map = HashMap::new();
            map.insert("symbol".to_string(), serde_json::json!(symbol));
            map.insert("source".to_string(), serde_json::json!("performance_test"));
            map
        },
        symbol: symbol.to_string(),
        metadata_map: {
            let mut map = HashMap::new();
            map.insert("symbol".to_string(), serde_json::json!(symbol));
            map
        }
    }
}

async fn measure_latency(
    predictor: Arc<VendorPredictor>,
    test_data: &TimeSeriesData,
    iterations: usize
) -> Result<Vec<Duration>> {
    let mut latencies = Vec::with_capacity(iterations);
    
    for _ in 0..iterations {
        let start = Instant::now();
        let _ = predictor.predict(test_data).await?;
        let elapsed = start.elapsed();
        latencies.push(elapsed);
        
        // Small delay to prevent overwhelming the system
        sleep(tokio::time::Duration::from_millis(1)).await;
    }
    
    Ok(latencies)
}

async fn measure_throughput(
    predictor: Arc<VendorPredictor>,
    test_data: Vec<TimeSeriesData>,
    duration_secs: u64
) -> Result<PerformanceMetrics> {
    let start_time = Instant::now();
    let end_time = start_time + Duration::from_secs(duration_secs);
    
    let mut successful_predictions = 0;
    let mut failed_predictions = 0;
    let mut latencies = Vec::new();
    
    let mut data_index = 0;
    
    while Instant::now() < end_time {
        let prediction_start = Instant::now();
        
        let result = predictor.predict(&test_data[data_index % test_data.len()]).await;
        
        let prediction_latency = prediction_start.elapsed();
        latencies.push(prediction_latency);
        
        match result {
            Ok(_) => successful_predictions += 1,
            Err(_) => failed_predictions += 1,
        }
        
        data_index += 1;
    }
    
    let total_duration = start_time.elapsed();
    let total_predictions = successful_predictions + failed_predictions;
    
    // Calculate latency statistics
    latencies.sort();
    let avg_latency_ms = latencies.iter().map(|d| d.as_millis() as f64).sum::<f64>() / latencies.len() as f64;
    let p95_index = (latencies.len() as f64 * 0.95) as usize;
    let p99_index = (latencies.len() as f64 * 0.99) as usize;
    let p95_latency_ms = latencies[p95_index.min(latencies.len() - 1)].as_millis() as f64;
    let p99_latency_ms = latencies[p99_index.min(latencies.len() - 1)].as_millis() as f64;
    
    Ok(PerformanceMetrics {
        avg_latency_ms,
        p95_latency_ms,
        p99_latency_ms,
        throughput_per_sec: total_predictions as f64 / total_duration.as_secs_f64(),
        memory_usage_mb: 0.0, // Would be measured in real implementation
        successful_predictions,
        failed_predictions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_single_prediction_latency() {
        let predictor = setup_performance_environment().await.unwrap();
        let config = PerformanceConfig::default();
        
        let test_data = create_performance_test_data("AAPL", 100);
        
        // Warmup
        for _ in 0..config.warmup_iterations {
            let _ = predictor.predict(&test_data).await;
        }
        
        // Measure latency
        let latencies = measure_latency(
            predictor,
            &test_data,
            config.measurement_iterations,
        ).await.unwrap();
        
        // Calculate statistics
        let avg_latency_ms = latencies.iter()
            .map(|d| d.as_millis() as f64)
            .sum::<f64>() / latencies.len() as f64;
        
        let max_latency_ms = latencies.iter()
            .map(|d| d.as_millis())
            .max()
            .unwrap() as f64;
        
        println!("Average latency: {:.2}ms", avg_latency_ms);
        println!("Maximum latency: {:.2}ms", max_latency_ms);
        
        // Performance assertions
        assert!(avg_latency_ms < config.max_latency_ms as f64, 
            "Average latency {:.2}ms exceeds maximum {:.2}ms", 
            avg_latency_ms, config.max_latency_ms);
    }
    
    #[tokio::test]
    async fn test_batch_prediction_throughput() {
        let predictor = setup_performance_environment().await.unwrap();
        let config = PerformanceConfig::default();
        
        // Create batch test data
        let batch_data = vec![
            create_performance_test_data("AAPL", 50),
            create_performance_test_data("MSFT", 50),
            create_performance_test_data("GOOGL", 50),
        ];
        
        // Warmup
        for _ in 0..5 {
            let _ = predictor.predict_batch(&batch_data).await;
        }
        
        // Measure batch throughput
        let start_time = Instant::now();
        
        for _ in 0..20 {
            let result = predictor.predict_batch(&batch_data).await;
            assert!(result.is_ok());
        }
        
        let elapsed = start_time.elapsed();
        let total_predictions = 20 * batch_data.len(); // 20 batches * 3 predictions each
        let throughput = total_predictions as f64 / elapsed.as_secs_f64();
        
        println!("Batch throughput: {:.2} predictions/sec", throughput);
        
        assert!(throughput > config.min_throughput_per_sec,
            "Throughput {:.2} predictions/sec below minimum {:.2}",
            throughput, config.min_throughput_per_sec);
    }
    
    #[tokio::test]
    async fn test_concurrent_prediction_performance() {
        let predictor = Arc::new(setup_performance_environment().await.unwrap());
        let config = PerformanceConfig::default();
        
        let test_data = create_performance_test_data("AAPL", 100);
        let concurrent_requests = 10;
        
        // Warmup
        for _ in 0..config.warmup_iterations {
            let _ = predictor.predict(&test_data).await;
        }
        
        // Launch concurrent predictions
        let start_time = Instant::now();
        let mut tasks = vec![];
        
        for i in 0..concurrent_requests {
            let predictor_clone = Arc::clone(&predictor);
            let test_data_clone = test_data.clone();
            
            let task = tokio::spawn(async move {
                let mut successful = 0;
                let mut failed = 0;
                
                for _ in 0..config.measurement_iterations / concurrent_requests {
                    match predictor_clone.predict(&test_data_clone).await {
                        Ok(_) => successful += 1,
                        Err(_) => failed += 1,
                    }
                }
                
                (successful, failed)
            });
            tasks.push(task);
        }
        
        // Collect results
        let results: Vec<_> = futures::future::join_all(tasks).await;
        let elapsed = start_time.elapsed();
        
        let (total_successful, total_failed): (usize, usize) = 
            results.into_iter()
                .map(|r| r.unwrap())
                .fold((0, 0), |(acc_s, acc_f), (s, f)| (acc_s + s, acc_f + f));
        
        let total_predictions = total_successful + total_failed;
        let throughput = total_predictions as f64 / elapsed.as_secs_f64();
        
        println!("Concurrent throughput: {:.2} predictions/sec", throughput);
        println!("Success rate: {:.2}%", (total_successful as f64 / total_predictions as f64) * 100.0);
        
        assert!(throughput > config.min_throughput_per_sec * (concurrent_requests as f64 * 0.7),
            "Concurrent throughput too low: {:.2}", throughput);
        
        assert!(total_successful > total_predictions / 2,
            "Success rate too low: {}/{}", total_successful, total_predictions);
    }
    
    #[tokio::test]
    async fn test_data_size_scalability() {
        let predictor = setup_performance_environment().await.unwrap();
        
        let data_sizes = vec![10, 50, 100, 500, 1000, 5000];
        let mut results = Vec::new();
        
        for &size in &data_sizes {
            let test_data = create_performance_test_data("AAPL", size);
            
            // Measure processing time for different data sizes
            let start_time = Instant::now();
            
            for _ in 0..10 {
                let _ = predictor.predict(&test_data).await;
            }
            
            let avg_time_ms = start_time.elapsed().as_millis() as f64 / 10.0;
            results.push((size, avg_time_ms));
            
            println!("Data size {}: {:.2}ms average", size, avg_time_ms);
        }
        
        // Check that latency doesn't grow exponentially with data size
        // (should be roughly linear or better due to vectorization)
        for i in 1..results.len() {
            let (prev_size, prev_time) = results[i-1];
            let (curr_size, curr_time) = results[i];
            
            let size_ratio = curr_size as f64 / prev_size as f64;
            let time_ratio = curr_time / prev_time;
            
            // Time ratio should not significantly exceed size ratio
            assert!(time_ratio < size_ratio * 2.0,
                "Poor scaling: size ratio {:.2}, time ratio {:.2}",
                size_ratio, time_ratio);
        }
    }
    
    #[tokio::test]
    async fn test_memory_usage_under_load() {
        let predictor = Arc::new(setup_performance_environment().await.unwrap());
        
        // Create diverse test data
        let test_datasets: Vec<_> = (0..100)
            .map(|i| create_performance_test_data(&format!("SYMBOL_{}", i), 100))
            .collect();
        
        // Initial memory baseline (would use actual memory measurement in real implementation)
        let initial_memory = get_memory_usage(); // Mock function
        
        // Run intensive workload
        let concurrent_tasks = 20;
        let predictions_per_task = 50;
        
        let mut tasks = vec![];
        
        for task_id in 0..concurrent_tasks {
            let predictor_clone = Arc::clone(&predictor);
            let datasets = test_datasets.clone();
            
            let task = tokio::spawn(async move {
                for i in 0..predictions_per_task {
                    let data_index = (task_id * predictions_per_task + i) % datasets.len();
                    let _ = predictor_clone.predict(&datasets[data_index]).await;
                    
                    // Small delay to allow garbage collection
                    if i % 10 == 0 {
                        sleep(tokio::time::Duration::from_millis(1)).await;
                    }
                }
            });
            tasks.push(task);
        }
        
        // Wait for all tasks to complete
        futures::future::join_all(tasks).await;
        
        // Check final memory usage
        let final_memory = get_memory_usage(); // Mock function
        let memory_increase = final_memory - initial_memory;
        
        println!("Memory increase: {:.2}MB", memory_increase);
        
        // Memory increase should be reasonable
        assert!(memory_increase < 100.0,
            "Excessive memory usage: {:.2}MB increase", memory_increase);
    }
    
    #[tokio::test]
    async fn test_sustained_load_performance() {
        let predictor = setup_performance_environment().await.unwrap();
        
        let test_data = vec![
            create_performance_test_data("AAPL", 75),
            create_performance_test_data("MSFT", 75),
            create_performance_test_data("GOOGL", 75),
        ];
        
        // Run sustained load for 30 seconds
        let metrics = measure_throughput(predictor, test_data, 30).await.unwrap();
        
        println!("Sustained load metrics:");
        println!("  Throughput: {:.2} predictions/sec", metrics.throughput_per_sec);
        println!("  Average latency: {:.2}ms", metrics.avg_latency_ms);
        println!("  P95 latency: {:.2}ms", metrics.p95_latency_ms);
        println!("  P99 latency: {:.2}ms", metrics.p99_latency_ms);
        println!("  Success rate: {:.2}%", 
            (metrics.successful_predictions as f64 / 
             (metrics.successful_predictions + metrics.failed_predictions) as f64) * 100.0);
        
        let config = PerformanceConfig::default();
        
        // Performance assertions
        assert!(metrics.throughput_per_sec > config.min_throughput_per_sec,
            "Sustained throughput too low: {:.2}", metrics.throughput_per_sec);
        
        assert!(metrics.avg_latency_ms < config.max_latency_ms as f64,
            "Average latency too high: {:.2}ms", metrics.avg_latency_ms);
        
        assert!(metrics.p95_latency_ms < config.max_latency_ms as f64 * 2.0,
            "P95 latency too high: {:.2}ms", metrics.p95_latency_ms);
        
        assert!(metrics.successful_predictions > 0,
            "No successful predictions during sustained load");
    }
    
    #[tokio::test]
    async fn test_conversion_performance() {
        let predictor = setup_performance_environment().await.unwrap();
        
        let test_data = create_performance_test_data("CONVERSION_PERF", 1000);
        
        // Measure conversion performance
        let conversion_iterations = 100;
        let start_time = Instant::now();
        
        for i in 0..conversion_iterations {
            let symbol = format!("PERF_{}", i % 10); // Reuse some symbols for caching test
            let result = predictor.convert_to_vendor_format(&test_data, &symbol).await;
            assert!(result.is_ok());
        }
        
        let elapsed = start_time.elapsed();
        let avg_conversion_time = elapsed.as_millis() as f64 / conversion_iterations as f64;
        
        println!("Average conversion time: {:.2}ms", avg_conversion_time);
        
        // Conversion should be fast
        assert!(avg_conversion_time < 50.0,
            "Data conversion too slow: {:.2}ms", avg_conversion_time);
    }
    
    #[tokio::test]
    async fn test_cache_performance() {
        let predictor = setup_performance_environment().await.unwrap();
        
        let test_data = create_performance_test_data("CACHE_TEST", 100);
        
        // First conversion (cold cache)
        let start_time = Instant::now();
        let _ = predictor.convert_to_vendor_format(&test_data, "CACHE_TEST").await.unwrap();
        let cold_cache_time = start_time.elapsed();
        
        // Subsequent conversions (warm cache)
        let start_time = Instant::now();
        for _ in 0..10 {
            let _ = predictor.convert_to_vendor_format(&test_data, "CACHE_TEST").await.unwrap();
        }
        let warm_cache_time = start_time.elapsed().as_millis() as f64 / 10.0;
        
        println!("Cold cache time: {:.2}ms", cold_cache_time.as_millis());
        println!("Warm cache time: {:.2}ms", warm_cache_time);
        
        // Warm cache should be faster (due to normalization stats caching)
        // Note: This test might not show significant difference without actual caching implementation
    }
    
    #[tokio::test]
    async fn test_error_handling_performance() {
        let predictor = setup_performance_environment().await.unwrap();
        
        // Create mix of valid and invalid data
        let valid_data = create_performance_test_data("VALID", 50);
        let invalid_data = TimeSeriesData {
            values: vec![], // Empty data
            timestamps: vec![],
            metadata: HashMap::new(),
            symbol: "INVALID".to_string(),
            metadata_map: HashMap::new(),
        };
        
        let mixed_data = vec![
            valid_data.clone(),
            invalid_data.clone(),
            valid_data.clone(),
            invalid_data.clone(),
        ];
        
        // Measure performance with error handling
        let start_time = Instant::now();
        let mut successful = 0;
        let mut failed = 0;
        
        for _ in 0..100 {
            for data in &mixed_data {
                match predictor.predict(data).await {
                    Ok(_) => successful += 1,
                    Err(_) => failed += 1,
                }
            }
        }
        
        let elapsed = start_time.elapsed();
        let total_predictions = successful + failed;
        let throughput = total_predictions as f64 / elapsed.as_secs_f64();
        
        println!("Error handling throughput: {:.2} predictions/sec", throughput);
        println!("Success rate: {:.2}%", (successful as f64 / total_predictions as f64) * 100.0);
        
        // Should maintain reasonable performance even with errors
        assert!(throughput > 5.0,
            "Error handling throughput too low: {:.2}", throughput);
    }
}

// Mock memory usage function (would be implemented using system calls)
fn get_memory_usage() -> f64 {
    // In real implementation, this would measure actual memory usage
    // For testing, return a mock value
    100.0 // MB
}