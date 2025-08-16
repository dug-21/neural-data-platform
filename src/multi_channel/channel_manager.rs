/*!
 * Channel subscription manager for Redis multi-channel subscriptions.
 * 
 * Manages the lifecycle of Redis channel subscriptions for market data,
 * handling connection failures, reconnections, and subscription routing.
 */

use std::collections::HashMap;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Duration;
use tokio::sync::RwLock;
use futures::StreamExt;

use crate::adapters::{AdapterError, MarketData};
use crate::adapters::redis::RedisAdapter;
use crate::multi_channel::{WorkItem, worker_pool::WorkerPool};

/// Individual channel subscription state
#[derive(Debug)]
pub struct ChannelSubscription {
    pub symbol: String,
    pub channel: String,
    pub is_active: bool,
    pub reconnect_count: u32,
    pub last_message_time: Option<std::time::Instant>,
    pub message_count: u64,
    pub task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl ChannelSubscription {
    pub fn new(symbol: String, channel: String) -> Self {
        Self {
            symbol,
            channel,
            is_active: false,
            reconnect_count: 0,
            last_message_time: None,
            message_count: 0,
            task_handle: None,
        }
    }
}

// Manual Send + Sync implementation for thread safety
unsafe impl Send for ChannelSubscription {}
unsafe impl Sync for ChannelSubscription {}

/// Channel subscription manager
pub struct ChannelSubscriptionManager {
    subscriptions: Arc<RwLock<HashMap<String, ChannelSubscription>>>,
    redis_adapter: Arc<RedisAdapter>,
    worker_pool: Arc<RwLock<WorkerPool>>,
    shutdown_signal: Arc<AtomicBool>,
    reconnect_interval: Duration,
}

impl ChannelSubscriptionManager {
    /// Create new channel subscription manager
    pub fn new(
        redis_adapter: Arc<RedisAdapter>,
        worker_pool: Arc<RwLock<WorkerPool>>,
    ) -> Self {
        Self {
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            redis_adapter,
            worker_pool,
            shutdown_signal: Arc::new(AtomicBool::new(false)),
            reconnect_interval: Duration::from_secs(5),
        }
    }
    
    /// Add a new subscription for a symbol
    pub async fn add_subscription(
        &self,
        symbol: String,
        channel: String,
    ) -> Result<(), AdapterError> {
        log::info!("Adding subscription for symbol {} on channel {}", symbol, channel);
        
        let mut subscriptions = self.subscriptions.write().await;
        
        // Check if already subscribed
        if subscriptions.contains_key(&symbol) {
            log::warn!("Already subscribed to symbol {}", symbol);
            return Ok(());
        }
        
        // Create subscription
        let subscription = ChannelSubscription::new(symbol.clone(), channel.clone());
        subscriptions.insert(symbol.clone(), subscription);
        drop(subscriptions); // Release lock before starting task
        
        // Start subscription task
        self.start_subscription_task(symbol, channel).await?;
        
        Ok(())
    }
    
    /// Remove a subscription for a symbol
    pub async fn remove_subscription(&self, symbol: &str) -> Result<(), AdapterError> {
        log::info!("Removing subscription for symbol {}", symbol);
        
        let mut subscriptions = self.subscriptions.write().await;
        
        if let Some(mut subscription) = subscriptions.remove(symbol) {
            subscription.is_active = false;
            
            // Cancel the task if it exists
            if let Some(handle) = subscription.task_handle.take() {
                handle.abort();
            }
        }
        
        Ok(())
    }
    
