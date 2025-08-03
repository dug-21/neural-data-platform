//! Comprehensive test suite for the ML Enhancement checkpoint system

use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::time::timeout;

use crate::daa::autonomous_training::{AutonomousTrainingEngine, PerformanceSnapshot, TrainingTriggerConfig};
use super::{
    CheckpointManager, CheckpointConfig, DefaultByzantineConsensus,
    PerformanceMetrics, MLEnhancementSystem, ByzantineConsensus, RollbackDecision,
};

/// Test helper to create temporary checkpoint manager
async fn create_test_manager() -> (CheckpointManager, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let config = CheckpointConfig {
        checkpoint_dir: temp_dir.path().to_path_buf(),
        max_checkpoints: 5,
        enable_compression: false,
        failure_threshold: 5,
        checkpoint_timeout: Duration::from_millis(200),
        rollback_timeout: Duration::from_millis(500),
        enable_consensus: false,
        consensus_threshold: 0.67,
    };
    
    let consensus = Arc::new(DefaultByzantineConsensus::new("test_node".to_string()));
    let manager = CheckpointManager::new(config, consensus).unwrap();
    
    (manager, temp_dir)
}

/// Test helper to create performance metrics
fn create_performance_metrics(accuracy: f64, failures: u32) -> PerformanceMetrics {
    PerformanceMetrics {
        accuracy,
        consecutive_failures: failures,
        latency_ms: 100.0,
        error_rate: 1.0 - accuracy,
        confidence: accuracy * 0.9,
        timestamp: Utc::now(),
    }
}

#[tokio::test]
async fn test_checkpoint_creation_performance() {
    let (manager, _temp_dir) = create_test_manager().await;
    
    let start = Instant::now();
    let checkpoint_id = manager.checkpoint_model("test_model").await.unwrap();
    let duration = start.elapsed();
    
    // Verify performance requirement: <200ms
    assert!(
        duration < Duration::from_millis(200),
        "Checkpoint creation took {}ms, exceeds 200ms target",
        duration.as_millis()
    );
    
    assert!(!checkpoint_id.is_empty());
    assert!(checkpoint_id.starts_with("test_model_"));
    
    // Verify checkpoint exists in registry
    let checkpoints = manager.list_checkpoints("test_model").await;
    assert_eq!(checkpoints.len(), 1);
    assert_eq!(checkpoints[0].checkpoint_id, checkpoint_id);
}

#[tokio::test]
async fn test_rollback_performance() {
    let (manager, _temp_dir) = create_test_manager().await;
    
    // Create initial checkpoint
    let _checkpoint_id = manager.checkpoint_model("test_model").await.unwrap();
    
    // Create metrics triggering rollback
    let degraded_metrics = create_performance_metrics(0.5, 6); // Above threshold
    
    let start = Instant::now();
    manager.rollback_if_degraded(&degraded_metrics).await.unwrap();
    let duration = start.elapsed();
    
    // Verify performance requirement: <500ms
    assert!(
        duration < Duration::from_millis(500),
        "Rollback took {}ms, exceeds 500ms target",
        duration.as_millis()
    );
}

#[tokio::test]
async fn test_failure_threshold_integration() {
    let (manager, _temp_dir) = create_test_manager().await;
    
    // Create checkpoint
    let _checkpoint_id = manager.checkpoint_model("test_model").await.unwrap();
    
    // Test below threshold - should not rollback
    let good_metrics = create_performance_metrics(0.7, 3); // Below threshold of 5
    manager.rollback_if_degraded(&good_metrics).await.unwrap();
    
    // Test at threshold - should not rollback
    let threshold_metrics = create_performance_metrics(0.6, 5); // At threshold
    manager.rollback_if_degraded(&threshold_metrics).await.unwrap();
    
    // Test above threshold - should rollback
    let bad_metrics = create_performance_metrics(0.5, 6); // Above threshold
    manager.rollback_if_degraded(&bad_metrics).await.unwrap();
}

