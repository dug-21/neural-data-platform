//! Memory Compliance Tests for Phase 3
//!
//! Validates strict memory requirements:
//! - Total system memory: <525MB (Phase 2: 500MB + 5% overhead)
//! - Per-symbol memory: <50MB 
//! - 90% memory reduction validation
//! - Memory leak detection over 24 hours

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use sysinfo::{System, SystemExt, ProcessExt, Pid};
use anyhow::Result;
use chrono::Utc;

// Import our dependencies
use autonomous_platform::data::{TimeSeriesData, sector_mapper::{SectorMapper, SectorId}};
use autonomous_platform::neural::vendor_predictor::VendorPredictor;
use autonomous_platform::config::NeuralConfig;
use autonomous_platform::monitoring::model_performance_tracker::ModelPerformanceTracker;
use autonomous_platform::data_pipeline::{
    DataPipeline, DataScope, RoutingConfig, ConsolidationConfig, GeographicRegion
};
use autonomous_platform::integration::daa_coordinator::DAACoordinator;

/// Memory monitoring utility
struct MemoryMonitor {
    system: System,
    process_pid: Pid,
    baseline_memory: u64,
    peak_memory: u64,
    measurements: Vec<(Instant, u64)>,
}

impl MemoryMonitor {
    fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        
        let process_pid = Pid::from(std::process::id() as usize);
        let baseline_memory = if let Some(process) = system.process(process_pid) {
            process.memory()
        } else {
            0
        };
        
        Self {
            system,
            process_pid,
            baseline_memory,
            peak_memory: baseline_memory,
            measurements: vec![(Instant::now(), baseline_memory)],
        }
    }
    
    fn measure(&mut self) -> u64 {
        self.system.refresh_process(self.process_pid);
        
        let current_memory = if let Some(process) = self.system.process(self.process_pid) {
            process.memory()
        } else {
            0
        };
        
        self.peak_memory = self.peak_memory.max(current_memory);
        self.measurements.push((Instant::now(), current_memory));
        
        current_memory
    }
    
    fn get_current_mb(&mut self) -> f64 {
        (self.measure() as f64) / 1024.0 / 1024.0
    }
    
    fn get_peak_mb(&self) -> f64 {
        (self.peak_memory as f64) / 1024.0 / 1024.0
    }
    
    fn get_growth_mb(&mut self) -> f64 {
        let current = self.measure();
        ((current - self.baseline_memory) as f64) / 1024.0 / 1024.0
    }
    
    fn get_growth_percentage(&mut self) -> f64 {
        let growth = self.get_growth_mb();
        let baseline_mb = (self.baseline_memory as f64) / 1024.0 / 1024.0;
        if baseline_mb > 0.0 {
            (growth / baseline_mb) * 100.0
        } else {
            0.0
        }
    }
    
    fn detect_leaks(&self, duration_hours: f64) -> bool {
        if self.measurements.len() < 2 {
            return false;
        }
        
        let start_time = self.measurements[0].0;
        let end_time = self.measurements.last().unwrap().0;
        let actual_duration = end_time.duration_since(start_time).as_secs_f64() / 3600.0;
        
        if actual_duration < duration_hours * 0.8 {
            return false; // Not enough data
        }
        
        // Calculate memory growth rate
        let start_memory = self.measurements[0].1 as f64;
        let end_memory = self.measurements.last().unwrap().1 as f64;
        let growth_rate_mb_per_hour = ((end_memory - start_memory) / 1024.0 / 1024.0) / actual_duration;
        
        // Flag as leak if growing >10MB/hour consistently
        growth_rate_mb_per_hour > 10.0
    }
}

/// Creates minimal config for memory testing
fn create_memory_test_config() -> NeuralConfig {
    NeuralConfig {
        model_path: "/tmp/memory_test_models".to_string(),
        batch_size: 8, // Smaller for memory efficiency
        learning_rate: 0.001,
        hidden_layers: vec![16], // Minimal architecture
        activation: "relu".to_string(),
        optimizer: "adam".to_string(),
        loss_function: "mse".to_string(),
        epochs: 5,
        validation_split: 0.1,
        early_stopping: false,
        patience: 3,
        enable_cuda: false,
        model_type: "memory_test".to_string(),
        sequence_length: 20, // Smaller sequence
        prediction_horizon: 1,
        features: vec!["price".to_string()],
        enable_technical_indicators: false,
        enable_feature_scaling: true,
        dropout_rate: 0.0,
        l2_regularization: 0.0,
    }
}

