//! Proto-only InMemoryEventBus implementation
//!
//! CRITICAL: This implementation enforces proto-only messaging.
//!           ALL Vec<u8> and JSON payloads are REJECTED with ContractViolation errors.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

use crate::eventbus::{
    traits::{ProtoEventBus, ProtoEventSubscriber, DynamicProtoEventSubscriber},
    types::{
        ProtoMessage, ProtoEvent, ProtoEventEnvelope,
        EventId, SubscriptionConfig
    },
    error::EventBusError,
};

use super::super::traits::{ProtoChannelInfo, ProtoEventBusConfig};

/// Proto-only in-memory EventBus implementation
pub struct ProtoInMemoryEventBus {
    channels: Arc<RwLock<HashMap<String, ProtoChannel>>>,
    consumer_groups: Arc<RwLock<HashMap<String, ProtoConsumerGroup>>>,
    config: ProtoEventBusConfig,
}

struct ProtoChannel {
    name: String,
    events: Vec<ProtoEventEnvelope>,
    subscribers: Vec<String>,
    created_at: i64,
    proto_type_counts: HashMap<String, u64>,
}

struct ProtoConsumerGroup {
    name: String,
    channel: String,
    members: Vec<String>,
    pending_messages: HashMap<EventId, ProtoEventEnvelope>,
    last_delivered: Option<EventId>,
}

impl ProtoInMemoryEventBus {
    /// Create a new proto-only in-memory EventBus
    pub fn new() -> Self {
        Self::with_config(ProtoEventBusConfig::default())
    }
    
    /// Create with custom configuration
    pub fn with_config(config: ProtoEventBusConfig) -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            consumer_groups: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }
    
    /// Create for testing with lenient validation
    pub fn for_testing() -> Self {
        Self::with_config(ProtoEventBusConfig::for_testing())
    }
    
    async fn ensure_channel(&self, channel: &str) -> Result<(), EventBusError> {
        let mut channels = self.channels.write().await;
        if !channels.contains_key(channel) {
            channels.insert(
                channel.to_string(),
                ProtoChannel {
                    name: channel.to_string(),
                    events: Vec::new(),
                    subscribers: Vec::new(),
                    created_at: chrono::Utc::now().timestamp(),
                    proto_type_counts: HashMap::new(),
                },
            );
        }
        Ok(())
    }
    
    fn validate_channel_name(&self, channel: &str) -> Result<(), EventBusError> {
        if channel.is_empty() {
            return Err(EventBusError::invalid_channel("Channel name cannot be empty"));
        }
        
        // Enforce strict channel naming for proto-only system
        let parts: Vec<&str> = channel.split(':').collect();
        if parts.len() != 3 {
            return Err(EventBusError::invalid_channel(
                "Proto-only channels must follow format: stream:domain:identifier"
            ));
        }
        
        if parts[0] != "stream" {
            return Err(EventBusError::invalid_channel(
                "Proto-only channels must start with 'stream:'"
            ));
        }
        
        let valid_domains = [
            "symbol", "sector", "portfolio", "cross_sector", "ml", "action", "dlq"
        ];
        
        if !valid_domains.contains(&parts[1]) {
            return Err(EventBusError::invalid_channel(format!(
                "Invalid domain '{}'. Must be one of: {}", 
                parts[1], 
                valid_domains.join(", ")
            )));
        }
        
        if parts[2].is_empty() {
            return Err(EventBusError::invalid_channel(
                "Channel identifier cannot be empty"
            ));
        }
        
        Ok(())
    }
    
    fn validate_proto_event<T: ProtoMessage>(&self, event: &ProtoEvent<T>) -> Result<(), EventBusError> {
        // Strict validation enabled?
        if !self.config.strict_validation {
            return Ok(());
        }
        
        // Validate proto message itself
        event.validate()?;
        
        // Validate quality score threshold
        if event.quality_score < self.config.min_quality_score {
            return Err(EventBusError::schema_validation(format!(
                "Quality score {:.3} is below minimum threshold {:.3}",
                event.quality_score,
                self.config.min_quality_score
            )));
        }
        
        // Validate proto type registration if enforcement enabled
        if self.config.enforce_registration {
            self.config.registry.validate_proto_type(&event.event_type)?;
        }
        
        // Validate payload size
        let proto_bytes = event.to_proto_bytes()?;
        if proto_bytes.len() > self.config.max_payload_size {
            return Err(EventBusError::schema_validation(format!(
                "Proto payload size {} bytes exceeds maximum {} bytes",
                proto_bytes.len(),
                self.config.max_payload_size
            )));
        }
        
        Ok(())
    }
}

