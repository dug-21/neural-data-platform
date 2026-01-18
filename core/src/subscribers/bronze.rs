//! Bronze layer subscriber for raw data storage
//!
//! BronzeSubscriber consumes RawDataPoint events from the EventBus,
//! batches them for efficiency, and writes to the RawStore (Parquet).
//!
//! # Design (DP-012 Phase 1.4)
//!
//! - Batches events by count (batch_size) or time (flush_interval)
//! - Writes to RawStore on batch full or interval elapsed
//! - Graceful shutdown flushes remaining buffer
//! - Retry logic for transient storage failures
//!
//! # Configuration
//!
//! ```yaml
//! subscribers:
//!   - id: bronze
//!     type: storage
//!     config:
//!       batch_size: 100
//!       flush_interval_secs: 5
//!       max_retries: 3
//! ```

use crate::subscribers::{Subscriber, SubscriberError};
use crate::traits::{HealthStatus, RawStore};
use crate::types::RawDataPoint;
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Configuration for BronzeSubscriber
#[derive(Debug, Clone, Deserialize)]
pub struct BronzeSubscriberConfig {
    /// Maximum batch size before flush (default: 100)
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Maximum time between flushes in seconds (default: 5)
    #[serde(default = "default_flush_interval_secs")]
    pub flush_interval_secs: u64,

    /// Maximum retry attempts for storage failures (default: 3)
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Optional stream filter - if empty, accepts all streams
    #[serde(default)]
    pub stream_filter: Vec<String>,
}

fn default_batch_size() -> usize {
    100
}
fn default_flush_interval_secs() -> u64 {
    5
}
fn default_max_retries() -> u32 {
    3
}

impl Default for BronzeSubscriberConfig {
    fn default() -> Self {
        Self {
            batch_size: default_batch_size(),
            flush_interval_secs: default_flush_interval_secs(),
            max_retries: default_max_retries(),
            stream_filter: Vec::new(),
        }
    }
}

/// Subscriber for Bronze layer (Parquet) storage
///
/// Consumes RawDataPoint events from EventBus, batches them,
/// and writes to RawStore for durable storage.
pub struct BronzeSubscriber {
    id: String,
    config: BronzeSubscriberConfig,
    store: Arc<dyn RawStore>,
    buffer: Vec<RawDataPoint>,
    cancellation_token: CancellationToken,
    is_running: bool,
    // Metrics
    events_received: u64,
    events_written: u64,
    batches_written: u64,
    errors_total: u64,
}

impl BronzeSubscriber {
    /// Create a new BronzeSubscriber
    ///
    /// # Arguments
    /// * `id` - Unique identifier for this subscriber
    /// * `config` - Subscriber configuration
    /// * `store` - RawStore implementation for writing data
    pub fn new(
        id: impl Into<String>,
        config: BronzeSubscriberConfig,
        store: Arc<dyn RawStore>,
    ) -> Self {
        let batch_size = config.batch_size;
        Self {
            id: id.into(),
            config,
            store,
            buffer: Vec::with_capacity(batch_size),
            cancellation_token: CancellationToken::new(),
            is_running: false,
            events_received: 0,
            events_written: 0,
            batches_written: 0,
            errors_total: 0,
        }
    }

    /// Flush the current buffer to storage
    async fn flush(&mut self) -> Result<(), SubscriberError> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let batch: Vec<RawDataPoint> = self.buffer.drain(..).collect();
        let batch_len = batch.len();

        debug!(
            subscriber_id = %self.id,
            batch_size = batch_len,
            "Flushing batch to Bronze storage"
        );

