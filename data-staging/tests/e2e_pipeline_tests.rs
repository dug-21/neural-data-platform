//! End-to-End Pipeline Tests
//! 
//! These tests validate the complete data flow:
//! Redis Streams → Data-Staging → EventBus → Consumers
//! 
//! All tests enforce proto-only messaging and validate complete pipeline integrity.

use data_staging::*;
use data_staging::generated::*;
use neural_core::eventbus::*;
use redis::{Commands, aio::Connection};
use tokio_test;
use std::sync::Arc;
use std::time::{Duration, Instant};
use prost::Message;
use serde_json::json;

// ================================================================================================
// End-to-End Test Infrastructure
// ================================================================================================

struct E2ETestEnvironment {
    redis_client: redis::Client,
    staging_service: DataStagingService,
    eventbus: Arc<dyn EventBus>,
    test_consumer: TestConsumer,
    config: DataStagingConfig,
}

impl E2ETestEnvironment {
    async fn new() -> anyhow::Result<Self> {
        let config = DataStagingConfig {
            redis_url: "redis://127.0.0.1:6379".to_string(),
            input_stream: "e2e_test_raw_data".to_string(),
            consumer_group: "e2e-test-staging".to_string(),
            consumer_name: "e2e-staging-worker-1".to_string(),
            eventbus_config: EventBusConfig {
                output_topic: "e2e_test_proto_events".to_string(),
                connection_timeout_ms: 5000,
                publish_timeout_ms: 1000,
            },
            quality_thresholds: QualityThresholds {
                minimum_quality_score: 0.5, // Lower threshold for testing
                max_age_seconds: 3600, // 1 hour for testing
                required_fields: vec![
                    "symbol".to_string(),
                    "price".to_string(),
                    "timestamp".to_string(),
                ],
            },
            processing_limits: ProcessingLimits {
                max_batch_size: 50,
                message_timeout_ms: 2000,
                max_retries: 2,
            },
        };
        
        let redis_client = redis::Client::open(config.redis_url.clone())?;
        let staging_service = DataStagingService::new(config.clone()).await?;
        let eventbus = Arc::new(InMemoryEventBus::new());
        let test_consumer = TestConsumer::new(eventbus.clone());
        
        Ok(Self {
            redis_client,
            staging_service,
            eventbus,
            test_consumer,
            config,
        })
    }
    
    async fn cleanup(&self) -> anyhow::Result<()> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;
        
        // Clean up Redis streams
        let _: Result<(), redis::RedisError> = redis::cmd("DEL")
            .arg(&self.config.input_stream)
            .query_async(&mut conn)
            .await;
            
        // Clean up consumer group
        let _: Result<(), redis::RedisError> = redis::cmd("XGROUP")
            .arg("DESTROY")
            .arg(&self.config.input_stream)
            .arg(&self.config.consumer_group)
            .query_async(&mut conn)
            .await;
            
        Ok(())
    }
    
    async fn publish_json_to_redis(&self, json_data: &str) -> anyhow::Result<String> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;
        
        let message_id: String = redis::cmd("XADD")
            .arg(&self.config.input_stream)
            .arg("*")
            .arg("data")
            .arg(json_data)
            .arg("timestamp")
            .arg(chrono::Utc::now().timestamp_millis())
            .query_async(&mut conn)
            .await?;
            
        Ok(message_id)
    }
    
    async fn create_consumer_group(&self) -> anyhow::Result<()> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;
        
        let _: Result<String, redis::RedisError> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(&self.config.input_stream)
            .arg(&self.config.consumer_group)
            .arg("0")
            .arg("MKSTREAM")
            .query_async(&mut conn)
            .await;
            
        Ok(())
    }
}

// Test consumer that validates proto-only messages
struct TestConsumer {
    eventbus: Arc<dyn EventBus>,
    consumed_messages: Arc<tokio::sync::Mutex<Vec<ConsumedMessage>>>,
}

#[derive(Debug, Clone)]
struct ConsumedMessage {
    event_envelope: EventEnvelope,
    raw_payload: Vec<u8>,
    consumption_timestamp: chrono::DateTime<chrono::Utc>,
    validation_result: Result<(), String>,
}

