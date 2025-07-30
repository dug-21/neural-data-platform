//! Event Bus Implementation for Integration Architecture
//!
//! Provides high-performance event distribution using broadcast channels
//! with persistence, metrics, and subscriber management.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;
use tracing::{debug, error, warn};

/// Core event bus for high-performance event distribution
pub struct EventBus<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// High-performance broadcast sender
    sender: broadcast::Sender<T>,
    
    /// Event persistence store for replay/debugging  
    event_store: Arc<RwLock<VecDeque<T>>>,
    
    /// Subscriber management
    subscribers: Arc<RwLock<HashMap<String, SubscriberInfo>>>,
    
    /// Performance metrics
    metrics: Arc<RwLock<EventBusMetrics>>,
    
    /// Configuration
    config: EventBusConfig,
}

/// Event bus configuration
#[derive(Debug, Clone)]
pub struct EventBusConfig {
    /// Maximum number of events to store in memory
    pub max_stored_events: usize,
    
    /// Channel capacity for broadcast
    pub channel_capacity: usize,
    
    /// Enable detailed metrics collection
    pub enable_metrics: bool,
    
    /// Enable event persistence
    pub enable_persistence: bool,
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            max_stored_events: 10_000,
            channel_capacity: 1_000,
            enable_metrics: true,
            enable_persistence: true,
        }
    }
}

/// Subscriber information for tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriberInfo {
    pub id: String,
    pub subscribed_at: DateTime<Utc>,
    pub message_count: u64,
    pub last_message_at: Option<DateTime<Utc>>,
    pub subscriber_type: String,
}

/// Event bus performance metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventBusMetrics {
    pub total_events_published: u64,
    pub total_events_delivered: u64,
    pub failed_deliveries: u64,
    pub active_subscribers: usize,
    pub average_latency_ms: f64,
    pub events_per_second: f64,
    pub last_reset: DateTime<Utc>,
    pub peak_events_per_second: f64,
    pub total_bytes_transmitted: u64,
}

