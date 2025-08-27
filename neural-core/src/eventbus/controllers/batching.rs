use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::sleep;

use crate::eventbus::types::Event;
use crate::eventbus::error::EventBusError;

/// Message batcher for efficient throughput
pub struct MessageBatcher {
    batch_configs: Arc<RwLock<HashMap<String, BatchConfig>>>,
    pending_batches: Arc<RwLock<HashMap<String, PendingBatch>>>,
    flush_handles: Arc<RwLock<HashMap<String, tokio::task::JoinHandle<()>>>>,
}

#[derive(Debug, Clone)]
pub struct BatchConfig {
    pub max_batch_size: usize,
    pub max_wait_ms: u64,
    pub compression_enabled: bool,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 100,
            max_wait_ms: 10,
            compression_enabled: false,
        }
    }
}

#[derive(Debug)]
struct PendingBatch {
    events: Vec<Event>,
    created_at: Instant,
    total_size_bytes: usize,
}

impl PendingBatch {
    fn new() -> Self {
        Self {
            events: Vec::new(),
            created_at: Instant::now(),
            total_size_bytes: 0,
        }
    }

    fn add(&mut self, event: Event) {
        // Estimate size (in production, would calculate actual serialized size)
        let event_size = event.payload.len() + event.event_type.len() + 100;
        self.total_size_bytes += event_size;
        self.events.push(event);
    }

    fn should_flush(&self, config: &BatchConfig) -> bool {
        self.events.len() >= config.max_batch_size ||
        self.created_at.elapsed().as_millis() >= config.max_wait_ms as u128
    }

    fn take_events(&mut self) -> Vec<Event> {
        self.total_size_bytes = 0;
        self.created_at = Instant::now();
        std::mem::take(&mut self.events)
    }
}

pub enum BatchDisposition {
    FlushNow(Vec<Event>),
    Pending,
}

