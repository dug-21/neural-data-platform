//! EventBus type definitions
//!
//! Core types used throughout the EventBus system.

mod event;
mod config;
mod proto_event;

// Legacy types (DEPRECATED - Phase 4 proto-only enforcement eliminates these)
pub use event::{Event, EventId, EventEnvelope};
pub use config::{SubscriptionConfig, ChannelInfo, StartPosition};

// Proto-only types (Phase 4 enforcement - MANDATORY for all new code)
pub use proto_event::{
    ProtoMessage, ProtoEvent, ProtoEventEnvelope, DynamicProtoEvent,
    reject_raw_payload, reject_json_payload,
};