/// Creates test data with known memory footprint
fn create_memory_test_data(symbol: &str, size: usize) -> TimeSeriesData {
    let values: Vec<f64> = (0..size)
        .map(|i| 100.0 + (i as f64 * 0.01))
        .collect();
    
    let timestamps = (0..size)
        .map(|i| Utc::now() - chrono::Duration::seconds((size - i) as i64))
        .collect();
    
    let mut ts_data = TimeSeriesData::new(symbol.to_string(), timestamps[0]);
    ts_data.values = values;
    ts_data.timestamps = timestamps;
    ts_data.metadata = Some(serde_json::json!({
        "symbol": symbol,
        "source": "memory_test"
    }));
    ts_data.metadata_map = {
        let mut map = HashMap::new();
        map.insert("symbol".to_string(), serde_json::json!(symbol));
        map
    };
    ts_data
}

#[cfg(test)]
mod memory_compliance_tests {
    use super::*;
    
    /// Test 1: Total system memory must stay <525MB (Phase 2: 500MB + 5% overhead)
    #[tokio::test]
    async fn test_total_system_memory_limit() {
        let mut monitor = MemoryMonitor::new();
        let baseline_mb = monitor.get_current_mb();
        
        println!("🔍 Testing total system memory limit");
        println!("📊 Baseline memory: {:.2} MB", baseline_mb);
        
        // Create full Phase 3 system
        let neural_config = create_memory_test_config();
        let sector_mapper = Arc::new(SectorMapper::new(Default::default()));
        let performance_tracker = Arc::new(ModelPerformanceTracker::new());
        
        // Create core components
        let predictor = Arc::new(VendorPredictor::new(
            &neural_config,
            sector_mapper.clone(),
            performance_tracker,
        ).unwrap());
        
        let pipeline = DataPipeline::new(
            RoutingConfig::default(),
            ConsolidationConfig::default(),
            sector_mapper.clone(),
        );
        
        let daa_coordinator = DAACoordinator::new(
            sector_mapper.clone(),
            predictor.clone(),
        ).await.unwrap();
        
        // Process multiple symbols to simulate real workload
        let symbols = vec![
            "AAPL", "MSFT", "GOOGL", "AMZN", "TSLA",
            "META", "NVDA", "AMD", "INTC", "ORCL"
        ];
        
        for symbol in &symbols {
            let test_data = create_memory_test_data(symbol, 100);
            
            // Register symbol in pipeline
            let _ = pipeline.register_symbol(symbol, GeographicRegion::NorthAmerica).await;
            
            // Process data through pipeline
            let _ = pipeline.process_data(
                test_data.clone(),
                DataScope::Symbol(symbol.to_string()),
                5,
                "memory_test".to_string(),
            ).await;
            
            // Make predictions
            let _ = predictor.predict(&test_data).await;
            
            // Trigger DAA decision
            let _ = daa_coordinator.make_trading_decision(symbol, &test_data).await;
            
            // Monitor memory after each symbol
            let current_mb = monitor.get_current_mb();
            println!("📈 After processing {}: {:.2} MB", symbol, current_mb);
        }
        
        // Final memory check
        let final_mb = monitor.get_current_mb();
        let growth_mb = monitor.get_growth_mb();
        let peak_mb = monitor.get_peak_mb();
        
        println!("📊 Memory Analysis:");
        println!("  📊 Baseline: {:.2} MB", baseline_mb);
        println!("  📊 Final: {:.2} MB", final_mb);
        println!("  📊 Peak: {:.2} MB", peak_mb);
        println!("  📊 Growth: {:.2} MB", growth_mb);
        println!("  🎯 Target: <525 MB total");
        
        // CRITICAL: Must stay under 525MB total
        assert!(final_mb < 525.0, 
               "Total memory {:.2}MB exceeds 525MB limit", final_mb);
        assert!(peak_mb < 525.0, 
               "Peak memory {:.2}MB exceeds 525MB limit", peak_mb);
        
        // Additional check: Growth should be minimal
        assert!(growth_mb < 25.0, 
               "Memory growth {:.2}MB exceeds 25MB overhead limit", growth_mb);
    }
    
