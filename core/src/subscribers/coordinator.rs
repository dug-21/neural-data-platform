//! Subscriber Coordinator for managing event bus consumers (dp-012)
//!
//! The SubscriberCoordinator manages the lifecycle of multiple subscribers:
//! - Registration of new subscribers
//! - Starting all subscribers concurrently
//! - Stopping all subscribers gracefully
//! - Health monitoring across all subscribers
//!
//! # Architecture (ADR-012-002)
//!
//! Each subscriber runs in its own tokio task, spawned by the coordinator.
//! This provides:
//! - Task isolation: One subscriber's panic doesn't crash others
//! - Independent backpressure: Each has its own receive loop
//! - Dynamic management: Can add/remove subscribers at runtime
//!
//! # Example
//!
//! ```ignore
//! let event_bus = EventBus::with_defaults();
//! let mut coordinator = SubscriberCoordinator::new(event_bus);
//!
//! // Register subscribers
//! coordinator.register(Box::new(bronze_subscriber))?;
//! coordinator.register(Box::new(silver_subscriber))?;
//!
//! // Start all subscribers
//! coordinator.start_all().await?;
//!
//! // Later: health check
//! let health = coordinator.health_check().await;
//!
//! // Graceful shutdown
//! coordinator.stop_all().await?;
//! ```

use super::{Subscriber, SubscriberError};
use crate::event_bus::EventBus;
use crate::traits::HealthStatus;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Aggregated health status from all subscribers
#[derive(Debug, Clone)]
pub struct CoordinatorHealth {
    /// Overall health (healthy only if all subscribers are healthy)
    pub overall_healthy: bool,
    /// Number of subscribers registered
    pub subscriber_count: usize,
    /// Number of subscribers currently running
    pub running_count: usize,
    /// Individual subscriber health statuses
    pub subscriber_health: HashMap<String, HealthStatus>,
}

impl Default for CoordinatorHealth {
    fn default() -> Self {
        Self {
            overall_healthy: true,
            subscriber_count: 0,
            running_count: 0,
            subscriber_health: HashMap::new(),
        }
    }
}

/// State of the coordinator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinatorState {
    /// Coordinator created but not started
    Idle,
    /// Coordinator is running (subscribers active)
    Running,
    /// Coordinator is stopping
    Stopping,
    /// Coordinator has stopped
    Stopped,
}

/// Manages multiple event bus subscribers
///
/// Provides lifecycle management for subscribers including:
/// - Registration
/// - Concurrent startup
/// - Graceful shutdown
/// - Health monitoring
pub struct SubscriberCoordinator {
    /// The event bus that subscribers consume from
    event_bus: Arc<EventBus>,
    /// Registered subscribers (not yet started)
    subscribers: Vec<Box<dyn Subscriber>>,
    /// Running subscriber tasks
    running_tasks: Vec<(String, JoinHandle<Result<(), SubscriberError>>)>,
    /// Current state
    state: CoordinatorState,
}

