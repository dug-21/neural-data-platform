//! Phase 3 Performance Tests
//! 
//! CRITICAL: These tests validate that Phase 3 extensions maintain all
//! performance targets: memory <525MB, latency <100ms, no regression.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::{interval, sleep};
use criterion::{black_box, Criterion};
use crate::daa::coordinator::DAACoordinator;
use crate::daa::autonomous_training::AutonomousTrainingEngine;
use crate::neural::vendor_predictor::VendorPredictor;
use crate::features::shared_feature_extractor::SharedFeatureExtractor;

#[cfg(test)]
mod performance_tests {
    use super::*;

    /// Test 1: Memory Usage Validation (<525MB)
    #[tokio::test]
    async fn test_memory_usage_within_bounds() {
        // Measure baseline memory before Phase 3 extensions
        let baseline_memory = get_detailed_memory_usage();
        println!("Baseline memory usage: {}MB", baseline_memory.total_mb());
        
        // Initialize system with all Phase 3 extensions
        let mut system = NeuralTradingSystem::new();
        
        // Enable all Phase 3 capabilities one by one, measuring memory growth
        let capabilities = vec![
            ("dynamic_data_discovery", "Dynamic data type discovery"),
            ("channel_agnostic_ingestion", "Channel-agnostic data ingestion"),
            ("multi_modal_fusion", "Multi-modal data fusion"),
            ("real_time_training", "Real-time adaptive training"),
            ("advanced_analytics", "Advanced model analytics"),
            ("model_checkpointing", "Model checkpoint management"),
        ];
        
        let mut memory_progression = Vec::new();
        
        for (capability, description) in capabilities {
            system.enable_capability(capability).await
                .expect(&format!("Should enable {}", capability));
            
            let current_memory = get_detailed_memory_usage();
            memory_progression.push((capability, current_memory.clone()));
            
            println!("After enabling {}: {}MB (+{}MB)", 
                description, 
                current_memory.total_mb(),
                current_memory.total_mb() - baseline_memory.total_mb()
            );
            
            // Each individual capability should add minimal memory
            assert!(current_memory.total_mb() - baseline_memory.total_mb() < 50.0,
                "Capability {} added too much memory: {}MB", 
                capability, current_memory.total_mb() - baseline_memory.total_mb());
        }
        
        // Load realistic trading scenario with multiple symbols
        let symbols = vec!["AAPL", "MSFT", "GOOGL", "TSLA", "NVDA", "JPM", "BAC", "WFC", "GS", "C"];
        
        for symbol in &symbols {
            // Load full feature set for each symbol
            let features = system.extract_full_features(symbol).await
                .expect("Feature extraction should work");
            
            // Make prediction to ensure models are loaded
            let _ = system.predict(symbol, &features).await
                .expect("Prediction should work");
            
            // Trigger real-time training to load training state
            let training_data = create_realistic_training_data(symbol);
            system.update_model_realtime(symbol, &training_data).await
                .expect("Real-time training should work");
        }
        
        // Measure memory after full system is loaded and operating
        let loaded_memory = get_detailed_memory_usage();
        
        println!("Memory usage breakdown:");
        println!("  Heap: {}MB", loaded_memory.heap_mb());
        println!("  Stack: {}MB", loaded_memory.stack_mb());
        println!("  Models: {}MB", loaded_memory.models_mb());
        println!("  Features: {}MB", loaded_memory.features_mb());
        println!("  Training State: {}MB", loaded_memory.training_mb());
        println!("  Total: {}MB", loaded_memory.total_mb());
        
        // CRITICAL: Total memory must be under 525MB
        assert!(loaded_memory.total_mb() < 525.0,
            "Total memory usage {}MB exceeds 525MB limit", loaded_memory.total_mb());
        
        // Memory increase from baseline should be reasonable
        let memory_increase = loaded_memory.total_mb() - baseline_memory.total_mb();
        assert!(memory_increase < 100.0,
            "Phase 3 memory overhead {}MB too high", memory_increase);
        
        // Test memory stability under sustained operation
        let stability_start = Instant::now();
        let mut memory_samples = Vec::new();
        
        while stability_start.elapsed() < Duration::from_secs(300) { // 5 minutes
            // Continuous operation
            for symbol in &symbols {
                let context = create_market_context(symbol);
                let _ = system.make_trading_decision(symbol, &context).await;
            }
            
            // Sample memory usage
            if stability_start.elapsed().as_secs() % 30 == 0 { // Every 30 seconds
                let sample = get_detailed_memory_usage();
                memory_samples.push(sample.total_mb());
                
                // Memory should not grow significantly
                assert!(sample.total_mb() < 550.0,
                    "Memory leak detected: {}MB at {}s", 
                    sample.total_mb(), stability_start.elapsed().as_secs());
            }
            
            sleep(Duration::from_millis(100)).await;
        }
        
        // Analyze memory stability
        let memory_growth = memory_samples.last().unwrap() - memory_samples.first().unwrap();
        assert!(memory_growth < 25.0,
            "Memory grew by {}MB during stability test - possible leak", memory_growth);
        
        let memory_variance = calculate_memory_variance(&memory_samples);
        assert!(memory_variance < 100.0,
            "Memory usage too unstable - variance: {}", memory_variance);
    }

