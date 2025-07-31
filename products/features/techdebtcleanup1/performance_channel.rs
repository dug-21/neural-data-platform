//! Performance Channel for Real-time Feedback Loop
//!
//! This module provides a multi-producer, single-consumer channel that bridges
//! performance monitoring components to the autonomous training engine.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use crate::monitoring::health::ComponentType;

/// Performance event emitted by various system components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceEvent {
    /// Timestamp when the event was generated
    pub timestamp: DateTime<Utc>,
    /// Source component that emitted the event
    pub source: PerformanceSource,
    /// Type of performance event
    pub event_type: PerformanceEventType,
    /// Additional metrics associated with the event
    pub metrics: HashMap<String, f64>,
}

/// Source of performance events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceSource {
    /// Neural predictor model
    NeuralPredictor { model_name: String },
    /// Trading strategy
    TradingStrategy { strategy_name: String },
    /// Event bus streaming system
    EventBus { event_type: String },
    /// Health monitoring system
    HealthMonitor { component: ComponentType },
    /// Backtesting engine
    BacktestEngine { session_id: String },
    /// DAA agent
    DaaAgent { agent_type: String },
}

/// Types of performance events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceEventType {
    /// Neural prediction completed
    PredictionCompleted {
        accuracy: f64,
        confidence: f64,
        latency_ms: u64,
        model_agreement: Option<f64>,
    },
    /// Trading signal generated
    TradingSignal {
        profit_loss: f64,
        sharpe_ratio: f64,
        max_drawdown: f64,
        position_size: f64,
    },
    /// System health metrics
    SystemHealth {
        cpu_usage: f64,
        memory_usage: f64,
        error_rate: f64,
        throughput: f64,
    },
    /// Model divergence detected
    ModelDivergence {
        model_agreement: f64,
        divergence_score: f64,
        affected_models: Vec<String>,
    },
    /// Prediction error
    PredictionError {
        error_type: String,
        error_rate: f64,
        affected_model: String,
    },
    /// Market regime change
    MarketRegimeChange {
        old_regime: String,
        new_regime: String,
        confidence: f64,
    },
}

/// Trait for components that emit performance events
#[async_trait::async_trait]
pub trait PerformanceEmitter: Send + Sync {
    /// Emit a performance event
    async fn emit_performance(&self, event: PerformanceEvent) -> Result<()>;
    
    /// Get performance channel sender
    fn get_performance_sender(&self) -> Option<mpsc::UnboundedSender<PerformanceEvent>>;
    
    /// Set performance channel sender
    fn set_performance_sender(&mut self, sender: mpsc::UnboundedSender<PerformanceEvent>);
}

/// Performance channel for event distribution
pub struct PerformanceChannel {
    /// Sender for performance events (cloneable for multiple producers)
    sender: mpsc::UnboundedSender<PerformanceEvent>,
    /// Receiver for performance events (single consumer)
    receiver: Option<mpsc::UnboundedReceiver<PerformanceEvent>>,
    /// Channel statistics
    stats: Arc<RwLock<ChannelStats>>,
}

/// Statistics for the performance channel
#[derive(Debug, Default)]
struct ChannelStats {
    total_events_sent: u64,
    total_events_dropped: u64,
    events_by_source: HashMap<String, u64>,
    events_by_type: HashMap<String, u64>,
    last_event_time: Option<DateTime<Utc>>,
}

impl PerformanceChannel {
    /// Create a new performance channel
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        
        Self {
            sender,
            receiver: Some(receiver),
            stats: Arc::new(RwLock::new(ChannelStats::default())),
        }
    }
    
    /// Get a sender for the channel (for producers)
    pub fn get_sender(&self) -> mpsc::UnboundedSender<PerformanceEvent> {
        self.sender.clone()
    }
    
    /// Take the receiver (for consumer - can only be called once)
    pub fn take_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<PerformanceEvent>> {
        self.receiver.take()
    }
    
    /// Send an event through the channel with statistics tracking
    pub async fn send_event(&self, event: PerformanceEvent) -> Result<()> {
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_events_sent += 1;
            stats.last_event_time = Some(event.timestamp);
            
            // Track by source
            let source_key = format!("{:?}", event.source);
            *stats.events_by_source.entry(source_key).or_insert(0) += 1;
            
            // Track by type
            let type_key = match &event.event_type {
                PerformanceEventType::PredictionCompleted { .. } => "PredictionCompleted",
                PerformanceEventType::TradingSignal { .. } => "TradingSignal",
                PerformanceEventType::SystemHealth { .. } => "SystemHealth",
                PerformanceEventType::ModelDivergence { .. } => "ModelDivergence",
                PerformanceEventType::PredictionError { .. } => "PredictionError",
                PerformanceEventType::MarketRegimeChange { .. } => "MarketRegimeChange",
            };
            *stats.events_by_type.entry(type_key.to_string()).or_insert(0) += 1;
        }
        
        // Send event
        if let Err(e) = self.sender.send(event) {
            // Update drop statistics
            {
                let mut stats = self.stats.write().await;
                stats.total_events_dropped += 1;
            }
            error!("Failed to send performance event: {}", e);
            return Err(anyhow::anyhow!("Channel send failed: {}", e));
        }
        
        Ok(())
    }
    
    /// Get channel statistics
    pub async fn get_statistics(&self) -> ChannelStatistics {
        let stats = self.stats.read().await;
        
        ChannelStatistics {
            total_events_sent: stats.total_events_sent,
            total_events_dropped: stats.total_events_dropped,
            events_by_source: stats.events_by_source.clone(),
            events_by_type: stats.events_by_type.clone(),
            last_event_time: stats.last_event_time,
            channel_capacity: self.sender.max_capacity(),
        }
    }
}

