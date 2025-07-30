//! Real-time Metrics Collector
//!
//! Collects performance metrics from various sources and prepares them for aggregation
//! and analysis. Provides high-throughput metric collection with minimal overhead.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, instrument, warn};

use super::super::performance_channel::{PerformanceEvent, PerformanceEventType, PerformanceSource};

/// Collected metric data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPoint {
    pub timestamp: DateTime<Utc>,
    pub metric_name: String,
    pub value: f64,
    pub tags: HashMap<String, String>,
    pub source: String,
    pub unit: MetricUnit,
}

/// Metric units for proper interpretation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricUnit {
    Count,
    Percentage,
    Milliseconds,
    Seconds,
    Bytes,
    BytesPerSecond,
    RequestsPerSecond,
    Currency,
    Ratio,
    Custom(String),
}

/// Aggregated metric statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricStatistics {
    pub metric_name: String,
    pub count: u64,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub std_dev: f64,
    pub percentiles: HashMap<String, f64>, // P50, P95, P99, etc.
    pub last_value: f64,
    pub last_updated: DateTime<Utc>,
    pub rate_per_second: f64,
    pub trend: TrendDirection,
}

/// Trend direction for metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    Increasing,
    Decreasing,
    Stable,
    Volatile,
}

/// Metric collection configuration
#[derive(Debug, Clone)]
pub struct CollectorConfig {
    pub buffer_size: usize,
    pub collection_interval: Duration,
    pub enable_statistics: bool,
    pub enable_trend_analysis: bool,
    pub retention_duration: Duration,
    pub high_frequency_metrics: Vec<String>,
    pub alert_thresholds: HashMap<String, AlertThreshold>,
}

/// Alert threshold configuration
#[derive(Debug, Clone)]
pub struct AlertThreshold {
    pub max_value: Option<f64>,
    pub min_value: Option<f64>,
    pub rate_of_change_max: Option<f64>,
    pub enabled: bool,
}

impl Default for CollectorConfig {
    fn default() -> Self {
        Self {
            buffer_size: 100000,
            collection_interval: Duration::seconds(1),
            enable_statistics: true,
            enable_trend_analysis: true,
            retention_duration: Duration::hours(24),
            high_frequency_metrics: vec![
                "prediction.latency".to_string(),
                "prediction.accuracy".to_string(),
                "system.cpu_usage".to_string(),
                "system.memory_usage".to_string(),
            ],
            alert_thresholds: HashMap::new(),
        }
    }
}

/// Real-time metrics collector
pub struct MetricsCollector {
    config: CollectorConfig,
    
    // Data storage
    metric_points: Arc<RwLock<VecDeque<MetricPoint>>>,
    metric_statistics: Arc<RwLock<HashMap<String, MetricStatistics>>>,
    
    // Communication channels
    event_rx: mpsc::UnboundedReceiver<PerformanceEvent>,
    metric_tx: mpsc::UnboundedSender<MetricPoint>,
    
    // Internal state
    last_collection: Arc<RwLock<DateTime<Utc>>>,
    collection_stats: Arc<RwLock<CollectionStatistics>>,
}

/// Collection statistics for monitoring
#[derive(Debug, Default)]
pub struct CollectionStatistics {
    total_events_processed: u64,
    total_metrics_collected: u64,
    metrics_per_second: f64,
    events_per_second: f64,
    buffer_utilization: f64,
    last_reset: DateTime<Utc>,
    processing_errors: u64,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new(
        config: CollectorConfig,
        event_rx: mpsc::UnboundedReceiver<PerformanceEvent>,
    ) -> (Self, mpsc::UnboundedReceiver<MetricPoint>) {
        let (metric_tx, metric_rx) = mpsc::unbounded_channel();
        
        let collector = Self {
            config,
            metric_points: Arc::new(RwLock::new(VecDeque::new())),
            metric_statistics: Arc::new(RwLock::new(HashMap::new())),
            event_rx,
            metric_tx,
            last_collection: Arc::new(RwLock::new(Utc::now())),
            collection_stats: Arc::new(RwLock::new(CollectionStatistics {
                last_reset: Utc::now(),
                ..Default::default()
            })),
        };
        
        info!("Created MetricsCollector with buffer size: {}", collector.config.buffer_size);
        
        (collector, metric_rx)
    }

