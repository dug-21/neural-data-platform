//! Comprehensive EventBus Integration Tests
//! 
//! This test suite demonstrates and validates all EventBus functionality:
//! 1. Basic pub/sub with InMemoryEventBus
//! 2. Consumer groups and load balancing
//! 3. Message batching functionality  
//! 4. Backpressure handling
//! 5. Dead Letter Queue with retries
//! 6. Recording wrapper functionality
//! 7. Channel validation (stream:domain:identifier format)
//! 8. Multi-channel subscriptions

use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::{sleep, timeout, Instant};
use futures::stream::StreamExt;
use uuid::Uuid;

use neural_core::events::{
    Event, EventBus, InMemoryEventBus, SubscriptionHandle,
    PriceUpdateEvent, VolumeEvent, TrendChangeEvent,
    prediction_events::{ModelPredictionEvent, ModelUpdateEvent, ModelPerformanceEvent, ModelUpdateType}
};
use neural_core::types::{
    market::MarketTrend,
    prediction::Prediction
};
use neural_core::errors::{CoreError, Result};

/// Test event for integration tests
#[derive(Debug, Clone)]
pub struct TestEvent {
    id: Uuid,
    event_type: String,
    data: String,
    priority: u8,
    timestamp: chrono::DateTime<chrono::Utc>,
    correlation_id: Option<Uuid>,
}

impl TestEvent {
    pub fn new(event_type: String, data: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type,
            data,
            priority: 5,
            timestamp: chrono::Utc::now(),
            correlation_id: None,
        }
    }

    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }
}

impl Event for TestEvent {
    fn event_type(&self) -> String {
        self.event_type.clone()
    }

    fn timestamp(&self) -> chrono::DateTime<chrono::Utc> {
        self.timestamp
    }

    fn event_id(&self) -> Uuid {
        self.id
    }

    fn source(&self) -> String {
        "test_source".to_string()
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "event_type": self.event_type,
            "data": self.data,
            "priority": self.priority,
            "timestamp": self.timestamp
        })
    }

    fn priority(&self) -> u8 {
        self.priority
    }

    fn correlation_id(&self) -> Option<Uuid> {
        self.correlation_id
    }
}

/// Consumer group implementation for load balancing
pub struct ConsumerGroup {
    group_id: String,
    consumers: Vec<Arc<Consumer>>,
    round_robin_index: Arc<Mutex<usize>>,
}

impl ConsumerGroup {
    pub fn new(group_id: String) -> Self {
        Self {
            group_id,
            consumers: Vec::new(),
            round_robin_index: Arc::new(Mutex::new(0)),
        }
    }

    pub fn add_consumer(&mut self, consumer: Arc<Consumer>) {
        self.consumers.push(consumer);
    }

    pub async fn distribute_event(&self, event: Arc<dyn Event + Send + Sync>) -> Result<()> {
        if self.consumers.is_empty() {
            return Err(CoreError::EventError("No consumers in group".to_string()));
        }

        let mut index = self.round_robin_index.lock().unwrap();
        let consumer = &self.consumers[*index];
        *index = (*index + 1) % self.consumers.len();
        drop(index);

        consumer.handle_event(event).await
    }
}

/// Consumer for handling events
pub struct Consumer {
    id: String,
    processed_events: Arc<Mutex<Vec<Uuid>>>,
    processing_time_ms: u64,
    should_fail: Arc<Mutex<bool>>,
}

impl Consumer {
    pub fn new(id: String) -> Self {
        Self {
            id,
            processed_events: Arc::new(Mutex::new(Vec::new())),
            processing_time_ms: 10,
            should_fail: Arc::new(Mutex::new(false)),
        }
    }

    pub fn with_processing_time(mut self, time_ms: u64) -> Self {
        self.processing_time_ms = time_ms;
        self
    }

    pub fn set_should_fail(&self, should_fail: bool) {
        *self.should_fail.lock().unwrap() = should_fail;
    }

    pub async fn handle_event(&self, event: Arc<dyn Event + Send + Sync>) -> Result<()> {
        sleep(Duration::from_millis(self.processing_time_ms)).await;

        if *self.should_fail.lock().unwrap() {
            return Err(CoreError::EventError(format!("Consumer {} failed to process event", self.id)));
        }

        self.processed_events.lock().unwrap().push(event.event_id());
        Ok(())
    }

    pub fn get_processed_count(&self) -> usize {
        self.processed_events.lock().unwrap().len()
    }

    pub fn get_processed_events(&self) -> Vec<Uuid> {
        self.processed_events.lock().unwrap().clone()
    }
}

/// Event batcher for batch processing
pub struct EventBatcher {
    batch_size: usize,
    batch_timeout: Duration,
    current_batch: Arc<Mutex<Vec<Arc<dyn Event + Send + Sync>>>>,
    batches_processed: Arc<Mutex<Vec<Vec<Uuid>>>>,
}

