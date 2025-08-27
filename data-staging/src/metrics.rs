//! Metrics Collection Module
//!
//! Collects and exports comprehensive metrics for the Data-Staging service including
//! processing rates, quality scores, error rates, and performance statistics.

use crate::DataQualityMetrics;
use prometheus::{
    Counter, Histogram, Gauge, IntCounter, IntGauge, 
    register_counter, register_histogram, register_gauge, register_int_counter, register_int_gauge,
    Registry, Encoder, TextEncoder
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use anyhow::Result;

/// Metrics collector for Data-Staging service
pub struct MetricsCollector {
    // Processing metrics
    messages_processed_total: IntCounter,
    messages_failed_total: IntCounter,
    messages_dlq_total: IntCounter,
    processing_duration: Histogram,
    
    // Quality metrics
    quality_score_gauge: Gauge,
    freshness_score_gauge: Gauge,
    completeness_score_gauge: Gauge,
    validity_score_gauge: Gauge,
    
    // Data age metrics
    data_age_seconds: Histogram,
    
    // EventBus metrics
    eventbus_publish_total: IntCounter,
    eventbus_publish_failed: IntCounter,
    eventbus_publish_duration: Histogram,
    
    // Redis metrics
    redis_consume_total: IntCounter,
    redis_consume_failed: IntCounter,
    redis_connection_errors: IntCounter,
    
    // System metrics
    active_connections: IntGauge,
    memory_usage_bytes: IntGauge,
    cpu_usage_percent: Gauge,
    
    // Batch metrics
    batch_size_histogram: Histogram,
    batch_processing_duration: Histogram,
    
    // Error category metrics
    json_parsing_errors: IntCounter,
    validation_errors: IntCounter,
    proto_transformation_errors: IntCounter,
    quality_check_errors: IntCounter,
    
    // Registry for exporting
    registry: Registry,
}

impl MetricsCollector {
    /// Create new metrics collector
    pub fn new() -> Result<Self> {
        info!("Initializing metrics collector");
        
        let registry = Registry::new();
        
        // Create and register all metrics
        let messages_processed_total = register_int_counter!(
            "data_staging_messages_processed_total",
            "Total number of messages processed by data-staging service"
        ).unwrap();
        
        let messages_failed_total = register_int_counter!(
            "data_staging_messages_failed_total", 
            "Total number of messages that failed processing"
        ).unwrap();
        
        let messages_dlq_total = register_int_counter!(
            "data_staging_messages_dlq_total",
            "Total number of messages sent to Dead Letter Queue"
        ).unwrap();
        
        let processing_duration = register_histogram!(
            "data_staging_processing_duration_seconds",
            "Time spent processing individual messages",
            vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]
        ).unwrap();
        
        let quality_score_gauge = register_gauge!(
            "data_staging_quality_score",
            "Current overall data quality score (0-1)"
        ).unwrap();
        
        let freshness_score_gauge = register_gauge!(
            "data_staging_freshness_score", 
            "Current data freshness score (0-1)"
        ).unwrap();
        
        let completeness_score_gauge = register_gauge!(
            "data_staging_completeness_score",
            "Current data completeness score (0-1)"
        ).unwrap();
        
        let validity_score_gauge = register_gauge!(
            "data_staging_validity_score",
            "Current data validity score (0-1)"
        ).unwrap();
        
        let data_age_seconds = register_histogram!(
            "data_staging_data_age_seconds",
            "Age of processed data in seconds",
            vec![1.0, 5.0, 10.0, 30.0, 60.0, 300.0, 600.0, 1800.0, 3600.0]
        ).unwrap();
        
        let eventbus_publish_total = register_int_counter!(
            "data_staging_eventbus_publish_total",
            "Total number of messages published to EventBus"
        ).unwrap();
        
        let eventbus_publish_failed = register_int_counter!(
            "data_staging_eventbus_publish_failed",
            "Total number of failed EventBus publishes"
        ).unwrap();
        
        let eventbus_publish_duration = register_histogram!(
            "data_staging_eventbus_publish_duration_seconds",
            "Time spent publishing to EventBus",
            vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]
        ).unwrap();
        
        let redis_consume_total = register_int_counter!(
            "data_staging_redis_consume_total",
            "Total number of messages consumed from Redis"
        ).unwrap();
        
        let redis_consume_failed = register_int_counter!(
            "data_staging_redis_consume_failed",
            "Total number of failed Redis consume operations"
        ).unwrap();
        
        let redis_connection_errors = register_int_counter!(
            "data_staging_redis_connection_errors",
            "Total number of Redis connection errors"
        ).unwrap();
        
        let active_connections = register_int_gauge!(
            "data_staging_active_connections",
            "Number of active connections"
        ).unwrap();
        
        let memory_usage_bytes = register_int_gauge!(
            "data_staging_memory_usage_bytes",
            "Current memory usage in bytes"
        ).unwrap();
        
        let cpu_usage_percent = register_gauge!(
            "data_staging_cpu_usage_percent",
            "Current CPU usage percentage"
        ).unwrap();
        
        let batch_size_histogram = register_histogram!(
            "data_staging_batch_size",
            "Size of message batches processed",
            vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0]
        ).unwrap();
        
        let batch_processing_duration = register_histogram!(
            "data_staging_batch_processing_duration_seconds", 
            "Time spent processing message batches",
            vec![0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0]
        ).unwrap();
        
        let json_parsing_errors = register_int_counter!(
            "data_staging_json_parsing_errors",
            "Total number of JSON parsing errors"
        ).unwrap();
        
        let validation_errors = register_int_counter!(
            "data_staging_validation_errors",
            "Total number of validation errors"
        ).unwrap();
        
        let proto_transformation_errors = register_int_counter!(
            "data_staging_proto_transformation_errors",
            "Total number of proto transformation errors"
        ).unwrap();
        
        let quality_check_errors = register_int_counter!(
            "data_staging_quality_check_errors", 
            "Total number of quality check errors"
        ).unwrap();
        
        info!("Metrics collector initialized successfully");
        
        Ok(Self {
            messages_processed_total,
            messages_failed_total,
            messages_dlq_total,
            processing_duration,
            quality_score_gauge,
            freshness_score_gauge,
            completeness_score_gauge,
            validity_score_gauge,
            data_age_seconds,
            eventbus_publish_total,
            eventbus_publish_failed,
            eventbus_publish_duration,
            redis_consume_total,
            redis_consume_failed,
            redis_connection_errors,
            active_connections,
            memory_usage_bytes,
            cpu_usage_percent,
            batch_size_histogram,
            batch_processing_duration,
            json_parsing_errors,
            validation_errors,
            proto_transformation_errors,
            quality_check_errors,
            registry,
        })
    }
    
    /// Record successful message processing
    pub async fn record_message_processed(&self, quality_metrics: &DataQualityMetrics) {
        debug!("Recording successful message processing");
        
        self.messages_processed_total.inc();
        
        // Update quality metrics
        self.quality_score_gauge.set(quality_metrics.overall_score as f64);
        self.freshness_score_gauge.set(quality_metrics.freshness_score as f64);
        self.completeness_score_gauge.set(quality_metrics.completeness_score as f64);
        self.validity_score_gauge.set(quality_metrics.validity_score as f64);
        
        // Record data age
        self.data_age_seconds.observe(quality_metrics.data_age_seconds as f64);
    }
    
    /// Record message processing failure
    pub async fn record_processing_error(&self) {
        debug!("Recording message processing error");
        self.messages_failed_total.inc();
    }
    
    /// Record message sent to DLQ
    pub async fn record_dlq_message(&self, error_category: &str) {
        debug!("Recording message sent to DLQ: {}", error_category);
        
        self.messages_dlq_total.inc();
        
        // Increment category-specific error counter
        match error_category {
            "JSON_PARSING" => self.json_parsing_errors.inc(),
            "VALIDATION" => self.validation_errors.inc(),
            "PROTO_TRANSFORMATION" => self.proto_transformation_errors.inc(),
            "QUALITY_CHECK" => self.quality_check_errors.inc(),
            _ => {
                debug!("Unknown error category: {}", error_category);
            }
        }
    }
    
    /// Record processing duration
    pub async fn record_processing_duration(&self, duration_seconds: f64) {
        debug!("Recording processing duration: {:.3}s", duration_seconds);
        self.processing_duration.observe(duration_seconds);
    }
    
    /// Record batch processing metrics
    pub async fn record_batch_processed(&self, batch_size: usize) {
        debug!("Recording batch processing: {} messages", batch_size);
        self.batch_size_histogram.observe(batch_size as f64);
    }
    
    /// Record batch processing duration
    pub async fn record_batch_duration(&self, duration_seconds: f64) {
        debug!("Recording batch duration: {:.3}s", duration_seconds);
        self.batch_processing_duration.observe(duration_seconds);
    }
    
    /// Record EventBus publish success
    pub async fn record_eventbus_publish_success(&self, duration_seconds: f64) {
        debug!("Recording EventBus publish success: {:.3}s", duration_seconds);
        self.eventbus_publish_total.inc();
        self.eventbus_publish_duration.observe(duration_seconds);
    }
    
    /// Record EventBus publish failure
    pub async fn record_eventbus_publish_failure(&self) {
        debug!("Recording EventBus publish failure");
        self.eventbus_publish_failed.inc();
    }
    
    /// Record Redis consume success
    pub async fn record_redis_consume_success(&self, message_count: usize) {
        debug!("Recording Redis consume success: {} messages", message_count);
        self.redis_consume_total.inc_by(message_count as u64);
    }
    
    /// Record Redis consume failure
    pub async fn record_redis_consume_failure(&self) {
        debug!("Recording Redis consume failure");
        self.redis_consume_failed.inc();
    }
    
    /// Record Redis connection error
    pub async fn record_redis_connection_error(&self) {
        debug!("Recording Redis connection error");
        self.redis_connection_errors.inc();
    }
    
    /// Update system resource metrics
    pub async fn update_system_metrics(&self) {
        // Update active connections
        self.active_connections.set(1); // Simplified - would be actual connection count
        
        // Update memory usage
        if let Ok(memory_info) = self.get_memory_usage().await {
            self.memory_usage_bytes.set(memory_info as i64);
        }
        
        // Update CPU usage
        if let Ok(cpu_usage) = self.get_cpu_usage().await {
            self.cpu_usage_percent.set(cpu_usage);
        }
    }
    
    /// Export metrics in Prometheus format
    pub async fn export_metrics(&self) -> Result<String> {
        debug!("Exporting metrics in Prometheus format");
        
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        
        let mut output = Vec::new();
        encoder.encode(&metric_families, &mut output)
            .map_err(|e| anyhow::anyhow!("Failed to encode metrics: {}", e))?;
        
        Ok(String::from_utf8(output)
            .map_err(|e| anyhow::anyhow!("Failed to convert metrics to string: {}", e))?)
    }
    
    /// Get current metrics summary
    pub async fn get_metrics_summary(&self) -> MetricsSummary {
        MetricsSummary {
            messages_processed: self.messages_processed_total.get(),
            messages_failed: self.messages_failed_total.get(),
            messages_dlq: self.messages_dlq_total.get(),
            current_quality_score: self.quality_score_gauge.get(),
            current_freshness_score: self.freshness_score_gauge.get(),
            current_completeness_score: self.completeness_score_gauge.get(),
            current_validity_score: self.validity_score_gauge.get(),
            eventbus_publishes: self.eventbus_publish_total.get(),
            eventbus_failures: self.eventbus_publish_failed.get(),
            redis_consumes: self.redis_consume_total.get(),
            redis_failures: self.redis_consume_failed.get(),
            active_connections: self.active_connections.get(),
            memory_usage_bytes: self.memory_usage_bytes.get(),
            cpu_usage_percent: self.cpu_usage_percent.get(),
            json_parsing_errors: self.json_parsing_errors.get(),
            validation_errors: self.validation_errors.get(),
            proto_transformation_errors: self.proto_transformation_errors.get(),
            quality_check_errors: self.quality_check_errors.get(),
        }
    }
    
    /// Calculate processing success rate
    pub async fn get_success_rate(&self) -> f64 {
        let processed = self.messages_processed_total.get() as f64;
        let failed = self.messages_failed_total.get() as f64;
        let total = processed + failed;
        
        if total > 0.0 {
            processed / total
        } else {
            0.0
        }
    }
    
    /// Calculate EventBus publish success rate
    pub async fn get_eventbus_success_rate(&self) -> f64 {
        let successful = self.eventbus_publish_total.get() as f64;
        let failed = self.eventbus_publish_failed.get() as f64;
        let total = successful + failed;
        
        if total > 0.0 {
            successful / total
        } else {
            0.0
        }
    }
    
    /// Check if service is healthy based on metrics
    pub async fn is_healthy(&self) -> bool {
        let success_rate = self.get_success_rate().await;
        let eventbus_success_rate = self.get_eventbus_success_rate().await;
        let quality_score = self.quality_score_gauge.get();
        
        // Service is healthy if:
        // - Success rate > 95%
        // - EventBus success rate > 95%
        // - Quality score > 0.7
        success_rate > 0.95 && eventbus_success_rate > 0.95 && quality_score > 0.7
    }
    
    /// Reset all metrics (useful for testing)
    pub async fn reset_metrics(&self) {
        info!("Resetting all metrics");
        
        // Reset counters by creating new instances
        // Note: Prometheus counters can't be reset directly in production
        // This is mainly for testing purposes
        
        self.quality_score_gauge.set(0.0);
        self.freshness_score_gauge.set(0.0);
        self.completeness_score_gauge.set(0.0);
        self.validity_score_gauge.set(0.0);
        self.cpu_usage_percent.set(0.0);
        self.active_connections.set(0);
        self.memory_usage_bytes.set(0);
    }
    
    /// Get memory usage in bytes
    async fn get_memory_usage(&self) -> Result<usize> {
        // This would use a system monitoring library in production
        // For now, return a placeholder value
        Ok(1024 * 1024 * 100) // 100 MB placeholder
    }
    
    /// Get CPU usage percentage
    async fn get_cpu_usage(&self) -> Result<f64> {
        // This would use a system monitoring library in production
        // For now, return a placeholder value
        Ok(5.0) // 5% placeholder
    }
}