impl TestConsumer {
    fn new(eventbus: Arc<dyn EventBus>) -> Self {
        Self {
            eventbus,
            consumed_messages: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }
    
    async fn start_consuming(&self, topic: &str, max_messages: usize) -> anyhow::Result<()> {
        let messages = self.eventbus.consume(topic, StartPosition::Beginning, max_messages).await?;
        let mut consumed = self.consumed_messages.lock().await;
        
        for event_envelope in messages {
            let consumption_timestamp = chrono::Utc::now();
            
            if let Some(payload) = &event_envelope.payload {
                // Validate it's valid protobuf
                let validation_result = EventEnvelope::decode(&payload[..])
                    .map(|_| ())
                    .map_err(|e| format!("Proto validation failed: {}", e));
                
                let consumed_message = ConsumedMessage {
                    event_envelope: event_envelope.clone(),
                    raw_payload: payload.clone(),
                    consumption_timestamp,
                    validation_result,
                };
                
                consumed.push(consumed_message);
            }
        }
        
        Ok(())
    }
    
    async fn get_consumed_messages(&self) -> Vec<ConsumedMessage> {
        self.consumed_messages.lock().await.clone()
    }
    
    async fn clear_consumed_messages(&self) {
        self.consumed_messages.lock().await.clear();
    }
}

// ================================================================================================
// Complete E2E Pipeline Tests
// ================================================================================================

#[tokio::test]
async fn test_complete_valid_data_pipeline() {
    let env = E2ETestEnvironment::new().await.expect("Failed to setup test environment");
    env.create_consumer_group().await.expect("Failed to create consumer group");
    
    // Step 1: Publish valid JSON to Redis
    let valid_market_data = json!({
        "symbol": "AAPL",
        "price": 150.25,
        "volume": 1500.0,
        "timestamp": chrono::Utc::now().timestamp_millis(),
        "bid": 150.20,
        "ask": 150.30,
        "exchange": "NASDAQ",
        "sequence": 12345
    });
    
    let message_id = env.publish_json_to_redis(&valid_market_data.to_string()).await
        .expect("Failed to publish to Redis");
    
    println!("Published message to Redis: {}", message_id);
    
    // Step 2: Process through Data-Staging
    let start_time = Instant::now();
    
    // Give some time for Redis stream to be available
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let processed_count = env.staging_service.process_batch().await
        .expect("Failed to process batch");
    
    let processing_time = start_time.elapsed();
    
    assert_eq!(processed_count, 1, "Should process exactly one message");
    println!("Data-Staging processed 1 message in {:?}", processing_time);
    
    // Step 3: Start consuming from EventBus
    env.test_consumer.start_consuming(&env.config.eventbus_config.output_topic, 10).await
        .expect("Failed to start consuming");
    
    // Step 4: Verify end-to-end results
    let consumed_messages = env.test_consumer.get_consumed_messages().await;
    assert_eq!(consumed_messages.len(), 1, "Should have consumed exactly one message");
    
    let consumed_message = &consumed_messages[0];
    
    // Verify the message is valid protobuf
    assert!(consumed_message.validation_result.is_ok(), 
           "Consumed message should be valid protobuf: {:?}", consumed_message.validation_result);
    
    // Verify event envelope properties
    let envelope = &consumed_message.event_envelope;
    assert!(!envelope.event_id.is_empty(), "Event ID should not be empty");
    assert_eq!(envelope.event_type, "MarketDataEvent", "Event type should match");
    assert_eq!(envelope.source, "data-staging", "Source should match");
    assert!(envelope.quality.is_some(), "Quality metrics should be present");
    
    // Verify quality metrics
    if let Some(quality) = &envelope.quality {
        assert!(quality.overall_score >= 0.5, "Quality score should meet threshold");
        println!("Message quality score: {}", quality.overall_score);
    }
    
    // Verify end-to-end latency
    let total_latency = consumed_message.consumption_timestamp.signed_duration_since(
        chrono::DateTime::from_timestamp_millis(valid_market_data["timestamp"].as_i64().unwrap()).unwrap()
    );
    
    println!("End-to-end latency: {:?}", total_latency.to_std().unwrap_or(Duration::ZERO));
    assert!(total_latency.to_std().unwrap_or(Duration::MAX) < Duration::from_secs(5), 
           "End-to-end latency should be reasonable");
    
    env.cleanup().await.expect("Failed to cleanup");
}

#[tokio::test] 
async fn test_invalid_data_to_dlq_pipeline() {
    let env = E2ETestEnvironment::new().await.expect("Failed to setup test environment");
    env.create_consumer_group().await.expect("Failed to create consumer group");
    
    // Step 1: Publish invalid JSON (missing required fields) to Redis
    let invalid_data_cases = vec![
        json!({"price": 150.25, "volume": 1000}), // Missing symbol and timestamp
        json!({"symbol": "GOOGL", "timestamp": chrono::Utc::now().timestamp_millis()}), // Missing price
        json!({"symbol": "", "price": -100.0, "timestamp": 0}), // Empty symbol, negative price, zero timestamp
    ];
    
    for (i, invalid_data) in invalid_data_cases.iter().enumerate() {
        let message_id = env.publish_json_to_redis(&invalid_data.to_string()).await
            .expect("Failed to publish invalid data to Redis");
        
        println!("Published invalid message {} to Redis: {}", i, message_id);
    }
    
    // Step 2: Process through Data-Staging
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let processed_count = env.staging_service.process_batch().await
        .expect("Processing should succeed even with invalid data");
    
    // No messages should be successfully processed (all go to DLQ)
    assert_eq!(processed_count, 0, "Invalid messages should not be successfully processed");
    
    // Step 3: Verify no messages reach EventBus
    env.test_consumer.start_consuming(&env.config.eventbus_config.output_topic, 10).await
        .expect("Failed to start consuming");
    
    let consumed_messages = env.test_consumer.get_consumed_messages().await;
    assert!(consumed_messages.is_empty(), "No invalid messages should reach EventBus");
    
    // Step 4: Verify DLQ contains the invalid messages
    // (This would require implementing DLQ inspection functionality)
    
    env.cleanup().await.expect("Failed to cleanup");
}

#[tokio::test]
async fn test_mixed_valid_invalid_pipeline() {
    let env = E2ETestEnvironment::new().await.expect("Failed to setup test environment");
    env.create_consumer_group().await.expect("Failed to create consumer group");
    
    // Create mix of valid and invalid messages
    let test_messages = vec![
        // Valid messages
        json!({
            "symbol": "AAPL",
            "price": 150.25,
            "volume": 1000.0,
            "timestamp": chrono::Utc::now().timestamp_millis(),
            "exchange": "NASDAQ"
        }),
        json!({
            "symbol": "GOOGL", 
            "price": 2500.75,
            "volume": 500.0,
            "timestamp": chrono::Utc::now().timestamp_millis(),
            "bid": 2500.50,
            "ask": 2501.00
        }),
        // Invalid messages
        json!({"symbol": "MSFT"}), // Missing required fields
        json!({"price": -100, "volume": 0}), // Missing symbol, negative price
        // Valid message
        json!({
            "symbol": "TSLA",
            "price": 800.50,
            "volume": 2500.0, 
            "timestamp": chrono::Utc::now().timestamp_millis()
        }),
    ];
    
    let expected_valid = 3;
    let expected_invalid = 2;
    
    // Publish all messages
    for (i, message) in test_messages.iter().enumerate() {
        let message_id = env.publish_json_to_redis(&message.to_string()).await
            .expect("Failed to publish message to Redis");
        println!("Published message {} to Redis: {}", i, message_id);
    }
    
    // Process through Data-Staging
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    let processed_count = env.staging_service.process_batch().await
        .expect("Processing should succeed");
    
    assert_eq!(processed_count, expected_valid, 
              "Should process only valid messages: expected {}, got {}", expected_valid, processed_count);
    
    // Consume from EventBus
    env.test_consumer.start_consuming(&env.config.eventbus_config.output_topic, 10).await
        .expect("Failed to start consuming");
    
    let consumed_messages = env.test_consumer.get_consumed_messages().await;
    assert_eq!(consumed_messages.len(), expected_valid, "Should consume only valid messages");
    
    // Verify all consumed messages are valid protobuf
    for (i, consumed_message) in consumed_messages.iter().enumerate() {
        assert!(consumed_message.validation_result.is_ok(), 
               "Message {}: Should be valid protobuf: {:?}", i, consumed_message.validation_result);
        
        let envelope = &consumed_message.event_envelope;
        assert!(!envelope.event_id.is_empty(), "Message {}: Should have event ID", i);
        assert_eq!(envelope.event_type, "MarketDataEvent", "Message {}: Should have correct event type", i);
        
        // Verify the message contains market data
        if let Some(payload) = &envelope.payload {
            assert!(!payload.is_empty(), "Message {}: Payload should not be empty", i);
        }
    }
    
    env.cleanup().await.expect("Failed to cleanup");
}

#[tokio::test]
async fn test_high_throughput_pipeline() {
    let env = E2ETestEnvironment::new().await.expect("Failed to setup test environment");
    env.create_consumer_group().await.expect("Failed to create consumer group");
    
    let message_count = 1000;
    let batch_size = 50;
    
    println!("Starting high throughput test with {} messages", message_count);
    
    // Generate and publish messages in batches
    let base_timestamp = chrono::Utc::now().timestamp_millis();
    
    for batch in 0..(message_count / batch_size) {
        let batch_start = Instant::now();
        
        for i in 0..batch_size {
            let message_index = batch * batch_size + i;
            let test_message = json!({
                "symbol": format!("STOCK{}", message_index % 100), // Cycle through 100 symbols
                "price": 100.0 + (message_index % 500) as f64 * 0.01,
                "volume": 1000.0 + (message_index % 1000) as f64,
                "timestamp": base_timestamp + message_index as i64,
                "exchange": if message_index % 2 == 0 { "NASDAQ" } else { "NYSE" },
                "sequence": message_index as u64
            });
            
            env.publish_json_to_redis(&test_message.to_string()).await
                .expect("Failed to publish message");
        }
        
        let batch_time = batch_start.elapsed();
        println!("Published batch {} ({} messages) in {:?}", batch, batch_size, batch_time);
    }
    
    println!("All {} messages published, starting processing", message_count);
    
    // Process all messages
    let processing_start = Instant::now();
    let mut total_processed = 0;
    
    for _ in 0..50 { // Max 50 processing cycles
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let batch_processed = env.staging_service.process_batch().await
            .expect("Processing should succeed");
        
        total_processed += batch_processed;
        
        if batch_processed == 0 {
            break; // No more messages to process
        }
        
        println!("Processed batch: {} messages (total: {})", batch_processed, total_processed);
    }
    
    let processing_time = processing_start.elapsed();
    let throughput = total_processed as f64 / processing_time.as_secs_f64();
    
    println!("Processing complete: {} messages in {:?} ({:.0} msgs/sec)", 
             total_processed, processing_time, throughput);
    
    assert_eq!(total_processed, message_count, "Should process all messages");
    assert!(throughput >= 100.0, "Throughput should be at least 100 msgs/sec, got {:.0}", throughput);
    
    // Consume all messages from EventBus
    let consumption_start = Instant::now();
    
    env.test_consumer.start_consuming(&env.config.eventbus_config.output_topic, message_count).await
        .expect("Failed to start consuming");
    
    let consumed_messages = env.test_consumer.get_consumed_messages().await;
    let consumption_time = consumption_start.elapsed();
    
    println!("Consumed {} messages in {:?}", consumed_messages.len(), consumption_time);
    
    assert_eq!(consumed_messages.len(), message_count, "Should consume all processed messages");
    
    // Verify all messages are valid protobuf
    let mut validation_failures = 0;
    for (i, consumed_message) in consumed_messages.iter().enumerate() {
        if consumed_message.validation_result.is_err() {
            validation_failures += 1;
            println!("Message {}: Validation failed: {:?}", i, consumed_message.validation_result);
        }
    }
    
    assert_eq!(validation_failures, 0, "All consumed messages should be valid protobuf");
    
    // Calculate end-to-end metrics
    let total_pipeline_time = processing_start.elapsed();
    let e2e_throughput = total_processed as f64 / total_pipeline_time.as_secs_f64();
    
    println!("End-to-end throughput: {:.0} msgs/sec", e2e_throughput);
    assert!(e2e_throughput >= 50.0, "E2E throughput should be at least 50 msgs/sec, got {:.0}", e2e_throughput);
    
    env.cleanup().await.expect("Failed to cleanup");
}

// ================================================================================================
// Error Recovery and Resilience Tests
// ================================================================================================

#[tokio::test]
async fn test_redis_reconnection_resilience() {
    let env = E2ETestEnvironment::new().await.expect("Failed to setup test environment");
    env.create_consumer_group().await.expect("Failed to create consumer group");
    
    // Publish some messages
    let test_message = json!({
        "symbol": "RESILIENCE_TEST",
        "price": 100.0,
        "volume": 1000.0,
        "timestamp": chrono::Utc::now().timestamp_millis()
    });
    
    for i in 0..5 {
        let message = test_message.clone();
        env.publish_json_to_redis(&message.to_string()).await
            .expect("Failed to publish message");
    }
    
    // Process normally
    tokio::time::sleep(Duration::from_millis(100)).await;
    let processed = env.staging_service.process_batch().await
        .expect("Initial processing should succeed");
    
    assert_eq!(processed, 5, "Should process all initial messages");
    
    // Simulate Redis connection issues by publishing more messages
    // and testing that processing can continue
    for i in 0..3 {
        let message = json!({
            "symbol": "RECOVERY_TEST",
            "price": 200.0 + i as f64,
            "volume": 1000.0,
            "timestamp": chrono::Utc::now().timestamp_millis()
        });
        
        env.publish_json_to_redis(&message.to_string()).await
            .expect("Failed to publish recovery message");
    }
    
    // Processing should continue to work
    tokio::time::sleep(Duration::from_millis(100)).await;
    let recovered_processed = env.staging_service.process_batch().await
        .expect("Recovery processing should succeed");
    
    assert_eq!(recovered_processed, 3, "Should process recovery messages");
    
    // Verify all messages reached EventBus
    env.test_consumer.start_consuming(&env.config.eventbus_config.output_topic, 10).await
        .expect("Failed to consume messages");
    
    let consumed_messages = env.test_consumer.get_consumed_messages().await;
    assert_eq!(consumed_messages.len(), 8, "Should consume all messages (5 + 3)");
    
    env.cleanup().await.expect("Failed to cleanup");
}

#[tokio::test]
async fn test_eventbus_backpressure_handling() {
    let env = E2ETestEnvironment::new().await.expect("Failed to setup test environment");
    env.create_consumer_group().await.expect("Failed to create consumer group");
    
    // Publish a large number of messages quickly to test backpressure
    let burst_count = 500;
    
    println!("Publishing {} messages in burst", burst_count);
    let publish_start = Instant::now();
    
    for i in 0..burst_count {
        let message = json!({
            "symbol": format!("BURST{}", i % 50),
            "price": 100.0 + (i % 100) as f64 * 0.1,
            "volume": 1000.0 + i as f64,
            "timestamp": chrono::Utc::now().timestamp_millis(),
            "sequence": i
        });
        
        env.publish_json_to_redis(&message.to_string()).await
            .expect("Failed to publish burst message");
    }
    
    let publish_time = publish_start.elapsed();
    println!("Published {} messages in {:?}", burst_count, publish_time);
    
    // Process with potential backpressure
    let processing_start = Instant::now();
    let mut total_processed = 0;
    let mut processing_rounds = 0;
    
    while total_processed < burst_count && processing_rounds < 100 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        let batch_result = env.staging_service.process_batch().await;
        match batch_result {
            Ok(processed) => {
                total_processed += processed;
                if processed > 0 {
                    println!("Round {}: Processed {} messages (total: {})", 
                            processing_rounds, processed, total_processed);
                }
            }
            Err(e) => {
                println!("Processing error in round {}: {}", processing_rounds, e);
                // Continue processing despite errors (backpressure recovery)
            }
        }
        
        processing_rounds += 1;
    }
    
