//! Enhanced Performance Channel Module
//!
//! Provides a high-performance broadcast channel that bridges performance monitoring to training decisions,
//! enabling real-time feedback loops for autonomous training with advanced event bus patterns.
//! 
//! PERFORMANCE TARGETS:
//! - Event emission latency <1ms
//! - Channel throughput >10k events/sec
//! - Training notification latency <5ms
//! - Memory usage <50MB for buffers

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::{debug, error, info, instrument, warn};

/// Performance event emitted by various system components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceEvent {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub source: PerformanceSource,
    pub event_type: PerformanceEventType,
    pub metrics: PerformanceMetrics,
    pub tags: HashMap<String, String>,
    pub correlation_id: Option<String>,
    pub priority: EventPriority,
}

/// Priority levels for performance events
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventPriority {
    Critical = 0,
    High = 1,
    Medium = 2,
    Low = 3,
}

/// Source of performance events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceSource {
    NeuralPredictor { 
        model_name: String,
        predictor_id: String,
    },
    TradingStrategy {
        strategy_name: String,
        strategy_id: String,
    },
    DataAdapter {
        adapter_name: String,
        adapter_type: String,
    },
    IntegrationHub {
        component_name: String,
    },
    System {
        service_name: String,
    },
    HealthMonitor { 
        component: ComponentType,
        monitor_id: String,
    },
    TrainingSystem {
        trainer_id: String,
        model_type: String,
    },
}

/// Types of performance events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceEventType {
    /// Neural prediction completed
    PredictionCompleted {
        model: String,
        accuracy: f64,
        confidence: f64,
        latency_ms: u64,
        input_features: usize,
        output_dimension: usize,
        timestamp: DateTime<Utc>,
    },
    
    /// Training event started
    TrainingStarted {
        model: String,
        training_type: String,
        estimated_duration_mins: u32,
    },
    
    /// Training completed
    TrainingCompleted {
        model: String,
        duration_mins: u32,
        performance_improvement: f64,
        new_accuracy: f64,
    },
    
    /// Trading signal generated
    TradingSignal {
        signal_type: String,
        profit_loss: f64,
        sharpe_ratio: f64,
        max_drawdown: f64,
        position_size: f64,
        risk_score: f64,
    },
    
    /// Model error occurred
    ModelError {
        model: String,
        error_type: String,
        error_message: String,
        recoverable: bool,
    },
    
    /// System health update
    SystemHealth {
        component: String,
        cpu_usage_percent: f64,
        memory_usage_mb: f64,
        error_rate: f64,
        availability_percent: f64,
    },
    
    /// Model divergence detected
    ModelDivergence {
        model_agreement: f64,
        divergence_score: f64,
        model_count: u32,
        disagreement_threshold: f64,
    },
    
    /// Performance degradation detected
    PerformanceDegradation {
        metric_name: String,
        current_value: f64,
        baseline_value: f64,
        degradation_percent: f64,
        impact_severity: String,
    },
    
    /// Performance alert
    Alert {
        alert_type: AlertType,
        message: String,
        severity: AlertSeverity,
        resolution_required: bool,
    },
    
    /// Metrics update
    MetricsUpdate {
        component: String,
        metrics: HashMap<String, f64>,
        timestamp: DateTime<Utc>,
    },
}

/// Alert types for performance monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    ModelFailure,
    PerformanceDegradation,
    LatencySpike,
    MemoryLeak,
    HighErrorRate,
    ServiceUnavailable,
    ConfigurationError,
    LowAccuracy,
    HighLatency,
    ResourceExhaustion,
    TrainingRequired,
    SystemError,
    Custom(String),
}

/// Alert severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertSeverity {
    Critical = 0,
    Warning = 1,
    Info = 2,
}

/// Component types for categorization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComponentType {
    NeuralNetwork,
    DataProcessor,
    TradingEngine,
    MarketDataFeed,
    DatabaseConnection,
    ExternalAPI,
    Integration,
    NeuralEngine,
    DataPipeline,
    RiskManager,
    BacktestEngine,
}