    /// Start collecting metrics from performance events
    #[instrument(skip(self))]
    pub async fn start_collection(&mut self) -> Result<()> {
        info!("Starting metrics collection");
        
        let mut collection_interval = tokio::time::interval(
            self.config.collection_interval.to_std()?
        );
        
        loop {
            tokio::select! {
                // Process incoming performance events
                Some(event) = self.event_rx.recv() => {
                    if let Err(e) = self.process_performance_event(event).await {
                        error!("Failed to process performance event: {}", e);
                        self.increment_error_count().await;
                    }
                }
                
                // Periodic collection tasks
                _ = collection_interval.tick() => {
                    if let Err(e) = self.perform_periodic_tasks().await {
                        error!("Failed to perform periodic tasks: {}", e);
                    }
                }
            }
        }
    }

    /// Process a performance event and extract metrics
    #[instrument(skip(self, event), fields(event_id = %event.id))]
    async fn process_performance_event(&self, event: PerformanceEvent) -> Result<()> {
        let metrics = self.extract_metrics_from_event(&event).await?;
        
        for metric in metrics {
            // Store metric point
            self.store_metric_point(metric.clone()).await?;
            
            // Update statistics if enabled
            if self.config.enable_statistics {
                self.update_metric_statistics(&metric).await?;
            }
            
            // Send to downstream consumers
            if let Err(e) = self.metric_tx.send(metric.clone()) {
                warn!("Failed to send metric point: {}", e);
            }
        }
        
        // Update collection statistics
        self.update_collection_stats().await;
        
        Ok(())
    }