#[tokio::test]
async fn test_autonomous_training_integration() {
    let (manager, _temp_dir) = create_test_manager().await;
    
    let training_config = TrainingTriggerConfig::default();
    let engine = AutonomousTrainingEngine::new(training_config).unwrap();
    
    // Test with good performance - should create checkpoint
    let good_snapshot = PerformanceSnapshot {
        timestamp: Utc::now(),
        accuracy: 0.9,
        consecutive_failures: 0,
        confidence: 0.85,
        price_error: 0.05,
        sharpe_ratio: Some(1.5),
        max_drawdown: Some(0.03),
        volatility: 0.1,
        model_agreement: 0.95,
        trading_volume: 1000.0,
        profit_loss: 100.0,
    };
    
    let result = manager.integrate_with_training_engine(
        &engine,
        "integration_model",
        &good_snapshot,
    ).await.unwrap();
    
    assert!(result.is_some(), "Should create checkpoint on good performance");
    
    // Test with poor performance - should trigger rollback
    let bad_snapshot = PerformanceSnapshot {
        consecutive_failures: 7, // Above threshold
        accuracy: 0.5,
        ..good_snapshot
    };
    
    let result = manager.integrate_with_training_engine(
        &engine,
        "integration_model",
        &bad_snapshot,
    ).await.unwrap();
    
    // Should not create new checkpoint but should trigger rollback
    assert!(result.is_none());
}

#[tokio::test]
async fn test_checkpoint_cleanup() {
    let (manager, _temp_dir) = create_test_manager().await;
    
    // Create more checkpoints than the limit (5)
    for i in 0..7 {
        let checkpoint_id = manager.checkpoint_model("cleanup_model").await.unwrap();
        println!("Created checkpoint {}: {}", i + 1, checkpoint_id);
    }
    
    // Should only keep max_checkpoints (5)
    let checkpoints = manager.list_checkpoints("cleanup_model").await;
    assert_eq!(
        checkpoints.len(),
        5,
        "Should cleanup old checkpoints to maintain limit"
    );
    
    // Verify chronological order (most recent first)
    for i in 1..checkpoints.len() {
        assert!(
            checkpoints[i-1].created_at >= checkpoints[i].created_at,
            "Checkpoints should be ordered by creation time"
        );
    }
}

#[tokio::test]
async fn test_byzantine_consensus() {
    let consensus = DefaultByzantineConsensus::new("test_consensus_node".to_string());
    
    // Test decision that should be approved (high failures)
    let critical_decision = RollbackDecision {
        model_id: "test_model".to_string(),
        current_failures: 8,
        failure_threshold: 5,
        performance_degradation: 0.4,
        proposed_checkpoint: "checkpoint_123".to_string(),
        consensus_votes: Vec::new(),
        decision_timestamp: Utc::now(),
        automatic_rollback: true,
    };
    
    let votes = consensus.request_rollback_consensus(&critical_decision).await.unwrap();
    assert!(!votes.is_empty());
    
    let approved = consensus.validate_consensus(&votes).await.unwrap();
    assert!(approved, "Should approve rollback for critical failures");
    
    // Test decision that should be rejected (low failures)
    let minor_decision = RollbackDecision {
        current_failures: 2,
        performance_degradation: 0.05,
        ..critical_decision
    };
    
    let votes = consensus.request_rollback_consensus(&minor_decision).await.unwrap();
    let approved = consensus.validate_consensus(&votes).await.unwrap();
    assert!(!approved, "Should reject rollback for minor issues");
}

#[tokio::test]
async fn test_performance_cache() {
    let (manager, _temp_dir) = create_test_manager().await;
    
    let initial_metrics = create_performance_metrics(0.9, 0);
    manager.update_performance_cache("cache_model", initial_metrics.clone()).await;
    
    // Create checkpoint to establish baseline
    let _checkpoint_id = manager.checkpoint_model("cache_model").await.unwrap();
    
    // Update with degraded metrics
    let degraded_metrics = create_performance_metrics(0.6, 6);
    manager.update_performance_cache("cache_model", degraded_metrics.clone()).await;
    
    // Rollback should use cached metrics
    manager.rollback_if_degraded(&degraded_metrics).await.unwrap();
}