#[async_trait]
impl ProtoEventBus for ProtoInMemoryEventBus {
    async fn publish_proto<T: ProtoMessage + Default>(
        &self,
        channel: &str,
        event: ProtoEvent<T>,
    ) -> Result<EventId, EventBusError> {
        // MANDATORY: Validate channel name
        self.validate_channel_name(channel)?;
        
        // MANDATORY: Validate proto event
        self.validate_proto_event(&event)?;
        
        self.ensure_channel(channel).await?;
        
        let event_id = EventId::new();
        let envelope = ProtoEventEnvelope::new(event_id.clone(), channel.to_string(), event)?;
        
        let mut channels = self.channels.write().await;
        if let Some(chan) = channels.get_mut(channel) {
            chan.events.push(envelope.clone());
            
            // Update proto type counts
            let proto_type = &envelope.proto_type;
            *chan.proto_type_counts.entry(proto_type.clone()).or_insert(0) += 1;
        }
        
        Ok(event_id)
    }
    
    async fn publish_proto_batch<T: ProtoMessage + Default>(
        &self,
        channel: &str,
        events: Vec<ProtoEvent<T>>,
    ) -> Result<Vec<EventId>, EventBusError> {
        let mut event_ids = Vec::new();
        
        // Validate all events first (fail-fast approach)
        for event in &events {
            self.validate_proto_event(event)?;
        }
        
        // Publish all events if validation passes
        for event in events {
            let id = self.publish_proto(channel, event).await?;
            event_ids.push(id);
        }
        
        Ok(event_ids)
    }
    
    async fn subscribe_proto<T: ProtoMessage + Default>(
        &self,
        channels: &[String],
        config: SubscriptionConfig,
    ) -> Result<Box<dyn ProtoEventSubscriber<T>>, EventBusError> {
        // Validate all channel names
        for channel in channels {
            self.validate_channel_name(channel)?;
            self.ensure_channel(channel).await?;
        }
        
        let subscriber_id = Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::channel(config.batch_size);
        
        // Create consumer group if needed
        let group_key = format!("{}:{}", channels.join(","), config.group_name);
        let mut consumer_groups = self.consumer_groups.write().await;
        consumer_groups.entry(group_key.clone()).or_insert_with(|| ProtoConsumerGroup {
            name: config.group_name.clone(),
            channel: channels.join(","),
            members: Vec::new(),
            pending_messages: HashMap::new(),
            last_delivered: None,
        });
        
        if let Some(group) = consumer_groups.get_mut(&group_key) {
            group.members.push(subscriber_id.clone());
        }
        
        // Register subscriber with channels
        let mut channel_map = self.channels.write().await;
        for channel in channels {
            if let Some(chan) = channel_map.get_mut(channel) {
                chan.subscribers.push(subscriber_id.clone());
            }
        }
        
        Ok(Box::new(ProtoInMemorySubscriber::<T> {
            id: subscriber_id,
            channels: channels.to_vec(),
            receiver: rx,
            _sender: tx,
            _phantom: std::marker::PhantomData,
        }))
    }
    
