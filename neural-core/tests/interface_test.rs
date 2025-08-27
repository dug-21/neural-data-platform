use neural_core::interfaces::*;
use tokio_test;

#[tokio::test]
async fn test_market_data_service_mock() {
    let mock = MockMarketDataService::expect_healthy();
    let result = mock.get_health_status().await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "healthy");
}

#[tokio::test]
async fn test_feature_engineering_service_mock() {
    let mock = MockFeatureEngineeringService::expect_healthy();
    let result = mock.get_health_status().await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "healthy");
}

#[tokio::test]
async fn test_model_management_service_mock() {
    let mock = MockModelManagementService::expect_healthy();
    let result = mock.get_health_status().await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "healthy");
}

#[tokio::test]
async fn test_trading_service_mock() {
    let mock = MockTradingService::expect_healthy();
    let result = mock.get_health_status().await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "healthy");
}

#[test]
fn test_service_error_retryable() {
    let error = ServiceError::ServiceUnavailable {
        service_name: "test".to_string(),
        reason: "maintenance".to_string(),
    };
    assert!(error.is_retryable());
    assert_eq!(error.retry_after_seconds(), Some(30));
    
    let error = ServiceError::NotFound {
        resource_type: "user".to_string(),
        resource_id: "123".to_string(),
    };
    assert!(!error.is_retryable());
    assert_eq!(error.retry_after_seconds(), None);
}

#[test]
fn test_symbol_creation() {
    let symbol: Symbol = "AAPL".into();
    assert_eq!(symbol.to_string(), "AAPL");
    
    let symbol = Symbol::from("GOOGL".to_string());
    assert_eq!(symbol.to_string(), "GOOGL");
}

#[test]
fn test_retry_config_default() {
    let config = RetryConfig::default();
    assert_eq!(config.max_attempts, 3);
    assert_eq!(config.base_delay, std::time::Duration::from_millis(100));
    assert_eq!(config.max_delay, std::time::Duration::from_secs(30));
    assert_eq!(config.backoff_multiplier, 2.0);
    assert!(config.jitter);
}

#[test]
fn test_time_range_creation() {
    use neural_core::interfaces::{TimeRange, Symbol, ServiceError};
    
    let now = chrono::Utc::now();
    let time_range = TimeRange {
        start: now - chrono::Duration::hours(1),
        end: now,
    };
    assert!(time_range.start < time_range.end);
    
    let symbol = Symbol::from("AAPL");
    assert_eq!(symbol.to_string(), "AAPL");
    
    let error = ServiceError::Internal {
        message: "Test error".to_string(),
    };
    assert!(matches!(error, ServiceError::Internal { .. }));
}