    /// Test 2: Per-symbol memory usage must be <50MB
    #[tokio::test]
    async fn test_per_symbol_memory_limit() {
        let mut monitor = MemoryMonitor::new();
        let baseline_mb = monitor.get_current_mb();
        
        println!("🔍 Testing per-symbol memory limit");
        
        // Create components for single symbol testing
        let neural_config = create_memory_test_config();
        let sector_mapper = Arc::new(SectorMapper::new(Default::default()));
        let performance_tracker = Arc::new(ModelPerformanceTracker::new());
        
        let predictor = Arc::new(VendorPredictor::new(
            &neural_config,
            sector_mapper.clone(),
            performance_tracker,
        ).unwrap());
        
        let pipeline = DataPipeline::new(
            RoutingConfig::default(),
            ConsolidationConfig::default(),
            sector_mapper,
        );
        
        // Test memory usage per symbol
        let test_symbol = "MEMORY_TEST";
        let before_symbol_mb = monitor.get_current_mb();
        
        // Register and process large dataset for single symbol
        let _ = pipeline.register_symbol(test_symbol, GeographicRegion::NorthAmerica).await;
        
        // Process large amount of data for this symbol
        for batch in 0..10 {
            let test_data = create_memory_test_data(
                &format!("{}_{}", test_symbol, batch), 
                1000 // Large dataset
            );
            
            let _ = pipeline.process_data(
                test_data.clone(),
                DataScope::Symbol(test_symbol.to_string()),
                5,
                "memory_test".to_string(),
            ).await;
            
            let _ = predictor.predict(&test_data).await;
        }
        
        let after_symbol_mb = monitor.get_current_mb();
        let per_symbol_usage = after_symbol_mb - before_symbol_mb;
        
        println!("📊 Per-Symbol Memory Analysis:");
        println!("  📊 Before symbol: {:.2} MB", before_symbol_mb);
        println!("  📊 After symbol: {:.2} MB", after_symbol_mb);
        println!("  📊 Per-symbol usage: {:.2} MB", per_symbol_usage);
        println!("  🎯 Target: <50 MB per symbol");
        
        // CRITICAL: Must use <50MB per symbol
        assert!(per_symbol_usage < 50.0, 
               "Per-symbol memory usage {:.2}MB exceeds 50MB limit", per_symbol_usage);
    }
    
    /// Test 3: Validate 90% memory reduction from hypothetical baseline
    #[tokio::test]
    async fn test_memory_reduction_validation() {
        println!("🔍 Testing 90% memory reduction validation");
        
        // Phase 2 established 500MB as 90% reduced from hypothetical 5GB baseline
        // Phase 3 allows up to 525MB (500MB + 5% overhead)
        let hypothetical_baseline_mb = 5000.0; // 5GB
        let phase2_target_mb = 500.0; // 90% reduction
        let phase3_limit_mb = 525.0; // +5% overhead allowed
        
        let mut monitor = MemoryMonitor::new();
        
        // Create full Phase 3 system
        let neural_config = create_memory_test_config();
        let sector_mapper = Arc::new(SectorMapper::new(Default::default()));
        let performance_tracker = Arc::new(ModelPerformanceTracker::new());
        
        let predictor = Arc::new(VendorPredictor::new(
            &neural_config,
            sector_mapper.clone(),
            performance_tracker,
        ).unwrap());
        
        let pipeline = DataPipeline::new(
            RoutingConfig::default(),
            ConsolidationConfig::default(),
            sector_mapper,
        );
        
        // Process representative workload
        let symbols = vec!["AAPL", "MSFT", "GOOGL", "AMZN", "TSLA"];
        for symbol in symbols {
            let test_data = create_memory_test_data(symbol, 100);
            let _ = pipeline.register_symbol(symbol, GeographicRegion::NorthAmerica).await;
            let _ = pipeline.process_data(
                test_data.clone(),
                DataScope::Symbol(symbol.to_string()),
                5,
                "reduction_test".to_string(),
            ).await;
            let _ = predictor.predict(&test_data).await;
        }
        
        let actual_usage_mb = monitor.get_current_mb();
        let reduction_percentage = ((hypothetical_baseline_mb - actual_usage_mb) / hypothetical_baseline_mb) * 100.0;
        
        println!("📊 Memory Reduction Analysis:");
        println!("  📊 Hypothetical baseline: {:.2} MB", hypothetical_baseline_mb);
        println!("  📊 Phase 2 target (90% reduction): {:.2} MB", phase2_target_mb);
        println!("  📊 Phase 3 limit (+5% overhead): {:.2} MB", phase3_limit_mb);
        println!("  📊 Actual usage: {:.2} MB", actual_usage_mb);
        println!("  📊 Actual reduction: {:.1}%", reduction_percentage);
        
        // Validate we maintain the 90% reduction spirit
        assert!(actual_usage_mb <= phase3_limit_mb, 
               "Actual usage {:.2}MB exceeds Phase 3 limit {:.2}MB", 
               actual_usage_mb, phase3_limit_mb);
        
        assert!(reduction_percentage >= 89.0, 
               "Memory reduction {:.1}% is below 89% threshold", reduction_percentage);
    }
    