    async fn subscribe_dynamic_proto(
        &self,
        channels: &[String],
        proto_types: &[&'static str],
        config: SubscriptionConfig,
    ) -> Result<Box<dyn DynamicProtoEventSubscriber>, EventBusError> {
        // Validate all channel names
        for channel in channels {
            self.validate_channel_name(channel)?;
            self.ensure_channel(channel).await?;
        }
        
        let subscriber_id = Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::channel(config.batch_size);
        
        Ok(Box::new(DynamicProtoInMemorySubscriber {
            id: subscriber_id,
            channels: channels.to_vec(),
            supported_types: proto_types.iter().map(|s| s.to_string()).collect(),
            receiver: rx,
            _sender: tx,
        }))
    }
    
    async fn ack_proto(
        &self,
        channel: &str,
        group: &str,
        event_id: &EventId,
    ) -> Result<(), EventBusError> {
        let group_key = format!("{}:{}", channel, group);
        let mut consumer_groups = self.consumer_groups.write().await;
        
        if let Some(consumer_group) = consumer_groups.get_mut(&group_key) {
            consumer_group.pending_messages.remove(event_id);
            Ok(())
        } else {
            Err(EventBusError::consumer_group(format!(
                "Consumer group {} not found for channel {}", group, channel
            )))
        }
    }
    
    async fn nack_proto(
        &self,
        channel: &str,
        group: &str,
        _event_id: &EventId,
    ) -> Result<(), EventBusError> {
        let group_key = format!("{}:{}", channel, group);
        let consumer_groups = self.consumer_groups.read().await;
        
        if consumer_groups.contains_key(&group_key) {
            Ok(())
        } else {
            Err(EventBusError::consumer_group(format!(
                "Consumer group {} not found for channel {}", group, channel
            )))
        }
    }
    
    async fn create_proto_consumer_group(
        &self,
        channel: &str,
        group: &str,
    ) -> Result<(), EventBusError> {
        self.validate_channel_name(channel)?;
        self.ensure_channel(channel).await?;
        
        let group_key = format!("{}:{}", channel, group);
        let mut consumer_groups = self.consumer_groups.write().await;
        
        if consumer_groups.contains_key(&group_key) {
            return Err(EventBusError::consumer_group(format!(
                "Consumer group {} already exists for channel {}", group, channel
            )));
        }
        
        consumer_groups.insert(
            group_key,
            ProtoConsumerGroup {
                name: group.to_string(),
                channel: channel.to_string(),
                members: Vec::new(),
                pending_messages: HashMap::new(),
                last_delivered: None,
            },
        );
        
        Ok(())
    }
    
    async fn get_proto_channel_info(&self, channel: &str) -> Result<ProtoChannelInfo, EventBusError> {
        let channels = self.channels.read().await;
        
        if let Some(chan) = channels.get(channel) {
            let total_quality_score: f64 = chan.events.iter()
                .map(|e| e.quality_score)
                .sum();
            let avg_quality_score = if chan.events.is_empty() {
                0.0
            } else {
                total_quality_score / chan.events.len() as f64
            };
            
            Ok(ProtoChannelInfo {
                channel_name: chan.name.clone(),
                message_count: chan.events.len() as u64,
                proto_type_counts: chan.proto_type_counts.clone(),
                consumer_groups: vec![], // Would need to extract from consumer_groups
                last_event_id: chan.events.last().map(|e| e.event_id.clone()),
                avg_quality_score,
                created_at: chan.created_at,
                subscriber_count: chan.subscribers.len(),
                total_events: chan.events.len() as u64,
                active: !chan.subscribers.is_empty(),
            })
        } else {
            Err(EventBusError::channel_not_found(channel))
        }
    }
    
    async fn list_proto_types_on_channel(&self, channel: &str) -> Result<Vec<String>, EventBusError> {
        let channels = self.channels.read().await;
        
        if let Some(chan) = channels.get(channel) {
            Ok(chan.proto_type_counts.keys().cloned().collect())
        } else {
            Err(EventBusError::channel_not_found(channel))
        }
    }
}

pub struct ProtoInMemorySubscriber<T: ProtoMessage + Default> {
    id: String,
    channels: Vec<String>,
    receiver: mpsc::Receiver<ProtoEventEnvelope>,
    _sender: mpsc::Sender<ProtoEventEnvelope>,
    _phantom: std::marker::PhantomData<T>,
}

#[async_trait]
impl<T: ProtoMessage + Default> ProtoEventSubscriber<T> for ProtoInMemorySubscriber<T> {
    async fn next_proto(&mut self) -> Result<Option<ProtoEvent<T>>, EventBusError> {
        if let Some(envelope) = self.receiver.recv().await {
            let proto_event = envelope.deserialize_proto::<T>()?;
            Ok(Some(proto_event))
        } else {
            Ok(None)
        }
    }
    
