//! EventBus type definitions
//!
//! Core types used throughout the EventBus system.

mod event;
mod config;

pub use event::{Event, EventId, EventEnvelope};
pub use config::{SubscriptionConfig, ChannelInfo, StartPosition};
