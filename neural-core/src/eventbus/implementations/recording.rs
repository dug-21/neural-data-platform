use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::Utc;

use crate::eventbus::{
    traits::{DynamicEventBus, DynamicProtoEventSubscriber, ProtoChannelInfo},
    types::{EventId, SubscriptionConfig},
    error::EventBusError,
};

/// Recording wrapper for EventBus implementations that records all operations for testing
pub struct RecordingEventBus {
    inner: Box<dyn DynamicEventBus>,
    recorded_publishes: Arc<RwLock<Vec<RecordedPublish>>>,
    recorded_subscriptions: Arc<RwLock<Vec<RecordedSubscription>>>,
    recorded_acks: Arc<RwLock<Vec<RecordedAck>>>,
    recording_enabled: Arc<RwLock<bool>>,
}

#[derive(Debug, Clone)]
pub struct RecordedPublish {
    pub timestamp: i64,
    pub channel: String,
    pub proto_type: String,
    pub payload_size: usize,
    pub event_id: EventId,
    pub quality_score: f64,
}

#[derive(Debug, Clone)]
pub struct RecordedSubscription {
    pub timestamp: i64,
    pub channels: Vec<String>,
    pub config: SubscriptionConfig,
    pub proto_type: Option<String>, // For typed subscriptions
}

#[derive(Debug, Clone)]
pub struct RecordedAck {
    pub timestamp: i64,
    pub channel: String,
    pub group: String,
    pub event_id: EventId,
    pub is_ack: bool, // true for ack, false for nack
}

impl RecordingEventBus {
    pub fn new(inner: Box<dyn DynamicEventBus>) -> Self {
        Self {
            inner,
            recorded_publishes: Arc::new(RwLock::new(Vec::new())),
            recorded_subscriptions: Arc::new(RwLock::new(Vec::new())),
            recorded_acks: Arc::new(RwLock::new(Vec::new())),
            recording_enabled: Arc::new(RwLock::new(true)),
        }
    }

    pub async fn get_recorded_publishes(&self) -> Vec<RecordedPublish> {
        self.recorded_publishes.read().await.clone()
    }

    pub async fn get_recorded_subscriptions(&self) -> Vec<RecordedSubscription> {
        self.recorded_subscriptions.read().await.clone()
    }

    pub async fn get_recorded_acks(&self) -> Vec<RecordedAck> {
        self.recorded_acks.read().await.clone()
    }

    pub async fn assert_proto_event_published(&self, channel: &str, proto_type: &str) -> bool {
        let publishes = self.recorded_publishes.read().await;
        publishes.iter().any(|p| p.channel == channel && p.proto_type == proto_type)
    }

    /// Legacy method name compatibility
    pub async fn assert_event_published(&self, channel: &str, proto_type: &str) -> bool {
        self.assert_proto_event_published(channel, proto_type).await
    }

    pub async fn assert_subscription_created(&self, channel: &str) -> bool {
        let subscriptions = self.recorded_subscriptions.read().await;
        subscriptions.iter().any(|s| s.channels.contains(&channel.to_string()))
    }

    pub async fn assert_event_acked(&self, event_id: &EventId) -> bool {
        let acks = self.recorded_acks.read().await;
        acks.iter().any(|a| &a.event_id == event_id && a.is_ack)
    }

    pub async fn clear_recordings(&self) {
        self.recorded_publishes.write().await.clear();
        self.recorded_subscriptions.write().await.clear();
        self.recorded_acks.write().await.clear();
    }

    pub async fn set_recording_enabled(&self, enabled: bool) {
        *self.recording_enabled.write().await = enabled;
    }

    pub async fn get_publish_count(&self, channel: Option<&str>) -> usize {
        let publishes = self.recorded_publishes.read().await;
        match channel {
            Some(ch) => publishes.iter().filter(|p| p.channel == ch).count(),
            None => publishes.len(),
        }
    }

    pub async fn get_last_published_proto_type(&self, channel: &str) -> Option<String> {
        let publishes = self.recorded_publishes.read().await;
        publishes.iter()
            .rev()
            .find(|p| p.channel == channel)
            .map(|p| p.proto_type.clone())
    }

    pub async fn get_publish_count_for_proto_type(&self, channel: &str, proto_type: &str) -> usize {
        let publishes = self.recorded_publishes.read().await;
        publishes.iter()
            .filter(|p| p.channel == channel && p.proto_type == proto_type)
            .count()
    }

