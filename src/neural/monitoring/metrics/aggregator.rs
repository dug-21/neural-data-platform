//! Real-time Metrics Aggregator
//!
//! Aggregates collected metrics into meaningful statistics and time-series data.
//! Provides real-time aggregation with configurable windows and statistical functions.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, instrument, warn};

use super::collector::{MetricPoint, MetricUnit, TrendDirection};
use super::super::performance_channel::CircularBuffer;

/// Aggregated time series data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedDataPoint {
    pub timestamp: DateTime<Utc>,
    pub metric_name: String,
    pub aggregation_type: AggregationType,
    pub value: f64,
    pub count: u64,
    pub window_size: Duration,
    pub tags: HashMap<String, String>,
}

/// Types of aggregation functions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregationType {
    Sum,
    Average,
    Min,
    Max,
    Count,
    Rate,
    Percentile(f64), // P50, P95, P99, etc.
    StandardDeviation,
    Variance,
    Median,
    Range,
}

/// Time window configuration for aggregations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeWindow {
    pub duration: Duration,
    pub slide_interval: Duration,
    pub name: String,
}

/// Aggregation rule defining how to aggregate metrics
#[derive(Debug, Clone)]
pub struct AggregationRule {
    pub metric_pattern: String,
    pub aggregation_types: Vec<AggregationType>,
    pub time_windows: Vec<TimeWindow>,
    pub group_by_tags: Vec<String>,
    pub enabled: bool,
}

/// Real-time statistics for a metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealTimeStatistics {
    pub metric_name: String,
    pub current_value: f64,
    pub min_value: f64,
    pub max_value: f64,
    pub average_value: f64,
    pub count: u64,
    pub sum: f64,
    pub variance: f64,
    pub standard_deviation: f64,
    pub percentiles: HashMap<String, f64>,
    pub rate_per_second: f64,
    pub trend: TrendDirection,
    pub last_updated: DateTime<Utc>,
    pub window_size: Duration,
}

/// Aggregator configuration
#[derive(Debug, Clone)]
pub struct AggregatorConfig {
    pub buffer_size: usize,
    pub default_windows: Vec<TimeWindow>,
    pub aggregation_rules: Vec<AggregationRule>,
    pub enable_real_time_stats: bool,
    pub enable_percentiles: bool,
    pub percentiles: Vec<f64>,
    pub cleanup_interval: Duration,
    pub max_memory_usage_mb: u64,
}

impl Default for AggregatorConfig {
    fn default() -> Self {
        Self {
            buffer_size: 50000,
            default_windows: vec![
                TimeWindow {
                    duration: Duration::minutes(1),
                    slide_interval: Duration::seconds(10),
                    name: "1m".to_string(),
                },
                TimeWindow {
                    duration: Duration::minutes(5),
                    slide_interval: Duration::minutes(1),
                    name: "5m".to_string(),
                },
                TimeWindow {
                    duration: Duration::minutes(15),
                    slide_interval: Duration::minutes(5),
                    name: "15m".to_string(),
                },
                TimeWindow {
                    duration: Duration::hours(1),
                    slide_interval: Duration::minutes(15),
                    name: "1h".to_string(),
                },
            ],
            aggregation_rules: Vec::new(),
            enable_real_time_stats: true,
            enable_percentiles: true,
            percentiles: vec![50.0, 95.0, 99.0, 99.9],
            cleanup_interval: Duration::minutes(5),
            max_memory_usage_mb: 200,
        }
    }
}

/// Windowed data for time-series aggregation
struct WindowedData {
    buffer: CircularBuffer<MetricPoint>,
    window: TimeWindow,
    last_aggregation: DateTime<Utc>,
    aggregated_points: VecDeque<AggregatedDataPoint>,
}

impl WindowedData {
    fn new(window: TimeWindow, buffer_size: usize) -> Self {
        Self {
            buffer: CircularBuffer::new(buffer_size),
            window,
            last_aggregation: Utc::now(),
            aggregated_points: VecDeque::new(),
        }
    }
}

/// Real-time metrics aggregator
pub struct MetricsAggregator {
    config: AggregatorConfig,
    
    // Data storage
    windowed_data: Arc<RwLock<HashMap<String, HashMap<String, WindowedData>>>>, // metric_name -> window_name -> data
    real_time_stats: Arc<RwLock<HashMap<String, RealTimeStatistics>>>,
    
