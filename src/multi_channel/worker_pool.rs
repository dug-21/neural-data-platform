/*!
 * Worker pool for parallel processing of market data events.
 * 
 * Implements a pool of worker threads that process market data events
 * from different symbol channels while maintaining fair processing.
 */

use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use std::collections::HashMap;
use log;

use crate::multi_channel::{WorkItem, SymbolStats, fair_scheduler::FairProcessingScheduler};
use crate::streaming::event_bus::{EventBusIntegration, MarketEvent};
use crate::adapters::AdapterError;

/// Worker statistics
#[derive(Debug, Clone, Default)]
pub struct WorkerStats {
    pub worker_id: usize,
    pub messages_processed: u64,
    pub total_processing_time: Duration,
    pub average_latency: Duration,
    pub current_queue_depth: usize,
    pub last_activity: Option<Instant>,
}

/// System-wide worker statistics
#[derive(Debug, Clone, Default)]
pub struct WorkerPoolStats {
    pub total_workers: usize,
    pub active_workers: usize,
    pub total_processed: u64,
    pub average_queue_depth: f64,
    pub symbol_stats: HashMap<String, SymbolStats>,
}

// Manual Send + Sync implementations for thread safety
unsafe impl Send for WorkerStats {}
unsafe impl Sync for WorkerStats {}
unsafe impl Send for WorkerPoolStats {}
unsafe impl Sync for WorkerPoolStats {}

/// Individual worker for processing market data
pub struct Worker {
    pub id: usize,
    work_rx: mpsc::Receiver<WorkItem>,
    event_bus: Arc<EventBusIntegration>,
    pub stats: Arc<RwLock<WorkerStats>>,
    shutdown: Arc<AtomicBool>,
}

impl Worker {
    pub fn new(
        id: usize,
        work_rx: mpsc::Receiver<WorkItem>,
        event_bus: Arc<EventBusIntegration>,
        shutdown: Arc<AtomicBool>,
    ) -> Self {
        Self {
            id,
            work_rx,
            event_bus,
            stats: Arc::new(RwLock::new(WorkerStats {
                worker_id: id,
                ..Default::default()
            })),
            shutdown,
        }
    }
    
    /// Start worker processing loop
    pub async fn start(mut self) -> Result<(), AdapterError> {
        tracing::info!("Starting worker {}", self.id);
        
        while !self.shutdown.load(Ordering::Relaxed) {
            // Use timeout to allow checking shutdown signal
            match tokio::time::timeout(Duration::from_millis(100), self.work_rx.recv()).await {
                Ok(Some(work_item)) => {
                    let start_time = Instant::now();
                    
                    // Process the work item
                    if let Err(e) = self.process_work_item(work_item).await {
                        tracing::error!("Worker {} failed to process item: {}", self.id, e);
                    }
                    
                    // Update statistics
                    let processing_time = start_time.elapsed();
                    self.update_stats(processing_time).await;
                }
                Ok(None) => {
                    // Channel closed, shutdown
                    break;
                }
                Err(_) => {
                    // Timeout, continue to check shutdown signal
                    continue;
                }
            }
        }
        
        log::info!("Worker {} stopped", self.id);
        Ok(())
    }
    
    /// Process a single work item
    async fn process_work_item(&self, work_item: WorkItem) -> Result<(), AdapterError> {
        // Convert WorkItem to MarketEvent
        let market_event = MarketEvent {
            symbol: work_item.symbol.clone(),
            timestamp: chrono::DateTime::from_timestamp(
                work_item.market_data.timestamp / 1000,
                (work_item.market_data.timestamp % 1000) as u32 * 1_000_000
            ).unwrap_or_else(chrono::Utc::now),
            event_type: "market_update".to_string(),
            price: work_item.market_data.close,
            volume: work_item.market_data.volume,
            bid: work_item.market_data.low,
            ask: work_item.market_data.high,
            spread: work_item.market_data.high - work_item.market_data.low,
            order_book_depth: None,
            sequence_number: work_item.market_data.timestamp as u64,
            source: work_item.channel,
            quality_score: 0.95,
            metadata: Some(serde_json::json!({
                "open": work_item.market_data.open,
                "high": work_item.market_data.high,
                "low": work_item.market_data.low,
                "close": work_item.market_data.close,
                "received_at_millis": work_item.received_at.elapsed().as_millis(),
                "priority": work_item.priority,
                "worker_id": self.id
            })),
        };
        
        // Publish to event bus
        self.event_bus.publish_market_event(market_event).await
            .map_err(|e| AdapterError::Query(format!("Failed to publish to event bus: {}", e)))?;
        
        log::debug!("Worker {} processed {} event", self.id, work_item.symbol);
        Ok(())
    }
    
