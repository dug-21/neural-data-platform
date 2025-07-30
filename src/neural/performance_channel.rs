//! Performance Channel Module
//!
//! Provides a broadcast channel that bridges performance monitoring to training decisions,
//! enabling real-time feedback loops for autonomous training with bounded buffer.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, info, warn};

/// Performance event emitted by various system components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceEvent {
    pub timestamp: DateTime<Utc>,
    pub source: PerformanceSource,
    pub event_type: PerformanceEventType,
    pub metrics: PerformanceMetrics,
}

/// Source of performance events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceSource {
    NeuralPredictor { model_name: String },
    TradingStrategy { strategy_name: String },
    EventBus { event_type: String },
    HealthMonitor { component: ComponentType },
    BacktestEngine { session_id: String },
}

/// Type of performance event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceEventType {
    PredictionCompleted {
        model: String,
        accuracy: f64,
        confidence: f64,
        latency_ms: u64,
        timestamp: DateTime<Utc>,
    },
    TradingSignal {
        profit_loss: f64,
        sharpe_ratio: f64,
        max_drawdown: f64,
    },
    SystemHealth {
        cpu_usage: f64,
        memory_usage: f64,
        error_rate: f64,
    },
    ModelDivergence {
        model_agreement: f64,
        divergence_score: f64,
    },
}

/// Additional performance metrics that can be attached to events
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub latency_p50: Option<f64>,
    pub latency_p95: Option<f64>,
    pub latency_p99: Option<f64>,
    pub throughput: Option<f64>,
    pub error_count: Option<u64>,
    pub success_count: Option<u64>,
    pub custom_metrics: Option<std::collections::HashMap<String, f64>>,
}

/// Component types for health monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComponentType {
    NeuralEngine,
    TradingEngine,
    DataPipeline,
    EventSystem,
    Storage,
    API,
    Custom(String),
}

/// Channel for performance events with broadcast distribution and bounded buffer
pub struct PerformanceChannel {
    tx: broadcast::Sender<PerformanceEvent>,
    metrics_buffer: Arc<Mutex<VecDeque<PerformanceEvent>>>,
    max_buffer_size: usize,
}

impl PerformanceChannel {
    /// Create a new performance channel with broadcast and receiver
    pub fn new(buffer_size: usize) -> (Self, broadcast::Receiver<PerformanceEvent>) {
        let (tx, rx) = broadcast::channel(buffer_size);
        
        let channel = Self {
            tx,
            metrics_buffer: Arc::new(Mutex::new(VecDeque::with_capacity(buffer_size))),
            max_buffer_size: buffer_size,
        };
        
        info!("Created PerformanceChannel with buffer size: {}", buffer_size);
        (channel, rx)
    }
    
    /// Emit a performance event to subscribers and buffer it
    pub async fn emit(&self, event: PerformanceEvent) -> Result<()> {
        debug!("Emitting performance event: {:?}", event.event_type);
        
        // Send to subscribers (ignore if no active receivers)
        let _ = self.tx.send(event.clone());
        
        // Buffer for analysis with thread-safe access
        {
            let mut buffer = self.metrics_buffer.lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock metrics buffer: {}", e))?;
            
            // Enforce max buffer size to prevent memory issues
            if buffer.len() >= self.max_buffer_size {
                let removed = buffer.pop_front();
                if let Some(old_event) = removed {
                    debug!("Removed old event from buffer: {:?}", old_event.timestamp);
                }
            }
            
            buffer.push_back(event);
            debug!("Buffer size after emit: {}/{}", buffer.len(), self.max_buffer_size);
        }
        
        Ok(())
    }
    
    /// Get recent metrics from the buffer
    pub fn get_recent_metrics(&self, count: usize) -> Vec<PerformanceEvent> {
        let buffer = match self.metrics_buffer.lock() {
            Ok(buffer) => buffer,
            Err(e) => {
                warn!("Failed to lock metrics buffer: {}", e);
                return Vec::new();
            }
        };
        
        let metrics: Vec<PerformanceEvent> = buffer.iter()
            .rev()
            .take(count)
            .cloned()
            .collect();
        
        info!("Retrieved {} recent metrics (requested: {})", metrics.len(), count);
        metrics
    }
    
    /// Get a new receiver for the broadcast channel
    pub fn subscribe(&self) -> broadcast::Receiver<PerformanceEvent> {
        self.tx.subscribe()
    }
    
    /// Get the current buffer size
    pub fn buffer_size(&self) -> usize {
        match self.metrics_buffer.lock() {
            Ok(buffer) => buffer.len(),
            Err(_) => 0,
        }
    }
    
    /// Clear the metrics buffer
    pub fn clear_buffer(&self) -> Result<()> {
        let mut buffer = self.metrics_buffer.lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock metrics buffer: {}", e))?;
        buffer.clear();
        info!("Cleared performance metrics buffer");
        Ok(())
    }
}

/// Trait for components that emit performance events
#[async_trait]
pub trait PerformanceEmitter: Send + Sync {
    /// Emit a performance event
    async fn emit_performance(&self, event: PerformanceEvent) -> Result<()>;

