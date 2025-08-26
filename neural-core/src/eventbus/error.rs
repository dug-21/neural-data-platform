//! EventBus error types
//!
//! Comprehensive error handling for the EventBus system.

use thiserror::Error;

/// EventBus error types
#[derive(Debug, Error)]
pub enum EventBusError {
    /// Invalid channel format or name
    #[error("Invalid channel: {0}")]
    InvalidChannel(String),
    
    /// Channel not found
    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    /// Subscriber not found
    #[error("Subscriber not found: {0}")]
    SubscriberNotFound(String),

    /// Channel already exists
    #[error("Channel already exists: {0}")]
    ChannelAlreadyExists(String),

    /// Consumer group error
    #[error("Consumer group error: {0}")]
    ConsumerGroup(String),
    
    /// Backend error (Redis, network, etc.)
    #[error("Backend error: {0}")]
    Backend(String),
    
    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    /// Backpressure throttling
    #[error("Throttled")]
    Throttled,
    
    /// Timeout error
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Send operation failed
    #[error("Send failed: {0}")]
    SendFailed(String),

    /// Internal system error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl EventBusError {
    /// Create a new InvalidChannel error
    pub fn invalid_channel<S: Into<String>>(channel: S) -> Self {
        Self::InvalidChannel(channel.into())
    }

    /// Create a new ChannelNotFound error
    pub fn channel_not_found<S: Into<String>>(channel: S) -> Self {
        Self::ChannelNotFound(channel.into())
    }

    /// Create a new SubscriberNotFound error
    pub fn subscriber_not_found<S: Into<String>>(subscriber_id: S) -> Self {
        Self::SubscriberNotFound(subscriber_id.into())
    }

    /// Create a new ConsumerGroup error
    pub fn consumer_group<S: Into<String>>(message: S) -> Self {
        Self::ConsumerGroup(message.into())
    }
    
    /// Create a new Backend error
    pub fn backend<S: Into<String>>(message: S) -> Self {
        Self::Backend(message.into())
    }
    
    /// Create a new Serialization error
    pub fn serialization<S: Into<String>>(message: S) -> Self {
        Self::Serialization(message.into())
    }
    
    /// Create a new Timeout error
    pub fn timeout<S: Into<String>>(message: S) -> Self {
        Self::Timeout(message.into())
    }

    /// Create a new Internal error
    pub fn internal<S: Into<String>>(message: S) -> Self {
        Self::Internal(message.into())
    }

    /// Create a new SendFailed error
    pub fn send_failed<S: Into<String>>(message: S) -> Self {
        Self::SendFailed(message.into())
    }
}