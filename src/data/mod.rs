//! Data module for time series processing and storage

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod storage;
pub mod cache;
pub mod pipeline;

// Re-export main types
pub use storage::{
    TimescaleDBStorage,
    TimeSeriesData as StorageTimeSeriesData,
    PredictionData,
    AggregatedStats,
};
pub use cache::{RedisCache, PredictionResult};
pub use pipeline::DataPipeline;

/// Time series data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesData {
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub indicators: HashMap<String, f64>,
}

/// Quality metrics for data monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetrics {
    pub data_completeness: f64,
    pub latency_ms: f64,
    pub error_rate: f64,
    pub overall_quality: f64,
    pub timestamp: DateTime<Utc>,
}

/// Platform metrics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformMetrics {
    pub total_records: u64,
    pub cache_hit_rate: f64,
    pub processing_throughput: f64,
    pub storage_usage_gb: f64,
    pub active_connections: u32,
    pub timestamp: DateTime<Utc>,
}

impl TimeSeriesData {
    /// Validate the time series data
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.symbol.is_empty() {
            anyhow::bail!("Symbol cannot be empty");
        }
        
        if self.high < self.low {
            anyhow::bail!("High price cannot be less than low price");
        }
        
        if self.open < 0.0 || self.high < 0.0 || self.low < 0.0 || self.close < 0.0 {
            anyhow::bail!("Prices cannot be negative");
        }
        
        if self.volume < 0.0 {
            anyhow::bail!("Volume cannot be negative");
        }
        
        Ok(())
    }
}

impl QualityMetrics {
    /// Create new quality metrics with calculated overall quality
    pub fn new(data_completeness: f64, latency_ms: f64, error_rate: f64) -> Self {
        let overall_quality = (data_completeness * 0.4) + ((1.0 - error_rate) * 0.4) + 
            ((1.0 - (latency_ms / 1000.0).min(1.0)) * 0.2);
        
        Self {
            data_completeness,
            latency_ms,
            error_rate,
            overall_quality,
            timestamp: Utc::now(),
        }
    }
}

impl PlatformMetrics {
    /// Create new platform metrics
    pub fn new(
        total_records: u64,
        cache_hit_rate: f64,
        processing_throughput: f64,
        storage_usage_gb: f64,
        active_connections: u32,
    ) -> Self {
        Self {
            total_records,
            cache_hit_rate,
            processing_throughput,
            storage_usage_gb,
            active_connections,
            timestamp: Utc::now(),
        }
    }
}