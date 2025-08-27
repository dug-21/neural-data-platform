//! Integration Tests for Data-Staging Service
//! 
//! End-to-end tests that verify the complete data flow from Redis consumption
//! to EventBus publishing with all transformation and validation steps.

use data_staging::*;
use std::collections::HashMap;
use tokio_test;

#[tokio::test]
#[ignore] // Requires Redis and EventBus infrastructure
async fn test_end_to_end_data_flow() {
    // Setup test configuration
    let config = DataStagingConfig {
        redis_url: "redis://localhost:6379".to_string(),
        input_stream: "test_market_data_raw".to_string(),
        consumer_group: "test_data_staging".to_string(),
        consumer_name: "test_consumer_1".to_string(),
        eventbus_config: EventBusConfig {
            output_topic: "test_market_data_proto".to_string(),
            connection_timeout_ms: 5000,
            publish_timeout_ms: 1000,
        },
        quality_thresholds: QualityThresholds {
            minimum_quality_score: 0.7,
            max_age_seconds: 300,
            required_fields: vec![
                "symbol".to_string(),
                "price".to_string(),
                "timestamp".to_string(),
            ],
        },
        processing_limits: ProcessingLimits {
            max_batch_size: 10,
            message_timeout_ms: 1000,
            max_retries: 3,
        },
    };
    
    // Create data staging service
    let mut service = DataStagingService::new(config).await.expect("Failed to create service");
    
    // Test data flow with valid market data
    let test_json = r#"
    {
        "symbol": "AAPL",
        "price": 150.25,
        "volume": 1000.0,
        "timestamp": 1640995200,
        "bid": 150.20,
        "ask": 150.30,
        "exchange": "NASDAQ",
        "sequence": 12345
    }
    "#;
    
    // Simulate processing a single message
    let result = service.process_message(test_json).await;
    assert!(result.is_ok(), "Valid message should process successfully");
    
    // Test with invalid data
    let invalid_json = r#"
    {
        "symbol": "AAPL",
        "price": -150.25,
        "volume": 1000.0,
        "timestamp": 1640995200
    }
    "#;
    
    let result = service.process_message(invalid_json).await;
    assert!(result.is_err(), "Invalid message should fail processing");
}

#[tokio::test]
async fn test_json_validation_integration() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 300,
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
        ],
    };
    
    let validator = json_validator::JsonValidator::new(&thresholds);
    
    // Test valid data
    let valid_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: Some(1000.0),
        timestamp: Some(chrono::Utc::now().timestamp()),
        bid: Some(150.20),
        ask: Some(150.30),
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: Some(151.0),
        low: Some(149.5),
        open: Some(150.0),
        close: Some(150.25),
        vwap: Some(150.1),
        metadata: HashMap::new(),
    };
    
    let validation_result = validator.validate(&valid_data);
    assert!(validation_result.is_ok());
    
    // Test quality scoring integration
    let scorer = quality_scorer::QualityScorer::new(&thresholds);
    let quality_metrics = scorer.calculate_quality(&valid_data);
    
    assert!(quality_metrics.overall_score > 0.8);
    assert!(quality_metrics.validity_score > 0.9);
    assert_eq!(quality_metrics.missing_required_fields, 0);
}

#[tokio::test]
async fn test_proto_transformation_integration() {
    let transformer = proto_transformer::ProtoTransformer::new();
    
    let raw_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: Some(1000.0),
        timestamp: Some(chrono::Utc::now().timestamp()),
        bid: Some(150.20),
        ask: Some(150.30),
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: None,
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata: HashMap::new(),
    };
    
    let quality_metrics = DataQualityMetrics {
        overall_score: 0.9,
        freshness_score: 0.95,
        completeness_score: 0.85,
        validity_score: 1.0,
        missing_required_fields: 0,
        present_optional_fields: 6,
        data_age_seconds: 10,
        validation_errors: vec![],
    };
    
    let result = transformer.transform_to_event_envelope(&raw_data, &quality_metrics);
    assert!(result.is_ok());
    
    let envelope = result.unwrap();
    assert!(!envelope.message_id.is_empty());
    assert_eq!(envelope.source, "data-staging");
    assert_eq!(envelope.domain, "market-data");
    assert!(envelope.payload.is_some());
    assert!(envelope.created_at.is_some());
    assert!(envelope.routing.is_some());
    assert!(envelope.quality.is_some());
}

