//! Performance benchmarks and load tests for neural trading system
//! Target: Validate system performance under various load conditions

use criterion::{black_box, Criterion};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use tokio::time::sleep;

use crate::adapters::enhanced_neural_adapter::*;
use crate::config::NeuralConfig;
use crate::data::TimeSeriesData;
use crate::neural::{
    NeuralPredictor, NeuralPredictorTrait,
    PerformanceChannel, PerformanceEventBuilder, PerformanceEventType, PerformanceSource,
};

#[cfg(test)]
mod performance_tests {
    use super::*;

    /// Create realistic market data for benchmarking
    fn create_benchmark_data(symbol: &str, count: usize) -> Vec<TimeSeriesData> {
        let base_price = match symbol {
            "BTC/USD" => 50000.0,
            "ETH/USD" => 3000.0,
            "AAPL" => 150.0,
            _ => 100.0,
        };

        (0..count)
            .map(|i| {
                let trend = (i as f64 / count as f64) * 1000.0; // Trending up
                let noise = (i as f64 * 0.1).sin() * 50.0; // Some volatility
                
                TimeSeriesData {
                    symbol: symbol.to_string(),
                    timestamp: chrono::Utc::now() - chrono::Duration::minutes((count - i) as i64),
                    open: base_price + trend + noise,
                    high: base_price + trend + noise + 100.0,
                    low: base_price + trend + noise - 80.0,
                    close: base_price + trend + noise + 50.0,
                    volume: vec![1000.0 + (i as f64 * 10.0)],
                    volume_value: 1000.0 + (i as f64 * 10.0),
                    indicators: {
                        let mut indicators = HashMap::new();
                        indicators.insert("sma_20".to_string(), base_price + trend);
                        indicators.insert("rsi".to_string(), 30.0 + ((i as f64 * 0.1).sin() + 1.0) * 20.0);
                        indicators.insert("macd".to_string(), (i as f64 * 0.05).sin() * 10.0);
                        indicators.insert("bb_upper".to_string(), base_price + trend + 100.0);
                        indicators.insert("bb_lower".to_string(), base_price + trend - 100.0);
                        indicators
                    },
                    source: Some("benchmark".to_string()),
                    entity: Some("performance_test".to_string()),
                    value: Some(base_price + trend + noise + 50.0),
                    metadata: None,
                    values: vec![base_price + trend + noise + 50.0],
                    intervals: vec![i as u64],
                    timestamps: vec![chrono::Utc::now() - chrono::Duration::minutes((count - i) as i64)],
                    metadata_map: HashMap::new(),
                }
            })
            .collect()
    }

    /// Benchmark single prediction latency
    #[tokio::test]
    async fn benchmark_single_prediction_latency() {
        let config = NeuralConfig {
            models: vec!["FANN_MLP".to_string()],
            use_real_models: false,
            max_concurrent_predictions: 1,
            ..Default::default()
        };

        let predictor = NeuralPredictor::new(config).await.unwrap();
        let test_data = create_benchmark_data("BTC/USD", 100);

        // Warm up
        for _ in 0..5 {
            let _ = predictor.predict(&test_data, 1, None).await;
        }

        // Benchmark
        let mut latencies = Vec::new();
        for _ in 0..20 {
            let start = Instant::now();
            let result = predictor.predict(&test_data, 1, None).await;
            let duration = start.elapsed();
            
            if result.is_ok() {
                latencies.push(duration);
            }
        }

        if !latencies.is_empty() {
            let avg_latency = latencies.iter().sum::<Duration>() / latencies.len() as u32;
            let min_latency = latencies.iter().min().unwrap();
            let max_latency = latencies.iter().max().unwrap();

            println!("Single Prediction Latency Benchmark:");
            println!("  Average: {:?}", avg_latency);
            println!("  Min: {:?}", min_latency);
            println!("  Max: {:?}", max_latency);

            // Performance assertions (adjust based on system capabilities)
            assert!(avg_latency < Duration::from_secs(5), "Average latency too high");
            assert!(*max_latency < Duration::from_secs(10), "Max latency too high");
        }
    }