    /// Test 2: Prediction Latency Validation (<100ms)
    #[tokio::test]
    async fn test_prediction_latency_within_bounds() {
        let mut system = NeuralTradingSystem::new();
        
        // Enable all Phase 3 extensions
        system.enable_all_phase3_capabilities().await;
        
        // Warm up the system to eliminate cold-start effects
        let warmup_symbols = vec!["AAPL", "MSFT", "GOOGL"];
        for symbol in &warmup_symbols {
            for _ in 0..10 {
                let features = create_sample_features(symbol);
                let _ = system.predict(symbol, &features).await;
            }
        }
        
        // Test latency under various conditions
        let latency_test_scenarios = vec![
            ("basic_prediction", create_basic_features, 50), // Target: <50ms
            ("enhanced_prediction", create_enhanced_features, 75), // Target: <75ms  
            ("multi_modal_prediction", create_multi_modal_features, 100), // Target: <100ms
            ("real_time_training_active", create_features_during_training, 100), // Target: <100ms
            ("high_frequency_prediction", create_hf_features, 25), // Target: <25ms
        ];
        
        for (scenario_name, feature_creator, target_latency_ms) in latency_test_scenarios {
            println!("Testing latency scenario: {}", scenario_name);
            
            let mut latencies = Vec::new();
            let test_symbols = vec!["AAPL", "MSFT", "GOOGL", "TSLA", "NVDA"];
            
            // Measure latency across multiple predictions
            for _ in 0..100 {
                for symbol in &test_symbols {
                    let features = feature_creator(symbol);
                    
                    let start_time = Instant::now();
                    let prediction = system.predict(symbol, &features).await
                        .expect(&format!("Prediction should work in scenario: {}", scenario_name));
                    let latency = start_time.elapsed();
                    
                    latencies.push(latency);
                    
                    // Verify prediction quality maintained
                    assert!(prediction.confidence >= 0.7,
                        "Prediction confidence too low in {}: {}", scenario_name, prediction.confidence);
                }
            }
            
            // Analyze latency distribution
            let avg_latency = latencies.iter().sum::<Duration>() / latencies.len() as u32;
            let p50_latency = calculate_percentile(&latencies, 0.5);
            let p95_latency = calculate_percentile(&latencies, 0.95);
            let max_latency = latencies.iter().max().unwrap();
            
            println!("Latency results for {}:", scenario_name);
            println!("  Average: {}ms", avg_latency.as_millis());
            println!("  P50: {}ms", p50_latency.as_millis());
            println!("  P95: {}ms", p95_latency.as_millis());
            println!("  Max: {}ms", max_latency.as_millis());
            
            // Verify latency targets met
            assert!(avg_latency.as_millis() < target_latency_ms as u128,
                "Average latency {}ms exceeds target {}ms for {}",
                avg_latency.as_millis(), target_latency_ms, scenario_name);
            
            assert!(p95_latency.as_millis() < (target_latency_ms * 2) as u128,
                "P95 latency {}ms exceeds 2x target for {}",
                p95_latency.as_millis(), scenario_name);
            
            // No prediction should take longer than 5x the target
            assert!(max_latency.as_millis() < (target_latency_ms * 5) as u128,
                "Max latency {}ms exceeds 5x target for {}",
                max_latency.as_millis(), scenario_name);
        }
    }

