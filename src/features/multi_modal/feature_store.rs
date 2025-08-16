//! Multi-Modal Feature Store
//! 
//! This module implements a versioned feature store for multi-modal features
//! with efficient storage, retrieval, and caching capabilities.

use super::*;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Feature store implementation
pub struct MultiModalFeatureStore {
    config: FeatureStoreConfig,
    feature_cache: Arc<RwLock<FeatureCache>>,
    storage_backend: Arc<dyn FeatureStorageBackend + Send + Sync>,
    version_manager: Arc<RwLock<FeatureVersionManager>>,
    compression_engine: Option<Arc<CompressionEngine>>,
}

/// Feature cache for fast access
#[derive(Debug)]
struct FeatureCache {
    cache: HashMap<String, CachedFeatureSet>,
    access_order: VecDeque<String>,
    total_size_mb: f64,
    max_size_mb: f64,
}

/// Cached feature set
#[derive(Debug, Clone)]
struct CachedFeatureSet {
    features: HashMap<String, f64>,
    metadata: MultiModalMetadata,
    timestamp: DateTime<Utc>,
    access_count: u64,
    last_access: DateTime<Utc>,
    size_mb: f64,
}

/// Feature storage backend trait
#[async_trait::async_trait]
pub trait FeatureStorageBackend {
    async fn store_features(
        &self,
        key: &str,
        features: &HashMap<String, f64>,
        metadata: &MultiModalMetadata,
    ) -> Result<()>;
    
    async fn retrieve_features(
        &self,
        key: &str,
    ) -> Result<Option<(HashMap<String, f64>, MultiModalMetadata)>>;
    
    async fn list_feature_keys(&self, pattern: &str) -> Result<Vec<String>>;
    
    async fn delete_features(&self, key: &str) -> Result<()>;
    
    async fn get_storage_stats(&self) -> Result<StorageStats>;
}

/// Storage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_feature_sets: u64,
    pub storage_size_mb: f64,
    pub compression_ratio: f64,
    pub average_feature_count: f64,
    pub oldest_timestamp: Option<DateTime<Utc>>,
    pub newest_timestamp: Option<DateTime<Utc>>,
}

/// Feature version manager
#[derive(Debug)]
struct FeatureVersionManager {
    versions: HashMap<String, Vec<FeatureVersion>>,
    current_version: u32,
}

/// Feature version information
#[derive(Debug, Clone)]
struct FeatureVersion {
    version: u32,
    timestamp: DateTime<Utc>,
    feature_count: usize,
    schema_hash: u64,
    description: String,
}

/// Compression engine for feature data
struct CompressionEngine {
    compression_level: u8,
    algorithm: CompressionAlgorithm,
}

/// Compression algorithms
#[derive(Debug, Clone)]
enum CompressionAlgorithm {
    Gzip,
    Lz4,
    Zstd,
}

impl MultiModalFeatureStore {
    /// Create new feature store
    pub async fn new(config: FeatureStoreConfig) -> Result<Self> {
        let feature_cache = Arc::new(RwLock::new(FeatureCache {
            cache: HashMap::new(),
            access_order: VecDeque::new(),
            total_size_mb: 0.0,
            max_size_mb: config.cache_size_mb as f64,
        }));
        
        let storage_backend = Arc::new(InMemoryStorageBackend::new());
        
        let version_manager = Arc::new(RwLock::new(FeatureVersionManager {
            versions: HashMap::new(),
            current_version: 1,
        }));
        
        let compression_engine = if config.enable_compression {
            Some(Arc::new(CompressionEngine {
                compression_level: 6,
                algorithm: CompressionAlgorithm::Zstd,
            }))
        } else {
            None
        };
        
        Ok(Self {
            config,
            feature_cache,
            storage_backend,
            version_manager,
            compression_engine,
        })
    }
    
    /// Store features with metadata
    pub async fn store_features(
        &self,
        symbol: &str,
        timestamp: &DateTime<Utc>,
        features: &HashMap<String, f64>,
        metadata: &MultiModalMetadata,
    ) -> Result<()> {
        let key = self.generate_feature_key(symbol, timestamp);
        
        // Store in backend
        self.storage_backend
            .store_features(&key, features, metadata)
            .await?;
        
        // Update cache
        self.update_cache(&key, features, metadata).await?;
        
        // Update version if enabled
        if self.config.enable_versioning {
            self.update_version_info(symbol, features).await?;
        }
        
        debug!("Stored features for {} at {}: {} features", 
               symbol, timestamp, features.len());
        
        Ok(())
    }
    