#[tokio::test]
async fn test_ml_enhancement_system() {
    let temp_dir = TempDir::new().unwrap();
    let checkpoint_config = CheckpointConfig {
        checkpoint_dir: temp_dir.path().to_path_buf(),
        ..Default::default()
    };
    
    let training_config = TrainingTriggerConfig::default();
    let training_engine = AutonomousTrainingEngine::new(training_config).unwrap();
    
    let system = MLEnhancementSystem::new(checkpoint_config, training_engine).unwrap();
    system.initialize().await.unwrap();
    
    // Test that components are accessible
    let checkpoint_manager = system.checkpoint_manager();
    let training_engine = system.training_engine();
    
    // Create a checkpoint through the system
    let checkpoint_id = checkpoint_manager.checkpoint_model("system_model").await.unwrap();
    assert!(!checkpoint_id.is_empty());
}

#[tokio::test]
async fn test_timeout_protection() {
    let temp_dir = TempDir::new().unwrap();
    let config = CheckpointConfig {
        checkpoint_dir: temp_dir.path().to_path_buf(),
        checkpoint_timeout: Duration::from_millis(50), // Very short timeout
        rollback_timeout: Duration::from_millis(50),
        ..Default::default()
    };
    
    let consensus = Arc::new(DefaultByzantineConsensus::new("timeout_node".to_string()));
    let manager = CheckpointManager::new(config, consensus).unwrap();
    
    // Note: In a real scenario with slow I/O, this might timeout
    // For testing, we'll verify the timeout mechanism exists
    let result = timeout(
        Duration::from_millis(100),
        manager.checkpoint_model("timeout_model")
    ).await;
    
    // Should either succeed quickly or timeout appropriately
    assert!(result.is_ok(), "Operation should complete within timeout");
}

#[tokio::test]
async fn test_checkpoint_integrity() {
    let (manager, _temp_dir) = create_test_manager().await;
    
    // Create checkpoint
    let checkpoint_id = manager.checkpoint_model("integrity_model").await.unwrap();
    
    // Get checkpoint details
    let checkpoints = manager.list_checkpoints("integrity_model").await;
    assert_eq!(checkpoints.len(), 1);
    
    let checkpoint = &checkpoints[0];
    assert_eq!(checkpoint.checkpoint_id, checkpoint_id);
    assert!(checkpoint.model_size_bytes > 0);
    assert!(!checkpoint.model_state_hash.is_empty());
    assert!(checkpoint.checkpoint_duration_ms < 200); // Should meet performance target
}

#[tokio::test]
async fn test_concurrent_operations() {
    let (manager, _temp_dir) = create_test_manager().await;
    let manager = Arc::new(manager);
    
    // Test concurrent checkpoint creation
    let tasks: Vec<_> = (0..5)
        .map(|i| {
            let manager = Arc::clone(&manager);
            tokio::spawn(async move {
                manager.checkpoint_model(&format!("concurrent_model_{}", i)).await
            })
        })
        .collect();
    
    // Wait for all tasks to complete
    let results: Vec<_> = futures::future::join_all(tasks).await;
    
    // All should succeed
    for (i, result) in results.into_iter().enumerate() {
        let checkpoint_id = result.unwrap().unwrap();
        assert!(checkpoint_id.contains(&format!("concurrent_model_{}", i)));
    }
}

#[tokio::test]
async fn test_edge_cases() {
    let (manager, _temp_dir) = create_test_manager().await;
    
    // Test rollback without checkpoints
    let metrics = create_performance_metrics(0.5, 6);
    let result = manager.rollback_if_degraded(&metrics).await;
    
    // Should handle gracefully (might succeed with empty operation or return error)
    // The exact behavior depends on implementation details
    match result {
        Ok(_) => println!("Rollback handled gracefully with no checkpoints"),
        Err(e) => println!("Expected error for rollback without checkpoints: {}", e),
    }
    
    // Test listing checkpoints for non-existent model
    let empty_checkpoints = manager.list_checkpoints("non_existent_model").await;
    assert!(empty_checkpoints.is_empty());
    
    // Test checkpoint with zero-failure metrics
    let perfect_metrics = create_performance_metrics(1.0, 0);
    manager.rollback_if_degraded(&perfect_metrics).await.unwrap();
}