    /// Extract metrics from a performance event
    async fn extract_metrics_from_event(&self, event: &PerformanceEvent) -> Result<Vec<MetricPoint>> {
        let mut metrics = Vec::new();
        let base_tags = self.create_base_tags(event);
        
        match &event.event_type {
            PerformanceEventType::PredictionCompleted {
                model,
                accuracy,
                confidence,
                latency_ms,
                input_features,
                output_dimension,
                ..
            } => {
                let mut tags = base_tags.clone();
                tags.insert("model".to_string(), model.clone());
                
                metrics.extend(vec![
                    MetricPoint {
                        timestamp: event.timestamp,
                        metric_name: "prediction.accuracy".to_string(),
                        value: *accuracy,
                        tags: tags.clone(),
                        source: "neural_predictor".to_string(),
                        unit: MetricUnit::Ratio,
                    },
                    MetricPoint {
                        timestamp: event.timestamp,
                        metric_name: "prediction.confidence".to_string(),
                        value: *confidence,
                        tags: tags.clone(),
                        source: "neural_predictor".to_string(),
                        unit: MetricUnit::Ratio,
                    },
                    MetricPoint {
                        timestamp: event.timestamp,
                        metric_name: "prediction.latency".to_string(),
                        value: *latency_ms as f64,
                        tags: tags.clone(),
                        source: "neural_predictor".to_string(),
                        unit: MetricUnit::Milliseconds,
                    },
                    MetricPoint {
                        timestamp: event.timestamp,
                        metric_name: "prediction.input_features".to_string(),
                        value: *input_features as f64,
                        tags: tags.clone(),
                        source: "neural_predictor".to_string(),
                        unit: MetricUnit::Count,
                    },
                    MetricPoint {
                        timestamp: event.timestamp,
                        metric_name: "prediction.output_dimension".to_string(),
                        value: *output_dimension as f64,
                        tags: tags.clone(),
                        source: "neural_predictor".to_string(),
                        unit: MetricUnit::Count,
                    },
                ]);
            }
            
            PerformanceEventType::TradingSignal {
                signal_type,
                profit_loss,
                sharpe_ratio,
                max_drawdown,
                position_size,
                risk_score,
            } => {
                let mut tags = base_tags.clone();
                tags.insert("signal_type".to_string(), signal_type.clone());
                
                metrics.extend(vec![
                    MetricPoint {
                        timestamp: event.timestamp,
                        metric_name: "trading.profit_loss".to_string(),
                        value: *profit_loss,
                        tags: tags.clone(),
                        source: "trading_engine".to_string(),
                        unit: MetricUnit::Currency,
                    },
                    MetricPoint {
                        timestamp: event.timestamp,
                        metric_name: "trading.sharpe_ratio".to_string(),
                        value: *sharpe_ratio,
                        tags: tags.clone(),
                        source: "trading_engine".to_string(),
                        unit: MetricUnit::Ratio,
                    },
                    MetricPoint {
                        timestamp: event.timestamp,
                        metric_name: "trading.max_drawdown".to_string(),
                        value: *max_drawdown,
                        tags: tags.clone(),
                        source: "trading_engine".to_string(),
                        unit: MetricUnit::Percentage,
                    },
                    MetricPoint {
                        timestamp: event.timestamp,
                        metric_name: "trading.position_size".to_string(),
                        value: *position_size,
                        tags: tags.clone(),
                        source: "trading_engine".to_string(),
                        unit: MetricUnit::Currency,
                    },
                    MetricPoint {
                        timestamp: event.timestamp,
                        metric_name: "trading.risk_score".to_string(),
                        value: *risk_score,
                        tags: tags.clone(),
                        source: "trading_engine".to_string(),
                        unit: MetricUnit::Ratio,
                    },
                ]);
            }
            
            PerformanceEventType::SystemHealth {
                cpu_usage,
                memory_usage,
                gpu_usage,
                disk_io,
                network_io,
                error_rate,
                active_connections,
            } => {
                metrics.extend(vec![
                    MetricPoint {
                        timestamp: event.timestamp,
                        metric_name: "system.cpu_usage".to_string(),
                        value: *cpu_usage,
                        tags: base_tags.clone(),
                        source: "system_monitor".to_string(),
                        unit: MetricUnit::Percentage,
                    },
                    MetricPoint {
                        timestamp: event.timestamp,
                        metric_name: "system.memory_usage".to_string(),
                        value: *memory_usage,
                        tags: base_tags.clone(),
                        source: "system_monitor".to_string(),
                        unit: MetricUnit::Percentage,
                    },
                    MetricPoint {
                        timestamp: event.timestamp,
                        metric_name: "system.disk_io".to_string(),
                        value: *disk_io,
                        tags: base_tags.clone(),
                        source: "system_monitor".to_string(),
                        unit: MetricUnit::BytesPerSecond,
                    },
                    MetricPoint {
                        timestamp: event.timestamp,
                        metric_name: "system.network_io".to_string(),
                        value: *network_io,
                        tags: base_tags.clone(),
                        source: "system_monitor".to_string(),
                        unit: MetricUnit::BytesPerSecond,
                    },
                    MetricPoint {
                        timestamp: event.timestamp,
                        metric_name: "system.error_rate".to_string(),
                        value: *error_rate,
                        tags: base_tags.clone(),
                        source: "system_monitor".to_string(),
                        unit: MetricUnit::Percentage,
                    },
                    MetricPoint {
                        timestamp: event.timestamp,
                        metric_name: "system.active_connections".to_string(),
                        value: *active_connections as f64,
                        tags: base_tags.clone(),
                        source: "system_monitor".to_string(),
                        unit: MetricUnit::Count,
                    },
                ]);
                
                if let Some(gpu_usage) = gpu_usage {
                    metrics.push(MetricPoint {
                        timestamp: event.timestamp,
                        metric_name: "system.gpu_usage".to_string(),
                        value: *gpu_usage,
                        tags: base_tags.clone(),
                        source: "system_monitor".to_string(),
                        unit: MetricUnit::Percentage,
                    });
                }
            }
            
            PerformanceEventType::TrainingCompleted {
                model_type,
                training_duration,
                final_accuracy,
                validation_score,
                epochs_completed,
            } => {
                let mut tags = base_tags.clone();
                tags.insert("model_type".to_string(), model_type.clone());
                
                metrics.extend(vec![
                    MetricPoint {
                        timestamp: event.timestamp,
                        metric_name: "training.duration".to_string(),
                        value: *training_duration as f64,
                        tags: tags.clone(),
                        source: "training_system".to_string(),
                        unit: MetricUnit::Seconds,
                    },
                    MetricPoint {
                        timestamp: event.timestamp,
                        metric_name: "training.final_accuracy".to_string(),
                        value: *final_accuracy,
                        tags: tags.clone(),
                        source: "training_system".to_string(),
                        unit: MetricUnit::Ratio,
                    },
                    MetricPoint {
                        timestamp: event.timestamp,
                        metric_name: "training.validation_score".to_string(),
                        value: *validation_score,
                        tags: tags.clone(),
                        source: "training_system".to_string(),
                        unit: MetricUnit::Ratio,
                    },
                    MetricPoint {
                        timestamp: event.timestamp,
                        metric_name: "training.epochs_completed".to_string(),
                        value: *epochs_completed as f64,
                        tags: tags.clone(),
                        source: "training_system".to_string(),
                        unit: MetricUnit::Count,
                    },
                ]);
            }
            
            _ => {
                // Handle other event types or extract custom metrics
                if let Some(custom_metrics) = &event.metrics.custom_metrics {
                    for (key, value) in custom_metrics {
                        metrics.push(MetricPoint {
                            timestamp: event.timestamp,
                            metric_name: format!("custom.{}", key),
                            value: *value,
                            tags: base_tags.clone(),
                            source: "custom".to_string(),
                            unit: MetricUnit::Custom("unknown".to_string()),
                        });
                    }
                }
            }
        }
        
        // Add common metrics from PerformanceMetrics
        self.extract_common_metrics(&event.metrics, &base_tags, event.timestamp, &mut metrics);
        
        Ok(metrics)
    }