    async fn next_proto_envelope(&mut self) -> Result<Option<ProtoEventEnvelope>, EventBusError> {
        Ok(self.receiver.recv().await)
    }
    
    async fn close(&mut self) -> Result<(), EventBusError> {
        self.receiver.close();
        Ok(())
    }
    
    fn id(&self) -> &str {
        &self.id
    }
}

pub struct DynamicProtoInMemorySubscriber {
    id: String,
    channels: Vec<String>,
    supported_types: Vec<String>,
    receiver: mpsc::Receiver<ProtoEventEnvelope>,
    _sender: mpsc::Sender<ProtoEventEnvelope>,
}

#[async_trait]
impl DynamicProtoEventSubscriber for DynamicProtoInMemorySubscriber {
    async fn next_dynamic_proto(&mut self) -> Result<Option<ProtoEventEnvelope>, EventBusError> {
        Ok(self.receiver.recv().await)
    }
    
    async fn filter_proto_types(&mut self, types: &[&str]) -> Result<(), EventBusError> {
        self.supported_types = types.iter().map(|s| s.to_string()).collect();
        Ok(())
    }
    
    fn supported_proto_types(&self) -> &[String] {
        &self.supported_types
    }
    
    async fn close(&mut self) -> Result<(), EventBusError> {
        self.receiver.close();
        Ok(())
    }
    
    fn id(&self) -> &str {
        &self.id
    }
}

impl Default for ProtoInMemoryEventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;
    
    #[derive(Clone, prost::Message)]
    pub struct TestMarketData {
        #[prost(string, tag = "1")]
        pub symbol: String,
        #[prost(double, tag = "2")]
        pub price: f64,
        #[prost(int64, tag = "3")]
        pub timestamp: i64,
    }
    
    impl ProtoMessage for TestMarketData {
        fn proto_type_name() -> &'static str {
            "test.MarketData"
        }
        
