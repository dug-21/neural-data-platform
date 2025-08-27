//! ⚠️  DEPRECATED Event types - SCHEDULED FOR REMOVAL
//!
//! 🚨 CONTRACT VIOLATION WARNING 🚨
//! These event types contain Vec<u8> payloads which are FORBIDDEN in Phase 4.
//! EventBus implementations MUST REJECT these events with ContractViolation errors.
//! 
//! ✅ MIGRATION PATH:
//! Use proto-only Event from `crate::events::Event` instead.
//! All payloads MUST be protobuf messages, NO EXCEPTIONS.
//!
//! This file exists only for backward compatibility during migration.
//! It will be REMOVED in the next phase.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::collections::HashMap;

/// Unique identifier for events
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(String);

impl EventId {
    /// Create a new unique event ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create an event ID from a string
    pub fn from<S: Into<String>>(id: S) -> Self {
        Self(id.into())
    }

    /// Get the underlying string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for EventId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for EventId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// ⚠️  DEPRECATED: Legacy Event type - Vec<u8> payloads are BANNED in Phase 4
/// 
/// Use `ProtoEvent<T>` instead for proto-only messaging.
/// This struct is kept only for backward compatibility during migration.
// TEMPORARILY REMOVED: #[deprecated] to eliminate warnings during migration
// This struct will be removed in the next phase after migration is complete
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Type of the event (e.g., "MarketData", "TrainingComplete")
    pub event_type: String,
    
    /// ❌ BANNED: Serialized payload - Vec<u8> is NO LONGER SUPPORTED
    /// Use ProtoEvent<T> with strongly-typed protobuf messages instead
    pub payload: Vec<u8>,
    
    /// Additional metadata as key-value pairs
    pub metadata: HashMap<String, String>,
    
    /// Unix timestamp when the event was created
    pub timestamp: i64,
}

impl Event {
    /// ⚠️  DEPRECATED: Create a new event with Vec<u8> payload
    /// 
    /// # Contract Violation Warning
    /// This method creates events with Vec<u8> payloads which are BANNED in Phase 4.
    /// EventBus implementations MUST reject these events with ContractViolation errors.
    /// 
    /// Use `ProtoEvent::new(proto_message)` instead.
    // TEMPORARILY REMOVED: #[deprecated] to eliminate warnings during migration
    // This method will be removed in the next phase after migration is complete
    pub fn new(event_type: String, payload: Vec<u8>) -> Self {
        Self {
            event_type,
            payload,
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
    
    /// Add metadata to the event
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
    
    /// Set custom timestamp
    pub fn with_timestamp(mut self, timestamp: i64) -> Self {
        self.timestamp = timestamp;
        self
    }
}

/// Event envelope containing an event with delivery metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// Unique identifier for this event
    pub event_id: EventId,
    
    /// Channel this event was delivered from
    pub channel: String,
    
    /// The actual event
    pub event: Event,
    
    /// Number of retry attempts
    pub retry_count: u32,
    
    /// Unix timestamp when the event was delivered
    pub delivered_at: i64,
}

impl EventEnvelope {
    /// Create a new event envelope
    pub fn new(event_id: EventId, channel: String, event: Event) -> Self {
        Self {
            event_id,
            channel,
            event,
            retry_count: 0,
            delivered_at: chrono::Utc::now().timestamp(),
        }
    }
    
    /// Increment retry count
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }
}

/// Generic event trait for extensibility (kept for backwards compatibility)
pub trait GenericEvent: Send + Sync + Clone + std::fmt::Debug {
    /// Get the event's unique identifier
    fn id(&self) -> &EventId;
    
    /// Get the timestamp when the event was created
    fn timestamp(&self) -> i64;
    
    /// Get the event type as a string
    fn event_type(&self) -> &str;
}