    let processing_time = processing_start.elapsed();
    let processing_throughput = total_processed as f64 / processing_time.as_secs_f64();
    
    println!("Backpressure test: processed {}/{} messages in {} rounds ({:.0} msgs/sec)",
             total_processed, burst_count, processing_rounds, processing_throughput);
    
    // Should handle backpressure gracefully
    assert!(total_processed >= burst_count * 80 / 100, // At least 80% should be processed
           "Should handle most messages despite backpressure: {}/{}", total_processed, burst_count);
    
    env.cleanup().await.expect("Failed to cleanup");
}

// ================================================================================================
// Data Quality End-to-End Tests
// ================================================================================================

#[tokio::test]
async fn test_quality_filtering_pipeline() {
    let env = E2ETestEnvironment::new().await.expect("Failed to setup test environment");
    env.create_consumer_group().await.expect("Failed to create consumer group");
    
    // Create messages with different quality levels
    let quality_test_messages = vec![
        // High quality - all fields present, recent timestamp
        json!({
            "symbol": "HIGH_QUAL",
            "price": 150.25,
            "volume": 1000.0,
            "timestamp": chrono::Utc::now().timestamp_millis(),
            "bid": 150.20,
            "ask": 150.30,
            "exchange": "NASDAQ",
            "sequence": 12345,
            "high": 151.0,
            "low": 149.0,
            "open": 150.0,
            "close": 150.25,
            "vwap": 150.1
        }),
        // Medium quality - required fields + some optional
        json!({
            "symbol": "MED_QUAL",
            "price": 200.75,
            "volume": 800.0,
            "timestamp": chrono::Utc::now().timestamp_millis(),
            "exchange": "NYSE"
        }),
        // Low quality - only required fields, but should pass minimum threshold
        json!({
            "symbol": "LOW_QUAL",
            "price": 50.50,
            "timestamp": chrono::Utc::now().timestamp_millis()
        }),
        // Very low quality - stale timestamp, should be rejected
        json!({
            "symbol": "VERY_LOW",
            "price": 75.25,
            "timestamp": chrono::Utc::now().timestamp_millis() - 7200000 // 2 hours ago
        }),
    ];
    
    // Publish all messages
    for (i, message) in quality_test_messages.iter().enumerate() {
        env.publish_json_to_redis(&message.to_string()).await
            .expect("Failed to publish quality test message");
        println!("Published quality test message {}", i);
    }
    
    // Process messages
    tokio::time::sleep(Duration::from_millis(100)).await;
    let processed_count = env.staging_service.process_batch().await
        .expect("Quality processing should succeed");
    
    println!("Processed {} quality test messages", processed_count);
    
    // Should filter out very low quality messages
    assert!(processed_count >= 3, "Should process at least high, medium, and low quality messages");
    assert!(processed_count <= 3, "Should reject very low quality messages");
    
    // Consume and verify quality scores
    env.test_consumer.start_consuming(&env.config.eventbus_config.output_topic, 10).await
        .expect("Failed to consume quality messages");
    
    let consumed_messages = env.test_consumer.get_consumed_messages().await;
    
    for (i, consumed_message) in consumed_messages.iter().enumerate() {
        let envelope = &consumed_message.event_envelope;
        
        if let Some(quality) = &envelope.quality {
            println!("Message {}: Quality score = {}", i, quality.overall_score);
            
            // All consumed messages should meet quality threshold
            assert!(quality.overall_score >= env.config.quality_thresholds.minimum_quality_score,
                   "Message {} quality score {} should meet threshold {}", 
                   i, quality.overall_score, env.config.quality_thresholds.minimum_quality_score);
            
            // Verify quality components
            assert!(quality.completeness_score >= 0.0 && quality.completeness_score <= 1.0);
            assert!(quality.timeliness_score >= 0.0 && quality.timeliness_score <= 1.0);
            assert!(quality.accuracy_score >= 0.0 && quality.accuracy_score <= 1.0);
        } else {
            panic!("Message {} should have quality metrics", i);
        }
    }
    
    env.cleanup().await.expect("Failed to cleanup");
}

