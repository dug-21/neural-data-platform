//! Proto-only EventBus traits for Phase 4 enforcement
//!
//! CRITICAL: This trait REJECTS all Vec<u8> and JSON payloads. 
//!           ONLY protobuf messages are accepted.

use async_trait::async_trait;

use crate::eventbus::{
    error::EventBusError,
    types::{EventId, ChannelInfo, SubscriptionConfig},
};

use crate::eventbus::types::{
    ProtoMessage, ProtoEvent, ProtoEventEnvelope, reject_raw_payload, reject_json_payload
};

/// Proto-only EventBus trait - ZERO tolerance for non-proto messages
#[async_trait]
pub trait ProtoEventBus: Send + Sync {
    /// Publish a proto message to a channel (type-safe)
    /// 
    /// # Arguments
    /// * `channel` - The channel name to publish to
    /// * `event` - The strongly-typed proto event to publish
    ///
    /// # Returns
    /// * `Ok(EventId)` if the event was published successfully
    /// * `Err(EventBusError::ContractViolation)` for any non-proto attempts
    /// * `Err(EventBusError::SchemaValidation)` for invalid proto messages
    async fn publish_proto<T: ProtoMessage + Default>(
        &self,
        channel: &str,
        event: ProtoEvent<T>,
    ) -> Result<EventId, EventBusError>;
    
    /// Publish a batch of proto messages (type-safe)
    async fn publish_proto_batch<T: ProtoMessage + Default>(
        &self,
        channel: &str,
        events: Vec<ProtoEvent<T>>,
    ) -> Result<Vec<EventId>, EventBusError>;
    
    /// Subscribe to proto messages of a specific type (type-safe)
    /// 
    /// # Arguments
    /// * `channels` - The channel names to subscribe to
    /// * `config` - Subscription configuration
    ///
    /// # Returns
    /// * `Ok(ProtoSubscriber<T>)` if subscription was successful
    /// * `Err(EventBusError)` if the subscription failed
    async fn subscribe_proto<T: ProtoMessage + Default>(
        &self,
        channels: &[String],
        config: SubscriptionConfig,
    ) -> Result<Box<dyn ProtoEventSubscriber<T>>, EventBusError>;
    
    /// Subscribe to multiple proto message types on same channel
    async fn subscribe_dynamic_proto(
        &self,
        channels: &[String],
        proto_types: &[&'static str],
        config: SubscriptionConfig,
    ) -> Result<Box<dyn DynamicProtoEventSubscriber>, EventBusError>;
    
    /// Acknowledge successful proto event processing
    async fn ack_proto(
        &self, 
        channel: &str, 
        group: &str, 
        event_id: &EventId
    ) -> Result<(), EventBusError>;
    
    /// Negative acknowledgment for failed proto event processing  
    async fn nack_proto(
        &self, 
        channel: &str, 
        group: &str, 
        event_id: &EventId
    ) -> Result<(), EventBusError>;
    
    /// Create a consumer group for proto events
    async fn create_proto_consumer_group(
        &self, 
        channel: &str, 
        group: &str
    ) -> Result<(), EventBusError>;
    
    /// Get channel information (proto-aware)
    async fn get_proto_channel_info(&self, channel: &str) -> Result<ProtoChannelInfo, EventBusError>;
    
    /// List all proto message types seen on a channel
    async fn list_proto_types_on_channel(&self, channel: &str) -> Result<Vec<String>, EventBusError>;

    // LEGACY METHODS - ALL MUST RETURN CONTRACT VIOLATIONS
    
    /// DEPRECATED: Raw publish is BANNED - proto messages ONLY
    async fn publish_raw(&self, _channel: &str, _payload: Vec<u8>) -> Result<EventId, EventBusError> {
        Err(reject_raw_payload())
    }
    
    /// DEPRECATED: JSON publish is BANNED - proto messages ONLY
    async fn publish_json(&self, _channel: &str, _payload: &str) -> Result<EventId, EventBusError> {
        Err(reject_json_payload())
    }
    
    /// DEPRECATED: Raw batch publish is BANNED - proto messages ONLY
    async fn publish_batch_raw(&self, _channel: &str, _payloads: Vec<Vec<u8>>) -> Result<Vec<EventId>, EventBusError> {
        Err(reject_raw_payload())
    }
}

/// Proto-only Event Subscriber trait for specific proto types
#[async_trait]
pub trait ProtoEventSubscriber<T: ProtoMessage>: Send + Sync {
    /// Get the next proto event of the specified type
    async fn next_proto(&mut self) -> Result<Option<ProtoEvent<T>>, EventBusError>;
    
