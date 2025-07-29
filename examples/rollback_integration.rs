//! Example showing integration of model rollback with neural adapters
//!
//! This demonstrates how to:
//! - Deploy new model versions atomically
//! - Monitor performance and trigger automatic rollbacks
//! - Integrate with health monitoring system
//! - Handle Docker container restarts safely

use anyhow::Result;
use autonomous_platform::adapters::{
    enhanced_neural_adapter::{EnhancedNeuralAdapter, EnhancedNeuralConfig},
    health_monitor::{HealthChecker, HealthMonitor, HealthMonitorConfig},
    model_rollback::{ModelRollbackManager, RollbackConfig, ModelMetrics},
};
use autonomous_platform::config::NeuralConfig;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info,autonomous_platform=debug")
        .init();

    info!("Starting rollback integration example");

    // Create rollback configuration
    let rollback_config = RollbackConfig {
        model_base_dir: PathBuf::from("./test_models"),
        metadata_backup_path: PathBuf::from("./test_metadata"),
        max_versions: 3,
        degradation_threshold: 10.0, // 10% performance drop triggers rollback
        evaluation_period: 60,       // 1 minute evaluation
        sample_count: 10,
        enable_auto_rollback: true,
        health_check_interval: Duration::from_secs(5),
        grace_period: Duration::from_secs(10),
        enable_metadata_backup: true,
    };

    // Create rollback manager
    let rollback_manager = Arc::new(ModelRollbackManager::new(rollback_config)?);

    // Create health monitor
    let health_config = HealthMonitorConfig {
        check_interval: Duration::from_secs(5),
        check_timeout: Duration::from_secs(2),
        ..Default::default()
    };
    let mut health_monitor = HealthMonitor::new(health_config);

    // Create neural configuration
    let neural_config = NeuralConfig {
        memory_gb: 2.0,
        models: vec!["enhanced_mlp".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.85,
        use_real_models: true,
        enable_health_checks: true,
        enable_fallback: true,
        enable_circuit_breakers: true,
        enable_graceful_degradation: true,
        enable_performance_monitoring: true,
        enable_adaptive_retry: true,
        enable_model_ensembles: false,
        model_timeout_seconds: 30,
        max_retries: 3,
        error_threshold: 0.05,
    };

    // Create enhanced neural adapter
    let enhanced_config = EnhancedNeuralConfig::from_neural_config(&neural_config);
    let neural_adapter = Arc::new(EnhancedNeuralAdapter::new(enhanced_config).await?);

    // Register health checker
    health_monitor.register_health_checker(
        "enhanced_mlp".to_string(),
        neural_adapter.clone() as Arc<dyn HealthChecker>,
    );

    // Link rollback manager with health checker
    let mut rollback_manager_mut = ModelRollbackManager::new(rollback_config)?;
    rollback_manager_mut.set_health_checker(neural_adapter.clone() as Arc<dyn HealthChecker>);
    let rollback_manager = Arc::new(rollback_manager_mut);

    // Start health monitoring
    health_monitor.start_monitoring().await?;

    // Simulate model deployment lifecycle
    info!("=== Phase 1: Deploy initial model (v1) ===");
    let v1_metrics = ModelMetrics {
        accuracy: 92.0,
        latency_ms: 50.0,
        error_rate: 8.0,
        memory_mb: 150,
        cpu_percent: 30.0,
        throughput: 20.0,
        timestamp: chrono::Utc::now(),
    };

    // Simulate model file creation
    std::fs::create_dir_all("./test_models")?;
    std::fs::write("./test_models/model_v1.bin", b"Model v1 binary data")?;

    let v1 = rollback_manager.deploy_model(
        "enhanced_mlp",
        &PathBuf::from("./test_models/model_v1.bin"),
        serde_json::json!({
            "version": "1.0.0",
            "training_date": "2024-01-01",
            "parameters": {
                "layers": [128, 64, 32],
                "learning_rate": 0.001
            }
        }),
        v1_metrics,
    ).await?;

    info!("Deployed model v1: {}", v1.version_id);

    // Let it run for a while
    sleep(Duration::from_secs(15)).await;

    // Check current status
    if let Some(active) = rollback_manager.get_active_version("enhanced_mlp").await {
        info!("Active model: {} (status: {:?})", active.version_id, active.status);
    }

    info!("=== Phase 2: Deploy improved model (v2) ===");
    let v2_metrics = ModelMetrics {
        accuracy: 95.0,
        latency_ms: 45.0,
        error_rate: 5.0,
        memory_mb: 160,
        cpu_percent: 32.0,
        throughput: 22.0,
        timestamp: chrono::Utc::now(),
    };

    std::fs::write("./test_models/model_v2.bin", b"Model v2 binary data")?;

    let v2 = rollback_manager.deploy_model(
        "enhanced_mlp",
        &PathBuf::from("./test_models/model_v2.bin"),
        serde_json::json!({
            "version": "2.0.0",
            "training_date": "2024-01-15",
            "parameters": {
                "layers": [256, 128, 64, 32],
                "learning_rate": 0.0005
            }
        }),
        v2_metrics,
    ).await?;

    info!("Deployed model v2: {}", v2.version_id);
    sleep(Duration::from_secs(10)).await;

    info!("=== Phase 3: Deploy degraded model (v3) to trigger rollback ===");
    // This model has worse performance to trigger automatic rollback
    let v3_metrics = ModelMetrics {
        accuracy: 80.0,  // Significant drop from 95% (>10% threshold)
        latency_ms: 100.0, // Much slower
        error_rate: 20.0,  // High error rate
        memory_mb: 200,
        cpu_percent: 50.0,
        throughput: 10.0,
        timestamp: chrono::Utc::now(),
    };

    std::fs::write("./test_models/model_v3.bin", b"Model v3 binary data (degraded)")?;

    let v3 = rollback_manager.deploy_model(
        "enhanced_mlp",
        &PathBuf::from("./test_models/model_v3.bin"),
        serde_json::json!({
            "version": "3.0.0",
            "training_date": "2024-01-20",
            "parameters": {
                "layers": [512, 256, 128, 64, 32], // Over-complex
                "learning_rate": 0.01 // Too high
            }
        }),
        v3_metrics,
    ).await?;

    info!("Deployed degraded model v3: {}", v3.version_id);
    warn!("Waiting for automatic rollback to trigger...");

    // Wait for automatic rollback to happen
    sleep(Duration::from_secs(30)).await;

    // Check if rollback occurred
    if let Some(active) = rollback_manager.get_active_version("enhanced_mlp").await {
        info!("Active model after rollback: {} (status: {:?})", active.version_id, active.status);
        
        if active.version_id == v2.version_id {
            info!("✅ Automatic rollback successful! Reverted to v2");
        } else {
            warn!("❌ Automatic rollback did not occur as expected");
        }
    }

    // Show rollback history
    info!("=== Rollback History ===");
    let history = rollback_manager.get_rollback_history().await;
    for (i, decision) in history.iter().enumerate() {
        info!("Rollback #{}: {:?}", i + 1, decision.reason);
        info!("  - Automatic: {}", decision.automatic);
        info!("  - Performance delta: {:?}", decision.performance_delta);
    }

    // Show version history
    info!("=== Version History ===");
    let versions = rollback_manager.get_version_history("enhanced_mlp").await;
    for version in versions {
        info!("{} - Status: {:?}, Rollbacks: {}", 
              version.version_id, version.status, version.rollback_count);
    }

    // Test manual rollback
    info!("=== Phase 4: Manual rollback test ===");
    match rollback_manager.manual_rollback(
        "enhanced_mlp",
        "admin",
        "Testing manual rollback functionality",
    ).await {
        Ok(version) => {
            info!("Manual rollback successful to: {}", version.version_id);
        }
        Err(e) => {
            warn!("Manual rollback failed: {}", e);
        }
    }

    // Test Docker restart resilience
    info!("=== Phase 5: Test Docker restart resilience ===");
    let current_path = rollback_manager.get_current_model_path("enhanced_mlp").await?;
    info!("Current model path (via symlink): {:?}", current_path);
    
    // Verify integrity after "restart"
    let integrity_ok = rollback_manager.verify_model_integrity("enhanced_mlp").await?;
    info!("Model integrity check: {}", if integrity_ok { "PASSED" } else { "FAILED" });

    // Cleanup old archives
    info!("=== Phase 6: Cleanup old archives ===");
    let cleaned = rollback_manager.cleanup_archives("enhanced_mlp", 2).await?;
    info!("Cleaned up {} old archived versions", cleaned);

    // Stop monitoring
    health_monitor.stop_monitoring().await;

    // Cleanup test files
    std::fs::remove_dir_all("./test_models").ok();
    std::fs::remove_dir_all("./test_metadata").ok();

    info!("Example completed successfully!");
    Ok(())
}