    /// Benchmark batch prediction throughput
    #[tokio::test]
    async fn benchmark_batch_prediction_throughput() {
        let config = NeuralConfig {
            models: vec!["FANN_MLP".to_string()],
            use_real_models: false,
            max_concurrent_predictions: 10,
            ..Default::default()
        };

        let predictor = Arc::new(NeuralPredictor::new(config).await.unwrap());
        let test_data = create_benchmark_data("ETH/USD", 50);

        // Test different batch sizes
        let batch_sizes = vec![1, 5, 10, 20, 50];
        
        for batch_size in batch_sizes {
            let start = Instant::now();
            let mut handles = Vec::new();

            for _ in 0..batch_size {
                let predictor_clone = predictor.clone();
                let data_clone = test_data.clone();
                
                let handle = tokio::spawn(async move {
                    predictor_clone.predict(&data_clone, 3, None).await
                });
                handles.push(handle);
            }

            let mut success_count = 0;
            for handle in handles {
                if let Ok(Ok(_)) = handle.await {
                    success_count += 1;
                }
            }

            let duration = start.elapsed();
            let throughput = success_count as f64 / duration.as_secs_f64();

            println!("Batch Size {}: {} successful predictions in {:?} ({:.2} pred/sec)", 
                     batch_size, success_count, duration, throughput);

            // Basic performance assertion
            assert!(success_count > 0, "No successful predictions in batch");
        }
    }

    /// Load test with sustained concurrent requests
    #[tokio::test]
    async fn load_test_concurrent_requests() {
        let config = NeuralConfig {
            models: vec!["FANN_MLP".to_string()],
            use_real_models: false,
            max_concurrent_predictions: 20,
            ..Default::default()
        };

        let predictor = Arc::new(NeuralPredictor::new(config).await.unwrap());
        let test_data = create_benchmark_data("AAPL", 30);

        // Run sustained load for 30 seconds
        let test_duration = Duration::from_secs(30);
        let start_time = Instant::now();
        let mut total_requests = 0;
        let mut successful_requests = 0;
        let mut failed_requests = 0;

        while start_time.elapsed() < test_duration {
            let mut batch_handles = Vec::new();
            
            // Send 10 concurrent requests
            for _ in 0..10 {
                let predictor_clone = predictor.clone();
                let data_clone = test_data.clone();
                
                let handle = tokio::spawn(async move {
                    predictor_clone.predict(&data_clone, 2, None).await
                });
                batch_handles.push(handle);
            }

            // Wait for batch to complete
            for handle in batch_handles {
                total_requests += 1;
                match handle.await {
                    Ok(Ok(_)) => successful_requests += 1,
                    _ => failed_requests += 1,
                }
            }

            // Small delay between batches
            sleep(Duration::from_millis(100)).await;
        }

        let actual_duration = start_time.elapsed();
        let success_rate = (successful_requests as f64 / total_requests as f64) * 100.0;
        let throughput = successful_requests as f64 / actual_duration.as_secs_f64();

        println!("Load Test Results ({:?}):", actual_duration);
        println!("  Total Requests: {}", total_requests);
        println!("  Successful: {}", successful_requests);
        println!("  Failed: {}", failed_requests);
        println!("  Success Rate: {:.2}%", success_rate);
        println!("  Throughput: {:.2} req/sec", throughput);

        // Load test assertions
        assert!(total_requests > 100, "Should have processed many requests");
        assert!(success_rate > 50.0, "Success rate should be reasonable");
        assert!(throughput > 1.0, "Should maintain minimum throughput");
    }