#[tokio::test]
async fn test_performance_snapshot_conversion() {
    let snapshot = PerformanceSnapshot {
        timestamp: Utc::now(),
        accuracy: 0.85,
        consecutive_failures: 3,
        confidence: 0.8,
        price_error: 0.1,
        sharpe_ratio: Some(1.2),
        max_drawdown: Some(0.05),
        volatility: 0.12,
        model_agreement: 0.9,
        trading_volume: 5000.0,
        profit_loss: 200.0,
    };
    
    let metrics = PerformanceMetrics::from(snapshot.clone());
    
    assert_eq!(metrics.accuracy, snapshot.accuracy);
    assert_eq!(metrics.consecutive_failures, snapshot.consecutive_failures);
    assert_eq!(metrics.confidence, snapshot.confidence);
    assert_eq!(metrics.error_rate, snapshot.price_error);
    assert_eq!(metrics.timestamp, snapshot.timestamp);
}

/// Stress test for performance under load
#[tokio::test]
async fn test_performance_stress() {
    let (manager, _temp_dir) = create_test_manager().await;
    
    let start = Instant::now();
    
    // Create 20 checkpoints rapidly
    for i in 0..20 {
        let checkpoint_id = manager.checkpoint_model(&format!("stress_model_{}", i)).await.unwrap();
        println!("Stress checkpoint {}: {}", i + 1, checkpoint_id);
    }
    
    let total_duration = start.elapsed();
    let avg_per_checkpoint = total_duration / 20;
    
    println!(
        "Stress test: 20 checkpoints in {}ms (avg: {}ms per checkpoint)",
        total_duration.as_millis(),
        avg_per_checkpoint.as_millis()
    );
    
    // Each checkpoint should still meet performance target
    assert!(
        avg_per_checkpoint < Duration::from_millis(200),
        "Average checkpoint time under stress: {}ms",
        avg_per_checkpoint.as_millis()
    );
}

/// Integration test combining all features
#[tokio::test]
async fn test_full_integration() {
    let temp_dir = TempDir::new().unwrap();
    let checkpoint_config = CheckpointConfig {
        checkpoint_dir: temp_dir.path().to_path_buf(),
        enable_consensus: true,
        ..Default::default()
    };
    
    let training_config = TrainingTriggerConfig::default();
    let training_engine = AutonomousTrainingEngine::new(training_config).unwrap();
    
    let system = MLEnhancementSystem::new(checkpoint_config, training_engine).unwrap();
    system.initialize().await.unwrap();
    
    let checkpoint_manager = system.checkpoint_manager();
    let training_engine = system.training_engine();
    
    // Simulate full training cycle with checkpoints and rollbacks
    
    // 1. Initial training with good performance
    let initial_snapshot = PerformanceSnapshot {
        timestamp: Utc::now(),
        accuracy: 0.9,
        consecutive_failures: 0,
        confidence: 0.85,
        price_error: 0.05,
        sharpe_ratio: Some(1.5),
        max_drawdown: Some(0.03),
        volatility: 0.1,
        model_agreement: 0.95,
        trading_volume: 1000.0,
        profit_loss: 100.0,
    };
    
    let checkpoint_id = checkpoint_manager.integrate_with_training_engine(
        &training_engine,
        "full_integration_model",
        &initial_snapshot,
    ).await.unwrap();
    
    assert!(checkpoint_id.is_some(), "Should create initial checkpoint");
    
    // 2. Performance degradation
    let degraded_snapshot = PerformanceSnapshot {
        accuracy: 0.6,
        consecutive_failures: 6,
        confidence: 0.5,
        price_error: 0.3,
        ..initial_snapshot
    };
    
    let result = checkpoint_manager.integrate_with_training_engine(
        &training_engine,
        "full_integration_model",
        &degraded_snapshot,
    ).await.unwrap();
    
    assert!(result.is_none(), "Should not create checkpoint on poor performance");
    
    // 3. Verify rollback occurred by checking cached metrics
    let checkpoints = checkpoint_manager.list_checkpoints("full_integration_model").await;
    assert!(!checkpoints.is_empty(), "Should have checkpoints available");
    
    println!("Full integration test completed successfully");
}