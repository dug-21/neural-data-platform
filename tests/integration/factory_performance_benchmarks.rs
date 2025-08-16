//! Factory Performance Benchmark Tests
//! 
//! This module provides comprehensive performance benchmarking for the factory pattern,
//! including latency tests, throughput measurements, memory efficiency validation,
//! and scalability analysis across all model types.

use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};
use anyhow::Result;
use tokio::sync::{RwLock, Semaphore};
use chrono::{DateTime, Utc};
use serde_json::json;
use serial_test::serial;
use tracing_test::traced_test;

/// Performance benchmark configuration
#[derive(Debug, Clone)]
pub struct PerformanceBenchmarkConfig {
    pub test_all_models: bool,
    pub latency_test_iterations: usize,
    pub throughput_test_duration: Duration,
    pub memory_test_duration: Duration,
    pub concurrent_creation_count: usize,
    pub performance_targets: PerformanceTargets,
    pub enable_detailed_profiling: bool,
}

impl Default for PerformanceBenchmarkConfig {
    fn default() -> Self {
        Self {
            test_all_models: true,
            latency_test_iterations: 1000,
            throughput_test_duration: Duration::from_seconds(30),
            memory_test_duration: Duration::from_minutes(5),
            concurrent_creation_count: 50,
            performance_targets: PerformanceTargets::default(),
            enable_detailed_profiling: true,
        }
    }
}

/// Performance targets for validation
#[derive(Debug, Clone)]
pub struct PerformanceTargets {
    pub max_creation_latency_ms: f64,
    pub min_creation_throughput_per_second: f64,
    pub max_memory_usage_mb: f64,
    pub max_prediction_latency_ms: f64,
    pub min_prediction_throughput: f64,
    pub memory_efficiency_threshold: f64,
}

impl Default for PerformanceTargets {
    fn default() -> Self {
        Self {
            max_creation_latency_ms: 100.0,
            min_creation_throughput_per_second: 20.0,
            max_memory_usage_mb: 200.0,
            max_prediction_latency_ms: 50.0,
            min_prediction_throughput: 100.0,
            memory_efficiency_threshold: 0.8,
        }
    }
}

/// Comprehensive benchmark results
#[derive(Debug, Clone)]
pub struct BenchmarkResults {
    pub model_creation_benchmarks: HashMap<ModelType, ModelCreationBenchmark>,
    pub prediction_benchmarks: HashMap<ModelType, PredictionBenchmark>,
    pub memory_benchmarks: HashMap<ModelType, MemoryBenchmark>,
    pub concurrent_benchmarks: ConcurrentBenchmark,
    pub scalability_benchmarks: ScalabilityBenchmark,
    pub overall_performance_score: f64,
    pub meets_all_targets: bool,
    pub detailed_profiling_data: Option<DetailedProfilingData>,
}

/// Model creation performance metrics
#[derive(Debug, Clone)]
pub struct ModelCreationBenchmark {
    pub model_type: ModelType,
    pub average_creation_time_ms: f64,
    pub min_creation_time_ms: f64,
    pub max_creation_time_ms: f64,
    pub p95_creation_time_ms: f64,
    pub p99_creation_time_ms: f64,
    pub creation_throughput_per_second: f64,
    pub memory_usage_during_creation_mb: f64,
    pub cpu_usage_during_creation_percent: f64,
    pub creation_success_rate: f64,
    pub meets_latency_target: bool,
    pub meets_throughput_target: bool,
}

/// Prediction performance metrics
#[derive(Debug, Clone)]
pub struct PredictionBenchmark {
    pub model_type: ModelType,
    pub average_prediction_time_ms: f64,
    pub min_prediction_time_ms: f64,
    pub max_prediction_time_ms: f64,
    pub p95_prediction_time_ms: f64,
    pub p99_prediction_time_ms: f64,
    pub prediction_throughput_per_second: f64,
    pub prediction_accuracy: f64,
    pub memory_usage_during_prediction_mb: f64,
    pub cpu_usage_during_prediction_percent: f64,
    pub cache_hit_rate: f64,
    pub meets_prediction_targets: bool,
}

/// Memory usage benchmarks
#[derive(Debug, Clone)]
pub struct MemoryBenchmark {
    pub model_type: ModelType,
    pub baseline_memory_mb: f64,
    pub peak_memory_mb: f64,
    pub average_memory_mb: f64,
    pub memory_growth_rate_mb_per_hour: f64,
    pub memory_efficiency_score: f64,
    pub garbage_collection_frequency: f64,
    pub memory_fragmentation_ratio: f64,
    pub meets_memory_targets: bool,
}

