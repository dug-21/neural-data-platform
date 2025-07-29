//! Performance Channel Module
//!
//! Provides a multi-producer, single-consumer channel that bridges performance monitoring
//! to training decisions, enabling real-time feedback loops for autonomous training.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

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
        accuracy: f64,
        confidence: f64,
        latency_ms: u64,
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

/// Channel for performance events with multi-producer, single-consumer pattern
pub struct PerformanceChannel {
    sender: mpsc::UnboundedSender<PerformanceEvent>,
    receiver: Option<mpsc::UnboundedReceiver<PerformanceEvent>>,
}

impl PerformanceChannel {
    /// Create a new performance channel
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self {
            sender,
            receiver: Some(receiver),
        }
    }

    /// Get a sender clone for emitting events
    pub fn get_sender(&self) -> mpsc::UnboundedSender<PerformanceEvent> {
        self.sender.clone()
    }

    /// Take the receiver (can only be called once)
    pub fn take_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<PerformanceEvent>> {
        self.receiver.take()
    }

    /// Send a performance event
    pub fn send(&self, event: PerformanceEvent) -> Result<()> {
        self.sender
            .send(event)
            .map_err(|e| anyhow::anyhow!("Failed to send performance event: {}", e))
    }
}

impl Default for PerformanceChannel {
    fn default() -> Self {
        Self::new()
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
        let channel = PerformanceChannel::new();
        assert!(channel.sender.send(create_test_event()).is_ok());
    }

    #[tokio::test]
    async fn test_performance_event_builder() {
        let event = PerformanceEventBuilder::new()
            .source(PerformanceSource::NeuralPredictor {
                model_name: "test".to_string(),
            })
            .event_type(PerformanceEventType::PredictionCompleted {
                accuracy: 0.95,
                confidence: 0.9,
                latency_ms: 100,
            })
            .custom_metric("test_metric".to_string(), 42.0)
            .build();

        assert!(event.is_ok());
        let event = event.unwrap();
        assert!(matches!(event.source, PerformanceSource::NeuralPredictor { .. }));
        assert!(event.metrics.custom_metrics.is_some());
    }

    #[tokio::test]
    async fn test_channel_send_receive() {
        let mut channel = PerformanceChannel::new();
        let sender = channel.get_sender();
        let mut receiver = channel.take_receiver().unwrap();

        let event = create_test_event();
        sender.send(event.clone()).unwrap();

        let received = receiver.recv().await.unwrap();
        assert_eq!(received.timestamp, event.timestamp);
    }

    fn create_test_event() -> PerformanceEvent {
        PerformanceEvent {
            timestamp: Utc::now(),
            source: PerformanceSource::NeuralPredictor {
                model_name: "test".to_string(),
            },
            event_type: PerformanceEventType::PredictionCompleted {
                accuracy: 0.95,
                confidence: 0.9,
                latency_ms: 100,
            },
            metrics: PerformanceMetrics::default(),
        }
    }
}