impl<T> EventBus<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// Create new event bus with configuration
    pub fn new(config: EventBusConfig) -> Self {
        let (sender, _) = broadcast::channel(config.channel_capacity);
        
        Self {
            sender,
            event_store: Arc::new(RwLock::new(VecDeque::with_capacity(config.max_stored_events))),
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(EventBusMetrics {
                last_reset: Utc::now(),
                ..Default::default()
            })),
            config,
        }
    }
    
    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(EventBusConfig::default())
    }
    
    /// Publish event to all subscribers
    pub async fn publish(&self, event: T) -> Result<usize> {
        let start_time = std::time::Instant::now();
        
        // Send to all subscribers
        let subscriber_count = match self.sender.send(event.clone()) {
            Ok(count) => count,
            Err(_) => {
                // Channel might be closed or no receivers
                warn!("Event bus publish failed - no active receivers");
                return Ok(0);
            }
        };
        
        // Store event if persistence is enabled
        if self.config.enable_persistence {
            self.store_event(event).await;
        }
        
        // Update metrics if enabled
        if self.config.enable_metrics {
            let latency = start_time.elapsed().as_millis() as f64;
            self.update_metrics(subscriber_count, latency).await;
        }
        
        debug!("Published event to {} subscribers", subscriber_count);
        Ok(subscriber_count)
    }
    
    /// Subscribe to events with subscriber tracking
    pub fn subscribe(&self, subscriber_id: String, subscriber_type: String) -> broadcast::Receiver<T> {
        let receiver = self.sender.subscribe();
        
        // Track subscriber
        let subscriber_info = SubscriberInfo {
            id: subscriber_id.clone(),
            subscribed_at: Utc::now(),
            message_count: 0,
            last_message_at: None,
            subscriber_type,
        };
        
        if let Ok(mut subscribers) = self.subscribers.write() {
            subscribers.insert(subscriber_id, subscriber_info);
        }
        
        // Update active subscriber count in metrics
        if self.config.enable_metrics {
            if let Ok(mut metrics) = self.metrics.write() {
                metrics.active_subscribers = self.subscribers.read().unwrap().len();
            }
        }
        
        receiver
    }
    
    /// Unsubscribe a specific subscriber
    pub fn unsubscribe(&self, subscriber_id: &str) -> Result<()> {
        if let Ok(mut subscribers) = self.subscribers.write() {
            subscribers.remove(subscriber_id);
        }
        
        // Update metrics
        if self.config.enable_metrics {
            if let Ok(mut metrics) = self.metrics.write() {
                metrics.active_subscribers = self.subscribers.read().unwrap().len();
            }
        }
        
        Ok(())
    }
    
    /// Get recent events from store
    pub fn get_recent_events(&self, count: usize) -> Vec<T> {
        if let Ok(store) = self.event_store.read() {
            store.iter()
                .rev()
                .take(count)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }
    
    /// Get current metrics
    pub fn get_metrics(&self) -> EventBusMetrics {
        if let Ok(metrics) = self.metrics.read() {
            metrics.clone()
        } else {
            EventBusMetrics::default()
        }
    }
    
    /// Reset metrics counters
    pub fn reset_metrics(&self) {
        if let Ok(mut metrics) = self.metrics.write() {
            *metrics = EventBusMetrics {
                last_reset: Utc::now(),
                active_subscribers: metrics.active_subscribers,
                ..Default::default()
            };
        }
    }
    
    /// Get subscriber information
    pub fn get_subscribers(&self) -> HashMap<String, SubscriberInfo> {
        if let Ok(subscribers) = self.subscribers.read() {
            subscribers.clone()
        } else {
            HashMap::new()
        }
    }
    
    /// Clear event store
    pub fn clear_event_store(&self) {
        if let Ok(mut store) = self.event_store.write() {
            store.clear();
        }
    }
    
    /// Get event store size
    pub fn event_store_size(&self) -> usize {
        if let Ok(store) = self.event_store.read() {
            store.len()
        } else {
            0
        }
    }
    
    /// Store event in persistence layer
    async fn store_event(&self, event: T) {
        if let Ok(mut store) = self.event_store.write() {
            // Add new event
            store.push_back(event);
            
            // Remove old events if over capacity
            while store.len() > self.config.max_stored_events {
                store.pop_front();
            }
        }
    }
    
    /// Update performance metrics
    async fn update_metrics(&self, subscriber_count: usize, latency_ms: f64) {
        if let Ok(mut metrics) = self.metrics.write() {
            metrics.total_events_published += 1;
            metrics.total_events_delivered += subscriber_count as u64;
            
            // Update latency (exponential moving average)
            metrics.average_latency_ms = if metrics.total_events_published == 1 {
                latency_ms
            } else {
                metrics.average_latency_ms * 0.9 + latency_ms * 0.1
            };
            
            // Calculate events per second
            let elapsed_secs = (Utc::now() - metrics.last_reset).num_seconds() as f64;
            if elapsed_secs > 0.0 {
                metrics.events_per_second = metrics.total_events_published as f64 / elapsed_secs;
                
                // Update peak
                if metrics.events_per_second > metrics.peak_events_per_second {
                    metrics.peak_events_per_second = metrics.events_per_second;
                }
            }
            
            // Update active subscribers
            metrics.active_subscribers = self.subscribers.read().unwrap().len();
        }
    }
}