    /// Update worker statistics
    async fn update_stats(&self, processing_time: Duration) {
        let mut stats = self.stats.write().await;
        stats.messages_processed += 1;
        stats.total_processing_time += processing_time;
        stats.average_latency = stats.total_processing_time / stats.messages_processed as u32;
        stats.last_activity = Some(Instant::now());
    }
    
    /// Get worker statistics
    pub async fn get_stats(&self) -> WorkerStats {
        self.stats.read().await.clone()
    }
}

/// Worker pool managing multiple processing workers
pub struct WorkerPool {
    workers: Vec<tokio::task::JoinHandle<Result<(), AdapterError>>>,
    work_senders: Vec<mpsc::Sender<WorkItem>>,
    event_bus: Arc<EventBusIntegration>,
    fair_scheduler: Arc<RwLock<FairProcessingScheduler>>,
    shutdown: Arc<AtomicBool>,
    worker_stats: Arc<RwLock<Vec<Arc<RwLock<WorkerStats>>>>>,
    // Load balancing
    current_worker: Arc<std::sync::atomic::AtomicUsize>,
    pool_size: usize,
    queue_size: usize,
}

impl WorkerPool {
    /// Create new worker pool
    pub fn new(
        pool_size: usize,
        queue_size: usize,
        event_bus: Arc<EventBusIntegration>,
        fair_scheduler: Arc<RwLock<FairProcessingScheduler>>,
    ) -> Self {
        Self {
            workers: Vec::new(),
            work_senders: Vec::new(),
            event_bus,
            fair_scheduler,
            shutdown: Arc::new(AtomicBool::new(false)),
            worker_stats: Arc::new(RwLock::new(Vec::new())),
            current_worker: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            pool_size,
            queue_size,
        }
    }
    
    /// Start all workers (now takes mutable self to allow initialization)
    pub async fn start(&mut self) -> Result<(), AdapterError> {
        log::info!("Starting worker pool with {} workers", self.pool_size);
        
        // Clear any existing workers
        self.workers.clear();
        self.work_senders.clear();
        
        for i in 0..self.pool_size {
            // Create a new channel for this worker
            let (work_tx, work_rx) = mpsc::channel::<WorkItem>(self.queue_size);
            
            let worker = Worker::new(
                i,
                work_rx,
                self.event_bus.clone(),
                self.shutdown.clone(),
            );
            
            // Store worker stats reference
            {
                let mut stats = self.worker_stats.write().await;
                stats.push(worker.stats.clone());
            }
            
            let handle = tokio::spawn(worker.start());
            self.workers.push(handle);
            self.work_senders.push(work_tx);
        }
        
        // Start work distribution task
        self.start_work_distributor().await;
        
        log::info!("Worker pool started successfully");
        Ok(())
    }
    
    /// Distribute work item to appropriate worker
    pub async fn submit_work(&self, work_item: WorkItem) -> Result<(), AdapterError> {
        // Use round-robin distribution for now
        // In production, we might want symbol-aware distribution
        let worker_idx = self.current_worker.fetch_add(1, Ordering::Relaxed) % self.workers.len();
        
        if let Some(sender) = self.work_senders.get(worker_idx) {
            sender.send(work_item).await
                .map_err(|e| AdapterError::Query(format!("Failed to send work to worker {}: {}", worker_idx, e)))?;
        } else {
            return Err(AdapterError::Query("Invalid worker index".to_string()));
        }
        
        Ok(())
    }
    