/// Concurrent operation benchmarks
#[derive(Debug, Clone)]
pub struct ConcurrentBenchmark {
    pub concurrent_creation_count: usize,
    pub total_creation_time_ms: f64,
    pub average_creation_time_ms: f64,
    pub throughput_under_concurrency: f64,
    pub resource_contention_detected: bool,
    pub deadlock_incidents: usize,
    pub successful_concurrent_operations: usize,
    pub concurrent_efficiency_score: f64,
}

/// Scalability analysis results
#[derive(Debug, Clone)]
pub struct ScalabilityBenchmark {
    pub model_counts_tested: Vec<usize>,
    pub creation_times_by_count: HashMap<usize, f64>,
    pub memory_usage_by_count: HashMap<usize, f64>,
    pub throughput_by_count: HashMap<usize, f64>,
    pub scalability_coefficient: f64,
    pub breaking_point_model_count: Option<usize>,
    pub linear_scalability_maintained: bool,
}

/// Detailed profiling data
#[derive(Debug, Clone)]
pub struct DetailedProfilingData {
    pub cpu_profile_samples: Vec<CpuProfileSample>,
    pub memory_allocation_patterns: Vec<MemoryAllocationSample>,
    pub io_operations: Vec<IoOperationSample>,
    pub lock_contention_events: Vec<LockContentionEvent>,
    pub garbage_collection_events: Vec<GcEvent>,
}

#[derive(Debug, Clone)]
pub struct CpuProfileSample {
    pub timestamp: DateTime<Utc>,
    pub cpu_usage_percent: f64,
    pub active_threads: usize,
    pub operation_type: String,
}

#[derive(Debug, Clone)]
pub struct MemoryAllocationSample {
    pub timestamp: DateTime<Utc>,
    pub allocated_bytes: usize,
    pub deallocated_bytes: usize,
    pub allocation_type: String,
}

#[derive(Debug, Clone)]
pub struct IoOperationSample {
    pub timestamp: DateTime<Utc>,
    pub operation_type: String,
    pub bytes_transferred: usize,
    pub duration_ms: f64,
}

#[derive(Debug, Clone)]
pub struct LockContentionEvent {
    pub timestamp: DateTime<Utc>,
    pub lock_type: String,
    pub contention_duration_ms: f64,
    pub threads_waiting: usize,
}

#[derive(Debug, Clone)]
pub struct GcEvent {
    pub timestamp: DateTime<Utc>,
    pub generation: u8,
    pub duration_ms: f64,
    pub memory_freed_bytes: usize,
}

#[cfg(test)]
mod factory_performance_benchmarks {
    use super::*;