impl<T> Clone for EventBus<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            event_store: Arc::clone(&self.event_store),
            subscribers: Arc::clone(&self.subscribers),
            metrics: Arc::clone(&self.metrics),
            config: self.config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};
    
    #[derive(Debug, Clone, PartialEq)]
    struct TestEvent {
        id: u64,
        message: String,
    }
    
    #[tokio::test]
    async fn test_event_bus_basic_functionality() {
        let event_bus = EventBus::with_defaults();
        
        // Subscribe to events
        let mut receiver = event_bus.subscribe("test_subscriber".to_string(), "test".to_string());
        
        // Publish event
        let test_event = TestEvent {
            id: 1,
            message: "Hello, World!".to_string(),
        };
        
        let subscriber_count = event_bus.publish(test_event.clone()).await.unwrap();
        assert_eq!(subscriber_count, 1);
        
        // Receive event
        let received_event = timeout(Duration::from_millis(100), receiver.recv())
            .await
            .expect("Timeout")
            .expect("Failed to receive");
        
        assert_eq!(received_event, test_event);
    }
    
    #[tokio::test]
    async fn test_multiple_subscribers() {
        let event_bus = EventBus::with_defaults();
        
        // Create multiple subscribers
        let mut receiver1 = event_bus.subscribe("subscriber1".to_string(), "type1".to_string());
        let mut receiver2 = event_bus.subscribe("subscriber2".to_string(), "type2".to_string());
        let mut receiver3 = event_bus.subscribe("subscriber3".to_string(), "type1".to_string());
        
        // Publish event
        let test_event = TestEvent {
            id: 42,
            message: "Broadcast test".to_string(),
        };
        
        let subscriber_count = event_bus.publish(test_event.clone()).await.unwrap();
        assert_eq!(subscriber_count, 3);
        
        // All receivers should get the event
        for receiver in [&mut receiver1, &mut receiver2, &mut receiver3] {
            let received = timeout(Duration::from_millis(100), receiver.recv())
                .await
                .expect("Timeout")
                .expect("Failed to receive");
            assert_eq!(received, test_event);
        }
    }
    
    #[tokio::test]
    async fn test_event_persistence() {
        let config = EventBusConfig {
            max_stored_events: 3,
            enable_persistence: true,
            ..Default::default()
        };
        
        let event_bus = EventBus::new(config);
        
        // Publish multiple events
        for i in 1..=5 {
            let event = TestEvent {
                id: i,
                message: format!("Event {}", i),
            };
            event_bus.publish(event).await.unwrap();
        }
        
        // Should only store the last 3 events
        let recent_events = event_bus.get_recent_events(10);
        assert_eq!(recent_events.len(), 3);
        
        // Should be in reverse order (most recent first)
        assert_eq!(recent_events[0].id, 5);
        assert_eq!(recent_events[1].id, 4);
        assert_eq!(recent_events[2].id, 3);
    }
    
    #[tokio::test]
    async fn test_metrics_collection() {
        let event_bus = EventBus::with_defaults();
        let _receiver = event_bus.subscribe("test".to_string(), "test".to_string());
        
        // Publish events
        for i in 1..=10 {
            let event = TestEvent {
                id: i,
                message: format!("Metrics test {}", i),
            };
            event_bus.publish(event).await.unwrap();
        }
        
        let metrics = event_bus.get_metrics();
        assert_eq!(metrics.total_events_published, 10);
        assert_eq!(metrics.total_events_delivered, 10);
        assert_eq!(metrics.active_subscribers, 1);
        assert!(metrics.average_latency_ms >= 0.0);
    }
    
    #[tokio::test]
    async fn test_subscriber_management() {
        let event_bus = EventBus::with_defaults();
        
        // Subscribe
        let _receiver = event_bus.subscribe("test_sub".to_string(), "test_type".to_string());
        
        // Check subscriber info
        let subscribers = event_bus.get_subscribers();
        assert_eq!(subscribers.len(), 1);
        assert!(subscribers.contains_key("test_sub"));
        
        let subscriber_info = &subscribers["test_sub"];
        assert_eq!(subscriber_info.id, "test_sub");
        assert_eq!(subscriber_info.subscriber_type, "test_type");
        assert_eq!(subscriber_info.message_count, 0);
        
        // Unsubscribe
        event_bus.unsubscribe("test_sub").unwrap();
        let subscribers_after = event_bus.get_subscribers();
        assert_eq!(subscribers_after.len(), 0);
    }
}