    /// Helper method to publish typed proto events (for tests)
    pub async fn publish<T: crate::eventbus::types::ProtoMessage + Default>(
        &self,
        channel: &str,
        event: crate::eventbus::types::ProtoEvent<T>,
    ) -> Result<EventId, EventBusError> {
        // Convert ProtoEvent to ProtoEventEnvelope
        let envelope = crate::eventbus::types::ProtoEventEnvelope::new(
            EventId::new(),
            channel.to_string(),
            event,
        )?;
        
        self.publish_envelope(channel, envelope).await
    }

    /// Helper method to subscribe to typed proto events (for tests)
    pub async fn subscribe<T: crate::eventbus::types::ProtoMessage + Default>(
        &self,
        channels: &[String],
        config: SubscriptionConfig,
    ) -> Result<Box<dyn crate::eventbus::traits::ProtoEventSubscriber<T>>, EventBusError> {
        // For testing, we can create a wrapper around dynamic subscription
        // This is a simplified approach - in production you'd use proper type registration
        Err(EventBusError::not_implemented("Typed subscription not implemented for DynamicEventBus wrapper"))
    }
}

#[async_trait]
impl DynamicEventBus for RecordingEventBus {
    async fn publish_envelope(
        &self,
        channel: &str,
        envelope: crate::eventbus::types::ProtoEventEnvelope,
    ) -> Result<EventId, EventBusError> {
        let event_id = self.inner.publish_envelope(channel, envelope.clone()).await?;
        
        if *self.recording_enabled.read().await {
            let mut publishes = self.recorded_publishes.write().await;
            publishes.push(RecordedPublish {
                timestamp: Utc::now().timestamp(),
                channel: channel.to_string(),
                proto_type: envelope.proto_type.clone(),
                payload_size: envelope.proto_bytes.len(),
                event_id: event_id.clone(),
                quality_score: envelope.quality_score,
            });
        }
        
        Ok(event_id)
    }

    async fn publish_batch_envelopes(
        &self,
        channel: &str,
        envelopes: Vec<crate::eventbus::types::ProtoEventEnvelope>,
    ) -> Result<Vec<EventId>, EventBusError> {
        let event_ids = self.inner.publish_batch_envelopes(channel, envelopes.clone()).await?;
        
        if *self.recording_enabled.read().await {
            let mut publishes = self.recorded_publishes.write().await;
            let timestamp = Utc::now().timestamp();
            
            for (envelope, event_id) in envelopes.iter().zip(event_ids.iter()) {
                publishes.push(RecordedPublish {
                    timestamp,
                    channel: channel.to_string(),
                    proto_type: envelope.proto_type.clone(),
                    payload_size: envelope.proto_bytes.len(),
                    event_id: event_id.clone(),
                    quality_score: envelope.quality_score,
                });
            }
        }
        
        Ok(event_ids)
    }

    // Note: DynamicEventBus doesn't support typed subscriptions
    // Use subscribe_dynamic instead

