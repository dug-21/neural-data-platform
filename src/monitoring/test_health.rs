use super::health::*;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_neural_health_check() {
    // Create a health monitor
    let monitor = HealthMonitor::new().await.expect("Failed to create health monitor");
    
    // Check neural system health
    let health = monitor.check_component_health(ComponentType::NeuralSystem).await;
    
    match health {
        Ok(component_health) => {
            println!("Neural System Health: {:?}", component_health.status);
            
            // Print metadata
            for (key, value) in &component_health.metadata {
                println!("  {}: {}", key, value);
            }
            
            // The status should be healthy since models directory exists
            assert!(component_health.is_healthy() || component_health.is_degraded());
        }
        Err(e) => panic!("Health check failed: {}", e),
    }
}

#[tokio::test]
async fn test_prometheus_metrics() {
    let monitor = HealthMonitor::new().await.expect("Failed to create health monitor");
    let endpoints = HealthEndpoints::new(std::sync::Arc::new(monitor));
    
    // Test metrics endpoint
    let metrics = endpoints.metrics_endpoint().await;
    
    match metrics {
        Ok(metrics_output) => {
            println!("Prometheus metrics output:");
            println!("{}", metrics_output);
            
            // Check for our new metrics
            assert!(metrics_output.contains("neural_trader_models_available"));
            assert!(metrics_output.contains("neural_trader_model_storage_mounted"));
            assert!(metrics_output.contains("neural_trader_model_storage_writable"));
            assert!(metrics_output.contains("neural_trader_model_storage_size_mb"));
            assert!(metrics_output.contains("neural_trader_model_storage_disk_available_gb"));
            assert!(metrics_output.contains("neural_trader_corrupted_models"));
        }
        Err(e) => panic!("Metrics endpoint failed: {}", e),
    }
}

#[tokio::test]
async fn test_disk_space_check() {
    use super::health::check_disk_space;
    use std::path::Path;
    
    let models_path = Path::new("./models");
    let disk_info = check_disk_space(models_path).await;
    
    println!("Disk Info: {:?}", disk_info);
    
    // Basic sanity checks
    assert!(disk_info.total_gb > 0.0);
    assert!(disk_info.available_gb >= 0.0);
    assert!(disk_info.used_percent >= 0.0 && disk_info.used_percent <= 100.0);
}

#[tokio::test]
async fn test_symlink_validation() {
    use super::health::check_symlinks;
    use std::path::Path;
    
    let models_path = Path::new("./models");
    let symlinks_valid = check_symlinks(models_path).await;
    
    println!("Symlinks valid: {}", symlinks_valid);
    
    // Should return true since no current directory exists or symlinks are valid
    assert!(symlinks_valid);
}

#[tokio::test]
async fn test_model_integrity() {
    use super::health::validate_model_integrity;
    use std::path::Path;
    
    let models_path = Path::new("./models");
    
    if models_path.exists() {
        // Test some existing model directories
        let production_path = models_path.join("production");
        let checkpoints_path = models_path.join("checkpoints");
        
        if production_path.exists() {
            let integrity_result = validate_model_integrity(&production_path).await;
            println!("Production models integrity: {}", integrity_result);
        }
        
        if checkpoints_path.exists() {
            let integrity_result = validate_model_integrity(&checkpoints_path).await;
            println!("Checkpoints models integrity: {}", integrity_result);
        }
    }
}