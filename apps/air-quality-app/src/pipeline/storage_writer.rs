//! Storage Writer for AIR-002 Pipeline
//!
//! This module provides the storage pipeline component that:
//! - Receives TimeSeriesPoint data from the MQTT handler via channel
//! - Batches points for efficient writes (default 100 points per batch)
//! - Flushes batches on timeout (default 5 seconds) or when batch is full
//! - Writes to ParquetStore for persistent storage
//!
//! ## DP-004: Raw JSON Storage (Bronze Layer)
//!
//! The `RawStorageWriter` variant stores exact source payloads as JSON blobs
//! using the 5-column schema: timestamp, source_id, ndp_id, context, raw_payload.
//! This enables schema evolution and reprocessing without data loss.

use neural_core::traits::{RawStore, Store};
use neural_core::types::RawDataPoint;
use neural_core::{CoreError, ParquetStore, TimeSeriesPoint};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Storage writer for the AIR-002 ingestion pipeline
///
/// Receives points from a channel, batches them, and writes to ParquetStore
pub struct StorageWriter {
    store: Arc<ParquetStore>,
    receiver: mpsc::Receiver<TimeSeriesPoint>,
    batch_size: usize,
    batch_timeout: Duration,
}

impl StorageWriter {
    /// Create a new storage writer
    ///
    /// # Arguments
    /// * `store` - Arc-wrapped ParquetStore for writing data
    /// * `receiver` - Channel receiver for incoming TimeSeriesPoints
    /// * `batch_size` - Number of points to accumulate before flushing (default: 100)
    /// * `batch_timeout` - Maximum time to wait before flushing incomplete batch (default: 5s)
    pub fn new(
        store: Arc<ParquetStore>,
        receiver: mpsc::Receiver<TimeSeriesPoint>,
        batch_size: Option<usize>,
        batch_timeout: Option<Duration>,
    ) -> Self {
        let batch_size = batch_size.unwrap_or(100);
        let batch_timeout = batch_timeout.unwrap_or(Duration::from_secs(5));

        info!(
            "Initializing storage writer (batch_size={}, timeout={}s)",
            batch_size,
            batch_timeout.as_secs()
        );

        Self {
            store,
            receiver,
            batch_size,
            batch_timeout,
        }
    }

    /// Run the storage pipeline
    ///
    /// Continuously receives points from the channel and writes them in batches.
    /// Flushes when either:
    /// - Batch size is reached
    /// - Timeout expires (to ensure timely writes even with low data rates)
    ///
    /// This should run indefinitely until the channel is closed.
    ///
    /// # Errors
    /// Returns error if storage write fails
    pub async fn run(mut self) -> Result<(), CoreError> {
        info!("Starting storage writer pipeline");

        let mut buffer: Vec<TimeSeriesPoint> = Vec::with_capacity(self.batch_size);
        let mut flush_interval = tokio::time::interval(self.batch_timeout);

        // Skip the first tick (happens immediately)
        flush_interval.tick().await;

        loop {
            tokio::select! {
                // Receive points from channel
                point_opt = self.receiver.recv() => {
                    match point_opt {
                        Some(point) => {
                            debug!("Received point for location: {}", point.location_id);
                            buffer.push(point);

                            // Flush if batch is full
                            if buffer.len() >= self.batch_size {
                                info!("Batch size reached ({}), flushing", buffer.len());
                                self.flush(&mut buffer).await?;
                            }
                        }
                        None => {
                            warn!("Channel closed, flushing remaining points and shutting down");
                            if !buffer.is_empty() {
                                self.flush(&mut buffer).await?;
                            }
                            info!("Storage writer shutdown complete");
                            return Ok(());
                        }
                    }
                }

                // Timeout - flush even if batch is not full
                _ = flush_interval.tick() => {
                    if !buffer.is_empty() {
                        info!("Timeout reached, flushing {} points", buffer.len());
                        self.flush(&mut buffer).await?;
                    }
                }
            }
        }
    }

    /// Flush buffered points to storage
    ///
    /// Writes all points in the buffer to ParquetStore and clears the buffer.
    ///
    /// # Errors
    /// Returns error if write_batch fails
    async fn flush(&self, buffer: &mut Vec<TimeSeriesPoint>) -> Result<(), CoreError> {
        if buffer.is_empty() {
            return Ok(());
        }

        let count = buffer.len();
        debug!("Flushing {} points to storage", count);

        match self.store.write_batch(buffer.clone()).await {
            Ok(_) => {
                info!("Successfully wrote {} points to storage", count);
                buffer.clear();
                Ok(())
            }
            Err(e) => {
                error!("Failed to write batch to storage: {}", e);
                Err(e)
            }
        }
    }
}