    /// Extract common metrics from PerformanceMetrics
    fn extract_common_metrics(
        &self,
        metrics: &super::super::performance_channel::PerformanceMetrics,
        base_tags: &HashMap<String, String>,
        timestamp: DateTime<Utc>,
        output: &mut Vec<MetricPoint>,
    ) {
        if let Some(latency_p50) = metrics.latency_p50 {
            output.push(MetricPoint {
                timestamp,
                metric_name: "latency.p50".to_string(),
                value: latency_p50,
                tags: base_tags.clone(),
                source: "metrics".to_string(),
                unit: MetricUnit::Milliseconds,
            });
        }
        
        if let Some(latency_p95) = metrics.latency_p95 {
            output.push(MetricPoint {
                timestamp,
                metric_name: "latency.p95".to_string(),
                value: latency_p95,
                tags: base_tags.clone(),
                source: "metrics".to_string(),
                unit: MetricUnit::Milliseconds,
            });
        }
        
        if let Some(latency_p99) = metrics.latency_p99 {
            output.push(MetricPoint {
                timestamp,
                metric_name: "latency.p99".to_string(),
                value: latency_p99,
                tags: base_tags.clone(),
                source: "metrics".to_string(),
                unit: MetricUnit::Milliseconds,
            });
        }
        
        if let Some(throughput) = metrics.throughput {
            output.push(MetricPoint {
                timestamp,
                metric_name: "throughput".to_string(),
                value: throughput,
                tags: base_tags.clone(),
                source: "metrics".to_string(),
                unit: MetricUnit::RequestsPerSecond,
            });
        }
        
        if let Some(error_count) = metrics.error_count {
            output.push(MetricPoint {
                timestamp,
                metric_name: "errors.count".to_string(),
                value: error_count as f64,
                tags: base_tags.clone(),
                source: "metrics".to_string(),
                unit: MetricUnit::Count,
            });
        }
        
        if let Some(success_count) = metrics.success_count {
            output.push(MetricPoint {
                timestamp,
                metric_name: "success.count".to_string(),
                value: success_count as f64,
                tags: base_tags.clone(),
                source: "metrics".to_string(),
                unit: MetricUnit::Count,
            });
        }
    }

