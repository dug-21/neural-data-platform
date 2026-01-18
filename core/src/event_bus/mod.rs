//! Event Bus for multi-consumer event broadcasting (dp-012)
//!
//! This module implements the event bus using tokio::broadcast for in-process
//! multi-consumer event distribution. Key design decisions from ADR-012-001:
//!
//! - Uses `tokio::broadcast` instead of external broker (no network overhead)
//! - Events are `Arc<RawDataPoint>` for zero-copy sharing
//! - Default capacity: 10,000 events with configurable overflow strategy
//! - Subscribers that lag too far behind will lose oldest messages
//!
//! # Architecture
//!
//! ```text
//! Sources → mpsc → IngestionCoordinator → EventBus → [Subscriber 1]
//!                                                   → [Subscriber 2]
//!                                                   → [Subscriber N]
//! ```
//!
//! # Example
//!
//! ```ignore
//! let config = EventBusConfig::default();
//! let event_bus = EventBus::new(config);
//!
//! // Get a receiver for subscribing
//! let mut receiver = event_bus.subscribe();
//!
//! // Publish an event
//! let point = Arc::new(RawDataPoint::new("source-1", json!({"value": 42})));
//! event_bus.publish(point)?;
//!
//! // Receive the event
//! let received = receiver.recv().await?;
//! ```

use crate::types::RawDataPoint;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::{debug, warn};

/// Default channel capacity (10,000 events)
pub const DEFAULT_CAPACITY: usize = 10_000;

/// Default lag warning threshold
pub const DEFAULT_LAG_WARNING_THRESHOLD: usize = 1_000;

/// Event bus errors
#[derive(Error, Debug, Clone)]
pub enum EventBusError {
    /// Failed to send event (all receivers dropped)
    #[error("Failed to send event: no active subscribers")]
    NoSubscribers,

    /// Channel is full and event was dropped (with drop strategy)
    #[error("Channel full, event dropped (lag: {lag} events)")]
    ChannelFull { lag: u64 },

    /// Receiver lagged and lost messages
    #[error("Receiver lagged, lost {lost} messages")]
    ReceiverLagged { lost: u64 },

    /// Internal error
    #[error("Internal event bus error: {0}")]
    Internal(String),
}

/// Strategy for handling channel overflow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverflowStrategy {
    /// Drop oldest events when buffer is full (default behavior of broadcast)
    #[default]
    DropOldest,
    /// Block sender until space is available (not recommended for real-time)
    Block,
}

/// Configuration for the event bus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBusConfig {
    /// Channel capacity (number of events to buffer)
    #[serde(default = "default_capacity")]
    pub capacity: usize,

    /// Strategy for handling overflow
    #[serde(default)]
    pub overflow_strategy: OverflowStrategy,

    /// Threshold for logging lag warnings
    #[serde(default = "default_lag_warning_threshold")]
    pub lag_warning_threshold: usize,
}

fn default_capacity() -> usize {
    DEFAULT_CAPACITY
}

fn default_lag_warning_threshold() -> usize {
    DEFAULT_LAG_WARNING_THRESHOLD
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            capacity: DEFAULT_CAPACITY,
            overflow_strategy: OverflowStrategy::DropOldest,
            lag_warning_threshold: DEFAULT_LAG_WARNING_THRESHOLD,
        }
    }
}

/// Metrics for monitoring event bus health
#[derive(Debug, Clone, Default)]
pub struct EventBusMetrics {
    /// Total events published since startup
    pub events_published: u64,
    /// Current number of active subscribers
    pub subscribers_count: usize,
    /// Number of events that lagged (were dropped due to slow subscribers)
    pub lag_events: u64,
    /// Number of failed publish attempts (no subscribers)
    pub failed_publishes: u64,
}

/// Event bus for multi-consumer event broadcasting
///
/// Uses `tokio::broadcast` internally for zero-copy event sharing.
/// All subscribers receive every event published after they subscribe.
pub struct EventBus {
    sender: broadcast::Sender<Arc<RawDataPoint>>,
    config: EventBusConfig,
    // Metrics counters
    events_published: AtomicU64,
    lag_events: AtomicU64,
    failed_publishes: AtomicU64,
}

impl EventBus {
    /// Create a new event bus with the given configuration
    pub fn new(config: EventBusConfig) -> Self {
        let (sender, _) = broadcast::channel(config.capacity);
        debug!(
            capacity = config.capacity,
            overflow_strategy = ?config.overflow_strategy,
            "EventBus created"
        );

        Self {
            sender,
            config,
            events_published: AtomicU64::new(0),
            lag_events: AtomicU64::new(0),
            failed_publishes: AtomicU64::new(0),
        }
    }