impl SubscriberCoordinator {
    /// Create a new coordinator with an event bus
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        debug!("Creating SubscriberCoordinator");
        Self {
            event_bus,
            subscribers: Vec::new(),
            running_tasks: Vec::new(),
            state: CoordinatorState::Idle,
        }
    }

    /// Register a subscriber with the coordinator
    ///
    /// # Arguments
    /// * `subscriber` - The subscriber to register
    ///
    /// # Errors
    /// Returns error if a subscriber with the same ID is already registered
    /// or if the coordinator is not in Idle state.
    pub fn register(&mut self, subscriber: Box<dyn Subscriber>) -> Result<(), SubscriberError> {
        if self.state != CoordinatorState::Idle {
            return Err(SubscriberError::ConfigError(format!(
                "Cannot register subscriber while coordinator is {:?}",
                self.state
            )));
        }

        let id = subscriber.id().to_string();

        // Check for duplicate IDs
        if self.subscribers.iter().any(|s| s.id() == id) {
            return Err(SubscriberError::ConfigError(format!(
                "Subscriber with ID '{}' already registered",
                id
            )));
        }

        info!(subscriber_id = %id, "Registering subscriber");
        self.subscribers.push(subscriber);
        Ok(())
    }

    /// Get the number of registered subscribers
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len() + self.running_tasks.len()
    }

    /// Get the current coordinator state
    pub fn state(&self) -> CoordinatorState {
        self.state
    }

    /// Start all registered subscribers
    ///
    /// Each subscriber is spawned in its own tokio task.
    ///
    /// # Errors
    /// Returns error if coordinator is not in Idle state.
    pub async fn start_all(&mut self) -> Result<(), SubscriberError> {
        if self.state != CoordinatorState::Idle {
            return Err(SubscriberError::StartupFailed(format!(
                "Cannot start: coordinator is {:?}",
                self.state
            )));
        }

        if self.subscribers.is_empty() {
            warn!("No subscribers registered, nothing to start");
            self.state = CoordinatorState::Running;
            return Ok(());
        }

        info!(
            subscriber_count = self.subscribers.len(),
            "Starting all subscribers"
        );

        self.state = CoordinatorState::Running;

        // Drain subscribers and spawn each in its own task
        let subscribers = std::mem::take(&mut self.subscribers);
        for mut subscriber in subscribers {
            let id = subscriber.id().to_string();
            let receiver = self.event_bus.subscribe();

            info!(subscriber_id = %id, "Spawning subscriber task");

            let handle = tokio::spawn(async move { subscriber.start(receiver).await });

            self.running_tasks.push((id, handle));
        }

        info!(
            running_count = self.running_tasks.len(),
            "All subscribers started"
        );

        Ok(())
    }

    /// Stop all running subscribers gracefully
    ///
    /// Waits for all subscriber tasks to complete.
    pub async fn stop_all(&mut self) -> Result<(), SubscriberError> {
        if self.state != CoordinatorState::Running {
            return Err(SubscriberError::ShutdownFailed(format!(
                "Cannot stop: coordinator is {:?}",
                self.state
            )));
        }

        info!(
            running_count = self.running_tasks.len(),
            "Stopping all subscribers"
        );

        self.state = CoordinatorState::Stopping;

        // Note: Actual subscriber shutdown is triggered by dropping the EventBus
        // or by each subscriber's cancellation token. We just wait for tasks here.

        let tasks = std::mem::take(&mut self.running_tasks);
        let mut errors = Vec::new();

        for (id, handle) in tasks {
            debug!(subscriber_id = %id, "Waiting for subscriber to stop");
            match handle.await {
                Ok(Ok(())) => {
                    info!(subscriber_id = %id, "Subscriber stopped successfully");
                }
                Ok(Err(e)) => {
                    error!(subscriber_id = %id, error = %e, "Subscriber stopped with error");
                    errors.push(format!("{}: {}", id, e));
                }
                Err(e) => {
                    error!(subscriber_id = %id, error = %e, "Subscriber task panicked");
                    errors.push(format!("{}: task panicked - {}", id, e));
                }
            }
        }

        self.state = CoordinatorState::Stopped;

        if errors.is_empty() {
            info!("All subscribers stopped successfully");
            Ok(())
        } else {
            Err(SubscriberError::ShutdownFailed(errors.join("; ")))
        }
    }

    /// Check health of all subscribers
    ///
    /// Note: This requires access to the subscribers which are moved when started.
    /// For running subscribers, we track their task handles instead.
    pub async fn health_check(&self) -> CoordinatorHealth {
        let subscriber_count = self.subscriber_count();
        let running_count = self.running_tasks.len();

        // Check if any tasks have finished unexpectedly
        let mut subscriber_health = HashMap::new();
        let mut all_healthy = true;

        for (id, handle) in &self.running_tasks {
            if handle.is_finished() {
                all_healthy = false;
                let mut details = HashMap::new();
                details.insert("status".to_string(), "task_finished".to_string());
                subscriber_health.insert(
                    id.clone(),
                    HealthStatus {
                        healthy: false,
                        message: "Subscriber task has finished unexpectedly".to_string(),
                        details,
                    },
                );
            } else {
                let mut details = HashMap::new();
                details.insert("status".to_string(), "running".to_string());
                subscriber_health.insert(
                    id.clone(),
                    HealthStatus {
                        healthy: true,
                        message: "Subscriber task is running".to_string(),
                        details,
                    },
                );
            }
        }

        CoordinatorHealth {
            overall_healthy: all_healthy && self.state == CoordinatorState::Running,
            subscriber_count,
            running_count,
            subscriber_health,
        }
    }

    /// Get IDs of all registered/running subscribers
    pub fn subscriber_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .subscribers
            .iter()
            .map(|s| s.id().to_string())
            .collect();
        ids.extend(self.running_tasks.iter().map(|(id, _)| id.clone()));
        ids
    }
}