// =============================================================================
// DP-004: RAW STORAGE WRITER (BRONZE LAYER)
// =============================================================================

/// Raw storage writer for DP-004 Bronze layer pipeline
///
/// Receives RawDataPoint from a channel, batches them, and writes to ParquetStore
/// using the 5-column schema for raw JSON preservation.
pub struct RawStorageWriter {
    store: Arc<ParquetStore>,
    receiver: mpsc::Receiver<RawDataPoint>,
    batch_size: usize,
    batch_timeout: Duration,
}

impl RawStorageWriter {
    /// Create a new raw storage writer
    ///
    /// # Arguments
    /// * `store` - Arc-wrapped ParquetStore for writing data
    /// * `receiver` - Channel receiver for incoming RawDataPoints
    /// * `batch_size` - Number of points to accumulate before flushing (default: 100)
    /// * `batch_timeout` - Maximum time to wait before flushing incomplete batch (default: 5s)
    pub fn new(
        store: Arc<ParquetStore>,
        receiver: mpsc::Receiver<RawDataPoint>,
        batch_size: Option<usize>,
        batch_timeout: Option<Duration>,
    ) -> Self {
        let batch_size = batch_size.unwrap_or(100);
        let batch_timeout = batch_timeout.unwrap_or(Duration::from_secs(5));

        info!(
            "Initializing raw storage writer (batch_size={}, timeout={}s)",
            batch_size,
            batch_timeout.as_secs()
        );

        Self {
            store,
            receiver,
            batch_size,
            batch_timeout,
        }
    }

    /// Run the raw storage pipeline
    ///
    /// Continuously receives raw data points from the channel and writes them in batches.
    /// This should run indefinitely until the channel is closed.
    ///
    /// # Errors
    /// Returns error if storage write fails
    pub async fn run(mut self) -> Result<(), CoreError> {
        info!("Starting raw storage writer pipeline");

        let mut buffer: Vec<RawDataPoint> = Vec::with_capacity(self.batch_size);
        let mut flush_interval = tokio::time::interval(self.batch_timeout);

        // Skip the first tick (happens immediately)
        flush_interval.tick().await;

        loop {
            tokio::select! {
                // Receive points from channel
                point_opt = self.receiver.recv() => {
                    match point_opt {
                        Some(point) => {
                            debug!("Received raw point from source: {}", point.source_id);
                            buffer.push(point);

                            // Flush if batch is full
                            if buffer.len() >= self.batch_size {
                                info!("Batch size reached ({}), flushing raw points", buffer.len());
                                self.flush(&mut buffer).await?;
                            }
                        }
                        None => {
                            warn!("Channel closed, flushing remaining raw points and shutting down");
                            if !buffer.is_empty() {
                                self.flush(&mut buffer).await?;
                            }
                            info!("Raw storage writer shutdown complete");
                            return Ok(());
                        }
                    }
                }

                // Timeout - flush even if batch is not full
                _ = flush_interval.tick() => {
                    if !buffer.is_empty() {
                        info!("Timeout reached, flushing {} raw points", buffer.len());
                        self.flush(&mut buffer).await?;
                    }
                }
            }
        }
    }

    /// Flush buffered raw points to storage
    ///
    /// Writes all points in the buffer to ParquetStore using the RawStore trait
    /// and clears the buffer.
    ///
    /// # Errors
    /// Returns error if write_raw_batch fails
    async fn flush(&self, buffer: &mut Vec<RawDataPoint>) -> Result<(), CoreError> {
        if buffer.is_empty() {
            return Ok(());
        }

        let count = buffer.len();
        debug!("Flushing {} raw points to storage", count);

        match self.store.write_raw_batch(buffer.clone()).await {
            Ok(_) => {
                info!("Successfully wrote {} raw points to storage", count);
                buffer.clear();
                Ok(())
            }
            Err(e) => {
                error!("Failed to write raw batch to storage: {}", e);
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn create_test_point(location_id: &str, value: f64) -> TimeSeriesPoint {
        TimeSeriesPoint {
            timestamp: Utc::now(),
            location_id: location_id.to_string(),
            value,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        }
    }

    #[tokio::test]
    async fn test_storage_writer_creation() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(ParquetStore::new(temp_dir.path()).unwrap());
        let (_sender, receiver) = mpsc::channel(100);

        let writer = StorageWriter::new(store, receiver, Some(50), Some(Duration::from_secs(10)));

        assert_eq!(writer.batch_size, 50);
        assert_eq!(writer.batch_timeout, Duration::from_secs(10));
    }

    #[tokio::test]
    async fn test_storage_writer_default_config() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(ParquetStore::new(temp_dir.path()).unwrap());
        let (_sender, receiver) = mpsc::channel(100);

        let writer = StorageWriter::new(store, receiver, None, None);

        assert_eq!(writer.batch_size, 100);
        assert_eq!(writer.batch_timeout, Duration::from_secs(5));
    }

    #[tokio::test]
    async fn test_storage_writer_flush_on_batch_size() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(ParquetStore::new(temp_dir.path()).unwrap());
        let (sender, receiver) = mpsc::channel(100);

        // Create writer with small batch size
        let writer = StorageWriter::new(
            store.clone(),
            receiver,
            Some(3),
            Some(Duration::from_secs(60)),
        );

        // Spawn writer task
        let writer_task = tokio::spawn(async move { writer.run().await });

        // Send 3 points (should trigger flush)
        for i in 0..3 {
            sender
                .send(create_test_point("sensor-001", i as f64))
                .await
                .unwrap();
        }

        // Give it time to process
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify points were written
        let points = store
            .query(
                "sensor-001",
                Utc::now() - chrono::Duration::hours(1),
                Utc::now() + chrono::Duration::hours(1),
                None,
            )
            .await
            .unwrap();

        assert_eq!(points.len(), 3);

        // Close channel and wait for writer to finish
        drop(sender);
        let _ = writer_task.await;
    }

