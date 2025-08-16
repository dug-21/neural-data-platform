//! Memory Budget Compliance Tests
//!
//! Ensures Phase 3 system operates within defined memory constraints

use anyhow::Result;
use std::sync::Arc;
use tracing_test::traced_test;

use neural_trader::integration::daa_coordinator::DaaCoordinator;
use neural_trader::neural::NeuralPredictor;

use crate::phase3::utilities::*;
use crate::phase3::fixtures::*;

#[traced_test]
#[tokio::test]
async fn test_strict_memory_budget_compliance() -> Result<()> {
    let memory_tracker = MemoryTracker::new(256); // Strict 256MB budget
    
    // Initialize core components
    let predictor = create_test_neural_predictor(Some(create_test_neural_config())).await?;
    assert!(memory_tracker.check_budget_compliance().await?);
    
    let market_hours = create_test_market_hours();
    let coordinator = DaaCoordinator::new(Arc::clone(&predictor), market_hours).await?;
    assert!(memory_tracker.check_budget_compliance().await?);
    
    // Process realistic data load
    let timestamp = chrono::Utc::now();
    for symbol in TEST_SYMBOLS {
        let base_price = get_market_cap_for_symbol(symbol) / 10.0; // Realistic price
        let data = create_realistic_time_series_data(symbol, base_price, timestamp);
        
        let _prediction = predictor.predict(&data).await?;
        let _decision = coordinator.process_market_data(&data).await?;
        
        // Check memory after each symbol
        assert!(memory_tracker.check_budget_compliance().await?);
    }
    
    let final_usage = memory_tracker.get_memory_usage_mb().await;
    println!("Final memory usage: {}MB (budget: 256MB)", final_usage);
    assert!(final_usage <= 256, "Memory budget exceeded: {}MB > 256MB", final_usage);
    
    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_memory_efficiency_under_load() -> Result<()> {
    let memory_tracker = MemoryTracker::new(512); // 512MB budget
    
    // Create multiple predictors and coordinators
    let mut components = Vec::new();
    for i in 0..10 {
        let config = NeuralConfig {
            model_path: format!("test_model_{}", i),
            ..create_test_neural_config()
        };
        let predictor = create_test_neural_predictor(Some(config)).await?;
        let market_hours = create_test_market_hours();
        let coordinator = DaaCoordinator::new(Arc::clone(&predictor), market_hours).await?;
        
        components.push((predictor, coordinator));
        
        // Check memory after each component
        assert!(memory_tracker.check_budget_compliance().await?);
    }
    
    // Simulate high-frequency trading load
    let timestamp = chrono::Utc::now();
    for round in 0..50 {
        for (i, (predictor, coordinator)) in components.iter().enumerate() {
            let symbol = TEST_SYMBOLS[i % TEST_SYMBOLS.len()];
            let base_price = 100.0 + (round as f64 * 0.5);
            let data = create_realistic_time_series_data(symbol, base_price, timestamp);
            
            let _prediction = predictor.predict(&data).await?;
            let _decision = coordinator.process_market_data(&data).await?;
        }
        
        // Check memory every 10 rounds
        if round % 10 == 0 {
            assert!(memory_tracker.check_budget_compliance().await?);
            let usage = memory_tracker.get_memory_usage_mb().await;
            println!("Memory usage at round {}: {}MB", round, usage);
        }
    }
    
    let final_usage = memory_tracker.get_memory_usage_mb().await;
    assert!(final_usage <= 512, "Memory budget exceeded under load: {}MB > 512MB", final_usage);
    
    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_memory_leak_detection() -> Result<()> {
    let memory_tracker = MemoryTracker::new(1024); // Large budget for leak detection
    
    let predictor = create_test_neural_predictor(None).await?;
    let market_hours = create_test_market_hours();
    let coordinator = DaaCoordinator::new(Arc::clone(&predictor), market_hours).await?;
    
    let baseline_usage = memory_tracker.get_memory_usage_mb().await;
    println!("Baseline memory usage: {}MB", baseline_usage);
    
    // Perform many operations to detect leaks
    let timestamp = chrono::Utc::now();
    for iteration in 0..1000 {
        let symbol = TEST_SYMBOLS[iteration % TEST_SYMBOLS.len()];
        let base_price = 100.0 + (iteration as f64 * 0.01);
        let data = create_realistic_time_series_data(symbol, base_price, timestamp);
        
        // Create predictions and decisions
        let _prediction = predictor.predict(&data).await?;
        let _decision = coordinator.process_market_data(&data).await?;
        
        // Check for memory leaks every 100 iterations
        if iteration % 100 == 0 && iteration > 0 {
            let current_usage = memory_tracker.get_memory_usage_mb().await;
            let growth = current_usage as f64 / baseline_usage as f64;
            
            println!("Iteration {}: {}MB (growth: {:.2}x)", iteration, current_usage, growth);
            
            // Memory should not grow significantly (allowing for some caching)
            assert!(growth < 2.0, "Potential memory leak detected: {:.2}x growth", growth);
        }
    }
    
    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_concurrent_memory_usage() -> Result<()> {
    let memory_tracker = MemoryTracker::new(768); // 768MB budget
    
    let predictor = Arc::new(create_test_neural_predictor(None).await?);
    let market_hours = create_test_market_hours();
    let coordinator = Arc::new(DaaCoordinator::new(Arc::clone(&predictor), market_hours).await?);
    
    // Launch concurrent tasks
    let mut handles = Vec::new();
    for i in 0..20 {
        let predictor_clone = Arc::clone(&predictor);
        let coordinator_clone = Arc::clone(&coordinator);
        let timestamp = chrono::Utc::now();
        
        let handle = tokio::spawn(async move {
            for j in 0..50 {
                let symbol = TEST_SYMBOLS[j % TEST_SYMBOLS.len()];
                let base_price = 100.0 + (i as f64 * j as f64 * 0.01);
                let data = create_realistic_time_series_data(symbol, base_price, timestamp);
                
                let _prediction = predictor_clone.predict(&data).await?;
                let _decision = coordinator_clone.process_market_data(&data).await?;
            }
            Ok::<(), anyhow::Error>(())
        });
        handles.push(handle);
    }
    
    // Monitor memory during concurrent execution
    let monitor_handle = {
        let memory_tracker = MemoryTracker::new(768);
        tokio::spawn(async move {
            for _ in 0..30 { // Monitor for 30 seconds
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                let usage = memory_tracker.get_memory_usage_mb().await;
                if usage > 768 {
                    return Err(anyhow::anyhow!("Memory budget exceeded during concurrent execution: {}MB", usage));
                }
            }
            Ok(())
        })
    };
    
    // Wait for all tasks to complete
    for handle in handles {
        handle.await??;
    }
    
    // Check final memory state
    monitor_handle.await??;
    assert!(memory_tracker.check_budget_compliance().await?);
    
    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_memory_optimization_effectiveness() -> Result<()> {
    // Test with optimization disabled vs enabled
    let memory_tracker_unoptimized = MemoryTracker::new(1024);
    let memory_tracker_optimized = MemoryTracker::new(512);
    
    // Unoptimized configuration
    let config_unoptimized = NeuralConfig {
        model_path: "unoptimized_model".to_string(),
        batch_size: 1, // Inefficient small batches
        ..create_test_neural_config()
    };
    
    // Optimized configuration
    let config_optimized = NeuralConfig {
        model_path: "optimized_model".to_string(),
        batch_size: 64, // Efficient larger batches
        ..create_test_neural_config()
    };
    
    // Test unoptimized
    let predictor_unoptimized = create_test_neural_predictor(Some(config_unoptimized)).await?;
    let timestamp = chrono::Utc::now();
    
    for symbol in TEST_SYMBOLS {
        let data = create_realistic_time_series_data(symbol, 100.0, timestamp);
        let _prediction = predictor_unoptimized.predict(&data).await?;
    }
    
    let unoptimized_usage = memory_tracker_unoptimized.get_memory_usage_mb().await;
    
    // Test optimized
    let predictor_optimized = create_test_neural_predictor(Some(config_optimized)).await?;
    
    for symbol in TEST_SYMBOLS {
        let data = create_realistic_time_series_data(symbol, 100.0, timestamp);
        let _prediction = predictor_optimized.predict(&data).await?;
    }
    
    let optimized_usage = memory_tracker_optimized.get_memory_usage_mb().await;
    
    println!("Unoptimized memory usage: {}MB", unoptimized_usage);
    println!("Optimized memory usage: {}MB", optimized_usage);
    
    // Optimized should use less memory
    assert!(optimized_usage <= unoptimized_usage, 
        "Optimization not effective: optimized={}MB vs unoptimized={}MB", 
        optimized_usage, unoptimized_usage);
    
    // Both should be within reasonable bounds
    assert!(optimized_usage <= 512);
    assert!(memory_tracker_optimized.check_budget_compliance().await?);
    
    Ok(())
}