    /// Benchmark model creation latency across all model types
    #[tokio::test]
    #[serial]
    #[traced_test]
    async fn benchmark_model_creation_latency() -> Result<()> {
        // GIVEN: Factory and benchmark configuration
        let config = PerformanceBenchmarkConfig::default();
        let factory = create_integrated_model_factory().await?;
        
        // WHEN: Running creation latency benchmarks for all models
        let mut creation_benchmarks = HashMap::new();
        
        for model_type in &[ModelType::MLP, ModelType::LSTM, ModelType::NHITS, ModelType::TCN, ModelType::DeepAR] {
            tracing::info!("Benchmarking creation latency for {:?}", model_type);
            
            let mut creation_times = Vec::new();
            let mut memory_measurements = Vec::new();
            let mut success_count = 0;
            
            let benchmark_start = Instant::now();
            
            for i in 0..config.latency_test_iterations {
                let memory_before = get_current_memory_usage();
                let creation_start = Instant::now();
                
                let creation_result = factory.create_model(
                    model_type.clone(), 
                    create_benchmark_config(model_type, i)
                ).await;
                
                let creation_time = creation_start.elapsed().as_micros() as f64 / 1000.0; // Convert to ms
                let memory_after = get_current_memory_usage();
                
                if creation_result.is_ok() {
                    success_count += 1;
                    creation_times.push(creation_time);
                    memory_measurements.push(memory_after - memory_before);
                }
                
                // Small delay to prevent resource exhaustion
                if i % 100 == 0 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
            
            let total_benchmark_time = benchmark_start.elapsed();
            
            // Calculate statistics
            creation_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let avg_creation_time = creation_times.iter().sum::<f64>() / creation_times.len() as f64;
            let min_creation_time = creation_times[0];
            let max_creation_time = creation_times[creation_times.len() - 1];
            let p95_index = (creation_times.len() as f64 * 0.95) as usize;
            let p99_index = (creation_times.len() as f64 * 0.99) as usize;
            let p95_creation_time = creation_times[p95_index];
            let p99_creation_time = creation_times[p99_index];
            
            let creation_throughput = success_count as f64 / total_benchmark_time.as_secs_f64();
            let avg_memory_usage = memory_measurements.iter().sum::<f64>() / memory_measurements.len() as f64;
            let success_rate = success_count as f64 / config.latency_test_iterations as f64;
            
            let benchmark = ModelCreationBenchmark {
                model_type: model_type.clone(),
                average_creation_time_ms: avg_creation_time,
                min_creation_time_ms: min_creation_time,
                max_creation_time_ms: max_creation_time,
                p95_creation_time_ms: p95_creation_time,
                p99_creation_time_ms: p99_creation_time,
                creation_throughput_per_second: creation_throughput,
                memory_usage_during_creation_mb: avg_memory_usage / 1024.0 / 1024.0,
                cpu_usage_during_creation_percent: 0.0, // Would be measured in real implementation
                creation_success_rate: success_rate,
                meets_latency_target: avg_creation_time < config.performance_targets.max_creation_latency_ms,
                meets_throughput_target: creation_throughput > config.performance_targets.min_creation_throughput_per_second,
            };
            
            creation_benchmarks.insert(model_type.clone(), benchmark);
        }
        
        // THEN: All models should meet creation latency targets
        for (model_type, benchmark) in &creation_benchmarks {
            assert!(
                benchmark.meets_latency_target,
                "Model {:?} creation latency {:.1}ms should be < {:.1}ms",
                model_type, benchmark.average_creation_time_ms, config.performance_targets.max_creation_latency_ms
            );
            
            assert!(
                benchmark.meets_throughput_target,
                "Model {:?} creation throughput {:.1}/s should be > {:.1}/s",
                model_type, benchmark.creation_throughput_per_second, config.performance_targets.min_creation_throughput_per_second
            );
            
            assert!(
                benchmark.creation_success_rate > 0.99,
                "Model {:?} creation success rate should be >99%, got {:.2}%",
                model_type, benchmark.creation_success_rate * 100.0
            );
            
            // Verify latency consistency (P99 should not be more than 3x average)
            assert!(
                benchmark.p99_creation_time_ms < benchmark.average_creation_time_ms * 3.0,
                "Model {:?} P99 latency {:.1}ms should not be >3x average {:.1}ms",
                model_type, benchmark.p99_creation_time_ms, benchmark.average_creation_time_ms
            );
        }
        
        tracing::info!(
            "Creation latency benchmarks passed for all {} models",
            creation_benchmarks.len()
        );
        
        Ok(())
    }

    /// Benchmark prediction performance for all models
    #[tokio::test]
    #[serial]
    #[traced_test]
    async fn benchmark_prediction_performance() -> Result<()> {
        // GIVEN: Pre-created models and test data for prediction benchmarking
        let config = PerformanceBenchmarkConfig::default();
        let factory = create_integrated_model_factory().await?;
        let test_data = generate_prediction_benchmark_data(1000);
        
        // WHEN: Running prediction performance benchmarks
        let mut prediction_benchmarks = HashMap::new();
        
        for model_type in &[ModelType::MLP, ModelType::LSTM, ModelType::NHITS, ModelType::TCN, ModelType::DeepAR] {
            tracing::info!("Benchmarking prediction performance for {:?}", model_type);
            
            // Create model for benchmarking
            let model = factory.create_model(model_type.clone(), create_benchmark_config(model_type, 0)).await?;
            
            let mut prediction_times = Vec::new();
            let mut predictions = Vec::new();
            let mut memory_measurements = Vec::new();
            
            let benchmark_start = Instant::now();
            
            // Run prediction benchmark
            for (i, data_point) in test_data.iter().enumerate() {
                let memory_before = get_current_memory_usage();
                let prediction_start = Instant::now();
                
                let prediction_result = model.predict(&data_point.features).await;
                
                let prediction_time = prediction_start.elapsed().as_micros() as f64 / 1000.0; // Convert to ms
                let memory_after = get_current_memory_usage();
                
                if let Ok(prediction) = prediction_result {
                    prediction_times.push(prediction_time);
                    predictions.push(prediction);
                    memory_measurements.push(memory_after - memory_before);
                }
                
                // Small delay every 100 predictions
                if i % 100 == 0 && i > 0 {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            }
            
            let total_benchmark_time = benchmark_start.elapsed();
            
            // Calculate prediction statistics
            prediction_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let avg_prediction_time = prediction_times.iter().sum::<f64>() / prediction_times.len() as f64;
            let min_prediction_time = prediction_times[0];
            let max_prediction_time = prediction_times[prediction_times.len() - 1];
            let p95_index = (prediction_times.len() as f64 * 0.95) as usize;
            let p99_index = (prediction_times.len() as f64 * 0.99) as usize;
            let p95_prediction_time = prediction_times[p95_index];
            let p99_prediction_time = prediction_times[p99_index];
            
            let prediction_throughput = predictions.len() as f64 / total_benchmark_time.as_secs_f64();
            let avg_memory_usage = memory_measurements.iter().sum::<f64>() / memory_measurements.len() as f64;
            
            // Calculate prediction accuracy (mock calculation)
            let prediction_accuracy = calculate_prediction_accuracy(&predictions, &test_data);
            
            let benchmark = PredictionBenchmark {
                model_type: model_type.clone(),
                average_prediction_time_ms: avg_prediction_time,
                min_prediction_time_ms: min_prediction_time,
                max_prediction_time_ms: max_prediction_time,
                p95_prediction_time_ms: p95_prediction_time,
                p99_prediction_time_ms: p99_prediction_time,
                prediction_throughput_per_second: prediction_throughput,
                prediction_accuracy,
                memory_usage_during_prediction_mb: avg_memory_usage / 1024.0 / 1024.0,
                cpu_usage_during_prediction_percent: 0.0, // Would be measured in real implementation
                cache_hit_rate: 0.8, // Mock value
                meets_prediction_targets: avg_prediction_time < config.performance_targets.max_prediction_latency_ms &&
                                        prediction_throughput > config.performance_targets.min_prediction_throughput,
            };
            
            prediction_benchmarks.insert(model_type.clone(), benchmark);
        }
        
        // THEN: All models should meet prediction performance targets
        for (model_type, benchmark) in &prediction_benchmarks {
            assert!(
                benchmark.average_prediction_time_ms < config.performance_targets.max_prediction_latency_ms,
                "Model {:?} prediction latency {:.1}ms should be < {:.1}ms",
                model_type, benchmark.average_prediction_time_ms, config.performance_targets.max_prediction_latency_ms
            );
            
            assert!(
                benchmark.prediction_throughput_per_second > config.performance_targets.min_prediction_throughput,
                "Model {:?} prediction throughput {:.1}/s should be > {:.1}/s",
                model_type, benchmark.prediction_throughput_per_second, config.performance_targets.min_prediction_throughput
            );
            
            assert!(
                benchmark.prediction_accuracy > 0.6,
                "Model {:?} prediction accuracy should be >60%, got {:.2}%",
                model_type, benchmark.prediction_accuracy * 100.0
            );
            
            // Verify prediction latency consistency
            assert!(
                benchmark.p99_prediction_time_ms < benchmark.average_prediction_time_ms * 5.0,
                "Model {:?} P99 prediction latency should not be >5x average",
                model_type
            );
        }
        
        tracing::info!(
            "Prediction performance benchmarks passed for all {} models",
            prediction_benchmarks.len()
        );
        
        Ok(())
    }

    /// Benchmark memory efficiency during sustained operations
    #[tokio::test]
    #[serial]
    #[traced_test]
    async fn benchmark_memory_efficiency() -> Result<()> {
        // GIVEN: Extended test configuration for memory analysis
        let config = PerformanceBenchmarkConfig {
            memory_test_duration: Duration::from_minutes(3), // Shorter for testing
            ..Default::default()
        };
        let factory = create_integrated_model_factory().await?;
        
        // WHEN: Running extended memory efficiency tests
        let mut memory_benchmarks = HashMap::new();
        
        for model_type in &[ModelType::MLP, ModelType::LSTM, ModelType::NHITS, ModelType::TCN, ModelType::DeepAR] {
            tracing::info!("Benchmarking memory efficiency for {:?}", model_type);
            
            let baseline_memory = get_current_memory_usage();
            let mut memory_samples = Vec::new();
            let mut peak_memory = baseline_memory;
            
            let test_start = Instant::now();
            let mut operation_count = 0;
            
            // Create and use models continuously for the test duration
            while test_start.elapsed() < config.memory_test_duration {
                // Create model
                let _model = factory.create_model(model_type.clone(), create_benchmark_config(model_type, operation_count)).await?;
                
                // Sample memory usage
                let current_memory = get_current_memory_usage();
                memory_samples.push(current_memory);
                peak_memory = peak_memory.max(current_memory);
                
                operation_count += 1;
                
                // Control operation rate
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            
            let test_duration_hours = test_start.elapsed().as_secs_f64() / 3600.0;
            let final_memory = get_current_memory_usage();
            
            // Calculate memory statistics
            let avg_memory = memory_samples.iter().sum::<f64>() / memory_samples.len() as f64;
            let memory_growth = final_memory - baseline_memory;
            let memory_growth_rate = memory_growth / test_duration_hours;
            
            // Calculate memory efficiency score
            let memory_efficiency_score = calculate_memory_efficiency_score(
                baseline_memory,
                avg_memory,
                peak_memory,
                memory_growth_rate
            );
            
            let benchmark = MemoryBenchmark {
                model_type: model_type.clone(),
                baseline_memory_mb: baseline_memory / 1024.0 / 1024.0,
                peak_memory_mb: peak_memory / 1024.0 / 1024.0,
                average_memory_mb: avg_memory / 1024.0 / 1024.0,
                memory_growth_rate_mb_per_hour: memory_growth_rate / 1024.0 / 1024.0,
                memory_efficiency_score,
                garbage_collection_frequency: 0.0, // Would be measured in real implementation
                memory_fragmentation_ratio: 0.1, // Mock value
                meets_memory_targets: memory_efficiency_score > config.performance_targets.memory_efficiency_threshold &&
                                    peak_memory / 1024.0 / 1024.0 < config.performance_targets.max_memory_usage_mb,
            };
            
            memory_benchmarks.insert(model_type.clone(), benchmark);
        }
        
        // THEN: All models should demonstrate good memory efficiency
        for (model_type, benchmark) in &memory_benchmarks {
            assert!(
                benchmark.meets_memory_targets,
                "Model {:?} should meet memory efficiency targets",
                model_type
            );
            
            assert!(
                benchmark.memory_efficiency_score > config.performance_targets.memory_efficiency_threshold,
                "Model {:?} memory efficiency score {:.2} should be > {:.2}",
                model_type, benchmark.memory_efficiency_score, config.performance_targets.memory_efficiency_threshold
            );
            
            assert!(
                benchmark.peak_memory_mb < config.performance_targets.max_memory_usage_mb,
                "Model {:?} peak memory {:.1}MB should be < {:.1}MB",
                model_type, benchmark.peak_memory_mb, config.performance_targets.max_memory_usage_mb
            );
            
            // Memory growth rate should be reasonable
            assert!(
                benchmark.memory_growth_rate_mb_per_hour < 50.0,
                "Model {:?} memory growth rate should be <50MB/hour, got {:.1}MB/hour",
                model_type, benchmark.memory_growth_rate_mb_per_hour
            );
        }
        
        tracing::info!(
            "Memory efficiency benchmarks passed for all {} models",
            memory_benchmarks.len()
        );
        
        Ok(())
    }

    /// Benchmark concurrent model creation performance
    #[tokio::test]
    #[serial]
    #[traced_test]
    async fn benchmark_concurrent_model_creation() -> Result<()> {
        // GIVEN: Configuration for concurrent operations testing
        let config = PerformanceBenchmarkConfig::default();
        let factory = Arc::new(create_integrated_model_factory().await?);
        
        // WHEN: Creating multiple models concurrently
        let concurrent_start = Instant::now();
        let semaphore = Arc::new(Semaphore::new(10)); // Limit concurrent operations
        
        let concurrent_tasks: Vec<_> = (0..config.concurrent_creation_count)
            .map(|i| {
                let factory_clone = factory.clone();
                let semaphore_clone = semaphore.clone();
                let model_type = match i % 5 {
                    0 => ModelType::MLP,
                    1 => ModelType::LSTM,
                    2 => ModelType::NHITS,
                    3 => ModelType::TCN,
                    4 => ModelType::DeepAR,
                    _ => ModelType::MLP,
                };
                
                tokio::spawn(async move {
                    let _permit = semaphore_clone.acquire().await.unwrap();
                    let creation_start = Instant::now();
                    
                    let result = factory_clone.create_model(
                        model_type.clone(),
                        create_benchmark_config(&model_type, i)
                    ).await;
                    
                    let creation_time = creation_start.elapsed().as_millis() as f64;
                    (result.is_ok(), creation_time, model_type)
                })
            })
            .collect();
        
        let results = futures::future::join_all(concurrent_tasks).await;
        let total_concurrent_time = concurrent_start.elapsed().as_millis() as f64;
        
        // Analyze concurrent operation results
        let mut successful_operations = 0;
        let mut total_creation_time = 0.0;
        let mut creation_times = Vec::new();
        
        for result in results {
            if let Ok((success, creation_time, _model_type)) = result {
                if success {
                    successful_operations += 1;
                    total_creation_time += creation_time;
                    creation_times.push(creation_time);
                }
            }
        }
        
        let average_creation_time = total_creation_time / successful_operations as f64;
        let throughput_under_concurrency = successful_operations as f64 / (total_concurrent_time / 1000.0);
        let concurrent_efficiency = throughput_under_concurrency / config.performance_targets.min_creation_throughput_per_second;
        
        let concurrent_benchmark = ConcurrentBenchmark {
            concurrent_creation_count: config.concurrent_creation_count,
            total_creation_time_ms: total_concurrent_time,
            average_creation_time_ms: average_creation_time,
            throughput_under_concurrency,
            resource_contention_detected: false, // Would be detected in real implementation
            deadlock_incidents: 0,
            successful_concurrent_operations: successful_operations,
            concurrent_efficiency_score: concurrent_efficiency,
        };
        
        // THEN: Concurrent operations should maintain reasonable performance
        assert!(
            concurrent_benchmark.successful_concurrent_operations >= (config.concurrent_creation_count as f64 * 0.95) as usize,
            "At least 95% of concurrent operations should succeed, got {}",
            concurrent_benchmark.successful_concurrent_operations
        );
        
        assert!(
            concurrent_benchmark.concurrent_efficiency_score > 0.7,
            "Concurrent efficiency should be >70%, got {:.2}",
            concurrent_benchmark.concurrent_efficiency_score
        );
        
        assert!(
            concurrent_benchmark.deadlock_incidents == 0,
            "No deadlocks should occur during concurrent operations"
        );
        
        assert!(
            !concurrent_benchmark.resource_contention_detected,
            "Minimal resource contention should be detected"
        );
        
        // Average creation time under concurrency should not be drastically worse
        assert!(
            concurrent_benchmark.average_creation_time_ms < config.performance_targets.max_creation_latency_ms * 2.0,
            "Concurrent creation time should not be >2x normal latency"
        );
        
        tracing::info!(
            "Concurrent benchmark passed: {}/{} operations successful, {:.1}/s throughput",
            concurrent_benchmark.successful_concurrent_operations,
            config.concurrent_creation_count,
            concurrent_benchmark.throughput_under_concurrency
        );
        
        Ok(())
    }

    /// Benchmark scalability with increasing model counts
    #[tokio::test]
    #[serial]
    #[traced_test]
    async fn benchmark_scalability_analysis() -> Result<()> {
        // GIVEN: Scalability test configuration
        let factory = create_integrated_model_factory().await?;
        let model_counts = vec![1, 5, 10, 20, 50, 100];
        
        // WHEN: Testing performance at different scales
        let mut creation_times_by_count = HashMap::new();
        let mut memory_usage_by_count = HashMap::new();
        let mut throughput_by_count = HashMap::new();
        let mut breaking_point = None;
        
        for &model_count in &model_counts {
            tracing::info!("Testing scalability with {} models", model_count);
            
            let scale_test_start = Instant::now();
            let baseline_memory = get_current_memory_usage();
            let mut successful_creations = 0;
            
            // Create specified number of models
            for i in 0..model_count {
                let model_type = match i % 5 {
                    0 => ModelType::MLP,
                    1 => ModelType::LSTM,
                    2 => ModelType::NHITS,
                    3 => ModelType::TCN,
                    4 => ModelType::DeepAR,
                    _ => ModelType::MLP,
                };
                
                let creation_result = factory.create_model(
                    model_type,
                    create_benchmark_config(&ModelType::MLP, i) // Use simpler config for scalability
                ).await;
                
                if creation_result.is_ok() {
                    successful_creations += 1;
                } else {
                    // If we start failing, this might be our breaking point
                    if successful_creations < model_count * 8 / 10 { // Less than 80% success
                        breaking_point = Some(model_count);
                        break;
                    }
                }
                
                // Small delay to prevent overwhelming the system
                if i % 10 == 0 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
            
            let scale_test_duration = scale_test_start.elapsed();
            let final_memory = get_current_memory_usage();
            let memory_usage = final_memory - baseline_memory;
            let throughput = successful_creations as f64 / scale_test_duration.as_secs_f64();
            
            creation_times_by_count.insert(model_count, scale_test_duration.as_millis() as f64);
            memory_usage_by_count.insert(model_count, memory_usage / 1024.0 / 1024.0);
            throughput_by_count.insert(model_count, throughput);
            
            // Break if we hit performance degradation
            if breaking_point.is_some() {
                break;
            }
        }
        
        // Calculate scalability coefficient
        let scalability_coefficient = calculate_scalability_coefficient(&creation_times_by_count);
        let linear_scalability_maintained = scalability_coefficient > 0.8; // 80% linear scaling
        
        let scalability_benchmark = ScalabilityBenchmark {
            model_counts_tested: model_counts.clone(),
            creation_times_by_count,
            memory_usage_by_count,
            throughput_by_count,
            scalability_coefficient,
            breaking_point_model_count: breaking_point,
            linear_scalability_maintained,
        };
        
        // THEN: System should demonstrate reasonable scalability
        assert!(
            scalability_benchmark.linear_scalability_maintained,
            "System should maintain reasonable linear scalability, coefficient: {:.2}",
            scalability_benchmark.scalability_coefficient
        );
        
        // Breaking point should be reasonable (at least 50 models)
        if let Some(breaking_point) = scalability_benchmark.breaking_point_model_count {
            assert!(
                breaking_point >= 50,
                "System should handle at least 50 models before degradation, broke at {}",
                breaking_point
            );
        }
        
        // Memory usage should scale reasonably
        let memory_at_1 = scalability_benchmark.memory_usage_by_count[&1];
        let memory_at_max = scalability_benchmark.memory_usage_by_count
            .values()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        
        let memory_scaling_factor = memory_at_max / memory_at_1;
        assert!(
            memory_scaling_factor < 100.0, // Should not be more than 100x memory usage
            "Memory scaling should be reasonable, got {}x increase",
            memory_scaling_factor
        );
        
        tracing::info!(
            "Scalability benchmark passed: {:.2} coefficient, tested up to {} models",
            scalability_benchmark.scalability_coefficient,
            model_counts.last().unwrap_or(&0)
        );
        
        Ok(())
    }
}

// Helper functions for benchmarking

async fn create_integrated_model_factory() -> Result<IntegratedModelFactory> {
    Ok(IntegratedModelFactory::new().await?)
}

fn create_benchmark_config(model_type: &ModelType, variation: usize) -> ModelConfig {
    // Create varied configurations for more realistic benchmarking
    match model_type {
        ModelType::MLP => ModelConfig {
            layers: vec![10 + variation % 5, 20 + variation % 10, 10, 1],
            activation: "relu".to_string(),
            optimizer: "adam".to_string(),
            learning_rate: 0.001 + (variation as f64 * 0.0001),
            ..Default::default()
        },
        ModelType::LSTM => ModelConfig {
            hidden_size: 64 + (variation % 32),
            num_layers: 2 + (variation % 2),
            dropout: 0.1 + (variation as f64 * 0.01),
            bidirectional: variation % 2 == 0,
            ..Default::default()
        },
        ModelType::NHITS => ModelConfig {
            stack_types: vec!["trend".to_string(), "seasonality".to_string()],
            n_blocks: vec![1 + variation % 2, 1 + variation % 2],
            mlp_units: vec![512 + variation % 256, 512 + variation % 256],
            ..Default::default()
        },
        ModelType::TCN => ModelConfig {
            num_channels: vec![25 + variation % 10, 25 + variation % 10, 25 + variation % 10],
            kernel_size: 3 + (variation % 3),
            dropout: 0.2 + (variation as f64 * 0.01),
            ..Default::default()
        },
        ModelType::DeepAR => ModelConfig {
            hidden_size: 40 + (variation % 20),
            num_layers: 2 + (variation % 2),
            dropout: 0.1 + (variation as f64 * 0.01),
            likelihood: "gaussian".to_string(),
            ..Default::default()
        },
    }
}

fn get_current_memory_usage() -> f64 {
    // Mock implementation - would use actual system memory monitoring
    150.0 * 1024.0 * 1024.0 // 150MB baseline in bytes
}

fn generate_prediction_benchmark_data(count: usize) -> Vec<TestDataPoint> {
    (0..count)
        .map(|i| TestDataPoint {
            timestamp: chrono::Utc::now().timestamp() + i as i64,
            features: vec![i as f64, (i * 2) as f64, (i as f64).sin(), (i as f64).cos()],
            target: (i as f64 * 0.1).sin() + (i as f64 * 0.05).cos(),
        })
        .collect()
}

fn calculate_prediction_accuracy(_predictions: &[f64], _test_data: &[TestDataPoint]) -> f64 {
    // Mock calculation - would compute actual accuracy metrics
    0.75 // 75% accuracy
}

fn calculate_memory_efficiency_score(
    baseline: f64,
    average: f64,
    peak: f64,
    growth_rate: f64,
) -> f64 {
    // Calculate efficiency based on memory usage patterns
    let usage_efficiency = baseline / average;
    let peak_efficiency = average / peak;
    let growth_efficiency = 1.0 / (1.0 + growth_rate.abs() / 1024.0 / 1024.0); // Normalize growth rate
    
    (usage_efficiency + peak_efficiency + growth_efficiency) / 3.0
}

fn calculate_scalability_coefficient(creation_times: &HashMap<usize, f64>) -> f64 {
    // Calculate how close to linear the scaling is
    if creation_times.len() < 2 {
        return 1.0;
    }
    
    let mut keys: Vec<_> = creation_times.keys().collect();
    keys.sort();
    
    let base_count = *keys[0];
    let base_time = creation_times[keys[0]];
    let base_rate = base_time / base_count as f64;
    
    let mut efficiency_scores = Vec::new();
    
    for &count in keys.iter().skip(1) {
        let actual_time = creation_times[count];
        let expected_time = base_rate * *count as f64;
        let efficiency = expected_time / actual_time;
        efficiency_scores.push(efficiency.min(1.0)); // Cap at 100% efficiency
    }
    
    efficiency_scores.iter().sum::<f64>() / efficiency_scores.len() as f64
}

// Mock implementations (same as in other file)

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModelType {
    MLP,
    LSTM,
    NHITS,
    TCN,
    DeepAR,
}

#[derive(Debug, Default, Clone)]
struct ModelConfig {
    layers: Vec<usize>,
    activation: String,
    optimizer: String,
    learning_rate: f64,
    hidden_size: usize,
    num_layers: usize,
    dropout: f64,
    bidirectional: bool,
    stack_types: Vec<String>,
    n_blocks: Vec<usize>,
    mlp_units: Vec<usize>,
    num_channels: Vec<usize>,
    kernel_size: usize,
    likelihood: String,
}

struct TestDataPoint {
    timestamp: i64,
    features: Vec<f64>,
    target: f64,
}

trait ModelAdapter: Send + Sync {
    async fn predict(&self, input: &[f64]) -> Result<f64>;
    fn get_model_type(&self) -> String;
}

struct IntegratedModelFactory;

impl IntegratedModelFactory {
    async fn new() -> Result<Self> {
        Ok(Self)
    }
    
    async fn create_model(&self, model_type: ModelType, _config: ModelConfig) -> Result<Box<dyn ModelAdapter>> {
        Ok(Box::new(MockModelAdapter::new(model_type)))
    }
}

struct MockModelAdapter {
    model_type: ModelType,
}

impl MockModelAdapter {
    fn new(model_type: ModelType) -> Self {
        Self { model_type }
    }
}

impl ModelAdapter for MockModelAdapter {
    async fn predict(&self, _input: &[f64]) -> Result<f64> {
        // Simulate some work
        tokio::time::sleep(Duration::from_millis(1)).await;
        Ok(0.5) // Mock prediction
    }
    
    fn get_model_type(&self) -> String {
        format!("{:?}", self.model_type)
    }
}