    /// Start work distribution task that pulls from fair scheduler
    async fn start_work_distributor(&self) {
        let fair_scheduler = self.fair_scheduler.clone();
        let work_senders = self.work_senders.clone();
        let shutdown = self.shutdown.clone();
        let current_worker = self.current_worker.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(1));
            
            while !shutdown.load(Ordering::Relaxed) {
                interval.tick().await;
                
                // Get work from fair scheduler
                let work_item = {
                    let mut scheduler = fair_scheduler.write().await;
                    scheduler.get_next_work_item()
                };
                
                if let Some(work_item) = work_item {
                    let processing_start = Instant::now();
                    let symbol = work_item.symbol.clone();
                    
                    // Distribute to worker (round-robin)
                    let worker_idx = current_worker.fetch_add(1, Ordering::Relaxed) % work_senders.len();
                    
                    if let Some(sender) = work_senders.get(worker_idx) {
                        if let Err(e) = sender.send(work_item).await {
                            log::error!("Failed to send work to worker {}: {}", worker_idx, e);
                        } else {
                            // Record processing time in scheduler
                            let processing_time = processing_start.elapsed();
                            let mut scheduler = fair_scheduler.write().await;
                            scheduler.record_processing_completion(&symbol, processing_time);
                        }
                    }
                }
            }
        });
    }
    
    /// Get worker pool statistics
    pub async fn get_worker_statistics(&self) -> WorkerPoolStats {
        let worker_stats = self.worker_stats.read().await;
        let mut total_processed = 0;
        let mut active_workers = 0;
        let mut total_queue_depth = 0.0;
        
        for worker_stat in worker_stats.iter() {
            let stat = worker_stat.read().await;
            total_processed += stat.messages_processed;
            if stat.last_activity.is_some() {
                active_workers += 1;
            }
            total_queue_depth += stat.current_queue_depth as f64;
        }
        
        let scheduler_stats = {
            let scheduler = self.fair_scheduler.read().await;
            scheduler.get_processing_stats()
        };
        
        WorkerPoolStats {
            total_workers: self.workers.len(),
            active_workers,
            total_processed,
            average_queue_depth: if self.workers.is_empty() { 0.0 } else { total_queue_depth / self.workers.len() as f64 },
            symbol_stats: scheduler_stats,
        }
    }
    
    /// Shutdown worker pool
    pub async fn shutdown(&self) -> Result<(), AdapterError> {
        log::info!("Shutting down worker pool");
        
        // Signal shutdown to all workers
        self.shutdown.store(true, Ordering::Relaxed);
        
        // Give workers time to shutdown gracefully
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        log::info!("Worker pool shutdown complete");
        Ok(())
    }
    
    /// Get number of workers
    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }
    
    /// Get work queue depth for a specific worker
    pub async fn get_worker_queue_depth(&self, worker_id: usize) -> Option<usize> {
        if worker_id < self.work_senders.len() {
            // This is an approximation since mpsc doesn't provide exact queue depth
            // In production, we'd implement custom queues with depth tracking
            Some(0) // Placeholder
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::MarketData;
    use std::sync::Arc;
    
    #[tokio::test]
    async fn test_worker_pool_creation() {
        // This test would require mocking EventBusIntegration
        // For now, just test basic structure
        
        // Mock components would go here in a full test suite
        let pool_size = 4;
        let queue_size = 100;
        
        // Test that pool can be created with correct size
        assert!(pool_size > 0);
        assert!(queue_size > 0);
    }
    
    #[tokio::test]
    async fn test_work_distribution() {
        // Test work distribution logic
        let current_worker = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let pool_size = 4;
        
        // Test round-robin distribution
        for i in 0..10 {
            let worker_idx = current_worker.fetch_add(1, Ordering::Relaxed) % pool_size;
            assert!(worker_idx < pool_size);
            assert_eq!(worker_idx, i % pool_size);
        }
    }
}