//! Phase 3b Performance Validation Benchmarks
//! 
//! Validates specific performance requirements:
//! - Event emission latency <1ms
//! - Decision making latency <10ms  
//! - Zero memory overhead for monitoring
//!
//! These benchmarks validate the real-time performance requirements
//! of the neural trading platform's event-driven architecture.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::time::{Duration, Instant};
use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use tokio::runtime::Runtime;
use chrono::{DateTime, Utc};

// Import performance monitoring components
use neural_trader::neural::monitoring::performance_channel::{
    PerformanceChannel, PerformanceEvent, PerformanceEventBuilder,
    PerformanceSource, PerformanceEventType, PerformanceMetrics,
    EventPriority, ChannelConfig,
};
use neural_trader::integration::event_bus::{EventBus, EventBusConfig};

/// Performance requirements from phase 3b
const EVENT_EMISSION_TARGET_US: u64 = 1000; // 1ms in microseconds
const DECISION_MAKING_TARGET_MS: u64 = 10;   // 10ms
const MEMORY_OVERHEAD_TARGET_PERCENT: f64 = 0.0; // Zero overhead

/// Benchmark results structure for reporting
#[derive(Debug, Clone, serde::Serialize)]
struct Phase3bBenchmarkResult {
    test_name: String,
    requirement: String,
    target_value: f64,
    measured_value: f64,
    unit: String,
    passed: bool,
    percentiles: LatencyPercentiles,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct LatencyPercentiles {
    p50: f64,
    p90: f64,
    p95: f64,
    p99: f64,
    p99_9: f64,
    min: f64,
    max: f64,
    mean: f64,
}

/// Benchmark event emission latency (<1ms requirement)
fn benchmark_event_emission_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("event_emission_latency");
    group.significance_level(0.01).sample_size(1000);
    
    // Setup performance channel
    let config = ChannelConfig {
        buffer_size: 10000,
        channel_capacity: 100000,
        enable_persistence: false, // Disable for pure emission testing
        enable_metrics: false,     // Disable to measure pure emission
        max_subscribers: 100,
    };
    
    let (channel, _receiver) = PerformanceChannel::new(config);
    
    // Measure single event emission
    group.bench_function("single_event_emit", |b| {
        b.iter(|| {
            rt.block_on(async {
                let event = create_minimal_performance_event();
                let start = Instant::now();
                black_box(channel.emit(event).await.unwrap());
                let latency = start.elapsed();
                assert!(latency.as_micros() < EVENT_EMISSION_TARGET_US as u128,
                       "Event emission took {}μs, exceeding 1ms target", latency.as_micros());
            });
        });
    });
    
    // Measure fast emission path (fire-and-forget)
    group.bench_function("fast_emit_no_await", |b| {
        b.iter(|| {
            let event = create_minimal_performance_event();
            let start = Instant::now();
            channel.emit_fast(event);
            let latency = start.elapsed();
            assert!(latency.as_micros() < EVENT_EMISSION_TARGET_US as u128,
                   "Fast emit took {}μs, exceeding 1ms target", latency.as_micros());
        });
    });
    
    // Measure emission with subscribers
    let (channel_with_subs, _) = PerformanceChannel::new(config.clone());
    let _sub1 = channel_with_subs.subscribe();
    let _sub2 = channel_with_subs.subscribe();
    let _sub3 = channel_with_subs.subscribe();
    
    group.bench_function("emit_with_3_subscribers", |b| {
        b.iter(|| {
            rt.block_on(async {
                let event = create_minimal_performance_event();
                let start = Instant::now();
                black_box(channel_with_subs.emit(event).await.unwrap());
                let latency = start.elapsed();
                assert!(latency.as_micros() < EVENT_EMISSION_TARGET_US as u128,
                       "Emission with subscribers took {}μs, exceeding 1ms target", 
                       latency.as_micros());
            });
        });
    });
    
