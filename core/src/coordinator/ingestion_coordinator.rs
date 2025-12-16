//! IngestionCoordinator - Central orchestrator for multi-stream data ingestion
//!
//! The coordinator owns the central channel and manages the flow of data from
//! multiple sources to storage writers.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{debug, info, warn};

use crate::error::{CoreError, CoreResult};
use crate::traits::{HealthStatus, TimeSeriesPoint};
use crate::types::StreamRecord;

/// Configuration for the IngestionCoordinator
#[derive(Debug, Clone)]
pub struct IngestionCoordinatorConfig {
    /// Capacity of the main ingestion channel
    pub channel_capacity: usize,
    /// Capacity of per-stream storage channels
    pub storage_channel_capacity: usize,
    /// How often to check for idle sources (seconds)
    pub health_check_interval_secs: u64,
    /// Maximum time to wait for graceful shutdown (seconds)
    pub shutdown_timeout_secs: u64,
}

impl Default for IngestionCoordinatorConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 1000,
            storage_channel_capacity: 500,
            health_check_interval_secs: 30,
            shutdown_timeout_secs: 10,
        }
    }
}

/// Handle for sending data to the coordinator
#[derive(Clone)]
pub struct IngestionHandle {
    sender: mpsc::Sender<StreamRecord>,
    source_id: String,
}

impl IngestionHandle {
    /// Send a record to the coordinator
    pub async fn send(&self, record: StreamRecord) -> CoreResult<()> {
        self.sender
            .send(record)
            .await
            .map_err(|e| CoreError::Source(format!("Failed to send to coordinator: {}", e)))
    }

    /// Send a batch of records to the coordinator
    pub async fn send_batch(&self, records: Vec<StreamRecord>) -> CoreResult<()> {
        for record in records {
            self.send(record).await?;
        }
        Ok(())
    }

    /// Try to send without blocking
    pub fn try_send(&self, record: StreamRecord) -> CoreResult<()> {
        self.sender
            .try_send(record)
            .map_err(|e| CoreError::Source(format!("Channel full or closed: {}", e)))
    }

    /// Get the source ID associated with this handle
    pub fn source_id(&self) -> &str {
        &self.source_id
    }
}

/// Storage channel registration
struct StorageChannel {
    sender: mpsc::Sender<TimeSeriesPoint>,
    #[allow(dead_code)]
    stream_id: String,
}

/// Coordinator statistics
#[derive(Debug, Clone, Default)]
pub struct CoordinatorStats {
    pub records_received: u64,
    pub records_routed: u64,
    pub records_dropped: u64,
    pub active_sources: usize,
    pub active_storage_channels: usize,
}

/// Central ingestion coordinator that owns the main channel
pub struct IngestionCoordinator {
    config: IngestionCoordinatorConfig,
    receiver: Arc<RwLock<Option<mpsc::Receiver<StreamRecord>>>>,
    sender: mpsc::Sender<StreamRecord>,
    storage_channels: Arc<RwLock<HashMap<String, StorageChannel>>>,
    is_running: Arc<RwLock<bool>>,
    shutdown_tx: Arc<RwLock<Option<oneshot::Sender<()>>>>,
    stats: Arc<RwLock<CoordinatorStats>>,
    source_handles: Arc<RwLock<HashMap<String, IngestionHandle>>>,
}

