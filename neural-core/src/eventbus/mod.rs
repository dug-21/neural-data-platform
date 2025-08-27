//! EventBus module - Proto-Only Asynchronous Event Messaging System
//!
//! This module provides a proto-only event bus implementation for
//! asynchronous message passing between components in the Neural Trader system.
//!
//! # Phase 4 Proto-Only Enforcement
//! 
//! ⚠️  **BREAKING CHANGE**: Vec<u8> and JSON payloads are NO LONGER SUPPORTED.
//!     All EventBus implementations MUST use protobuf messages only.
//!     Legacy types are deprecated and will be rejected with ContractViolation errors.
//!
//! ## Migration Guide
//! - Replace old `Event` with `ProtoEvent<T>`
//! - Replace generic EventBus usage with proto-only EventBus
//! - Use Data-Staging service to convert JSON to proto messages
//! - Update all subscribers to use `ProtoEventSubscriber<T>`

// Module declarations
pub mod traits;
pub mod types;
pub mod error;
pub mod implementations;
pub mod controllers;
pub mod proto_messages;
// pub mod proto_implementations;

// Test modules
#[cfg(test)]
pub mod tests;

// Re-exports for convenience
pub use error::EventBusError;

// PRIMARY EXPORTS: Proto-only types are now the main EventBus API
pub use traits::{
    EventBus, ProtoEventBus, ProtoEventSubscriber, DynamicProtoEventSubscriber,
    ProtoChannelInfo, ProtoMessageRegistry, ProtoEventBusConfig,
};
pub use types::{
    ProtoMessage, ProtoEvent, ProtoEventEnvelope, DynamicProtoEvent,
    EventId, SubscriptionConfig, reject_raw_payload, reject_json_payload,
};

// Proto-only implementation exports
pub use implementations::{InMemoryEventBus, ProtoInMemoryEventBus};

// Sample proto message implementations
pub use proto_messages::*;

// Legacy exports for backward compatibility (to avoid compilation errors) - DEPRECATED  
// WARNING: Event struct is deprecated but still exported for backward compatibility
pub use traits::{EventSubscriber, GenericEventSubscriber, LegacyEventBusV2};
pub use types::{Event, EventEnvelope, ChannelInfo, StartPosition};

// Result type alias
pub type Result<T> = std::result::Result<T, EventBusError>;