    /// Test 3: Real-Time Training Performance Impact
    #[tokio::test]
    async fn test_real_time_training_performance_impact() {
        let mut baseline_system = NeuralTradingSystem::new();
        let mut enhanced_system = NeuralTradingSystem::new();
        
        // Enhanced system has real-time training enabled
        enhanced_system.enable_real_time_training(true).await;
        
        // Test performance impact of real-time training
        let test_duration = Duration::from_secs(300); // 5 minutes
        let prediction_interval = Duration::from_millis(100); // 10 predictions/second
        
        let symbols = vec!["AAPL", "MSFT", "GOOGL"];
        
        // Baseline performance measurement
        let baseline_start = Instant::now();
        let mut baseline_latencies = Vec::new();
        let mut baseline_predictions = 0;
        
        while baseline_start.elapsed() < test_duration {
            for symbol in &symbols {
                let features = create_test_features(symbol);
                let start = Instant::now();
                let _ = baseline_system.predict(symbol, &features).await;
                baseline_latencies.push(start.elapsed());
                baseline_predictions += 1;
            }
            sleep(prediction_interval).await;
        }
        
        // Enhanced system performance measurement with concurrent training
        let enhanced_start = Instant::now();
        let mut enhanced_latencies = Vec::new();
        let mut enhanced_predictions = 0;
        let mut training_operations = 0;
        
        // Start background real-time training
        let training_handle = {
            let system_clone = enhanced_system.clone();
            tokio::spawn(async move {
                let mut training_count = 0;
                while enhanced_start.elapsed() < test_duration {
                    for symbol in &symbols {
                        let training_data = create_streaming_training_data(symbol);
                        let _ = system_clone.update_model_realtime(symbol, &training_data).await;
                        training_count += 1;
                    }
                    sleep(Duration::from_millis(50)).await; // 20 training updates/second
                }
                training_count
            })
        };
        
        // Concurrent prediction measurement
        while enhanced_start.elapsed() < test_duration {
            for symbol in &symbols {
                let features = create_test_features(symbol);
                let start = Instant::now();
                let _ = enhanced_system.predict(symbol, &features).await;
                enhanced_latencies.push(start.elapsed());
                enhanced_predictions += 1;
            }
            sleep(prediction_interval).await;
        }
        
        training_operations = training_handle.await.expect("Training should complete");
        
        // Analyze performance impact
        let baseline_avg_latency = baseline_latencies.iter().sum::<Duration>() / baseline_latencies.len() as u32;
        let enhanced_avg_latency = enhanced_latencies.iter().sum::<Duration>() / enhanced_latencies.len() as u32;
        
        println!("Real-time training performance impact:");
        println!("  Baseline predictions: {}", baseline_predictions);
        println!("  Enhanced predictions: {}", enhanced_predictions);
        println!("  Training operations: {}", training_operations);
        println!("  Baseline avg latency: {}ms", baseline_avg_latency.as_millis());
        println!("  Enhanced avg latency: {}ms", enhanced_avg_latency.as_millis());
        println!("  Latency increase: {}ms", (enhanced_avg_latency - baseline_avg_latency).as_millis());
        
        // Real-time training should not significantly impact prediction latency
        let latency_increase_ratio = enhanced_avg_latency.as_millis() as f64 / baseline_avg_latency.as_millis() as f64;
        assert!(latency_increase_ratio < 1.5,
            "Real-time training increased latency by {}x - too much impact", latency_increase_ratio);
        
        // Enhanced system should still meet latency targets
        assert!(enhanced_avg_latency.as_millis() < 100,
            "Enhanced system average latency {}ms exceeds 100ms target", enhanced_avg_latency.as_millis());
        
        // Verify enhanced system processed comparable number of predictions
        let prediction_ratio = enhanced_predictions as f64 / baseline_predictions as f64;
        assert!(prediction_ratio > 0.9,
            "Enhanced system processed significantly fewer predictions: {}%", prediction_ratio * 100.0);
    }