    // Communication
    metric_rx: mpsc::UnboundedReceiver<MetricPoint>,
    aggregated_tx: mpsc::UnboundedSender<AggregatedDataPoint>,
    
    // State
    last_cleanup: Arc<RwLock<DateTime<Utc>>>,
    processing_stats: Arc<RwLock<ProcessingStatistics>>,
}

/// Processing statistics
#[derive(Debug, Default)]
struct ProcessingStatistics {
    total_metrics_processed: u64,
    aggregations_computed: u64,
    memory_usage_mb: f64,
    processing_rate: f64,
    last_reset: DateTime<Utc>,
}

impl MetricsAggregator {
    /// Create a new metrics aggregator
    pub fn new(
        config: AggregatorConfig,
        metric_rx: mpsc::UnboundedReceiver<MetricPoint>,
    ) -> (Self, mpsc::UnboundedReceiver<AggregatedDataPoint>) {
        let (aggregated_tx, aggregated_rx) = mpsc::unbounded_channel();
        
        let aggregator = Self {
            config,
            windowed_data: Arc::new(RwLock::new(HashMap::new())),
            real_time_stats: Arc::new(RwLock::new(HashMap::new())),
            metric_rx,
            aggregated_tx,
            last_cleanup: Arc::new(RwLock::new(Utc::now())),
            processing_stats: Arc::new(RwLock::new(ProcessingStatistics {
                last_reset: Utc::now(),
                ..Default::default()
            })),
        };
        
        info!("Created MetricsAggregator with {} default windows", 
              aggregator.config.default_windows.len());
        
        (aggregator, aggregated_rx)
    }

    /// Start aggregating metrics
    #[instrument(skip(self))]
    pub async fn start_aggregation(&mut self) -> Result<()> {
        info!("Starting metrics aggregation");
        
        // Initialize windowed data for default windows
        self.initialize_windows().await?;
        
        let mut cleanup_interval = tokio::time::interval(
            self.config.cleanup_interval.to_std()?
        );
        
        loop {
            tokio::select! {
                // Process incoming metric points
                Some(metric) = self.metric_rx.recv() => {
                    if let Err(e) = self.process_metric_point(metric).await {
                        error!("Failed to process metric point: {}", e);
                    }
                }
                
                // Periodic cleanup and maintenance
                _ = cleanup_interval.tick() => {
                    if let Err(e) = self.perform_maintenance().await {
                        error!("Failed to perform maintenance: {}", e);
                    }
                }
            }
        }
    }

    /// Initialize windowed data structures
    async fn initialize_windows(&self) -> Result<()> {
        let mut windowed_data = self.windowed_data.write().await;
        
        // For now, we'll initialize windows dynamically as metrics arrive
        // This prevents memory waste for unused metrics
        debug!("Windowed data structures initialized");
        
        Ok(())
    }

    /// Process a single metric point
    #[instrument(skip(self, metric), fields(metric_name = %metric.metric_name))]
    async fn process_metric_point(&self, metric: MetricPoint) -> Result<()> {
        // Update real-time statistics
        if self.config.enable_real_time_stats {
            self.update_real_time_stats(&metric).await?;
        }
        
        // Add to windowed data structures
        self.add_to_windows(&metric).await?;
        
        // Check if any windows need aggregation
        self.check_and_aggregate_windows(&metric.metric_name).await?;
        
        // Update processing statistics
        self.update_processing_stats().await;
        
        Ok(())
    }

