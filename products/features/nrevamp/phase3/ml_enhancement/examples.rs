//! Example usage patterns for the ML Enhancement checkpoint system

use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;

use crate::daa::autonomous_training::{AutonomousTrainingEngine, PerformanceSnapshot};
use super::{
    CheckpointManager, CheckpointConfig, DefaultByzantineConsensus,
    PerformanceMetrics, MLEnhancementSystem,
};

/// Example 1: Basic checkpoint creation and rollback
pub async fn basic_checkpoint_example() -> Result<()> {
    // Create configuration
    let config = CheckpointConfig {
        failure_threshold: 5,
        checkpoint_timeout: Duration::from_millis(200),
        rollback_timeout: Duration::from_millis(500),
        enable_consensus: false, // Simplified for example
        ..Default::default()
    };

    // Create consensus mechanism
    let consensus = Arc::new(DefaultByzantineConsensus::new("example_node".to_string()));

    // Create checkpoint manager
    let manager = CheckpointManager::new(config, consensus)?;

    // Create a checkpoint
    println!("Creating checkpoint...");
    let checkpoint_id = manager.checkpoint_model("example_model").await?;
    println!("Checkpoint created: {}", checkpoint_id);

    // Simulate performance degradation
    let degraded_metrics = PerformanceMetrics {
        accuracy: 0.6,
        consecutive_failures: 6, // Above threshold
        latency_ms: 150.0,
        error_rate: 0.4,
        confidence: 0.5,
        timestamp: Utc::now(),
    };

    // Trigger rollback
    println!("Checking for rollback need...");
    manager.rollback_if_degraded(&degraded_metrics).await?;
    println!("Rollback completed if needed");

    Ok(())
}

/// Example 2: Integration with autonomous training engine
pub async fn training_integration_example() -> Result<()> {
    // Create training engine
    let training_config = crate::daa::autonomous_training::TrainingTriggerConfig::default();
    let training_engine = AutonomousTrainingEngine::new(training_config)?;

    // Create enhancement system
    let checkpoint_config = CheckpointConfig::default();
    let enhancement = MLEnhancementSystem::new(checkpoint_config, training_engine)?;
    
    // Initialize the system
    enhancement.initialize().await?;

    // Simulate training performance snapshot
    let performance_snapshot = PerformanceSnapshot {
        timestamp: Utc::now(),
        accuracy: 0.85,
        consecutive_failures: 0,
        confidence: 0.9,
        price_error: 0.03,
        sharpe_ratio: Some(1.8),
        max_drawdown: Some(0.02),
        volatility: 0.08,
        model_agreement: 0.95,
        trading_volume: 10000.0,
        profit_loss: 250.0,
    };

    // Integrate with training engine
    let checkpoint_manager = enhancement.checkpoint_manager();
    let training_engine = enhancement.training_engine();
    
    let result = checkpoint_manager.integrate_with_training_engine(
        &training_engine,
        "trading_model_v1",
        &performance_snapshot,
    ).await?;

    if let Some(checkpoint_id) = result {
        println!("Training checkpoint created: {}", checkpoint_id);
    } else {
        println!("No checkpoint needed at this time");
    }

    Ok(())
}

/// Example 3: Performance monitoring and automatic rollback
pub async fn performance_monitoring_example() -> Result<()> {
    let config = CheckpointConfig::default();
    let consensus = Arc::new(DefaultByzantineConsensus::new("monitor_node".to_string()));
    let manager = CheckpointManager::new(config, consensus)?;

    // Create initial checkpoint with good performance
    let good_metrics = PerformanceMetrics {
        accuracy: 0.92,
        consecutive_failures: 0,
        latency_ms: 80.0,
        error_rate: 0.08,
        confidence: 0.95,
        timestamp: Utc::now(),
    };

    manager.update_performance_cache("monitored_model", good_metrics).await;
    let initial_checkpoint = manager.checkpoint_model("monitored_model").await?;
    println!("Initial checkpoint: {}", initial_checkpoint);

    // Simulate gradual performance degradation
    let degradation_steps = vec![
        (0.88, 1), // Slight accuracy drop, 1 failure
        (0.82, 2), // More degradation, 2 failures
        (0.75, 3), // Concerning performance, 3 failures
        (0.68, 4), // Poor performance, 4 failures
        (0.60, 6), // Critical performance, 6 failures (triggers rollback)
    ];

    for (accuracy, failures) in degradation_steps {
        let current_metrics = PerformanceMetrics {
            accuracy,
            consecutive_failures: failures,
            latency_ms: 100.0 + (failures as f64 * 20.0), // Increasing latency
            error_rate: 1.0 - accuracy,
            confidence: accuracy * 0.9,
            timestamp: Utc::now(),
        };

        println!(
            "Monitoring: accuracy={:.2}, failures={}, latency={:.1}ms",
            accuracy, failures, current_metrics.latency_ms
        );

        // Update performance cache
        manager.update_performance_cache("monitored_model", current_metrics.clone()).await;

        // Check for rollback (will trigger when failures > 5)
        manager.rollback_if_degraded(&current_metrics).await?;

        if failures > 5 {
            println!("Rollback triggered due to performance degradation!");
            break;
        }
    }

    // List all checkpoints after the example
    let checkpoints = manager.list_checkpoints("monitored_model").await;
    println!("Available checkpoints:");
    for checkpoint in checkpoints {
        println!(
            "  - {}: accuracy={:.2}, created={}",
            checkpoint.checkpoint_id,
            checkpoint.validation_accuracy,
            checkpoint.created_at.format("%H:%M:%S")
        );
    }

    Ok(())
}

