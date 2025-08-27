//! Common test utilities and fixtures for Data-Staging tests
//! 
//! This module provides shared functionality used across all test modules.

use data_staging::*;
use data_staging::generated::*;
use serde_json::json;
use std::collections::HashMap;
use prost::Message;

/// Test data generator for consistent test fixtures
pub struct TestDataGenerator;

impl TestDataGenerator {
    /// Generate a variety of valid market data for testing
    pub fn generate_valid_market_data_batch(count: usize) -> Vec<RawMarketData> {
        let symbols = vec!["AAPL", "GOOGL", "MSFT", "TSLA", "AMZN", "NVDA", "META", "NFLX", "CRM", "ORCL"];
        let exchanges = vec!["NASDAQ", "NYSE", "ARCA"];
        
        (0..count).map(|i| {
            let symbol = symbols[i % symbols.len()];
            let exchange = exchanges[i % exchanges.len()];
            let base_price = 100.0 + (i % 500) as f64;
            
            RawMarketData {
                symbol: Some(symbol.to_string()),
                price: Some(base_price + (i as f64 * 0.01)),
                volume: Some(1000.0 + (i % 1000) as f64),
                timestamp: Some(chrono::Utc::now().timestamp_millis() - (i as i64 * 1000)),
                bid: Some(base_price - 0.05),
                ask: Some(base_price + 0.05),
                exchange: Some(exchange.to_string()),
                sequence: Some((i as u64) + 1),
                high: if i % 3 == 0 { Some(base_price + 1.0) } else { None },
                low: if i % 3 == 1 { Some(base_price - 1.0) } else { None },
                open: if i % 2 == 0 { Some(base_price - 0.5) } else { None },
                close: if i % 4 == 0 { Some(base_price + 0.25) } else { None },
                vwap: if i % 5 == 0 { Some(base_price + 0.1) } else { None },
                metadata: if i % 10 == 0 { 
                    let mut meta = HashMap::new();
                    meta.insert("test_flag".to_string(), json!(true));
                    meta.insert("batch_id".to_string(), json!(i / 10));
                    meta
                } else { 
                    HashMap::new() 
                },
            }
        }).collect()
    }
    
    /// Generate invalid market data for testing rejection scenarios
    pub fn generate_invalid_market_data_batch() -> Vec<RawMarketData> {
        vec![
            // Missing symbol
            RawMarketData {
                symbol: None,
                price: Some(150.25),
                volume: Some(1000.0),
                timestamp: Some(chrono::Utc::now().timestamp_millis()),
                ..Default::default()
            },
            // Missing price
            RawMarketData {
                symbol: Some("INVALID1".to_string()),
                price: None,
                volume: Some(1000.0),
                timestamp: Some(chrono::Utc::now().timestamp_millis()),
                ..Default::default()
            },
            // Missing timestamp
            RawMarketData {
                symbol: Some("INVALID2".to_string()),
                price: Some(150.25),
                volume: Some(1000.0),
                timestamp: None,
                ..Default::default()
            },
            // Empty symbol
            RawMarketData {
                symbol: Some("".to_string()),
                price: Some(150.25),
                volume: Some(1000.0),
                timestamp: Some(chrono::Utc::now().timestamp_millis()),
                ..Default::default()
            },
            // Negative price
            RawMarketData {
                symbol: Some("INVALID3".to_string()),
                price: Some(-150.25),
                volume: Some(1000.0),
                timestamp: Some(chrono::Utc::now().timestamp_millis()),
                ..Default::default()
            },
            // Zero price
            RawMarketData {
                symbol: Some("INVALID4".to_string()),
                price: Some(0.0),
                volume: Some(1000.0),
                timestamp: Some(chrono::Utc::now().timestamp_millis()),
                ..Default::default()
            },
            // Future timestamp
            RawMarketData {
                symbol: Some("INVALID5".to_string()),
                price: Some(150.25),
                volume: Some(1000.0),
                timestamp: Some(chrono::Utc::now().timestamp_millis() + 3600000), // 1 hour future
                ..Default::default()
            },
            // Very old timestamp
            RawMarketData {
                symbol: Some("INVALID6".to_string()),
                price: Some(150.25),
                volume: Some(1000.0),
                timestamp: Some(chrono::Utc::now().timestamp_millis() - 86400000), // 24 hours ago
                ..Default::default()
            },
        ]
    }
    