impl std::fmt::Debug for SubscriberCoordinator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubscriberCoordinator")
            .field("state", &self.state)
            .field("registered_subscribers", &self.subscribers.len())
            .field("running_tasks", &self.running_tasks.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::EventBusConfig;
    use crate::types::RawDataPoint;
    use serde_json::json;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::sync::broadcast;
    use tokio_util::sync::CancellationToken;

    // ========== TEST HELPERS ==========

    /// A simple test subscriber that counts events
    struct TestSubscriber {
        id: String,
        started: Arc<AtomicBool>,
        stopped: Arc<AtomicBool>,
        events_received: Arc<AtomicUsize>,
        should_fail_start: bool,
        should_fail_stop: bool,
        cancellation_token: CancellationToken,
    }

    impl TestSubscriber {
        fn new(id: impl Into<String>) -> Self {
            Self {
                id: id.into(),
                started: Arc::new(AtomicBool::new(false)),
                stopped: Arc::new(AtomicBool::new(false)),
                events_received: Arc::new(AtomicUsize::new(0)),
                should_fail_start: false,
                should_fail_stop: false,
                cancellation_token: CancellationToken::new(),
            }
        }

        fn with_fail_start(mut self) -> Self {
            self.should_fail_start = true;
            self
        }

        fn started(&self) -> bool {
            self.started.load(Ordering::SeqCst)
        }

        fn stopped(&self) -> bool {
            self.stopped.load(Ordering::SeqCst)
        }

        fn events_count(&self) -> usize {
            self.events_received.load(Ordering::SeqCst)
        }

        fn cancellation_token(&self) -> CancellationToken {
            self.cancellation_token.clone()
        }
    }

    #[async_trait::async_trait]
    impl Subscriber for TestSubscriber {
        fn id(&self) -> &str {
            &self.id
        }

        async fn start(
            &mut self,
            mut receiver: broadcast::Receiver<Arc<RawDataPoint>>,
        ) -> Result<(), SubscriberError> {
            if self.should_fail_start {
                return Err(SubscriberError::StartupFailed("Test failure".to_string()));
            }

            self.started.store(true, Ordering::SeqCst);

            // Receive loop with cancellation support
            loop {
                tokio::select! {
                    _ = self.cancellation_token.cancelled() => {
                        break;
                    }
                    result = receiver.recv() => {
                        match result {
                            Ok(_) => {
                                self.events_received.fetch_add(1, Ordering::SeqCst);
                            }
                            Err(broadcast::error::RecvError::Lagged(_)) => {
                                continue;
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                break;
                            }
                        }
                    }
                }
            }

            Ok(())
        }

        async fn stop(&mut self) -> Result<(), SubscriberError> {
            if self.should_fail_stop {
                return Err(SubscriberError::ShutdownFailed("Test failure".to_string()));
            }
            self.stopped.store(true, Ordering::SeqCst);
            Ok(())
        }

        fn accepts_stream(&self, _stream_id: &str) -> bool {
            true
        }

        async fn health_check(&self) -> HealthStatus {
            HealthStatus {
                healthy: self.started.load(Ordering::SeqCst),
                message: "Test subscriber".to_string(),
                details: HashMap::new(),
            }
        }
    }

    fn create_test_event_bus() -> Arc<EventBus> {
        Arc::new(EventBus::new(EventBusConfig {
            capacity: 100,
            ..Default::default()
        }))
    }

    // ========== TDD CYCLE 1: Coordinator Creation ==========

    #[test]
    fn test_coordinator_creation() {
        let event_bus = create_test_event_bus();
        let coordinator = SubscriberCoordinator::new(event_bus);

        assert_eq!(coordinator.state(), CoordinatorState::Idle);
        assert_eq!(coordinator.subscriber_count(), 0);
    }

    #[test]
    fn test_coordinator_debug_impl() {
        let event_bus = create_test_event_bus();
        let coordinator = SubscriberCoordinator::new(event_bus);

        let debug_str = format!("{:?}", coordinator);
        assert!(debug_str.contains("SubscriberCoordinator"));
        assert!(debug_str.contains("Idle"));
    }

    // ========== TDD CYCLE 2: Registration ==========

    #[test]
    fn test_register_subscriber() {
        let event_bus = create_test_event_bus();
        let mut coordinator = SubscriberCoordinator::new(event_bus);

        let subscriber = TestSubscriber::new("test-1");
        let result = coordinator.register(Box::new(subscriber));

        assert!(result.is_ok());
        assert_eq!(coordinator.subscriber_count(), 1);
    }

    #[test]
    fn test_register_multiple_subscribers() {
        let event_bus = create_test_event_bus();
        let mut coordinator = SubscriberCoordinator::new(event_bus);

        coordinator
            .register(Box::new(TestSubscriber::new("test-1")))
            .unwrap();
        coordinator
            .register(Box::new(TestSubscriber::new("test-2")))
            .unwrap();
        coordinator
            .register(Box::new(TestSubscriber::new("test-3")))
            .unwrap();

        assert_eq!(coordinator.subscriber_count(), 3);
        assert!(coordinator.subscriber_ids().contains(&"test-1".to_string()));
        assert!(coordinator.subscriber_ids().contains(&"test-2".to_string()));
        assert!(coordinator.subscriber_ids().contains(&"test-3".to_string()));
    }

    #[test]
    fn test_register_duplicate_id_fails() {
        let event_bus = create_test_event_bus();
        let mut coordinator = SubscriberCoordinator::new(event_bus);

        coordinator
            .register(Box::new(TestSubscriber::new("test-1")))
            .unwrap();
        let result = coordinator.register(Box::new(TestSubscriber::new("test-1")));

        assert!(result.is_err());
        match result.unwrap_err() {
            SubscriberError::ConfigError(msg) => {
                assert!(msg.contains("already registered"));
            }
            e => panic!("Expected ConfigError, got {:?}", e),
        }
    }

    // ========== TDD CYCLE 3: Start All ==========

    #[tokio::test]
    async fn test_start_all_empty_succeeds() {
        let event_bus = create_test_event_bus();
        let mut coordinator = SubscriberCoordinator::new(event_bus);

        let result = coordinator.start_all().await;

        assert!(result.is_ok());
        assert_eq!(coordinator.state(), CoordinatorState::Running);
    }

    #[tokio::test]
    async fn test_start_all_with_subscribers() {
        let event_bus = create_test_event_bus();
        let mut coordinator = SubscriberCoordinator::new(event_bus.clone());

        let subscriber1 = TestSubscriber::new("test-1");
        let cancel1 = subscriber1.cancellation_token();
        let subscriber2 = TestSubscriber::new("test-2");
        let cancel2 = subscriber2.cancellation_token();

        coordinator.register(Box::new(subscriber1)).unwrap();
        coordinator.register(Box::new(subscriber2)).unwrap();

        let result = coordinator.start_all().await;

        assert!(result.is_ok());
        assert_eq!(coordinator.state(), CoordinatorState::Running);

        // Subscribers are moved to tasks, so running_tasks should have entries
        assert_eq!(coordinator.running_tasks.len(), 2);

        // Clean up: cancel subscribers so tasks exit
        cancel1.cancel();
        cancel2.cancel();
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    #[tokio::test]
    async fn test_start_all_twice_fails() {
        let event_bus = create_test_event_bus();
        let mut coordinator = SubscriberCoordinator::new(event_bus);

        coordinator.start_all().await.unwrap();
        let result = coordinator.start_all().await;

        assert!(result.is_err());
        match result.unwrap_err() {
            SubscriberError::StartupFailed(msg) => {
                assert!(msg.contains("Running"));
            }
            e => panic!("Expected StartupFailed, got {:?}", e),
        }
    }

    // ========== TDD CYCLE 4: Stop All ==========

    #[tokio::test]
    async fn test_stop_all() {
        let event_bus = create_test_event_bus();
        let mut coordinator = SubscriberCoordinator::new(event_bus.clone());

        let subscriber = TestSubscriber::new("test-1");
        let cancel = subscriber.cancellation_token();
        coordinator.register(Box::new(subscriber)).unwrap();
        coordinator.start_all().await.unwrap();

        // Signal the subscriber to stop via cancellation token
        cancel.cancel();

        // Give tasks time to notice cancellation
        tokio::time::sleep(Duration::from_millis(100)).await;

        let result = coordinator.stop_all().await;

        assert!(result.is_ok());
        assert_eq!(coordinator.state(), CoordinatorState::Stopped);
    }

    #[tokio::test]
    async fn test_stop_all_when_not_running_fails() {
        let event_bus = create_test_event_bus();
        let mut coordinator = SubscriberCoordinator::new(event_bus);

        let result = coordinator.stop_all().await;

        assert!(result.is_err());
        match result.unwrap_err() {
            SubscriberError::ShutdownFailed(msg) => {
                assert!(msg.contains("Idle"));
            }
            e => panic!("Expected ShutdownFailed, got {:?}", e),
        }
    }

    // ========== TDD CYCLE 5: Health Check ==========

    #[tokio::test]
    async fn test_health_check_no_subscribers() {
        let event_bus = create_test_event_bus();
        let coordinator = SubscriberCoordinator::new(event_bus);

        let health = coordinator.health_check().await;

        assert_eq!(health.subscriber_count, 0);
        assert_eq!(health.running_count, 0);
    }

    #[tokio::test]
    async fn test_health_check_running_subscribers() {
        let event_bus = create_test_event_bus();
        let mut coordinator = SubscriberCoordinator::new(event_bus.clone());

        let sub1 = TestSubscriber::new("test-1");
        let cancel1 = sub1.cancellation_token();
        let sub2 = TestSubscriber::new("test-2");
        let cancel2 = sub2.cancellation_token();

        coordinator.register(Box::new(sub1)).unwrap();
        coordinator.register(Box::new(sub2)).unwrap();
        coordinator.start_all().await.unwrap();

        // Give tasks time to start
        tokio::time::sleep(Duration::from_millis(50)).await;

        let health = coordinator.health_check().await;

        assert!(health.overall_healthy);
        assert_eq!(health.running_count, 2);
        assert!(health.subscriber_health.contains_key("test-1"));
        assert!(health.subscriber_health.contains_key("test-2"));

        // Clean up: cancel subscribers so tasks exit
        cancel1.cancel();
        cancel2.cancel();
    }

    #[tokio::test]
    async fn test_health_check_detects_finished_tasks() {
        let event_bus = create_test_event_bus();
        let mut coordinator = SubscriberCoordinator::new(event_bus.clone());

        let subscriber = TestSubscriber::new("test-1");
        let cancel = subscriber.cancellation_token();
        coordinator.register(Box::new(subscriber)).unwrap();
        coordinator.start_all().await.unwrap();

        // Cancel the subscriber so its task finishes
        cancel.cancel();

        // Wait for task to finish
        tokio::time::sleep(Duration::from_millis(200)).await;

        let health = coordinator.health_check().await;

        // Task finished should be detected as unhealthy
        assert!(!health.overall_healthy);
        let sub_health = health.subscriber_health.get("test-1").unwrap();
        assert!(!sub_health.healthy);
    }

    // ========== TDD CYCLE 6: Integration ==========

    #[tokio::test]
    async fn test_coordinator_full_lifecycle() {
        let event_bus = create_test_event_bus();
        let mut coordinator = SubscriberCoordinator::new(event_bus.clone());

        // Register (capture cancellation tokens before moving subscribers)
        let bronze_sub = TestSubscriber::new("bronze");
        let bronze_cancel = bronze_sub.cancellation_token();
        coordinator.register(Box::new(bronze_sub)).unwrap();

        let silver_sub = TestSubscriber::new("silver");
        let silver_cancel = silver_sub.cancellation_token();
        coordinator.register(Box::new(silver_sub)).unwrap();

        assert_eq!(coordinator.subscriber_count(), 2);
        assert_eq!(coordinator.state(), CoordinatorState::Idle);

        // Start
        coordinator.start_all().await.unwrap();
        assert_eq!(coordinator.state(), CoordinatorState::Running);

        // Health check
        tokio::time::sleep(Duration::from_millis(50)).await;
        let health = coordinator.health_check().await;
        assert!(health.overall_healthy);
        assert_eq!(health.running_count, 2);

        // Publish events
        use crate::types::RawDataPoint;
        for i in 0..5 {
            let point = Arc::new(RawDataPoint::new("test-source", json!({"seq": i})));
            event_bus.publish(point).unwrap();
        }

        // Give time for processing
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Signal subscribers to stop via cancellation tokens
        bronze_cancel.cancel();
        silver_cancel.cancel();

        // Give tasks time to notice cancellation
        tokio::time::sleep(Duration::from_millis(100)).await;

        let result = coordinator.stop_all().await;
        assert!(result.is_ok());
        assert_eq!(coordinator.state(), CoordinatorState::Stopped);
    }

    // ========== TDD CYCLE 7: CoordinatorHealth ==========

    #[test]
    fn test_coordinator_health_default() {
        let health = CoordinatorHealth::default();

        assert!(health.overall_healthy);
        assert_eq!(health.subscriber_count, 0);
        assert_eq!(health.running_count, 0);
        assert!(health.subscriber_health.is_empty());
    }
}