impl MessageBatcher {
    pub fn new() -> Self {
        Self {
            batch_configs: Arc::new(RwLock::new(HashMap::new())),
            pending_batches: Arc::new(RwLock::new(HashMap::new())),
            flush_handles: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn set_config(&self, channel: &str, config: BatchConfig) {
        let mut configs = self.batch_configs.write().await;
        configs.insert(channel.to_string(), config);
    }

    pub async fn add_event(
        &self,
        channel: &str,
        event: Event,
    ) -> Result<BatchDisposition, EventBusError> {
        let config = self.get_config(channel).await;
        
        let mut pending_batches = self.pending_batches.write().await;
        let batch = pending_batches.entry(channel.to_string())
            .or_insert_with(PendingBatch::new);
        
        batch.add(event);
        
        if batch.should_flush(&config) {
            let events = batch.take_events();
            drop(pending_batches);  // Release lock early
            
            // Cancel any pending flush timer
            self.cancel_flush_timer(channel).await;
            
            Ok(BatchDisposition::FlushNow(events))
        } else {
            // Schedule flush timer if not already scheduled
            if !self.has_flush_timer(channel).await {
                self.schedule_flush(channel.to_string(), config.max_wait_ms).await;
            }
            Ok(BatchDisposition::Pending)
        }
    }

    pub async fn force_flush(&self, channel: &str) -> Option<Vec<Event>> {
        let mut pending_batches = self.pending_batches.write().await;
        pending_batches.get_mut(channel)
            .map(|batch| batch.take_events())
            .filter(|events| !events.is_empty())
    }

    pub async fn flush_all(&self) -> HashMap<String, Vec<Event>> {
        let mut result = HashMap::new();
        let mut pending_batches = self.pending_batches.write().await;
        
        for (channel, batch) in pending_batches.iter_mut() {
            let events = batch.take_events();
            if !events.is_empty() {
                result.insert(channel.clone(), events);
            }
        }
        
        result
    }

    async fn get_config(&self, channel: &str) -> BatchConfig {
        let configs = self.batch_configs.read().await;
        configs.get(channel).cloned().unwrap_or_default()
    }

    async fn has_flush_timer(&self, channel: &str) -> bool {
        let handles = self.flush_handles.read().await;
        handles.contains_key(channel)
    }

    async fn cancel_flush_timer(&self, channel: &str) {
        let mut handles = self.flush_handles.write().await;
        if let Some(handle) = handles.remove(channel) {
            handle.abort();
        }
    }

    async fn schedule_flush(&self, channel: String, wait_ms: u64) {
        let pending_batches = self.pending_batches.clone();
        let flush_handles = self.flush_handles.clone();
        let channel_key = channel.clone();
        
        let handle = tokio::spawn(async move {
            sleep(Duration::from_millis(wait_ms)).await;
            
            // Time to flush
            let mut batches = pending_batches.write().await;
            if let Some(batch) = batches.get_mut(&channel_key) {
                let _events = batch.take_events();
                // In a real implementation, would trigger flush callback here
            }
            
            // Remove self from handles
            let mut handles = flush_handles.write().await;
            handles.remove(&channel_key);
        });
        
        let mut handles = self.flush_handles.write().await;
        handles.insert(channel, handle);
    }

    pub async fn get_pending_count(&self, channel: &str) -> usize {
        let pending_batches = self.pending_batches.read().await;
        pending_batches.get(channel)
            .map(|batch| batch.events.len())
            .unwrap_or(0)
    }

    pub async fn get_pending_size_bytes(&self, channel: &str) -> usize {
        let pending_batches = self.pending_batches.read().await;
        pending_batches.get(channel)
            .map(|batch| batch.total_size_bytes)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_event(_id: &str) -> Event {
        Event {
            event_type: "TestEvent".to_string(),
            payload: vec![1, 2, 3],
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    #[tokio::test]
    async fn test_batch_by_size() {
        let batcher = MessageBatcher::new();
        
        batcher.set_config(
            "stream:symbol:AAPL",
            BatchConfig {
                max_batch_size: 3,
                max_wait_ms: 1000,
                compression_enabled: false,
            },
        ).await;
        
        // Add events that don't reach batch size
        for i in 0..2 {
            let result = batcher.add_event(
                "stream:symbol:AAPL",
                create_test_event(&i.to_string()),
            ).await.unwrap();
            
            assert!(matches!(result, BatchDisposition::Pending));
        }
        
        // Add event that triggers batch
        let result = batcher.add_event(
            "stream:symbol:AAPL",
            create_test_event("3"),
        ).await.unwrap();
        
        if let BatchDisposition::FlushNow(events) = result {
            assert_eq!(events.len(), 3);
        } else {
            panic!("Expected FlushNow");
        }
    }

    #[tokio::test]
    async fn test_force_flush() {
        let batcher = MessageBatcher::new();
        
        // Add some events
        for i in 0..2 {
            batcher.add_event(
                "stream:symbol:AAPL",
                create_test_event(&i.to_string()),
            ).await.unwrap();
        }
        
        // Force flush
        let flushed = batcher.force_flush("stream:symbol:AAPL").await;
        assert!(flushed.is_some());
        assert_eq!(flushed.unwrap().len(), 2);
        
        // Verify batch is empty
        assert_eq!(batcher.get_pending_count("stream:symbol:AAPL").await, 0);
    }

    #[tokio::test]
    async fn test_flush_all() {
        let batcher = MessageBatcher::new();
        
        // Add events to multiple channels
        batcher.add_event("stream:symbol:AAPL", create_test_event("1")).await.unwrap();
        batcher.add_event("stream:symbol:MSFT", create_test_event("2")).await.unwrap();
        batcher.add_event("stream:symbol:GOOGL", create_test_event("3")).await.unwrap();
        
        // Flush all
        let all_flushed = batcher.flush_all().await;
        assert_eq!(all_flushed.len(), 3);
        assert!(all_flushed.contains_key("stream:symbol:AAPL"));
        assert!(all_flushed.contains_key("stream:symbol:MSFT"));
        assert!(all_flushed.contains_key("stream:symbol:GOOGL"));
    }
}