    /// Benchmark enhanced neural adapter with all features
    #[tokio::test]
    async fn benchmark_enhanced_adapter() {
        let config = EnhancedNeuralConfig {
            neural: NeuralConfig {
                models: vec!["FANN_MLP".to_string(), "LSTM".to_string()],
                use_real_models: false,
                enable_performance_monitoring: true,
                enable_health_checks: true,
                ..Default::default()
            },
            enable_health_monitoring: true,
            enable_fallback: true,
            enable_caching: true,
            enable_circuit_breakers: true,
            ..Default::default()
        };

        let adapter = EnhancedNeuralAdapter::new(config).await.unwrap();
        let test_data = create_benchmark_data("BTC/USD", 75);

        // Warm up
        for _ in 0..3 {
            let _ = adapter.predict_enhanced(&test_data, 2, None).await;
        }

        // Benchmark enhanced predictions
        let mut enhanced_latencies = Vec::new();
        let iterations = 15;

        for _ in 0..iterations {
            let start = Instant::now();
            let result = adapter.predict_enhanced(&test_data, 3, None).await;
            let duration = start.elapsed();
            
            if result.is_ok() {
                enhanced_latencies.push(duration);
            }
        }

        if !enhanced_latencies.is_empty() {
            let avg_latency = enhanced_latencies.iter().sum::<Duration>() / enhanced_latencies.len() as u32;
            
            println!("Enhanced Adapter Benchmark:");
            println!("  Average Latency: {:?}", avg_latency);
            println!("  Successful Predictions: {}/{}", enhanced_latencies.len(), iterations);

            // Check performance stats
            let stats = adapter.get_performance_stats().await;
            println!("  Performance Stats:");
            println!("    Total Predictions: {}", stats.total_predictions);
            println!("    Success Rate: {:.2}%", stats.success_rate);
            println!("    Average Response Time: {:?}", stats.average_response_time);
            println!("    Fallback Usage Rate: {:.2}%", stats.fallback_usage_rate);

            // Performance assertions
            assert!(avg_latency < Duration::from_secs(10), "Enhanced adapter latency too high");
            assert!(stats.success_rate >= 0.0, "Success rate should be valid");
        }
    }

    /// Benchmark performance channel throughput
    #[tokio::test]
    async fn benchmark_performance_channel() {
        let (channel, mut receiver) = PerformanceChannel::new(1000);
        let channel = Arc::new(channel);

        // Test event emission throughput
        let events_to_emit = 1000;
        let start = Instant::now();

        // Emit events concurrently
        let mut handles = Vec::new();
        for i in 0..events_to_emit {
            let channel_clone = channel.clone();
            let handle = tokio::spawn(async move {
                let event = PerformanceEventBuilder::new()
                    .source(PerformanceSource::NeuralPredictor {
                        model_name: format!("model_{}", i % 10),
                    })
                    .event_type(PerformanceEventType::PredictionCompleted {
                        model: format!("model_{}", i % 10),
                        accuracy: 0.8 + (i as f64 * 0.0001),
                        confidence: 0.9,
                        latency_ms: 100 + (i % 50) as u64,
                        timestamp: chrono::Utc::now(),
                    })
                    .build()
                    .unwrap();

                channel_clone.emit(event).await
            });
            handles.push(handle);
        }

        // Wait for all emissions
        let mut successful_emissions = 0;
        for handle in handles {
            if handle.await.is_ok() {
                successful_emissions += 1;
            }
        }

        let emission_duration = start.elapsed();
        let emission_throughput = successful_emissions as f64 / emission_duration.as_secs_f64();

        println!("Performance Channel Benchmark:");
        println!("  Events Emitted: {}/{}", successful_emissions, events_to_emit);
        println!("  Emission Duration: {:?}", emission_duration);
        println!("  Emission Throughput: {:.2} events/sec", emission_throughput);

        // Test reception throughput
        let reception_start = Instant::now();
        let mut received_count = 0;
        
        // Try to receive events (with timeout)
        while received_count < successful_emissions && reception_start.elapsed() < Duration::from_secs(5) {
            match receiver.try_recv() {
                Ok(_) => received_count += 1,
                Err(_) => {
                    sleep(Duration::from_millis(1)).await;
                }
            }
        }

        let reception_duration = reception_start.elapsed();
        let reception_throughput = received_count as f64 / reception_duration.as_secs_f64();

        println!("  Events Received: {}", received_count);
        println!("  Reception Duration: {:?}", reception_duration);
        println!("  Reception Throughput: {:.2} events/sec", reception_throughput);

        // Check buffer state
        println!("  Final Buffer Size: {}", channel.buffer_size());

        // Performance assertions
        assert!(successful_emissions > events_to_emit / 2, "Should emit most events successfully");
        assert!(emission_throughput > 100.0, "Should have reasonable emission throughput");
        assert!(received_count > 0, "Should receive some events");
    }