/// Performance metrics container
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    // Latency metrics
    pub latency_p50: Option<f64>,
    pub latency_p95: Option<f64>,
    pub latency_p99: Option<f64>,
    pub latency_max: Option<f64>,
    pub latency_min: Option<f64>,
    
    // Throughput metrics  
    pub throughput: Option<f64>,
    pub requests_per_second: Option<f64>,
    pub success_count: Option<u64>,
    pub error_count: Option<u64>,
    pub retry_count: Option<u64>,
    
    // Resource metrics
    pub memory_usage_mb: Option<f64>,
    pub memory_usage_bytes: Option<u64>,
    pub cpu_usage_percent: Option<f64>,
    pub gpu_memory_usage: Option<u64>,
    
    // Business metrics
    pub accuracy: Option<f64>,
    pub confidence: Option<f64>,
    pub model_confidence: Option<f64>,
    pub model_accuracy: Option<f64>,
    pub training_loss: Option<f64>,
    pub validation_loss: Option<f64>,
    pub profit_loss: Option<f64>,
    pub risk_score: Option<f64>,
    pub position_value: Option<f64>,
    
    // Custom metrics
    pub custom_metrics: Option<HashMap<String, f64>>,
    pub tags: Option<HashMap<String, String>>,
}

/// Channel configuration for performance monitoring
#[derive(Debug, Clone)]
pub struct ChannelConfig {
    pub buffer_size: usize,
    pub channel_capacity: usize,
    pub enable_persistence: bool,
    pub enable_metrics: bool,
    pub max_subscribers: usize,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            buffer_size: 1000,
            channel_capacity: 10000,
            enable_persistence: true,
            enable_metrics: true,
            max_subscribers: 100,
        }
    }
}

/// Channel statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelStatistics {
    pub total_events_emitted: u64,
    pub events_per_second: f64,
    pub average_latency_ms: f64,
    pub buffer_utilization_percent: f64,
    pub active_subscribers: usize,
    pub dropped_events: u64,
    pub last_event_timestamp: Option<DateTime<Utc>>,
}

impl Default for ChannelStatistics {
    fn default() -> Self {
        Self {
            total_events_emitted: 0,
            events_per_second: 0.0,
            average_latency_ms: 0.0,
            buffer_utilization_percent: 0.0,
            active_subscribers: 0,
            dropped_events: 0,
            last_event_timestamp: None,
        }
    }
}

/// Circular buffer for efficient event storage
pub struct CircularBuffer<T> {
    buffer: VecDeque<T>,
    capacity: usize,
}

impl<T> CircularBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }
    
    pub fn push(&mut self, item: T) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(item);
    }
    
    pub fn get_recent(&self, count: usize) -> Vec<&T> {
        self.buffer.iter().rev().take(count).collect()
    }
    
    pub fn len(&self) -> usize {
        self.buffer.len()
    }
    
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

/// High-performance event channel with broadcast capabilities
pub struct PerformanceChannel {
    sender: broadcast::Sender<PerformanceEvent>,
    buffer: Arc<RwLock<CircularBuffer<PerformanceEvent>>>,
    statistics: Arc<RwLock<ChannelStatistics>>,
    config: ChannelConfig,
}

impl PerformanceChannel {
    /// Create new performance channel
    pub fn new(config: ChannelConfig) -> (Self, broadcast::Receiver<PerformanceEvent>) {
        let (sender, receiver) = broadcast::channel(config.channel_capacity);
        
        let channel = Self {
            sender,
            buffer: Arc::new(RwLock::new(CircularBuffer::new(config.buffer_size))),
            statistics: Arc::new(RwLock::new(ChannelStatistics::default())),
            config,
        };
        
        (channel, receiver)
    }
    
    /// Create with buffer size
    pub fn new_with_buffer(buffer_size: usize) -> (Self, broadcast::Receiver<PerformanceEvent>) {
        let config = ChannelConfig {
            buffer_size,
            ..Default::default()
        };
        Self::new(config)
    }
    