/// Summary of current metrics
#[derive(Debug, Clone)]
pub struct MetricsSummary {
    pub messages_processed: u64,
    pub messages_failed: u64,
    pub messages_dlq: u64,
    pub current_quality_score: f64,
    pub current_freshness_score: f64,
    pub current_completeness_score: f64,
    pub current_validity_score: f64,
    pub eventbus_publishes: u64,
    pub eventbus_failures: u64,
    pub redis_consumes: u64,
    pub redis_failures: u64,
    pub active_connections: i64,
    pub memory_usage_bytes: i64,
    pub cpu_usage_percent: f64,
    pub json_parsing_errors: u64,
    pub validation_errors: u64,
    pub proto_transformation_errors: u64,
    pub quality_check_errors: u64,
}

impl MetricsSummary {
    /// Calculate overall success rate
    pub fn success_rate(&self) -> f64 {
        let total = self.messages_processed + self.messages_failed;
        if total > 0 {
            self.messages_processed as f64 / total as f64
        } else {
            0.0
        }
    }
    
    /// Calculate EventBus success rate
    pub fn eventbus_success_rate(&self) -> f64 {
        let total = self.eventbus_publishes + self.eventbus_failures;
        if total > 0 {
            self.eventbus_publishes as f64 / total as f64
        } else {
            0.0
        }
    }
    
