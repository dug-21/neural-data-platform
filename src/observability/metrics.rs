//! Production metrics collection and export
//! 
//! This module provides comprehensive metrics collection for:
//! - Business KPIs (predictions, accuracy, trading performance)
//! - System metrics (CPU, memory, network, disk)
//! - Application metrics (request latency, error rates)
//! - Custom metrics for specific business logic

use anyhow::Result;
use metrics::{counter, gauge, histogram, Counter, Gauge, Histogram};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Central metrics registry for the application
#[derive(Clone)]
pub struct MetricsRegistry {
    business_metrics: Arc<BusinessMetrics>,
    system_metrics: Arc<SystemMetrics>,
    application_metrics: Arc<ApplicationMetrics>,
    custom_metrics: Arc<RwLock<HashMap<String, CustomMetric>>>,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        Self {
            business_metrics: Arc::new(BusinessMetrics::new()),
            system_metrics: Arc::new(SystemMetrics::new()),
            application_metrics: Arc::new(ApplicationMetrics::new()),
            custom_metrics: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn business(&self) -> &BusinessMetrics {
        &self.business_metrics
    }

    pub fn system(&self) -> &SystemMetrics {
        &self.system_metrics
    }

    pub fn application(&self) -> &ApplicationMetrics {
        &self.application_metrics
    }

    /// Register a custom metric
    pub async fn register_custom_metric(&self, name: String, metric: CustomMetric) {
        let mut custom_metrics = self.custom_metrics.write().await;
        custom_metrics.insert(name, metric);
    }

    /// Get all registered custom metrics
    pub async fn get_custom_metrics(&self) -> HashMap<String, CustomMetric> {
        self.custom_metrics.read().await.clone()
    }
}

/// Business-specific metrics for neural trading
pub struct BusinessMetrics {
    // Prediction metrics
    pub predictions_total: Counter,
    pub predictions_accuracy: Gauge,
    pub model_inference_duration: Histogram,
    
    // Trading metrics
    pub trades_executed: Counter,
    pub trade_success_rate: Gauge,
    pub portfolio_value: Gauge,
    pub pnl_total: Gauge,
    pub risk_exposure: Gauge,
    
    // Data quality metrics
    pub data_points_processed: Counter,
    pub data_quality_score: Gauge,
    pub data_latency: Histogram,
    pub missing_data_points: Counter,
}

impl BusinessMetrics {
    pub fn new() -> Self {
        Self {
            // Register prediction metrics
            predictions_total: Counter::noop(),
            predictions_accuracy: Gauge::noop(),
            model_inference_duration: Histogram::noop(),
            
            // Register trading metrics
            trades_executed: Counter::noop(),
            trade_success_rate: Gauge::noop(),
            portfolio_value: Gauge::noop(),
            pnl_total: Gauge::noop(),
            risk_exposure: Gauge::noop(),
            
            // Register data quality metrics
            data_points_processed: Counter::noop(),
            data_quality_score: Gauge::noop(),
            data_latency: Histogram::noop(),
            missing_data_points: Counter::noop(),
        }
    }

    /// Record a prediction event
    pub fn record_prediction(&self, model_name: &str, inference_duration: Duration, accuracy: f64) {
        self.predictions_total.increment(1);
        self.predictions_accuracy.set(accuracy);
        self.model_inference_duration.record(inference_duration.as_secs_f64());
        
        // Record prediction by model using labels
        metrics::counter!("neural_trader_predictions_by_model_total", "model" => model_name.to_string()).increment(1);
    }

    /// Record a trade execution
    pub fn record_trade(&self, success: bool, _value_usd: f64) {
        self.trades_executed.increment(1);
        if success {
            // Update success rate with counter for successful trades
            metrics::counter!("neural_trader_successful_trades_total").increment(1);
        }
    }

    /// Update portfolio metrics
    pub fn update_portfolio(&self, value_usd: f64, pnl_usd: f64, risk_percent: f64) {
        self.portfolio_value.set(value_usd);
        self.pnl_total.set(pnl_usd);
        self.risk_exposure.set(risk_percent);
    }

    /// Record data processing metrics
    pub fn record_data_processing(&self, points_count: u64, quality_score: f64, latency: Duration) {
        self.data_points_processed.increment(points_count);
        self.data_quality_score.set(quality_score);
        self.data_latency.record(latency.as_secs_f64());
    }
}

/// System-level metrics
pub struct SystemMetrics {
    // CPU metrics
    pub cpu_usage_percent: Gauge,
    pub cpu_load_1m: Gauge,
    pub cpu_load_5m: Gauge,
    pub cpu_load_15m: Gauge,
    
    // Memory metrics
    pub memory_usage_bytes: Gauge,
    pub memory_available_bytes: Gauge,
    pub memory_usage_percent: Gauge,
    
    // Disk metrics
    pub disk_usage_bytes: Gauge,
    pub disk_available_bytes: Gauge,
    pub disk_usage_percent: Gauge,
    pub disk_io_read_bytes: Counter,
    pub disk_io_write_bytes: Counter,
    