impl IngestionCoordinator {
    /// Create a new IngestionCoordinator
    pub fn new(config: IngestionCoordinatorConfig) -> Self {
        let (sender, receiver) = mpsc::channel(config.channel_capacity);

        Self {
            config,
            receiver: Arc::new(RwLock::new(Some(receiver))),
            sender,
            storage_channels: Arc::new(RwLock::new(HashMap::new())),
            is_running: Arc::new(RwLock::new(false)),
            shutdown_tx: Arc::new(RwLock::new(None)),
            stats: Arc::new(RwLock::new(CoordinatorStats::default())),
            source_handles: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new ingestion handle for a source
    pub async fn create_handle(&self, source_id: String) -> IngestionHandle {
        let handle = IngestionHandle {
            sender: self.sender.clone(),
            source_id: source_id.clone(),
        };

        let mut handles = self.source_handles.write().await;
        handles.insert(source_id, handle.clone());

        handle
    }

    /// Remove a source handle
    pub async fn remove_handle(&self, source_id: &str) {
        let mut handles = self.source_handles.write().await;
        handles.remove(source_id);
    }

    /// Register a storage channel for a stream
    pub async fn register_storage_channel(
        &self,
        stream_id: String,
        sender: mpsc::Sender<TimeSeriesPoint>,
    ) {
        let mut channels = self.storage_channels.write().await;
        channels.insert(
            stream_id.clone(),
            StorageChannel {
                sender,
                stream_id: stream_id.clone(),
            },
        );
        debug!("Registered storage channel for stream: {}", stream_id);

        let mut stats = self.stats.write().await;
        stats.active_storage_channels = channels.len();
    }

    /// Unregister a storage channel
    pub async fn unregister_storage_channel(&self, stream_id: &str) {
        let mut channels = self.storage_channels.write().await;
        channels.remove(stream_id);
        debug!("Unregistered storage channel for stream: {}", stream_id);

        let mut stats = self.stats.write().await;
        stats.active_storage_channels = channels.len();
    }

    /// Start the coordinator's routing loop
    pub async fn start(&self) -> CoreResult<()> {
        info!("Starting IngestionCoordinator");

        // Take ownership of the receiver
        let receiver = {
            let mut recv_guard = self.receiver.write().await;
            recv_guard.take()
        };

        let mut receiver = receiver.ok_or_else(|| {
            CoreError::Config("Coordinator already started or receiver taken".to_string())
        })?;

        *self.is_running.write().await = true;

        // Create shutdown channel
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
        *self.shutdown_tx.write().await = Some(shutdown_tx);

        // Clone Arcs for the routing task
        let storage_channels = self.storage_channels.clone();
        let is_running = self.is_running.clone();
        let stats = self.stats.clone();

        // Spawn the routing loop
        tokio::spawn(async move {
            info!("Coordinator routing loop started");

            loop {
                tokio::select! {
                    biased;

                    // Check for shutdown signal
                    _ = &mut shutdown_rx => {
                        info!("Coordinator received shutdown signal");
                        break;
                    }

                    // Process incoming records
                    Some(record) = receiver.recv() => {
                        // Update stats
                        {
                            let mut s = stats.write().await;
                            s.records_received += 1;
                        }

                        // Route to appropriate storage channel
                        let channels = storage_channels.read().await;
                        if let Some(channel) = channels.get(&record.stream_id) {
                            // Convert StreamRecord to TimeSeriesPoint for storage
                            let point = record.point.clone();

                            if let Err(e) = channel.sender.send(point).await {
                                warn!(
                                    "Failed to send to storage channel for {}: {}",
                                    record.stream_id, e
                                );
                                let mut s = stats.write().await;
                                s.records_dropped += 1;
                            } else {
                                debug!("Routed record to stream: {}", record.stream_id);
                                let mut s = stats.write().await;
                                s.records_routed += 1;
                            }
                        } else {
                            // No storage channel registered for this stream
                            warn!(
                                "No storage channel for stream: {}. Dropping record.",
                                record.stream_id
                            );
                            let mut s = stats.write().await;
                            s.records_dropped += 1;
                        }
                    }

                    // No more records
                    else => {
                        info!("Coordinator channel closed, exiting routing loop");
                        break;
                    }
                }

                // Check if we should still be running
                if !*is_running.read().await {
                    break;
                }
            }

            info!("Coordinator routing loop ended");
        });

        Ok(())
    }

    /// Stop the coordinator gracefully
    pub async fn stop(&self) -> CoreResult<()> {
        info!("Stopping IngestionCoordinator");

        *self.is_running.write().await = false;

        // Send shutdown signal
        if let Some(tx) = self.shutdown_tx.write().await.take() {
            let _ = tx.send(());
        }

        // Wait for pending records to drain (with timeout)
        let timeout = Duration::from_secs(self.config.shutdown_timeout_secs);
        tokio::time::sleep(timeout).await;

        info!("IngestionCoordinator stopped");
        Ok(())
    }

    /// Check if the coordinator is running
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    /// Get coordinator statistics
    pub async fn stats(&self) -> CoordinatorStats {
        self.stats.read().await.clone()
    }

    /// Get health status
    pub async fn health_check(&self) -> HealthStatus {
        let is_running = *self.is_running.read().await;
        let stats = self.stats.read().await;

        let mut details = HashMap::new();
        details.insert("records_received".to_string(), stats.records_received.to_string());
        details.insert("records_routed".to_string(), stats.records_routed.to_string());
        details.insert("records_dropped".to_string(), stats.records_dropped.to_string());
        details.insert("active_sources".to_string(), stats.active_sources.to_string());
        details.insert(
            "active_storage_channels".to_string(),
            stats.active_storage_channels.to_string(),
        );

        if is_running {
            HealthStatus {
                healthy: true,
                message: "IngestionCoordinator running".to_string(),
                details,
            }
        } else {
            HealthStatus {
                healthy: false,
                message: "IngestionCoordinator not running".to_string(),
                details,
            }
        }
    }

    /// Get the number of active source handles
    pub async fn active_source_count(&self) -> usize {
        self.source_handles.read().await.len()
    }

    /// Get list of registered stream IDs
    pub async fn registered_streams(&self) -> Vec<String> {
        self.storage_channels
            .read()
            .await
            .keys()
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_point() -> TimeSeriesPoint {
        TimeSeriesPoint {
            timestamp: Utc::now(),
            location_id: "test-location".to_string(),
            value: 25.5,
            tags: HashMap::new(),
        }
    }

    fn create_test_record(stream_id: &str) -> StreamRecord {
        StreamRecord::new(stream_id.to_string(), create_test_point())
    }

    #[tokio::test]
    async fn test_coordinator_creation() {
        let config = IngestionCoordinatorConfig::default();
        let coordinator = IngestionCoordinator::new(config.clone());

        assert!(!coordinator.is_running().await);
        assert_eq!(coordinator.active_source_count().await, 0);
    }

    #[tokio::test]
    async fn test_create_handle() {
        let coordinator = IngestionCoordinator::new(IngestionCoordinatorConfig::default());
        let handle = coordinator.create_handle("source-001".to_string()).await;

        assert_eq!(handle.source_id(), "source-001");
        assert_eq!(coordinator.active_source_count().await, 1);
    }

    #[tokio::test]
    async fn test_remove_handle() {
        let coordinator = IngestionCoordinator::new(IngestionCoordinatorConfig::default());
        coordinator.create_handle("source-001".to_string()).await;
        assert_eq!(coordinator.active_source_count().await, 1);

        coordinator.remove_handle("source-001").await;
        assert_eq!(coordinator.active_source_count().await, 0);
    }

    #[tokio::test]
    async fn test_register_storage_channel() {
        let coordinator = IngestionCoordinator::new(IngestionCoordinatorConfig::default());
        let (tx, _rx) = mpsc::channel::<TimeSeriesPoint>(100);

        coordinator
            .register_storage_channel("air-quality".to_string(), tx)
            .await;

        let streams = coordinator.registered_streams().await;
        assert!(streams.contains(&"air-quality".to_string()));
    }

    #[tokio::test]
    async fn test_unregister_storage_channel() {
        let coordinator = IngestionCoordinator::new(IngestionCoordinatorConfig::default());
        let (tx, _rx) = mpsc::channel::<TimeSeriesPoint>(100);

        coordinator
            .register_storage_channel("air-quality".to_string(), tx)
            .await;
        coordinator.unregister_storage_channel("air-quality").await;

        let streams = coordinator.registered_streams().await;
        assert!(!streams.contains(&"air-quality".to_string()));
    }

    #[tokio::test]
    async fn test_start_and_stop() {
        let coordinator = IngestionCoordinator::new(IngestionCoordinatorConfig {
            shutdown_timeout_secs: 1,
            ..Default::default()
        });

        coordinator.start().await.unwrap();
        assert!(coordinator.is_running().await);

        coordinator.stop().await.unwrap();
        assert!(!coordinator.is_running().await);
    }

    #[tokio::test]
    async fn test_health_check_not_running() {
        let coordinator = IngestionCoordinator::new(IngestionCoordinatorConfig::default());
        let health = coordinator.health_check().await;

        assert!(!health.healthy);
        assert!(health.message.contains("not running"));
    }

    #[tokio::test]
    async fn test_health_check_running() {
        let coordinator = IngestionCoordinator::new(IngestionCoordinatorConfig {
            shutdown_timeout_secs: 1,
            ..Default::default()
        });

        coordinator.start().await.unwrap();
        let health = coordinator.health_check().await;

        assert!(health.healthy);
        assert!(health.message.contains("running"));

        coordinator.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_routing_to_storage_channel() {
        let coordinator = IngestionCoordinator::new(IngestionCoordinatorConfig {
            shutdown_timeout_secs: 1,
            ..Default::default()
        });

        // Register storage channel
        let (storage_tx, mut storage_rx) = mpsc::channel::<TimeSeriesPoint>(100);
        coordinator
            .register_storage_channel("test-stream".to_string(), storage_tx)
            .await;

        // Start coordinator
        coordinator.start().await.unwrap();

        // Create handle and send record
        let handle = coordinator.create_handle("source-001".to_string()).await;
        let record = create_test_record("test-stream");
        handle.send(record.clone()).await.unwrap();

        // Wait for routing
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify record was routed
        let received = storage_rx.try_recv();
        assert!(received.is_ok());
        assert_eq!(received.unwrap().value, 25.5);

        coordinator.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let coordinator = IngestionCoordinator::new(IngestionCoordinatorConfig {
            shutdown_timeout_secs: 1,
            ..Default::default()
        });

        // Register storage channel
        let (storage_tx, mut storage_rx) = mpsc::channel::<TimeSeriesPoint>(100);
        coordinator
            .register_storage_channel("test-stream".to_string(), storage_tx)
            .await;

        coordinator.start().await.unwrap();

        // Send records
        let handle = coordinator.create_handle("source-001".to_string()).await;
        for _ in 0..5 {
            handle.send(create_test_record("test-stream")).await.unwrap();
        }

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Drain receiver
        while storage_rx.try_recv().is_ok() {}

        let stats = coordinator.stats().await;
        assert_eq!(stats.records_received, 5);
        assert_eq!(stats.records_routed, 5);
        assert_eq!(stats.records_dropped, 0);

        coordinator.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_dropped_records_for_unknown_stream() {
        let coordinator = IngestionCoordinator::new(IngestionCoordinatorConfig {
            shutdown_timeout_secs: 1,
            ..Default::default()
        });

        // Don't register any storage channel
        coordinator.start().await.unwrap();

        // Send record to unknown stream
        let handle = coordinator.create_handle("source-001".to_string()).await;
        handle
            .send(create_test_record("unknown-stream"))
            .await
            .unwrap();

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(100)).await;

        let stats = coordinator.stats().await;
        assert_eq!(stats.records_received, 1);
        assert_eq!(stats.records_dropped, 1);

        coordinator.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_handle_try_send() {
        let coordinator = IngestionCoordinator::new(IngestionCoordinatorConfig::default());
        let handle = coordinator.create_handle("source-001".to_string()).await;

        // try_send should work when channel is not full
        let record = create_test_record("test-stream");
        assert!(handle.try_send(record).is_ok());
    }

    #[tokio::test]
    async fn test_handle_send_batch() {
        let coordinator = IngestionCoordinator::new(IngestionCoordinatorConfig {
            shutdown_timeout_secs: 1,
            ..Default::default()
        });

        let (storage_tx, mut storage_rx) = mpsc::channel::<TimeSeriesPoint>(100);
        coordinator
            .register_storage_channel("test-stream".to_string(), storage_tx)
            .await;

        coordinator.start().await.unwrap();

        let handle = coordinator.create_handle("source-001".to_string()).await;
        let records: Vec<StreamRecord> = (0..3)
            .map(|_| create_test_record("test-stream"))
            .collect();

        handle.send_batch(records).await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Count received
        let mut count = 0;
        while storage_rx.try_recv().is_ok() {
            count += 1;
        }
        assert_eq!(count, 3);

        coordinator.stop().await.unwrap();
    }
}