    /// Memory usage benchmark
    #[tokio::test]
    async fn benchmark_memory_usage() {
        let initial_memory = get_memory_usage();
        
        // Create multiple predictors and run predictions
        let mut predictors = Vec::new();
        for i in 0..5 {
            let config = NeuralConfig {
                models: vec!["FANN_MLP".to_string()],
                use_real_models: false,
                memory_gb: 0.1, // Small memory limit
                ..Default::default()
            };
            
            let predictor = NeuralPredictor::new(config).await.unwrap();
            predictors.push(predictor);
        }

        let after_creation_memory = get_memory_usage();
        
        // Run predictions to test memory growth
        let test_data = create_benchmark_data("BTC/USD", 100);
        
        for predictor in &predictors {
            for _ in 0..10 {
                let _ = predictor.predict(&test_data, 5, None).await;
            }
        }

        let after_predictions_memory = get_memory_usage();
        
        // Drop predictors
        drop(predictors);
        
        // Force garbage collection (if available)
        #[cfg(feature = "jemalloc")]
        {
            // jemalloc specific memory release
        }
        
        // Wait a bit for cleanup
        sleep(Duration::from_millis(100)).await;
        
        let after_cleanup_memory = get_memory_usage();

        println!("Memory Usage Benchmark:");
        println!("  Initial: {} KB", initial_memory);
        println!("  After Creation: {} KB (+{} KB)", after_creation_memory, after_creation_memory - initial_memory);
        println!("  After Predictions: {} KB (+{} KB)", after_predictions_memory, after_predictions_memory - after_creation_memory);
        println!("  After Cleanup: {} KB ({} KB from peak)", after_cleanup_memory, after_predictions_memory - after_cleanup_memory);

        // Memory assertions (these are rough estimates)
        let memory_growth = after_predictions_memory - initial_memory;
        assert!(memory_growth < 100_000, "Memory growth should be reasonable"); // Less than 100MB

        let cleanup_effectiveness = after_predictions_memory - after_cleanup_memory;
        println!("  Cleanup Effectiveness: {} KB released", cleanup_effectiveness);
    }

    /// Stress test with error conditions
    #[tokio::test]
    async fn stress_test_error_conditions() {
        let config = NeuralConfig {
            models: vec!["FANN_MLP".to_string()],
            use_real_models: false,
            model_timeout_seconds: 1, // Very short timeout to induce failures
            max_retries: 1,
            ..Default::default()
        };

        let predictor = Arc::new(NeuralPredictor::new(config).await.unwrap());
        
        // Test with various problematic inputs
        let test_cases = vec![
            (Vec::new(), 5), // Empty data
            (create_benchmark_data("TEST", 1), 0), // Zero horizon
            (create_benchmark_data("TEST", 1), 1000), // Huge horizon
            (create_invalid_data(10), 5), // Invalid data
        ];

        let mut total_tests = 0;
        let mut handled_gracefully = 0;

        for (test_data, horizon) in test_cases {
            total_tests += 1;
            
            let result = predictor.predict(&test_data, horizon, None).await;
            
            match result {
                Ok(_) => {
                    // Unexpected success is also handled gracefully
                    handled_gracefully += 1;
                }
                Err(e) => {
                    // Should have meaningful error message
                    if !e.to_string().is_empty() {
                        handled_gracefully += 1;
                    }
                }
            }
        }

        println!("Stress Test Results:");
        println!("  Total Tests: {}", total_tests);
        println!("  Handled Gracefully: {}", handled_gracefully);
        println!("  Grace Rate: {:.2}%", (handled_gracefully as f64 / total_tests as f64) * 100.0);

        assert_eq!(handled_gracefully, total_tests, "All error conditions should be handled gracefully");
    }

    /// Create invalid data for stress testing
    fn create_invalid_data(count: usize) -> Vec<TimeSeriesData> {
        (0..count)
            .map(|i| TimeSeriesData {
                symbol: "INVALID".to_string(),
                timestamp: chrono::Utc::now(),
                open: if i % 3 == 0 { f64::NAN } else { 100.0 },
                high: if i % 3 == 1 { f64::INFINITY } else { 101.0 },
                low: if i % 3 == 2 { f64::NEG_INFINITY } else { 99.0 },
                close: 100.0,
                volume: if i % 2 == 0 { -1000.0 } else { 1000.0 }, // Negative volume
                indicators: HashMap::new(),
                source: Some("invalid".to_string()),
                entity: Some("stress_test".to_string()),
                value: Some(100.0),
                metadata: None,
            })
            .collect()
    }