    /// Emit performance event to all subscribers
    pub async fn emit(&self, event: PerformanceEvent) -> Result<()> {
        let start_time = std::time::Instant::now();
        
        // Send to all subscribers
        let subscriber_count = match self.sender.send(event.clone()) {
            Ok(count) => count,
            Err(_) => {
                warn!("No active subscribers for performance event");
                0
            }
        };
        
        // Store in buffer if persistence enabled
        if self.config.enable_persistence {
            if let Ok(mut buffer) = self.buffer.write() {
                buffer.push(event.clone());
            }
        }
        
        // Update statistics if enabled
        if self.config.enable_metrics {
            let latency = start_time.elapsed().as_millis() as f64;
            self.update_statistics(subscriber_count, latency).await;
        }
        
        debug!("Emitted performance event to {} subscribers", subscriber_count);
        Ok(())
    }
    
    /// Emit event with fire-and-forget semantics (maximum performance)
    pub fn emit_fast(&self, event: PerformanceEvent) {
        let _ = self.sender.send(event.clone());
        
        // Store in buffer without blocking
        if self.config.enable_persistence {
            if let Ok(mut buffer) = self.buffer.try_write() {
                buffer.push(event);
            }
        }
    }
    
    /// Subscribe to performance events
    pub fn subscribe(&self) -> broadcast::Receiver<PerformanceEvent> {
        self.sender.subscribe()
    }
    
    /// Get recent events from buffer
    pub fn get_recent_metrics(&self, count: usize) -> Vec<PerformanceEvent> {
        if let Ok(buffer) = self.buffer.read() {
            buffer.get_recent(count).into_iter().cloned().collect()
        } else {
            Vec::new()
        }
    }
    
    /// Get buffer size
    pub fn buffer_size(&self) -> usize {
        if let Ok(buffer) = self.buffer.read() {
            buffer.len()
        } else {
            0
        }
    }
    
    /// Get channel statistics
    pub fn get_statistics(&self) -> Result<ChannelStatistics> {
        if let Ok(stats) = self.statistics.read() {
            Ok(stats.clone())
        } else {
            Err(anyhow::anyhow!("Failed to read statistics"))
        }
    }
    
    /// Clear event buffer
    pub fn clear_buffer(&self) -> Result<()> {
        if let Ok(mut buffer) = self.buffer.write() {
            buffer.clear();
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to clear buffer"))
        }
    }
    
    /// Update channel statistics
    async fn update_statistics(&self, subscriber_count: usize, latency_ms: f64) {
        if let Ok(mut stats) = self.statistics.write() {
            stats.total_events_emitted += 1;
            stats.active_subscribers = subscriber_count;
            stats.last_event_timestamp = Some(Utc::now());
            
            // Update average latency (exponential moving average)
            if stats.total_events_emitted == 1 {
                stats.average_latency_ms = latency_ms;
            } else {
                stats.average_latency_ms = stats.average_latency_ms * 0.9 + latency_ms * 0.1;
            }
            
            // Update buffer utilization
            if let Ok(buffer) = self.buffer.read() {
                stats.buffer_utilization_percent = 
                    (buffer.len() as f64 / self.config.buffer_size as f64) * 100.0;
            }
        }
    }
}

impl Clone for PerformanceChannel {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            buffer: Arc::clone(&self.buffer),
            statistics: Arc::clone(&self.statistics),
            config: self.config.clone(),
        }
    }
}

/// Builder for creating performance events
pub struct PerformanceEventBuilder {
    event: PerformanceEvent,
}

impl PerformanceEventBuilder {
    pub fn new() -> Self {
        let event = PerformanceEvent {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            source: PerformanceSource::System {
                service_name: "unknown".to_string(),
            },
            event_type: PerformanceEventType::MetricsUpdate {
                component: "unknown".to_string(),
                metrics: HashMap::new(),
                timestamp: Utc::now(),
            },
            metrics: PerformanceMetrics::default(),
            tags: HashMap::new(),
            correlation_id: None,
            priority: EventPriority::Medium,
        };
        
        Self { event }
    }
    