    /// Get total error count
    pub fn total_errors(&self) -> u64 {
        self.messages_failed + self.messages_dlq + self.eventbus_failures + self.redis_failures
    }
    
    /// Check if metrics indicate healthy service
    pub fn is_healthy(&self) -> bool {
        self.success_rate() > 0.95 && 
        self.eventbus_success_rate() > 0.95 && 
        self.current_quality_score > 0.7
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    
    #[tokio::test]
    async fn test_metrics_collector_creation() {
        let collector = MetricsCollector::new().unwrap();
        
        // Test initial state
        let summary = collector.get_metrics_summary().await;
        assert_eq!(summary.messages_processed, 0);
        assert_eq!(summary.messages_failed, 0);
        assert_eq!(summary.current_quality_score, 0.0);
    }
    
    #[tokio::test]
    async fn test_message_processing_metrics() {
        let collector = MetricsCollector::new().unwrap();
        
        let quality_metrics = DataQualityMetrics {
            overall_score: 0.9,
            freshness_score: 0.95,
            completeness_score: 0.85,
            validity_score: 1.0,
            missing_required_fields: 0,
            present_optional_fields: 8,
            data_age_seconds: 10,
            validation_errors: vec![],
        };
        
        collector.record_message_processed(&quality_metrics).await;
        
        let summary = collector.get_metrics_summary().await;
        assert_eq!(summary.messages_processed, 1);
        assert_eq!(summary.current_quality_score, 0.9);
        assert_eq!(summary.current_freshness_score, 0.95);
    }
    
    #[tokio::test]
    async fn test_error_metrics() {
        let collector = MetricsCollector::new().unwrap();
        
        collector.record_processing_error().await;
        collector.record_dlq_message("JSON_PARSING").await;
        collector.record_dlq_message("VALIDATION").await;
        
        let summary = collector.get_metrics_summary().await;
        assert_eq!(summary.messages_failed, 1);
        assert_eq!(summary.messages_dlq, 2);
        assert_eq!(summary.json_parsing_errors, 1);
        assert_eq!(summary.validation_errors, 1);
    }
    
    #[tokio::test]
    async fn test_success_rate_calculation() {
        let collector = MetricsCollector::new().unwrap();
        
        let quality_metrics = DataQualityMetrics {
            overall_score: 0.9,
            freshness_score: 0.95,
            completeness_score: 0.85,
            validity_score: 1.0,
            missing_required_fields: 0,
            present_optional_fields: 8,
            data_age_seconds: 10,
            validation_errors: vec![],
        };
        
        // Record some successes and failures
        for _ in 0..9 {
            collector.record_message_processed(&quality_metrics).await;
        }
        collector.record_processing_error().await;
        
        let success_rate = collector.get_success_rate().await;
        assert_eq!(success_rate, 0.9); // 9 out of 10 successful
    }
    
    #[tokio::test]
    async fn test_health_check() {
        let collector = MetricsCollector::new().unwrap();
        
        let quality_metrics = DataQualityMetrics {
            overall_score: 0.95,
            freshness_score: 0.95,
            completeness_score: 0.95,
            validity_score: 1.0,
            missing_required_fields: 0,
            present_optional_fields: 8,
            data_age_seconds: 10,
            validation_errors: vec![],
        };
        
        // Record high success rate
        for _ in 0..100 {
            collector.record_message_processed(&quality_metrics).await;
            collector.record_eventbus_publish_success(0.001).await;
        }
        
        assert!(collector.is_healthy().await);
    }
    
    #[tokio::test]
    async fn test_metrics_export() {
        let collector = MetricsCollector::new().unwrap();
        
        let quality_metrics = DataQualityMetrics {
            overall_score: 0.9,
            freshness_score: 0.95,
            completeness_score: 0.85,
            validity_score: 1.0,
            missing_required_fields: 0,
            present_optional_fields: 8,
            data_age_seconds: 10,
            validation_errors: vec![],
        };
        
        collector.record_message_processed(&quality_metrics).await;
        
        let exported_metrics = collector.export_metrics().await.unwrap();
        
        assert!(exported_metrics.contains("data_staging_messages_processed_total"));
        assert!(exported_metrics.contains("data_staging_quality_score"));
    }
    
    #[test]
    fn test_metrics_summary_calculations() {
        let summary = MetricsSummary {
            messages_processed: 90,
            messages_failed: 10,
            messages_dlq: 5,
            current_quality_score: 0.85,
            current_freshness_score: 0.9,
            current_completeness_score: 0.8,
            current_validity_score: 0.95,
            eventbus_publishes: 85,
            eventbus_failures: 5,
            redis_consumes: 100,
            redis_failures: 2,
            active_connections: 1,
            memory_usage_bytes: 104857600,
            cpu_usage_percent: 5.0,
            json_parsing_errors: 3,
            validation_errors: 4,
            proto_transformation_errors: 2,
            quality_check_errors: 1,
        };
        
        assert_eq!(summary.success_rate(), 0.9);
        assert_eq!(summary.eventbus_success_rate(), 0.9444444444444444);
        assert_eq!(summary.total_errors(), 22);
        assert!(!summary.is_healthy()); // Quality score too low
    }
}