//! EventSubscriber trait definition
//!
//! Trait for event subscribers that can receive events from channels.

use async_trait::async_trait;
use crate::eventbus::{
    types::EventEnvelope,
    error::EventBusError,
};

/// Concrete trait for event subscribers
#[async_trait]
pub trait EventSubscriber: Send + Sync {
    /// Get the next event from the subscription
    ///
    /// # Returns
    /// * `Ok(Some(EventEnvelope))` if an event is available
    /// * `Ok(None)` if no events are available
    /// * `Err(EventBusError)` if an error occurred
    async fn next(&mut self) -> Result<Option<EventEnvelope>, EventBusError>;
    
    /// Close the subscription
    ///
    /// This method should clean up any resources associated with the subscription.
    async fn close(&mut self) -> Result<(), EventBusError>;
}

/// Generic EventSubscriber trait for backwards compatibility
#[async_trait]
pub trait GenericEventSubscriber: Send + Sync {
    /// The event type this subscriber receives
    type Event: Send + Sync;

    /// Get the unique identifier for this subscriber
    ///
    /// # Returns
    /// The subscriber's unique ID as a string slice
    fn id(&self) -> &str;

    /// Receive the next event from the subscribed channel
    ///
    /// # Returns
    /// * `Some(Event)` if an event was received
    /// * `None` if no event is available or the channel is closed
    async fn receive(&mut self) -> Option<Self::Event>;

    /// Close the subscriber and clean up resources
    ///
    /// This method should be called when the subscriber is no longer needed
    /// to ensure proper cleanup of resources.
    async fn close(&mut self);
}