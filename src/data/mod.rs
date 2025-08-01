//! Data module for time series processing and storage

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod cache;
pub mod market_context;
pub mod storage;
pub mod sector_mapper;
pub mod data_converter;

// Re-export main types
pub use cache::{PredictionResult, RedisCache};
pub use market_context::MarketContext;
pub use storage::{
    AggregatedStats, PredictionData, TimeSeriesData as StorageTimeSeriesData, TimescaleDBStorage,
};

/// Time series data point - enhanced for vendor model integration
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
    
    // Enhanced fields for vendor model integration
    /// Raw price values for time series analysis
    pub values: Vec<f64>,
    /// Timestamps corresponding to values (for time-based feature engineering)
    pub timestamps: Vec<DateTime<Utc>>,
    /// Additional metadata for vendor model conversion
    pub metadata_map: HashMap<String, serde_json::Value>,
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
    /// Create new TimeSeriesData with enhanced fields
    pub fn new(symbol: String, timestamp: DateTime<Utc>) -> Self {
        Self {
            symbol,
            timestamp,
            open: 0.0,
            high: 0.0,
            low: 0.0,
            close: 0.0,
            volume: 0.0,
            indicators: HashMap::new(),
            source: None,
            entity: None,
            value: None,
            metadata: None,
            values: Vec::new(),
            timestamps: Vec::new(),
            metadata_map: HashMap::new(),
        }
    }
    
    /// Add a price value to the time series
    pub fn add_value(&mut self, value: f64, timestamp: DateTime<Utc>) {
        self.values.push(value);
        self.timestamps.push(timestamp);
    }
    
    /// Add multiple values at once
    pub fn add_values(&mut self, values: Vec<f64>, timestamps: Vec<DateTime<Utc>>) {
        if values.len() != timestamps.len() {
            tracing::warn!("Values and timestamps length mismatch: {} vs {}", 
                values.len(), timestamps.len());
        }
        
        self.values.extend(values);
        self.timestamps.extend(timestamps);
    }
    
    /// Get the most recent value
    pub fn latest_value(&self) -> Option<f64> {
        self.values.last().copied()
    }
    
    /// Get values in a specific time range
    pub fn get_values_in_range(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<(f64, DateTime<Utc>)> {
        self.values
            .iter()
            .zip(self.timestamps.iter())
            .filter(|(_, &ts)| ts >= start && ts <= end)
            .map(|(&val, &ts)| (val, ts))
            .collect()
    }

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
        
        // Validate enhanced fields
        if self.values.len() != self.timestamps.len() {
            anyhow::bail!("Values and timestamps must have the same length");
        }

        Ok(())
    }

    /// Convert to storage format
    pub fn to_storage_format(&self) -> storage::TimeSeriesData {
        let mut metadata_json = serde_json::json!({
            "symbol": self.symbol,
            "open": self.open,
            "high": self.high,
            "low": self.low,
            "close": self.close,
            "volume": self.volume,
            "indicators": self.indicators,
            "values": self.values,
            "timestamps": self.timestamps,
            "metadata_map": self.metadata_map
        });
        
        // Merge with existing metadata if present
        if let Some(existing_metadata) = &self.metadata {
            if let (Some(existing_obj), Some(new_obj)) = (existing_metadata.as_object(), metadata_json.as_object_mut()) {
                for (key, value) in existing_obj {
                    new_obj.insert(key.clone(), value.clone());
                }
            }
        }
        
        storage::TimeSeriesData {
            timestamp: self.timestamp,
            source: self
                .source
                .clone()
                .unwrap_or_else(|| "neural-trader".to_string()),
            entity: self.entity.clone().unwrap_or_else(|| self.symbol.clone()),
            value: self.value.unwrap_or(self.close),
            metadata: Some(metadata_json),
        }
    }

    /// Create from storage format
    pub fn from_storage_format(data: &storage::TimeSeriesData) -> Self {
        let metadata = data.metadata.as_ref().and_then(|m| m.as_object());

        let values: Vec<f64> = metadata
            .and_then(|m| {
                m.get("values")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
            })
            .unwrap_or_else(|| vec![data.value]);
            
        let timestamps: Vec<DateTime<Utc>> = metadata
            .and_then(|m| {
                m.get("timestamps")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
            })
            .unwrap_or_else(|| vec![data.timestamp]);
            
        let metadata_map: HashMap<String, serde_json::Value> = metadata
            .and_then(|m| {
                m.get("metadata_map")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
            })
            .unwrap_or_default();

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
            values,
            timestamps,
            metadata_map,
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