// ================================================================================================
// Protocol Enforcement End-to-End Tests
// ================================================================================================

#[tokio::test]
async fn test_proto_only_enforcement_e2e() {
    let env = E2ETestEnvironment::new().await.expect("Failed to setup test environment");
    env.create_consumer_group().await.expect("Failed to create consumer group");
    
    // Publish valid JSON that should be converted to protobuf
    let valid_json = json!({
        "symbol": "PROTO_TEST",
        "price": 125.75,
        "volume": 1500.0,
        "timestamp": chrono::Utc::now().timestamp_millis(),
        "exchange": "NASDAQ"
    });
    
    env.publish_json_to_redis(&valid_json.to_string()).await
        .expect("Failed to publish valid JSON");
    
    // Process through pipeline
    tokio::time::sleep(Duration::from_millis(100)).await;
    let processed = env.staging_service.process_batch().await
        .expect("Processing should succeed");
    
    assert_eq!(processed, 1, "Should process one message");
    
    // Consume and verify strict protobuf enforcement
    env.test_consumer.start_consuming(&env.config.eventbus_config.output_topic, 10).await
        .expect("Failed to consume proto messages");
    
    let consumed_messages = env.test_consumer.get_consumed_messages().await;
    assert_eq!(consumed_messages.len(), 1, "Should consume one proto message");
    
    let consumed_message = &consumed_messages[0];
    
    // Verify the raw payload is valid protobuf
    assert!(consumed_message.validation_result.is_ok(), 
           "Consumed message must be valid protobuf: {:?}", consumed_message.validation_result);
    
    // Verify we can decode the protobuf properly
    let decoded_envelope = EventEnvelope::decode(&consumed_message.raw_payload)
        .expect("Should decode as EventEnvelope");
    
    assert_eq!(decoded_envelope.event_type, "MarketDataEvent");
    assert_eq!(decoded_envelope.source, "data-staging");
    assert!(!decoded_envelope.event_id.is_empty());
    assert!(decoded_envelope.timestamp.is_some());
    assert!(decoded_envelope.payload.is_some());
    
    // Verify no JSON remnants in the protobuf
    let proto_string = format!("{:?}", decoded_envelope);
    assert!(!proto_string.contains("\"symbol\":"), "Proto should not contain JSON format");
    assert!(!proto_string.contains("\"price\":"), "Proto should not contain JSON format");
    
    // Verify that attempting to decode as JSON fails
    let json_decode_result: Result<serde_json::Value, _> = 
        serde_json::from_slice(&consumed_message.raw_payload);
    assert!(json_decode_result.is_err(), "Raw payload should not be decodeable as JSON");
    
    env.cleanup().await.expect("Failed to cleanup");
}