    /// Update real-time statistics for a metric
    async fn update_real_time_stats(&self, metric: &MetricPoint) -> Result<()> {
        let mut stats = self.real_time_stats.write().await;
        let stat = stats.entry(metric.metric_name.clone()).or_insert_with(|| {
            RealTimeStatistics {
                metric_name: metric.metric_name.clone(),
                current_value: 0.0,
                min_value: f64::INFINITY,
                max_value: f64::NEG_INFINITY,
                average_value: 0.0,
                count: 0,
                sum: 0.0,
                variance: 0.0,
                standard_deviation: 0.0,
                percentiles: HashMap::new(),
                rate_per_second: 0.0,
                trend: TrendDirection::Stable,
                last_updated: metric.timestamp,
                window_size: Duration::minutes(5), // Default window
            }
        });
        
        // Update basic statistics
        stat.current_value = metric.value;
        stat.min_value = stat.min_value.min(metric.value);
        stat.max_value = stat.max_value.max(metric.value);
        stat.count += 1;
        stat.sum += metric.value;
        stat.average_value = stat.sum / stat.count as f64;
        
        // Calculate variance and standard deviation (running calculation)
        if stat.count > 1 {
            let delta = metric.value - stat.average_value;
            stat.variance = ((stat.count - 1) as f64 * stat.variance + delta * delta) / stat.count as f64;
            stat.standard_deviation = stat.variance.sqrt();
        }
        
        // Calculate rate
        let time_diff = (metric.timestamp - stat.last_updated).num_seconds() as f64;
        if time_diff > 0.0 {
            stat.rate_per_second = 1.0 / time_diff;
        }
        
        stat.last_updated = metric.timestamp;
        
        Ok(())
    }

    /// Add metric to appropriate time windows
    async fn add_to_windows(&self, metric: &MetricPoint) -> Result<()> {
        let mut windowed_data = self.windowed_data.write().await;
        
        // Get or create metric entry
        let metric_windows = windowed_data.entry(metric.metric_name.clone())
            .or_insert_with(HashMap::new);
        
        // Add to each default window
        for window in &self.config.default_windows {
            let windowed_data_entry = metric_windows.entry(window.name.clone())
                .or_insert_with(|| WindowedData::new(window.clone(), self.config.buffer_size));
            
            windowed_data_entry.buffer.push(metric.clone());
        }
        
        Ok(())
    }

    /// Check and aggregate windows that are ready
    async fn check_and_aggregate_windows(&self, metric_name: &str) -> Result<()> {
        let mut windowed_data = self.windowed_data.write().await;
        
        if let Some(metric_windows) = windowed_data.get_mut(metric_name) {
            let now = Utc::now();
            
            for (window_name, windowed_data_entry) in metric_windows.iter_mut() {
                let time_since_last = now - windowed_data_entry.last_aggregation;
                
                if time_since_last >= windowed_data_entry.window.slide_interval {
                    if let Ok(aggregated_points) = self.aggregate_window_data(
                        metric_name,
                        window_name,
                        windowed_data_entry,
                        now,
                    ).await {
                        // Send aggregated points downstream
                        for point in aggregated_points {
                            if let Err(e) = self.aggregated_tx.send(point) {
                                warn!("Failed to send aggregated point: {}", e);
                            }
                        }
                    }
                    
                    windowed_data_entry.last_aggregation = now;
                }
            }
        }
        
        Ok(())
    }

    /// Aggregate data within a window
    async fn aggregate_window_data(
        &self,
        metric_name: &str,
        window_name: &str,
        windowed_data: &mut WindowedData,
        timestamp: DateTime<Utc>,
    ) -> Result<Vec<AggregatedDataPoint>> {
        let cutoff = timestamp - windowed_data.window.duration;
        
        // Get relevant data points within the window
        let window_points: Vec<&MetricPoint> = windowed_data.buffer.iter()
            .filter(|point| point.timestamp > cutoff)
            .collect();
        
        if window_points.is_empty() {
            return Ok(Vec::new());
        }
        
        let mut aggregated_points = Vec::new();
        
        // Perform different aggregations
        let aggregation_types = vec![
            AggregationType::Average,
            AggregationType::Min,
            AggregationType::Max,
            AggregationType::Count,
            AggregationType::Sum,
        ];
        
        if self.config.enable_percentiles {
            for percentile in &self.config.percentiles {
                aggregation_types.push(AggregationType::Percentile(*percentile));
            }
        }
        
        for agg_type in aggregation_types {
            if let Some(aggregated_value) = self.compute_aggregation(&window_points, &agg_type) {
                aggregated_points.push(AggregatedDataPoint {
                    timestamp,
                    metric_name: metric_name.to_string(),
                    aggregation_type: agg_type,
                    value: aggregated_value,
                    count: window_points.len() as u64,
                    window_size: windowed_data.window.duration,
                    tags: HashMap::from([
                        ("window".to_string(), window_name.to_string()),
                    ]),
                });
            }
        }
        
        // Keep limited history of aggregated points
        for point in &aggregated_points {
            windowed_data.aggregated_points.push_back(point.clone());
            if windowed_data.aggregated_points.len() > 1000 {
                windowed_data.aggregated_points.pop_front();
            }
        }
        
        Ok(aggregated_points)
    }