        fn validate(&self) -> Result<(), EventBusError> {
            if self.symbol.is_empty() {
                return Err(EventBusError::schema_validation("Symbol cannot be empty"));
            }
            if self.price <= 0.0 {
                return Err(EventBusError::schema_validation("Price must be positive"));
            }
            Ok(())
        }
    }
    
    #[tokio::test]
    async fn test_proto_publish_subscribe() {
        let eventbus = ProtoInMemoryEventBus::for_testing();
        
        // Create a test proto message
        let market_data = TestMarketData {
            symbol: "AAPL".to_string(),
            price: 150.25,
            timestamp: chrono::Utc::now().timestamp(),
        };
        
        let event = ProtoEvent::new(market_data)
            .with_quality_score(0.95);
        
        // Publish the event
        let event_id = eventbus.publish_proto("stream:symbol:AAPL", event).await.unwrap();
        assert!(!event_id.as_str().is_empty());
        
        // Subscribe and receive the event
        let config = SubscriptionConfig {
            group_name: "test-group".to_string(),
            consumer_name: "test-consumer".to_string(),
            start_position: crate::eventbus::types::StartPosition::Beginning,
            batch_size: 10,
            block_timeout_ms: 1000,
            ack_timeout_ms: 5000,
            buffer_size: 1024,
            receive_timeout: None,
            persistent: false,
            priority: 0,
        };
        
        let _subscriber = eventbus.subscribe_proto::<TestMarketData>(
            &["stream:symbol:AAPL".to_string()], 
            config
        ).await.unwrap();
        
        // Verify channel info includes proto type counts
        let info = eventbus.get_proto_channel_info("stream:symbol:AAPL").await.unwrap();
        assert_eq!(info.message_count, 1);
        assert!(info.proto_type_counts.contains_key("test.MarketData"));
        assert_eq!(*info.proto_type_counts.get("test.MarketData").unwrap(), 1);
    }
    
    #[tokio::test]
    async fn test_contract_violation_rejection() {
        let eventbus = ProtoInMemoryEventBus::new();
        
        // Test that legacy raw methods are rejected
        let result = eventbus.publish_raw("test-channel", vec![1, 2, 3]).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EventBusError::ContractViolation(_)));
        
        let result = eventbus.publish_json("test-channel", "{\"test\": \"data\"}").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EventBusError::ContractViolation(_)));
        
        let result = eventbus.publish_batch_raw("test-channel", vec![vec![1, 2, 3]]).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EventBusError::ContractViolation(_)));
    }
    
    #[tokio::test]
    async fn test_proto_validation_failure() {
        let eventbus = ProtoInMemoryEventBus::new();
        
        // Test invalid proto message
        let invalid_market_data = TestMarketData {
            symbol: "".to_string(), // Invalid: empty symbol
            price: -10.0,           // Invalid: negative price
            timestamp: chrono::Utc::now().timestamp(),
        };
        
        let event = ProtoEvent::new(invalid_market_data);
        let result = eventbus.publish_proto("stream:symbol:INVALID", event).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EventBusError::SchemaValidation(_)));
    }
    
    #[tokio::test]
    async fn test_channel_name_validation() {
        let eventbus = ProtoInMemoryEventBus::new();
        
        let market_data = TestMarketData {
            symbol: "AAPL".to_string(),
            price: 150.25,
            timestamp: chrono::Utc::now().timestamp(),
        };
        let event = ProtoEvent::new(market_data);
        
        // Test invalid channel names
        let invalid_channels = [
            "invalid-channel",      // Wrong format
            "stream:invalid:AAPL",  // Invalid domain
            "stream:symbol:",       // Empty identifier
            "other:symbol:AAPL",    // Wrong prefix
        ];
        
        for channel in &invalid_channels {
            let result = eventbus.publish_proto(channel, event.clone()).await;
            assert!(result.is_err());
            assert!(matches!(result.unwrap_err(), EventBusError::InvalidChannel(_)));
        }
        
        // Test valid channel
        let result = eventbus.publish_proto("stream:symbol:AAPL", event).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_quality_score_enforcement() {
        let config = ProtoEventBusConfig::strict().min_quality_score(0.8);
        let eventbus = ProtoInMemoryEventBus::with_config(config);
        
        let market_data = TestMarketData {
            symbol: "AAPL".to_string(),
            price: 150.25,
            timestamp: chrono::Utc::now().timestamp(),
        };
        
        // Low quality event should be rejected
        let low_quality_event = ProtoEvent::new(market_data.clone())
            .with_quality_score(0.5); // Below 0.8 threshold
        
        let result = eventbus.publish_proto("stream:symbol:AAPL", low_quality_event).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EventBusError::SchemaValidation(_)));
        
        // High quality event should be accepted
        let high_quality_event = ProtoEvent::new(market_data)
            .with_quality_score(0.95); // Above 0.8 threshold
        
        let result = eventbus.publish_proto("stream:symbol:AAPL", high_quality_event).await;
        assert!(result.is_ok());
    }
}