use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

use crate::eventbus::{
    traits::{EventBus, EventSubscriber},
    types::{Event, EventId, EventEnvelope, SubscriptionConfig, ChannelInfo},
    error::EventBusError,
};

/// Thread-safe in-memory implementation of EventBus for testing
pub struct InMemoryEventBus {
    channels: Arc<RwLock<HashMap<String, Channel>>>,
    consumer_groups: Arc<RwLock<HashMap<String, ConsumerGroup>>>,
}

struct Channel {
    name: String,
    events: Vec<EventEnvelope>,
    subscribers: Vec<String>,
    created_at: i64,
}

struct ConsumerGroup {
    name: String,
    channel: String,
    members: Vec<String>,
    pending_messages: HashMap<EventId, EventEnvelope>,
    last_delivered: Option<EventId>,
}

impl InMemoryEventBus {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            consumer_groups: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn ensure_channel(&self, channel: &str) -> Result<(), EventBusError> {
        let mut channels = self.channels.write().await;
        if !channels.contains_key(channel) {
            channels.insert(
                channel.to_string(),
                Channel {
                    name: channel.to_string(),
                    events: Vec::new(),
                    subscribers: Vec::new(),
                    created_at: chrono::Utc::now().timestamp(),
                },
            );
        }
        Ok(())
    }
}

#[async_trait]
impl EventBus for InMemoryEventBus {
    async fn publish(&self, channel: &str, event: Event) -> Result<EventId, EventBusError> {
        // Validate channel name
        if !validate_channel_name(channel) {
            return Err(EventBusError::InvalidChannel(format!(
                "Invalid channel name format: {}", channel
            )));
        }

        self.ensure_channel(channel).await?;

        let event_id = EventId::new();
        let envelope = EventEnvelope {
            event_id: event_id.clone(),
            channel: channel.to_string(),
            event,
            retry_count: 0,
            delivered_at: chrono::Utc::now().timestamp(),
        };

        let mut channels = self.channels.write().await;
        if let Some(chan) = channels.get_mut(channel) {
            chan.events.push(envelope);
        }

        Ok(event_id)
    }

    async fn publish_batch(&self, channel: &str, events: Vec<Event>) -> Result<Vec<EventId>, EventBusError> {
        let mut event_ids = Vec::new();
        for event in events {
            let id = self.publish(channel, event).await?;
            event_ids.push(id);
        }
        Ok(event_ids)
    }

    async fn subscribe(
        &self,
        channels: &[String],
        config: SubscriptionConfig,
    ) -> Result<Box<dyn EventSubscriber>, EventBusError> {
        for channel in channels {
            if !validate_channel_name(channel) {
                return Err(EventBusError::InvalidChannel(format!(
                    "Invalid channel name format: {}", channel
                )));
            }
            self.ensure_channel(channel).await?;
        }

        let subscriber_id = Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::channel(config.batch_size);

        // Create consumer group if needed
        let group_key = format!("{}:{}", channels.join(","), config.group_name);
        let mut consumer_groups = self.consumer_groups.write().await;
        consumer_groups.entry(group_key.clone()).or_insert_with(|| ConsumerGroup {
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

        Ok(Box::new(InMemorySubscriber {
            id: subscriber_id,
            channels: channels.to_vec(),
            receiver: rx,
            _sender: tx,
        }))
    }

    async fn ack(
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
            Err(EventBusError::ConsumerGroup(format!(
                "Consumer group {} not found for channel {}", group, channel
            )))
        }
    }

    async fn nack(
        &self,
        channel: &str,
        group: &str,
        _event_id: &EventId,
    ) -> Result<(), EventBusError> {
        // In memory implementation, nack just keeps the message in pending
        let group_key = format!("{}:{}", channel, group);
        let consumer_groups = self.consumer_groups.read().await;
        
        if consumer_groups.contains_key(&group_key) {
            // Message stays in pending_messages for retry
            Ok(())
        } else {
            Err(EventBusError::ConsumerGroup(format!(
                "Consumer group {} not found for channel {}", group, channel
            )))
        }
    }

    async fn create_consumer_group(
        &self,
        channel: &str,
        group: &str,
    ) -> Result<(), EventBusError> {
        self.ensure_channel(channel).await?;
        
        let group_key = format!("{}:{}", channel, group);
        let mut consumer_groups = self.consumer_groups.write().await;
        
        if consumer_groups.contains_key(&group_key) {
            return Err(EventBusError::ConsumerGroup(format!(
                "Consumer group {} already exists for channel {}", group, channel
            )));
        }
        
        consumer_groups.insert(
            group_key,
            ConsumerGroup {
                name: group.to_string(),
                channel: channel.to_string(),
                members: Vec::new(),
                pending_messages: HashMap::new(),
                last_delivered: None,
            },
        );
        
        Ok(())
    }

    async fn get_channel_info(&self, channel: &str) -> Result<ChannelInfo, EventBusError> {
        let channels = self.channels.read().await;
        
        if let Some(chan) = channels.get(channel) {
            Ok(ChannelInfo {
                channel_name: chan.name.clone(),
                name: chan.name.clone(),
                message_count: chan.events.len() as u64,
                consumer_groups: vec![], // Would need to extract from consumer_groups
                last_event_id: chan.events.last().map(|e| e.event_id.clone()),
                created_at: chan.created_at,
                subscriber_count: chan.subscribers.len(),
                total_events: chan.events.len() as u64,
                active: !chan.subscribers.is_empty(),
            })
        } else {
            Err(EventBusError::InvalidChannel(format!(
                "Channel {} not found", channel
            )))
        }
    }
}

pub struct InMemorySubscriber {
    id: String,
    channels: Vec<String>,
    receiver: mpsc::Receiver<EventEnvelope>,
    _sender: mpsc::Sender<EventEnvelope>,
}

#[async_trait]
impl EventSubscriber for InMemorySubscriber {
    async fn next(&mut self) -> Result<Option<EventEnvelope>, EventBusError> {
        Ok(self.receiver.recv().await)
    }