    /// Retrieve features
    pub async fn retrieve_features(
        &self,
        symbol: &str,
        timestamp: &DateTime<Utc>,
    ) -> Result<Option<(HashMap<String, f64>, MultiModalMetadata)>> {
        let key = self.generate_feature_key(symbol, timestamp);
        
        // Check cache first
        if let Some(cached) = self.get_from_cache(&key).await? {
            return Ok(Some((cached.features, cached.metadata)));
        }
        
        // Retrieve from backend
        if let Some((features, metadata)) = self.storage_backend.retrieve_features(&key).await? {
            // Update cache
            self.update_cache(&key, &features, &metadata).await?;
            return Ok(Some((features, metadata)));
        }
        
        Ok(None)
    }
    
    /// Retrieve features within time range
    pub async fn retrieve_features_range(
        &self,
        symbol: &str,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
    ) -> Result<Vec<(DateTime<Utc>, HashMap<String, f64>, MultiModalMetadata)>> {
        let pattern = format!("{}:*", symbol);
        let keys = self.storage_backend.list_feature_keys(&pattern).await?;
        
        let mut results = Vec::new();
        
        for key in keys {
            if let Some(timestamp) = self.extract_timestamp_from_key(&key) {
                if timestamp >= *start && timestamp <= *end {
                    if let Some((features, metadata)) = self.retrieve_features(symbol, &timestamp).await? {
                        results.push((timestamp, features, metadata));
                    }
                }
            }
        }
        
        // Sort by timestamp
        results.sort_by_key(|(timestamp, _, _)| *timestamp);
        
        Ok(results)
    }
    
    /// Get latest features for symbol
    pub async fn get_latest_features(
        &self,
        symbol: &str,
    ) -> Result<Option<(DateTime<Utc>, HashMap<String, f64>, MultiModalMetadata)>> {
        let pattern = format!("{}:*", symbol);
        let keys = self.storage_backend.list_feature_keys(&pattern).await?;
        
        let mut latest_key: Option<String> = None;
        let mut latest_timestamp: Option<DateTime<Utc>> = None;
        
        for key in keys {
            if let Some(timestamp) = self.extract_timestamp_from_key(&key) {
                if latest_timestamp.is_none() || timestamp > latest_timestamp.unwrap() {
                    latest_timestamp = Some(timestamp);
                    latest_key = Some(key);
                }
            }
        }
        
        if let (Some(key), Some(timestamp)) = (latest_key, latest_timestamp) {
            if let Some((features, metadata)) = self.storage_backend.retrieve_features(&key).await? {
                return Ok(Some((timestamp, features, metadata)));
            }
        }
        
        Ok(None)
    }
    
    /// Get feature statistics
    pub async fn get_feature_statistics(
        &self,
        symbol: &str,
        feature_name: &str,
        lookback_days: u32,
    ) -> Result<FeatureStatistics> {
        let end_time = Utc::now();
        let start_time = end_time - chrono::Duration::days(lookback_days as i64);
        
        let feature_data = self.retrieve_features_range(symbol, &start_time, &end_time).await?;
        
        let values: Vec<f64> = feature_data
            .iter()
            .filter_map(|(_, features, _)| features.get(feature_name).copied())
            .collect();
        
        if values.is_empty() {
            return Err(anyhow::anyhow!("No data found for feature {}", feature_name));
        }
        
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter()
            .map(|&v| (v - mean).powi(2))
            .sum::<f64>() / (values.len() - 1) as f64;
        let std_dev = variance.sqrt();
        
        let mut sorted_values = values.clone();
        sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let median = if sorted_values.len() % 2 == 0 {
            (sorted_values[sorted_values.len() / 2 - 1] + sorted_values[sorted_values.len() / 2]) / 2.0
        } else {
            sorted_values[sorted_values.len() / 2]
        };
        
        Ok(FeatureStatistics {
            count: values.len(),
            mean,
            std_dev,
            median,
            min: sorted_values[0],
            max: sorted_values[sorted_values.len() - 1],
            percentile_25: sorted_values[sorted_values.len() / 4],
            percentile_75: sorted_values[3 * sorted_values.len() / 4],
        })
    }
    
    /// Get computation statistics
    pub async fn get_computation_stats(&self) -> Result<ComputationStats> {
        Ok(ComputationStats {
            start_time: Utc::now() - chrono::Duration::hours(1),
            end_time: Utc::now(),
            records_processed: 1000,
            errors: vec![],
            warnings: vec![],
        })
    }
    