    /// Generate JSON strings for testing
    pub fn generate_valid_json_batch(count: usize) -> Vec<String> {
        (0..count).map(|i| {
            json!({
                "symbol": format!("TEST{}", i),
                "price": 100.0 + i as f64,
                "volume": 1000.0 + (i * 10) as f64,
                "timestamp": chrono::Utc::now().timestamp_millis(),
                "exchange": if i % 2 == 0 { "NASDAQ" } else { "NYSE" },
                "sequence": i as u64 + 1
            }).to_string()
        }).collect()
    }
    
    /// Generate malformed JSON for testing
    pub fn generate_malformed_json_batch() -> Vec<String> {
        vec![
            r#"{"symbol": "TEST", "price": 100.0"#.to_string(),        // Missing closing brace
            r#"{"symbol" "TEST", "price": 100.0}"#.to_string(),        // Missing colon
            r#"{"symbol": TEST, "price": 100.0}"#.to_string(),         // Unquoted value
            r#"{symbol: "TEST", "price": 100.0}"#.to_string(),         // Unquoted key
            r#"{"symbol": "TEST", "price": 100.0,}"#.to_string(),      // Trailing comma
            "".to_string(),                                            // Empty string
            "not json at all".to_string(),                             // Plain text
            r#"null"#.to_string(),                                     // JSON null
            r#"[]"#.to_string(),                                       // JSON array
            r#"123"#.to_string(),                                      // JSON number
        ]
    }
    
    /// Generate various non-protobuf binary data for rejection testing
    pub fn generate_non_protobuf_binary_data() -> Vec<Vec<u8>> {
        vec![
            // Raw binary patterns
            vec![0x01, 0x02, 0x03, 0x04, 0x05],
            vec![0xFF; 100],
            vec![0x00; 50],
            vec![0xAA; 200],
            (0..256).map(|i| i as u8).collect(),
            
            // Text formats as bytes
            b"This is plain text".to_vec(),
            b"symbol,price,volume\nAAPL,150.25,1000".to_vec(),        // CSV
            b"<root><symbol>AAPL</symbol></root>".to_vec(),            // XML
            b"---\nsymbol: AAPL\nprice: 150.25\n---".to_vec(),        // YAML
            
            // JSON as bytes (should be rejected by protobuf validation)
            r#"{"symbol": "AAPL", "price": 150.25}"#.as_bytes().to_vec(),
            r#"[]"#.as_bytes().to_vec(),
            r#"null"#.as_bytes().to_vec(),
            
            // File format signatures
            vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A], // PNG signature
            vec![0xFF, 0xD8, 0xFF, 0xE0],                          // JPEG signature
            vec![0x50, 0x4B, 0x03, 0x04],                          // ZIP signature
            vec![0x1F, 0x8B, 0x08],                                // GZIP signature
            
            // Large binary data
            vec![0x42; 10000],  // 10KB of 'B'
            vec![0x13; 50000],  // 50KB of 0x13
            
            // Random-like patterns
            (0..1000).map(|i| ((i * 17 + 23) % 256) as u8).collect(), // Pseudo-random
            
            // Empty and minimal data
            vec![],        // Empty
            vec![0x00],    // Single null
            vec![0xFF],    // Single max
        ]
    }
    
    /// Create valid EventEnvelope protobuf for testing
    pub fn create_valid_event_envelope(id_suffix: Option<&str>) -> EventEnvelope {
        let suffix = id_suffix.unwrap_or("test");
        
        EventEnvelope {
            event_id: format!("test-event-{}-{}", suffix, uuid::Uuid::new_v4()),
            timestamp: Some(prost_types::Timestamp {
                seconds: chrono::Utc::now().timestamp(),
                nanos: 0,
            }),
            event_type: "MarketDataEvent".to_string(),
            source: "test-data-staging".to_string(),
            payload: Some(format!("test payload for {}", suffix).into_bytes()),
            quality: Some(neural_trader::market_data::v1::DataQuality {
                completeness_score: 0.95,
                timeliness_score: 0.98,
                accuracy_score: 0.92,
                overall_score: 0.95,
                issues: vec![],
            }),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("test_type".to_string(), "unit_test".to_string());
                meta.insert("generated_by".to_string(), "TestDataGenerator".to_string());
                if let Some(s) = id_suffix {
                    meta.insert("suffix".to_string(), s.to_string());
                }
                meta
            },
            correlation_id: Some(format!("corr-{}", uuid::Uuid::new_v4())),
            trace_id: Some(format!("trace-{}", uuid::Uuid::new_v4())),
        }
    }
}