    /// Get performance channel sender
    fn get_performance_sender(&self) -> Option<mpsc::UnboundedSender<PerformanceEvent>>;

    /// Set performance channel sender
    fn set_performance_sender(&mut self, sender: mpsc::UnboundedSender<PerformanceEvent>);
}

/// Builder for performance events
pub struct PerformanceEventBuilder {
    timestamp: DateTime<Utc>,
    source: Option<PerformanceSource>,
    event_type: Option<PerformanceEventType>,
    metrics: PerformanceMetrics,
}

impl PerformanceEventBuilder {
    /// Create a new event builder
    pub fn new() -> Self {
        Self {
            timestamp: Utc::now(),
            source: None,
            event_type: None,
            metrics: PerformanceMetrics::default(),
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

    /// Add custom metrics
    pub fn metrics(mut self, metrics: PerformanceMetrics) -> Self {
        self.metrics = metrics;
        self
    }

    /// Add a custom metric
    pub fn custom_metric(mut self, key: String, value: f64) -> Self {
        if self.metrics.custom_metrics.is_none() {
            self.metrics.custom_metrics = Some(std::collections::HashMap::new());
        }
        if let Some(ref mut custom) = self.metrics.custom_metrics {
            custom.insert(key, value);
        }
        self
    }

    /// Build the performance event
    pub fn build(self) -> Result<PerformanceEvent> {
        let source = self
            .source
            .ok_or_else(|| anyhow::anyhow!("Performance event source is required"))?;
        let event_type = self
            .event_type
            .ok_or_else(|| anyhow::anyhow!("Performance event type is required"))?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_performance_channel_creation() {
        let (channel, mut receiver) = PerformanceChannel::new(100);
        
        // Test emit functionality
        let event = create_test_event();
        assert!(channel.emit(event.clone()).await.is_ok());
        
        // Test that receiver gets the event
        match receiver.try_recv() {
            Ok(received) => assert_eq!(received.timestamp, event.timestamp),
            Err(_) => panic!("Should have received event"),
        }
    }

    #[tokio::test]
    async fn test_performance_event_builder() {
        let event = PerformanceEventBuilder::new()
            .source(PerformanceSource::NeuralPredictor {
                model_name: "test".to_string(),
            })
            .event_type(PerformanceEventType::PredictionCompleted {
                model: "test".to_string(),
                accuracy: 0.95,
                confidence: 0.9,
                latency_ms: 100,
                timestamp: Utc::now(),
            })
            .custom_metric("test_metric".to_string(), 42.0)
            .build();

        assert!(event.is_ok());
        let event = event.unwrap();
        assert!(matches!(event.source, PerformanceSource::NeuralPredictor { .. }));
        assert!(event.metrics.custom_metrics.is_some());
    }

    #[tokio::test]
    async fn test_channel_broadcast_multiple_receivers() {
        let (channel, mut receiver1) = PerformanceChannel::new(100);
        let mut receiver2 = channel.subscribe();

        let event = create_test_event();
        channel.emit(event.clone()).await.unwrap();

        // Both receivers should get the event
        let received1 = receiver1.try_recv().unwrap();
        let received2 = receiver2.try_recv().unwrap();
        
        assert_eq!(received1.timestamp, event.timestamp);
        assert_eq!(received2.timestamp, event.timestamp);
    }
    
    #[tokio::test]
    async fn test_metrics_buffer() {
        let (channel, _receiver) = PerformanceChannel::new(5);
        
        // Emit multiple events
        for i in 0..10 {
            let mut event = create_test_event();
            if let PerformanceEventType::PredictionCompleted { ref mut accuracy, .. } = event.event_type {
                *accuracy = i as f64 / 10.0;
            }
            channel.emit(event).await.unwrap();
        }
        
        // Buffer should have max 5 events (oldest were removed)
        assert_eq!(channel.buffer_size(), 5);
        
        // Get recent metrics
        let recent = channel.get_recent_metrics(3);
        assert_eq!(recent.len(), 3);
        
        // Should get the most recent events (reversed order)
        if let PerformanceEventType::PredictionCompleted { accuracy, .. } = &recent[0].event_type {
            assert_eq!(*accuracy, 0.9); // 9th event (most recent)
        }
    }
    
    #[tokio::test]
    async fn test_clear_buffer() {
        let (channel, _receiver) = PerformanceChannel::new(10);
        
        // Add some events
        for _ in 0..5 {
            channel.emit(create_test_event()).await.unwrap();
        }
        
        assert_eq!(channel.buffer_size(), 5);
        
        // Clear buffer
        channel.clear_buffer().unwrap();
        assert_eq!(channel.buffer_size(), 0);
    }

    fn create_test_event() -> PerformanceEvent {
        PerformanceEvent {
            timestamp: Utc::now(),
            source: PerformanceSource::NeuralPredictor {
                model_name: "test".to_string(),
            },
            event_type: PerformanceEventType::PredictionCompleted {
                model: "test".to_string(),
                accuracy: 0.95,
                confidence: 0.9,
                latency_ms: 100,
                timestamp: Utc::now(),
            },
            metrics: PerformanceMetrics::default(),
        }
    }
}