    async fn subscribe_dynamic(
        &self,
        channels: &[String],
        proto_types: &[&'static str],
        config: SubscriptionConfig,
    ) -> Result<Box<dyn DynamicProtoEventSubscriber>, EventBusError> {
        let subscriber = self.inner.subscribe_dynamic(channels, proto_types, config.clone()).await?;
        
        if *self.recording_enabled.read().await {
            let mut subscriptions = self.recorded_subscriptions.write().await;
            subscriptions.push(RecordedSubscription {
                timestamp: Utc::now().timestamp(),
                channels: channels.to_vec(),
                config,
                proto_type: None, // Dynamic subscription doesn't have single proto type
            });
        }
        
        Ok(subscriber)
    }

    async fn ack(
        &self,
        channel: &str,
        group: &str,
        event_id: &EventId,
    ) -> Result<(), EventBusError> {
        let result = self.inner.ack(channel, group, event_id).await?;
        
        if *self.recording_enabled.read().await {
            let mut acks = self.recorded_acks.write().await;
            acks.push(RecordedAck {
                timestamp: Utc::now().timestamp(),
                channel: channel.to_string(),
                group: group.to_string(),
                event_id: event_id.clone(),
                is_ack: true,
            });
        }
        
        Ok(result)
    }

    async fn nack(
        &self,
        channel: &str,
        group: &str,
        event_id: &EventId,
    ) -> Result<(), EventBusError> {
        let result = self.inner.nack(channel, group, event_id).await?;
        
        if *self.recording_enabled.read().await {
            let mut acks = self.recorded_acks.write().await;
            acks.push(RecordedAck {
                timestamp: Utc::now().timestamp(),
                channel: channel.to_string(),
                group: group.to_string(),
                event_id: event_id.clone(),
                is_ack: false,
            });
        }
        
        Ok(result)
    }

    async fn create_consumer_group(
        &self,
        channel: &str,
        group: &str,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eventbus::implementations::inmemory::InMemoryEventBus;
    use std::collections::HashMap;

    // Mock proto message for testing
    #[derive(Clone, prost::Message)]
    pub struct TestProtoMessage {
        #[prost(string, tag = "1")]
        pub content: String,
        #[prost(int64, tag = "2")]
        pub value: i64,
    }

    impl crate::eventbus::types::ProtoMessage for TestProtoMessage {
        fn proto_type_name() -> &'static str {
            "test.TestProtoMessage"
        }
    }

    #[tokio::test]
    async fn test_recording_captures_proto_publish() {
        let inner: Box<dyn DynamicEventBus> = Box::new(InMemoryEventBus::new());
        let recording_bus = RecordingEventBus::new(inner);
        
        let message = TestProtoMessage {
            content: "test content".to_string(),
            value: 42,
        };
        
        let event = crate::eventbus::types::ProtoEvent::new(message);
        
        let event_id = recording_bus.publish("stream:symbol:TEST", event).await.unwrap();
        
        assert!(recording_bus.assert_proto_event_published("stream:symbol:TEST", "test.TestProtoMessage").await);
        assert_eq!(recording_bus.get_publish_count(Some("stream:symbol:TEST")).await, 1);
        assert_eq!(recording_bus.get_publish_count_for_proto_type("stream:symbol:TEST", "test.TestProtoMessage").await, 1);
        
        let proto_type = recording_bus.get_last_published_proto_type("stream:symbol:TEST").await.unwrap();
        assert_eq!(proto_type, "test.TestProtoMessage");
        
        let publishes = recording_bus.get_recorded_publishes().await;
        assert_eq!(publishes.len(), 1);
        assert_eq!(publishes[0].event_id, event_id);
        assert_eq!(publishes[0].proto_type, "test.TestProtoMessage");
    }

    #[tokio::test]
    async fn test_recording_captures_proto_subscription() {
        let inner: Box<dyn DynamicEventBus> = Box::new(InMemoryEventBus::new());
        let recording_bus = RecordingEventBus::new(inner);
        
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
        
        let _subscriber = recording_bus.subscribe_dynamic(
            &["stream:symbol:TEST".to_string()],
            &["test.TestProtoMessage"],
            config
        ).await.unwrap();
        
        assert!(recording_bus.assert_subscription_created("stream:symbol:TEST").await);
        
        let subscriptions = recording_bus.get_recorded_subscriptions().await;
        assert_eq!(subscriptions.len(), 1);
        assert_eq!(subscriptions[0].channels[0], "stream:symbol:TEST");
        assert_eq!(subscriptions[0].proto_type, Some("test.TestProtoMessage".to_string()));
    }

    #[tokio::test]
    async fn test_recording_clear_and_disable() {
        let inner: Box<dyn DynamicEventBus> = Box::new(InMemoryEventBus::new());
        let recording_bus = RecordingEventBus::new(inner);
        
        let message = TestProtoMessage {
            content: "test content".to_string(),
            value: 42,
        };
        let event = crate::eventbus::types::ProtoEvent::new(message);
        
        // Record some events
        recording_bus.publish("stream:symbol:TEST", event.clone()).await.unwrap();
        assert_eq!(recording_bus.get_publish_count(None).await, 1);
        
        // Clear recordings
        recording_bus.clear_recordings().await;
        assert_eq!(recording_bus.get_publish_count(None).await, 0);
        
        // Disable recording
        recording_bus.set_recording_enabled(false).await;
        recording_bus.publish("stream:symbol:TEST", event).await.unwrap();
        assert_eq!(recording_bus.get_publish_count(None).await, 0);
    }
}