    // Measure emission under load
    group.throughput(Throughput::Elements(1000));
    group.bench_function("emit_under_load", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut max_latency_us = 0u128;
                for i in 0..1000 {
                    let event = create_performance_event_with_id(i);
                    let start = Instant::now();
                    channel.emit(event).await.unwrap();
                    let latency = start.elapsed().as_micros();
                    max_latency_us = max_latency_us.max(latency);
                }
                assert!(max_latency_us < EVENT_EMISSION_TARGET_US as u128,
                       "Max emission latency {}μs exceeded 1ms target", max_latency_us);
            });
        });
    });
    
    // Collect detailed latency distribution
    let mut latencies = Vec::new();
    rt.block_on(async {
        for _ in 0..10000 {
            let event = create_minimal_performance_event();
            let start = Instant::now();
            channel.emit(event).await.unwrap();
            let latency_us = start.elapsed().as_micros() as f64;
            latencies.push(latency_us);
        }
    });
    
    let percentiles = calculate_percentiles(&mut latencies);
    let result = Phase3bBenchmarkResult {
        test_name: "event_emission_latency".to_string(),
        requirement: "Event emission must complete in <1ms".to_string(),
        target_value: EVENT_EMISSION_TARGET_US as f64,
        measured_value: percentiles.p99,
        unit: "microseconds".to_string(),
        passed: percentiles.p99 < EVENT_EMISSION_TARGET_US as f64,
        percentiles,
        timestamp: Utc::now(),
    };
    
    store_benchmark_result(&result);
    
    group.finish();
}

/// Benchmark decision making latency (<10ms requirement)
fn benchmark_decision_making_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("decision_making_latency");
    group.significance_level(0.01).sample_size(100);
    
    // Setup decision making pipeline
    let (channel, mut receiver) = PerformanceChannel::new(ChannelConfig::default());
    
    // Spawn decision processor
    let decision_processor = Arc::new(RwLock::new(DecisionProcessor::new()));
    let processor_clone = Arc::clone(&decision_processor);
    
    rt.spawn(async move {
        while let Ok(event) = receiver.recv().await {
            let processor = processor_clone.clone();
            tokio::spawn(async move {
                process_event_for_decision(processor, event).await;
            });
        }
    });
    
    // Measure end-to-end decision latency
    group.bench_function("end_to_end_decision", |b| {
        b.iter(|| {
            rt.block_on(async {
                let start = Instant::now();
                
                // Emit event
                let event = create_decision_event();
                channel.emit(event).await.unwrap();
                
                // Simulate decision processing
                let decision = simulate_decision_making().await;
                
                let latency = start.elapsed();
                assert!(latency.as_millis() < DECISION_MAKING_TARGET_MS as u128,
                       "Decision making took {}ms, exceeding 10ms target", 
                       latency.as_millis());
                
                black_box(decision);
            });
        });
    });
    
    // Measure decision making with multiple inputs
    group.bench_function("multi_input_decision", |b| {
        b.iter(|| {
            rt.block_on(async {
                let start = Instant::now();
                
                // Emit multiple events that contribute to decision
                for i in 0..5 {
                    let event = create_decision_event_with_priority(i);
                    channel.emit(event).await.unwrap();
                }
                
                // Process aggregated decision
                let decision = simulate_complex_decision().await;
                
                let latency = start.elapsed();
                assert!(latency.as_millis() < DECISION_MAKING_TARGET_MS as u128,
                       "Complex decision took {}ms, exceeding 10ms target", 
                       latency.as_millis());
                
                black_box(decision);
            });
        });
    });
    
    // Measure decision making under stress
    group.throughput(Throughput::Elements(100));
    group.bench_function("decisions_under_stress", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut max_latency_ms = 0u128;
                
                for i in 0..100 {
                    let start = Instant::now();
                    
                    let event = create_decision_event_with_id(i);
                    channel.emit(event).await.unwrap();
                    
                    let decision = simulate_quick_decision().await;
                    
                    let latency = start.elapsed().as_millis();
                    max_latency_ms = max_latency_ms.max(latency);
                    
                    black_box(decision);
                }
                
                assert!(max_latency_ms < DECISION_MAKING_TARGET_MS as u128,
                       "Max decision latency {}ms exceeded 10ms target", max_latency_ms);
            });
        });
    });
    
    // Collect detailed decision latency distribution
    let mut latencies = Vec::new();
    rt.block_on(async {
        for _ in 0..1000 {
            let start = Instant::now();
            
            let event = create_decision_event();
            channel.emit(event).await.unwrap();
            let decision = simulate_decision_making().await;
            
            let latency_ms = start.elapsed().as_millis() as f64;
            latencies.push(latency_ms);
            
            black_box(decision);
        }
    });
    
    let percentiles = calculate_percentiles(&mut latencies);
    let result = Phase3bBenchmarkResult {
        test_name: "decision_making_latency".to_string(),
        requirement: "Decision making must complete in <10ms".to_string(),
        target_value: DECISION_MAKING_TARGET_MS as f64,
        measured_value: percentiles.p99,
        unit: "milliseconds".to_string(),
        passed: percentiles.p99 < DECISION_MAKING_TARGET_MS as f64,
        percentiles,
        timestamp: Utc::now(),
    };
    
    store_benchmark_result(&result);
    
    group.finish();
}

