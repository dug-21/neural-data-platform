use autonomous_platform::monitoring::health::*;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    println!("🔍 Testing Neural Trader Health Monitoring System");
    println!("================================================");
    
    // Create health monitor
    let monitor = Arc::new(HealthMonitor::new().await?);
    
    // Test neural system health check
    println!("\n🧠 Checking Neural System Health...");
    let neural_health = monitor.check_component_health(ComponentType::NeuralSystem).await?;
    
    println!("Status: {}", neural_health.status);
    println!("Response Time: {:?}ms", neural_health.response_time_ms);
    
    println!("\nMetadata:");
    for (key, value) in &neural_health.metadata {
        println!("  {}: {}", key, value);
    }
    
    // Test overall system health
    println!("\n📊 Checking Overall System Health...");
    let system_health = monitor.get_system_health().await?;
    
    println!("Overall Status: {}", system_health.overall_status);
    println!("Health Score: {:.2}", system_health.health_score());
    println!("Components: {} total, {} healthy, {} degraded, {} unhealthy", 
             system_health.total_components,
             system_health.healthy_components,
             system_health.degraded_components,
             system_health.unhealthy_components);
    
    // Test Prometheus metrics
    println!("\n📈 Testing Prometheus Metrics...");
    let endpoints = HealthEndpoints::new(monitor.clone());
    let metrics = endpoints.metrics_endpoint().await?;
    
    // Extract just model-related metrics
    let model_metrics: Vec<&str> = metrics
        .lines()
        .filter(|line| line.contains("neural_trader_"))
        .collect();
    
    println!("Model Storage Metrics:");
    for metric in model_metrics {
        println!("  {}", metric);
    }
    
    // Test health endpoints
    println!("\n🌐 Testing Health Endpoints...");
    
    // Basic health endpoint
    let health_response = endpoints.health_endpoint().await?;
    println!("Health Endpoint Response:");
    println!("{}", health_response);
    
    // Components endpoint
    let components_response = endpoints.components_endpoint().await?;
    println!("\nComponents Endpoint Response:");
    println!("{}", components_response);
    
    println!("\n✅ All health monitoring tests completed successfully!");
    
    Ok(())
}