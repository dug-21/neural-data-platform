//! EventBus V2 trait definition - DEPRECATED
//!
//! DEPRECATED: Use ProtoEventBus instead. This trait is maintained for legacy compatibility only.
//! Vec<u8> payloads are BANNED in Phase 4 - use proto messages only.

use async_trait::async_trait;
use crate::eventbus::{
    types::{EventId, SubscriptionConfig, ChannelInfo, reject_raw_payload},
    error::EventBusError,
};
use super::subscriber::EventSubscriber;

/// DEPRECATED: EventBus V2 trait - Use ProtoEventBus instead
/// 
/// This trait is deprecated in Phase 4. All Vec<u8> methods return contract violations.
#[deprecated(note = "Use ProtoEventBus instead. Vec<u8> payloads are banned.")]
#[async_trait]
pub trait EventBus: Send + Sync {
    /// DEPRECATED: Use publish_proto instead - this method returns contract violations
    async fn publish_raw(&self, _channel: &str, _payload: Vec<u8>) -> Result<EventId, EventBusError> {
        Err(reject_raw_payload())
    }
    
    /// DEPRECATED: Use publish_proto_batch instead - this method returns contract violations
    async fn publish_batch_raw(&self, _channel: &str, _payloads: Vec<Vec<u8>>) -> Result<Vec<EventId>, EventBusError> {
        Err(reject_raw_payload())
    }
    
    /// DEPRECATED: Use subscribe_proto instead
    async fn subscribe(
        &self,
        _channels: &[String],
        _config: SubscriptionConfig,
    ) -> Result<Box<dyn EventSubscriber>, EventBusError> {
        Err(reject_raw_payload())
    }
    
    /// Acknowledge successful event processing (still valid)
    async fn ack(&self, channel: &str, group: &str, event_id: &EventId) -> Result<(), EventBusError>;
    
    /// Negative acknowledgment for failed processing (still valid)
    async fn nack(&self, channel: &str, group: &str, event_id: &EventId) -> Result<(), EventBusError>;
    
    /// Create a consumer group (still valid)
    async fn create_consumer_group(&self, channel: &str, group: &str) -> Result<(), EventBusError>;
    
    /// Get channel information (still valid)
    async fn get_channel_info(&self, channel: &str) -> Result<ChannelInfo, EventBusError>;
}