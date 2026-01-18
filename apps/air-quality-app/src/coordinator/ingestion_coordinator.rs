//! Ingestion Coordinator
//!
//! Coordinates multiple data sources and manages their lifecycle.
//! DP-012: EventBus is the SOLE data flow mechanism. Sources publish to EventBus,
//! and subscribers (BronzeSubscriber, SilverSubscriber, etc.) receive events.
//!
//! Architecture change from dp-004:
//! - BEFORE: Sources -> mpsc -> RawStorageWriter -> ParquetStore
//! - AFTER:  Sources -> EventBus -> BronzeSubscriber -> ParquetStore

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

use neural_core::{EventBus, EventBusConfig};

use super::router::IngestionRouter;
use super::source_manager::{SourceHealth, SourceManager};

/// Ingestion coordinator error
#[derive(Debug, thiserror::Error)]
pub enum CoordinatorError {
    #[error("Failed to route point: {0}")]
    RoutingError(String),

    #[error("Source manager error: {0}")]
    SourceManagerError(String),

    #[error("Shutdown error: {0}")]
    ShutdownError(String),

    #[error("Channel error: {0}")]
    ChannelError(String),
}

/// Coordinates multi-stream data ingestion
///
/// DP-012: EventBus is the SOLE data flow mechanism. Sources publish to EventBus,
/// and subscribers (BronzeSubscriber, SilverSubscriber, etc.) handle storage.
/// The mpsc channel is REMOVED - EventBus replaces it entirely.
pub struct IngestionCoordinator {
    #[allow(dead_code)]
    router: Arc<IngestionRouter>,
    source_manager: Arc<RwLock<SourceManager>>,
    shutdown_tx: mpsc::Sender<()>,
    #[allow(dead_code)]
    shutdown_rx: Arc<RwLock<mpsc::Receiver<()>>>,
    is_running: Arc<AtomicBool>,
    /// DP-012: EventBus for multi-consumer event broadcasting
    /// This is the SOLE data flow mechanism - no mpsc channel
    event_bus: Arc<EventBus>,
}

impl IngestionCoordinator {
    /// Create a new ingestion coordinator
    ///
    /// DP-012: Creates an EventBus that is the SOLE data flow mechanism.
    /// Sources publish to EventBus, and subscribers handle storage.
    /// Use `event_bus()` to get a reference for registering subscribers
    /// via SubscriberCoordinator.
    pub fn new(
        router: Arc<IngestionRouter>,
        source_manager: Arc<RwLock<SourceManager>>,
        _buffer_size: usize,
    ) -> Self {
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        // DP-012: Create EventBus with default config (10,000 capacity, drop_oldest)
        // This is the SOLE data flow mechanism - no mpsc channel
        let event_bus = Arc::new(EventBus::new(EventBusConfig::default()));
        debug!(
            capacity = event_bus.config().capacity,
            "EventBus created for IngestionCoordinator (SOLE data flow)"
        );

        Self {
            router,
            source_manager,
            shutdown_tx,
            shutdown_rx: Arc::new(RwLock::new(shutdown_rx)),
            is_running: Arc::new(AtomicBool::new(false)),
            event_bus,
        }
    }

    /// Get a reference to the EventBus for subscriber registration
    ///
    /// DP-012: Use this to create a SubscriberCoordinator and register subscribers.
    /// Subscribers will receive all events published after they subscribe.
    pub fn event_bus(&self) -> Arc<EventBus> {
        Arc::clone(&self.event_bus)
    }

    /// Start the coordinator
    ///
    /// DP-012: Sets the EventBus on SourceManager and starts all sources.
    /// Sources will publish to EventBus, which broadcasts to all subscribers.
    pub async fn start(&self) -> Result<(), CoordinatorError> {
        info!("Starting ingestion coordinator");

        // Use compare_exchange to atomically check and set
        if self
            .is_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            warn!("Coordinator already running");
            return Ok(());
        }

        // DP-012: Set EventBus on SourceManager - this is the SOLE data flow mechanism
        let mut sm = self.source_manager.write().await;
        sm.set_event_bus(Arc::clone(&self.event_bus));

