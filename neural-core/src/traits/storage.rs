//! Storage abstraction traits
//! Module size: <150 lines as per requirements

use crate::errors::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub backend: StorageBackend,
    pub connection_string: String,
    pub max_connections: Option<u32>,
    pub timeout_seconds: Option<u32>,
    pub enable_compression: bool,
    pub enable_encryption: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: StorageBackend::Memory,
            connection_string: "memory://".to_string(),
            max_connections: Some(10),
            timeout_seconds: Some(30),
            enable_compression: false,
            enable_encryption: false,
        }
    }
}

/// Supported storage backends
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StorageBackend {
    Memory,
    Redis,
    PostgreSQL,
    TimescaleDB,
    InfluxDB,
    S3,
}

/// Generic storage trait for data persistence
#[async_trait]
pub trait Storage: Send + Sync {
    /// Store a key-value pair
    async fn set(&self, key: &str, value: &[u8]) -> Result<()>;
    
    /// Retrieve value by key
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    
    /// Delete a key
    async fn delete(&self, key: &str) -> Result<bool>;
    
    /// Check if key exists
    async fn exists(&self, key: &str) -> Result<bool>;
    
    /// Set key with expiration
    async fn set_with_ttl(&self, key: &str, value: &[u8], ttl_seconds: u64) -> Result<()>;
    
    /// Get multiple keys at once
    async fn get_many(&self, keys: &[&str]) -> Result<HashMap<String, Vec<u8>>>;
    
    /// Set multiple key-value pairs
    async fn set_many(&self, items: &HashMap<String, Vec<u8>>) -> Result<()>;
    
    /// List keys matching a pattern
    async fn list_keys(&self, pattern: &str) -> Result<Vec<String>>;
    
    /// Get storage backend type
    fn backend_type(&self) -> StorageBackend;
    
    /// Health check
    async fn health_check(&self) -> Result<StorageHealth>;
    
    /// Get storage statistics
    async fn stats(&self) -> Result<StorageStats>;
}

/// Time series storage trait for market data
#[async_trait]
pub trait TimeSeriesStorage: Storage {
    /// Store time series data point
    async fn store_point(&self, series: &str, timestamp: DateTime<Utc>, value: f64) -> Result<()>;
    
    /// Store multiple data points
    async fn store_points(&self, series: &str, points: &[(DateTime<Utc>, f64)]) -> Result<()>;
    
    /// Query time series data in range
    async fn query_range(
        &self,
        series: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<(DateTime<Utc>, f64)>>;
    
    /// Get latest value
    async fn get_latest(&self, series: &str) -> Result<Option<(DateTime<Utc>, f64)>>;
    
    /// Aggregate data (avg, sum, min, max, count)
    async fn aggregate(
        &self,
        series: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        aggregation: AggregationType,
        interval: String,
    ) -> Result<Vec<(DateTime<Utc>, f64)>>;
}

/// Aggregation types for time series
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregationType {
    Avg,
    Sum,
    Min,
    Max,
    Count,
    First,
    Last,
}

/// Storage health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageHealth {
    pub is_healthy: bool,
    pub response_time_ms: u64,
    pub error_message: Option<String>,
    pub last_check: DateTime<Utc>,
}

/// Storage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_keys: u64,
    pub memory_usage_bytes: u64,
    pub operations_per_second: f64,
    pub hit_ratio: f64,
    pub error_rate: f64,
    pub uptime_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_storage_config_default() {
        let config = StorageConfig::default();
        assert_eq!(config.backend, StorageBackend::Memory);
        assert_eq!(config.connection_string, "memory://");
        assert!(!config.enable_compression);
        assert!(!config.enable_encryption);
    }
    
    #[test]
    fn test_storage_backend_serialization() {
        let backend = StorageBackend::Redis;
        let serialized = serde_json::to_string(&backend).unwrap();
        let deserialized: StorageBackend = serde_json::from_str(&serialized).unwrap();
        assert_eq!(backend, deserialized);
    }
}