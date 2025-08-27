//! Dynamic EventBus trait for dyn compatibility
//!
//! This trait provides dynamic dispatch for EventBus operations
//! by using type erasure with ProtoEventEnvelope.

use async_trait::async_trait;
use crate::eventbus::{
    error::EventBusError,
    types::{ProtoEventEnvelope, EventId, SubscriptionConfig},
};

use super::{DynamicProtoEventSubscriber, ProtoChannelInfo};

/// Dynamic EventBus trait that can be used as a trait object
/// 
/// This trait avoids generic parameters to enable dyn compatibility.
/// All proto messages are handled through type-erased ProtoEventEnvelope.
#[async_trait]
pub trait DynamicEventBus: Send + Sync {
    /// Publish a proto message using type-erased envelope
    async fn publish_envelope(
        &self,
        channel: &str,
        envelope: ProtoEventEnvelope,
    ) -> Result<EventId, EventBusError>;
    
    /// Publish a batch of proto envelopes
    async fn publish_batch_envelopes(
        &self,
        channel: &str,
        envelopes: Vec<ProtoEventEnvelope>,
    ) -> Result<Vec<EventId>, EventBusError>;
    
    /// Subscribe to multiple proto message types dynamically
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
}

/// Wrapper to make any EventBus trait dyn-compatible
pub struct EventBusWrapper<T> {
    inner: T,
}

impl<T> EventBusWrapper<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
    
    pub fn inner(&self) -> &T {
        &self.inner
    }
}

#[async_trait]
impl<T: super::EventBus> DynamicEventBus for EventBusWrapper<T> {
    async fn publish_envelope(
        &self,
        channel: &str,
        envelope: ProtoEventEnvelope,
    ) -> Result<EventId, EventBusError> {
        // For generic EventBus, we need to call the specific type method
        // This is a limitation - we can't truly type-erase without knowing the type
        // In practice, you would register type handlers or use a registry
        Err(EventBusError::NotImplemented(
            "publish_envelope requires type registration for generic EventBus".to_string()
        ))
    }
    
    async fn publish_batch_envelopes(
        &self,
        _channel: &str,
        _envelopes: Vec<ProtoEventEnvelope>,
    ) -> Result<Vec<EventId>, EventBusError> {
        Err(EventBusError::NotImplemented(
            "publish_batch_envelopes requires type registration for generic EventBus".to_string()
        ))
    }
    
    async fn subscribe_dynamic(
        &self,
        channels: &[String],
        proto_types: &[&'static str],
        config: SubscriptionConfig,
    ) -> Result<Box<dyn DynamicProtoEventSubscriber>, EventBusError> {
        self.inner.subscribe_dynamic(channels, proto_types, config).await
    }
    
    async fn ack(
        &self, 
        channel: &str, 
        group: &str, 
        event_id: &EventId
    ) -> Result<(), EventBusError> {
        self.inner.ack(channel, group, event_id).await
    }
    
    async fn nack(
        &self, 
        channel: &str, 
        group: &str, 
        event_id: &EventId
    ) -> Result<(), EventBusError> {
        self.inner.nack(channel, group, event_id).await
    }
    
    async fn create_consumer_group(
        &self, 
        channel: &str, 
        group: &str
    ) -> Result<(), EventBusError> {
        self.inner.create_consumer_group(channel, group).await
    }
    
    async fn get_channel_info(&self, channel: &str) -> Result<ProtoChannelInfo, EventBusError> {
        self.inner.get_channel_info(channel).await
    }
    
    async fn list_proto_types_on_channel(&self, channel: &str) -> Result<Vec<String>, EventBusError> {
        self.inner.list_proto_types_on_channel(channel).await
    }

    async fn list_channels(&self) -> Result<Vec<String>, EventBusError> {
        self.inner.list_channels().await
    }

    async fn channel_subscriber_count(&self, channel: &str) -> Result<usize, EventBusError> {
        self.inner.channel_subscriber_count(channel).await
    }
}