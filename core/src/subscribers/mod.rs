//! Subscriber module for Event Bus consumers
//!
//! This module provides the Subscriber trait and implementations for
//! consuming events from the EventBus and processing them.
//!
//! # Architecture (DP-012)
//!
//! Subscribers consume RawDataPoint events from the EventBus and process them:
//! - BronzeSubscriber: Batches and writes to Parquet (RawStore)
//! - SilverSubscriber: Transforms and writes to TimescaleDB
//! - ProcessorSubscriber: Wraps processors for alerting
//! - EventNotifier: Publishes lightweight MQTT notifications
//! - SubscriberCoordinator: Manages lifecycle of multiple subscribers
//!
//! # Design Principles
//!
//! - Each subscriber runs in its own tokio task for isolation
//! - Subscribers handle their own buffering and flush timing
//! - Graceful shutdown via CancellationToken
//! - Error handling: log and continue, don't crash

mod bronze;
pub mod coordinator;
mod notifier;
mod processor;
mod silver;

pub use bronze::{BronzeSubscriber, BronzeSubscriberConfig};
pub use coordinator::{CoordinatorHealth, CoordinatorState, SubscriberCoordinator};
pub use notifier::{EventNotification, EventNotifier, EventNotifierConfig, EventNotifierState};
pub use processor::{ProcessorSubscriber, ProcessorSubscriberConfig, ProcessorSubscriberState};
pub use silver::{CatchUpConfig, SilverSubscriber, SilverSubscriberConfig, SubscriberState};

// Re-export NoBronzeReader for use when catch-up is not needed
// Defined in this module (above)

use crate::traits::HealthStatus;
use crate::types::RawDataPoint;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Error type for subscriber operations
#[derive(Debug, thiserror::Error)]
pub enum SubscriberError {
    #[error("Failed to start subscriber: {0}")]
    StartupFailed(String),

    #[error("Failed to stop subscriber: {0}")]
    ShutdownFailed(String),

    #[error("Processing error: {0}")]
    ProcessingError(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Catch-up error: {0}")]
    CatchUpError(String),

    #[error("Transform error: {0}")]
    TransformError(String),

    #[error("Hot reload not supported for this subscriber")]
    HotReloadNotSupported,

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<crate::error::CoreError> for SubscriberError {
    fn from(err: crate::error::CoreError) -> Self {
        SubscriberError::Internal(err.to_string())
    }
}

/// Trait for reading historical data from Bronze layer
///
/// Used by SilverSubscriber for catch-up processing when it needs
/// to recover data from before it started listening to the EventBus.
///
/// # Implementations
/// - ParquetBronzeReader: Reads from Bronze Parquet files
/// - NoBronzeReader: Dummy implementation (no catch-up)
/// - TestBronzeReader: In-memory implementation for testing
#[async_trait]
pub trait BronzeReader: Send + Sync {
    /// Read raw data points since a given timestamp
    ///
    /// # Arguments
    /// * `since` - Only return points with timestamp >= since
    /// * `stream_filter` - Optional stream ID to filter by
    ///
    /// # Returns
    /// Vector of RawDataPoint ordered by timestamp ascending
    async fn read_since(
        &self,
        since: chrono::DateTime<chrono::Utc>,
        stream_filter: Option<&str>,
    ) -> Result<Vec<RawDataPoint>, crate::error::CoreError>;

    /// Get the latest timestamp in Bronze storage
    ///
    /// # Arguments
    /// * `stream_filter` - Optional stream ID to filter by
    ///
    /// # Returns
    /// The most recent timestamp, or None if no data exists
    async fn get_latest_timestamp(
        &self,
        stream_filter: Option<&str>,
    ) -> Result<Option<chrono::DateTime<chrono::Utc>>, crate::error::CoreError>;
}

/// Dummy BronzeReader that never returns data (no catch-up support).
/// Use this when creating SilverSubscriber without catch-up capability.
#[derive(Debug, Clone, Default)]
pub struct NoBronzeReader;

#[async_trait]
impl BronzeReader for NoBronzeReader {
    async fn read_since(
        &self,
        _since: chrono::DateTime<chrono::Utc>,
        _stream_filter: Option<&str>,
    ) -> Result<Vec<RawDataPoint>, crate::error::CoreError> {
        Ok(Vec::new())
    }

    async fn get_latest_timestamp(
        &self,
        _stream_filter: Option<&str>,
    ) -> Result<Option<chrono::DateTime<chrono::Utc>>, crate::error::CoreError> {
        Ok(None)
    }
}

/// Core trait for event bus subscribers
///
/// # Lifecycle
/// 1. Create subscriber with configuration
/// 2. Call `start()` with broadcast receiver
/// 3. Subscriber processes events until `stop()` called
/// 4. On `stop()`, flush buffers and cleanup
///
/// # Error Handling
/// - Subscribers should handle errors internally
/// - Log errors but continue processing
/// - Propagate fatal errors via Result
#[async_trait]
pub trait Subscriber: Send + Sync {
    /// Unique identifier for this subscriber
    ///
    /// Used for:
    /// - Configuration lookup
    /// - Metrics labeling
    /// - Logging context
    fn id(&self) -> &str;

    /// Start consuming from the event bus
    ///
    /// # Arguments
    /// * `receiver` - Broadcast receiver for events
    ///
    /// # Implementation Notes
    /// 1. Use `tokio::select!` for timeout-based flushing
    /// 2. Handle `RecvError::Lagged` by logging and continuing
    /// 3. Exit loop on `RecvError::Closed`
    async fn start(
        &mut self,
        receiver: broadcast::Receiver<Arc<RawDataPoint>>,
    ) -> Result<(), SubscriberError>;

    /// Stop consuming gracefully
    ///
    /// # Implementation Notes
    /// 1. Signal internal tasks to stop
    /// 2. Flush any buffered data
    /// 3. Close connections/resources
    async fn stop(&mut self) -> Result<(), SubscriberError>;

    /// Check if this subscriber processes a given stream
    ///
    /// # Arguments
    /// * `stream_id` - Stream identifier from RawDataPoint.source_id
    ///
    /// # Returns
    /// * `true` - Process this stream
    /// * `false` - Skip this stream
    ///
    /// # Default Behavior
    /// If no stream filter configured, returns true for all streams
    fn accepts_stream(&self, stream_id: &str) -> bool;

    /// Health check for monitoring
    ///
    /// # Returns
    /// HealthStatus indicating subscriber state
    async fn health_check(&self) -> HealthStatus;

    /// Reconfigure subscriber (hot reload)
    ///
    /// # Default Implementation
    /// Returns error indicating hot reload not supported
    async fn reconfigure(&mut self, _config: serde_json::Value) -> Result<(), SubscriberError> {
        Err(SubscriberError::HotReloadNotSupported)
    }
}
