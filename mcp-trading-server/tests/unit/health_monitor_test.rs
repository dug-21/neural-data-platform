use mcp_trading_server::tools::health::HealthMonitorTool;
use mcp_trading_server::integrations::monitor::MonitorClient;
use serde_json::json;

#[tokio::test]
async fn test_get_system_status() {
    // Arrange
    let monitor_client = setup_test_monitor_client();
    let health_tool = HealthMonitorTool::new(monitor_client);
    
    // Act
    let result = health_tool.get_system_status().await;
    
    // Assert
    assert!(result.is_ok());
    let status = result.unwrap();
    assert!(["healthy", "degraded", "unhealthy"].contains(&status["overall_status"].as_str().unwrap()));
    assert!(status["components"].as_object().is_some());
    assert!(status["timestamp"].as_str().is_some());
}

#[tokio::test]
async fn test_get_component_health() {
    // Arrange
    let monitor_client = setup_test_monitor_client();
    let health_tool = HealthMonitorTool::new(monitor_client);
    
    // Act
    let components = vec!["database", "redis", "neural", "agents"];
    for component in components {
        let result = health_tool.get_component_health(component).await;
        
        // Assert
        assert!(result.is_ok());
        let health = result.unwrap();
        assert_eq!(health["component"], component);
        assert!(health["status"].as_str().is_some());
        assert!(health["latency_ms"].as_f64().is_some());
        assert!(health["last_check"].as_str().is_some());
    }
}

#[tokio::test]
async fn test_get_performance_metrics() {
    // Arrange
    let monitor_client = setup_test_monitor_client();
    let health_tool = HealthMonitorTool::new(monitor_client);
    
    // Act
    let result = health_tool.get_performance_metrics("5m").await;
    
    // Assert
    assert!(result.is_ok());
    let metrics = result.unwrap();
    assert!(metrics["cpu_usage"].as_f64().is_some());
    assert!(metrics["memory_usage"].as_f64().is_some());
    assert!(metrics["disk_usage"].as_f64().is_some());
    assert!(metrics["network_io"].as_object().is_some());
    assert!(metrics["api_latency"].as_object().is_some());
}

#[tokio::test]
async fn test_get_error_logs() {
    // Arrange
    let monitor_client = setup_test_monitor_client();
    let health_tool = HealthMonitorTool::new(monitor_client);
    
    // Act
    let result = health_tool.get_error_logs(10).await;
    
    // Assert
    assert!(result.is_ok());
    let logs = result.unwrap();
    assert!(logs.as_array().is_some());
    
    if let Some(log_array) = logs.as_array() {
        for log in log_array {
            assert!(log["timestamp"].as_str().is_some());
            assert!(log["level"].as_str().is_some());
            assert!(log["message"].as_str().is_some());
            assert!(log["component"].as_str().is_some());
        }
    }
}

#[tokio::test]
async fn test_get_alert_status() {
    // Arrange
    let monitor_client = setup_test_monitor_client();
    let health_tool = HealthMonitorTool::new(monitor_client);
    
    // Act
    let result = health_tool.get_alert_status().await;
    
    // Assert
    assert!(result.is_ok());
    let alerts = result.unwrap();
    assert!(alerts["active_alerts"].as_array().is_some());
    assert!(alerts["alert_count"].as_u64().is_some());
    
    if let Some(alert_array) = alerts["active_alerts"].as_array() {
        for alert in alert_array {
            assert!(alert["id"].as_str().is_some());
            assert!(alert["severity"].as_str().is_some());
            assert!(alert["message"].as_str().is_some());
            assert!(alert["triggered_at"].as_str().is_some());
        }
    }
}

#[tokio::test]
async fn test_run_health_check() {
    // Arrange
    let monitor_client = setup_test_monitor_client();
    let health_tool = HealthMonitorTool::new(monitor_client);
    
    // Act
    let result = health_tool.run_health_check().await;
    
    // Assert
    assert!(result.is_ok());
    let check_result = result.unwrap();
    assert!(check_result["checks_passed"].as_u64().is_some());
    assert!(check_result["checks_failed"].as_u64().is_some());
    assert!(check_result["details"].as_array().is_some());
}

fn setup_test_monitor_client() -> MonitorClient {
    // Create monitor client with actual system monitoring
    MonitorClient::new()
}