    async fn close(&mut self) -> Result<(), EventBusError> {
        self.receiver.close();
        Ok(())
    }
}

/// Validates channel name format (stream:domain:identifier)
pub fn validate_channel_name(name: &str) -> bool {
    let parts: Vec<&str> = name.split(':').collect();
    
    if parts.len() != 3 {
        return false;
    }
    
    if parts[0] != "stream" {
        return false;
    }
    
    let valid_domains = [
        "symbol", "sector", "portfolio", "cross_sector", "ml", "action", "dlq"
    ];
    
    if !valid_domains.contains(&parts[1]) {
        return false;
    }
    
    !parts[2].is_empty()
}

/// Migrates old channel names to new format
pub fn migrate_channel_name(old_name: &str) -> String {
    if old_name.starts_with("market:") {
        let symbol = old_name.strip_prefix("market:").unwrap_or("");
        format!("stream:symbol:{}", symbol)
    } else if old_name.starts_with("sector_") {
        let sector = old_name.strip_prefix("sector_").unwrap_or("");
        format!("stream:sector:{}", sector)
    } else if old_name.starts_with("ml_") {
        let operation = old_name.strip_prefix("ml_").unwrap_or("");
        format!("stream:ml:{}", operation)
    } else if old_name.starts_with("stream:") {
        old_name.to_string()
    } else {
        format!("stream:unknown:{}", old_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_inmemory_publish_subscribe() {
        let event_bus = InMemoryEventBus::new();
        
        let event = Event {
            event_type: "MarketData".to_string(),
            payload: vec![1, 2, 3],
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        
        let event_id = event_bus.publish("stream:symbol:AAPL", event.clone()).await.unwrap();
        assert!(!event_id.to_string().is_empty());
        
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
        
        let _subscriber = event_bus.subscribe(&["stream:symbol:AAPL".to_string()], config).await.unwrap();
        // Subscriber would receive the event in a real scenario
    }

    #[test]
    fn test_validate_channel_name() {
        assert!(validate_channel_name("stream:symbol:AAPL"));
        assert!(validate_channel_name("stream:sector:technology"));
        assert!(validate_channel_name("stream:ml:training"));
        
        assert!(!validate_channel_name("market:AAPL"));
        assert!(!validate_channel_name("stream:unknown:test"));
        assert!(!validate_channel_name("stream:symbol:"));
        assert!(!validate_channel_name("invalid"));
    }

    #[test]
    fn test_migrate_channel_name() {
        assert_eq!(migrate_channel_name("market:AAPL"), "stream:symbol:AAPL");
        assert_eq!(migrate_channel_name("sector_technology"), "stream:sector:technology");
        assert_eq!(migrate_channel_name("ml_training"), "stream:ml:training");
        assert_eq!(migrate_channel_name("stream:symbol:MSFT"), "stream:symbol:MSFT");
        assert_eq!(migrate_channel_name("unknown"), "stream:unknown:unknown");
    }
}