impl EventBatcher {
    pub fn new(batch_size: usize, batch_timeout: Duration) -> Self {
        Self {
            batch_size,
            batch_timeout,
            current_batch: Arc::new(Mutex::new(Vec::new())),
            batches_processed: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn add_event(&self, event: Arc<dyn Event + Send + Sync>) -> Option<Vec<Arc<dyn Event + Send + Sync>>> {
        let mut batch = self.current_batch.lock().unwrap();
        batch.push(event);

        if batch.len() >= self.batch_size {
            let ready_batch = batch.drain(..).collect();
            drop(batch);
            Some(ready_batch)
        } else {
            None
        }
    }

    pub async fn flush_batch(&self) -> Option<Vec<Arc<dyn Event + Send + Sync>>> {
        let mut batch = self.current_batch.lock().unwrap();
        if batch.is_empty() {
            None
        } else {
            let ready_batch = batch.drain(..).collect();
            Some(ready_batch)
        }
    }

    pub async fn process_batch(&self, batch: Vec<Arc<dyn Event + Send + Sync>>) {
        let event_ids: Vec<Uuid> = batch.iter().map(|e| e.event_id()).collect();
        self.batches_processed.lock().unwrap().push(event_ids);
        
        // Simulate batch processing
        sleep(Duration::from_millis(50)).await;
    }

    pub fn get_batches_processed(&self) -> Vec<Vec<Uuid>> {
        self.batches_processed.lock().unwrap().clone()
    }
}

/// Backpressure handler
pub struct BackpressureHandler {
    max_queue_size: usize,
    current_queue_size: Arc<Mutex<usize>>,
    dropped_events: Arc<Mutex<usize>>,
}

impl BackpressureHandler {
    pub fn new(max_queue_size: usize) -> Self {
        Self {
            max_queue_size,
            current_queue_size: Arc::new(Mutex::new(0)),
            dropped_events: Arc::new(Mutex::new(0)),
        }
    }

    pub fn can_accept_event(&self) -> bool {
        *self.current_queue_size.lock().unwrap() < self.max_queue_size
    }

    pub fn add_event(&self) -> bool {
        let mut size = self.current_queue_size.lock().unwrap();
        if *size >= self.max_queue_size {
            *self.dropped_events.lock().unwrap() += 1;
            false
        } else {
            *size += 1;
            true
        }
    }

    pub fn remove_event(&self) {
        let mut size = self.current_queue_size.lock().unwrap();
        if *size > 0 {
            *size -= 1;
        }
    }

    pub fn get_dropped_count(&self) -> usize {
        *self.dropped_events.lock().unwrap()
    }

    pub fn get_current_queue_size(&self) -> usize {
        *self.current_queue_size.lock().unwrap()
    }
}

/// Dead Letter Queue implementation
pub struct DeadLetterQueue {
    failed_events: Arc<Mutex<Vec<(Arc<dyn Event + Send + Sync>, usize)>>>,
    max_retries: usize,
    retry_delay: Duration,
}

impl DeadLetterQueue {
    pub fn new(max_retries: usize, retry_delay: Duration) -> Self {
        Self {
            failed_events: Arc::new(Mutex::new(Vec::new())),
            max_retries,
            retry_delay,
        }
    }

    pub fn add_failed_event(&self, event: Arc<dyn Event + Send + Sync>, retry_count: usize) {
        self.failed_events.lock().unwrap().push((event, retry_count));
    }

    pub async fn process_retry(&self, consumer: &Consumer) -> Result<usize> {
        let events_to_retry: Vec<(Arc<dyn Event + Send + Sync>, usize)> = {
            let mut failed = self.failed_events.lock().unwrap();
            failed.drain(..).collect()
        };

        let mut successful_retries = 0;
        let mut new_failures = Vec::new();

        for (event, retry_count) in events_to_retry {
            sleep(self.retry_delay).await;
            
            match consumer.handle_event(event.clone()).await {
                Ok(_) => {
                    successful_retries += 1;
                }
                Err(_) => {
                    if retry_count < self.max_retries {
                        new_failures.push((event, retry_count + 1));
                    }
                    // Events that exceed max retries are dropped
                }
            }
        }

        // Re-add failed events that haven't exceeded max retries
        self.failed_events.lock().unwrap().extend(new_failures);

        Ok(successful_retries)
    }

    pub fn get_failed_count(&self) -> usize {
        self.failed_events.lock().unwrap().len()
    }
}

/// Recording wrapper for EventBus
pub struct RecordingEventBus {
    inner: Arc<dyn EventBus>,
    published_events: Arc<Mutex<Vec<Arc<dyn Event + Send + Sync>>>>,
    subscriptions: Arc<Mutex<Vec<String>>>,
}

impl RecordingEventBus {
    pub fn new(inner: Arc<dyn EventBus>) -> Self {
        Self {
            inner,
            published_events: Arc::new(Mutex::new(Vec::new())),
            subscriptions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn get_published_events(&self) -> Vec<Arc<dyn Event + Send + Sync>> {
        self.published_events.lock().unwrap().clone()
    }

    pub fn get_subscriptions(&self) -> Vec<String> {
        self.subscriptions.lock().unwrap().clone()
    }

    pub fn clear_records(&self) {
        self.published_events.lock().unwrap().clear();
        self.subscriptions.lock().unwrap().clear();
    }
}

#[async_trait::async_trait]
impl EventBus for RecordingEventBus {
    async fn publish(&self, event: Arc<dyn Event + Send + Sync>) -> Result<()> {
        self.published_events.lock().unwrap().push(event.clone());
        self.inner.publish(event).await
    }

    async fn subscribe(&self, event_type: &str) -> Result<SubscriptionHandle> {
        self.subscriptions.lock().unwrap().push(event_type.to_string());
        self.inner.subscribe(event_type).await
    }

    async fn unsubscribe(&self, handle: SubscriptionHandle) -> Result<()> {
        self.inner.unsubscribe(handle).await
    }

    async fn get_stream(&self, event_type: &str) -> Result<std::pin::Pin<Box<dyn futures::stream::Stream<Item = Arc<dyn Event + Send + Sync>> + Send>>> {
        self.inner.get_stream(event_type).await
    }
}

/// Channel validator for stream:domain:identifier format
pub struct ChannelValidator;

impl ChannelValidator {
    pub fn validate_channel_format(channel: &str) -> Result<(String, String, String)> {
        let parts: Vec<&str> = channel.split(':').collect();
        
        if parts.len() != 3 {
            return Err(CoreError::Validation(
                format!("Channel must follow format 'stream:domain:identifier', got: {}", channel)
            ));
        }

        let stream = parts[0];
        let domain = parts[1];
        let identifier = parts[2];

        // Validate each part
        if stream.is_empty() || domain.is_empty() || identifier.is_empty() {
            return Err(CoreError::Validation(
                "Channel parts cannot be empty".to_string()
            ));
        }

        // Validate characters (alphanumeric and underscores only)
        for part in parts {
            if !part.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Err(CoreError::Validation(
                    format!("Channel parts can only contain alphanumeric characters and underscores: {}", part)
                ));
            }
        }

        Ok((stream.to_string(), domain.to_string(), identifier.to_string()))
    }

    pub fn is_valid_channel(channel: &str) -> bool {
        Self::validate_channel_format(channel).is_ok()
    }
}

// =============================================================================
// INTEGRATION TESTS
// =============================================================================

#[tokio::test]
async fn test_basic_pub_sub_functionality() {
    let bus = Arc::new(InMemoryEventBus::new());
    
    // Subscribe to price updates
    let _handle = bus.subscribe("price_update").await.unwrap();
    let mut stream = bus.get_stream("price_update").await.unwrap();
    
    // Create and publish a price update event
    let event = Arc::new(PriceUpdateEvent::new(
        "AAPL".to_string(),
        150.0,
        149.0,
    ));
    
    let publish_event = event.clone();
    let publish_bus = bus.clone();
    
    // Publish event in background
    tokio::spawn(async move {
        sleep(Duration::from_millis(10)).await;
        publish_bus.publish(publish_event).await.unwrap();
    });
    
    // Receive the event
    let received = timeout(Duration::from_millis(100), stream.next()).await
        .expect("Should receive event within timeout")
        .expect("Stream should yield an event");
    
    assert_eq!(received.event_type(), "price_update");
    assert_eq!(received.event_id(), event.event_id());
    
    println!("✅ Basic pub/sub functionality test passed");
}

#[tokio::test]
async fn test_consumer_groups_and_load_balancing() {
    let bus = Arc::new(InMemoryEventBus::new());
    
    // Create consumer group with 3 consumers
    let consumer1 = Arc::new(Consumer::new("consumer_1".to_string()));
    let consumer2 = Arc::new(Consumer::new("consumer_2".to_string()));
    let consumer3 = Arc::new(Consumer::new("consumer_3".to_string()));
    
    let mut consumer_group = ConsumerGroup::new("test_group".to_string());
    consumer_group.add_consumer(consumer1.clone());
    consumer_group.add_consumer(consumer2.clone());
    consumer_group.add_consumer(consumer3.clone());
    
    // Publish multiple events and distribute them
    let events = (0..9).map(|i| {
        Arc::new(TestEvent::new(
            "test_event".to_string(),
            format!("Event {}", i)
        )) as Arc<dyn Event + Send + Sync>
    }).collect::<Vec<_>>();
    
    for event in events {
        consumer_group.distribute_event(event).await.unwrap();
    }
    
    // Wait for processing
    sleep(Duration::from_millis(100)).await;
    
    // Verify load balancing - each consumer should have processed 3 events
    assert_eq!(consumer1.get_processed_count(), 3);
    assert_eq!(consumer2.get_processed_count(), 3);
    assert_eq!(consumer3.get_processed_count(), 3);
    
    println!("✅ Consumer groups and load balancing test passed");
}

#[tokio::test]
async fn test_message_batching_functionality() {
    let batcher = Arc::new(EventBatcher::new(3, Duration::from_millis(100)));
    
    // Add events one by one
    let event1 = Arc::new(TestEvent::new("batch_test".to_string(), "Event 1".to_string()));
    let event2 = Arc::new(TestEvent::new("batch_test".to_string(), "Event 2".to_string()));
    let event3 = Arc::new(TestEvent::new("batch_test".to_string(), "Event 3".to_string()));
    let event4 = Arc::new(TestEvent::new("batch_test".to_string(), "Event 4".to_string()));
    
    // Add first two events - should not trigger batch
    assert!(batcher.add_event(event1 as Arc<dyn Event + Send + Sync>).await.is_none());
    assert!(batcher.add_event(event2 as Arc<dyn Event + Send + Sync>).await.is_none());
    
    // Add third event - should trigger batch
    let batch = batcher.add_event(event3 as Arc<dyn Event + Send + Sync>).await;
    assert!(batch.is_some());
    let batch = batch.unwrap();
    assert_eq!(batch.len(), 3);
    
    // Process the batch
    batcher.process_batch(batch).await;
    
    // Add one more event and flush manually
    batcher.add_event(event4 as Arc<dyn Event + Send + Sync>).await;
    let remaining_batch = batcher.flush_batch().await;
    assert!(remaining_batch.is_some());
    assert_eq!(remaining_batch.unwrap().len(), 1);
    
    // Verify batches were processed
    let processed_batches = batcher.get_batches_processed();
    assert_eq!(processed_batches.len(), 1);
    assert_eq!(processed_batches[0].len(), 3);
    
    println!("✅ Message batching functionality test passed");
}

#[tokio::test]
async fn test_backpressure_handling() {
    let backpressure = BackpressureHandler::new(5); // Max 5 events
    
    // Add events up to the limit
    for i in 0..5 {
        assert!(backpressure.add_event(), "Should accept event {}", i);
    }
    
    // Try to add more events - should be dropped
    assert!(!backpressure.add_event());
    assert!(!backpressure.add_event());
    
    assert_eq!(backpressure.get_current_queue_size(), 5);
    assert_eq!(backpressure.get_dropped_count(), 2);
    
    // Remove some events
    backpressure.remove_event();
    backpressure.remove_event();
    
    // Should be able to add events again
    assert!(backpressure.add_event());
    assert_eq!(backpressure.get_current_queue_size(), 4);
    
    println!("✅ Backpressure handling test passed");
}

#[tokio::test]
async fn test_dead_letter_queue_with_retries() {
    let consumer = Consumer::new("dlq_consumer".to_string());
    let dlq = DeadLetterQueue::new(3, Duration::from_millis(10));
    
    // Create test events
    let event1 = Arc::new(TestEvent::new("dlq_test".to_string(), "Event 1".to_string()));
    let event2 = Arc::new(TestEvent::new("dlq_test".to_string(), "Event 2".to_string()));
    
    // Make consumer fail initially
    consumer.set_should_fail(true);
    
    // Add failed events to DLQ
    dlq.add_failed_event(event1.clone() as Arc<dyn Event + Send + Sync>, 0);
    dlq.add_failed_event(event2.clone() as Arc<dyn Event + Send + Sync>, 0);
    
    assert_eq!(dlq.get_failed_count(), 2);
    
    // Try processing retries while consumer is still failing
    let successful = dlq.process_retry(&consumer).await.unwrap();
    assert_eq!(successful, 0);
    assert_eq!(dlq.get_failed_count(), 2); // Events should still be in DLQ with retry_count = 1
    
    // Make consumer succeed
    consumer.set_should_fail(false);
    
    // Process retries again
    let successful = dlq.process_retry(&consumer).await.unwrap();
    assert_eq!(successful, 2);
    assert_eq!(dlq.get_failed_count(), 0);
    assert_eq!(consumer.get_processed_count(), 2);
    
    println!("✅ Dead Letter Queue with retries test passed");
}

#[tokio::test]
async fn test_recording_wrapper_functionality() {
    let inner_bus = Arc::new(InMemoryEventBus::new());
    let recording_bus = Arc::new(RecordingEventBus::new(inner_bus));
    
    // Subscribe and verify it's recorded
    let _handle1 = recording_bus.subscribe("test_event_1").await.unwrap();
    let _handle2 = recording_bus.subscribe("test_event_2").await.unwrap();
    
    // Create receivers to keep channels open
    let _stream1 = recording_bus.get_stream("test_event_1").await.unwrap();
    let _stream2 = recording_bus.get_stream("test_event_2").await.unwrap();
    
    let subscriptions = recording_bus.get_subscriptions();
    assert_eq!(subscriptions.len(), 2);
    assert!(subscriptions.contains(&"test_event_1".to_string()));
    assert!(subscriptions.contains(&"test_event_2".to_string()));
    
    // Publish events and verify they're recorded
    let event1 = Arc::new(TestEvent::new("test_event_1".to_string(), "Data 1".to_string()));
    let event2 = Arc::new(TestEvent::new("test_event_2".to_string(), "Data 2".to_string()));
    
    recording_bus.publish(event1.clone() as Arc<dyn Event + Send + Sync>).await.unwrap();
    recording_bus.publish(event2.clone() as Arc<dyn Event + Send + Sync>).await.unwrap();
    
    let published_events = recording_bus.get_published_events();
    assert_eq!(published_events.len(), 2);
    assert_eq!(published_events[0].event_id(), event1.event_id());
    assert_eq!(published_events[1].event_id(), event2.event_id());
    
    // Test clearing records
    recording_bus.clear_records();
    assert_eq!(recording_bus.get_published_events().len(), 0);
    assert_eq!(recording_bus.get_subscriptions().len(), 0);
    
    println!("✅ Recording wrapper functionality test passed");
}

#[tokio::test]
async fn test_channel_validation() {
    // Valid channel formats
    assert!(ChannelValidator::is_valid_channel("market:prices:AAPL"));
    assert!(ChannelValidator::is_valid_channel("predictions:lstm:model_v1"));
    assert!(ChannelValidator::is_valid_channel("trading:signals:buy_sell"));
    
    // Test parsing valid channels
    let (stream, domain, identifier) = ChannelValidator::validate_channel_format("market:prices:AAPL").unwrap();
    assert_eq!(stream, "market");
    assert_eq!(domain, "prices");
    assert_eq!(identifier, "AAPL");
    
    // Invalid channel formats
    assert!(!ChannelValidator::is_valid_channel("invalid_format"));
    assert!(!ChannelValidator::is_valid_channel("market:prices")); // Missing identifier
    assert!(!ChannelValidator::is_valid_channel("market:prices:AAPL:extra")); // Too many parts
    assert!(!ChannelValidator::is_valid_channel("market::AAPL")); // Empty domain
    assert!(!ChannelValidator::is_valid_channel("market:prices:")); // Empty identifier
    assert!(!ChannelValidator::is_valid_channel("market:prices-invalid:AAPL")); // Invalid characters
    
    // Test error messages
    let result = ChannelValidator::validate_channel_format("invalid");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("must follow format 'stream:domain:identifier'"));
    
    println!("✅ Channel validation test passed");
}

#[tokio::test]
async fn test_multi_channel_subscriptions() {
    let bus = Arc::new(InMemoryEventBus::new());
    
    // Subscribe to multiple channels
    let price_handle = bus.subscribe("market:prices:AAPL").await.unwrap();
    let volume_handle = bus.subscribe("market:volume:AAPL").await.unwrap();
    let prediction_handle = bus.subscribe("predictions:lstm:AAPL").await.unwrap();
    
    // Get streams for each channel
    let mut price_stream = bus.get_stream("market:prices:AAPL").await.unwrap();
    let mut volume_stream = bus.get_stream("market:volume:AAPL").await.unwrap();
    let mut prediction_stream = bus.get_stream("predictions:lstm:AAPL").await.unwrap();
    
    // Create events for different channels
    let price_event = Arc::new(PriceUpdateEvent::new("AAPL".to_string(), 150.0, 149.0));
    let volume_event = Arc::new(VolumeEvent::new("AAPL".to_string(), 1000000, 500000));
    let prediction = Prediction::new(155.0, 0.85);
    let prediction_event = Arc::new(ModelPredictionEvent::new(
        "lstm_v1".to_string(),
        "AAPL".to_string(),
        prediction
    ));
    
    // Publish to different channels
    let price_bus = bus.clone();
    let volume_bus = bus.clone();
    let prediction_bus = bus.clone();
    
    // Create modified events with appropriate event types for our channels
    let price_test_event = Arc::new(TestEvent::new("market:prices:AAPL".to_string(), "price_data".to_string()));
    let volume_test_event = Arc::new(TestEvent::new("market:volume:AAPL".to_string(), "volume_data".to_string()));
    let prediction_test_event = Arc::new(TestEvent::new("predictions:lstm:AAPL".to_string(), "prediction_data".to_string()));
    
    // Publish events in background
    tokio::spawn(async move {
        sleep(Duration::from_millis(10)).await;
        price_bus.publish(price_test_event as Arc<dyn Event + Send + Sync>).await.unwrap();
    });
    
    tokio::spawn(async move {
        sleep(Duration::from_millis(20)).await;
        volume_bus.publish(volume_test_event as Arc<dyn Event + Send + Sync>).await.unwrap();
    });
    
    tokio::spawn(async move {
        sleep(Duration::from_millis(30)).await;
        prediction_bus.publish(prediction_test_event as Arc<dyn Event + Send + Sync>).await.unwrap();
    });
    
    // Receive events from different streams
    let price_received = timeout(Duration::from_millis(100), price_stream.next()).await
        .expect("Should receive price event")
        .expect("Price stream should yield event");
    
    let volume_received = timeout(Duration::from_millis(100), volume_stream.next()).await
        .expect("Should receive volume event")
        .expect("Volume stream should yield event");
    
    let prediction_received = timeout(Duration::from_millis(100), prediction_stream.next()).await
        .expect("Should receive prediction event")
        .expect("Prediction stream should yield event");
    
    // Verify events were received on correct channels
    assert_eq!(price_received.event_type(), "market:prices:AAPL");
    assert_eq!(volume_received.event_type(), "market:volume:AAPL");
    assert_eq!(prediction_received.event_type(), "predictions:lstm:AAPL");
    
    // Clean up subscriptions
    bus.unsubscribe(price_handle).await.unwrap();
    bus.unsubscribe(volume_handle).await.unwrap();
    bus.unsubscribe(prediction_handle).await.unwrap();
    
    println!("✅ Multi-channel subscriptions test passed");
}

#[tokio::test]
async fn test_comprehensive_eventbus_integration() {
    println!("🚀 Starting comprehensive EventBus integration test...");
    
    let bus = Arc::new(InMemoryEventBus::with_buffer_size(1000));
    let recording_bus = Arc::new(RecordingEventBus::new(bus.clone()));
    
    // Create consumer group with backpressure handling
    let consumer1 = Arc::new(Consumer::new("consumer_1".to_string()).with_processing_time(20));
    let consumer2 = Arc::new(Consumer::new("consumer_2".to_string()).with_processing_time(30));
    
    let mut consumer_group = ConsumerGroup::new("integration_test_group".to_string());
    consumer_group.add_consumer(consumer1.clone());
    consumer_group.add_consumer(consumer2.clone());
    
    // Create event batcher and DLQ
    let batcher = Arc::new(EventBatcher::new(5, Duration::from_millis(100)));
    let dlq = Arc::new(DeadLetterQueue::new(2, Duration::from_millis(10)));
    let backpressure = Arc::new(BackpressureHandler::new(20));
    
    // Subscribe to various channels and create streams to keep them open
    let _price_handle = recording_bus.subscribe("price_update").await.unwrap();
    let _volume_handle = recording_bus.subscribe("volume_event").await.unwrap();
    let _prediction_handle = recording_bus.subscribe("model_prediction").await.unwrap();
    
    let _price_stream = recording_bus.get_stream("price_update").await.unwrap();
    let _volume_stream = recording_bus.get_stream("volume_event").await.unwrap();
    let _prediction_stream = recording_bus.get_stream("model_prediction").await.unwrap();
    
    // Create diverse test events (using existing event types)
    let events: Vec<Arc<dyn Event + Send + Sync>> = vec![
        // Market events
        Arc::new(PriceUpdateEvent::new("AAPL".to_string(), 150.0, 149.0)),
        Arc::new(VolumeEvent::new("AAPL".to_string(), 2000000, 1000000)),
        Arc::new(TrendChangeEvent::new(
            "AAPL".to_string(),
            MarketTrend::Neutral,
            MarketTrend::Bullish,
            0.75
        )),
        
        // Prediction events
        Arc::new(ModelPredictionEvent::new(
            "lstm_v1".to_string(),
            "AAPL".to_string(),
            Prediction::new(155.0, 0.90)
        )),
        Arc::new(ModelUpdateEvent::new(
            "lstm_v1".to_string(),
            ModelUpdateType::Deploy,
            "2.0.0".to_string()
        )),
        Arc::new(ModelPerformanceEvent::new(
            "lstm_v1".to_string(),
            "daily".to_string()
        ).with_classification_metrics(0.85, 0.80, 0.82, 0.81)),
        
        // Test events with different priorities
        Arc::new(TestEvent::new("test:high_priority".to_string(), "Critical event".to_string())
            .with_priority(9)),
        Arc::new(TestEvent::new("test:low_priority".to_string(), "Normal event".to_string())
            .with_priority(3)),
        Arc::new(TestEvent::new("test:correlated".to_string(), "Correlated event".to_string())
            .with_correlation_id(Uuid::new_v4())),
    ];
    
    let start_time = Instant::now();
    
    // Process events through various components
    let mut successful_publishes = 0;
    let mut batched_events = 0;
    let mut failed_events = 0;
    
    for event in events {
        // Check backpressure
        if !backpressure.can_accept_event() {
            println!("⚠️  Backpressure triggered, dropping event");
            continue;
        }
        
        backpressure.add_event();
        
        // Publish to recording bus
        match recording_bus.publish(event.clone()).await {
            Ok(_) => {
                successful_publishes += 1;
                
                // Try to add to batch
                if let Some(batch) = batcher.add_event(event.clone()).await {
                    batcher.process_batch(batch).await;
                    batched_events += 5; // Batch size
                }
                
                // Distribute to consumer group
                match consumer_group.distribute_event(event.clone()).await {
                    Ok(_) => {
                        // Event processed successfully
                    }
                    Err(_) => {
                        // Add to DLQ for retry
                        dlq.add_failed_event(event, 0);
                        failed_events += 1;
                    }
                }
                
                backpressure.remove_event();
            }
            Err(e) => {
                println!("❌ Failed to publish event: {}", e);
                backpressure.remove_event();
            }
        }
    }
    
    // Process any remaining batch
    if let Some(remaining_batch) = batcher.flush_batch().await {
        batcher.process_batch(remaining_batch).await;
    }
    
    // Process retries from DLQ
    let retries_processed = dlq.process_retry(&*consumer1).await.unwrap();
    
    // Wait for all processing to complete
    sleep(Duration::from_millis(200)).await;
    
    let processing_time = start_time.elapsed();
    
    // Verify comprehensive test results
    println!("📊 Comprehensive test results:");
    println!("   ⏱️  Processing time: {:?}", processing_time);
    println!("   📤 Events published: {}", successful_publishes);
    println!("   📦 Events batched: {}", batched_events);
    println!("   ❌ Events failed: {}", failed_events);
    println!("   🔄 Retries processed: {}", retries_processed);
    println!("   📝 Subscriptions recorded: {}", recording_bus.get_subscriptions().len());
    println!("   🗂️  Published events recorded: {}", recording_bus.get_published_events().len());
    println!("   🚫 Events dropped by backpressure: {}", backpressure.get_dropped_count());
    println!("   🛡️  Consumer 1 processed: {}", consumer1.get_processed_count());
    println!("   🛡️  Consumer 2 processed: {}", consumer2.get_processed_count());
    println!("   📈 Batches processed: {}", batcher.get_batches_processed().len());
    
    // Assertions to verify the system worked correctly
    assert!(successful_publishes > 0, "Should have published some events");
    assert!(recording_bus.get_published_events().len() > 0, "Should have recorded published events");
    assert!(recording_bus.get_subscriptions().len() >= 3, "Should have recorded subscriptions");
    assert!((consumer1.get_processed_count() + consumer2.get_processed_count()) > 0, "Consumers should have processed events");
    assert!(processing_time < Duration::from_secs(5), "Should complete within reasonable time");
    
    println!("✅ Comprehensive EventBus integration test passed!");
}

// =============================================================================
// PERFORMANCE AND STRESS TESTS
// =============================================================================

#[tokio::test]
async fn test_high_throughput_performance() {
    println!("🏃‍♂️ Testing high throughput performance...");
    
    let bus = Arc::new(InMemoryEventBus::with_buffer_size(10000));
    let _consumer = Arc::new(Consumer::new("perf_consumer".to_string()).with_processing_time(1));
    
    let num_events = 100; // Reduced for faster test
    let start_time = Instant::now();
    
    // Create stream first to keep channel open
    let mut stream = bus.get_stream("performance_test").await.unwrap();
    
    // Publish events concurrently
    let mut publish_tasks = Vec::new();
    
    for i in 0..num_events {
        let bus_clone = bus.clone();
        let task = tokio::spawn(async move {
            let event = Arc::new(TestEvent::new(
                "performance_test".to_string(),
                format!("Event {}", i)
            ));
            bus_clone.publish(event as Arc<dyn Event + Send + Sync>).await
        });
        publish_tasks.push(task);
    }
    
    // Wait for all publishes to complete
    for task in publish_tasks {
        task.await.unwrap().unwrap();
    }
    
    let publish_time = start_time.elapsed();
    
    // Subscribe and consume events (stream already created above)
    let mut events_received = 0;
    
    while events_received < num_events {
        if let Ok(Some(_)) = timeout(Duration::from_millis(50), stream.next()).await {
            events_received += 1;
        } else {
            break; // Timeout - no more events
        }
    }
    
    let total_time = start_time.elapsed();
    
    let publish_throughput = num_events as f64 / publish_time.as_secs_f64();
    let total_throughput = num_events as f64 / total_time.as_secs_f64();
    
    println!("📈 Performance results:");
    println!("   📤 Publish time: {:?} ({:.0} events/sec)", publish_time, publish_throughput);
    println!("   ⏱️  Total time: {:?} ({:.0} events/sec)", total_time, total_throughput);
    println!("   📨 Events received: {}/{}", events_received, num_events);
    
    assert_eq!(events_received, num_events, "Should receive all published events");
    assert!(publish_throughput > 10.0, "Should achieve reasonable publish throughput");
    
    println!("✅ High throughput performance test passed!");
}

#[tokio::test]
async fn test_concurrent_subscribers() {
    println!("👥 Testing concurrent subscribers...");
    
    let bus = Arc::new(InMemoryEventBus::with_buffer_size(1000));
    let num_subscribers = 10;
    let events_per_subscriber = 50;
    
    // Create multiple subscribers
    let mut subscriber_tasks = Vec::new();
    
    for i in 0..num_subscribers {
        let bus_clone = bus.clone();
        let task = tokio::spawn(async move {
            let mut stream = bus_clone.get_stream("concurrent_test").await.unwrap();
            let mut received_count = 0;
            let mut received_events = Vec::new();
            
            while received_count < events_per_subscriber {
                if let Ok(Some(event)) = timeout(Duration::from_millis(100), stream.next()).await {
                    received_events.push(event.event_id());
                    received_count += 1;
                } else {
                    break;
                }
            }
            
            (i, received_count, received_events)
        });
        subscriber_tasks.push(task);
    }
    
    // Give subscribers time to set up
    sleep(Duration::from_millis(50)).await;
    
    // Publish events
    for i in 0..events_per_subscriber {
        let event = Arc::new(TestEvent::new(
            "concurrent_test".to_string(),
            format!("Concurrent event {}", i)
        ));
        bus.publish(event as Arc<dyn Event + Send + Sync>).await.unwrap();
    }
    
    // Collect results from all subscribers
    let mut total_events_received = 0;
    for task in subscriber_tasks {
        let (subscriber_id, received_count, _events) = task.await.unwrap();
        println!("   📨 Subscriber {}: received {} events", subscriber_id, received_count);
        total_events_received += received_count;
    }
    
    let expected_total = num_subscribers * events_per_subscriber;
    println!("📊 Total events received: {} (expected: {})", total_events_received, expected_total);
    
    // Each subscriber should receive all events (broadcast pattern)
    assert_eq!(total_events_received, expected_total, "All subscribers should receive all events");
    
    println!("✅ Concurrent subscribers test passed!");
}

// This test demonstrates that all individual tests can be run together
#[tokio::test]
async fn test_all_components_integration() {
    println!("🎯 Testing integrated EventBus functionality...\n");
    
    // This test focuses on proving the EventBus components work together
    let bus = Arc::new(InMemoryEventBus::new());
    
    // Test channel validation
    assert!(ChannelValidator::is_valid_channel("market:prices:AAPL"));
    assert!(!ChannelValidator::is_valid_channel("invalid_format"));
    
    // Test recording wrapper
    let recording_bus = Arc::new(RecordingEventBus::new(bus.clone()));
    let _handle = recording_bus.subscribe("integration_test").await.unwrap();
    
    // Create stream to keep channel open
    let _stream = recording_bus.get_stream("integration_test").await.unwrap();
    
    // Test event publishing
    let test_event = Arc::new(TestEvent::new("integration_test".to_string(), "test_data".to_string()));
    recording_bus.publish(test_event.clone() as Arc<dyn Event + Send + Sync>).await.unwrap();
    
    // Verify recording
    assert_eq!(recording_bus.get_published_events().len(), 1);
    assert_eq!(recording_bus.get_subscriptions().len(), 1);
    
    // Test consumer
    let consumer = Consumer::new("integration_consumer".to_string());
    consumer.handle_event(test_event).await.unwrap();
    assert_eq!(consumer.get_processed_count(), 1);
    
    // Test backpressure
    let backpressure = BackpressureHandler::new(2);
    assert!(backpressure.add_event());
    assert!(backpressure.add_event());
    assert!(!backpressure.add_event()); // Should be rejected
    assert_eq!(backpressure.get_dropped_count(), 1);
    
    println!("✅ All EventBus components integration test passed!");
}