impl Default for RawMarketData {
    fn default() -> Self {
        Self {
            symbol: None,
            price: None,
            volume: None,
            timestamp: None,
            bid: None,
            ask: None,
            exchange: None,
            sequence: None,
            high: None,
            low: None,
            open: None,
            close: None,
            vwap: None,
            metadata: HashMap::new(),
        }
    }
}

/// Assertion helpers for testing
pub struct TestAssertions;

impl TestAssertions {
    /// Assert that data is valid protobuf EventEnvelope
    pub fn assert_valid_proto_envelope(data: &[u8]) {
        let result = EventEnvelope::decode(data);
        assert!(result.is_ok(), "Data should decode as valid EventEnvelope protobuf: {:?}", 
                String::from_utf8_lossy(data));
        
        let envelope = result.unwrap();
        assert!(!envelope.event_id.is_empty(), "EventEnvelope should have non-empty event_id");
        assert!(!envelope.event_type.is_empty(), "EventEnvelope should have non-empty event_type");
        assert!(!envelope.source.is_empty(), "EventEnvelope should have non-empty source");
    }
    
    /// Assert that data is NOT valid protobuf
    pub fn assert_invalid_proto(data: &[u8]) {
        let result = EventEnvelope::decode(data);
        assert!(result.is_err(), "Data should NOT decode as valid protobuf: {:?}", 
                String::from_utf8_lossy(data));
    }
    
    /// Assert quality metrics meet requirements  
    pub fn assert_quality_meets_threshold(quality: &neural_trader::market_data::v1::DataQuality, threshold: f32) {
        assert!(quality.overall_score >= threshold, 
               "Quality score {} should meet threshold {}", quality.overall_score, threshold);
        assert!(quality.completeness_score >= 0.0 && quality.completeness_score <= 1.0,
               "Completeness score should be in range [0,1]: {}", quality.completeness_score);
        assert!(quality.timeliness_score >= 0.0 && quality.timeliness_score <= 1.0,
               "Timeliness score should be in range [0,1]: {}", quality.timeliness_score);
        assert!(quality.accuracy_score >= 0.0 && quality.accuracy_score <= 1.0,
               "Accuracy score should be in range [0,1]: {}", quality.accuracy_score);
    }
    
    /// Assert performance metrics meet requirements
    pub fn assert_performance_requirements(
        throughput_msgs_per_sec: f64, 
        avg_latency: std::time::Duration,
        memory_increase_mb: f64
    ) {
        assert!(throughput_msgs_per_sec >= 10_000.0, 
               "Throughput {} msgs/sec should be ≥10,000", throughput_msgs_per_sec);
        assert!(avg_latency.as_millis() <= 1, 
               "Average latency {:?} should be ≤1ms", avg_latency);
        assert!(memory_increase_mb <= 50.0,
               "Memory increase {:.1}MB should be ≤50MB", memory_increase_mb);
    }
    
    /// Assert that error is of expected category
    pub fn assert_error_category(error: &DataStagingError, expected_category: &str) {
        assert_eq!(error.category(), expected_category,
                  "Error should be in category '{}': {:?}", expected_category, error);
    }
}

/// Mock implementations for testing
pub mod mocks {
    use super::*;
    use neural_core::eventbus::*;
    use std::sync::{Arc, Mutex};
    
    /// Mock EventBus for testing
    pub struct MockEventBus {
        published_messages: Arc<Mutex<Vec<Event>>>,
        should_fail_publish: Arc<Mutex<bool>>,
    }
    
    impl MockEventBus {
        pub fn new() -> Self {
            Self {
                published_messages: Arc::new(Mutex::new(Vec::new())),
                should_fail_publish: Arc::new(Mutex::new(false)),
            }
        }
        
        pub fn set_should_fail(&self, should_fail: bool) {
            *self.should_fail_publish.lock().unwrap() = should_fail;
        }
        
        pub fn get_published_messages(&self) -> Vec<Event> {
            self.published_messages.lock().unwrap().clone()
        }
        
        pub fn clear_messages(&self) {
            self.published_messages.lock().unwrap().clear();
        }
    }
    
    #[async_trait::async_trait]
    impl EventBus for MockEventBus {
        async fn publish(&self, topic: &str, event: Event) -> Result<(), EventBusError> {
            if *self.should_fail_publish.lock().unwrap() {
                return Err(EventBusError::PublishError(format!("Mock failure for topic: {}", topic)));
            }
            
            self.published_messages.lock().unwrap().push(event);
            Ok(())
        }
        
