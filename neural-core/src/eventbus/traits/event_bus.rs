//! EventBus trait definition
//!
//! Core trait for event bus implementations.

use async_trait::async_trait;
use super::subscriber::GenericEventSubscriber;

/// Core EventBus trait for asynchronous event messaging
#[async_trait]
pub trait EventBus: Send + Sync {
    /// Event type that this bus handles
    type Event: Send + Sync + Clone;
    
    /// Error type for this bus
    type Error: Send + Sync;

    /// Publish an event to a channel
    ///
    /// # Arguments
    /// * `channel` - The channel name to publish to
    /// * `event` - The event to publish
    ///
    /// # Returns
    /// * `Ok(())` if the event was published successfully
    /// * `Err(Error)` if the publish operation failed
    async fn publish(&self, channel: &str, event: Self::Event) -> Result<(), Self::Error>;

    /// Subscribe to a channel
    ///
    /// # Arguments
    /// * `channel` - The channel name to subscribe to
    ///
    /// # Returns
    /// * `Ok(EventSubscriber)` if subscription was successful
    /// * `Err(Error)` if the subscription failed
    async fn subscribe(
        &self,
        channel: &str,
    ) -> Result<Box<dyn GenericEventSubscriber<Event = Self::Event>>, Self::Error>;

    /// Unsubscribe from a channel
    ///
    /// # Arguments
    /// * `channel` - The channel name to unsubscribe from
    /// * `subscriber_id` - The ID of the subscriber to remove
    ///
    /// # Returns
    /// * `Ok(())` if unsubscription was successful
    /// * `Err(Error)` if the unsubscription failed
    async fn unsubscribe(&self, channel: &str, subscriber_id: &str) -> Result<(), Self::Error>;

    /// List all available channels
    ///
    /// # Returns
    /// * `Ok(Vec<String>)` containing channel names
    /// * `Err(Error)` if the operation failed
    async fn list_channels(&self) -> Result<Vec<String>, Self::Error>;

    /// Get the number of subscribers for a channel
    ///
    /// # Arguments
    /// * `channel` - The channel name to query
    ///
    /// # Returns
    /// * `Ok(usize)` containing the subscriber count
    /// * `Err(Error)` if the operation failed
    async fn channel_subscriber_count(&self, channel: &str) -> Result<usize, Self::Error>;
}