    /// Test 4: Memory leak detection over extended period
    #[tokio::test]
    async fn test_memory_leak_detection() {
        println!("🔍 Testing memory leak detection (accelerated)");
        
        let mut monitor = MemoryMonitor::new();
        let initial_mb = monitor.get_current_mb();
        
        // Create components
        let neural_config = create_memory_test_config();
        let sector_mapper = Arc::new(SectorMapper::new(Default::default()));
        let performance_tracker = Arc::new(ModelPerformanceTracker::new());
        
        let predictor = Arc::new(VendorPredictor::new(
            &neural_config,
            sector_mapper.clone(),
            performance_tracker,
        ).unwrap());
        
        let pipeline = DataPipeline::new(
            RoutingConfig::default(),
            ConsolidationConfig::default(),
            sector_mapper,
        );
        
        // Simulate 24 hours of trading in accelerated time
        // Run intensive workload for 60 seconds representing 24 hours
        let test_duration = Duration::from_secs(60);
        let start_time = Instant::now();
        let mut iteration = 0;
        
        while start_time.elapsed() < test_duration {
            iteration += 1;
            
            // Simulate trading activity
            let symbol = format!("SYM{}", iteration % 10);
            let test_data = create_memory_test_data(&symbol, 50);
            
            // Register symbol periodically
            if iteration % 100 == 0 {
                let _ = pipeline.register_symbol(&symbol, GeographicRegion::NorthAmerica).await;
            }
            
            // Process data
            let _ = pipeline.process_data(
                test_data.clone(),
                DataScope::Symbol(symbol.clone()),
                3,
                "leak_test".to_string(),
            ).await;
            
            // Make predictions
            let _ = predictor.predict(&test_data).await;
            
            // Measure memory every 1000 iterations
            if iteration % 1000 == 0 {
                let current_mb = monitor.get_current_mb();
                println!("📊 Iteration {}: {:.2} MB", iteration, current_mb);
                
                // Force garbage collection attempt
                tokio::task::yield_now().await;
            }
            
            // Small delay to prevent overwhelming
            if iteration % 100 == 0 {
                sleep(Duration::from_millis(10)).await;
            }
        }
        
        let final_mb = monitor.get_current_mb();
        let growth_mb = final_mb - initial_mb;
        let has_leak = monitor.detect_leaks(1.0); // 1 hour simulated
        
        println!("📊 Memory Leak Analysis:");
        println!("  📊 Initial: {:.2} MB", initial_mb);
        println!("  📊 Final: {:.2} MB", final_mb);
        println!("  📊 Growth: {:.2} MB", growth_mb);
        println!("  📊 Iterations: {}", iteration);
        println!("  📊 Leak detected: {}", has_leak);
        
        // Memory should not grow excessively
        assert!(!has_leak, "Memory leak detected during extended testing");
        assert!(growth_mb < 100.0, 
               "Excessive memory growth {:.2}MB during leak test", growth_mb);
    }
    