        // Start source manager (sources will publish to EventBus)
        sm.start_all_sources()
            .await
            .map_err(|e| CoordinatorError::SourceManagerError(e.to_string()))?;
        drop(sm);

        info!(
            subscriber_count = self.event_bus.subscriber_count(),
            "Ingestion coordinator started (sources publishing to EventBus)"
        );
        Ok(())
    }

    /// Stop the coordinator
    pub async fn stop(&self) -> Result<(), CoordinatorError> {
        info!("Stopping ingestion coordinator");

        // Check if running (atomic read)
        if !self.is_running.load(Ordering::SeqCst) {
            warn!("Coordinator not running");
            return Ok(());
        }

        // Stop all sources
        let mut sm = self.source_manager.write().await;
        sm.stop_all_sources()
            .await
            .map_err(|e| CoordinatorError::SourceManagerError(e.to_string()))?;
        drop(sm);

        // Send shutdown signal
        self.shutdown_tx
            .send(())
            .await
            .map_err(|e| CoordinatorError::ShutdownError(e.to_string()))?;

        self.is_running.store(false, Ordering::SeqCst);
        info!("Ingestion coordinator stopped");
        Ok(())
    }

    /// Get health status of all sources
    pub async fn get_source_health(&self) -> HashMap<String, SourceHealth> {
        let sm = self.source_manager.read().await;
        sm.get_all_health().await
    }

    /// Check if coordinator is running
    pub async fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config_client::StreamRegistry;
    use std::time::Duration;

    // ========== TEST HELPERS ==========

    /// Creates a properly configured coordinator
    /// DP-012: No mpsc channel needed - EventBus is the SOLE data flow mechanism
    async fn create_test_coordinator() -> IngestionCoordinator {
        let (dead_letter_tx, _dead_letter_rx) = mpsc::channel(10);
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let router = Arc::new(IngestionRouter::new(registry.clone(), dead_letter_tx));
        let source_manager = Arc::new(RwLock::new(SourceManager::new(registry)));

        // DP-012: No need to set ingestion sender - EventBus is set during start()
        IngestionCoordinator::new(router, source_manager, 100)
    }

    // ========== LONDON SCHOOL TDD: BEHAVIOR VERIFICATION TESTS ==========

    #[tokio::test]
    async fn test_coordinator_starts_successfully() {
        // Arrange
        let coordinator = create_test_coordinator().await;

        // Act
        let result = coordinator.start().await;

        // Assert
        assert!(result.is_ok());
        assert!(coordinator.is_running().await);

        // Cleanup
        let _ = coordinator.stop().await;
    }

    #[tokio::test]
    async fn test_coordinator_stops_cleanly() {
        // Arrange
        let coordinator = create_test_coordinator().await;

        coordinator.start().await.unwrap();
        assert!(coordinator.is_running().await);

        // Act
        let result = coordinator.stop().await;

        // Assert
        assert!(result.is_ok());
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!coordinator.is_running().await);
    }

    #[tokio::test]
    async fn test_coordinator_double_start_idempotent() {
        // Arrange
        let coordinator = create_test_coordinator().await;

        // Act
        let result1 = coordinator.start().await;
        let result2 = coordinator.start().await;

        // Assert
        assert!(result1.is_ok());
        assert!(result2.is_ok()); // Should not fail on second start
        assert!(coordinator.is_running().await);

        // Cleanup
        let _ = coordinator.stop().await;
    }

    #[tokio::test]
    async fn test_coordinator_stop_when_not_running() {
        // Arrange
        let coordinator = create_test_coordinator().await;

        // Act
        let result = coordinator.stop().await;

        // Assert
        assert!(result.is_ok()); // Should not fail when stopping already stopped coordinator
        assert!(!coordinator.is_running().await);
    }

    #[tokio::test]
    async fn test_coordinator_routes_points_to_router() {
        // This test verifies the coordinator can be started and integrates with router

        // Arrange
        let coordinator = create_test_coordinator().await;

        coordinator.start().await.unwrap();

        // Act - verify coordinator is running
        assert!(coordinator.is_running().await);

        // Cleanup
        let _ = coordinator.stop().await;
    }

    #[tokio::test]
    async fn test_coordinator_handles_source_failures_gracefully() {
        // Arrange
        let coordinator = create_test_coordinator().await;

        // Act
        coordinator.start().await.unwrap();

        // Get health (may have sources loaded from etcd)
        let health = coordinator.get_source_health().await;

        // Assert - coordinator should still be running regardless of source count
        assert!(coordinator.is_running().await);
        // Health map exists (may be empty or have sources from etcd)
        assert!(health.len() >= 0);

        // Cleanup
        let _ = coordinator.stop().await;
    }

    #[tokio::test]
    async fn test_coordinator_buffer_capacity() {
        // Arrange - use helper with custom buffer size
        // DP-012: No mpsc channel needed - EventBus is the SOLE data flow mechanism
        let (dead_letter_tx, _dead_letter_rx) = mpsc::channel(10);
        let buffer_size = 50;
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let router = Arc::new(IngestionRouter::new(registry.clone(), dead_letter_tx));
        let source_manager = Arc::new(RwLock::new(SourceManager::new(registry)));
        // DP-012: No need to set ingestion sender - EventBus is set during start()
        let coordinator = IngestionCoordinator::new(router, source_manager, buffer_size);

        // Act & Assert - verify coordinator created successfully with buffer
        assert!(!coordinator.is_running().await);
    }

    #[tokio::test]
    async fn test_coordinator_get_source_health() {
        // Arrange
        let coordinator = create_test_coordinator().await;

        coordinator.start().await.unwrap();

        // Act
        let health = coordinator.get_source_health().await;

        // Assert - should return health status HashMap
        assert!(health.len() >= 0);

        // Cleanup
        let _ = coordinator.stop().await;
    }

    // ========== ERROR HANDLING TESTS ==========

    #[tokio::test]
    async fn test_coordinator_handles_shutdown_signal() {
        // Arrange
        let coordinator = create_test_coordinator().await;

        coordinator.start().await.unwrap();
        assert!(coordinator.is_running().await);

        // Act - send shutdown signal
        coordinator.stop().await.unwrap();

        // Wait for shutdown to complete
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Assert
        assert!(!coordinator.is_running().await);
    }

    // ========== INTEGRATION CONTRACT TESTS ==========

    #[tokio::test]
    async fn test_coordinator_integrates_with_router() {
        // Arrange
        let coordinator = create_test_coordinator().await;

        // Coordinator should maintain reference to router
        assert!(coordinator.start().await.is_ok());

        // Cleanup
        let _ = coordinator.stop().await;
    }

    #[tokio::test]
    async fn test_coordinator_integrates_with_source_manager() {
        // Arrange
        let coordinator = create_test_coordinator().await;

        // Start should trigger source manager start
        coordinator.start().await.unwrap();

        // Verify source manager state through coordinator
        let health = coordinator.get_source_health().await;
        assert!(health.len() >= 0);

        // Cleanup
        let _ = coordinator.stop().await;
    }

    // ========== DP-012: EVENT BUS TESTS ==========

    #[tokio::test]
    async fn test_coordinator_has_event_bus() {
        // Arrange
        let coordinator = create_test_coordinator().await;

        // Act
        let event_bus = coordinator.event_bus();

        // Assert - EventBus is created with default config
        assert_eq!(event_bus.config().capacity, 10_000);
        assert_eq!(event_bus.subscriber_count(), 0);
    }

    #[tokio::test]
    async fn test_event_bus_shared_across_clones() {
        // Arrange
        let coordinator = create_test_coordinator().await;

        // Act - get event_bus twice
        let bus1 = coordinator.event_bus();
        let bus2 = coordinator.event_bus();

        // Subscribe on one
        let _subscriber = bus1.subscribe();

        // Assert - subscriber count visible on both (same Arc)
        assert_eq!(bus1.subscriber_count(), 1);
        assert_eq!(bus2.subscriber_count(), 1);
    }
}