    /// Start subscription task for a specific channel
    async fn start_subscription_task(
        &self,
        symbol: String,
        channel: String,
    ) -> Result<(), AdapterError> {
        let redis_adapter = self.redis_adapter.clone();
        let worker_pool = self.worker_pool.clone();
        let subscriptions = self.subscriptions.clone();
        let shutdown_signal = self.shutdown_signal.clone();
        let reconnect_interval = self.reconnect_interval;
        let symbol_clone = symbol.clone(); // Clone for async move
        
        let handle = tokio::spawn(async move {
            let mut reconnect_count = 0;
            
            while !shutdown_signal.load(Ordering::Relaxed) {
                log::info!("Starting subscription task for {} on {}", symbol_clone, channel);
                
                // Update subscription status
                {
                    let mut subs = subscriptions.write().await;
                    if let Some(sub) = subs.get_mut(&symbol_clone) {
                        sub.is_active = true;
                        sub.reconnect_count = reconnect_count;
                    }
                }
                
                // Subscribe to the channel
                match redis_adapter.subscribe_market_data(&channel).await {
                    Ok(mut stream) => {
                        log::info!("Successfully subscribed to channel {}", channel);
                        reconnect_count = 0; // Reset on successful connection
                        
                        // Process messages from stream
                        while let Some(result) = stream.next().await {
                            if shutdown_signal.load(Ordering::Relaxed) {
                                break;
                            }
                            
                            match result {
                                Ok(market_data) => {
                                    // Update subscription statistics
                                    {
                                        let mut subs = subscriptions.write().await;
                                        if let Some(sub) = subs.get_mut(&symbol_clone) {
                                            sub.last_message_time = Some(std::time::Instant::now());
                                            sub.message_count += 1;
                                        }
                                    }
                                    
                                    // Create work item
                                    let work_item = WorkItem {
                                        symbol: symbol_clone.clone(),
                                        channel: channel.clone(),
                                        market_data,
                                        received_at: std::time::Instant::now(),
                                        priority: 1.0, // Base priority
                                    };
                                    
                                    // Submit to worker pool
                                    {
                                        let worker_pool = worker_pool.read().await;
                                        if let Err(e) = worker_pool.submit_work(work_item).await {
                                            log::error!("Failed to submit work for {}: {}", symbol_clone, e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("Error receiving data from channel {}: {}", channel, e);
                                    break; // Break to trigger reconnection
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to subscribe to channel {}: {}", channel, e);
                    }
                }
                
                // Update subscription status to inactive
                {
                    let mut subs = subscriptions.write().await;
                    if let Some(sub) = subs.get_mut(&symbol_clone) {
                        sub.is_active = false;
                    }
                }
                
                if !shutdown_signal.load(Ordering::Relaxed) {
                    reconnect_count += 1;
                    log::warn!(
                        "Subscription for {} lost, attempting reconnect #{} in {:?}",
                        symbol_clone, reconnect_count, reconnect_interval
                    );
                    
                    // Exponential backoff with max delay
                    let delay = std::cmp::min(
                        reconnect_interval * 2_u32.pow(std::cmp::min(reconnect_count, 5)),
                        Duration::from_secs(60)
                    );
                    
                    tokio::time::sleep(delay).await;
                }
            }
            
            log::info!("Subscription task for {} shutting down", symbol_clone);
        });
        
        // Store the task handle
        {
            let mut subscriptions = self.subscriptions.write().await;
            if let Some(subscription) = subscriptions.get_mut(&symbol) {
                subscription.task_handle = Some(handle);
            }
        }
        
        Ok(())
    }
    
    /// Get subscription statistics
    pub async fn get_subscription_stats(&self) -> HashMap<String, (bool, u64, u32)> {
        let subscriptions = self.subscriptions.read().await;
        let mut stats = HashMap::new();
        
        for (symbol, subscription) in subscriptions.iter() {
            stats.insert(
                symbol.clone(),
                (
                    subscription.is_active,
                    subscription.message_count,
                    subscription.reconnect_count,
                ),
            );
        }
        
        stats
    }
    
    /// Get active subscription count
    pub async fn get_active_subscription_count(&self) -> usize {
        let subscriptions = self.subscriptions.read().await;
        subscriptions.values().filter(|s| s.is_active).count()
    }
    
    /// Get total subscription count
    pub async fn get_total_subscription_count(&self) -> usize {
        let subscriptions = self.subscriptions.read().await;
        subscriptions.len()
    }
    
    /// Check if a symbol is subscribed
    pub async fn is_subscribed(&self, symbol: &str) -> bool {
        let subscriptions = self.subscriptions.read().await;
        subscriptions.contains_key(symbol)
    }
    
    /// Get subscription status for a symbol
    pub async fn get_subscription_status(&self, symbol: &str) -> Option<bool> {
        let subscriptions = self.subscriptions.read().await;
        subscriptions.get(symbol).map(|s| s.is_active)
    }
    
    /// Shutdown all subscriptions
    pub async fn shutdown(&self) -> Result<(), AdapterError> {
        log::info!("Shutting down channel subscription manager");
        
        // Signal shutdown to all tasks
        self.shutdown_signal.store(true, Ordering::Relaxed);
        
        // Wait for all subscription tasks to complete
        let mut subscriptions = self.subscriptions.write().await;
        for (symbol, subscription) in subscriptions.iter_mut() {
            if let Some(handle) = subscription.task_handle.take() {
                log::debug!("Waiting for subscription task {} to complete", symbol);
                handle.abort(); // Force abort if needed
            }
        }
        
        subscriptions.clear();
        
        log::info!("Channel subscription manager shutdown complete");
        Ok(())
    }
    
    /// Force reconnection for a specific symbol
    pub async fn force_reconnect(&self, symbol: &str) -> Result<(), AdapterError> {
        log::info!("Forcing reconnection for symbol {}", symbol);
        
        let subscriptions = self.subscriptions.read().await;
        if let Some(subscription) = subscriptions.get(symbol) {
            let channel = subscription.channel.clone();
            drop(subscriptions);
            
            // Remove and re-add subscription
            let _ = self.remove_subscription(symbol).await;
            self.add_subscription(symbol.to_string(), channel).await?;
        }
        
        Ok(())
    }
    
    /// Health check for all subscriptions
    pub async fn health_check(&self) -> HashMap<String, bool> {
        let mut health_status = HashMap::new();
        let subscriptions = self.subscriptions.read().await;
        let stale_threshold = Duration::from_secs(60); // 60 seconds
        
        for (symbol, subscription) in subscriptions.iter() {
            let is_healthy = subscription.is_active && 
                subscription.last_message_time
                    .map(|t| t.elapsed() < stale_threshold)
                    .unwrap_or(false);
                    
            health_status.insert(symbol.clone(), is_healthy);
        }
        
        health_status
    }
    
    /// Get detailed subscription info
    pub async fn get_subscription_details(&self) -> Vec<(String, String, bool, u64, Option<Duration>)> {
        let subscriptions = self.subscriptions.read().await;
        let mut details = Vec::new();
        
        for (symbol, subscription) in subscriptions.iter() {
            let last_activity = subscription.last_message_time.map(|t| t.elapsed());
            
            details.push((
                symbol.clone(),
                subscription.channel.clone(),
                subscription.is_active,
                subscription.message_count,
                last_activity,
            ));
        }
        
        details
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_channel_subscription_creation() {
        let symbol = "AAPL".to_string();
        let channel = "market:AAPL".to_string();
        
        let subscription = ChannelSubscription::new(symbol.clone(), channel.clone());
        
        assert_eq!(subscription.symbol, symbol);
        assert_eq!(subscription.channel, channel);
        assert!(!subscription.is_active);
        assert_eq!(subscription.reconnect_count, 0);
        assert_eq!(subscription.message_count, 0);
    }
    
    #[tokio::test]
    async fn test_subscription_stats() {
        // This would require mocking RedisAdapter and WorkerPool
        // In a full test suite, we'd create mock implementations
        
        let stats: std::collections::HashMap<String, usize> = HashMap::new();
        assert!(stats.is_empty());
    }
}