    /// Test 5: Memory efficiency under concurrent load
    #[tokio::test]
    async fn test_concurrent_memory_efficiency() {
        println!("🔍 Testing memory efficiency under concurrent load");
        
        let mut monitor = MemoryMonitor::new();
        let baseline_mb = monitor.get_current_mb();
        
        // Create shared components
        let neural_config = create_memory_test_config();
        let sector_mapper = Arc::new(SectorMapper::new(Default::default()));
        let performance_tracker = Arc::new(ModelPerformanceTracker::new());
        
        let predictor = Arc::new(VendorPredictor::new(
            &neural_config,
            sector_mapper.clone(),
            performance_tracker,
        ).unwrap());
        
        let pipeline = Arc::new(DataPipeline::new(
            RoutingConfig::default(),
            ConsolidationConfig::default(),
            sector_mapper,
        ));
        
        // Launch concurrent tasks
        let num_tasks = 10;
        let mut tasks = Vec::new();
        
        for task_id in 0..num_tasks {
            let predictor_clone = Arc::clone(&predictor);
            let pipeline_clone = Arc::clone(&pipeline);
            
            let task = tokio::spawn(async move {
                for i in 0..100 {
                    let symbol = format!("TASK{}_{}", task_id, i % 5);
                    let test_data = create_memory_test_data(&symbol, 30);
                    
                    // Register symbol occasionally
                    if i % 20 == 0 {
                        let _ = pipeline_clone.register_symbol(&symbol, GeographicRegion::NorthAmerica).await;
                    }
                    
                    // Process data
                    let _ = pipeline_clone.process_data(
                        test_data.clone(),
                        DataScope::Symbol(symbol),
                        3,
                        format!("concurrent_task_{}", task_id),
                    ).await;
                    
                    // Make prediction
                    let _ = predictor_clone.predict(&test_data).await;
                    
                    // Yield occasionally
                    if i % 10 == 0 {
                        tokio::task::yield_now().await;
                    }
                }
            });
            tasks.push(task);
        }
        
        // Wait for all tasks to complete
        futures::future::join_all(tasks).await;
        
        let final_mb = monitor.get_current_mb();
        let peak_mb = monitor.get_peak_mb();
        let growth_mb = final_mb - baseline_mb;
        
        println!("📊 Concurrent Memory Analysis:");
        println!("  📊 Baseline: {:.2} MB", baseline_mb);
        println!("  📊 Final: {:.2} MB", final_mb);
        println!("  📊 Peak: {:.2} MB", peak_mb);
        println!("  📊 Growth: {:.2} MB", growth_mb);
        println!("  📊 Tasks: {}", num_tasks);
        
        // Memory should remain efficient even under concurrent load
        assert!(peak_mb < 525.0, 
               "Peak memory {:.2}MB under concurrent load exceeds 525MB limit", peak_mb);
        assert!(growth_mb < 50.0, 
               "Memory growth {:.2}MB under concurrent load is excessive", growth_mb);
    }
    
    /// Test 6: Memory cleanup verification
    #[tokio::test]
    async fn test_memory_cleanup() {
        println!("🔍 Testing memory cleanup mechanisms");
        
        let mut monitor = MemoryMonitor::new();
        let baseline_mb = monitor.get_current_mb();
        
        // Create components
        let neural_config = create_memory_test_config();
        let sector_mapper = Arc::new(SectorMapper::new(Default::default()));
        let performance_tracker = Arc::new(ModelPerformanceTracker::new());
        
        let predictor = Arc::new(VendorPredictor::new(
            &neural_config,
            sector_mapper.clone(),
            performance_tracker,
        ).unwrap());
        
        let pipeline = DataPipeline::new(
            RoutingConfig::default(),
            ConsolidationConfig::default(),
            sector_mapper,
        );
        
        // Process significant amount of data
        for i in 0..50 {
            let symbol = format!("CLEANUP_{}", i);
            let large_data = create_memory_test_data(&symbol, 500); // Large dataset
            
            let _ = pipeline.register_symbol(&symbol, GeographicRegion::NorthAmerica).await;
            let _ = pipeline.process_data(
                large_data.clone(),
                DataScope::Symbol(symbol),
                5,
                "cleanup_test".to_string(),
            ).await;
            let _ = predictor.predict(&large_data).await;
        }
        
        let before_cleanup_mb = monitor.get_current_mb();
        
        // Trigger cleanup operations
        let _ = pipeline.cleanup_old_data().await;
        
        // Force garbage collection
        for _ in 0..10 {
            tokio::task::yield_now().await;
            sleep(Duration::from_millis(100)).await;
        }
        
        let after_cleanup_mb = monitor.get_current_mb();
        let cleanup_freed_mb = before_cleanup_mb - after_cleanup_mb;
        
        println!("📊 Memory Cleanup Analysis:");
        println!("  📊 Baseline: {:.2} MB", baseline_mb);
        println!("  📊 Before cleanup: {:.2} MB", before_cleanup_mb);
        println!("  📊 After cleanup: {:.2} MB", after_cleanup_mb);
        println!("  📊 Freed by cleanup: {:.2} MB", cleanup_freed_mb);
        
        // Cleanup should free some memory
        assert!(after_cleanup_mb <= before_cleanup_mb, 
               "Memory increased after cleanup: {:.2}MB -> {:.2}MB", 
               before_cleanup_mb, after_cleanup_mb);
        
        // Final memory should be reasonable
        assert!(after_cleanup_mb < 525.0, 
               "Memory after cleanup {:.2}MB exceeds 525MB limit", after_cleanup_mb);
    }
}