/// Public statistics structure
#[derive(Debug, Clone, Serialize)]
pub struct ChannelStatistics {
    pub total_events_sent: u64,
    pub total_events_dropped: u64,
    pub events_by_source: HashMap<String, u64>,
    pub events_by_type: HashMap<String, u64>,
    pub last_event_time: Option<DateTime<Utc>>,
    pub channel_capacity: Option<usize>,
}

/// Builder for performance events
pub struct PerformanceEventBuilder {
    timestamp: DateTime<Utc>,
    source: Option<PerformanceSource>,
    event_type: Option<PerformanceEventType>,
    metrics: HashMap<String, f64>,
}

impl PerformanceEventBuilder {
    /// Create a new event builder
    pub fn new() -> Self {
        Self {
            timestamp: Utc::now(),
            source: None,
            event_type: None,
            metrics: HashMap::new(),
        }
    }
    
    /// Set the event source
    pub fn source(mut self, source: PerformanceSource) -> Self {
        self.source = Some(source);
        self
    }
    
    /// Set the event type
    pub fn event_type(mut self, event_type: PerformanceEventType) -> Self {
        self.event_type = Some(event_type);
        self
    }
    
    /// Add a metric
    pub fn metric(mut self, key: &str, value: f64) -> Self {
        self.metrics.insert(key.to_string(), value);
        self
    }
    
    /// Add multiple metrics
    pub fn metrics(mut self, metrics: HashMap<String, f64>) -> Self {
        self.metrics.extend(metrics);
        self
    }
    
    /// Build the event
    pub fn build(self) -> Result<PerformanceEvent> {
        let source = self.source.ok_or_else(|| anyhow::anyhow!("Source is required"))?;
        let event_type = self.event_type.ok_or_else(|| anyhow::anyhow!("Event type is required"))?;
        
        Ok(PerformanceEvent {
            timestamp: self.timestamp,
            source,
            event_type,
            metrics: self.metrics,
        })
    }
}

impl Default for PerformanceEventBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper functions for creating common performance events
impl PerformanceEvent {
    /// Create a prediction completed event
    pub fn prediction_completed(
        model_name: String,
        accuracy: f64,
        confidence: f64,
        latency_ms: u64,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            source: PerformanceSource::NeuralPredictor { model_name },
            event_type: PerformanceEventType::PredictionCompleted {
                accuracy,
                confidence,
                latency_ms,
                model_agreement: None,
            },
            metrics: HashMap::new(),
        }
    }
    
    /// Create a trading signal event
    pub fn trading_signal(
        strategy_name: String,
        profit_loss: f64,
        sharpe_ratio: f64,
        max_drawdown: f64,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            source: PerformanceSource::TradingStrategy { strategy_name },
            event_type: PerformanceEventType::TradingSignal {
                profit_loss,
                sharpe_ratio,
                max_drawdown,
                position_size: 0.0,
            },
            metrics: HashMap::new(),
        }
    }
    
    /// Create a system health event
    pub fn system_health(
        component: ComponentType,
        cpu_usage: f64,
        memory_usage: f64,
        error_rate: f64,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            source: PerformanceSource::HealthMonitor { component },
            event_type: PerformanceEventType::SystemHealth {
                cpu_usage,
                memory_usage,
                error_rate,
                throughput: 0.0,
            },
            metrics: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_performance_channel_creation() {
        let mut channel = PerformanceChannel::new();
        
        // Should be able to get sender
        let _sender = channel.get_sender();
        
        // Should be able to take receiver (only once)
        let receiver = channel.take_receiver();
        assert!(receiver.is_some());
        
        // Second take should return None
        let receiver2 = channel.take_receiver();
        assert!(receiver2.is_none());
    }
    
    #[tokio::test]
    async fn test_event_sending_and_receiving() {
        let mut channel = PerformanceChannel::new();
        let sender = channel.get_sender();
        let mut receiver = channel.take_receiver().unwrap();
        
        // Create and send event
        let event = PerformanceEvent::prediction_completed(
            "TestModel".to_string(),
            0.85,
            0.9,
            100,
        );
        
        channel.send_event(event.clone()).await.unwrap();
        
        // Receive event
        let received = receiver.recv().await.unwrap();
        assert_eq!(received.timestamp, event.timestamp);
        
        // Check statistics
        let stats = channel.get_statistics().await;
        assert_eq!(stats.total_events_sent, 1);
        assert_eq!(stats.total_events_dropped, 0);
    }
    
    #[tokio::test]
    async fn test_event_builder() {
        let event = PerformanceEventBuilder::new()
            .source(PerformanceSource::NeuralPredictor {
                model_name: "TestModel".to_string(),
            })
            .event_type(PerformanceEventType::PredictionCompleted {
                accuracy: 0.85,
                confidence: 0.9,
                latency_ms: 100,
                model_agreement: Some(0.95),
            })
            .metric("custom_metric", 42.0)
            .build()
            .unwrap();
        
        assert_eq!(event.metrics.get("custom_metric"), Some(&42.0));
    }
    
    #[tokio::test]
    async fn test_multiple_producers() {
        let channel = PerformanceChannel::new();
        let sender1 = channel.get_sender();
        let sender2 = channel.get_sender();
        
        // Both senders should work
        let event1 = PerformanceEvent::prediction_completed("Model1".to_string(), 0.8, 0.85, 50);
        let event2 = PerformanceEvent::trading_signal("Strategy1".to_string(), 0.05, 1.2, 0.1);
        
        sender1.send(event1).unwrap();
        sender2.send(event2).unwrap();
        
        // Check statistics
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        let stats = channel.get_statistics().await;
        assert_eq!(stats.total_events_sent, 2);
    }
}