    /// Get the next proto event envelope (with routing metadata)
    async fn next_proto_envelope(&mut self) -> Result<Option<ProtoEventEnvelope>, EventBusError>;
    
    /// Close the subscription
    async fn close(&mut self) -> Result<(), EventBusError>;
    
    /// Get the subscriber ID
    fn id(&self) -> &str;
    
    /// Get the proto type name this subscriber handles
    fn proto_type_name(&self) -> &'static str {
        T::proto_type_name()
    }
}

/// Dynamic proto event subscriber for handling multiple proto types
#[async_trait]
pub trait DynamicProtoEventSubscriber: Send + Sync {
    /// Get the next proto envelope (any supported proto type)
    async fn next_dynamic_proto(&mut self) -> Result<Option<ProtoEventEnvelope>, EventBusError>;
    
    /// Filter to specific proto types
    async fn filter_proto_types(&mut self, types: &[&str]) -> Result<(), EventBusError>;
    
    /// Get supported proto types
    fn supported_proto_types(&self) -> &[String];
    
    /// Close the subscription
    async fn close(&mut self) -> Result<(), EventBusError>;
    
    /// Get the subscriber ID
    fn id(&self) -> &str;
}

/// Proto-aware channel information
#[derive(Debug, Clone)]
pub struct ProtoChannelInfo {
    /// Channel name
    pub channel_name: String,
    
    /// Total message count
    pub message_count: u64,
    
    /// Proto type distribution
    pub proto_type_counts: std::collections::HashMap<String, u64>,
    
    /// Consumer groups
    pub consumer_groups: Vec<String>,
    
    /// Last event ID
    pub last_event_id: Option<EventId>,
    
    /// Average quality score
    pub avg_quality_score: f64,
    
    /// Channel creation timestamp
    pub created_at: i64,
    
    /// Number of active subscribers
    pub subscriber_count: usize,
    
    /// Total events processed
    pub total_events: u64,
    
    /// Is channel active
    pub active: bool,
}

impl From<ChannelInfo> for ProtoChannelInfo {
    fn from(info: ChannelInfo) -> Self {
        Self {
            channel_name: info.channel_name,
            message_count: info.message_count,
            proto_type_counts: std::collections::HashMap::new(),
            consumer_groups: vec![], // Would need to extract from info
            last_event_id: info.last_event_id,
            avg_quality_score: 1.0, // Default for non-proto channels
            created_at: info.created_at,
            subscriber_count: info.subscriber_count,
            total_events: info.total_events,
            active: info.active,
        }
    }
}

/// Proto message registry for runtime type checking
pub struct ProtoMessageRegistry {
    /// Registered proto types
    registered_types: std::collections::HashSet<&'static str>,
}

impl ProtoMessageRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self {
            registered_types: std::collections::HashSet::new(),
        }
    }
    
    /// Register a proto message type
    pub fn register<T: ProtoMessage>(&mut self) {
        self.registered_types.insert(T::proto_type_name());
    }
    
    /// Check if a proto type is registered
    pub fn is_registered(&self, proto_type: &str) -> bool {
        self.registered_types.contains(proto_type)
    }
    
    /// List all registered proto types
    pub fn list_registered_types(&self) -> Vec<&'static str> {
        self.registered_types.iter().copied().collect()
    }
    
    /// Validate that a proto type is allowed
    pub fn validate_proto_type(&self, proto_type: &str) -> Result<(), EventBusError> {
        if !self.is_registered(proto_type) {
            return Err(EventBusError::contract_violation(format!(
                "Proto type '{}' is not registered. Only registered proto types are allowed.",
                proto_type
            )));
        }
        Ok(())
    }
}

