//! Event system for Neural Trader V2
//! Module size: <150 lines as per requirements

pub mod traits;
pub mod market_events;
pub mod prediction_events;

// Re-exports
pub use traits::{Event, EventBus};
pub use market_events::{MarketEvent, PriceUpdateEvent, VolumeEvent, TrendChangeEvent};
pub use prediction_events::{PredictionEvent, ModelUpdateEvent, ModelPerformanceEvent};

use crate::errors::{CoreError, Result};
use async_trait::async_trait;
use dashmap::DashMap;
use futures::stream::{Stream, StreamExt};
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

/// Event subscription handle
#[derive(Debug, Clone)]
pub struct SubscriptionHandle {
    pub id: Uuid,
    pub event_type: String,
}

/// In-memory event bus implementation
pub struct InMemoryEventBus {
    subscribers: Arc<DashMap<String, broadcast::Sender<Arc<dyn Event + Send + Sync>>>>,
    buffer_size: usize,
}

impl InMemoryEventBus {
    /// Create new event bus with default buffer size
    pub fn new() -> Self {
        Self::with_buffer_size(1000)
    }
    
    /// Create event bus with custom buffer size
    pub fn with_buffer_size(buffer_size: usize) -> Self {
        Self {
            subscribers: Arc::new(DashMap::new()),
            buffer_size,
        }
    }
    
    /// Get or create broadcaster for event type
    fn get_or_create_broadcaster(&self, event_type: &str) -> broadcast::Sender<Arc<dyn Event + Send + Sync>> {
        if let Some(broadcaster) = self.subscribers.get(event_type) {
            broadcaster.clone()
        } else {
            let (tx, _) = broadcast::channel(self.buffer_size);
            self.subscribers.insert(event_type.to_string(), tx.clone());
            tx
        }
    }
}

#[async_trait]
impl EventBus for InMemoryEventBus {
    async fn publish(&self, event: Arc<dyn Event + Send + Sync>) -> Result<()> {
        let event_type = event.event_type();
        let broadcaster = self.get_or_create_broadcaster(&event_type);
        
        broadcaster.send(event)
            .map_err(|e| CoreError::EventError(format!("Failed to publish event: {}", e)))?;
        
        Ok(())
    }
    
    async fn subscribe(&self, event_type: &str) -> Result<SubscriptionHandle> {
        let _broadcaster = self.get_or_create_broadcaster(event_type);
        let handle = SubscriptionHandle {
            id: Uuid::new_v4(),
            event_type: event_type.to_string(),
        };
        
        Ok(handle)
    }
    
    async fn unsubscribe(&self, _handle: SubscriptionHandle) -> Result<()> {
        // In this simple implementation, we don't track individual subscribers
        // In production, you'd want to track and remove specific receivers
        Ok(())
    }
    
    async fn get_stream(&self, event_type: &str) -> Result<std::pin::Pin<Box<dyn Stream<Item = Arc<dyn Event + Send + Sync>> + Send>>> {
        let broadcaster = self.get_or_create_broadcaster(event_type);
        let receiver = broadcaster.subscribe();
        
        let stream = tokio_stream::wrappers::BroadcastStream::new(receiver)
            .filter_map(|result| async move { result.ok() });
            
        Ok(Box::pin(stream))
    }
}

impl Default for InMemoryEventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::market_events::PriceUpdateEvent;

    #[tokio::test]
    async fn test_event_bus_publish_subscribe() {
        let bus = InMemoryEventBus::new();
        
        // Subscribe first to ensure receiver exists
        let handle = bus.subscribe("price_update").await.unwrap();
        assert_eq!(handle.event_type, "price_update");
        
        // Create a receiver to keep the channel open
        let broadcaster = bus.get_or_create_broadcaster("price_update");
        let _receiver = broadcaster.subscribe();
        
        let event = Arc::new(PriceUpdateEvent::new(
            "AAPL".to_string(),
            150.0,
            149.5,
        ));
        
        // Test publishing
        bus.publish(event.clone()).await.unwrap();
        
        // Test unsubscribe
        bus.unsubscribe(handle).await.unwrap();
    }
}