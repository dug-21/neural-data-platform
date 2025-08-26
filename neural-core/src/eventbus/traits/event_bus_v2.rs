//! EventBus V2 trait definition - Concrete implementation
//!
//! Concrete EventBus trait matching the specification requirements.

use async_trait::async_trait;
use crate::eventbus::{
    types::{Event, EventId, SubscriptionConfig, ChannelInfo},
    error::EventBusError,
};
use super::EventSubscriber;

/// Concrete EventBus trait for production use
#[async_trait]
pub trait EventBus: Send + Sync {
    /// Publish an event to a channel
    async fn publish(&self, channel: &str, event: Event) -> Result<EventId, EventBusError>;
    
    /// Publish a batch of events to a channel
    async fn publish_batch(&self, channel: &str, events: Vec<Event>) -> Result<Vec<EventId>, EventBusError>;
    
    /// Subscribe to one or more channels
    async fn subscribe(
        &self,
        channels: &[String],
        config: SubscriptionConfig,
    ) -> Result<Box<dyn EventSubscriber>, EventBusError>;
    
    /// Acknowledge successful event processing
    async fn ack(&self, channel: &str, group: &str, event_id: &EventId) -> Result<(), EventBusError>;
    
    /// Negative acknowledgment for failed processing  
    async fn nack(&self, channel: &str, group: &str, event_id: &EventId) -> Result<(), EventBusError>;
    
    /// Create a consumer group
    async fn create_consumer_group(&self, channel: &str, group: &str) -> Result<(), EventBusError>;
    
    /// Get channel information
    async fn get_channel_info(&self, channel: &str) -> Result<ChannelInfo, EventBusError>;
}