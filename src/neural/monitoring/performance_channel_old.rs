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
    
    /// Training event
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
}

/// Performance metrics container
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    // Latency metrics
    pub latency_p50: Option<f64>,
    pub latency_p95: Option<f64>,
    pub latency_p99: Option<f64>,
    pub latency_max: Option<f64>,
    
    // Throughput metrics  
    pub requests_per_second: Option<f64>,
    pub success_count: Option<u64>,
    pub error_count: Option<u64>,
    
    // Resource metrics
    pub memory_usage_mb: Option<f64>,
    pub cpu_usage_percent: Option<f64>,
    
    // Business metrics
    pub accuracy: Option<f64>,
    pub confidence: Option<f64>,
    
    // Custom metrics
    pub custom_metrics: Option<HashMap<String, f64>>,
} 
        event_type: String,
        component_id: String,
    },
    HealthMonitor { 
        component: ComponentType,
        monitor_id: String,
    },
    BacktestEngine { 
        session_id: String,
        engine_id: String,
    },
    TrainingSystem {
        trainer_id: String,
        model_type: String,
    },
}

/// Type of performance event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceEventType {
    PredictionCompleted {
        model: String,
        accuracy: f64,
        confidence: f64,
        latency_ms: u64,
        input_features: usize,
        output_dimension: usize,
        timestamp: DateTime<Utc>,
    },
    TradingSignal {
        signal_type: String,
        profit_loss: f64,
        sharpe_ratio: f64,
        max_drawdown: f64,
        position_size: f64,
        risk_score: f64,
    },
    SystemHealth {
        cpu_usage: f64,
        memory_usage: f64,
        gpu_usage: Option<f64>,
        disk_io: f64,
        network_io: f64,
        error_rate: f64,
        active_connections: u32,
    },
    ModelDivergence {
        model_agreement: f64,
        divergence_score: f64,
        model_count: u32,
        disagreement_threshold: f64,
    },
    TrainingTriggered {
        trigger_reason: String,
        model_type: String,
        data_points: usize,
        expected_duration: u64,
    },
    TrainingCompleted {
        model_type: String,
        training_duration: u64,
        final_accuracy: f64,
        validation_score: f64,
        epochs_completed: u32,
    },
    PerformanceDegradation {
        metric_name: String,
        current_value: f64,
        baseline_value: f64,
        degradation_percent: f64,
        impact_severity: String,
    },
    Alert {
        alert_type: AlertType,
        message: String,
        severity: AlertSeverity,
        resolution_required: bool,
    },
}

/// Alert types for notification system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    LowAccuracy,
    HighLatency,
    ResourceExhaustion,
    ModelFailure,
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

/// Enhanced performance metrics with statistical data
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
    
    // Error metrics
    pub error_count: Option<u64>,
    pub success_count: Option<u64>,
    pub retry_count: Option<u64>,
    
    // Resource metrics
    pub memory_usage_bytes: Option<u64>,
    pub cpu_usage_percent: Option<f64>,
    pub gpu_memory_usage: Option<u64>,
    
    // Model-specific metrics
    pub model_confidence: Option<f64>,
    pub model_accuracy: Option<f64>,
    pub training_loss: Option<f64>,
    pub validation_loss: Option<f64>,
    
    // Business metrics
    pub profit_loss: Option<f64>,
    pub risk_score: Option<f64>,
    pub position_value: Option<f64>,
    
    // Custom metrics for extensibility
    pub custom_metrics: Option<HashMap<String, f64>>,
    pub tags: Option<HashMap<String, String>>,
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
    TrainingSystem,
    PerformanceMonitor,
    Custom(String),
}

/// Circular buffer for efficient event storage
pub struct CircularBuffer<T> {
    buffer: VecDeque<T>,
    capacity: usize,
    total_inserted: u64,
}