    /// Compute aggregation value
    fn compute_aggregation(&self, points: &[&MetricPoint], agg_type: &AggregationType) -> Option<f64> {
        if points.is_empty() {
            return None;
        }
        
        let values: Vec<f64> = points.iter().map(|p| p.value).collect();
        
        match agg_type {
            AggregationType::Sum => Some(values.iter().sum()),
            AggregationType::Average => Some(values.iter().sum::<f64>() / values.len() as f64),
            AggregationType::Min => values.iter().fold(f64::INFINITY, |a, &b| a.min(b)).into(),
            AggregationType::Max => values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)).into(),
            AggregationType::Count => Some(values.len() as f64),
            AggregationType::Rate => {
                if points.len() < 2 {
                    return Some(0.0);
                }
                
                let time_span = (points.last().unwrap().timestamp - points.first().unwrap().timestamp)
                    .num_seconds() as f64;
                
                if time_span > 0.0 {
                    Some(values.len() as f64 / time_span)
                } else {
                    Some(0.0)
                }
            }
            AggregationType::Percentile(p) => {
                let mut sorted_values = values.clone();
                sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
                
                let index = ((*p / 100.0) * (sorted_values.len() - 1) as f64).round() as usize;
                sorted_values.get(index).copied()
            }
            AggregationType::StandardDeviation => {
                if values.len() < 2 {
                    return Some(0.0);
                }
                
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let variance = values.iter()
                    .map(|v| (v - mean).powi(2))
                    .sum::<f64>() / values.len() as f64;
                
                Some(variance.sqrt())
            }
            AggregationType::Variance => {
                if values.len() < 2 {
                    return Some(0.0);
                }
                
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let variance = values.iter()
                    .map(|v| (v - mean).powi(2))
                    .sum::<f64>() / values.len() as f64;
                
                Some(variance)
            }
            AggregationType::Median => {
                let mut sorted_values = values.clone();
                sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
                
                let len = sorted_values.len();
                if len % 2 == 0 {
                    Some((sorted_values[len / 2 - 1] + sorted_values[len / 2]) / 2.0)
                } else {
                    Some(sorted_values[len / 2])
                }
            }
            AggregationType::Range => {
                let min = values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
                let max = values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
                Some(max - min)
            }
        }
    }

    /// Update processing statistics
    async fn update_processing_stats(&self) {
        if let Ok(mut stats) = self.processing_stats.write() {
            stats.total_metrics_processed += 1;
            stats.processing_rate = stats.total_metrics_processed as f64 
                / (Utc::now() - stats.last_reset).num_seconds() as f64;
        }
    }

    /// Perform periodic maintenance
    async fn perform_maintenance(&self) -> Result<()> {
        let now = Utc::now();
        
        // Clean up old windowed data
        self.cleanup_old_data(now).await?;
        
        // Update memory usage statistics
        self.update_memory_usage().await?;
        
        // Check memory limits
        self.check_memory_limits().await?;
        
        *self.last_cleanup.write().await = now;
        
        debug!("Performed aggregator maintenance");
        Ok(())
    }

    /// Clean up old data to prevent memory leaks
    async fn cleanup_old_data(&self, now: DateTime<Utc>) -> Result<()> {
        let mut windowed_data = self.windowed_data.write().await;
        
        for (_, metric_windows) in windowed_data.iter_mut() {
            for (_, windowed_data_entry) in metric_windows.iter_mut() {
                let cutoff = now - windowed_data_entry.window.duration * 2; // Keep extra buffer
                
                // Clean up buffered points
                while let Some(point) = windowed_data_entry.buffer.iter().next() {
                    if point.timestamp <= cutoff {
                        // Remove old point (simplified - would need actual removal method)
                    } else {
                        break;
                    }
                }
                
                // Clean up aggregated points
                windowed_data_entry.aggregated_points.retain(|point| point.timestamp > cutoff);
            }
        }
        
        Ok(())
    }

    /// Update memory usage statistics (simplified)
    async fn update_memory_usage(&self) -> Result<()> {
        if let Ok(mut stats) = self.processing_stats.write() {
            // Simplified memory calculation - would use actual memory profiling in production
            let windowed_data = self.windowed_data.read().await;
            let total_metrics = windowed_data.len();
            let total_windows: usize = windowed_data.values()
                .map(|windows| windows.len())
                .sum();
            
            // Rough estimate: each metric point ~100 bytes, each window ~1KB overhead
            stats.memory_usage_mb = (total_metrics * 100 + total_windows * 1024) as f64 / 1024.0 / 1024.0;
        }
        
        Ok(())
    }

    /// Check if memory usage exceeds limits
    async fn check_memory_limits(&self) -> Result<()> {
        if let Ok(stats) = self.processing_stats.read() {
            if stats.memory_usage_mb > self.config.max_memory_usage_mb as f64 {
                warn!("Memory usage {} MB exceeds limit {} MB", 
                      stats.memory_usage_mb, self.config.max_memory_usage_mb);
                
                // Could trigger more aggressive cleanup here
            }
        }
        
        Ok(())
    }

    /// Get real-time statistics for all metrics
    pub async fn get_real_time_statistics(&self) -> HashMap<String, RealTimeStatistics> {
        self.real_time_stats.read().await.clone()
    }

    /// Get real-time statistics for a specific metric
    pub async fn get_metric_statistics(&self, metric_name: &str) -> Option<RealTimeStatistics> {
        self.real_time_stats.read().await.get(metric_name).cloned()
    }

    /// Get aggregated data for a metric and window
    pub async fn get_aggregated_data(
        &self, 
        metric_name: &str, 
        window_name: &str, 
        limit: Option<usize>
    ) -> Vec<AggregatedDataPoint> {
        let windowed_data = self.windowed_data.read().await;
        
        if let Some(metric_windows) = windowed_data.get(metric_name) {
            if let Some(windowed_data_entry) = metric_windows.get(window_name) {
                let mut points: Vec<AggregatedDataPoint> = windowed_data_entry
                    .aggregated_points
                    .iter()
                    .cloned()
                    .collect();
                
                if let Some(limit) = limit {
                    points.truncate(limit);
                }
                
                return points;
            }
        }
        
        Vec::new()
    }

    /// Get processing statistics
    pub async fn get_processing_statistics(&self) -> ProcessingStatistics {
        self.processing_stats.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_aggregator_creation() {
        let (_tx, rx) = mpsc::unbounded_channel();
        let config = AggregatorConfig::default();
        
        let (aggregator, _agg_rx) = MetricsAggregator::new(config, rx);
        assert_eq!(aggregator.config.buffer_size, 50000);
    }

    #[tokio::test]
    async fn test_aggregation_computation() {
        let (_tx, rx) = mpsc::unbounded_channel();
        let config = AggregatorConfig::default();
        let (aggregator, _agg_rx) = MetricsAggregator::new(config, rx);
        
        let points = vec![
            MetricPoint {
                timestamp: Utc::now(),
                metric_name: "test".to_string(),
                value: 10.0,
                tags: HashMap::new(),
                source: "test".to_string(),
                unit: MetricUnit::Count,
            },
            MetricPoint {
                timestamp: Utc::now(),
                metric_name: "test".to_string(),
                value: 20.0,
                tags: HashMap::new(),
                source: "test".to_string(),
                unit: MetricUnit::Count,
            },
            MetricPoint {
                timestamp: Utc::now(),
                metric_name: "test".to_string(),
                value: 30.0,
                tags: HashMap::new(),
                source: "test".to_string(),
                unit: MetricUnit::Count,
            },
        ];
        
        let point_refs: Vec<&MetricPoint> = points.iter().collect();
        
        assert_eq!(aggregator.compute_aggregation(&point_refs, &AggregationType::Sum), Some(60.0));
        assert_eq!(aggregator.compute_aggregation(&point_refs, &AggregationType::Average), Some(20.0));
        assert_eq!(aggregator.compute_aggregation(&point_refs, &AggregationType::Min), Some(10.0));
        assert_eq!(aggregator.compute_aggregation(&point_refs, &AggregationType::Max), Some(30.0));
        assert_eq!(aggregator.compute_aggregation(&point_refs, &AggregationType::Count), Some(3.0));
        assert_eq!(aggregator.compute_aggregation(&point_refs, &AggregationType::Median), Some(20.0));
    }
}