        async fn consume(&self, topic: &str, start_position: StartPosition, max_messages: usize) -> Result<Vec<EventEnvelope>, EventBusError> {
            // Return mock EventEnvelopes based on published messages
            let messages = self.published_messages.lock().unwrap();
            let envelopes: Vec<EventEnvelope> = messages.iter()
                .take(max_messages)
                .enumerate()
                .map(|(i, event)| EventEnvelope {
                    event_id: format!("mock-{}-{}", topic, i),
                    timestamp: Some(prost_types::Timestamp {
                        seconds: chrono::Utc::now().timestamp(),
                        nanos: 0,
                    }),
                    event_type: "MockEvent".to_string(),
                    source: "mock_eventbus".to_string(),
                    payload: Some(event.payload.clone()),
                    quality: None,
                    metadata: HashMap::new(),
                    correlation_id: Some(event.id.clone()),
                    trace_id: Some(format!("mock-trace-{}", i)),
                })
                .collect();
            
            Ok(envelopes)
        }
        
        async fn subscribe(&self, topic: &str, config: SubscriptionConfig) -> Result<Box<dyn EventSubscriber>, EventBusError> {
            // Return a mock subscriber
            Ok(Box::new(MockSubscriber::new(topic.to_string())))
        }
        
        async fn create_topic(&self, topic: &str, config: Option<serde_json::Value>) -> Result<(), EventBusError> {
            // Mock topic creation - always succeeds
            Ok(())
        }
        
        async fn delete_topic(&self, topic: &str) -> Result<(), EventBusError> {
            // Mock topic deletion - always succeeds
            Ok(())
        }
        
        async fn list_topics(&self) -> Result<Vec<String>, EventBusError> {
            Ok(vec!["mock_topic_1".to_string(), "mock_topic_2".to_string()])
        }
        
        async fn get_topic_info(&self, topic: &str) -> Result<ChannelInfo, EventBusError> {
            Ok(ChannelInfo {
                name: topic.to_string(),
                message_count: self.published_messages.lock().unwrap().len() as u64,
                consumer_count: 1,
                last_message_time: Some(chrono::Utc::now()),
                config: serde_json::Value::Null,
            })
        }
    }
    
    /// Mock EventSubscriber for testing
    struct MockSubscriber {
        topic: String,
    }
    
    impl MockSubscriber {
        fn new(topic: String) -> Self {
            Self { topic }
        }
    }
    
    #[async_trait::async_trait]
    impl EventSubscriber for MockSubscriber {
        async fn next(&mut self) -> Result<Option<EventEnvelope>, EventBusError> {
            // Return mock envelope
            Ok(Some(EventEnvelope {
                event_id: format!("mock-sub-{}", uuid::Uuid::new_v4()),
                timestamp: Some(prost_types::Timestamp {
                    seconds: chrono::Utc::now().timestamp(),
                    nanos: 0,
                }),
                event_type: "MockSubscriberEvent".to_string(),
                source: "mock_subscriber".to_string(),
                payload: Some(b"mock subscriber payload".to_vec()),
                quality: None,
                metadata: HashMap::new(),
                correlation_id: Some(format!("mock-corr-{}", uuid::Uuid::new_v4())),
                trace_id: Some(format!("mock-trace-{}", uuid::Uuid::new_v4())),
            }))
        }
    }
}

/// Test timing utilities
pub struct TestTimer {
    start_time: std::time::Instant,
}

impl TestTimer {
    pub fn start() -> Self {
        Self {
            start_time: std::time::Instant::now(),
        }
    }
    
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }
    
    pub fn elapsed_ms(&self) -> u128 {
        self.elapsed().as_millis()
    }
    
    pub fn elapsed_micros(&self) -> u128 {
        self.elapsed().as_micros()
    }
    
    pub fn assert_elapsed_under(&self, max_duration: std::time::Duration, operation: &str) {
        let elapsed = self.elapsed();
        assert!(elapsed <= max_duration, 
               "{} took {:?}, should be ≤{:?}", operation, elapsed, max_duration);
    }
}

/// Memory measurement utilities  
pub struct MemoryMeasurement {
    initial_memory: usize,
}

impl MemoryMeasurement {
    pub fn start() -> Self {
        Self {
            initial_memory: Self::get_memory_usage(),
        }
    }
    
    fn get_memory_usage() -> usize {
        // In a real implementation, this would measure actual memory usage
        // For testing, we'll simulate with a placeholder value
        std::mem::size_of::<DataStagingService>() * 1000
    }
    
