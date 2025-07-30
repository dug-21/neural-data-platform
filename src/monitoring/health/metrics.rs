//! Health Monitoring Metrics Collection
//!
//! Performance metrics collection and calculation for system health monitoring.

use anyhow::Result;
use chrono::Utc;
use metrics::{counter, histogram};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use super::config::{ComponentType, PerformanceMetrics};

/// Metrics collector for performance data
#[derive(Debug, Clone)]
pub struct MetricsCollector {
    start_time: Instant,
    latency_histogram: Arc<Mutex<Vec<Duration>>>,
    throughput_counter: Arc<Mutex<u64>>,
    error_counter: Arc<Mutex<u64>>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            latency_histogram: Arc::new(Mutex::new(Vec::new())),
            throughput_counter: Arc::new(Mutex::new(0)),
            error_counter: Arc::new(Mutex::new(0)),
        }
    }

    /// Record a latency measurement
    pub async fn record_latency(&self, _component: &ComponentType, latency: Duration) {
        let mut histogram = self.latency_histogram.lock().await;
        histogram.push(latency);

        // Keep only last 1000 measurements
        if histogram.len() > 1000 {
            histogram.drain(0..100);
        }

        // Record to metrics crate
        histogram!("component_response_time").record(latency.as_secs_f64());
    }

    /// Record a throughput event
    pub async fn record_throughput(&self) {
        let mut counter = self.throughput_counter.lock().await;
        *counter += 1;
    }

    /// Record an error
    pub async fn record_error(&self, _component: &ComponentType, _error: &str) {
        let mut counter = self.error_counter.lock().await;
        *counter += 1;

        counter!("component_errors_total").increment(1);
    }

    /// Calculate performance metrics
    pub async fn calculate_metrics(&self) -> Result<PerformanceMetrics> {
        let histogram = self.latency_histogram.lock().await;
        let throughput = *self.throughput_counter.lock().await;
        let errors = *self.error_counter.lock().await;

        let mut latencies = histogram.clone();
        latencies.sort();

        let latency_p50 = latencies
            .get(latencies.len() / 2)
            .copied()
            .unwrap_or(Duration::from_millis(0));
        let latency_p95 = latencies
            .get((latencies.len() * 95) / 100)
            .copied()
            .unwrap_or(Duration::from_millis(0));
        let latency_p99 = latencies
            .get((latencies.len() * 99) / 100)
            .copied()
            .unwrap_or(Duration::from_millis(0));

        let elapsed = self.start_time.elapsed();
        let throughput_per_sec = if elapsed.as_secs() > 0 {
            throughput as f64 / elapsed.as_secs() as f64
        } else {
            0.0
        };

        let error_rate = if throughput > 0 {
            errors as f64 / throughput as f64
        } else {
            0.0
        };

        Ok(PerformanceMetrics {
            latency_p50,
            latency_p95,
            latency_p99,
            throughput_per_sec,
            error_rate,
            cpu_usage_percent: self.get_cpu_usage().await,
            memory_usage_mb: self.get_memory_usage().await,
            disk_usage_percent: self.get_disk_usage().await,
            network_bytes_in: 0,
            network_bytes_out: 0,
            timestamp: Utc::now(),
        })
    }

    /// Reset all metrics
    pub async fn reset(&self) {
        let mut histogram = self.latency_histogram.lock().await;
        let mut throughput = self.throughput_counter.lock().await;
        let mut errors = self.error_counter.lock().await;

        histogram.clear();
        *throughput = 0;
        *errors = 0;
    }

    /// Get current error count
    pub async fn get_error_count(&self) -> u64 {
        *self.error_counter.lock().await
    }

    /// Get current throughput count
    pub async fn get_throughput_count(&self) -> u64 {
        *self.throughput_counter.lock().await
    }

    /// Get latency statistics
    pub async fn get_latency_stats(&self) -> (Duration, Duration, Duration) {
        let histogram = self.latency_histogram.lock().await;
        let mut latencies = histogram.clone();
        latencies.sort();

        let p50 = latencies
            .get(latencies.len() / 2)
            .copied()
            .unwrap_or(Duration::from_millis(0));
        let p95 = latencies
            .get((latencies.len() * 95) / 100)
            .copied()
            .unwrap_or(Duration::from_millis(0));
        let p99 = latencies
            .get((latencies.len() * 99) / 100)
            .copied()
            .unwrap_or(Duration::from_millis(0));

        (p50, p95, p99)
    }

    /// Get CPU usage (placeholder implementation)
    async fn get_cpu_usage(&self) -> f64 {
        // In a real implementation, this would query system CPU usage
        // For now, return a mock value
        45.0
    }

    /// Get memory usage (placeholder implementation)
    async fn get_memory_usage(&self) -> u64 {
        // In a real implementation, this would query system memory usage
        // For now, return a mock value
        512
    }

    /// Get disk usage (placeholder implementation)
    async fn get_disk_usage(&self) -> f64 {
        // In a real implementation, this would query system disk usage
        // For now, return a mock value
        25.0
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}