#[tokio::test]
async fn test_quality_scoring_integration() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 300,
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
        ],
    };
    
    let scorer = quality_scorer::QualityScorer::new(&thresholds);
    
    // Test with high quality data
    let high_quality_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: Some(1000.0),
        timestamp: Some(chrono::Utc::now().timestamp() - 10), // 10 seconds old
        bid: Some(150.20),
        ask: Some(150.30),
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: Some(151.0),
        low: Some(149.5),
        open: Some(150.0),
        close: Some(150.25),
        vwap: Some(150.1),
        metadata: HashMap::new(),
    };
    
    let metrics = scorer.calculate_quality(&high_quality_data);
    assert!(metrics.overall_score >= thresholds.minimum_quality_score);
    assert!(metrics.freshness_score > 0.9);
    assert!(metrics.validity_score > 0.9);
    
    // Test with low quality data
    let low_quality_data = RawMarketData {
        symbol: None, // Missing required field
        price: Some(-150.25), // Invalid price
        volume: Some(-1000.0), // Invalid volume
        timestamp: Some(chrono::Utc::now().timestamp() - 1000), // Too old
        bid: Some(150.30), // Invalid spread
        ask: Some(150.20),
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: None,
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata: HashMap::new(),
    };
    
    let metrics = scorer.calculate_quality(&low_quality_data);
    assert!(metrics.overall_score < thresholds.minimum_quality_score);
    assert!(metrics.missing_required_fields > 0);
    assert!(!metrics.validation_errors.is_empty());
}

#[tokio::test]
#[ignore] // Requires EventBus infrastructure
async fn test_eventbus_publishing_integration() {
    let config = EventBusConfig {
        output_topic: "test_market_data_proto".to_string(),
        connection_timeout_ms: 5000,
        publish_timeout_ms: 1000,
    };
    
    let mut publisher = eventbus_publisher::EventBusPublisher::new(&config).await
        .expect("Failed to create EventBus publisher");
    
    // Create test envelope
    let envelope = create_test_event_envelope();
    
    // Test publishing
    let result = publisher.publish_proto(envelope).await;
    assert!(result.is_ok(), "Publishing valid envelope should succeed");
    
    // Check stats
    let stats = publisher.get_stats().await;
    assert_eq!(stats.published_count, 1);
    assert_eq!(stats.failed_count, 0);
    assert!(stats.is_healthy());
}

#[tokio::test]
#[ignore] // Requires Redis infrastructure
async fn test_dlq_integration() {
    let config = DataStagingConfig::default();
    let mut dlq_manager = dlq_manager::DlqManager::new(&config).await
        .expect("Failed to create DLQ manager");
    
    let original_data = r#"{"symbol": "AAPL", "price": -150.25}"#;
    let error_message = "Validation failed: negative price";
    
    // Send to DLQ
    let result = dlq_manager.send_to_dlq(original_data, error_message).await;
    assert!(result.is_ok(), "Sending to DLQ should succeed");
    
    // Get DLQ stats
    let stats = dlq_manager.get_dlq_stats().await.expect("Failed to get DLQ stats");
    assert!(stats.total_messages > 0);
}

#[tokio::test]
async fn test_metrics_integration() {
    let metrics = metrics::MetricsCollector::new().expect("Failed to create metrics collector");
    
    // Record some operations
    let quality_metrics = DataQualityMetrics {
        overall_score: 0.9,
        freshness_score: 0.95,
        completeness_score: 0.85,
        validity_score: 1.0,
        missing_required_fields: 0,
        present_optional_fields: 8,
        data_age_seconds: 10,
        validation_errors: vec![],
    };
    
    metrics.record_message_processed(&quality_metrics).await;
    metrics.record_eventbus_publish_success(0.005).await;
    metrics.record_batch_processed(5).await;
    
    // Check metrics summary
    let summary = metrics.get_metrics_summary().await;
    assert_eq!(summary.messages_processed, 1);
    assert_eq!(summary.eventbus_publishes, 1);
    assert_eq!(summary.current_quality_score, 0.9);
    
    // Test metrics export
    let exported = metrics.export_metrics().await.expect("Failed to export metrics");
    assert!(exported.contains("data_staging_messages_processed_total"));
}

