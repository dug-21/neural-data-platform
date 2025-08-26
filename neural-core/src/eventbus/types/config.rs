//! Configuration types for EventBus
//!
//! Types for configuring EventBus behavior and channel management.

use serde::{Deserialize, Serialize};
use std::time::Duration;
use super::EventId;

/// Starting position for consuming messages from a channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StartPosition {
    /// Start consuming from the beginning of the stream
    Beginning,
    
    /// Start consuming only new messages (default)
    End,
    
    /// Start consuming from after a specific event ID
    After(EventId),
    
    /// Start consuming from a specific timestamp
    FromTimestamp(i64),
}

/// Configuration for event subscriptions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionConfig {
    /// Name of the consumer group for load balancing
    pub group_name: String,
    
    /// Unique name for this consumer within the group
    pub consumer_name: String,
    
    /// Starting position for consuming messages
    pub start_position: StartPosition,
    
    /// Maximum number of messages to receive in a batch
    pub batch_size: usize,
    
    /// Timeout for blocking read operations (milliseconds)
    pub block_timeout_ms: u64,
    
    /// Timeout for acknowledging messages (milliseconds)
    pub ack_timeout_ms: u64,
    
    /// Maximum number of events to buffer for this subscription (legacy)
    pub buffer_size: usize,
    
    /// Timeout for receive operations (legacy)
    pub receive_timeout: Option<Duration>,
    
    /// Whether to enable persistent delivery (legacy)
    pub persistent: bool,
    
    /// Priority level for this subscription (higher = more priority, legacy)
    pub priority: u8,
}

impl Default for SubscriptionConfig {
    fn default() -> Self {
        Self {
            group_name: "default-group".to_string(),
            consumer_name: format!("consumer-{}", uuid::Uuid::new_v4()),
            start_position: StartPosition::End,
            batch_size: 10,
            block_timeout_ms: 5000,
            ack_timeout_ms: 30000,
            buffer_size: 1024,
            receive_timeout: Some(Duration::from_secs(30)),
            persistent: false,
            priority: 0,
        }
    }
}

impl SubscriptionConfig {
    /// Create a new subscription configuration
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Set the buffer size
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self.batch_size = size;
        self
    }
    
    /// Set the receive timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.receive_timeout = Some(timeout);
        self.block_timeout_ms = timeout.as_millis() as u64;
        self
    }
    
    /// Disable receive timeout
    pub fn without_timeout(mut self) -> Self {
        self.receive_timeout = None;
        self
    }
    
    /// Enable persistent delivery
    pub fn persistent(mut self) -> Self {
        self.persistent = true;
        self
    }
    
    /// Set the priority level
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }
}

/// Information about a channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    /// Channel name
    pub channel_name: String,
    
    /// Channel name (legacy field)
    pub name: String,
    
    /// Number of messages in the channel
    pub message_count: u64,
    
    /// Consumer groups for this channel
    pub consumer_groups: Vec<String>,
    
    /// Last event ID in the channel
    pub last_event_id: Option<EventId>,
    
    /// Channel creation timestamp (unix timestamp)
    pub created_at: i64,
    
    /// Number of active subscribers (legacy)
    pub subscriber_count: usize,
    
    /// Total number of events published to this channel (legacy)
    pub total_events: u64,
    
    /// Whether the channel is active (has subscribers, legacy)
    pub active: bool,
}

impl ChannelInfo {
    /// Create new channel info
    pub fn new(name: String) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            channel_name: name.clone(),
            name,
            message_count: 0,
            consumer_groups: Vec::new(),
            last_event_id: None,
            created_at: now,
            subscriber_count: 0,
            total_events: 0,
            active: false,
        }
    }
    
    /// Check if the channel is active
    pub fn is_active(&self) -> bool {
        self.active && self.subscriber_count > 0
    }
    
    /// Update subscriber count
    pub fn set_subscriber_count(&mut self, count: usize) {
        self.subscriber_count = count;
        self.active = count > 0;
    }
    
    /// Increment event counter
    pub fn increment_events(&mut self) {
        self.total_events += 1;
        self.message_count += 1;
    }
}