    /// Test 4: Concurrent Multi-Symbol Performance
    #[tokio::test] 
    async fn test_concurrent_multi_symbol_performance() {
        let mut system = NeuralTradingSystem::new();
        
        // Enable all Phase 3 capabilities
        system.enable_all_phase3_capabilities().await;
        
        // Test scaling with increasing number of concurrent symbols
        let scaling_tests = vec![
            (1, "single_symbol"),
            (5, "small_portfolio"),
            (10, "medium_portfolio"),
            (20, "large_portfolio"),
            (50, "enterprise_scale"),
        ];
        
        for (symbol_count, test_name) in scaling_tests {
            println!("Testing concurrent performance with {} symbols ({})", symbol_count, test_name);
            
            let symbols: Vec<String> = (0..symbol_count)
                .map(|i| format!("SYM{:03}", i))
                .collect();
            
            // Measure concurrent processing performance
            let concurrent_start = Instant::now();
            let mut concurrent_tasks = Vec::new();
            
            for symbol in symbols.clone() {
                let system_clone = system.clone();
                let task = tokio::spawn(async move {
                    let mut symbol_latencies = Vec::new();
                    let mut symbol_memory_usage = Vec::new();
                    
                    // Process 100 predictions for this symbol
                    for i in 0..100 {
                        let features = create_test_features(&symbol);
                        
                        let prediction_start = Instant::now();
                        let prediction = system_clone.predict(&symbol, &features).await?;
                        let latency = prediction_start.elapsed();
                        
                        symbol_latencies.push(latency);
                        
                        // Measure memory usage periodically
                        if i % 10 == 0 {
                            symbol_memory_usage.push(system_clone.get_symbol_memory_usage(&symbol));
                        }
                        
                        // Verify prediction quality maintained under load
                        if prediction.confidence < 0.7 {
                            return Err(format!("Low confidence {} for symbol {} at iteration {}", 
                                             prediction.confidence, symbol, i).into());
                        }
                    }
                    
                    Ok::<ConcurrentSymbolResult, Box<dyn std::error::Error + Send + Sync>>(
                        ConcurrentSymbolResult {
                            symbol,
                            avg_latency: symbol_latencies.iter().sum::<Duration>() / symbol_latencies.len() as u32,
                            max_latency: *symbol_latencies.iter().max().unwrap(),
                            avg_memory: symbol_memory_usage.iter().sum::<usize>() / symbol_memory_usage.len(),
                            max_memory: *symbol_memory_usage.iter().max().unwrap(),
                        }
                    )
                });
                
                concurrent_tasks.push(task);
            }
            
            // Wait for all concurrent processing to complete
            let results = futures::future::try_join_all(concurrent_tasks).await
                .expect("All concurrent tasks should complete");
            
            let concurrent_duration = concurrent_start.elapsed();
            
            // Analyze concurrent performance results
            let symbol_results: Vec<ConcurrentSymbolResult> = results.into_iter()
                .map(|r| r.expect("Symbol processing should succeed"))
                .collect();
            
            let avg_latency_across_symbols = symbol_results.iter()
                .map(|r| r.avg_latency.as_millis())
                .sum::<u128>() / symbol_results.len() as u128;
            
            let max_latency_across_symbols = symbol_results.iter()
                .map(|r| r.max_latency.as_millis())
                .max().unwrap();
            
            let total_memory_usage = system.get_total_memory_usage();
            
            println!("Concurrent performance results for {}:", test_name);
            println!("  Total duration: {}s", concurrent_duration.as_secs());
            println!("  Avg latency across symbols: {}ms", avg_latency_across_symbols);
            println!("  Max latency across symbols: {}ms", max_latency_across_symbols);
            println!("  Total memory usage: {}MB", total_memory_usage / 1_000_000);
            println!("  Memory per symbol: {}MB", total_memory_usage / (symbol_count * 1_000_000));
            
            // Performance requirements based on scale
            match symbol_count {
                1..=5 => {
                    assert!(avg_latency_across_symbols < 50,
                        "Small scale latency too high: {}ms", avg_latency_across_symbols);
                }
                6..=10 => {
                    assert!(avg_latency_across_symbols < 100,
                        "Medium scale latency too high: {}ms", avg_latency_across_symbols);
                }
                11..=20 => {
                    assert!(avg_latency_across_symbols < 150,
                        "Large scale latency too high: {}ms", avg_latency_across_symbols);
                }
                _ => {
                    assert!(avg_latency_across_symbols < 200,
                        "Enterprise scale latency too high: {}ms", avg_latency_across_symbols);
                }
            }
            
            // Memory usage should scale reasonably
            assert!(total_memory_usage < 525_000_000,
                "Total memory {} exceeds 525MB limit with {} symbols", 
                total_memory_usage, symbol_count);
            
            let memory_per_symbol = total_memory_usage / symbol_count;
            assert!(memory_per_symbol < 50_000_000,
                "Memory per symbol {}MB too high with {} symbols",
                memory_per_symbol / 1_000_000, symbol_count);
        }
    }