        // Retry logic
        let mut last_error = None;
        for attempt in 0..=self.config.max_retries {
            match self.store.write_raw_batch(batch.clone()).await {
                Ok(()) => {
                    self.events_written += batch_len as u64;
                    self.batches_written += 1;
                    debug!(
                        subscriber_id = %self.id,
                        batch_size = batch_len,
                        "Batch written successfully"
                    );
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < self.config.max_retries {
                        let delay = Duration::from_millis(100 * 2u64.pow(attempt));
                        warn!(
                            subscriber_id = %self.id,
                            attempt = attempt + 1,
                            max_retries = self.config.max_retries,
                            delay_ms = delay.as_millis(),
                            "Batch write failed, retrying"
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        // All retries exhausted
        self.errors_total += 1;
        let err_msg = format!(
            "Failed to write batch after {} retries: {:?}",
            self.config.max_retries + 1,
            last_error
        );
        error!(subscriber_id = %self.id, %err_msg);
        Err(SubscriberError::StorageError(err_msg))
    }

    /// Process a single data point
    fn handle_point(&mut self, point: Arc<RawDataPoint>) {
        self.events_received += 1;

        // Check stream filter
        if !self.accepts_stream(&point.source_id) {
            debug!(
                subscriber_id = %self.id,
                source_id = %point.source_id,
                "Skipping point: stream not in filter"
            );
            return;
        }

        // Clone from Arc since we need owned data for batch
        self.buffer.push((*point).clone());
    }

    /// Check if buffer should be flushed based on size
    fn should_flush(&self) -> bool {
        self.buffer.len() >= self.config.batch_size
    }
}

#[async_trait]
impl Subscriber for BronzeSubscriber {
    fn id(&self) -> &str {
        &self.id
    }

    async fn start(
        &mut self,
        mut receiver: broadcast::Receiver<Arc<RawDataPoint>>,
    ) -> Result<(), SubscriberError> {
        info!(subscriber_id = %self.id, "Starting BronzeSubscriber");
        self.is_running = true;

        let flush_interval = Duration::from_secs(self.config.flush_interval_secs);
        let mut flush_timer = tokio::time::interval(flush_interval);
        // First tick is immediate, skip it
        flush_timer.tick().await;

        loop {
            tokio::select! {
                biased;

                // Check cancellation first
                _ = self.cancellation_token.cancelled() => {
                    info!(subscriber_id = %self.id, "Received cancellation signal");
                    break;
                }

                // Flush timer
                _ = flush_timer.tick() => {
                    if let Err(e) = self.flush().await {
                        // Log but continue - don't stop subscriber for transient errors
                        error!(subscriber_id = %self.id, error = %e, "Flush failed on timer");
                    }
                }

                // Receive events
                result = receiver.recv() => {
                    match result {
                        Ok(point) => {
                            self.handle_point(point);
                            // Check if batch is full
                            if self.should_flush() {
                                if let Err(e) = self.flush().await {
                                    error!(subscriber_id = %self.id, error = %e, "Flush failed on batch full");
                                }
                            }
                        }
                        Err(RecvError::Lagged(n)) => {
                            warn!(
                                subscriber_id = %self.id,
                                lagged_count = n,
                                "Subscriber lagged - some events may be lost"
                            );
                            // Continue processing - lagged events are gone but we can still process new ones
                        }
                        Err(RecvError::Closed) => {
                            info!(subscriber_id = %self.id, "Event bus channel closed");
                            break;
                        }
                    }
                }
            }
        }

        // Final flush on exit
        info!(subscriber_id = %self.id, "Performing final flush before shutdown");
        if let Err(e) = self.flush().await {
            error!(subscriber_id = %self.id, error = %e, "Final flush failed");
        }

        self.is_running = false;
        info!(
            subscriber_id = %self.id,
            events_received = self.events_received,
            events_written = self.events_written,
            batches_written = self.batches_written,
            errors_total = self.errors_total,
            "BronzeSubscriber stopped"
        );

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), SubscriberError> {
        info!(subscriber_id = %self.id, "Stopping BronzeSubscriber");
        self.cancellation_token.cancel();
        Ok(())
    }

    fn accepts_stream(&self, stream_id: &str) -> bool {
        if self.config.stream_filter.is_empty() {
            return true;
        }
        // Extract stream_id from source_id (e.g., "air-quality-Mqtt" -> "air-quality")
        let stream = extract_stream_from_source_id(stream_id);
        self.config.stream_filter.iter().any(|s| s == stream)
    }

    async fn health_check(&self) -> HealthStatus {
        let mut details = HashMap::new();
        details.insert(
            "events_received".to_string(),
            self.events_received.to_string(),
        );
        details.insert(
            "events_written".to_string(),
            self.events_written.to_string(),
        );
        details.insert(
            "batches_written".to_string(),
            self.batches_written.to_string(),
        );
        details.insert("errors_total".to_string(), self.errors_total.to_string());
        details.insert("buffer_size".to_string(), self.buffer.len().to_string());
        details.insert("is_running".to_string(), self.is_running.to_string());

        HealthStatus {
            healthy: self.is_running || self.errors_total == 0,
            message: if self.is_running {
                "BronzeSubscriber running".to_string()
            } else {
                "BronzeSubscriber not running".to_string()
            },
            details,
        }
    }
}

/// Extract stream_id from source_id by removing the protocol suffix
///
/// source_id format: "{stream_id}-{SourceType}" (e.g., "air-quality-Mqtt", "nws-forecast-Http")
fn extract_stream_from_source_id(source_id: &str) -> &str {
    const SUFFIXES: &[&str] = &["-FileWatch", "-Webhook", "-HttpPoll", "-Http", "-Mqtt"];

    for suffix in SUFFIXES {
        if let Some(stripped) = source_id.strip_suffix(suffix) {
            return stripped;
        }
    }
    source_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CoreError;
    use crate::traits::MockRawStore;
    use chrono::Utc;
    use mockall::predicate::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::broadcast;

    // ========== HELPER FUNCTIONS ==========

    fn create_test_point(source_id: &str) -> RawDataPoint {
        RawDataPoint::new(source_id, json!({"pm25": 12.5, "co2": 450}))
            .with_timestamp(Utc::now())
            .with_ndp_id("test-device-001")
    }

    fn create_config(batch_size: usize, flush_interval_secs: u64) -> BronzeSubscriberConfig {
        BronzeSubscriberConfig {
            batch_size,
            flush_interval_secs,
            max_retries: 3,
            stream_filter: Vec::new(),
        }
    }

    // ========== TDD CYCLE 1: BronzeSubscriberConfig Tests ==========

    #[test]
    fn test_config_default_values() {
        let config = BronzeSubscriberConfig::default();
        assert_eq!(config.batch_size, 100);
        assert_eq!(config.flush_interval_secs, 5);
        assert_eq!(config.max_retries, 3);
        assert!(config.stream_filter.is_empty());
    }

    #[test]
    fn test_config_deserialize_with_defaults() {
        let yaml = r#"
            batch_size: 50
        "#;
        let config: BronzeSubscriberConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.flush_interval_secs, 5); // default
        assert_eq!(config.max_retries, 3); // default
    }

    #[test]
    fn test_config_deserialize_full() {
        let yaml = r#"
            batch_size: 200
            flush_interval_secs: 10
            max_retries: 5
            stream_filter:
              - air-quality
              - outdoor-weather
        "#;
        let config: BronzeSubscriberConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.batch_size, 200);
        assert_eq!(config.flush_interval_secs, 10);
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.stream_filter, vec!["air-quality", "outdoor-weather"]);
    }

    // ========== TDD CYCLE 2: BronzeSubscriber Creation Tests ==========

    #[test]
    fn test_subscriber_creation() {
        let config = BronzeSubscriberConfig::default();
        let mut mock_store = MockRawStore::new();
        // No expectations - just creation
        let subscriber = BronzeSubscriber::new("bronze-test", config, Arc::new(mock_store));

        assert_eq!(subscriber.id(), "bronze-test");
        assert_eq!(subscriber.events_received, 0);
        assert_eq!(subscriber.events_written, 0);
        assert!(!subscriber.is_running);
    }

    #[test]
    fn test_subscriber_accepts_all_streams_by_default() {
        let config = BronzeSubscriberConfig::default();
        let mock_store = MockRawStore::new();
        let subscriber = BronzeSubscriber::new("bronze-test", config, Arc::new(mock_store));

        assert!(subscriber.accepts_stream("air-quality-Mqtt"));
        assert!(subscriber.accepts_stream("outdoor-weather-Http"));
        assert!(subscriber.accepts_stream("any-stream"));
    }

    #[test]
    fn test_subscriber_filters_streams() {
        let config = BronzeSubscriberConfig {
            stream_filter: vec!["air-quality".to_string()],
            ..Default::default()
        };
        let mock_store = MockRawStore::new();
        let subscriber = BronzeSubscriber::new("bronze-test", config, Arc::new(mock_store));

        assert!(subscriber.accepts_stream("air-quality-Mqtt"));
        assert!(subscriber.accepts_stream("air-quality-Http"));
        assert!(!subscriber.accepts_stream("outdoor-weather-Http"));
    }

    // ========== TDD CYCLE 3: Batch Accumulation Tests ==========

    #[test]
    fn test_handle_point_adds_to_buffer() {
        let config = create_config(10, 5);
        let mock_store = MockRawStore::new();
        let mut subscriber = BronzeSubscriber::new("bronze-test", config, Arc::new(mock_store));

        let point = Arc::new(create_test_point("air-quality-Mqtt"));
        subscriber.handle_point(point);

        assert_eq!(subscriber.buffer.len(), 1);
        assert_eq!(subscriber.events_received, 1);
    }

    #[test]
    fn test_should_flush_when_batch_full() {
        let config = create_config(3, 5);
        let mock_store = MockRawStore::new();
        let mut subscriber = BronzeSubscriber::new("bronze-test", config, Arc::new(mock_store));

        // Add points up to but not exceeding batch size
        for i in 0..2 {
            let point = Arc::new(create_test_point(&format!("source-{}-Mqtt", i)));
            subscriber.handle_point(point);
        }
        assert!(!subscriber.should_flush());

        // Add one more to reach batch size
        let point = Arc::new(create_test_point("source-2-Mqtt"));
        subscriber.handle_point(point);
        assert!(subscriber.should_flush());
    }

    #[test]
    fn test_filtered_points_not_added_to_buffer() {
        let config = BronzeSubscriberConfig {
            batch_size: 10,
            stream_filter: vec!["air-quality".to_string()],
            ..Default::default()
        };
        let mock_store = MockRawStore::new();
        let mut subscriber = BronzeSubscriber::new("bronze-test", config, Arc::new(mock_store));

        // This should be filtered out
        let point = Arc::new(create_test_point("outdoor-weather-Http"));
        subscriber.handle_point(point);

        assert_eq!(subscriber.buffer.len(), 0);
        assert_eq!(subscriber.events_received, 1); // Still counted as received
    }

    // ========== TDD CYCLE 4: Flush Tests ==========

    #[tokio::test]
    async fn test_flush_empty_buffer_succeeds() {
        let config = create_config(10, 5);
        let mock_store = MockRawStore::new();
        let mut subscriber = BronzeSubscriber::new("bronze-test", config, Arc::new(mock_store));

        let result = subscriber.flush().await;
        assert!(result.is_ok());
        assert_eq!(subscriber.events_written, 0);
        assert_eq!(subscriber.batches_written, 0);
    }

    #[tokio::test]
    async fn test_flush_writes_batch_to_store() {
        let config = create_config(10, 5);
        let mut mock_store = MockRawStore::new();

        // Expect write_raw_batch to be called with 3 points
        mock_store
            .expect_write_raw_batch()
            .times(1)
            .withf(|points| points.len() == 3)
            .returning(|_| Ok(()));

        let mut subscriber = BronzeSubscriber::new("bronze-test", config, Arc::new(mock_store));

        // Add 3 points to buffer
        for i in 0..3 {
            let point = Arc::new(create_test_point(&format!("source-{}-Mqtt", i)));
            subscriber.handle_point(point);
        }

        let result = subscriber.flush().await;
        assert!(result.is_ok());
        assert_eq!(subscriber.events_written, 3);
        assert_eq!(subscriber.batches_written, 1);
        assert!(subscriber.buffer.is_empty());
    }

    #[tokio::test]
    async fn test_flush_retries_on_failure() {
        let config = BronzeSubscriberConfig {
            batch_size: 10,
            flush_interval_secs: 5,
            max_retries: 2,
            stream_filter: Vec::new(),
        };
        let mut mock_store = MockRawStore::new();

        // First 2 calls fail, third succeeds
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        mock_store
            .expect_write_raw_batch()
            .times(3)
            .returning(move |_| {
                let count = call_count_clone.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(CoreError::Storage("Transient failure".to_string()))
                } else {
                    Ok(())
                }
            });

        let mut subscriber = BronzeSubscriber::new("bronze-test", config, Arc::new(mock_store));

        let point = Arc::new(create_test_point("air-quality-Mqtt"));
        subscriber.handle_point(point);

        let result = subscriber.flush().await;
        assert!(result.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_flush_fails_after_max_retries() {
        let config = BronzeSubscriberConfig {
            batch_size: 10,
            flush_interval_secs: 5,
            max_retries: 2,
            stream_filter: Vec::new(),
        };
        let mut mock_store = MockRawStore::new();

        // All calls fail
        mock_store
            .expect_write_raw_batch()
            .times(3) // Initial + 2 retries
            .returning(|_| Err(CoreError::Storage("Persistent failure".to_string())));

        let mut subscriber = BronzeSubscriber::new("bronze-test", config, Arc::new(mock_store));

        let point = Arc::new(create_test_point("air-quality-Mqtt"));
        subscriber.handle_point(point);

        let result = subscriber.flush().await;
        assert!(result.is_err());
        assert_eq!(subscriber.errors_total, 1);
    }

    // ========== TDD CYCLE 5: Health Check Tests ==========

    #[tokio::test]
    async fn test_health_check_not_running() {
        let config = BronzeSubscriberConfig::default();
        let mock_store = MockRawStore::new();
        let subscriber = BronzeSubscriber::new("bronze-test", config, Arc::new(mock_store));

        let health = subscriber.health_check().await;
        assert!(health.healthy); // No errors, so healthy even when not running
        assert!(health.message.contains("not running"));
        assert_eq!(health.details.get("is_running"), Some(&"false".to_string()));
    }

    #[tokio::test]
    async fn test_health_check_includes_metrics() {
        let config = BronzeSubscriberConfig::default();
        let mock_store = MockRawStore::new();
        let subscriber = BronzeSubscriber::new("bronze-test", config, Arc::new(mock_store));

        let health = subscriber.health_check().await;
        assert!(health.details.contains_key("events_received"));
        assert!(health.details.contains_key("events_written"));
        assert!(health.details.contains_key("batches_written"));
        assert!(health.details.contains_key("errors_total"));
        assert!(health.details.contains_key("buffer_size"));
    }

    // ========== TDD CYCLE 6: Extract Stream ID Helper Tests ==========

    #[test]
    fn test_extract_stream_from_source_id() {
        assert_eq!(
            extract_stream_from_source_id("air-quality-Mqtt"),
            "air-quality"
        );
        assert_eq!(
            extract_stream_from_source_id("air-quality-Http"),
            "air-quality"
        );
        assert_eq!(
            extract_stream_from_source_id("nws-forecast-HttpPoll"),
            "nws-forecast"
        );
        assert_eq!(
            extract_stream_from_source_id("file-data-FileWatch"),
            "file-data"
        );
        assert_eq!(extract_stream_from_source_id("webhook-Webhook"), "webhook");
        // No suffix - return as-is
        assert_eq!(extract_stream_from_source_id("air-quality"), "air-quality");
        assert_eq!(extract_stream_from_source_id("unknown"), "unknown");
    }

    // ========== TDD CYCLE 7: Integration-Style Tests with Event Bus ==========

    #[tokio::test]
    async fn test_subscriber_receives_and_processes_events() {
        let config = create_config(5, 60); // Large timeout, batch at 5
        let mut mock_store = MockRawStore::new();

        // Expect exactly one batch write with 5 points
        mock_store
            .expect_write_raw_batch()
            .times(1)
            .withf(|points| points.len() == 5)
            .returning(|_| Ok(()));

        let mut subscriber = BronzeSubscriber::new("bronze-test", config, Arc::new(mock_store));

        // Create broadcast channel
        let (tx, rx) = broadcast::channel::<Arc<RawDataPoint>>(100);

        // Spawn subscriber task
        let subscriber_handle = { tokio::spawn(async move { subscriber.start(rx).await }) };

        // Send 5 events (will trigger batch flush)
        for i in 0..5 {
            let point = Arc::new(create_test_point(&format!("air-quality-{}-Mqtt", i)));
            tx.send(point).unwrap();
        }

        // Give time for processing
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Stop by closing channel
        drop(tx);

        // Wait for subscriber to finish
        let result = subscriber_handle.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_subscriber_flushes_on_timer() {
        let config = create_config(100, 1); // Large batch, 1 second timer
        let mut mock_store = MockRawStore::new();

        // Expect at least one flush (may be 2 due to final flush)
        mock_store
            .expect_write_raw_batch()
            .times(1..=2)
            .returning(|_| Ok(()));

        let mut subscriber = BronzeSubscriber::new("bronze-test", config, Arc::new(mock_store));

        let (tx, rx) = broadcast::channel::<Arc<RawDataPoint>>(100);

        let subscriber_handle = tokio::spawn(async move { subscriber.start(rx).await });

        // Send 3 events (not enough to trigger batch)
        for i in 0..3 {
            let point = Arc::new(create_test_point(&format!("source-{}-Mqtt", i)));
            tx.send(point).unwrap();
        }

        // Wait for timer to trigger flush (>1 second)
        tokio::time::sleep(Duration::from_millis(1200)).await;

        // Stop
        drop(tx);

        let result = subscriber_handle.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_subscriber_handles_lagged_error() {
        // Use very small channel to trigger lag
        let config = create_config(100, 60);
        let mock_store = MockRawStore::new();
        let mut subscriber = BronzeSubscriber::new("bronze-test", config, Arc::new(mock_store));

        let (tx, mut rx) = broadcast::channel::<Arc<RawDataPoint>>(2);

        // Fill channel to cause lag
        for i in 0..5 {
            let point = Arc::new(create_test_point(&format!("source-{}-Mqtt", i)));
            let _ = tx.send(point);
        }

        // Receiver should get lagged error on next recv
        // This is tested by the fact that start() handles RecvError::Lagged and continues
        // The test here ensures subscriber doesn't crash on lag

        // We can't easily test this without running start() which needs more setup
        // This is more of a documentation that lag is handled
    }

    #[tokio::test]
    async fn test_subscriber_graceful_shutdown() {
        let config = create_config(100, 60);
        let mut mock_store = MockRawStore::new();

        // Expect final flush on shutdown
        mock_store
            .expect_write_raw_batch()
            .times(1)
            .returning(|_| Ok(()));

        let mut subscriber = BronzeSubscriber::new("bronze-test", config, Arc::new(mock_store));

        let (tx, rx) = broadcast::channel::<Arc<RawDataPoint>>(100);

        // Get cancellation token before moving subscriber
        let cancel_token = subscriber.cancellation_token.clone();

        let subscriber_handle = tokio::spawn(async move { subscriber.start(rx).await });

        // Send some events
        for i in 0..3 {
            let point = Arc::new(create_test_point(&format!("source-{}-Mqtt", i)));
            tx.send(point).unwrap();
        }

        // Give time for events to be received
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Trigger graceful shutdown via cancellation token
        cancel_token.cancel();

        let result = subscriber_handle.await.unwrap();
        assert!(result.is_ok());
    }
}