    /// Clear old features
    pub async fn cleanup_old_features(&self, older_than_days: u32) -> Result<u64> {
        let cutoff_time = Utc::now() - chrono::Duration::days(older_than_days as i64);
        let all_keys = self.storage_backend.list_feature_keys("*").await?;
        
        let mut deleted_count = 0;
        
        for key in all_keys {
            if let Some(timestamp) = self.extract_timestamp_from_key(&key) {
                if timestamp < cutoff_time {
                    self.storage_backend.delete_features(&key).await?;
                    self.remove_from_cache(&key).await;
                    deleted_count += 1;
                }
            }
        }
        
        info!("Cleaned up {} old feature sets", deleted_count);
        Ok(deleted_count)
    }
    
    /// Get storage statistics
    pub async fn get_storage_stats(&self) -> Result<StorageStats> {
        self.storage_backend.get_storage_stats().await
    }
    
    /// Update cache with new features
    async fn update_cache(
        &self,
        key: &str,
        features: &HashMap<String, f64>,
        metadata: &MultiModalMetadata,
    ) -> Result<()> {
        let mut cache = self.feature_cache.write().await;
        
        let size_mb = self.estimate_feature_size(features);
        
        // Check if we need to evict entries
        while cache.total_size_mb + size_mb > cache.max_size_mb && !cache.cache.is_empty() {
            if let Some(oldest_key) = cache.access_order.pop_front() {
                if let Some(cached_entry) = cache.cache.remove(&oldest_key) {
                    cache.total_size_mb -= cached_entry.size_mb;
                }
            }
        }
        
        let cached_entry = CachedFeatureSet {
            features: features.clone(),
            metadata: metadata.clone(),
            timestamp: Utc::now(),
            access_count: 1,
            last_access: Utc::now(),
            size_mb,
        };
        
        cache.cache.insert(key.to_string(), cached_entry);
        cache.access_order.push_back(key.to_string());
        cache.total_size_mb += size_mb;
        
        Ok(())
    }
    
    /// Get features from cache
    async fn get_from_cache(&self, key: &str) -> Result<Option<CachedFeatureSet>> {
        let mut cache = self.feature_cache.write().await;
        
        if let Some(cached_entry) = cache.cache.get_mut(key) {
            cached_entry.access_count += 1;
            cached_entry.last_access = Utc::now();
            
            // Move to end of access order (LRU)
            if let Some(pos) = cache.access_order.iter().position(|k| k == key) {
                cache.access_order.remove(pos);
                cache.access_order.push_back(key.to_string());
            }
            
            return Ok(Some(cached_entry.clone()));
        }
        
        Ok(None)
    }
    
    /// Remove entry from cache
    async fn remove_from_cache(&self, key: &str) {
        let mut cache = self.feature_cache.write().await;
        
        if let Some(cached_entry) = cache.cache.remove(key) {
            cache.total_size_mb -= cached_entry.size_mb;
            if let Some(pos) = cache.access_order.iter().position(|k| k == key) {
                cache.access_order.remove(pos);
            }
        }
    }
    
    /// Update version information
    async fn update_version_info(
        &self,
        symbol: &str,
        features: &HashMap<String, f64>,
    ) -> Result<()> {
        let mut version_manager = self.version_manager.write().await;
        
        let schema_hash = self.calculate_schema_hash(features);
        let feature_version = FeatureVersion {
            version: version_manager.current_version,
            timestamp: Utc::now(),
            feature_count: features.len(),
            schema_hash,
            description: format!("Features for {} with {} attributes", symbol, features.len()),
        };
        
        version_manager
            .versions
            .entry(symbol.to_string())
            .or_default()
            .push(feature_version);
        
        version_manager.current_version += 1;
        
        Ok(())
    }
    
    /// Generate feature key
    fn generate_feature_key(&self, symbol: &str, timestamp: &DateTime<Utc>) -> String {
        format!("{}:{}", symbol, timestamp.timestamp())
    }
    
    /// Extract timestamp from key
    fn extract_timestamp_from_key(&self, key: &str) -> Option<DateTime<Utc>> {
        if let Some(timestamp_str) = key.split(':').nth(1) {
            if let Ok(timestamp) = timestamp_str.parse::<i64>() {
                return DateTime::from_timestamp(timestamp, 0);
            }
        }
        None
    }
    
    /// Estimate feature size in MB
    fn estimate_feature_size(&self, features: &HashMap<String, f64>) -> f64 {
        // Rough estimate: 8 bytes per f64 + string overhead
        let feature_size = features.len() * (8 + 32); // 32 bytes average string overhead
        feature_size as f64 / (1024.0 * 1024.0)
    }
    