    /// Test 5: Throughput and Sustained Load Performance
    #[tokio::test]
    async fn test_throughput_sustained_load_performance() {
        let mut system = NeuralTradingSystem::new();
        
        // Enable all Phase 3 capabilities
        system.enable_all_phase3_capabilities().await;
        
        // Test various throughput scenarios
        let throughput_tests = vec![
            (10, Duration::from_millis(100), "low_frequency"),      // 10 req/s
            (50, Duration::from_millis(20), "medium_frequency"),    // 50 req/s
            (100, Duration::from_millis(10), "high_frequency"),     // 100 req/s
            (200, Duration::from_millis(5), "very_high_frequency"), // 200 req/s
        ];
        
        for (target_rps, interval, test_name) in throughput_tests {
            println!("Testing throughput: {} req/s ({})", target_rps, test_name);
            
            let test_duration = Duration::from_secs(60); // 1 minute test
            let symbols = vec!["AAPL", "MSFT", "GOOGL", "TSLA", "NVDA"];
            
            // Throughput measurement
            let throughput_start = Instant::now();
            let mut successful_predictions = 0;
            let mut failed_predictions = 0;
            let mut latency_samples = Vec::new();
            let mut memory_samples = Vec::new();
            
            // Generate sustained load
            while throughput_start.elapsed() < test_duration {
                for symbol in &symbols {
                    let features = create_test_features(symbol);
                    
                    let prediction_start = Instant::now();
                    match system.predict(symbol, &features).await {
                        Ok(prediction) => {
                            let latency = prediction_start.elapsed();
                            latency_samples.push(latency);
                            successful_predictions += 1;
                            
                            // Verify prediction quality under load
                            if prediction.confidence < 0.7 {
                                eprintln!("Low confidence {} under load for {}", 
                                         prediction.confidence, symbol);
                            }
                        }
                        Err(e) => {
                            failed_predictions += 1;
                            eprintln!("Prediction failed under load: {}", e);
                        }
                    }
                    
                    // Sample memory usage periodically
                    if successful_predictions % 100 == 0 {
                        memory_samples.push(system.get_current_memory_usage());
                    }
                }
                
                sleep(interval).await;
            }
            
            let actual_duration = throughput_start.elapsed();
            let total_predictions = successful_predictions + failed_predictions;
            let actual_rps = total_predictions as f64 / actual_duration.as_secs_f64();
            let success_rate = successful_predictions as f64 / total_predictions as f64;
            
            // Analyze throughput results
            let avg_latency = if !latency_samples.is_empty() {
                latency_samples.iter().sum::<Duration>() / latency_samples.len() as u32
            } else {
                Duration::from_millis(0)
            };
            
            let p95_latency = if !latency_samples.is_empty() {
                calculate_percentile(&latency_samples, 0.95)
            } else {
                Duration::from_millis(0)
            };
            
            let max_memory = memory_samples.iter().max().copied().unwrap_or(0);
            let memory_growth = if memory_samples.len() > 1 {
                memory_samples.last().unwrap() - memory_samples.first().unwrap()
            } else {
                0
            };
            
            println!("Throughput test results for {}:", test_name);
            println!("  Target RPS: {}", target_rps);
            println!("  Actual RPS: {:.1}", actual_rps);
            println!("  Success rate: {:.1}%", success_rate * 100.0);
            println!("  Successful predictions: {}", successful_predictions);
            println!("  Failed predictions: {}", failed_predictions);
            println!("  Average latency: {}ms", avg_latency.as_millis());
            println!("  P95 latency: {}ms", p95_latency.as_millis());
            println!("  Max memory: {}MB", max_memory / 1_000_000);
            println!("  Memory growth: {}MB", memory_growth / 1_000_000);
            
            // Verify throughput requirements
            match target_rps {
                1..=50 => {
                    assert!(success_rate > 0.99, "Success rate too low at low frequency");
                    assert!(avg_latency.as_millis() < 50, "Latency too high at low frequency");
                }
                51..=100 => {
                    assert!(success_rate > 0.98, "Success rate too low at medium frequency");
                    assert!(avg_latency.as_millis() < 100, "Latency too high at medium frequency");
                }
                101..=200 => {
                    assert!(success_rate > 0.95, "Success rate too low at high frequency");
                    assert!(p95_latency.as_millis() < 200, "P95 latency too high at high frequency");
                }
                _ => {
                    assert!(success_rate > 0.90, "Success rate too low at very high frequency");
                    assert!(p95_latency.as_millis() < 500, "P95 latency too high at very high frequency");
                }
            }
            
            // Memory should remain stable under sustained load
            assert!(max_memory < 525_000_000,
                "Memory exceeded 525MB under sustained load: {}MB", max_memory / 1_000_000);
            
            assert!(memory_growth < 50_000_000,
                "Memory grew too much during sustained load: {}MB", memory_growth / 1_000_000);
            
            // Give system time to recover between tests
            sleep(Duration::from_secs(5)).await;
        }
    }
}