impl Default for ProtoMessageRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for proto-only EventBus configurations
pub struct ProtoEventBusConfig {
    /// Enable strict validation
    pub strict_validation: bool,
    
    /// Maximum quality score threshold (events below this are rejected)
    pub min_quality_score: f64,
    
    /// Enable proto type registry enforcement
    pub enforce_registration: bool,
    
    /// Maximum payload size for proto messages
    pub max_payload_size: usize,
    
    /// Proto message registry
    pub registry: ProtoMessageRegistry,
}

impl Default for ProtoEventBusConfig {
    fn default() -> Self {
        Self {
            strict_validation: true,
            min_quality_score: 0.5, // Minimum 50% quality score
            enforce_registration: true,
            max_payload_size: 1024 * 1024, // 1MB max
            registry: ProtoMessageRegistry::new(),
        }
    }
}

impl ProtoEventBusConfig {
    /// Create a new config with default strict settings
    pub fn strict() -> Self {
        Self {
            strict_validation: true,
            min_quality_score: 0.8, // Higher quality threshold
            enforce_registration: true,
            max_payload_size: 512 * 1024, // 512KB max
            registry: ProtoMessageRegistry::new(),
        }
    }
    
    /// Create a config for testing (more lenient)
    pub fn for_testing() -> Self {
        Self {
            strict_validation: true,
            min_quality_score: 0.0, // Allow any quality for testing
            enforce_registration: false, // Don't enforce registration in tests
            max_payload_size: 10 * 1024 * 1024, // 10MB for testing
            registry: ProtoMessageRegistry::new(),
        }
    }
    
    /// Register a proto message type
    pub fn register_proto_type<T: ProtoMessage>(&mut self) -> &mut Self {
        self.registry.register::<T>();
        self
    }
    
    /// Set minimum quality score
    pub fn min_quality_score(mut self, score: f64) -> Self {
        self.min_quality_score = score.clamp(0.0, 1.0);
        self
    }
    
    /// Set maximum payload size
    pub fn max_payload_size(mut self, size: usize) -> Self {
        self.max_payload_size = size;
        self
    }
    
    /// Enable or disable strict validation
    pub fn strict_validation(mut self, enabled: bool) -> Self {
        self.strict_validation = enabled;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[derive(Clone, prost::Message)]
    pub struct TestProtoMessage {
        #[prost(string, tag = "1")]
        pub content: String,
    }
    
    impl ProtoMessage for TestProtoMessage {
        fn proto_type_name() -> &'static str {
            "test.TestProtoMessage"
        }
    }
    
    #[test]
    fn test_proto_message_registry() {
        let mut registry = ProtoMessageRegistry::new();
        registry.register::<TestProtoMessage>();
        
        assert!(registry.is_registered("test.TestProtoMessage"));
        assert!(!registry.is_registered("unknown.Message"));
        
        let types = registry.list_registered_types();
        assert_eq!(types.len(), 1);
        assert_eq!(types[0], "test.TestProtoMessage");
    }
    
    #[test]
    fn test_proto_eventbus_config() {
        let mut config = ProtoEventBusConfig::default();
        config.register_proto_type::<TestProtoMessage>()
            .min_quality_score = 0.9;
        config.max_payload_size = 2048;
        
        assert_eq!(config.min_quality_score, 0.9);
        assert_eq!(config.max_payload_size, 2048);
        assert!(config.registry.is_registered("test.TestProtoMessage"));
    }
    
    #[test]
    fn test_contract_violation_methods() {
        // These would be implemented by concrete EventBus implementations
        // The trait methods should return contract violation errors
        let error = reject_raw_payload();
        assert!(matches!(error, EventBusError::ContractViolation(_)));
        
        let error = reject_json_payload();
        assert!(matches!(error, EventBusError::ContractViolation(_)));
    }
}