    /// Create a new event bus with default configuration
    pub fn with_defaults() -> Self {
        Self::new(EventBusConfig::default())
    }

    /// Subscribe to the event bus
    ///
    /// Returns a receiver that will receive all events published after this call.
    /// Each subscriber gets its own independent receiver.
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<RawDataPoint>> {
        let receiver = self.sender.subscribe();
        debug!(
            subscribers = self.sender.receiver_count(),
            "New subscriber added"
        );
        receiver
    }

    /// Publish an event to all subscribers
    ///
    /// # Errors
    ///
    /// Returns `EventBusError::NoSubscribers` if there are no active subscribers.
    pub fn publish(&self, event: Arc<RawDataPoint>) -> Result<usize, EventBusError> {
        match self.sender.send(event) {
            Ok(receiver_count) => {
                self.events_published.fetch_add(1, Ordering::Relaxed);
                debug!(
                    receivers = receiver_count,
                    total_published = self.events_published.load(Ordering::Relaxed),
                    "Event published"
                );
                Ok(receiver_count)
            }
            Err(_) => {
                self.failed_publishes.fetch_add(1, Ordering::Relaxed);
                warn!("Failed to publish event: no active subscribers");
                Err(EventBusError::NoSubscribers)
            }
        }
    }

    /// Get current number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Get current event bus metrics
    pub fn metrics(&self) -> EventBusMetrics {
        EventBusMetrics {
            events_published: self.events_published.load(Ordering::Relaxed),
            subscribers_count: self.sender.receiver_count(),
            lag_events: self.lag_events.load(Ordering::Relaxed),
            failed_publishes: self.failed_publishes.load(Ordering::Relaxed),
        }
    }

    /// Record a lag event (called by subscribers when they detect lag)
    pub fn record_lag(&self, lost_count: u64) {
        self.lag_events.fetch_add(lost_count, Ordering::Relaxed);
        if lost_count as usize >= self.config.lag_warning_threshold {
            warn!(
                lost = lost_count,
                threshold = self.config.lag_warning_threshold,
                "Subscriber lag exceeded warning threshold"
            );
        }
    }

