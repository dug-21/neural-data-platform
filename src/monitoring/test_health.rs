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

// Removed test_disk_space_check, test_symlink_validation and test_model_integrity 
// - The underlying functions (check_disk_space, check_symlinks, validate_model_integrity) 
// - were removed from the health module during architectural simplification