impl<T> CircularBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
            total_inserted: 0,
        }
    }

    pub fn push(&mut self, item: T) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(item);
        self.total_inserted += 1;
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buffer.iter()
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn total_inserted(&self) -> u64 {
        self.total_inserted
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

/// Enhanced performance channel with event bus capabilities
pub struct PerformanceChannel {
    tx: broadcast::Sender<PerformanceEvent>,
    metrics_buffer: Arc<RwLock<CircularBuffer<PerformanceEvent>>>,
    event_filters: Arc<RwLock<HashMap<String, EventFilter>>>,
    statistics: Arc<RwLock<ChannelStatistics>>,
    config: ChannelConfig,
}

/// Event filter for selective processing
#[derive(Debug, Clone)]
pub struct EventFilter {
    pub source_patterns: Vec<String>,
    pub event_type_patterns: Vec<String>,
    pub priority_threshold: EventPriority,
    pub enabled: bool,
}

/// Channel statistics for monitoring
#[derive(Debug, Default)]
pub struct ChannelStatistics {
    pub total_events_emitted: u64,
    pub total_events_filtered: u64,
    pub active_subscribers: u32,
    pub buffer_utilization: f64,
    pub average_emission_latency_ns: u64,
    pub events_by_priority: HashMap<EventPriority, u64>,
    pub events_by_source: HashMap<String, u64>,
    pub last_reset: DateTime<Utc>,
}

/// Configuration for the performance channel
#[derive(Debug, Clone)]
pub struct ChannelConfig {
    pub buffer_size: usize,
    pub broadcast_capacity: usize,
    pub enable_metrics: bool,
    pub enable_filtering: bool,
    pub max_emission_latency_ms: u64,
    pub statistics_window_seconds: u64,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            buffer_size: 50000,         // 50k events buffer
            broadcast_capacity: 1000,   // 1k broadcast buffer
            enable_metrics: true,
            enable_filtering: true,
            max_emission_latency_ms: 1, // <1ms target
            statistics_window_seconds: 300, // 5 minutes
        }
    }
}

impl PerformanceChannel {
    /// Create a new enhanced performance channel
    pub fn new(config: ChannelConfig) -> (Self, broadcast::Receiver<PerformanceEvent>) {
        let (tx, rx) = broadcast::channel(config.broadcast_capacity);
        
        let channel = Self {
            tx,
            metrics_buffer: Arc::new(RwLock::new(CircularBuffer::new(config.buffer_size))),
            event_filters: Arc::new(RwLock::new(HashMap::new())),
            statistics: Arc::new(RwLock::new(ChannelStatistics {
                last_reset: Utc::now(),
                ..Default::default()
            })),
            config,
        };
        
        info!("Created Enhanced PerformanceChannel with buffer size: {}, broadcast capacity: {}", 
              config.buffer_size, config.broadcast_capacity);
        
        (channel, rx)
    }

    /// Emit a performance event with sub-millisecond latency target
    #[instrument(skip(self, event), fields(event_id = %event.id))]
    pub async fn emit(&self, event: PerformanceEvent) -> Result<()> {
        let start_time = std::time::Instant::now();
        
        // Apply filters if enabled
        if self.config.enable_filtering {
            if let Ok(filters) = self.event_filters.read() {
                if !self.passes_filters(&event, &filters) {
                    if let Ok(mut stats) = self.statistics.write() {
                        stats.total_events_filtered += 1;
                    }
                    return Ok(());
                }
            }
        }

        // Emit to subscribers (non-blocking)
        let subscriber_count = self.tx.receiver_count();
        let send_result = self.tx.send(event.clone());
        
        match send_result {
            Ok(_) => {
                debug!("Event {} emitted to {} subscribers", event.id, subscriber_count);
            }
            Err(broadcast::error::SendError(_)) => {
                debug!("No active subscribers for event {}", event.id);
            }
        }

        // Buffer for analysis (fast write lock)
        if let Ok(mut buffer) = self.metrics_buffer.write() {
            buffer.push(event.clone());
        }

        // Update statistics if enabled
        if self.config.enable_metrics {
            let emission_latency = start_time.elapsed().as_nanos() as u64;
            if let Ok(mut stats) = self.statistics.write() {
                stats.total_events_emitted += 1;
                stats.active_subscribers = subscriber_count as u32;
                stats.buffer_utilization = self.buffer_utilization();
                stats.average_emission_latency_ns = 
                    (stats.average_emission_latency_ns + emission_latency) / 2;
                
                // Track by priority
                *stats.events_by_priority.entry(event.priority.clone()).or_insert(0) += 1;
                
                // Track by source type
                let source_key = match &event.source {
                    PerformanceSource::NeuralPredictor { model_name, .. } => 
                        format!("neural:{}", model_name),
                    PerformanceSource::TradingStrategy { strategy_name, .. } => 
                        format!("trading:{}", strategy_name),
                    PerformanceSource::TrainingSystem { model_type, .. } => 
                        format!("training:{}", model_type),
                    _ => "other".to_string(),
                };
                *stats.events_by_source.entry(source_key).or_insert(0) += 1;
            }
        }

        // Check latency target
        let total_latency = start_time.elapsed().as_millis() as u64;
        if total_latency > self.config.max_emission_latency_ms {
            warn!("Event emission exceeded latency target: {}ms > {}ms", 
                  total_latency, self.config.max_emission_latency_ms);
        }

        Ok(())
    }