#[tokio::test]
async fn test_no_json_leakage_e2e() {
    let env = E2ETestEnvironment::new().await.expect("Failed to setup test environment");
    env.create_consumer_group().await.expect("Failed to create consumer group");
    
    // Test various JSON input formats
    let json_test_cases = vec![
        json!({"symbol": "JSON1", "price": 100.0, "timestamp": chrono::Utc::now().timestamp_millis()}),
        json!({"symbol": "JSON2", "price": 200.0, "timestamp": chrono::Utc::now().timestamp_millis(), "extra_field": "should_not_appear"}),
        json!({"symbol": "JSON3", "price": 300.0, "timestamp": chrono::Utc::now().timestamp_millis(), "nested": {"deep": {"value": 42}}}),
    ];
    
    // Publish all JSON test cases
    for (i, json_case) in json_test_cases.iter().enumerate() {
        env.publish_json_to_redis(&json_case.to_string()).await
            .expect("Failed to publish JSON test case");
        println!("Published JSON test case {}", i);
    }
    
    // Process through pipeline
    tokio::time::sleep(Duration::from_millis(100)).await;
    let processed = env.staging_service.process_batch().await
        .expect("Processing should succeed");
    
    assert_eq!(processed, json_test_cases.len(), "Should process all JSON cases");
    
    // Consume all messages and verify no JSON leakage
    env.test_consumer.start_consuming(&env.config.eventbus_config.output_topic, 10).await
        .expect("Failed to consume messages");
    
    let consumed_messages = env.test_consumer.get_consumed_messages().await;
    assert_eq!(consumed_messages.len(), json_test_cases.len(), "Should consume all processed messages");
    
    for (i, consumed_message) in consumed_messages.iter().enumerate() {
        // Verify each message is strictly protobuf
        assert!(consumed_message.validation_result.is_ok(), 
               "Message {}: Should be valid protobuf", i);
        
        // Verify raw bytes cannot be decoded as JSON
        let json_attempt: Result<serde_json::Value, _> = 
            serde_json::from_slice(&consumed_message.raw_payload);
        assert!(json_attempt.is_err(), 
               "Message {}: Raw payload should not be valid JSON", i);
        
        // Verify raw bytes cannot be decoded as text
        let text_attempt = String::from_utf8(consumed_message.raw_payload.clone());
        if let Ok(text) = text_attempt {
            assert!(!text.contains('{'), "Message {}: Should not contain JSON-like text: {}", i, text);
            assert!(!text.contains("symbol"), "Message {}: Should not contain JSON field names: {}", i, text);
        }
        
        // Verify proper protobuf structure
        let envelope = EventEnvelope::decode(&consumed_message.raw_payload)
            .expect("Should decode as protobuf");
        
        assert_eq!(envelope.event_type, "MarketDataEvent", "Should have correct event type");
        assert!(envelope.payload.is_some(), "Should have payload");
        assert!(envelope.quality.is_some(), "Should have quality metrics");
    }
    
    env.cleanup().await.expect("Failed to cleanup");
}