    pub fn source(mut self, source: PerformanceSource) -> Self {
        self.event.source = source;
        self
    }
    
    pub fn event_type(mut self, event_type: PerformanceEventType) -> Self {
        self.event.event_type = event_type;
        self
    }
    
    pub fn metrics(mut self, metrics: PerformanceMetrics) -> Self {
        self.event.metrics = metrics;
        self
    }
    
    pub fn priority(mut self, priority: EventPriority) -> Self {
        self.event.priority = priority;
        self
    }
    
    pub fn tag(mut self, key: String, value: String) -> Self {
        self.event.tags.insert(key, value);
        self
    }
    
    pub fn correlation_id(mut self, correlation_id: String) -> Self {
        self.event.correlation_id = Some(correlation_id);
        self
    }
    
    pub fn custom_metric(mut self, key: String, value: f64) -> Self {
        if self.event.metrics.custom_metrics.is_none() {
            self.event.metrics.custom_metrics = Some(HashMap::new());
        }
        
        if let Some(ref mut custom_metrics) = self.event.metrics.custom_metrics {
            custom_metrics.insert(key, value);
        }
        
        self
    }
    
    pub fn build(self) -> Result<PerformanceEvent> {
        Ok(self.event)
    }
}

impl Default for PerformanceEventBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for components that emit performance events
#[async_trait]
pub trait PerformanceEmitter: Send + Sync {
    /// Emit a performance event
    async fn emit_performance(&self, event: PerformanceEvent) -> Result<()>;
    
    /// Get the performance sender (if available)
    fn get_performance_sender(&self) -> Option<mpsc::UnboundedSender<PerformanceEvent>>;
    
    /// Set the performance sender
    fn set_performance_sender(&mut self, sender: mpsc::UnboundedSender<PerformanceEvent>);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_performance_channel_creation() {
        let config = ChannelConfig::default();
        let (channel, _receiver) = PerformanceChannel::new(config);
        
        assert_eq!(channel.buffer_size(), 0);
        
        let stats = channel.get_statistics().unwrap();
        assert_eq!(stats.total_events_emitted, 0);
    }

    #[tokio::test]
    async fn test_event_emission() {
        let (channel, mut receiver) = PerformanceChannel::new_with_buffer(100);
        
        let event = PerformanceEventBuilder::new()
            .source(PerformanceSource::NeuralPredictor {
                model_name: "test_model".to_string(),
                predictor_id: "pred_1".to_string(),
            })
            .event_type(PerformanceEventType::PredictionCompleted {
                model: "test_model".to_string(),
                accuracy: 0.95,
                confidence: 0.9,
                latency_ms: 100,
                input_features: 10,
                output_dimension: 1,
                timestamp: Utc::now(),
            })
            .build()
            .unwrap();
        
        channel.emit(event.clone()).await.unwrap();
        
        // Should receive the event
        let received = timeout(Duration::from_millis(100), receiver.recv())
            .await
            .expect("Timeout")
            .expect("Failed to receive");
        
        assert_eq!(received.id, event.id);
        assert_eq!(channel.buffer_size(), 1);
    }

    #[tokio::test]
    async fn test_event_builder() {
        let event = PerformanceEventBuilder::new()
            .source(PerformanceSource::System {
                service_name: "test_service".to_string(),
            })
            .event_type(PerformanceEventType::Alert {
                alert_type: AlertType::SystemError,
                message: "Test alert".to_string(),
                severity: AlertSeverity::Warning,
                resolution_required: true,
            })
            .priority(EventPriority::High)
            .tag("environment".to_string(), "test".to_string())
            .custom_metric("test_metric".to_string(), 42.0)
            .build()
            .unwrap();
        
        assert!(matches!(event.priority, EventPriority::High));
        assert_eq!(event.tags.get("environment"), Some(&"test".to_string()));
        
        if let Some(ref custom_metrics) = event.metrics.custom_metrics {
            assert_eq!(custom_metrics.get("test_metric"), Some(&42.0));
        } else {
            panic!("Custom metrics not found");
        }
    }
}