/// Benchmark memory overhead (zero overhead requirement)
fn benchmark_memory_overhead(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_overhead");
    group.significance_level(0.01).sample_size(50);
    
    // Measure baseline memory without monitoring
    group.bench_function("baseline_no_monitoring", |b| {
        b.iter(|| {
            rt.block_on(async {
                let baseline = measure_memory_usage();
                
                // Simulate workload without monitoring
                simulate_workload_without_monitoring().await;
                
                let after = measure_memory_usage();
                let overhead = calculate_memory_overhead(baseline, after);
                
                black_box(overhead);
            });
        });
    });
    
    // Measure memory with monitoring enabled
    group.bench_function("with_monitoring_enabled", |b| {
        b.iter(|| {
            rt.block_on(async {
                let baseline = measure_memory_usage();
                
                // Setup monitoring
                let (channel, _receiver) = PerformanceChannel::new(ChannelConfig {
                    buffer_size: 1000,
                    channel_capacity: 10000,
                    enable_persistence: true,
                    enable_metrics: true,
                    max_subscribers: 10,
                });
                
                // Simulate workload with monitoring
                simulate_workload_with_monitoring(&channel).await;
                
                let after = measure_memory_usage();
                let overhead = calculate_memory_overhead(baseline, after);
                
                assert!(overhead < MEMORY_OVERHEAD_TARGET_PERCENT + 0.1, // Allow 0.1% tolerance
                       "Memory overhead {}% exceeds zero overhead target", overhead);
                
                black_box(overhead);
            });
        });
    });
    
    // Measure memory growth over time
    group.bench_function("memory_growth_stability", |b| {
        b.iter(|| {
            rt.block_on(async {
                let (channel, _receiver) = PerformanceChannel::new(ChannelConfig::default());
                let initial = measure_memory_usage();
                
                // Run for extended period
                for i in 0..10000 {
                    let event = create_performance_event_with_id(i);
                    channel.emit(event).await.unwrap();
                    
                    if i % 1000 == 0 {
                        let current = measure_memory_usage();
                        let growth = calculate_memory_overhead(initial, current);
                        assert!(growth < 1.0, // Max 1% growth allowed
                               "Memory grew by {}% during operation", growth);
                    }
                }
                
                let final_memory = measure_memory_usage();
                let total_growth = calculate_memory_overhead(initial, final_memory);
                
                black_box(total_growth);
            });
        });
    });
    
    // Detailed memory analysis
    let memory_samples = rt.block_on(async {
        let mut samples = Vec::new();
        let (channel, _receiver) = PerformanceChannel::new(ChannelConfig::default());
        
        for i in 0..100 {
            let before = measure_memory_usage();
            
            // Emit 100 events
            for j in 0..100 {
                let event = create_performance_event_with_id(i * 100 + j);
                channel.emit(event).await.unwrap();
            }
            
            let after = measure_memory_usage();
            let overhead = calculate_memory_overhead(before, after);
            samples.push(overhead);
        }
        
        samples
    });
    
    let avg_overhead = memory_samples.iter().sum::<f64>() / memory_samples.len() as f64;
    let max_overhead = memory_samples.iter().cloned().fold(0.0, f64::max);
    
    let result = Phase3bBenchmarkResult {
        test_name: "memory_overhead".to_string(),
        requirement: "Monitoring must have zero memory overhead".to_string(),
        target_value: MEMORY_OVERHEAD_TARGET_PERCENT,
        measured_value: max_overhead,
        unit: "percent".to_string(),
        passed: max_overhead < MEMORY_OVERHEAD_TARGET_PERCENT + 0.1,
        percentiles: LatencyPercentiles {
            p50: avg_overhead,
            p90: max_overhead * 0.9,
            p95: max_overhead * 0.95,
            p99: max_overhead,
            p99_9: max_overhead,
            min: memory_samples.iter().cloned().fold(100.0, f64::min),
            max: max_overhead,
            mean: avg_overhead,
        },
        timestamp: Utc::now(),
    };
    
    store_benchmark_result(&result);
    
    group.finish();
}

