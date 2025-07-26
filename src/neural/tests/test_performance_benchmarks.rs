//! Performance Benchmarks and Stress Tests for Neural Prediction System
//! 
//! This module provides comprehensive performance testing for:
//! - Prediction latency and throughput
//! - Memory usage optimization
//! - Concurrent request handling
//! - Large dataset processing
//! - Cache effectiveness
//! - Model training performance

use super::super::enhanced_predictor::*;
use super::super::fann_predictor::*;
use super::super::{PredictionResult, NeuralPredictorTrait};
use crate::config::NeuralConfig;
use crate::data::TimeSeriesData;

use chrono::{DateTime, Utc, TimeZone};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio;
use anyhow::Result;
use serde_json::json;
use tracing_test::traced_test;

/// Helper function to create performance test configuration
fn create_performance_config() -> NeuralConfig {
    NeuralConfig {
        memory_gb: 2.0,
        models: vec![
            "MLP".to_string(),
            "NHITS".to_string(), 
            "DeepAR".to_string(),
            "LSTM".to_string()
        ],
        prediction_cache_ttl: 600,
        model_load_timeout: 120,
        max_concurrent_predictions: 50,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    }
}

/// Helper function to create large dataset for performance testing
fn create_large_dataset(count: usize) -> Vec<TimeSeriesData> {
    (0..count)
        .map(|i| {
            let mut indicators = HashMap::new();
            indicators.insert("rsi".to_string(), 30.0 + (i as f64 % 40.0));
            indicators.insert("macd".to_string(), -1.0 + (i as f64 % 20.0) * 0.1);
            indicators.insert("bb_position".to_string(), (i as f64 % 10.0) * 0.1);
            indicators.insert("volume_sma".to_string(), 1000000.0 + (i as f64 * 100.0));
            indicators.insert("price_momentum".to_string(), -0.5 + (i as f64 % 10.0) * 0.1);
            
            let base_price = 100.0;
            let trend = (i as f64 * 0.01).sin() * 5.0;
            let noise = (i as f64 * 0.1).cos() * 0.5;
            let price = base_price + trend + noise;
            
            TimeSeriesData {
                timestamp: Utc.timestamp_opt(1640000000 + (i as i64 * 60), 0).unwrap(), // 1-minute intervals
                symbol: "PERF_TEST".to_string(),
                open: price - 0.1,
                high: price + 0.2,
                low: price - 0.2,
                close: price,
                volume: 1000000.0 + (i as f64 * 1000.0),
                indicators,
                source: Some("performance_test".to_string()),
                entity: Some("PERF_TEST".to_string()),
                value: Some(price),
                metadata: Some(json!({
                    "test_id": i,
                    "batch": i / 1000,
                    "synthetic": true
                })),
            }
        })
        .collect()
}

/// Performance metrics structure
#[derive(Debug, Clone)]
struct PerformanceMetrics {
    duration: Duration,
    throughput_predictions_per_second: f64,
    memory_usage_mb: f64,
    cache_hit_rate: f64,
    avg_confidence: f64,
    success_rate: f64,
}

/// Helper function to measure memory usage (simplified)
fn estimate_memory_usage() -> f64 {
    // This is a simplified estimate - in practice you'd use system metrics
    // For now, return a placeholder value
    let process = std::process::Command::new("ps")
        .args(&["-o", "rss=", "-p", &std::process::id().to_string()])
        .output();
        
    match process {
        Ok(output) => {
            let rss_kb = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<f64>()
                .unwrap_or(0.0);
            rss_kb / 1024.0 // Convert to MB
        },
        Err(_) => 100.0 // Fallback estimate
    }
}

