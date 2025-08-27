//! EventBus trait definitions - Phase 4 Proto-Only
//!
//! CRITICAL: All traits now enforce proto-only messaging.

mod event_bus;
mod event_bus_v2;
mod subscriber;
mod proto_event_bus;
mod dynamic_event_bus;

// PRIMARY TRAITS: Proto-only EventBus is now the main EventBus
pub use event_bus::EventBus;
pub use proto_event_bus::{
    ProtoEventBus, ProtoEventSubscriber, DynamicProtoEventSubscriber,
    ProtoChannelInfo, ProtoMessageRegistry, ProtoEventBusConfig,
};

// Dynamic traits for dyn compatibility
pub use dynamic_event_bus::{DynamicEventBus, EventBusWrapper};

// Legacy traits - keep accessible for backward compatibility
pub use event_bus_v2::EventBus as LegacyEventBusV2;
pub use subscriber::{EventSubscriber, GenericEventSubscriber};

// Keep the generic trait available as GenericEventBus (DEPRECATED)
#[deprecated(since = "0.1.0", note = "Use proto-only EventBus instead.")]
pub use event_bus::EventBus as GenericEventBus;