//! Multi-Channel Subscription Manager
//!
//! Manages multiple symbol-specific market data subscriptions with fair processing.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use crate::adapters::{redis::RedisAdapter, AdapterError};
use crate::streaming::event_bus::EventBusIntegration;
use futures::StreamExt;

/// Manages symbol-specific subscriptions for fair processing
pub struct SubscriptionManager {
    redis_adapter: Arc<RedisAdapter>,
    event_bus: Arc<EventBusIntegration>,
    subscriptions: Arc<RwLock<HashMap<String, SubscriptionHandle>>>,
    shutdown: Arc<AtomicBool>,
}

/// Handle for a single subscription
pub struct SubscriptionHandle {
    pub symbol: String,
    pub channel: String,
    pub handle: tokio::task::JoinHandle<Result<(), AdapterError>>,
    pub created_at: Instant,
}

impl SubscriptionManager {
    /// Create new subscription manager
    pub fn new(
        redis_adapter: Arc<RedisAdapter>,
        event_bus: Arc<EventBusIntegration>,
    ) -> Self {
        Self {
            redis_adapter,
            event_bus,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }
    
    /// Start subscription for a symbol
    pub async fn subscribe_symbol(&self, symbol: &str) -> Result<()> {
        let channel = format!("market:{}", symbol);
        let redis_adapter = self.redis_adapter.clone();
        let event_bus = self.event_bus.clone();
        let shutdown = self.shutdown.clone();
        let symbol_name = symbol.to_string();
        
        let handle = tokio::spawn(async move {
            match redis_adapter.subscribe_market_data(&channel).await {
                Ok(mut stream) => {
                    while let Some(result) = stream.next().await {
                        if shutdown.load(Ordering::Relaxed) {
                            break;
                        }
                        
                        match result {
                            Ok(market_data) => {
                                // Process market data
                                // Implementation would go here
                            }
                            Err(e) => {
                                tracing::error!("Error in subscription {}: {}", symbol_name, e);
                            }
                        }
                    }
                    Ok(())
                }
                Err(e) => Err(e),
            }
        });
        
        let subscription = SubscriptionHandle {
            symbol: symbol.to_string(),
            channel: channel.clone(),
            handle,
            created_at: Instant::now(),
        };
        
        let mut subs = self.subscriptions.write().await;
        subs.insert(symbol.to_string(), subscription);
        
        tracing::info!("Started subscription for symbol: {} on channel: {}", symbol, channel);
        Ok(())
    }
    
    /// Stop all subscriptions
    pub async fn shutdown(&self) -> Result<()> {
        self.shutdown.store(true, Ordering::Relaxed);
        
        let mut subs = self.subscriptions.write().await;
        for (symbol, subscription) in subs.drain() {
            subscription.handle.abort();
            tracing::info!("Stopped subscription for symbol: {}", symbol);
        }
        
        Ok(())
    }
}