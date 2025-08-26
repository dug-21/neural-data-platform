use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::Utc;

use crate::eventbus::{
    traits::{EventBus, EventSubscriber},
    types::{Event, EventId, SubscriptionConfig, ChannelInfo},
    error::EventBusError,
};

/// Recording wrapper for EventBus implementations that records all operations for testing
pub struct RecordingEventBus {
    inner: Box<dyn EventBus>,
    recorded_publishes: Arc<RwLock<Vec<RecordedPublish>>>,
    recorded_subscriptions: Arc<RwLock<Vec<RecordedSubscription>>>,
    recorded_acks: Arc<RwLock<Vec<RecordedAck>>>,
    recording_enabled: Arc<RwLock<bool>>,
}

#[derive(Debug, Clone)]
pub struct RecordedPublish {
    pub timestamp: i64,
    pub channel: String,
    pub event: Event,
    pub event_id: EventId,
}

#[derive(Debug, Clone)]
pub struct RecordedSubscription {
    pub timestamp: i64,
    pub channels: Vec<String>,
    pub config: SubscriptionConfig,
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
    pub fn new(inner: Box<dyn EventBus>) -> Self {
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

    pub async fn assert_event_published(&self, channel: &str, event_type: &str) -> bool {
        let publishes = self.recorded_publishes.read().await;
        publishes.iter().any(|p| p.channel == channel && p.event.event_type == event_type)
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

    pub async fn get_last_published_event(&self, channel: &str) -> Option<Event> {
        let publishes = self.recorded_publishes.read().await;
        publishes.iter()
            .rev()
            .find(|p| p.channel == channel)
            .map(|p| p.event.clone())
    }
}

#[async_trait]
impl EventBus for RecordingEventBus {
    async fn publish(&self, channel: &str, event: Event) -> Result<EventId, EventBusError> {
        let result = self.inner.publish(channel, event.clone()).await?;
        
        if *self.recording_enabled.read().await {
            let mut publishes = self.recorded_publishes.write().await;
            publishes.push(RecordedPublish {
                timestamp: Utc::now().timestamp(),
                channel: channel.to_string(),
                event,
                event_id: result.clone(),
            });
        }
        
        Ok(result)
    }

    async fn publish_batch(&self, channel: &str, events: Vec<Event>) -> Result<Vec<EventId>, EventBusError> {
        let result = self.inner.publish_batch(channel, events.clone()).await?;
        
        if *self.recording_enabled.read().await {
            let mut publishes = self.recorded_publishes.write().await;
            for (event, event_id) in events.into_iter().zip(result.iter()) {
                publishes.push(RecordedPublish {
                    timestamp: Utc::now().timestamp(),
                    channel: channel.to_string(),
                    event,
                    event_id: event_id.clone(),
                });
            }
        }
        
        Ok(result)
    }

    async fn subscribe(
        &self,
        channels: &[String],
        config: SubscriptionConfig,
    ) -> Result<Box<dyn EventSubscriber>, EventBusError> {
        let result = self.inner.subscribe(channels, config.clone()).await?;
        
        if *self.recording_enabled.read().await {
            let mut subscriptions = self.recorded_subscriptions.write().await;
            subscriptions.push(RecordedSubscription {
                timestamp: Utc::now().timestamp(),
                channels: channels.to_vec(),
                config,
            });
        }
        
        Ok(result)
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

    async fn get_channel_info(&self, channel: &str) -> Result<ChannelInfo, EventBusError> {
        self.inner.get_channel_info(channel).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eventbus::implementations::inmemory::InMemoryEventBus;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_recording_captures_publish() {
        let inner = Box::new(InMemoryEventBus::new());
        let recording_bus = RecordingEventBus::new(inner);
        
        let event = Event {
            event_type: "TestEvent".to_string(),
            payload: vec![1, 2, 3],
            metadata: HashMap::new(),
            timestamp: Utc::now().timestamp(),
        };
        
        let event_id = recording_bus.publish("stream:symbol:TEST", event.clone()).await.unwrap();
        
        assert!(recording_bus.assert_event_published("stream:symbol:TEST", "TestEvent").await);
        assert_eq!(recording_bus.get_publish_count(Some("stream:symbol:TEST")).await, 1);
        
        let last_event = recording_bus.get_last_published_event("stream:symbol:TEST").await.unwrap();
        assert_eq!(last_event.event_type, "TestEvent");
        
        let publishes = recording_bus.get_recorded_publishes().await;
        assert_eq!(publishes.len(), 1);
        assert_eq!(publishes[0].event_id, event_id);
    }

    #[tokio::test]
    async fn test_recording_captures_subscription() {
        let inner = Box::new(InMemoryEventBus::new());
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
        
        let _subscriber = recording_bus.subscribe(
            &["stream:symbol:TEST".to_string()],
            config
        ).await.unwrap();
        
        assert!(recording_bus.assert_subscription_created("stream:symbol:TEST").await);
        
        let subscriptions = recording_bus.get_recorded_subscriptions().await;
        assert_eq!(subscriptions.len(), 1);
        assert_eq!(subscriptions[0].channels[0], "stream:symbol:TEST");
    }

    #[tokio::test]
    async fn test_recording_clear_and_disable() {
        let inner = Box::new(InMemoryEventBus::new());
        let recording_bus = RecordingEventBus::new(inner);
        
        let event = Event {
            event_type: "TestEvent".to_string(),
            payload: vec![1, 2, 3],
            metadata: HashMap::new(),
            timestamp: Utc::now().timestamp(),
        };
        
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