    /// Create base tags from event
    fn create_base_tags(&self, event: &PerformanceEvent) -> HashMap<String, String> {
        let mut tags = HashMap::new();
        
        // Add source information
        match &event.source {
            PerformanceSource::NeuralPredictor { model_name, predictor_id } => {
                tags.insert("source".to_string(), "neural_predictor".to_string());
                tags.insert("model_name".to_string(), model_name.clone());
                tags.insert("predictor_id".to_string(), predictor_id.clone());
            }
            PerformanceSource::TradingStrategy { strategy_name, strategy_id } => {
                tags.insert("source".to_string(), "trading_strategy".to_string());
                tags.insert("strategy_name".to_string(), strategy_name.clone());
                tags.insert("strategy_id".to_string(), strategy_id.clone());
            }
            PerformanceSource::TrainingSystem { trainer_id, model_type } => {
                tags.insert("source".to_string(), "training_system".to_string());
                tags.insert("trainer_id".to_string(), trainer_id.clone());
                tags.insert("model_type".to_string(), model_type.clone());
            }
            _ => {
                tags.insert("source".to_string(), "unknown".to_string());
            }
        }
        
        // Add event tags
        for (key, value) in &event.tags {
            tags.insert(key.clone(), value.clone());
        }
        
        // Add priority
        tags.insert("priority".to_string(), format!("{:?}", event.priority));
        
        // Add correlation ID if present
        if let Some(correlation_id) = &event.correlation_id {
            tags.insert("correlation_id".to_string(), correlation_id.clone());
        }
        
        tags
    }

    /// Store metric point in buffer
    async fn store_metric_point(&self, metric: MetricPoint) -> Result<()> {
        let mut points = self.metric_points.write().await;
        
        // Enforce buffer size limit
        if points.len() >= self.config.buffer_size {
            points.pop_front();
        }
        
        points.push_back(metric);
        Ok(())
    }

    /// Update metric statistics
    async fn update_metric_statistics(&self, metric: &MetricPoint) -> Result<()> {
        let mut stats = self.metric_statistics.write().await;
        let stat = stats.entry(metric.metric_name.clone()).or_insert_with(|| {
            MetricStatistics {
                metric_name: metric.metric_name.clone(),
                count: 0,
                sum: 0.0,
                min: f64::INFINITY,
                max: f64::NEG_INFINITY,
                mean: 0.0,
                std_dev: 0.0,
                percentiles: HashMap::new(),
                last_value: 0.0,
                last_updated: metric.timestamp,
                rate_per_second: 0.0,
                trend: TrendDirection::Stable,
            }
        });
        
        // Update basic statistics
        stat.count += 1;
        stat.sum += metric.value;
        stat.min = stat.min.min(metric.value);
        stat.max = stat.max.max(metric.value);
        stat.mean = stat.sum / stat.count as f64;
        stat.last_value = metric.value;
        stat.last_updated = metric.timestamp;
        
        // Calculate rate (simplified)
        let time_diff = (metric.timestamp - stat.last_updated).num_seconds() as f64;
        if time_diff > 0.0 {
            stat.rate_per_second = 1.0 / time_diff;
        }
        
        Ok(())
    }

    /// Update collection statistics
    async fn update_collection_stats(&self) {
        if let Ok(mut stats) = self.collection_stats.write() {
            stats.total_events_processed += 1;
            
            let buffer_len = if let Ok(points) = self.metric_points.read().await {
                points.len()
            } else {
                0
            };
            
            stats.buffer_utilization = (buffer_len as f64 / self.config.buffer_size as f64) * 100.0;
        }
    }

    /// Increment error count
    async fn increment_error_count(&self) {
        if let Ok(mut stats) = self.collection_stats.write() {
            stats.processing_errors += 1;
        }
    }

