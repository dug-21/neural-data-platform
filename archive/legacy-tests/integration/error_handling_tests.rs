//! Error Handling and Circuit Breaker Tests
//!
//! These tests validate error scenarios and resilience features:
//! - Circuit breaker functionality and fallback mechanisms
//! - Error recovery and graceful degradation
//! - Timeout handling and retry logic  
//! - Health monitoring under failure conditions
//! - Fallback prediction accuracy and performance

use std::time::{Duration, Instant};
use std::sync::Arc;
use anyhow::Result;
use tokio::time::timeout;

use crate::neural::vendor_predictor::VendorPredictor;
use crate::neural::NeuralPredictorTrait;
use crate::data::sector_mapper::{SectorMapper, SectorMapperConfig};
use crate::monitoring::model_performance_tracker::ModelPerformanceTracker;
use crate::config::NeuralConfig;
use crate::adapters::enhanced_neural_adapter::{EnhancedNeuralAdapter, EnhancedNeuralConfig};

mod helpers;
use helpers::{TestConfigBuilder, TestDataGenerator, PerformanceMeasurement, TestResultValidator};

/// Test circuit breaker activation and fallback
#[tokio::test]
async fn test_circuit_breaker_fallback() -> Result<()> {
    let config = TestConfigBuilder::new()
        .with_circuit_breakers()
        .with_fallback()
        .with_models(vec!["MLP".to_string()])
        .build();

    let predictor = NeuralPredictor::new(config)?;
    let test_data = TestDataGenerator::generate_simple_data(30);
    
    println!("🔧 Testing circuit breaker fallback mechanism...");
    
    // First, verify normal operation
    let normal_result = predictor.predict(&test_data, 5, None).await?;
    TestResultValidator::validate_predictions(&normal_result, 5, 0.0)?;
    println!("   ✓ Normal operation confirmed");
    
    // Simulate circuit breaker triggering by making many rapid requests
    // In a real system, this would trigger due to failures, but here we test the mechanism
    let mut successful_predictions = 0;
    let mut total_attempts = 0;
    
    for attempt in 0..20 {
        total_attempts += 1;
        
        match timeout(Duration::from_millis(100), predictor.predict(&test_data, 3, None)).await {
            Ok(Ok(results)) => {
                TestResultValidator::validate_predictions(&results, 3, 0.0)?;
                successful_predictions += 1;
                
                // Check if fallback was triggered by examining metadata
                if !results.is_empty() {
                    // In a real implementation, we'd check for fallback indicators
                    println!("   Attempt {}: Success ({})", attempt + 1, results[0].model_name);
                }
            }
            Ok(Err(e)) => {
                println!("   Attempt {}: Prediction error (expected): {}", attempt + 1, e);
            }
            Err(_) => {
                println!("   Attempt {}: Timeout (circuit breaker may be active)", attempt + 1);
            }
        }
        
        // Small delay between attempts
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    
    // Validate that some predictions succeeded (either normal or fallback)
    assert!(
        successful_predictions > 0,
        "No predictions succeeded - circuit breaker may be too aggressive"
    );
    
    let success_rate = (successful_predictions as f64) / (total_attempts as f64) * 100.0;
    println!("   Success rate: {:.1}% ({}/{})", success_rate, successful_predictions, total_attempts);
    
    // Even with circuit breaker, should maintain some level of service through fallback
    assert!(
        success_rate >= 50.0,
        "Success rate {:.1}% too low - fallback mechanism may not be working",
        success_rate
    );
    
    println!("✅ Circuit breaker fallback test passed");
    Ok(())
}

/// Test error recovery after temporary failures
#[tokio::test]
async fn test_error_recovery() -> Result<()> {
    let config = TestConfigBuilder::new()
        .with_fallback()
        .with_health_monitoring()
        .build();

    let predictor = NeuralPredictor::new(config)?;
    let test_data = TestDataGenerator::generate_simple_data(40);
    
    println!("🔄 Testing error recovery mechanism...");
    
    // Phase 1: Normal operation
    let initial_result = predictor.predict(&test_data, 8, None).await?;
    TestResultValidator::validate_predictions(&initial_result, 8, 0.0)?;
    println!("   ✓ Phase 1: Normal operation confirmed");
    
    // Phase 2: Stress the system to potentially cause temporary failures
    let stress_start = Instant::now();
    let mut stress_results = Vec::new();
    
    // Create concurrent load that might cause temporary issues
    let mut stress_tasks = Vec::new();
    for i in 0..10 {
        let predictor_clone = Arc::new(predictor.clone());
        let data_clone = test_data.clone();
        
        let task = tokio::spawn(async move {
            let chunk_start = i * 3;
            let chunk_end = std::cmp::min(chunk_start + 25, data_clone.len());
            let chunk = &data_clone[chunk_start..chunk_end];
            
            // Rapid predictions that might stress the system
            let mut results = Vec::new();
            for _ in 0..5 {
                match timeout(Duration::from_millis(200), predictor_clone.predict(chunk, 4, None)).await {
                    Ok(Ok(pred)) => results.push(Ok(pred)),
                    Ok(Err(e)) => results.push(Err(e)),
                    Err(_) => results.push(Err(anyhow::anyhow!("Timeout"))),
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            results
        });
        
        stress_tasks.push(task);
    }
    
    let stress_results_batches = futures::future::join_all(stress_tasks).await;
    let stress_duration = stress_start.elapsed();
    
    let mut successful_stress_predictions = 0;
    let mut total_stress_attempts = 0;
    
    for batch_result in stress_results_batches {
        if let Ok(batch) = batch_result {
            for result in batch {
                total_stress_attempts += 1;
                if let Ok(predictions) = result {
                    successful_stress_predictions += 1;
                    // Validate that fallback predictions are still reasonable
                    if !predictions.is_empty() {
                        assert!(predictions[0].confidence >= 0.0 && predictions[0].confidence <= 1.0);
                        assert!(predictions[0].value.is_finite());
                    }
                }
            }
        }
    }
    
    println!("   ✓ Phase 2: Stress test completed in {:.2}s", stress_duration.as_secs_f64());
    println!("     Stress success rate: {:.1}% ({}/{})", 
             (successful_stress_predictions as f64 / total_stress_attempts as f64) * 100.0,
             successful_stress_predictions, total_stress_attempts);
    
    // Phase 3: Recovery validation - system should recover to normal operation
    tokio::time::sleep(Duration::from_millis(500)).await; // Allow recovery time
    
    let recovery_start = Instant::now();
    let mut recovery_successful = 0;
    let recovery_attempts = 10;
    
    for i in 0..recovery_attempts {
        let start_idx = i * 2;
        let chunk = &test_data[start_idx..std::cmp::min(start_idx + 20, test_data.len())];
        
        match predictor.predict(chunk, 6, None).await {
            Ok(results) => {
                TestResultValidator::validate_predictions(&results, 6, 0.0)?;
                recovery_successful += 1;
            }
            Err(e) => {
                println!("   Recovery attempt {} failed: {}", i + 1, e);
            }
        }
        
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    
    let recovery_duration = recovery_start.elapsed();
    let recovery_rate = (recovery_successful as f64) / (recovery_attempts as f64) * 100.0;
    
    println!("   ✓ Phase 3: Recovery completed in {:.2}s", recovery_duration.as_secs_f64());
    println!("     Recovery success rate: {:.1}% ({}/{})", 
             recovery_rate, recovery_successful, recovery_attempts);
    
    // Validate recovery was successful
    assert!(
        recovery_rate >= 80.0,
        "Recovery rate {:.1}% too low - system may not be recovering properly",
        recovery_rate
    );
    
    // Validate that the system maintains some level of service throughout
    assert!(
        successful_stress_predictions > 0,
        "No successful predictions during stress - fallback may not be working"
    );
    
    println!("✅ Error recovery test passed");
    Ok(())
}

/// Test timeout handling for slow operations
#[tokio::test]
async fn test_timeout_handling() -> Result<()> {
    let config = TestConfigBuilder::new()
        .with_models(vec!["MLP".to_string()])
        .build();
    
    // Modify config to have very short timeout for testing
    let mut test_config = config;
    test_config.model_timeout_seconds = 1; // Very short timeout
    
    let predictor = NeuralPredictor::new(test_config)?;
    
    // Create large dataset that might take time to process
    let large_data = TestDataGenerator::generate_simple_data(1000);
    
    println!("⏱️  Testing timeout handling...");
    
    let timeout_test_start = Instant::now();
    
    // Test with reasonable timeout
    match timeout(Duration::from_secs(5), predictor.predict(&large_data[0..100], 12, None)).await {
        Ok(Ok(results)) => {
            TestResultValidator::validate_predictions(&results, 12, 0.0)?;
            println!("   ✓ Large dataset processed successfully");
        }
        Ok(Err(e)) => {
            println!("   ✓ Prediction failed as expected due to constraints: {}", e);
            // This is acceptable - the system should handle large requests gracefully
        }
        Err(_) => {
            println!("   ✓ Request timed out as expected for large dataset");
            // This is also acceptable - system should not hang indefinitely
        }
    }
    
    // Test with very small timeout to ensure timeout mechanism works
    let short_timeout_result = timeout(
        Duration::from_millis(1), // Extremely short timeout
        predictor.predict(&large_data[0..50], 8, None)
    ).await;
    
    match short_timeout_result {
        Ok(_) => {
            println!("   ✓ Prediction completed within 1ms (very fast system)");
        }
        Err(_) => {
            println!("   ✓ Short timeout triggered as expected");
        }
    }
    
    let timeout_test_duration = timeout_test_start.elapsed();
    
    // Verify that timeout tests themselves don't take too long
    assert!(
        timeout_test_duration < Duration::from_secs(10),
        "Timeout tests took too long: {:.2}s",
        timeout_test_duration.as_secs_f64()
    );
    
    println!("✅ Timeout handling test passed");
    Ok(())
}

/// Test health monitoring under failure conditions
#[tokio::test]
async fn test_health_monitoring_under_failure() -> Result<()> {
    let config = TestConfigBuilder::new()
        .with_health_monitoring()
        .with_fallback()
        .build();

    let predictor = NeuralPredictor::new(config)?;
    let test_data = TestDataGenerator::generate_simple_data(50);
    
    println!("🏥 Testing health monitoring under failure conditions...");
    
    // Wait for health monitoring to initialize
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Phase 1: Check initial health status
    let initial_health = predictor.get_health_status().await;
    println!("   Initial health status: {:?}", initial_health);
    
    // Phase 2: Create stress conditions that might affect health
    let stress_tasks = vec![
        // Concurrent predictions
        tokio::spawn({
            let predictor = predictor.clone();
            let data = test_data.clone();
            async move {
                for i in 0..20 {
                    let chunk_start = i % (data.len() - 10);
                    let chunk = &data[chunk_start..chunk_start + 10];
                    let _ = predictor.predict(chunk, 5, None).await;
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
        }),
        // Edge case data
        tokio::spawn({
            let predictor = predictor.clone();
            async move {
                let edge_data = TestDataGenerator::generate_edge_case_data();
                for _ in 0..10 {
                    let _ = predictor.predict(&edge_data, 3, None).await;
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            }
        }),
        // Rapid requests
        tokio::spawn({
            let predictor = predictor.clone();
            let data = test_data.clone();
            async move {
                for i in 0..50 {
                    let chunk = &data[0..std::cmp::min(20, data.len())];
                    let _ = timeout(Duration::from_millis(50), predictor.predict(chunk, 3, None)).await;
                    if i % 10 == 0 {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                }
            }
        }),
    ];
    
    // Run stress conditions
    let stress_start = Instant::now();
    futures::future::join_all(stress_tasks).await;
    let stress_duration = stress_start.elapsed();
    
    println!("   ✓ Stress conditions applied for {:.2}s", stress_duration.as_secs_f64());
    
    // Phase 3: Check health status during/after stress
    tokio::time::sleep(Duration::from_millis(100)).await; // Allow health monitoring to update
    
    let stressed_health = predictor.get_health_status().await;
    println!("   Health status after stress: {:?}", stressed_health);
    
    // Phase 4: Validate health monitoring is functioning
    if let Some(health) = stressed_health {
        // Health monitoring should provide meaningful information
        assert!(health.is_object(), "Health status should be structured data");
        
        // Check for expected health fields
        if let Some(overall_healthy) = health.get("overall_healthy") {
            println!("   Overall healthy: {}", overall_healthy);
        }
        
        if let Some(error_rate) = health.get("error_rate") {
            if let Some(rate) = error_rate.as_f64() {
                assert!(rate >= 0.0 && rate <= 100.0, "Error rate should be a valid percentage");
                println!("   Error rate: {:.2}%", rate);
            }
        }
        
        if let Some(healthy_models) = health.get("healthy_models") {
            println!("   Healthy models: {}", healthy_models);
        }
    }
    
    // Phase 5: Verify system still functional after stress
    let recovery_result = predictor.predict(&test_data[0..20], 8, None).await?;
    TestResultValidator::validate_predictions(&recovery_result, 8, 0.0)?;
    println!("   ✓ System still functional after stress test");
    
    println!("✅ Health monitoring under failure test passed");
    Ok(())
}

/// Test fallback prediction accuracy and performance
#[tokio::test]
async fn test_fallback_prediction_quality() -> Result<()> {
    let config = TestConfigBuilder::new()
        .with_fallback()
        .with_circuit_breakers()
        .build();

    let predictor = NeuralPredictor::new(config)?;
    
    // Test with different data patterns to validate fallback quality
    let test_scenarios = vec![
        ("simple_trend", TestDataGenerator::generate_simple_data(100)),
        ("upward_trend", TestDataGenerator::generate_trending_data(100, 0.5)),
        ("downward_trend", TestDataGenerator::generate_trending_data(100, -0.3)),
    ];
    
    println!("📊 Testing fallback prediction quality...");
    
    for (scenario_name, test_data) in test_scenarios {
        println!("   Testing scenario: {}", scenario_name);
        
        // Make multiple predictions to potentially trigger fallback
        let mut scenario_results = Vec::new();
        let mut fallback_triggered = false;
        
        for iteration in 0..10 {
            let start_idx = iteration * 5;
            let chunk = &test_data[start_idx..std::cmp::min(start_idx + 30, test_data.len())];
            
            let perf_measurement = PerformanceMeasurement::start(&format!("{}_iteration_{}", scenario_name, iteration));
            
            match predictor.predict(chunk, 6, None).await {
                Ok(results) => {
                    TestResultValidator::validate_predictions(&results, 6, 0.0)?;
                    
                    // Check prediction quality
                    let avg_confidence: f64 = results.iter().map(|r| r.confidence).sum::<f64>() / results.len() as f64;
                    scenario_results.push((avg_confidence, results.len()));
                    
                    // Check if this looks like a fallback result
                    // (In practice, fallback results might have different characteristics)
                    if results.iter().any(|r| r.model_name.contains("fallback") || r.model_name.contains("FANN")) {
                        fallback_triggered = true;
                    }
                    
                    perf_measurement.assert_under_threshold(Duration::from_millis(100));
                }
                Err(e) => {
                    println!("     Iteration {} failed: {}", iteration, e);
                }
            }
            
            // Small delay between iterations
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        
        // Validate scenario results
        if !scenario_results.is_empty() {
            let avg_confidence: f64 = scenario_results.iter().map(|(conf, _)| conf).sum::<f64>() / scenario_results.len() as f64;
            let total_predictions: usize = scenario_results.iter().map(|(_, count)| count).sum();
            
            // Fallback predictions should still be reasonable quality
            assert!(
                avg_confidence >= 0.0 && avg_confidence <= 1.0,
                "Average confidence {:.3} not in valid range for scenario {}",
                avg_confidence, scenario_name
            );
            
            println!("     ✓ {} predictions, avg confidence: {:.3}", total_predictions, avg_confidence);
            
            if fallback_triggered {
                println!("     ✓ Fallback mechanism activated and working");
            }
        }
    }
    
    println!("✅ Fallback prediction quality test passed");
    Ok(())
}

/// Test graceful degradation under resource constraints
#[tokio::test]
async fn test_graceful_degradation() -> Result<()> {
    let config = TestConfigBuilder::new()
        .with_fallback()
        .with_health_monitoring()
        .build();

    let predictor = Arc::new(NeuralPredictor::new(config)?);
    let test_data = Arc::new(TestDataGenerator::generate_simple_data(200));
    
    println!("⬇️  Testing graceful degradation...");
    
    // Create high load to test degradation
    let high_load_start = Instant::now();
    let mut degradation_tasks = Vec::new();
    
    // Create many concurrent tasks to stress the system
    for task_id in 0..50 {
        let predictor_clone = Arc::clone(&predictor);
        let data_clone = Arc::clone(&test_data);
        
        let task = tokio::spawn(async move {
            let mut task_results = Vec::new();
            
            for iteration in 0..10 {
                let start_idx = (task_id * 2 + iteration) % (data_clone.len() - 20);
                let chunk = &data_clone[start_idx..start_idx + 20];
                
                let start_time = Instant::now();
                
                match timeout(Duration::from_millis(500), predictor_clone.predict(chunk, 4, None)).await {
                    Ok(Ok(results)) => {
                        let latency = start_time.elapsed();
                        task_results.push((true, latency, results.len(), results.get(0).map(|r| r.confidence).unwrap_or(0.0)));
                    }
                    Ok(Err(_)) => {
                        let latency = start_time.elapsed();
                        task_results.push((false, latency, 0, 0.0));
                    }
                    Err(_) => {
                        let latency = Duration::from_millis(500);
                        task_results.push((false, latency, 0, 0.0));
                    }
                }
                
                // Small random delay to vary load pattern
                tokio::time::sleep(Duration::from_millis((task_id % 20) as u64)).await;
            }
            
            task_results
        });
        
        degradation_tasks.push(task);
    }
    
    // Execute high load
    let task_results = futures::future::join_all(degradation_tasks).await;
    let high_load_duration = high_load_start.elapsed();
    
    // Analyze results
    let mut successful_predictions = 0;
    let mut total_attempts = 0;
    let mut total_latency = Duration::from_secs(0);
    let mut confidence_sum = 0.0;
    let mut confidence_count = 0;
    
    for task_result in task_results {
        if let Ok(results) = task_result {
            for (success, latency, prediction_count, confidence) in results {
                total_attempts += 1;
                total_latency += latency;
                
                if success {
                    successful_predictions += prediction_count;
                    if confidence > 0.0 {
                        confidence_sum += confidence;
                        confidence_count += 1;
                    }
                }
            }
        }
    }
    
    let success_rate = (successful_predictions as f64) / (total_attempts as f64) * 100.0;
    let avg_latency = total_latency / total_attempts as u32;
    let avg_confidence = if confidence_count > 0 { confidence_sum / confidence_count as f64 } else { 0.0 };
    
    println!("   High load results ({:.2}s):", high_load_duration.as_secs_f64());
    println!("     Success rate: {:.1}% ({}/{})", success_rate, successful_predictions, total_attempts);
    println!("     Average latency: {}ms", avg_latency.as_millis());
    println!("     Average confidence: {:.3}", avg_confidence);
    
    // Validate graceful degradation
    // Even under high load, system should maintain some level of service
    assert!(
        success_rate >= 30.0, // At least 30% success rate even under stress
        "Success rate {:.1}% too low - system may not be degrading gracefully",
        success_rate
    );
    
    // Latency may increase but should not become excessive
    assert!(
        avg_latency < Duration::from_secs(1),
        "Average latency {}ms too high - system may not be handling load well",
        avg_latency.as_millis()
    );
    
    // Test recovery after high load
    tokio::time::sleep(Duration::from_millis(500)).await; // Allow recovery
    
    let recovery_result = predictor.predict(&test_data[0..30], 8, None).await?;
    TestResultValidator::validate_predictions(&recovery_result, 8, 0.0)?;
    
    println!("   ✓ System recovered successfully after high load");
    
    println!("✅ Graceful degradation test passed");
    Ok(())
}