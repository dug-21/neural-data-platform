//! EventBus module - Asynchronous event messaging system
//!
//! This module provides a channel-based event bus implementation for
//! asynchronous message passing between components in the Neural Trader system.

// Module declarations
pub mod traits;
pub mod types;
pub mod error;
pub mod implementations;
pub mod controllers;

// Re-exports for convenience
pub use error::EventBusError;
pub use traits::{EventBus, EventSubscriber};
pub use types::{Event, EventId, EventEnvelope, SubscriptionConfig, ChannelInfo, StartPosition};

// Implementation re-exports
pub use implementations::{
    inmemory::InMemoryEventBus,
    recording::RecordingEventBus,
    redis::RedisEventBus,
};

// Result type alias
pub type Result<T> = std::result::Result<T, EventBusError>;
