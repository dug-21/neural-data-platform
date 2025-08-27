//! Proto-only EventBus trait definition - Phase 4 enforcement
//!
//! CRITICAL: This trait REJECTS all Vec<u8> and JSON payloads.
//!           ONLY protobuf messages are accepted.

use async_trait::async_trait;
use crate::eventbus::{
    error::EventBusError,
    types::{
        ProtoMessage, ProtoEvent, EventId,
        SubscriptionConfig, reject_raw_payload, reject_json_payload
    },
};

use super::super::traits::{
    ProtoEventSubscriber, DynamicProtoEventSubscriber, ProtoChannelInfo
};

/// Proto-only EventBus trait - ZERO tolerance for non-proto messages
#[async_trait]
pub trait EventBus: Send + Sync {
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
    async fn publish<T: ProtoMessage + Default>(
        &self,
        channel: &str,
        event: ProtoEvent<T>,
    ) -> Result<EventId, EventBusError>;
    
    /// Publish a batch of proto messages (type-safe)
    async fn publish_batch<T: ProtoMessage + Default>(
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
    async fn subscribe<T: ProtoMessage + Default>(
        &self,
        channels: &[String],
        config: SubscriptionConfig,
    ) -> Result<Box<dyn ProtoEventSubscriber<T>>, EventBusError>;
    
    /// Subscribe to multiple proto message types on same channel
    async fn subscribe_dynamic(
        &self,
        channels: &[String],
        proto_types: &[&'static str],
        config: SubscriptionConfig,
    ) -> Result<Box<dyn DynamicProtoEventSubscriber>, EventBusError>;
    
    /// Acknowledge successful proto event processing
    async fn ack(
        &self, 
        channel: &str, 
        group: &str, 
        event_id: &EventId
    ) -> Result<(), EventBusError>;
    
    /// Negative acknowledgment for failed proto event processing  
    async fn nack(
        &self, 
        channel: &str, 
        group: &str, 
        event_id: &EventId
    ) -> Result<(), EventBusError>;
    
    /// Create a consumer group for proto events
    async fn create_consumer_group(
        &self, 
        channel: &str, 
        group: &str
    ) -> Result<(), EventBusError>;
    
    /// Get channel information (proto-aware)
    async fn get_channel_info(&self, channel: &str) -> Result<ProtoChannelInfo, EventBusError>;
    
    /// List all proto message types seen on a channel
    async fn list_proto_types_on_channel(&self, channel: &str) -> Result<Vec<String>, EventBusError>;

    /// List all available channels
    async fn list_channels(&self) -> Result<Vec<String>, EventBusError>;

    /// Get the number of subscribers for a channel
    async fn channel_subscriber_count(&self, channel: &str) -> Result<usize, EventBusError>;

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