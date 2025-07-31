//! Data module for time series processing and storage

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod cache;
pub mod market_context;
pub mod storage;

// Re-export main types
pub use cache::{PredictionResult, RedisCache};
pub use market_context::MarketContext;
pub use storage::{
    AggregatedStats, PredictionData, TimeSeriesData as StorageTimeSeriesData, TimescaleDBStorage,
};

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
    // Storage compatibility fields
    pub source: Option<String>,
    pub entity: Option<String>,
    pub value: Option<f64>,
    pub metadata: Option<serde_json::Value>,
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

    /// Convert to storage format
    pub fn to_storage_format(&self) -> storage::TimeSeriesData {
        storage::TimeSeriesData {
            timestamp: self.timestamp,
            source: self
                .source
                .clone()
                .unwrap_or_else(|| "neural-trader".to_string()),
            entity: self.entity.clone().unwrap_or_else(|| self.symbol.clone()),
            value: self.value.unwrap_or(self.close),
            metadata: self.metadata.clone().or_else(|| {
                Some(serde_json::json!({
                    "symbol": self.symbol,
                    "open": self.open,
                    "high": self.high,
                    "low": self.low,
                    "close": self.close,
                    "volume": self.volume,
                    "indicators": self.indicators
                }))
            }),
        }
    }

    /// Create from storage format
    pub fn from_storage_format(data: &storage::TimeSeriesData) -> Self {
        let metadata = data.metadata.as_ref().and_then(|m| m.as_object());

        Self {
            symbol: metadata
                .and_then(|m| {
                    m.get("symbol")
                        .and_then(|v| v.as_str().map(|s| s.to_string()))
                })
                .unwrap_or_else(|| data.entity.clone()),
            timestamp: data.timestamp,
            open: metadata
                .and_then(|m| m.get("open").and_then(|v| v.as_f64()))
                .unwrap_or(data.value),
            high: metadata
                .and_then(|m| m.get("high").and_then(|v| v.as_f64()))
                .unwrap_or(data.value),
            low: metadata
                .and_then(|m| m.get("low").and_then(|v| v.as_f64()))
                .unwrap_or(data.value),
            close: metadata
                .and_then(|m| m.get("close").and_then(|v| v.as_f64()))
                .unwrap_or(data.value),
            volume: metadata
                .and_then(|m| m.get("volume").and_then(|v| v.as_f64()))
                .unwrap_or(0.0),
            indicators: metadata
                .and_then(|m| {
                    m.get("indicators")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                })
                .unwrap_or_default(),
            source: Some(data.source.clone()),
            entity: Some(data.entity.clone()),
            value: Some(data.value),
            metadata: data.metadata.clone(),
        }
    }
}

impl QualityMetrics {
    /// Create new quality metrics with calculated overall quality
    pub fn new(data_completeness: f64, latency_ms: f64, error_rate: f64) -> Self {
        let overall_quality = (data_completeness * 0.4)
            + ((1.0 - error_rate) * 0.4)
            + ((1.0 - (latency_ms / 1000.0).min(1.0)) * 0.2);

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