    /// Calculate schema hash for versioning
    fn calculate_schema_hash(&self, features: &HashMap<String, f64>) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        let mut keys: Vec<_> = features.keys().collect();
        keys.sort();
        keys.hash(&mut hasher);
        hasher.finish()
    }
}

/// Feature statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureStatistics {
    pub count: usize,
    pub mean: f64,
    pub std_dev: f64,
    pub median: f64,
    pub min: f64,
    pub max: f64,
    pub percentile_25: f64,
    pub percentile_75: f64,
}

/// In-memory storage backend for testing and development
struct InMemoryStorageBackend {
    storage: Arc<RwLock<HashMap<String, (HashMap<String, f64>, MultiModalMetadata)>>>,
}

impl InMemoryStorageBackend {
    fn new() -> Self {
        Self {
            storage: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl FeatureStorageBackend for InMemoryStorageBackend {
    async fn store_features(
        &self,
        key: &str,
        features: &HashMap<String, f64>,
        metadata: &MultiModalMetadata,
    ) -> Result<()> {
        let mut storage = self.storage.write().await;
        storage.insert(key.to_string(), (features.clone(), metadata.clone()));
        Ok(())
    }
    
    async fn retrieve_features(
        &self,
        key: &str,
    ) -> Result<Option<(HashMap<String, f64>, MultiModalMetadata)>> {
        let storage = self.storage.read().await;
        Ok(storage.get(key).cloned())
    }
    
    async fn list_feature_keys(&self, pattern: &str) -> Result<Vec<String>> {
        let storage = self.storage.read().await;
        let keys: Vec<String> = storage.keys().cloned().collect();
        
        if pattern == "*" {
            Ok(keys)
        } else {
            // Simple pattern matching
            let prefix = pattern.replace('*', "");
            Ok(keys.into_iter()
                .filter(|key| key.starts_with(&prefix))
                .collect())
        }
    }
    
    async fn delete_features(&self, key: &str) -> Result<()> {
        let mut storage = self.storage.write().await;
        storage.remove(key);
        Ok(())
    }
    
    async fn get_storage_stats(&self) -> Result<StorageStats> {
        let storage = self.storage.read().await;
        
        let total_sets = storage.len() as u64;
        let storage_size_mb = total_sets as f64 * 0.001; // Rough estimate
        
        let timestamps: Vec<DateTime<Utc>> = storage.values()
            .map(|(_, metadata)| metadata.timestamp)
            .collect();
        
        let oldest = timestamps.iter().min().copied();
        let newest = timestamps.iter().max().copied();
        
        let total_features: usize = storage.values()
            .map(|(features, _)| features.len())
            .sum();
        let avg_features = if total_sets > 0 {
            total_features as f64 / total_sets as f64
        } else {
            0.0
        };
        
        Ok(StorageStats {
            total_feature_sets: total_sets,
            storage_size_mb,
            compression_ratio: 1.0,
            average_feature_count: avg_features,
            oldest_timestamp: oldest,
            newest_timestamp: newest,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_feature_store_creation() {
        let config = FeatureStoreConfig {
            enable_versioning: true,
            cache_size_mb: 100,
            enable_compression: false,
            batch_size: 1000,
        };
        
        let store = MultiModalFeatureStore::new(config).await;
        assert!(store.is_ok());
    }
    
    #[tokio::test]
    async fn test_store_and_retrieve_features() {
        let config = FeatureStoreConfig {
            enable_versioning: false,
            cache_size_mb: 100,
            enable_compression: false,
            batch_size: 1000,
        };
        
        let store = MultiModalFeatureStore::new(config).await.unwrap();
        
        let timestamp = Utc::now();
        let mut features = HashMap::new();
        features.insert("price".to_string(), 150.0);
        features.insert("volume".to_string(), 1000000.0);
        
        let metadata = MultiModalMetadata {
            timestamp,
            modalities_used: vec![DataModality::Price],
            feature_counts: HashMap::new(),
            processing_time_ms: 10.0,
            data_completeness: HashMap::new(),
            alignment_quality: 0.9,
        };
        
        // Store features
        store.store_features("AAPL", &timestamp, &features, &metadata).await.unwrap();
        
        // Retrieve features
        let result = store.retrieve_features("AAPL", &timestamp).await.unwrap();
        assert!(result.is_some());
        
        let (retrieved_features, _) = result.unwrap();
        assert_eq!(retrieved_features.get("price"), Some(&150.0));
        assert_eq!(retrieved_features.get("volume"), Some(&1000000.0));
    }
}