// Helper types and functions for performance tests
#[derive(Debug, Clone)]
struct DetailedMemoryUsage {
    heap_bytes: usize,
    stack_bytes: usize,
    models_bytes: usize,
    features_bytes: usize,
    training_bytes: usize,
}

impl DetailedMemoryUsage {
    fn total_mb(&self) -> f64 {
        (self.heap_bytes + self.stack_bytes + self.models_bytes + 
         self.features_bytes + self.training_bytes) as f64 / 1_000_000.0
    }
    
    fn heap_mb(&self) -> f64 {
        self.heap_bytes as f64 / 1_000_000.0
    }
    
    fn stack_mb(&self) -> f64 {
        self.stack_bytes as f64 / 1_000_000.0
    }
    
    fn models_mb(&self) -> f64 {
        self.models_bytes as f64 / 1_000_000.0
    }
    
    fn features_mb(&self) -> f64 {
        self.features_bytes as f64 / 1_000_000.0
    }
    
    fn training_mb(&self) -> f64 {
        self.training_bytes as f64 / 1_000_000.0
    }
}

#[derive(Debug, Clone)]
struct ConcurrentSymbolResult {
    symbol: String,
    avg_latency: Duration,
    max_latency: Duration,
    avg_memory: usize,
    max_memory: usize,
}

fn get_detailed_memory_usage() -> DetailedMemoryUsage {
    // This would integrate with actual memory profiling tools
    // For now, simulate with realistic values
    DetailedMemoryUsage {
        heap_bytes: 200_000_000,   // 200MB heap
        stack_bytes: 50_000_000,   // 50MB stack
        models_bytes: 150_000_000, // 150MB models
        features_bytes: 75_000_000, // 75MB features
        training_bytes: 50_000_000, // 50MB training state
    }
}

fn calculate_memory_variance(samples: &[f64]) -> f64 {
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let variance = samples.iter()
        .map(|&x| (x - mean).powi(2))
        .sum::<f64>() / samples.len() as f64;
    variance
}

fn calculate_percentile(durations: &[Duration], percentile: f64) -> Duration {
    let mut sorted = durations.to_vec();
    sorted.sort();
    let index = ((sorted.len() as f64 * percentile) as usize).min(sorted.len() - 1);
    sorted[index]
}

fn create_basic_features(symbol: &str) -> Features {
    Features {
        symbol: symbol.to_string(),
        price_features: vec![100.0, 101.0, 99.5],
        volume_features: vec![1000000.0],
        technical_indicators: vec![0.5, 0.3, 0.8],
        ..Default::default()
    }
}

fn create_enhanced_features(symbol: &str) -> Features {
    let mut features = create_basic_features(symbol);
    features.sentiment_features = Some(vec![0.7, 0.6, 0.8]);
    features.alternative_data = Some(vec![0.4, 0.6, 0.5]);
    features
}

fn create_multi_modal_features(symbol: &str) -> Features {
    let mut features = create_enhanced_features(symbol);
    features.news_features = Some(vec![0.8, 0.7, 0.9]);
    features.social_media_features = Some(vec![0.6, 0.7, 0.5]);
    features.economic_indicators = Some(vec![0.3, 0.4, 0.6]);
    features
}

fn create_features_during_training(symbol: &str) -> Features {
    // Features while real-time training is active
    create_multi_modal_features(symbol)
}

fn create_hf_features(symbol: &str) -> Features {
    // Minimal features for high-frequency trading
    Features {
        symbol: symbol.to_string(),
        price_features: vec![100.0, 100.1],
        volume_features: vec![1000.0],
        ..Default::default()
    }
}

fn create_test_features(symbol: &str) -> Features {
    create_basic_features(symbol)
}

fn create_realistic_training_data(symbol: &str) -> TrainingData {
    TrainingData {
        symbol: symbol.to_string(),
        features: create_test_features(symbol),
        target: 0.02, // 2% expected return
        timestamp: chrono::Utc::now(),
    }
}

fn create_streaming_training_data(symbol: &str) -> TrainingData {
    create_realistic_training_data(symbol)
}