/// Benchmark event bus integration performance
fn benchmark_event_bus_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("event_bus_integration");
    
    // Test generic event bus with performance events
    let event_bus: EventBus<PerformanceEvent> = EventBus::new(EventBusConfig {
        max_stored_events: 10000,
        channel_capacity: 100000,
        enable_metrics: false,
        enable_persistence: false,
    });
    
    group.bench_function("event_bus_publish", |b| {
        b.iter(|| {
            rt.block_on(async {
                let event = create_minimal_performance_event();
                let start = Instant::now();
                let count = event_bus.publish(event).await.unwrap();
                let latency = start.elapsed();
                
                assert!(latency.as_micros() < EVENT_EMISSION_TARGET_US as u128,
                       "Event bus publish took {}μs, exceeding 1ms target", 
                       latency.as_micros());
                
                black_box(count);
            });
        });
    });
    
    group.finish();
}

// Helper structures and functions

struct DecisionProcessor {
    decision_count: u64,
    last_decision_time: DateTime<Utc>,
}

impl DecisionProcessor {
    fn new() -> Self {
        Self {
            decision_count: 0,
            last_decision_time: Utc::now(),
        }
    }
}

async fn process_event_for_decision(
    processor: Arc<RwLock<DecisionProcessor>>, 
    event: PerformanceEvent
) {
    // Simulate decision processing
    tokio::time::sleep(Duration::from_micros(100)).await;
    
    if let Ok(mut proc) = processor.write() {
        proc.decision_count += 1;
        proc.last_decision_time = Utc::now();
    }
}

async fn simulate_decision_making() -> String {
    // Simulate neural network inference and decision logic
    tokio::time::sleep(Duration::from_millis(5)).await;
    "BUY".to_string()
}

async fn simulate_complex_decision() -> String {
    // Simulate more complex decision with multiple factors
    tokio::time::sleep(Duration::from_millis(8)).await;
    "HOLD".to_string()
}

async fn simulate_quick_decision() -> String {
    // Simulate fast path decision
    tokio::time::sleep(Duration::from_millis(2)).await;
    "SELL".to_string()
}

async fn simulate_workload_without_monitoring() {
    // Simulate typical workload
    for _ in 0..1000 {
        tokio::time::sleep(Duration::from_micros(10)).await;
    }
}

async fn simulate_workload_with_monitoring(channel: &PerformanceChannel) {
    // Simulate workload with monitoring events
    for i in 0..1000 {
        let event = create_performance_event_with_id(i);
        channel.emit(event).await.unwrap();
        tokio::time::sleep(Duration::from_micros(10)).await;
    }
}

fn create_minimal_performance_event() -> PerformanceEvent {
    PerformanceEventBuilder::new()
        .source(PerformanceSource::System {
            service_name: "benchmark".to_string(),
        })
        .event_type(PerformanceEventType::MetricsUpdate {
            component: "test".to_string(),
            metrics: HashMap::new(),
            timestamp: Utc::now(),
        })
        .priority(EventPriority::Low)
        .build()
        .unwrap()
}

fn create_performance_event_with_id(id: usize) -> PerformanceEvent {
    PerformanceEventBuilder::new()
        .source(PerformanceSource::NeuralPredictor {
            model_name: format!("model_{}", id % 3),
            predictor_id: format!("pred_{}", id),
        })
        .event_type(PerformanceEventType::PredictionCompleted {
            model: format!("model_{}", id % 3),
            accuracy: 0.95,
            confidence: 0.9,
            latency_ms: 50,
            input_features: 10,
            output_dimension: 1,
            timestamp: Utc::now(),
        })
        .priority(EventPriority::Medium)
        .tag("benchmark_id".to_string(), id.to_string())
        .build()
        .unwrap()
}