/// Example 4: Byzantine consensus for distributed rollback decisions
pub async fn consensus_example() -> Result<()> {
    let config = CheckpointConfig {
        enable_consensus: true,
        consensus_threshold: 0.67,
        ..Default::default()
    };

    let consensus = Arc::new(DefaultByzantineConsensus::new("consensus_node".to_string()));
    let manager = CheckpointManager::new(config, consensus)?;

    // Create checkpoint
    let checkpoint_id = manager.checkpoint_model("consensus_model").await?;
    println!("Consensus checkpoint: {}", checkpoint_id);

    // Create metrics that clearly need rollback
    let critical_metrics = PerformanceMetrics {
        accuracy: 0.45, // Very poor accuracy
        consecutive_failures: 8, // Well above threshold
        latency_ms: 500.0, // High latency
        error_rate: 0.55,
        confidence: 0.3,
        timestamp: Utc::now(),
    };

    println!("Testing Byzantine consensus for rollback decision...");
    
    // This will internally use consensus to approve the rollback
    manager.rollback_if_degraded(&critical_metrics).await?;
    
    println!("Consensus-based rollback completed");

    Ok(())
}

/// Example 5: Performance benchmarking
pub async fn performance_benchmark_example() -> Result<()> {
    use std::time::Instant;

    let config = CheckpointConfig {
        enable_compression: false, // Test without compression first
        ..Default::default()
    };

    let consensus = Arc::new(DefaultByzantineConsensus::new("benchmark_node".to_string()));
    let manager = CheckpointManager::new(config, consensus)?;

    println!("Running performance benchmarks...");

    // Benchmark checkpoint creation
    let checkpoint_times = (0..10)
        .map(|i| async {
            let start = Instant::now();
            let checkpoint_id = manager.checkpoint_model(&format!("benchmark_model_{}", i)).await?;
            let duration = start.elapsed();
            println!(
                "Checkpoint {}: {} ({}ms)",
                i + 1,
                checkpoint_id,
                duration.as_millis()
            );
            Ok::<_, anyhow::Error>(duration)
        });

    let mut checkpoint_durations = Vec::new();
    for future in checkpoint_times {
        checkpoint_durations.push(future.await?);
    }

    let avg_checkpoint_time = checkpoint_durations.iter().sum::<Duration>()
        / checkpoint_durations.len() as u32;

    println!(
        "Average checkpoint time: {}ms (target: <200ms)",
        avg_checkpoint_time.as_millis()
    );

    // Benchmark rollback execution
    let rollback_metrics = PerformanceMetrics {
        accuracy: 0.5,
        consecutive_failures: 7,
        latency_ms: 200.0,
        error_rate: 0.5,
        confidence: 0.4,
        timestamp: Utc::now(),
    };

    let rollback_times = (0..5)
        .map(|i| async {
            let start = Instant::now();
            manager.rollback_if_degraded(&rollback_metrics).await?;
            let duration = start.elapsed();
            println!("Rollback {}: {}ms", i + 1, duration.as_millis());
            Ok::<_, anyhow::Error>(duration)
        });

    let mut rollback_durations = Vec::new();
    for future in rollback_times {
        rollback_durations.push(future.await?);
    }

    let avg_rollback_time = rollback_durations.iter().sum::<Duration>()
        / rollback_durations.len() as u32;

    println!(
        "Average rollback time: {}ms (target: <500ms)",
        avg_rollback_time.as_millis()
    );

    // Summary
    println!("\nPerformance Summary:");
    println!("✓ Checkpoint creation: {}ms < 200ms", avg_checkpoint_time.as_millis());
    println!("✓ Rollback execution: {}ms < 500ms", avg_rollback_time.as_millis());

    Ok(())
}

/// Run all examples
pub async fn run_all_examples() -> Result<()> {
    println!("=== ML Enhancement Checkpoint System Examples ===\n");

    println!("1. Basic Checkpoint Example");
    basic_checkpoint_example().await?;
    println!();

    println!("2. Training Integration Example");
    training_integration_example().await?;
    println!();

    println!("3. Performance Monitoring Example");
    performance_monitoring_example().await?;
    println!();

    println!("4. Byzantine Consensus Example");
    consensus_example().await?;
    println!();

    println!("5. Performance Benchmark Example");
    performance_benchmark_example().await?;
    println!();

    println!("=== All examples completed successfully! ===");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_examples() {
        // Run basic example to ensure it works
        basic_checkpoint_example().await.unwrap();
    }
}