    /// Get the configuration
    pub fn config(&self) -> &EventBusConfig {
        &self.config
    }
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBus")
            .field("config", &self.config)
            .field("subscriber_count", &self.sender.receiver_count())
            .field(
                "events_published",
                &self.events_published.load(Ordering::Relaxed),
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::time::{timeout, Duration};

    // ========== TDD CYCLE 1: EventBusConfig ==========

    #[test]
    fn test_config_default_values() {
        let config = EventBusConfig::default();

        assert_eq!(config.capacity, DEFAULT_CAPACITY);
        assert_eq!(config.overflow_strategy, OverflowStrategy::DropOldest);
        assert_eq!(config.lag_warning_threshold, DEFAULT_LAG_WARNING_THRESHOLD);
    }

    #[test]
    fn test_config_custom_values() {
        let config = EventBusConfig {
            capacity: 5_000,
            overflow_strategy: OverflowStrategy::Block,
            lag_warning_threshold: 500,
        };

        assert_eq!(config.capacity, 5_000);
        assert_eq!(config.overflow_strategy, OverflowStrategy::Block);
        assert_eq!(config.lag_warning_threshold, 500);
    }

    #[test]
    fn test_config_serde_round_trip() {
        let config = EventBusConfig {
            capacity: 8_000,
            overflow_strategy: OverflowStrategy::DropOldest,
            lag_warning_threshold: 800,
        };

        let json_str = serde_json::to_string(&config).unwrap();
        let restored: EventBusConfig = serde_json::from_str(&json_str).unwrap();

        assert_eq!(restored.capacity, config.capacity);
        assert_eq!(restored.overflow_strategy, config.overflow_strategy);
        assert_eq!(restored.lag_warning_threshold, config.lag_warning_threshold);
    }

    #[test]
    fn test_config_deserialize_with_defaults() {
        // JSON with only capacity specified
        let json_str = r#"{"capacity": 2000}"#;
        let config: EventBusConfig = serde_json::from_str(json_str).unwrap();

        assert_eq!(config.capacity, 2000);
        assert_eq!(config.overflow_strategy, OverflowStrategy::DropOldest);
        assert_eq!(config.lag_warning_threshold, DEFAULT_LAG_WARNING_THRESHOLD);
    }

    // ========== TDD CYCLE 2: EventBus Construction ==========

    #[test]
    fn test_event_bus_creation_with_config() {
        let config = EventBusConfig {
            capacity: 1000,
            overflow_strategy: OverflowStrategy::DropOldest,
            lag_warning_threshold: 100,
        };
        let bus = EventBus::new(config.clone());

        assert_eq!(bus.config().capacity, 1000);
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn test_event_bus_creation_with_defaults() {
        let bus = EventBus::with_defaults();

        assert_eq!(bus.config().capacity, DEFAULT_CAPACITY);
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn test_event_bus_debug_impl() {
        let bus = EventBus::with_defaults();
        let debug_str = format!("{:?}", bus);

        assert!(debug_str.contains("EventBus"));
        assert!(debug_str.contains("subscriber_count"));
    }

    // ========== TDD CYCLE 3: Subscribe ==========

    #[test]
    fn test_subscribe_increments_count() {
        let bus = EventBus::with_defaults();
        assert_eq!(bus.subscriber_count(), 0);

        let _receiver1 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 1);

        let _receiver2 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 2);
    }

    #[test]
    fn test_subscribe_count_decrements_on_drop() {
        let bus = EventBus::with_defaults();

        {
            let _receiver = bus.subscribe();
            assert_eq!(bus.subscriber_count(), 1);
        }
        // Receiver dropped
        assert_eq!(bus.subscriber_count(), 0);
    }

    // ========== TDD CYCLE 4: Publish ==========

    #[tokio::test]
    async fn test_publish_with_subscriber() {
        let bus = EventBus::with_defaults();
        let mut receiver = bus.subscribe();

        let point = Arc::new(RawDataPoint::new("test-source", json!({"value": 42})));
        let result = bus.publish(point.clone());

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1); // 1 receiver

        let received = timeout(Duration::from_millis(100), receiver.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(received.source_id, "test-source");
        assert_eq!(received.raw_payload["value"], 42);
    }

    #[tokio::test]
    async fn test_publish_without_subscriber_returns_error() {
        let bus = EventBus::with_defaults();

        let point = Arc::new(RawDataPoint::new("test-source", json!({"value": 42})));
        let result = bus.publish(point);

        assert!(result.is_err());
        match result.unwrap_err() {
            EventBusError::NoSubscribers => {}
            e => panic!("Expected NoSubscribers error, got {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_publish_to_multiple_subscribers() {
        let bus = EventBus::with_defaults();
        let mut receiver1 = bus.subscribe();
        let mut receiver2 = bus.subscribe();
        let mut receiver3 = bus.subscribe();

        let point = Arc::new(RawDataPoint::new("test-source", json!({"value": 42})));
        let result = bus.publish(point.clone());

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3); // 3 receivers

        // All receivers should get the event
        let r1 = timeout(Duration::from_millis(100), receiver1.recv())
            .await
            .unwrap()
            .unwrap();
        let r2 = timeout(Duration::from_millis(100), receiver2.recv())
            .await
            .unwrap()
            .unwrap();
        let r3 = timeout(Duration::from_millis(100), receiver3.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(r1.source_id, "test-source");
        assert_eq!(r2.source_id, "test-source");
        assert_eq!(r3.source_id, "test-source");
    }

    #[tokio::test]
    async fn test_publish_preserves_arc_data() {
        let bus = EventBus::with_defaults();
        let mut receiver = bus.subscribe();

        let original = RawDataPoint::new(
            "complex-source",
            json!({
                "nested": {"deep": {"value": 123}},
                "array": [1, 2, 3],
                "string": "hello"
            }),
        )
        .with_ndp_id("test-id")
        .with_context(json!({"meta": "data"}));

        let point = Arc::new(original.clone());
        bus.publish(point).unwrap();

        let received = timeout(Duration::from_millis(100), receiver.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(received.source_id, original.source_id);
        assert_eq!(received.ndp_id, original.ndp_id);
        assert_eq!(received.raw_payload, original.raw_payload);
        assert_eq!(received.context, original.context);
    }

    // ========== TDD CYCLE 5: Metrics ==========

    #[tokio::test]
    async fn test_metrics_initial_values() {
        let bus = EventBus::with_defaults();
        let metrics = bus.metrics();

        assert_eq!(metrics.events_published, 0);
        assert_eq!(metrics.subscribers_count, 0);
        assert_eq!(metrics.lag_events, 0);
        assert_eq!(metrics.failed_publishes, 0);
    }

    #[tokio::test]
    async fn test_metrics_after_publish() {
        let bus = EventBus::with_defaults();
        let _receiver = bus.subscribe();

        for i in 0..5 {
            let point = Arc::new(RawDataPoint::new("test-source", json!({"value": i})));
            bus.publish(point).unwrap();
        }

        let metrics = bus.metrics();
        assert_eq!(metrics.events_published, 5);
        assert_eq!(metrics.subscribers_count, 1);
    }

    #[tokio::test]
    async fn test_metrics_failed_publishes() {
        let bus = EventBus::with_defaults();
        // No subscribers

        for _ in 0..3 {
            let point = Arc::new(RawDataPoint::new("test-source", json!({"value": 1})));
            let _ = bus.publish(point); // Ignore error
        }

        let metrics = bus.metrics();
        assert_eq!(metrics.failed_publishes, 3);
    }

    #[tokio::test]
    async fn test_record_lag() {
        let bus = EventBus::with_defaults();

        bus.record_lag(100);
        bus.record_lag(200);

        let metrics = bus.metrics();
        assert_eq!(metrics.lag_events, 300);
    }

    // ========== TDD CYCLE 6: Error Types ==========

    #[test]
    fn test_error_display() {
        let no_subs = EventBusError::NoSubscribers;
        assert!(no_subs.to_string().contains("no active subscribers"));

        let channel_full = EventBusError::ChannelFull { lag: 100 };
        assert!(channel_full.to_string().contains("Channel full"));
        assert!(channel_full.to_string().contains("100"));

        let lagged = EventBusError::ReceiverLagged { lost: 50 };
        assert!(lagged.to_string().contains("lagged"));
        assert!(lagged.to_string().contains("50"));
    }

    #[test]
    fn test_error_clone() {
        let err = EventBusError::ChannelFull { lag: 100 };
        let cloned = err.clone();

        match cloned {
            EventBusError::ChannelFull { lag } => assert_eq!(lag, 100),
            _ => panic!("Wrong error type after clone"),
        }
    }

    // ========== TDD CYCLE 7: Integration Scenarios ==========

    #[tokio::test]
    async fn test_late_subscriber_only_gets_new_events() {
        let bus = EventBus::with_defaults();
        let mut early_receiver = bus.subscribe();

        // Publish first event
        let point1 = Arc::new(RawDataPoint::new("source", json!({"seq": 1})));
        bus.publish(point1).unwrap();

        // Late subscriber joins
        let mut late_receiver = bus.subscribe();

        // Publish second event
        let point2 = Arc::new(RawDataPoint::new("source", json!({"seq": 2})));
        bus.publish(point2).unwrap();

        // Early receiver gets both
        let r1 = timeout(Duration::from_millis(100), early_receiver.recv())
            .await
            .unwrap()
            .unwrap();
        let r2 = timeout(Duration::from_millis(100), early_receiver.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(r1.raw_payload["seq"], 1);
        assert_eq!(r2.raw_payload["seq"], 2);

        // Late receiver only gets second event
        let late_r = timeout(Duration::from_millis(100), late_receiver.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(late_r.raw_payload["seq"], 2);
    }

    #[tokio::test]
    async fn test_subscriber_unsubscribe_doesnt_affect_others() {
        let bus = EventBus::with_defaults();
        let mut receiver1 = bus.subscribe();
        let receiver2 = bus.subscribe();
        let mut receiver3 = bus.subscribe();

        assert_eq!(bus.subscriber_count(), 3);

        // Drop receiver2
        drop(receiver2);
        assert_eq!(bus.subscriber_count(), 2);

        // Publish should still work
        let point = Arc::new(RawDataPoint::new("source", json!({"value": 1})));
        let result = bus.publish(point);
        assert_eq!(result.unwrap(), 2);

        // Remaining receivers should get the event
        let _ = timeout(Duration::from_millis(100), receiver1.recv())
            .await
            .unwrap()
            .unwrap();
        let _ = timeout(Duration::from_millis(100), receiver3.recv())
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn test_high_throughput_scenario() {
        let config = EventBusConfig {
            capacity: 1000,
            overflow_strategy: OverflowStrategy::DropOldest,
            lag_warning_threshold: 100,
        };
        let bus = EventBus::new(config);
        let mut receiver = bus.subscribe();

        // Publish many events quickly
        for i in 0..100 {
            let point = Arc::new(RawDataPoint::new("source", json!({"seq": i})));
            bus.publish(point).unwrap();
        }

        let metrics = bus.metrics();
        assert_eq!(metrics.events_published, 100);

        // Receive all events
        for i in 0..100 {
            let received = timeout(Duration::from_millis(100), receiver.recv())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(received.raw_payload["seq"], i);
        }
    }
}
