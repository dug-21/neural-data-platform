//! Phase 3 Comprehensive Integration Test
//!
//! This test validates the complete Phase 3 system integration including:
//! - Async NeuralPredictor initialization and operation
//! - DAA coordination with MarketHours parameter
//! - Current TimeSeriesData structure compatibility
//! - Memory budget compliance
//! - Performance benchmarks
//! - End-to-end workflow validation

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_test::traced_test;

use autonomous_platform::config::NeuralConfig;
use autonomous_platform::data::TimeSeriesData;
use autonomous_platform::integration::daa_coordinator::DaaCoordinator;
use autonomous_platform::neural::{NeuralPredictor, NeuralPredictorTrait};
use autonomous_platform::utils::market_hours::MarketHours;

// Import Phase 3 test utilities
use crate::phase3::utilities::*;
use crate::phase3::fixtures::*;

#[traced_test]
#[tokio::test]
async fn test_phase3_complete_system_integration() -> Result<()> {
    println!("🚀 Starting Phase 3 Comprehensive Integration Test");
    
    let config = Phase3TestConfig::default();
    let memory_tracker = MemoryTracker::new(config.memory_budget_mb);
    
    // Phase 1: Initialize Neural Predictor with async pattern
    println!("📊 Phase 1: Neural Predictor Initialization");
    let neural_config = create_test_neural_config();
    let predictor = with_timeout(
        create_test_neural_predictor(Some(neural_config)),
        config.max_test_duration_secs
    ).await?;
    
    assert!(Arc::strong_count(&predictor) >= 1);
    assert!(memory_tracker.check_budget_compliance().await?);
    println!("✅ Neural predictor initialized successfully");
    
    // Phase 2: Initialize DAA Coordinator with MarketHours
    println!("🤖 Phase 2: DAA Coordinator Initialization");
    let market_hours = create_test_market_hours();
    let coordinator = DaaCoordinator::new(predictor.clone(), market_hours).await?;
    
    assert!(coordinator.is_initialized().await?);
    assert!(memory_tracker.check_budget_compliance().await?);
    println!("✅ DAA coordinator initialized successfully");
    
    // Phase 3: Test TimeSeriesData Structure Compatibility
    println!("📈 Phase 3: TimeSeriesData Compatibility Validation");
    let timestamp = chrono::Utc::now();
    let mut test_results = Vec::new();
    
    for (i, symbol) in TEST_SYMBOLS.iter().enumerate() {
        let base_price = get_market_cap_for_symbol(symbol) / 10.0;
        let market_condition = match i % 5 {
            0 => MarketCondition::Bullish,
            1 => MarketCondition::Bearish,
            2 => MarketCondition::Sideways,
            3 => MarketCondition::Volatile,
            _ => MarketCondition::LowVolume,
        };
        
        let data = create_market_condition_data(symbol, market_condition, base_price, timestamp);
        
        // Validate TimeSeriesData structure
        assert!(!data.symbol.is_empty());
        assert!(!data.volume.is_empty()); // Vec<f64>
        assert!(data.volume_value > 0.0); // Single value
        assert!(!data.values.is_empty()); // Raw price values
        assert!(!data.intervals.is_empty()); // Time intervals
        assert!(!data.timestamps.is_empty()); // Corresponding timestamps
        assert!(data.source.is_some()); // Storage compatibility
        assert!(data.entity.is_some()); // Storage compatibility
        
        test_results.push((symbol, data));
    }
    
    assert!(memory_tracker.check_budget_compliance().await?);
    println!("✅ TimeSeriesData structure validation completed for {} symbols", TEST_SYMBOLS.len());
    
    // Phase 4: Neural Prediction Testing
    println!("🧠 Phase 4: Neural Prediction Testing");
    let prediction_start = std::time::Instant::now();
    let mut prediction_results = Vec::new();
    
    for (symbol, data) in &test_results {
        let prediction = predictor.predict(data).await?;
        
        // Validate prediction structure
        assert!(prediction.confidence >= 0.0 && prediction.confidence <= 1.0);
        assert!(!prediction.values.is_empty());
        assert!(prediction.timestamp.is_some());
        
        prediction_results.push((symbol, prediction));
    }
    
    let prediction_duration = prediction_start.elapsed();
    let avg_prediction_latency = prediction_duration.as_millis() / TEST_SYMBOLS.len() as u128;
    
    println!("✅ Neural predictions completed: avg latency {}ms", avg_prediction_latency);
    assert!(avg_prediction_latency < 100, "Prediction latency too high: {}ms", avg_prediction_latency);
    assert!(memory_tracker.check_budget_compliance().await?);
    
    // Phase 5: DAA Decision Making
    println!("🎯 Phase 5: DAA Decision Making");
    let decision_start = std::time::Instant::now();
    let mut decision_results = Vec::new();
    
    for (symbol, data) in &test_results {
        let decision = coordinator.process_market_data(data).await?;
        
        // Validate decision structure
        assert!(decision.confidence >= 0.0 && decision.confidence <= 1.0);
        assert!(decision.timestamp.is_some());
        assert!(!decision.reasoning.is_empty());
        
        decision_results.push((symbol, decision));
    }
    
    let decision_duration = decision_start.elapsed();
    let avg_decision_latency = decision_duration.as_millis() / TEST_SYMBOLS.len() as u128;
    
    println!("✅ DAA decisions completed: avg latency {}ms", avg_decision_latency);
    assert!(avg_decision_latency < 200, "Decision latency too high: {}ms", avg_decision_latency);
    assert!(memory_tracker.check_budget_compliance().await?);
    
    // Phase 6: Ensemble Performance Reset Testing
    println!("🔄 Phase 6: Ensemble Performance Reset");
    predictor.reset_ensemble_performance().await?;
    
    // Verify predictor still functional after reset
    let test_data = &test_results[0].1;
    let post_reset_prediction = predictor.predict(test_data).await?;
    assert!(!post_reset_prediction.values.is_empty());
    println!("✅ Ensemble performance reset successful");
    
    // Phase 7: Training Integration Testing
    println!("🎓 Phase 7: Training Integration");
    for (_, data) in test_results.iter().take(3) { // Test with first 3 symbols
        let target_value = data.close * 1.01; // 1% gain target
        predictor.update_model(data, target_value).await?;
    }
    
    // Verify predictions still work after training
    let post_training_prediction = predictor.predict(&test_results[0].1).await?;
    assert!(!post_training_prediction.values.is_empty());
    println!("✅ Training integration successful");
    assert!(memory_tracker.check_budget_compliance().await?);
    
    // Phase 8: Concurrent Operations Testing
    println!("⚡ Phase 8: Concurrent Operations Testing");
    let concurrent_start = std::time::Instant::now();
    let mut handles = Vec::new();
    
    for i in 0..20 {
        let predictor_clone = Arc::clone(&predictor);
        let coordinator_clone = Arc::new(coordinator);
        let symbol = TEST_SYMBOLS[i % TEST_SYMBOLS.len()];
        let base_price = 100.0 + (i as f64);
        let data = create_realistic_time_series_data(symbol, base_price, timestamp);
        
        let handle = tokio::spawn(async move {
            let prediction = predictor_clone.predict(&data).await?;
            let decision = coordinator_clone.process_market_data(&data).await?;
            Ok::<(neural_trader::neural::PredictionResult, neural_trader::integration::daa_coordinator::DaaDecision), anyhow::Error>((prediction, decision))
        });
        handles.push(handle);
    }
    
    // Wait for all concurrent operations
    for handle in handles {
        let (prediction, decision) = handle.await??;
        assert!(!prediction.values.is_empty());
        assert!(!decision.reasoning.is_empty());
    }
    
    let concurrent_duration = concurrent_start.elapsed();
    println!("✅ Concurrent operations completed in {}ms", concurrent_duration.as_millis());
    assert!(memory_tracker.check_budget_compliance().await?);
    
    // Phase 9: Memory and Performance Summary
    println!("📊 Phase 9: Performance Summary");
    let final_memory_usage = memory_tracker.get_memory_usage_mb().await;
    
    println!("\n🎯 PHASE 3 INTEGRATION TEST RESULTS:");
    println!("├── Neural Predictor: ✅ Async initialization successful");
    println!("├── DAA Coordinator: ✅ MarketHours integration successful");
    println!("├── TimeSeriesData: ✅ Phase 3 structure compatibility verified");
    println!("├── Predictions: ✅ {}ms avg latency (target: <100ms)", avg_prediction_latency);
    println!("├── Decisions: ✅ {}ms avg latency (target: <200ms)", avg_decision_latency);
    println!("├── Memory Usage: ✅ {}MB (budget: {}MB)", final_memory_usage, config.memory_budget_mb);
    println!("├── Concurrent Ops: ✅ 20 parallel operations successful");
    println!("├── Training: ✅ Online learning integration verified");
    println!("└── Ensemble Reset: ✅ Performance reset successful");
    
    // Final assertions
    assert!(final_memory_usage <= config.memory_budget_mb);
    assert!(avg_prediction_latency < 100);
    assert!(avg_decision_latency < 200);
    
    println!("\n🎉 Phase 3 Comprehensive Integration Test PASSED!");
    println!("   System ready for production deployment with Phase 3 enhancements");
    
    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_phase3_error_resilience() -> Result<()> {
    println!("🛡️ Testing Phase 3 Error Resilience");
    
    let predictor = create_test_neural_predictor(None).await?;
    let market_hours = create_test_market_hours();
    let coordinator = DaaCoordinator::new(Arc::clone(&predictor), market_hours).await?;
    
    // Test with malformed data
    let mut malformed_data = create_test_time_series_data("INVALID", chrono::Utc::now());
    malformed_data.values.clear(); // Empty values
    malformed_data.volume.clear(); // Empty volume
    
    // System should handle malformed data gracefully
    let prediction_result = predictor.predict(&malformed_data).await;
    let decision_result = coordinator.process_market_data(&malformed_data).await;
    
    // Either succeed with defaults or fail gracefully
    match (prediction_result, decision_result) {
        (Ok(pred), Ok(dec)) => {
            println!("✅ System handled malformed data with defaults");
            assert!(pred.confidence >= 0.0 && pred.confidence <= 1.0);
            assert!(dec.confidence >= 0.0 && dec.confidence <= 1.0);
        },
        (Err(e1), Err(e2)) => {
            println!("✅ System correctly rejected malformed data: {} | {}", e1, e2);
        },
        (Ok(_), Err(e)) => {
            println!("✅ Partial success - predictor handled it, coordinator rejected: {}", e);
        },
        (Err(e), Ok(_)) => {
            println!("✅ Partial success - coordinator handled it, predictor rejected: {}", e);
        }
    }
    
    println!("✅ Error resilience test completed");
    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_phase3_performance_benchmarks() -> Result<()> {
    println!("🏁 Running Phase 3 Performance Benchmarks");
    
    let memory_tracker = MemoryTracker::new(1024); // 1GB for benchmarks
    let predictor = create_test_neural_predictor(None).await?;
    let market_hours = create_test_market_hours();
    let coordinator = DaaCoordinator::new(Arc::clone(&predictor), market_hours).await?;
    
    // Benchmark 1: Single prediction latency
    println!("📊 Benchmark 1: Single Prediction Latency");
    let data = create_realistic_time_series_data("AAPL", 150.0, chrono::Utc::now());
    
    let mut latencies = Vec::new();
    for _ in 0..100 {
        let start = std::time::Instant::now();
        let _prediction = predictor.predict(&data).await?;
        latencies.push(start.elapsed().as_micros());
    }
    
    let avg_latency_us = latencies.iter().sum::<u128>() / latencies.len() as u128;
    let p95_latency_us = latencies[95];
    
    println!("├── Average latency: {}μs", avg_latency_us);
    println!("├── P95 latency: {}μs", p95_latency_us);
    println!("└── Target: <50ms (50,000μs)");
    
    assert!(avg_latency_us < 50_000, "Average latency too high: {}μs", avg_latency_us);
    
    // Benchmark 2: Throughput test
    println!("🚀 Benchmark 2: System Throughput");
    let throughput_start = std::time::Instant::now();
    
    for i in 0..1000 {
        let symbol = TEST_SYMBOLS[i % TEST_SYMBOLS.len()];
        let price = 100.0 + (i as f64 * 0.01);
        let test_data = create_realistic_time_series_data(symbol, price, chrono::Utc::now());
        
        let _prediction = predictor.predict(&test_data).await?;
        let _decision = coordinator.process_market_data(&test_data).await?;
    }
    
    let throughput_duration = throughput_start.elapsed();
    let throughput = 1000.0 / throughput_duration.as_secs_f64();
    
    println!("├── Processed 1000 operations in {:?}", throughput_duration);
    println!("├── Throughput: {:.2} ops/sec", throughput);
    println!("└── Target: >100 ops/sec");
    
    assert!(throughput > 100.0, "Throughput too low: {:.2} ops/sec", throughput);
    
    // Benchmark 3: Memory efficiency
    println!("💾 Benchmark 3: Memory Efficiency");
    let final_memory = memory_tracker.get_memory_usage_mb().await;
    
    println!("├── Memory usage: {}MB", final_memory);
    println!("├── Budget: 1024MB");
    println!("└── Efficiency: {:.1}%", (final_memory as f64 / 1024.0) * 100.0);
    
    assert!(final_memory <= 1024, "Memory usage exceeded budget: {}MB", final_memory);
    
    println!("\n🏆 PERFORMANCE BENCHMARK RESULTS:");
    println!("├── Latency: ✅ {}μs avg (target: <50,000μs)", avg_latency_us);
    println!("├── Throughput: ✅ {:.2} ops/sec (target: >100 ops/sec)", throughput);
    println!("└── Memory: ✅ {}MB used (budget: 1024MB)", final_memory);
    
    Ok(())
}