    /// Perform periodic maintenance tasks
    async fn perform_periodic_tasks(&self) -> Result<()> {
        let now = Utc::now();
        
        // Clean up old data
        self.cleanup_old_data(now).await?;
        
        // Update trend analysis if enabled
        if self.config.enable_trend_analysis {
            self.update_trend_analysis().await?;
        }
        
        // Check alert thresholds
        self.check_alert_thresholds().await?;
        
        *self.last_collection.write().await = now;
        
        Ok(())
    }

    /// Clean up old metric data
    async fn cleanup_old_data(&self, now: DateTime<Utc>) -> Result<()> {
        let cutoff = now - self.config.retention_duration;
        
        let mut points = self.metric_points.write().await;
        points.retain(|point| point.timestamp > cutoff);
        
        debug!("Cleaned up metrics older than {}", cutoff);
        Ok(())
    }

    /// Update trend analysis for metrics
    async fn update_trend_analysis(&self) -> Result<()> {
        // Simplified trend analysis - would be more sophisticated in production
        let mut stats = self.metric_statistics.write().await;
        
        for stat in stats.values_mut() {
            // Determine trend based on recent values (simplified)
            if stat.count > 10 {
                stat.trend = TrendDirection::Stable; // Would calculate actual trend
            }
        }
        
        Ok(())
    }

    /// Check alert thresholds
    async fn check_alert_thresholds(&self) -> Result<()> {
        let stats = self.metric_statistics.read().await;
        
        for (metric_name, threshold) in &self.config.alert_thresholds {
            if !threshold.enabled {
                continue;
            }
            
            if let Some(stat) = stats.get(metric_name) {
                if let Some(max_value) = threshold.max_value {
                    if stat.last_value > max_value {
                        warn!("Metric {} exceeded max threshold: {} > {}", 
                              metric_name, stat.last_value, max_value);
                    }
                }
                
                if let Some(min_value) = threshold.min_value {
                    if stat.last_value < min_value {
                        warn!("Metric {} below min threshold: {} < {}", 
                              metric_name, stat.last_value, min_value);
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Get metric statistics
    pub async fn get_metric_statistics(&self) -> HashMap<String, MetricStatistics> {
        self.metric_statistics.read().await.clone()
    }

    /// Get collection statistics
    pub async fn get_collection_statistics(&self) -> CollectionStatistics {
        self.collection_stats.read().await.clone()
    }

    /// Get recent metrics for a specific metric name
    pub async fn get_recent_metrics(&self, metric_name: &str, limit: usize) -> Vec<MetricPoint> {
        let points = self.metric_points.read().await;
        points.iter()
            .filter(|point| point.metric_name == metric_name)
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_metrics_collector_creation() {
        let (_tx, rx) = mpsc::unbounded_channel();
        let config = CollectorConfig::default();
        
        let (collector, _metric_rx) = MetricsCollector::new(config, rx);
        assert_eq!(collector.config.buffer_size, 100000);
    }

    #[tokio::test]
    async fn test_metric_extraction() {
        let (_tx, rx) = mpsc::unbounded_channel();
        let config = CollectorConfig::default();
        let (collector, _metric_rx) = MetricsCollector::new(config, rx);
        
        use crate::neural::monitoring::performance_channel::*;
        
        let event = PerformanceEventBuilder::new()
            .source(PerformanceSource::NeuralPredictor {
                model_name: "test_model".to_string(),
                predictor_id: "pred1".to_string(),
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
        
        let metrics = collector.extract_metrics_from_event(&event).await.unwrap();
        assert!(metrics.len() >= 5); // Should extract multiple metrics
        
        // Check for expected metrics
        let metric_names: Vec<String> = metrics.iter().map(|m| m.metric_name.clone()).collect();
        assert!(metric_names.contains(&"prediction.accuracy".to_string()));
        assert!(metric_names.contains(&"prediction.confidence".to_string()));
        assert!(metric_names.contains(&"prediction.latency".to_string()));
    }
}