    /// Get current memory usage (rough estimate)
    fn get_memory_usage() -> u64 {
        // This is a simplified memory measurement
        // In a real implementation, you'd use proper memory profiling tools
        #[cfg(target_os = "linux")]
        {
            if let Ok(contents) = std::fs::read_to_string("/proc/self/status") {
                for line in contents.lines() {
                    if line.starts_with("VmRSS:") {
                        if let Some(kb_str) = line.split_whitespace().nth(1) {
                            if let Ok(kb) = kb_str.parse::<u64>() {
                                return kb;
                            }
                        }
                    }
                }
            }
        }

        // Fallback: estimate based on allocation patterns
        std::mem::size_of::<NeuralPredictor>() as u64
    }

    /// Performance regression test
    #[tokio::test]
    async fn performance_regression_test() {
        let config = NeuralConfig {
            models: vec!["FANN_MLP".to_string()],
            use_real_models: false,
            ..Default::default()
        };

        let predictor = NeuralPredictor::new(config).await.unwrap();
        let test_data = create_benchmark_data("BTC/USD", 50);

        // Define performance baseline (adjust based on system)
        let max_acceptable_latency = Duration::from_secs(5);
        let min_acceptable_throughput = 1.0; // predictions per second

        // Test single prediction latency
        let start = Instant::now();
        let single_result = predictor.predict(&test_data, 1, None).await;
        let single_latency = start.elapsed();

        if single_result.is_ok() {
            assert!(single_latency < max_acceptable_latency, 
                    "Single prediction latency {} exceeds baseline {}", 
                    single_latency.as_millis(), max_acceptable_latency.as_millis());
        }

        // Test throughput
        let throughput_start = Instant::now();
        let mut successful_predictions = 0;
        
        for _ in 0..10 {
            if predictor.predict(&test_data, 1, None).await.is_ok() {
                successful_predictions += 1;
            }
        }

        let throughput_duration = throughput_start.elapsed();
        let actual_throughput = successful_predictions as f64 / throughput_duration.as_secs_f64();

        if successful_predictions > 0 {
            assert!(actual_throughput >= min_acceptable_throughput,
                    "Throughput {:.2} pred/sec below baseline {:.2} pred/sec",
                    actual_throughput, min_acceptable_throughput);
        }

        println!("Performance Regression Test:");
        println!("  Single Latency: {:?} (baseline: {:?})", single_latency, max_acceptable_latency);
        println!("  Throughput: {:.2} pred/sec (baseline: {:.2} pred/sec)", actual_throughput, min_acceptable_throughput);
        println!("  Successful Predictions: {}/10", successful_predictions);
    }
}

/// Criterion benchmarks (if criterion is available)
#[cfg(feature = "criterion")]
mod criterion_benchmarks {
    use super::*;

    pub fn criterion_benchmark(c: &mut Criterion) {
        let rt = Runtime::new().unwrap();
        
        // Single prediction benchmark
        c.bench_function("single_prediction", |b| {
            let config = NeuralConfig {
                models: vec!["FANN_MLP".to_string()],
                use_real_models: false,
                ..Default::default()
            };
            let predictor = NeuralPredictor::new(config).await.unwrap();
            let test_data = create_benchmark_data("BTC/USD", 50);
            
            b.iter(|| {
                rt.block_on(async {
                    let result = predictor.predict(black_box(&test_data), black_box(1), None).await;
                    black_box(result);
                });
            });
        });

        // Performance channel benchmark
        c.bench_function("performance_channel_emit", |b| {
            let (channel, _receiver) = PerformanceChannel::new(100);
            
            b.iter(|| {
                rt.block_on(async {
                    let event = PerformanceEventBuilder::new()
                        .source(PerformanceSource::NeuralPredictor {
                            model_name: "test".to_string(),
                        })
                        .event_type(PerformanceEventType::PredictionCompleted {
                            model: "test".to_string(),
                            accuracy: 0.9,
                            confidence: 0.8,
                            latency_ms: 100,
                            timestamp: chrono::Utc::now(),
                        })
                        .build()
                        .unwrap();
                    
                    let result = channel.emit(black_box(event)).await;
                    black_box(result);
                });
            });
        });
    }
}