    pub fn memory_increase(&self) -> isize {
        Self::get_memory_usage() as isize - self.initial_memory as isize
    }
    
    pub fn memory_increase_mb(&self) -> f64 {
        self.memory_increase() as f64 / (1024.0 * 1024.0)
    }
    
    pub fn assert_memory_increase_under(&self, max_mb: f64, operation: &str) {
        let increase_mb = self.memory_increase_mb();
        assert!(increase_mb <= max_mb,
               "{} increased memory by {:.1}MB, should be ≤{:.1}MB", 
               operation, increase_mb, max_mb);
    }
}

#[cfg(test)]
mod common_tests {
    use super::*;
    
    #[test]
    fn test_data_generator_valid_batch() {
        let batch = TestDataGenerator::generate_valid_market_data_batch(10);
        assert_eq!(batch.len(), 10);
        
        for (i, data) in batch.iter().enumerate() {
            assert!(data.symbol.is_some(), "Item {} should have symbol", i);
            assert!(data.price.is_some(), "Item {} should have price", i);
            assert!(data.timestamp.is_some(), "Item {} should have timestamp", i);
            assert!(data.price.unwrap() > 0.0, "Item {} should have positive price", i);
        }
    }
    
    #[test]
    fn test_data_generator_invalid_batch() {
        let batch = TestDataGenerator::generate_invalid_market_data_batch();
        assert!(!batch.is_empty(), "Should generate invalid data");
        
        // Each item should have some validation issue
        for (i, data) in batch.iter().enumerate() {
            let has_issue = data.symbol.is_none() || 
                           data.price.is_none() || 
                           data.timestamp.is_none() ||
                           data.symbol.as_ref().map_or(false, |s| s.is_empty()) ||
                           data.price.map_or(false, |p| p <= 0.0) ||
                           data.timestamp.map_or(false, |t| {
                               let now = chrono::Utc::now().timestamp_millis();
                               t > now + 3600000 || t < now - 86400000
                           });
            
            assert!(has_issue, "Invalid data item {} should have validation issue", i);
        }
    }
    
    #[test]
    fn test_non_protobuf_binary_generation() {
        let binary_data = TestDataGenerator::generate_non_protobuf_binary_data();
        assert!(!binary_data.is_empty(), "Should generate non-protobuf data");
        
        // Verify none of the data decodes as valid EventEnvelope protobuf
        for (i, data) in binary_data.iter().enumerate() {
            let result = EventEnvelope::decode(&data[..]);
            assert!(result.is_err(), "Binary data item {} should not decode as protobuf", i);
        }
    }
    
    #[test] 
    fn test_valid_event_envelope_creation() {
        let envelope = TestDataGenerator::create_valid_event_envelope(Some("test"));
        
        assert!(!envelope.event_id.is_empty());
        assert!(envelope.event_id.contains("test"));
        assert_eq!(envelope.event_type, "MarketDataEvent");
        assert_eq!(envelope.source, "test-data-staging");
        assert!(envelope.payload.is_some());
        assert!(envelope.quality.is_some());
        assert!(envelope.correlation_id.is_some());
        assert!(envelope.trace_id.is_some());
        
        // Verify it encodes/decodes properly
        let encoded = envelope.encode_to_vec();
        let decoded = EventEnvelope::decode(&encoded[..]);
        assert!(decoded.is_ok(), "Generated envelope should be valid protobuf");
    }
    
    #[test]
    fn test_assertions_helper() {
        let valid_envelope = TestDataGenerator::create_valid_event_envelope(None);
        let encoded = valid_envelope.encode_to_vec();
        
        // Should pass validation
        TestAssertions::assert_valid_proto_envelope(&encoded);
        
        // Should fail for non-proto data
        let json_bytes = r#"{"not": "protobuf"}"#.as_bytes();
        TestAssertions::assert_invalid_proto(json_bytes);
        
        // Test quality assertions
        if let Some(quality) = &valid_envelope.quality {
            TestAssertions::assert_quality_meets_threshold(quality, 0.8);
        }
    }
    
    #[test]
    fn test_timer_utility() {
        let timer = TestTimer::start();
        
        // Simulate some work
        std::thread::sleep(std::time::Duration::from_millis(10));
        
        let elapsed = timer.elapsed();
        assert!(elapsed >= std::time::Duration::from_millis(10));
        assert!(elapsed < std::time::Duration::from_millis(100));
        
        println!("Timer test elapsed: {:?}", elapsed);
    }
}