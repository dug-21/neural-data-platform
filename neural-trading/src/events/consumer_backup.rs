//! Proto-only Event Consumer Implementation
//!
//! This consumer ONLY accepts protobuf messages via ProtoEventBus.
//! ALL Vec<u8> and JSON handling has been REMOVED per Phase 4 specification.

use anyhow::{Result, Context};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{info, debug, error, warn};

use neural_core::eventbus::{
    traits::{ProtoEventBus, DynamicProtoEventSubscriber},
    types::{ProtoEvent, ProtoMessage, SubscriptionConfig, EventId},
    implementations::ProtoInMemoryEventBus,
    error::EventBusError,
};

use crate::daa::coordinator::DAACoordinator;

/// Proto-only Event Consumer - NO Vec<u8> or JSON support
pub struct EventConsumer {
    eventbus: Arc<dyn ProtoEventBus>,
    daa_coordinator: Arc<DAACoordinator>,
    subscriber: Arc<RwLock<Option<Box<dyn DynamicProtoEventSubscriber>>>>,
    consumer_task: Arc<RwLock<Option<JoinHandle<()>>>>,
    channels: Vec<String>,
    consumer_group: String,
}

impl EventConsumer {
    /// Create new proto-only event consumer
    pub async fn new(
        redis_url: String,
        daa_coordinator: Arc<DAACoordinator>,
    ) -> Result<Self> {
        info!("Initializing proto-only EventConsumer");
        
        // Create ProtoEventBus connection (replace with actual Redis connection)
        let eventbus = Self::create_proto_eventbus(&redis_url).await
            .context("Failed to create ProtoEventBus connection")?;
        
        // Define channels to consume from
        let channels = vec![
            "market_data_proto".to_string(),
            "neural_predictions_proto".to_string(),
            "trading_signals_proto".to_string(),
            "risk_alerts_proto".to_string(),
        ];
        
        let consumer = Self {
            eventbus,
            daa_coordinator,
            subscriber: Arc::new(RwLock::new(None)),
            consumer_task: Arc::new(RwLock::new(None)),
            channels,
            consumer_group: "neural-trading".to_string(),
        };
        
        info!("Proto-only EventConsumer initialized successfully");
        Ok(consumer)
    }

    /// Start consuming proto events
    pub async fn start(&self) -> Result<()> {
        info!("Starting proto-only Event Consumer");
        
        // Create consumer groups for all channels
        for channel in &self.channels {
            if let Err(e) = self.eventbus.create_proto_consumer_group(channel, &self.consumer_group).await {
                warn!("Consumer group creation failed for channel {}: {}", channel, e);
                // Continue - group might already exist
            }
        }
        
        // Define proto types we can handle
        let proto_types = vec![
            "neural_trader.MarketDataEvent",
            "neural_trader.NeuralPredictionEvent", 
            "neural_trader.TradingSignalEvent",
            "neural_trader.RiskAlertEvent",
        ];
        
        // Create dynamic proto subscriber
        let config = SubscriptionConfig {
            consumer_group: Some(self.consumer_group.clone()),
            auto_ack: false,
            batch_size: 10,
            timeout_ms: 5000,
        };
        
        let subscriber = self.eventbus.subscribe_dynamic_proto(
            &self.channels,
            &proto_types,
            config
        ).await.context("Failed to create dynamic proto subscriber")?;
        
        // Store subscriber
        {
            let mut sub_guard = self.subscriber.write().await;
            *sub_guard = Some(subscriber);
        }
        
        // Start consumer task
        let task = self.start_consumer_loop().await;
        {
            let mut task_guard = self.consumer_task.write().await;
            *task_guard = Some(task);
        }
        
        info!("Proto-only Event Consumer started successfully");
        Ok(())
    }

    /// Stop consuming events
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping proto-only Event Consumer");
        
        // Stop consumer task
        {
            let mut task_guard = self.consumer_task.write().await;
            if let Some(task) = task_guard.take() {
                task.abort();
            }
        }
        
        // Clear subscriber
        {
            let mut sub_guard = self.subscriber.write().await;
            *sub_guard = None;
        }
        
        info!("Proto-only Event Consumer stopped");
        Ok(())
    }
    
    /// Get consumer statistics
    pub async fn get_stats(&self) -> ConsumerStats {
        // TODO: Implement actual stats tracking
        ConsumerStats {
            channels_subscribed: self.channels.len(),
            messages_processed: 0,
            messages_acked: 0,
            messages_nacked: 0,
            processing_errors: 0,
        }
    }
    
    // Private methods
    
    /// Create ProtoEventBus connection
    async fn create_proto_eventbus(redis_url: &str) -> Result<Arc<dyn ProtoEventBus>> {
        // TODO: Replace with actual Redis ProtoEventBus when available
        debug!("Creating ProtoEventBus connection to {}", redis_url);
        
        // For now, use in-memory implementation
        let eventbus = ProtoInMemoryEventBus::new();
        Ok(Arc::new(eventbus))
    }
    
    /// Start the consumer event loop
    async fn start_consumer_loop(&self) -> JoinHandle<()> {
        let subscriber = self.subscriber.clone();
        let eventbus = self.eventbus.clone();
        let daa_coordinator = self.daa_coordinator.clone();
        let channels = self.channels.clone();
        let consumer_group = self.consumer_group.clone();
        
        tokio::spawn(async move {
            info!("Starting proto event consumer loop");
            
            loop {
                let subscriber_guard = subscriber.read().await;
                let subscriber_ref = match subscriber_guard.as_ref() {
                    Some(sub) => sub,
                    None => {
                        warn!("No subscriber available, stopping consumer loop");
                        break;
                    }
                };
                
                // Receive proto events (stub implementation)
                // TODO: Implement actual proto event receiving
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            
            info!("Proto event consumer loop ended");
        })
    }
}

/// Consumer statistics
#[derive(Debug, Clone)]
pub struct ConsumerStats {
    pub channels_subscribed: usize,
    pub messages_processed: u64,
    pub messages_acked: u64,
    pub messages_nacked: u64,
    pub processing_errors: u64,
}