//! TDD Tests for MCP System Status Tool

use anyhow::Result;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use autonomous_platform::mcp::trading_tools::TradingMcpTools;
use autonomous_platform::monitoring::{HealthMonitor, ComponentType, HealthStatus};
use autonomous_platform::config::load_default_config;

#[tokio::test]
async fn test_system_status_all_healthy() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let monitor = Arc::new(HealthMonitor::new(config.clone()));
    let tools = TradingMcpTools::with_monitor(monitor.clone());
    
    // Start monitoring
    monitor.start_monitoring().await?;
    sleep(Duration::from_millis(100)).await;
    
    // Act
    let params = json!({
        "detailed": true
    });
    
    let result = tools.system_status(params).await?;
    
    // Assert
    assert_eq!(result["status"], "operational");
    assert!(result["uptime_seconds"].as_u64().unwrap() > 0);
    assert!(result["components"].is_object());
    
    let components = result["components"].as_object().unwrap();
    assert!(components.contains_key("database"));
    assert!(components.contains_key("cache"));
    assert!(components.contains_key("neural"));
    assert!(components.contains_key("agents"));
    assert!(components.contains_key("data_pipeline"));
    
    // Detailed status should include metrics
    assert!(result["metrics"].is_object());
    assert!(result["performance"].is_object());
    
    Ok(())
}

#[tokio::test]
async fn test_system_status_with_component_issues() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let monitor = Arc::new(HealthMonitor::new(config.clone()));
    let tools = TradingMcpTools::with_monitor(monitor.clone());
    
    // Simulate component issue
    monitor.update_component_health(
        ComponentType::Database,
        HealthStatus::Degraded("High latency detected".to_string())
    ).await;
    
    // Act
    let params = json!({
        "detailed": true,
        "include_alerts": true
    });
    
    let result = tools.system_status(params).await?;
    
    // Assert
    assert_eq!(result["status"], "degraded");
    assert_eq!(result["components"]["database"]["status"], "degraded");
    assert!(result["components"]["database"]["message"].is_string());
    
    assert!(result["alerts"].is_array());
    let alerts = result["alerts"].as_array().unwrap();
    assert!(!alerts.is_empty());
    
    Ok(())
}

#[tokio::test]
async fn test_system_status_performance_metrics() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let monitor = Arc::new(HealthMonitor::new(config.clone()));
    let tools = TradingMcpTools::with_monitor(monitor.clone());
    
    // Generate some activity
    for _ in 0..10 {
        monitor.record_request_latency("api", Duration::from_millis(50)).await;
        monitor.increment_processed_count("market_data").await;
    }
    
    // Act
    let params = json!({
        "metrics_window": "1m"
    });
    
    let result = tools.system_status(params).await?;
    
    // Assert
    assert!(result["performance"]["avg_latency_ms"].is_number());
    assert!(result["performance"]["requests_per_second"].is_number());
    assert!(result["performance"]["processed_items"].is_object());
    
    let processed = result["performance"]["processed_items"].as_object().unwrap();
    assert_eq!(processed["market_data"], 10);
    
    Ok(())
}

#[tokio::test]
async fn test_system_status_resource_usage() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let monitor = Arc::new(HealthMonitor::new(config.clone()));
    let tools = TradingMcpTools::with_monitor(monitor.clone());
    
    // Act
    let params = json!({
        "include_resources": true
    });
    
    let result = tools.system_status(params).await?;
    
    // Assert
    assert!(result["resources"].is_object());
    
    let resources = result["resources"].as_object().unwrap();
    assert!(resources["cpu_usage_percent"].is_number());
    assert!(resources["memory_used_mb"].is_number());
    assert!(resources["memory_total_mb"].is_number());
    assert!(resources["disk_usage"].is_object());
    
    // Verify resource values are reasonable
    let cpu = resources["cpu_usage_percent"].as_f64().unwrap();
    assert!(cpu >= 0.0 && cpu <= 100.0);
    
    let mem_used = resources["memory_used_mb"].as_u64().unwrap();
    let mem_total = resources["memory_total_mb"].as_u64().unwrap();
    assert!(mem_used <= mem_total);
    
    Ok(())
}

#[tokio::test]
async fn test_system_status_trading_metrics() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let monitor = Arc::new(HealthMonitor::new(config.clone()));
    let tools = TradingMcpTools::with_monitor(monitor.clone());
    
    // Record trading activity
    monitor.record_trade("BTC/USD", "buy", 0.1, 45000.0).await;
    monitor.record_trade("ETH/USD", "sell", 1.0, 3000.0).await;
    monitor.record_prediction_accuracy(0.85).await;
    
    // Act
    let params = json!({
        "include_trading_stats": true
    });
    
    let result = tools.system_status(params).await?;
    
    // Assert
    assert!(result["trading_stats"].is_object());
    
    let stats = result["trading_stats"].as_object().unwrap();
    assert_eq!(stats["total_trades"], 2);
    assert!(stats["active_positions"].is_array());
    assert!(stats["prediction_accuracy"].as_f64().unwrap() == 0.85);
    assert!(stats["volume_24h"].is_number());
    
    Ok(())
}