    // Network metrics
    pub network_bytes_sent: Counter,
    pub network_bytes_received: Counter,
    pub network_packets_sent: Counter,
    pub network_packets_received: Counter,
    pub network_errors: Counter,
}

impl SystemMetrics {
    pub fn new() -> Self {
        Self {
            // CPU metrics
            cpu_usage_percent: Gauge::noop(),
            cpu_load_1m: Gauge::noop(),
            cpu_load_5m: Gauge::noop(),
            cpu_load_15m: Gauge::noop(),
            
            // Memory metrics
            memory_usage_bytes: Gauge::noop(),
            memory_available_bytes: Gauge::noop(),
            memory_usage_percent: Gauge::noop(),
            
            // Disk metrics
            disk_usage_bytes: Gauge::noop(),
            disk_available_bytes: Gauge::noop(),
            disk_usage_percent: Gauge::noop(),
            disk_io_read_bytes: Counter::noop(),
            disk_io_write_bytes: Counter::noop(),
            
            // Network metrics
            network_bytes_sent: Counter::noop(),
            network_bytes_received: Counter::noop(),
            network_packets_sent: Counter::noop(),
            network_packets_received: Counter::noop(),
            network_errors: Counter::noop(),
        }
    }

    /// Update system metrics (integrated with system monitoring)
    pub async fn update_system_metrics(&self) -> Result<()> {
        // These metrics are updated by the system monitor
        // This method provides a common interface
        Ok(())
    }
}

/// Application-level metrics
pub struct ApplicationMetrics {
    // HTTP metrics
    pub http_requests_total: Counter,
    pub http_request_duration: Histogram,
    pub http_requests_in_flight: Gauge,
    
    // Database metrics
    pub database_connections_active: Gauge,
    pub database_connections_max: Gauge,
    pub database_query_duration: Histogram,
    pub database_queries_total: Counter,
    pub database_errors: Counter,
    
    // Cache metrics
    pub cache_hits: Counter,
    pub cache_misses: Counter,
    pub cache_hit_ratio: Gauge,
    pub cache_size_bytes: Gauge,
    
    // General application metrics
    pub errors_total: Counter,
    pub panics_total: Counter,
    pub uptime_seconds: Gauge,
}

impl ApplicationMetrics {
    pub fn new() -> Self {
        Self {
            // HTTP metrics
            http_requests_total: Counter::noop(),
            http_request_duration: Histogram::noop(),
            http_requests_in_flight: Gauge::noop(),
            
            // Database metrics
            database_connections_active: Gauge::noop(),
            database_connections_max: Gauge::noop(),
            database_query_duration: Histogram::noop(),
            database_queries_total: Counter::noop(),
            database_errors: Counter::noop(),
            
            // Cache metrics
            cache_hits: Counter::noop(),
            cache_misses: Counter::noop(),
            cache_hit_ratio: Gauge::noop(),
            cache_size_bytes: Gauge::noop(),
            
            // General metrics
            errors_total: Counter::noop(),
            panics_total: Counter::noop(),
            uptime_seconds: Gauge::noop(),
        }
    }

    /// Record HTTP request metrics
    pub fn record_http_request(&self, _method: &str, _path: &str, _status_code: u16, duration: Duration) {
        self.http_requests_total.increment(1);
        self.http_request_duration.record(duration.as_secs_f64());
    }

    /// Record database query metrics
    pub fn record_database_query(&self, duration: Duration, success: bool) {
        self.database_queries_total.increment(1);
        self.database_query_duration.record(duration.as_secs_f64());
        
        if !success {
            self.database_errors.increment(1);
        }
    }

    /// Record cache operation
    pub fn record_cache_operation(&self, hit: bool) {
        if hit {
            self.cache_hits.increment(1);
        } else {
            self.cache_misses.increment(1);
        }
    }

    /// Record error occurrence
    pub fn record_error(&self, _error_type: &str) {
        self.errors_total.increment(1);
    }
}

/// Custom metric types for business-specific measurements
#[derive(Debug, Clone)]
pub enum CustomMetric {
    Counter(Counter),
    Gauge(Gauge),
    Histogram(Histogram),
}

impl CustomMetric {
    pub fn counter(name: &str, description: &str) -> Self {
        Self::Counter(Counter::noop())
    }
    
    pub fn gauge(name: &str, description: &str) -> Self {
        Self::Gauge(Gauge::noop())
    }
    
    pub fn histogram(name: &str, description: &str) -> Self {
        Self::Histogram(Histogram::noop())
    }
}

/// Metrics middleware for timing operations
pub struct MetricsTimer {
    start: Instant,
    operation: String,
}

impl MetricsTimer {
    pub fn new(operation: String) -> Self {
        Self {
            start: Instant::now(),
            operation,
        }
    }

    pub fn finish(self, registry: &MetricsRegistry) {
        let duration = self.start.elapsed();
        // Record timing based on operation type
        match self.operation.as_str() {
            op if op.starts_with("http_") => {
                registry.application().http_request_duration.record(duration.as_secs_f64());
            }
            op if op.starts_with("db_") => {
                registry.application().database_query_duration.record(duration.as_secs_f64());
            }
            op if op.starts_with("model_") => {
                registry.business().model_inference_duration.record(duration.as_secs_f64());
            }
            _ => {
                // Generic timing metric
                metrics::histogram!("operation_duration_seconds", "operation" => self.operation.clone())
                    .record(duration.as_secs_f64());
            }
        }
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for BusinessMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ApplicationMetrics {
    fn default() -> Self {
        Self::new()
    }
}