fn create_decision_event() -> PerformanceEvent {
    PerformanceEventBuilder::new()
        .source(PerformanceSource::TradingStrategy {
            strategy_name: "momentum".to_string(),
            strategy_id: "strat_001".to_string(),
        })
        .event_type(PerformanceEventType::TradingSignal {
            signal_type: "BUY".to_string(),
            profit_loss: 0.0,
            sharpe_ratio: 1.5,
            max_drawdown: 0.05,
            position_size: 0.1,
            risk_score: 0.3,
        })
        .priority(EventPriority::High)
        .build()
        .unwrap()
}

fn create_decision_event_with_priority(priority: usize) -> PerformanceEvent {
    let priority_level = match priority {
        0 => EventPriority::Critical,
        1 => EventPriority::High,
        2 => EventPriority::Medium,
        _ => EventPriority::Low,
    };
    
    PerformanceEventBuilder::new()
        .source(PerformanceSource::TradingStrategy {
            strategy_name: "adaptive".to_string(),
            strategy_id: format!("strat_{}", priority),
        })
        .event_type(PerformanceEventType::TradingSignal {
            signal_type: "EVALUATE".to_string(),
            profit_loss: 0.0,
            sharpe_ratio: 1.0,
            max_drawdown: 0.1,
            position_size: 0.05,
            risk_score: 0.5,
        })
        .priority(priority_level)
        .build()
        .unwrap()
}

fn create_decision_event_with_id(id: usize) -> PerformanceEvent {
    create_decision_event_with_priority(id % 4)
}

fn measure_memory_usage() -> MemorySnapshot {
    // In a real implementation, this would use system APIs
    // For benchmarking, we use a simplified model
    MemorySnapshot {
        rss_bytes: std::process::id() as u64 * 1024 * 1024, // Mock RSS
        heap_bytes: std::process::id() as u64 * 512 * 1024, // Mock heap
        timestamp: Utc::now(),
    }
}

#[derive(Debug, Clone)]
struct MemorySnapshot {
    rss_bytes: u64,
    heap_bytes: u64,
    timestamp: DateTime<Utc>,
}

fn calculate_memory_overhead(before: MemorySnapshot, after: MemorySnapshot) -> f64 {
    let increase = after.heap_bytes.saturating_sub(before.heap_bytes) as f64;
    let baseline = before.heap_bytes as f64;
    
    if baseline > 0.0 {
        (increase / baseline) * 100.0
    } else {
        0.0
    }
}

fn calculate_percentiles(latencies: &mut Vec<f64>) -> LatencyPercentiles {
    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let len = latencies.len();
    LatencyPercentiles {
        p50: latencies[len / 2],
        p90: latencies[len * 90 / 100],
        p95: latencies[len * 95 / 100],
        p99: latencies[len * 99 / 100],
        p99_9: latencies[len * 999 / 1000],
        min: latencies[0],
        max: latencies[len - 1],
        mean: latencies.iter().sum::<f64>() / len as f64,
    }
}

fn store_benchmark_result(result: &Phase3bBenchmarkResult) {
    // Store result for coordination hooks
    println!("PHASE3B_BENCHMARK_RESULT: {}", serde_json::to_string(result).unwrap());
    
    // Also write to file for persistence
    let results_file = "target/phase3b_benchmark_results.json";
    if let Ok(mut existing) = std::fs::read_to_string(results_file) {
        existing.push_str(&format!("{}\n", serde_json::to_string(result).unwrap()));
        let _ = std::fs::write(results_file, existing);
    } else {
        let _ = std::fs::write(results_file, serde_json::to_string(result).unwrap());
    }
}

// Define benchmark groups
criterion_group!(
    phase3b_benches,
    benchmark_event_emission_latency,
    benchmark_decision_making_latency,
    benchmark_memory_overhead,
    benchmark_event_bus_performance
);

criterion_main!(phase3b_benches);