    /// Emit event with fire-and-forget semantics for maximum performance
    pub fn emit_fast(&self, event: PerformanceEvent) {
        // Non-blocking, best-effort emission
        let _ = self.tx.send(event.clone());
        
        // Fast buffer insert without error handling
        if let Ok(mut buffer) = self.metrics_buffer.try_write() {
            buffer.push(event);
        }
    }

    /// Get recent metrics from the buffer
    pub fn get_recent_metrics(&self, count: usize) -> Vec<PerformanceEvent> {
        match self.metrics_buffer.read() {
            Ok(buffer) => {
                buffer.iter()
                    .rev()
                    .take(count)
                    .cloned()
                    .collect()
            }
            Err(e) => {
                warn!("Failed to read metrics buffer: {}", e);
                Vec::new()
            }
        }
    }

    /// Get metrics by filter criteria
    pub fn get_filtered_metrics<F>(&self, filter: F, limit: Option<usize>) -> Vec<PerformanceEvent>
    where
        F: Fn(&PerformanceEvent) -> bool,
    {
        match self.metrics_buffer.read() {
            Ok(buffer) => {
                let mut results: Vec<PerformanceEvent> = buffer.iter()
                    .filter(|event| filter(event))
                    .cloned()
                    .collect();
                
                if let Some(limit) = limit {
                    results.truncate(limit);
                }
                
                results
            }
            Err(_) => Vec::new(),
        }
    }

    /// Subscribe to performance events
    pub fn subscribe(&self) -> broadcast::Receiver<PerformanceEvent> {
        self.tx.subscribe()
    }

    /// Subscribe with filter
    pub fn subscribe_filtered(&self, filter: EventFilter) -> (broadcast::Receiver<PerformanceEvent>, String) {
        let filter_id = format!("filter_{}", chrono::Utc::now().timestamp_nanos());
        
        if let Ok(mut filters) = self.event_filters.write() {
            filters.insert(filter_id.clone(), filter);
        }
        
        (self.tx.subscribe(), filter_id)
    }

    /// Add or update event filter
    pub fn add_filter(&self, filter_id: String, filter: EventFilter) -> Result<()> {
        self.event_filters.write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire filter lock: {}", e))?
            .insert(filter_id, filter);
        Ok(())
    }

    /// Remove event filter
    pub fn remove_filter(&self, filter_id: &str) -> bool {
        match self.event_filters.write() {
            Ok(mut filters) => filters.remove(filter_id).is_some(),
            Err(_) => false,
        }
    }

    /// Get channel statistics
    pub fn get_statistics(&self) -> Result<ChannelStatistics> {
        self.statistics.read()
            .map_err(|e| anyhow::anyhow!("Failed to read statistics: {}", e))
            .map(|stats| stats.clone())
    }

    /// Reset statistics
    pub fn reset_statistics(&self) -> Result<()> {
        let mut stats = self.statistics.write()
            .map_err(|e| anyhow::anyhow!("Failed to write statistics: {}", e))?;
        
        *stats = ChannelStatistics {
            last_reset: Utc::now(),
            ..Default::default()
        };
        
        Ok(())
    }

    /// Get current buffer utilization as percentage
    pub fn buffer_utilization(&self) -> f64 {
        match self.metrics_buffer.read() {
            Ok(buffer) => (buffer.len() as f64 / buffer.capacity() as f64) * 100.0,
            Err(_) => 0.0,
        }
    }

    /// Get buffer size
    pub fn buffer_size(&self) -> usize {
        match self.metrics_buffer.read() {
            Ok(buffer) => buffer.len(),
            Err(_) => 0,
        }
    }

    /// Clear metrics buffer
    pub fn clear_buffer(&self) -> Result<()> {
        self.metrics_buffer.write()
            .map_err(|e| anyhow::anyhow!("Failed to clear buffer: {}", e))?
            .clear();
        
        info!("Cleared performance metrics buffer");
        Ok(())
    }

    /// Check if event passes active filters
    fn passes_filters(&self, event: &PerformanceEvent, filters: &HashMap<String, EventFilter>) -> bool {
        if filters.is_empty() {
            return true;
        }

        for filter in filters.values() {
            if !filter.enabled {
                continue;
            }

            // Check priority threshold
            if event.priority > filter.priority_threshold {
                continue;
            }

            // Check source patterns (simplified pattern matching)
            if !filter.source_patterns.is_empty() {
                let source_str = format!("{:?}", event.source);
                if !filter.source_patterns.iter().any(|pattern| source_str.contains(pattern)) {
                    continue;
                }
            }

            // If we reach here, the event passes this filter
            return true;
        }

        // No filters matched
        false
    }
}