    #[tokio::test]
    async fn test_storage_writer_shutdown_on_channel_close() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(ParquetStore::new(temp_dir.path()).unwrap());
        let (sender, receiver) = mpsc::channel(100);

        let writer = StorageWriter::new(
            store.clone(),
            receiver,
            Some(100),
            Some(Duration::from_secs(60)),
        );

        // Send a few points
        for i in 0..5 {
            sender
                .send(create_test_point("sensor-002", i as f64))
                .await
                .unwrap();
        }

        // Close channel - should trigger flush and shutdown
        drop(sender);

        // Run writer (should exit cleanly)
        let result = writer.run().await;
        assert!(result.is_ok());

        // Verify points were flushed on shutdown
        let points = store
            .query(
                "sensor-002",
                Utc::now() - chrono::Duration::hours(1),
                Utc::now() + chrono::Duration::hours(1),
                None,
            )
            .await
            .unwrap();

        assert_eq!(points.len(), 5);
    }

    #[tokio::test]
    async fn test_storage_writer_multiple_locations() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(ParquetStore::new(temp_dir.path()).unwrap());
        let (sender, receiver) = mpsc::channel(100);

        let writer = StorageWriter::new(
            store.clone(),
            receiver,
            Some(10),
            Some(Duration::from_secs(60)),
        );

        // Spawn writer task
        let writer_task = tokio::spawn(async move { writer.run().await });

        // Send points for multiple locations
        for i in 0..5 {
            sender
                .send(create_test_point("sensor-001", i as f64))
                .await
                .unwrap();
            sender
                .send(create_test_point("sensor-002", (i + 10) as f64))
                .await
                .unwrap();
        }

        // Give it time to process
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify points were written for both locations
        let points_1 = store
            .query(
                "sensor-001",
                Utc::now() - chrono::Duration::hours(1),
                Utc::now() + chrono::Duration::hours(1),
                None,
            )
            .await
            .unwrap();

        let points_2 = store
            .query(
                "sensor-002",
                Utc::now() - chrono::Duration::hours(1),
                Utc::now() + chrono::Duration::hours(1),
                None,
            )
            .await
            .unwrap();

        assert_eq!(points_1.len(), 5);
        assert_eq!(points_2.len(), 5);

        // Close and cleanup
        drop(sender);
        let _ = writer_task.await;
    }

    #[tokio::test]
    async fn test_flush_empty_buffer() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(ParquetStore::new(temp_dir.path()).unwrap());
        let (_sender, receiver) = mpsc::channel(100);

        let writer = StorageWriter::new(store, receiver, None, None);
        let mut buffer = Vec::new();

        // Flushing empty buffer should succeed without error
        let result = writer.flush(&mut buffer).await;
        assert!(result.is_ok());
        assert_eq!(buffer.len(), 0);
    }

    // =========================================================================
    // DP-004: RAW STORAGE WRITER TESTS
    // =========================================================================

    use neural_core::traits::RawStore;

    fn create_test_raw_point(source_id: &str, value: i32) -> RawDataPoint {
        RawDataPoint::new(
            source_id,
            serde_json::json!({"value": value, "source": source_id}),
        )
    }

    #[tokio::test]
    async fn test_raw_storage_writer_creation() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(ParquetStore::new(temp_dir.path()).unwrap());
        let (_sender, receiver) = mpsc::channel(100);

        let writer =
            RawStorageWriter::new(store, receiver, Some(50), Some(Duration::from_secs(10)));

        assert_eq!(writer.batch_size, 50);
        assert_eq!(writer.batch_timeout, Duration::from_secs(10));
    }

    #[tokio::test]
    async fn test_raw_storage_writer_default_config() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(ParquetStore::new(temp_dir.path()).unwrap());
        let (_sender, receiver) = mpsc::channel(100);

        let writer = RawStorageWriter::new(store, receiver, None, None);

        assert_eq!(writer.batch_size, 100);
        assert_eq!(writer.batch_timeout, Duration::from_secs(5));
    }

    #[tokio::test]
    async fn test_raw_storage_writer_flush_on_batch_size() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(ParquetStore::new(temp_dir.path()).unwrap());
        let (sender, receiver) = mpsc::channel(100);

        // Create writer with small batch size
        let writer = RawStorageWriter::new(
            store.clone(),
            receiver,
            Some(3),
            Some(Duration::from_secs(60)),
        );

        // Spawn writer task
        let writer_task = tokio::spawn(async move { writer.run().await });

        // Send 3 raw points (should trigger flush)
        for i in 0..3 {
            sender
                .send(create_test_raw_point("source-001", i))
                .await
                .unwrap();
        }

        // Give it time to process
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify points were written
        let points = store
            .query_raw(
                Utc::now() - chrono::Duration::hours(1),
                Utc::now() + chrono::Duration::hours(1),
                Some("source-001".to_string()),
            )
            .await
            .unwrap();

        assert_eq!(points.len(), 3);

        // Close channel and wait for writer to finish
        drop(sender);
        let _ = writer_task.await;
    }

    #[tokio::test]
    async fn test_raw_storage_writer_shutdown_on_channel_close() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(ParquetStore::new(temp_dir.path()).unwrap());
        let (sender, receiver) = mpsc::channel(100);

        let writer = RawStorageWriter::new(
            store.clone(),
            receiver,
            Some(100),
            Some(Duration::from_secs(60)),
        );

        // Send a few raw points
        for i in 0..5 {
            sender
                .send(create_test_raw_point("source-002", i))
                .await
                .unwrap();
        }

        // Close channel - should trigger flush and shutdown
        drop(sender);

        // Run writer (should exit cleanly)
        let result = writer.run().await;
        assert!(result.is_ok());

        // Verify points were flushed on shutdown
        let points = store
            .query_raw(
                Utc::now() - chrono::Duration::hours(1),
                Utc::now() + chrono::Duration::hours(1),
                Some("source-002".to_string()),
            )
            .await
            .unwrap();

        assert_eq!(points.len(), 5);
    }

    #[tokio::test]
    async fn test_raw_storage_writer_preserves_json_payload() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(ParquetStore::new(temp_dir.path()).unwrap());
        let (sender, receiver) = mpsc::channel(100);

        let writer = RawStorageWriter::new(
            store.clone(),
            receiver,
            Some(100),
            Some(Duration::from_secs(60)),
        );

        // Create a complex JSON payload
        let complex_payload = serde_json::json!({
            "nested": {"value": 42, "name": "test"},
            "array": [1, 2, 3],
            "string": "hello",
            "boolean": true,
            "null_field": null
        });

        let point = RawDataPoint::new("complex-source", complex_payload.clone())
            .with_ndp_id("test-ndp-001")
            .with_context(serde_json::json!({"room": "office"}));

        sender.send(point).await.unwrap();

        // Close channel to trigger flush
        drop(sender);

        // Run writer
        let result = writer.run().await;
        assert!(result.is_ok());

        // Query and verify payload is preserved exactly
        let results = store
            .query_raw(
                Utc::now() - chrono::Duration::hours(1),
                Utc::now() + chrono::Duration::hours(1),
                Some("complex-source".to_string()),
            )
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].raw_payload, complex_payload);
        assert_eq!(results[0].ndp_id, Some("test-ndp-001".to_string()));
        assert_eq!(
            results[0].context,
            Some(serde_json::json!({"room": "office"}))
        );
    }

    #[tokio::test]
    async fn test_raw_flush_empty_buffer() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(ParquetStore::new(temp_dir.path()).unwrap());
        let (_sender, receiver) = mpsc::channel(100);

        let writer = RawStorageWriter::new(store, receiver, None, None);
        let mut buffer = Vec::new();

        // Flushing empty buffer should succeed without error
        let result = writer.flush(&mut buffer).await;
        assert!(result.is_ok());
        assert_eq!(buffer.len(), 0);
    }
}