mod latency_performance_tests {
    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn test_single_prediction_latency() -> Result<()> {
        let config = create_performance_config();
        let predictor = EnhancedNeuralPredictor::new(config)?;
        let test_data = create_large_dataset(100);
        
        // Warm up
        let _ = predictor.predict_with_confidence(&test_data, 1).await?;
        
        // Measure single prediction latency
        let start = Instant::now();
        let predictions = predictor.predict_with_confidence(&test_data, 5).await?;
        let duration = start.elapsed();
        
        assert_eq!(predictions.len(), 5);
        
        // Latency should be reasonable (less than 1 second for small dataset)
        assert!(duration < Duration::from_secs(1), 
               "Single prediction took too long: {:?}", duration);
        
        println!("✅ Single prediction latency: {:?}", duration);
        
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_batch_prediction_latency() -> Result<()> {
        let config = create_performance_config();
        let predictor = EnhancedNeuralPredictor::new(config)?;
        let test_data = create_large_dataset(500);
        
        // Measure batch prediction latency
        let start = Instant::now();
        let predictions = predictor.predict_with_confidence(&test_data, 10).await?;
        let duration = start.elapsed();
        
        assert_eq!(predictions.len(), 10);
        
        // Calculate throughput
        let throughput = predictions.len() as f64 / duration.as_secs_f64();
        
        println!("✅ Batch prediction latency: {:?}", duration);
        println!("✅ Throughput: {:.2} predictions/second", throughput);
        
        // Should achieve reasonable throughput
        assert!(throughput > 1.0, "Throughput too low: {:.2} pred/sec", throughput);
        
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_ensemble_prediction_latency() -> Result<()> {
        let config = create_performance_config();
        let fann_predictor = FannPredictor::new(config)?;
        let test_data = create_large_dataset(200);
        
        let models = vec!["MLP".to_string(), "NHITS".to_string(), "DeepAR".to_string()];
        
        // Measure ensemble prediction latency
        let start = Instant::now();
        let predictions = fann_predictor.predict_ensemble(&test_data, 8, &models, None).await?;
        let duration = start.elapsed();
        
        assert_eq!(predictions.len(), 8);
        
        println!("✅ Ensemble prediction latency: {:?}", duration);
        
        // Ensemble should complete within reasonable time
        assert!(duration < Duration::from_secs(5), 
               "Ensemble prediction took too long: {:?}", duration);
        
        Ok(())
    }
}

mod throughput_performance_tests {
    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn test_concurrent_prediction_throughput() -> Result<()> {
        let config = create_performance_config();
        let predictor = Arc::new(EnhancedNeuralPredictor::new(config)?);
        let test_data = create_large_dataset(100);
        
        let num_concurrent_requests = 10;
        let predictions_per_request = 5;
        
        // Spawn concurrent prediction tasks
        let start = Instant::now();
        let mut handles = vec![];
        
        for i in 0..num_concurrent_requests {
            let predictor_clone = Arc::clone(&predictor);
            let data_clone = test_data.clone();
            
            let handle = tokio::spawn(async move {
                let horizon = predictions_per_request + (i % 3); // Vary horizon slightly
                predictor_clone.predict_with_confidence(&data_clone, horizon).await
            });
            handles.push(handle);
        }
        
        // Wait for all tasks to complete
        let mut total_predictions = 0;
        let mut successful_requests = 0;
        
        for handle in handles {
            match handle.await {
                Ok(Ok(predictions)) => {
                    total_predictions += predictions.len();
                    successful_requests += 1;
                },
                _ => {
                    println!("⚠️ A concurrent request failed");
                }
            }
        }
        
        let duration = start.elapsed();
        let throughput = total_predictions as f64 / duration.as_secs_f64();
        let success_rate = successful_requests as f64 / num_concurrent_requests as f64;
        
        println!("✅ Concurrent throughput: {:.2} predictions/second", throughput);
        println!("✅ Success rate: {:.2}%", success_rate * 100.0);
        
        // Should handle concurrent requests well
        assert!(success_rate >= 0.8, "Success rate too low: {:.2}%", success_rate * 100.0);
        assert!(throughput > 5.0, "Throughput too low: {:.2} pred/sec", throughput);
        
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_sustained_load_performance() -> Result<()> {
        let config = create_performance_config();
        let predictor = EnhancedNeuralPredictor::new(config)?;
        let test_data = create_large_dataset(150);
        
        let num_iterations = 20;
        let mut total_duration = Duration::from_secs(0);
        let mut total_predictions = 0;
        let mut successful_iterations = 0;
        
        // Sustained load test
        for i in 0..num_iterations {
            let start = Instant::now();
            
            match predictor.predict_with_confidence(&test_data, 3).await {
                Ok(predictions) => {
                    let duration = start.elapsed();
                    total_duration += duration;
                    total_predictions += predictions.len();
                    successful_iterations += 1;
                    
                    // Small delay between requests
                    tokio::time::sleep(Duration::from_millis(100)).await;
                },
                Err(_) => {
                    println!("⚠️ Iteration {} failed", i);
                }
            }
        }
        
        let avg_latency = total_duration / successful_iterations.max(1) as u32;
        let overall_throughput = total_predictions as f64 / total_duration.as_secs_f64();
        let success_rate = successful_iterations as f64 / num_iterations as f64;
        
        println!("✅ Sustained load - Average latency: {:?}", avg_latency);
        println!("✅ Sustained load - Overall throughput: {:.2} pred/sec", overall_throughput);
        println!("✅ Sustained load - Success rate: {:.2}%", success_rate * 100.0);
        
        // Performance should remain stable under sustained load
        assert!(success_rate >= 0.9, "Success rate degraded: {:.2}%", success_rate * 100.0);
        assert!(avg_latency < Duration::from_secs(2), "Average latency too high: {:?}", avg_latency);
        
        Ok(())
    }
}

mod memory_performance_tests {
    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn test_memory_usage_with_large_datasets() -> Result<()> {
        let config = create_performance_config();
        let predictor = EnhancedNeuralPredictor::new(config)?;
        
        // Measure memory before processing
        let initial_memory = estimate_memory_usage();
        
        // Process increasingly large datasets
        let dataset_sizes = vec![100, 500, 1000, 2000];
        let mut memory_usage_points = vec![];
        
        for size in dataset_sizes {
            let large_data = create_large_dataset(size);
            
            // Process the dataset
            let _ = predictor.predict_with_confidence(&large_data, 5).await?;
            
            // Measure memory usage
            let current_memory = estimate_memory_usage();
            memory_usage_points.push((size, current_memory));
            
            println!("Dataset size: {}, Memory usage: {:.2} MB", size, current_memory);
            
            // Small delay to allow garbage collection
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        let final_memory = estimate_memory_usage();
        let memory_increase = final_memory - initial_memory;
        
        println!("✅ Memory increase: {:.2} MB", memory_increase);
        
        // Memory usage should be reasonable (allowing for some growth)
        assert!(memory_increase < 500.0, "Memory usage too high: {:.2} MB", memory_increase);
        
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_memory_efficiency_with_caching() -> Result<()> {
        let config = create_performance_config();
        let fann_predictor = FannPredictor::new(config)?;
        let test_data = create_large_dataset(200);
        
        let initial_memory = estimate_memory_usage();
        
        // First prediction (should be cached)
        let start1 = Instant::now();
        let _predictions1 = fann_predictor.predict(&test_data, 5, None).await?;
        let duration1 = start1.elapsed();
        
        let memory_after_first = estimate_memory_usage();
        
        // Second prediction with same data (should use cache)
        let start2 = Instant::now();
        let _predictions2 = fann_predictor.predict(&test_data, 5, None).await?;
        let duration2 = start2.elapsed();
        
        let memory_after_second = estimate_memory_usage();
        
        println!("✅ First prediction time: {:?}", duration1);
        println!("✅ Second prediction time: {:?}", duration2);
        println!("✅ Memory after first: {:.2} MB", memory_after_first);
        println!("✅ Memory after second: {:.2} MB", memory_after_second);
        
        // Second prediction should be faster (cached)
        assert!(duration2 <= duration1, "Cache not working - second prediction slower");
        
        // Memory should not increase significantly for cached predictions
        let cache_memory_increase = memory_after_second - memory_after_first;
        assert!(cache_memory_increase < 50.0, "Cache using too much memory: {:.2} MB", cache_memory_increase);
        
        Ok(())
    }
}

mod scalability_tests {
    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn test_scalability_with_increasing_horizons() -> Result<()> {
        let config = create_performance_config();
        let predictor = EnhancedNeuralPredictor::new(config)?;
        let test_data = create_large_dataset(300);
        
        let horizons = vec![1, 5, 10, 20, 50];
        let mut performance_metrics = vec![];
        
        for horizon in horizons {
            let start = Instant::now();
            
            match predictor.predict_with_confidence(&test_data, horizon).await {
                Ok(predictions) => {
                    let duration = start.elapsed();
                    let throughput = predictions.len() as f64 / duration.as_secs_f64();
                    
                    performance_metrics.push((horizon, duration, throughput));
                    
                    println!("Horizon: {}, Duration: {:?}, Throughput: {:.2} pred/sec", 
                            horizon, duration, throughput);
                    
                    assert_eq!(predictions.len(), horizon);
                },
                Err(e) => {
                    println!("⚠️ Failed at horizon {}: {}", horizon, e);
                }
            }
        }
        
        // Should handle increasing horizons gracefully
        assert!(performance_metrics.len() >= 3, "Too many horizon tests failed");
        
        // Performance should scale reasonably with horizon
        for (horizon, duration, throughput) in &performance_metrics {
            assert!(duration.as_secs() < 10, "Horizon {} took too long: {:?}", horizon, duration);
            assert!(*throughput > 0.5, "Throughput too low for horizon {}: {:.2}", horizon, throughput);
        }
        
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_scalability_with_model_count() -> Result<()> {
        let config = create_performance_config();
        let fann_predictor = FannPredictor::new(config)?;
        let test_data = create_large_dataset(200);
        
        let model_sets = vec![
            vec!["MLP".to_string()],
            vec!["MLP".to_string(), "NHITS".to_string()],
            vec!["MLP".to_string(), "NHITS".to_string(), "DeepAR".to_string()],
            vec!["MLP".to_string(), "NHITS".to_string(), "DeepAR".to_string(), "LSTM".to_string()],
        ];
        
        let mut ensemble_metrics = vec![];
        
        for models in model_sets {
            let start = Instant::now();
            
            match fann_predictor.predict_ensemble(&test_data, 5, &models, None).await {
                Ok(predictions) => {
                    let duration = start.elapsed();
                    
                    ensemble_metrics.push((models.len(), duration));
                    
                    println!("Models: {}, Duration: {:?}", models.len(), duration);
                    
                    assert_eq!(predictions.len(), 5);
                    
                    // Check ensemble quality
                    for pred in &predictions {
                        assert!(pred.model_name.contains("ensemble"));
                        assert!(pred.confidence >= 0.0 && pred.confidence <= 1.0);
                    }
                },
                Err(e) => {
                    println!("⚠️ Failed with {} models: {}", models.len(), e);
                }
            }
        }
        
        // Should scale with number of models
        assert!(ensemble_metrics.len() >= 2, "Too many ensemble tests failed");
        
        // Performance should scale reasonably with model count
        for (model_count, duration) in &ensemble_metrics {
            assert!(duration.as_secs() < 15, "{} models took too long: {:?}", model_count, duration);
        }
        
        Ok(())
    }
}

mod stress_tests {
    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn test_high_frequency_requests() -> Result<()> {
        let config = create_performance_config();
        let predictor = Arc::new(EnhancedNeuralPredictor::new(config)?);
        let test_data = create_large_dataset(100);
        
        let num_requests = 50;
        let start = Instant::now();
        
        // Fire many requests rapidly
        let mut handles = vec![];
        for i in 0..num_requests {
            let predictor_clone = Arc::clone(&predictor);
            let data_clone = test_data.clone();
            
            let handle = tokio::spawn(async move {
                let horizon = 1 + (i % 5); // Vary horizon 1-5
                predictor_clone.predict_with_confidence(&data_clone, horizon).await
            });
            handles.push(handle);
        }
        
        // Collect results
        let mut successful = 0;
        let mut total_predictions = 0;
        
        for handle in handles {
            match handle.await {
                Ok(Ok(predictions)) => {
                    successful += 1;
                    total_predictions += predictions.len();
                },
                _ => {}
            }
        }
        
        let duration = start.elapsed();
        let success_rate = successful as f64 / num_requests as f64;
        let throughput = total_predictions as f64 / duration.as_secs_f64();
        
        println!("✅ High frequency test - Success rate: {:.2}%", success_rate * 100.0);
        println!("✅ High frequency test - Throughput: {:.2} pred/sec", throughput);
        
        // Should handle high frequency requests
        assert!(success_rate >= 0.7, "Success rate too low under stress: {:.2}%", success_rate * 100.0);
        
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_error_recovery_performance() -> Result<()> {
        let config = create_performance_config();
        let predictor = EnhancedNeuralPredictor::new(config)?;
        
        // Test with problematic data that might cause errors
        let mut problematic_data = create_large_dataset(50);
        
        // Inject some problematic values
        problematic_data[10].close = f64::NAN;
        problematic_data[20].volume = f64::INFINITY;
        problematic_data[30].high = f64::NEG_INFINITY;
        
        let start = Instant::now();
        
        // Should handle errors gracefully
        let result = predictor.predict_with_confidence(&problematic_data, 3).await;
        
        let duration = start.elapsed();
        
        println!("✅ Error recovery test duration: {:?}", duration);
        
        match result {
            Ok(predictions) => {
                // If successful, predictions should be valid
                for pred in &predictions {
                    assert!(pred.confidence.is_finite());
                    assert!(pred.value.is_finite());
                }
                println!("✅ Successfully handled problematic data");
            },
            Err(_) => {
                // Also acceptable to reject problematic data
                println!("✅ Appropriately rejected problematic data");
            }
        }
        
        // Should complete quickly even with errors
        assert!(duration < Duration::from_secs(5), "Error handling took too long: {:?}", duration);
        
        Ok(())
    }
}

mod cache_performance_tests {
    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn test_cache_hit_rate_performance() -> Result<()> {
        let config = create_performance_config();
        let fann_predictor = FannPredictor::new(config)?;
        let test_data = create_large_dataset(150);
        
        let num_repeated_requests = 10;
        let mut cache_hits = 0;
        let mut total_requests = 0;
        
        // Make initial request (cache miss)
        let initial_start = Instant::now();
        let _initial_result = fann_predictor.predict(&test_data, 3, None).await?;
        let initial_duration = initial_start.elapsed();
        
        // Make repeated requests (should be cache hits)
        for _ in 0..num_repeated_requests {
            let start = Instant::now();
            let _result = fann_predictor.predict(&test_data, 3, None).await?;
            let duration = start.elapsed();
            
            total_requests += 1;
            
            // If much faster than initial, likely a cache hit
            if duration < initial_duration / 2 {
                cache_hits += 1;
            }
        }
        
        let cache_hit_rate = cache_hits as f64 / total_requests as f64;
        
        println!("✅ Cache hit rate: {:.2}%", cache_hit_rate * 100.0);
        println!("✅ Initial request: {:?}", initial_duration);
        
        // Should achieve good cache hit rate
        assert!(cache_hit_rate >= 0.8, "Cache hit rate too low: {:.2}%", cache_hit_rate * 100.0);
        
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_cache_memory_efficiency() -> Result<()> {
        let config = create_performance_config();
        let fann_predictor = FannPredictor::new(config)?;
        
        let initial_memory = estimate_memory_usage();
        
        // Create many different datasets to test cache behavior
        let num_datasets = 20;
        for i in 0..num_datasets {
            let mut test_data = create_large_dataset(100);
            
            // Make each dataset slightly different
            for data_point in &mut test_data {
                data_point.close += i as f64 * 0.1;
            }
            
            let _ = fann_predictor.predict(&test_data, 3, None).await?;
        }
        
        let final_memory = estimate_memory_usage();
        let memory_increase = final_memory - initial_memory;
        
        println!("✅ Memory increase with {} cached datasets: {:.2} MB", num_datasets, memory_increase);
        
        // Cache should not consume excessive memory
        assert!(memory_increase < 200.0, "Cache using too much memory: {:.2} MB", memory_increase);
        
        Ok(())
    }
}

/// Comprehensive performance benchmark test
#[tokio::test]
#[traced_test]
async fn test_comprehensive_performance_benchmark() -> Result<()> {
    println!("🚀 Running Comprehensive Performance Benchmark");
    
    let config = create_performance_config();
    let enhanced_predictor = EnhancedNeuralPredictor::new(config.clone())?;
    let fann_predictor = FannPredictor::new(config)?;
    
    let test_data = create_large_dataset(500);
    let initial_memory = estimate_memory_usage();
    
    // Benchmark 1: Enhanced Predictor Performance
    let start = Instant::now();
    let enhanced_predictions = enhanced_predictor.predict_with_confidence(&test_data, 10).await?;
    let enhanced_duration = start.elapsed();
    
    // Benchmark 2: FANN Ensemble Performance
    let models = vec!["MLP".to_string(), "NHITS".to_string(), "DeepAR".to_string()];
    let start = Instant::now();
    let ensemble_predictions = fann_predictor.predict_ensemble(&test_data, 10, &models, None).await?;
    let ensemble_duration = start.elapsed();
    
    // Benchmark 3: Concurrent Performance
    let predictor_arc = Arc::new(enhanced_predictor);
    let start = Instant::now();
    let mut handles = vec![];
    
    for i in 0..5 {
        let predictor_clone = Arc::clone(&predictor_arc);
        let data_clone = test_data.clone();
        
        let handle = tokio::spawn(async move {
            predictor_clone.predict_with_confidence(&data_clone, 5 + (i % 3)).await
        });
        handles.push(handle);
    }
    
    let mut concurrent_success = 0;
    for handle in handles {
        if handle.await.is_ok() {
            concurrent_success += 1;
        }
    }
    let concurrent_duration = start.elapsed();
    
    let final_memory = estimate_memory_usage();
    
    // Calculate comprehensive metrics
    let enhanced_throughput = enhanced_predictions.len() as f64 / enhanced_duration.as_secs_f64();
    let ensemble_throughput = ensemble_predictions.len() as f64 / ensemble_duration.as_secs_f64();
    let concurrent_success_rate = concurrent_success as f64 / 5.0;
    let memory_usage = final_memory - initial_memory;
    
    // Calculate average confidence
    let enhanced_avg_confidence = enhanced_predictions.iter()
        .map(|p| p.confidence)
        .sum::<f64>() / enhanced_predictions.len() as f64;
    
    let ensemble_avg_confidence = ensemble_predictions.iter()
        .map(|p| p.confidence)
        .sum::<f64>() / ensemble_predictions.len() as f64;
    
    // Print comprehensive benchmark results
    println!("📊 PERFORMANCE BENCHMARK RESULTS");
    println!("=====================================");
    println!("📈 Enhanced Predictor:");
    println!("   • Duration: {:?}", enhanced_duration);
    println!("   • Throughput: {:.2} predictions/second", enhanced_throughput);
    println!("   • Average Confidence: {:.3}", enhanced_avg_confidence);
    println!("");
    println!("🤖 FANN Ensemble:");
    println!("   • Duration: {:?}", ensemble_duration);
    println!("   • Throughput: {:.2} predictions/second", ensemble_throughput);
    println!("   • Average Confidence: {:.3}", ensemble_avg_confidence);
    println!("");
    println!("⚡ Concurrent Performance:");
    println!("   • Duration: {:?}", concurrent_duration);
    println!("   • Success Rate: {:.1}%", concurrent_success_rate * 100.0);
    println!("");
    println!("💾 Memory Usage: {:.2} MB", memory_usage);
    println!("=====================================");
    
    // Performance assertions
    assert!(enhanced_throughput > 1.0, "Enhanced predictor throughput too low");
    assert!(ensemble_throughput > 0.5, "Ensemble throughput too low");
    assert!(concurrent_success_rate >= 0.8, "Concurrent success rate too low");
    assert!(memory_usage < 300.0, "Memory usage too high");
    assert!(enhanced_avg_confidence > 0.1, "Average confidence too low");
    assert!(ensemble_avg_confidence > 0.1, "Ensemble confidence too low");
    
    println!("✅ All performance benchmarks passed!");
    
    Ok(())
}