/// Builder for performance events with fluent API
pub struct PerformanceEventBuilder {
    id: String,
    timestamp: DateTime<Utc>,
    source: Option<PerformanceSource>,
    event_type: Option<PerformanceEventType>,
    metrics: PerformanceMetrics,
    tags: HashMap<String, String>,
    correlation_id: Option<String>,
    priority: EventPriority,
}

impl PerformanceEventBuilder {
    pub fn new() -> Self {
        Self {
            id: format!("event_{}", chrono::Utc::now().timestamp_nanos()),
            timestamp: Utc::now(),
            source: None,
            event_type: None,
            metrics: PerformanceMetrics::default(),
            tags: HashMap::new(),
            correlation_id: None,
            priority: EventPriority::Medium,
        }
    }

    pub fn id(mut self, id: String) -> Self {
        self.id = id;
        self
    }

    pub fn source(mut self, source: PerformanceSource) -> Self {
        self.source = Some(source);
        self
    }

    pub fn event_type(mut self, event_type: PerformanceEventType) -> Self {
        self.event_type = Some(event_type);
        self
    }

    pub fn metrics(mut self, metrics: PerformanceMetrics) -> Self {
        self.metrics = metrics;
        self
    }

    pub fn tag(mut self, key: String, value: String) -> Self {
        self.tags.insert(key, value);
        self
    }

    pub fn correlation_id(mut self, correlation_id: String) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    pub fn priority(mut self, priority: EventPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn build(self) -> Result<PerformanceEvent> {
        let source = self.source
            .ok_or_else(|| anyhow::anyhow!("Performance event source is required"))?;
        let event_type = self.event_type
            .ok_or_else(|| anyhow::anyhow!("Performance event type is required"))?;

        Ok(PerformanceEvent {
            id: self.id,
            timestamp: self.timestamp,
            source,
            event_type,
            metrics: self.metrics,
            tags: self.tags,
            correlation_id: self.correlation_id,
            priority: self.priority,
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
    async fn test_enhanced_channel_creation() {
        let config = ChannelConfig::default();
        let (channel, mut receiver) = PerformanceChannel::new(config);
        
        let event = create_test_event();
        assert!(channel.emit(event.clone()).await.is_ok());
        
        match receiver.try_recv() {
            Ok(received) => assert_eq!(received.id, event.id),
            Err(_) => panic!("Should have received event"),
        }
    }

    #[tokio::test]
    async fn test_circular_buffer() {
        let mut buffer = CircularBuffer::new(3);
        
        buffer.push(1);
        buffer.push(2);
        buffer.push(3);
        buffer.push(4); // Should evict 1
        
        let items: Vec<&i32> = buffer.iter().collect();
        assert_eq!(items, vec![&2, &3, &4]);
        assert_eq!(buffer.total_inserted(), 4);
    }

    #[tokio::test]
    async fn test_event_filtering() {
        let config = ChannelConfig::default();
        let (channel, _receiver) = PerformanceChannel::new(config);
        
        let filter = EventFilter {
            source_patterns: vec!["NeuralPredictor".to_string()],
            event_type_patterns: vec![],
            priority_threshold: EventPriority::High,
            enabled: true,
        };
        
        channel.add_filter("test_filter".to_string(), filter).unwrap();
        
        // This should pass the filter
        let event = PerformanceEventBuilder::new()
            .source(PerformanceSource::NeuralPredictor {
                model_name: "test".to_string(),
                predictor_id: "pred1".to_string(),
            })
            .event_type(PerformanceEventType::PredictionCompleted {
                model: "test".to_string(),
                accuracy: 0.95,
                confidence: 0.9,
                latency_ms: 100,
                input_features: 10,
                output_dimension: 1,
                timestamp: Utc::now(),
            })
            .priority(EventPriority::High)
            .build()
            .unwrap();
        
        assert!(channel.emit(event).await.is_ok());
        
        let stats = channel.get_statistics().unwrap();
        assert_eq!(stats.total_events_emitted, 1);
    }

    fn create_test_event() -> PerformanceEvent {
        PerformanceEventBuilder::new()
            .source(PerformanceSource::NeuralPredictor {
                model_name: "test".to_string(),
                predictor_id: "pred1".to_string(),
            })
            .event_type(PerformanceEventType::PredictionCompleted {
                model: "test".to_string(),
                accuracy: 0.95,
                confidence: 0.9,
                latency_ms: 100,
                input_features: 10,
                output_dimension: 1,
                timestamp: Utc::now(),
            })
            .build()
            .unwrap()
    }
}