#[tokio::test]
async fn test_error_flow_integration() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.9, // High threshold
        max_age_seconds: 60, // Short time window
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
            "volume".to_string(),
        ],
    };
    
    let validator = json_validator::JsonValidator::new(&thresholds);
    let scorer = quality_scorer::QualityScorer::new(&thresholds);
    
    // Test data that should fail validation and quality checks
    let problematic_data = RawMarketData {
        symbol: Some("".to_string()), // Empty symbol
        price: Some(-100.0), // Negative price
        volume: None, // Missing required field
        timestamp: Some(chrono::Utc::now().timestamp() - 3600), // Too old
        bid: Some(150.30), // Invalid spread
        ask: Some(150.20),
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: None,
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata: HashMap::new(),
    };
    
    // Validation should fail
    let validation_result = validator.validate(&problematic_data);
    assert!(validation_result.is_err());
    
    // Quality scoring should identify issues
    let quality_metrics = scorer.calculate_quality(&problematic_data);
    assert!(quality_metrics.overall_score < thresholds.minimum_quality_score);
    assert!(quality_metrics.missing_required_fields > 0);
    assert!(!quality_metrics.validation_errors.is_empty());
}

#[tokio::test]
async fn test_batch_processing_integration() {
    let config = DataStagingConfig {
        processing_limits: ProcessingLimits {
            max_batch_size: 5,
            message_timeout_ms: 1000,
            max_retries: 3,
        },
        ..Default::default()
    };
    
    // Simulate batch processing logic
    let batch_messages = vec![
        r#"{"symbol": "AAPL", "price": 150.25, "timestamp": 1640995200}"#,
        r#"{"symbol": "GOOGL", "price": 2800.50, "timestamp": 1640995201}"#,
        r#"{"symbol": "MSFT", "price": 330.75, "timestamp": 1640995202}"#,
        r#"{"symbol": "INVALID", "price": -100.0, "timestamp": 1640995203}"#, // Should fail
        r#"{"symbol": "TSLA", "price": 1000.0, "timestamp": 1640995204}"#,
    ];
    
    let validator = json_validator::JsonValidator::new(&config.quality_thresholds);
    let scorer = quality_scorer::QualityScorer::new(&config.quality_thresholds);
    let transformer = proto_transformer::ProtoTransformer::new();
    
    let mut successful_transformations = 0;
    let mut failed_validations = 0;
    
    for json_data in batch_messages {
        // Parse JSON
        match serde_json::from_str::<RawMarketData>(json_data) {
            Ok(raw_data) => {
                // Validate
                match validator.validate(&raw_data) {
                    Ok(()) => {
                        // Score quality
                        let quality_metrics = scorer.calculate_quality(&raw_data);
                        
                        if quality_metrics.overall_score >= config.quality_thresholds.minimum_quality_score {
                            // Transform to proto
                            match transformer.transform_to_event_envelope(&raw_data, &quality_metrics) {
                                Ok(_envelope) => {
                                    successful_transformations += 1;
                                }
                                Err(_) => {
                                    failed_validations += 1;
                                }
                            }
                        } else {
                            failed_validations += 1;
                        }
                    }
                    Err(_) => {
                        failed_validations += 1;
                    }
                }
            }
            Err(_) => {
                failed_validations += 1;
            }
        }
    }
    
    assert_eq!(successful_transformations, 4); // 4 valid messages
    assert_eq!(failed_validations, 1); // 1 invalid message
}

// Helper function to create test EventEnvelope
fn create_test_event_envelope() -> generated::EventEnvelope {
    use prost_types::Timestamp;
    use std::collections::HashMap;
    
    let now = chrono::Utc::now();
    
    generated::EventEnvelope {
        message_id: "test-integration-123".to_string(),
        correlation_id: "test-correlation".to_string(),
        source: "data-staging".to_string(),
        domain: "market-data".to_string(),
        event_type: "MarketDataEvent".to_string(),
        schema_version: "1.0".to_string(),
        created_at: Some(Timestamp {
            seconds: now.timestamp(),
            nanos: 0,
        }),
        ingested_at: Some(Timestamp {
            seconds: now.timestamp(),
            nanos: 0,
        }),
        routing: Some(generated::RoutingMetadata {
            topic: "test_market_data_proto".to_string(),
            partition_key: "AAPL".to_string(),
            priority: 1,
            ttl_seconds: 300,
            tags: vec!["test".to_string()],
            retry_policy: None,
        }),
        quality: Some(generated::QualityMetadata {
            completeness: 90.0,
            latency_ms: 100,
            validation_status: generated::ValidationStatus::Passed as i32,
            quality_score: 85.0,
            anomalies: vec![],
        }),
        payload: Some(prost_types::Any {
            type_url: "type.googleapis.com/test".to_string(),
            value: b"test integration payload".to_vec(),
        }),
        headers: HashMap::new(),
        tracing: None,
    }
}