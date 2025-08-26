//! EventBus trait definitions
//!
//! Core traits for the event bus system.

mod event_bus;
mod event_bus_v2;
mod subscriber;

// Use the concrete V2 trait as the primary EventBus trait
pub use event_bus_v2::EventBus;
pub use subscriber::EventSubscriber;

// Keep the generic trait available as GenericEventBus
pub use event_bus::EventBus as GenericEventBus;
