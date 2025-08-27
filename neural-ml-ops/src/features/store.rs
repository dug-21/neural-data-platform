//! Feature Store Implementation
//!
//! Extracted and refactored from trading-specific feature store to be domain agnostic.
//! Provides persistent storage, caching, and retrieval of features.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};

use super::{
    CompressionAlgorithm, CompressionConfig, Feature, FeatureStoreConfig, FeatureStoreTrait,
    StorageBackend, StorageStats,
};

/// Main feature store implementation
pub struct FeatureStore {
    config: FeatureStoreConfig,
    storage_backend: Box<dyn FeatureStoreTrait>,
    cache: Arc<RwLock<FeatureCache>>,
    stats: Arc<RwLock<StoreStatistics>>,
}

/// In-memory feature cache
#[derive(Debug)]
struct FeatureCache {
    features: HashMap<String, CachedFeatureSet>,
    cache_size_bytes: usize,
    max_size_bytes: usize,
    hit_count: u64,
    miss_count: u64,
}

/// Cached feature set with metadata
#[derive(Debug, Clone)]
struct CachedFeatureSet {
    features: Vec<Feature>,
    cached_at: DateTime<Utc>,
    access_count: u32,
    size_bytes: usize,
}

/// Store operation statistics
#[derive(Debug, Default)]
struct StoreStatistics {
    total_features_stored: u64,
    total_features_retrieved: u64,
    total_storage_operations: u64,
    total_cache_hits: u64,
    total_cache_misses: u64,
    last_cleanup_time: Option<DateTime<Utc>>,
    storage_errors: u64,
}

/// Feature versioning information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureVersion {
    pub version_id: String,
    pub created_at: DateTime<Utc>,
    pub description: Option<String>,
    pub feature_count: usize,
    pub checksum: String,
}

impl FeatureStore {
    /// Create a new feature store with the given configuration
    pub async fn new(config: FeatureStoreConfig) -> Result<Self> {
        info!("Initializing Feature Store with backend: {:?}", config.storage_backend);
        
        // Create storage backend
        let storage_backend = Self::create_storage_backend(&config).await?;
        
        // Initialize cache
        let max_cache_size = config.cache_size_mb * 1024 * 1024; // Convert MB to bytes
        let cache = Arc::new(RwLock::new(FeatureCache {
            features: HashMap::new(),
            cache_size_bytes: 0,
            max_size_bytes: max_cache_size,
            hit_count: 0,
            miss_count: 0,
        }));
        
        let stats = Arc::new(RwLock::new(StoreStatistics::default()));
        
        let store = Self {
            config,
            storage_backend,
            cache,
            stats,
        };
        
        // Start background cleanup task
        store.start_cleanup_task().await;
        
        info!("Feature Store initialized successfully");
        Ok(store)
    }
    
    /// Store features with optional versioning
    pub async fn store_features(
        &self,
        namespace: &str,
        features: &[Feature],
        version: Option<&str>,
    ) -> Result<()> {
        info!("Storing {} features in namespace: {}", features.len(), namespace);
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_features_stored += features.len() as u64;
            stats.total_storage_operations += 1;
        }
        
        // Apply compression if enabled
        let processed_features = if self.config.compression.enabled {
            self.compress_features(features).await?
        } else {
            features.to_vec()
        };
        
        // Store in backend
        match self.storage_backend.store_features(namespace, &processed_features, version).await {
            Ok(_) => {
                // Update cache
                self.update_cache(namespace, &processed_features).await;
                info!("Successfully stored {} features", features.len());
                Ok(())
            }
            Err(e) => {
                error!("Failed to store features: {}", e);
                self.stats.write().await.storage_errors += 1;
                Err(e)
            }
        }
    }
    
    /// Retrieve features by name and optional time range
    pub async fn retrieve_features(
        &self,
        namespace: &str,
        feature_names: &[String],
        time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
        version: Option<&str>,
    ) -> Result<Vec<Feature>> {
        debug!("Retrieving {} features from namespace: {}", feature_names.len(), namespace);
        
        // Check cache first
        let cache_key = self.generate_cache_key(namespace, feature_names, &time_range, version);
        if let Some(cached_features) = self.get_from_cache(&cache_key).await {
            self.stats.write().await.total_cache_hits += 1;
            return Ok(cached_features);
        }
        
        // Cache miss - retrieve from backend
        self.stats.write().await.total_cache_misses += 1;
        
        match self.storage_backend.retrieve_features(namespace, feature_names, time_range, version).await {
            Ok(features) => {
                // Apply decompression if needed
                let processed_features = if self.config.compression.enabled {
                    self.decompress_features(&features).await?
                } else {
                    features
                };
                
                // Apply time range filtering if specified
                let filtered_features = if let Some((start_time, end_time)) = time_range {
                    processed_features.into_iter()
                        .filter(|f| f.timestamp >= start_time && f.timestamp <= end_time)
                        .collect()
                } else {
                    processed_features
                };
                
                // Update cache
                self.cache_features(&cache_key, &filtered_features).await;
                
                // Update statistics
                {
                    let mut stats = self.stats.write().await;
                    stats.total_features_retrieved += filtered_features.len() as u64;
                    stats.total_storage_operations += 1;
                }
                
                info!("Retrieved {} features from storage", filtered_features.len());
                Ok(filtered_features)
            }
            Err(e) => {
                error!("Failed to retrieve features: {}", e);
                self.stats.write().await.storage_errors += 1;
                Err(e)
            }
        }
    }
    
    /// Extract features using the internal feature request system
    pub async fn extract_features(
        &self,
        request: super::FeatureRequest,
    ) -> Result<Vec<Feature>> {
        info!("Processing feature extraction request for: {}", request.data_source);
        
        // This would integrate with the feature engineering module
        // For now, we'll return a placeholder response
        let timestamp = Utc::now();
        let sample_features = vec![
            Feature {
                name: "sample_feature_1".to_string(),
                value: 42.0,
                feature_type: super::FeatureType::Numerical,
                timestamp,
                metadata: Some([("source".to_string(), request.data_source.clone())].into()),
            },
            Feature {
                name: "sample_feature_2".to_string(),
                value: 24.0,
                feature_type: super::FeatureType::Numerical,
                timestamp,
                metadata: Some([("source".to_string(), request.data_source)].into()),
            },
        ];
        
        Ok(sample_features)
    }
    
    /// List available features in a namespace
    pub async fn list_features(&self, namespace: &str) -> Result<Vec<String>> {
        self.storage_backend.list_features(namespace).await
    }
    
    /// Get feature versions for a namespace
    pub async fn get_feature_versions(&self, namespace: &str) -> Result<Vec<FeatureVersion>> {
        // This would be implemented by backends that support versioning
        // For now, return a placeholder
        Ok(vec![FeatureVersion {
            version_id: "v1.0.0".to_string(),
            created_at: Utc::now(),
            description: Some("Initial version".to_string()),
            feature_count: 0,
            checksum: "placeholder".to_string(),
        }])
    }
    
    /// Create a new feature version
    pub async fn create_version(
        &self,
        namespace: &str,
        version_id: &str,
        description: Option<&str>,
    ) -> Result<FeatureVersion> {
        info!("Creating feature version: {} for namespace: {}", version_id, namespace);
        
        let version = FeatureVersion {
            version_id: version_id.to_string(),
            created_at: Utc::now(),
            description: description.map(|s| s.to_string()),
            feature_count: 0, // Would be calculated from actual features
            checksum: self.calculate_namespace_checksum(namespace).await?,
        };
        
        // Store version metadata (implementation depends on backend)
        Ok(version)
    }
    
    /// Clean up old features beyond retention period
    pub async fn cleanup_old_features(&self) -> Result<usize> {
        let cutoff_time = Utc::now() - Duration::days(self.config.retention_days as i64);
        info!("Cleaning up features older than: {}", cutoff_time);
        
        let cleaned_count = self.storage_backend.cleanup_old_features(cutoff_time).await?;
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.last_cleanup_time = Some(Utc::now());
        }
        
        // Clear cache of old entries
        self.clear_old_cache_entries().await;
        
        info!("Cleaned up {} old features", cleaned_count);
        Ok(cleaned_count)
    }
    
    /// Get comprehensive storage statistics
    pub async fn get_statistics(&self) -> Result<StorageStats> {
        let backend_stats = self.storage_backend.get_stats().await?;
        let cache = self.cache.read().await;
        let store_stats = self.stats.read().await;
        
        Ok(StorageStats {
            total_features: backend_stats.total_features,
            total_namespaces: backend_stats.total_namespaces,
            storage_size_mb: backend_stats.storage_size_mb,
            oldest_feature: backend_stats.oldest_feature,
            newest_feature: backend_stats.newest_feature,
            cache_hit_ratio: if (cache.hit_count + cache.miss_count) > 0 {
                cache.hit_count as f64 / (cache.hit_count + cache.miss_count) as f64
            } else {
                0.0
            },
        })
    }
    
    /// Get detailed store metrics
    pub async fn get_store_metrics(&self) -> StoreMetrics {
        let stats = self.stats.read().await;
        let cache = self.cache.read().await;
        
        StoreMetrics {
            features_stored: stats.total_features_stored,
            features_retrieved: stats.total_features_retrieved,
            storage_operations: stats.total_storage_operations,
            cache_hits: stats.total_cache_hits,
            cache_misses: stats.total_cache_misses,
            cache_hit_ratio: if (cache.hit_count + cache.miss_count) > 0 {
                cache.hit_count as f64 / (cache.hit_count + cache.miss_count) as f64
            } else {
                0.0
            },
            cache_size_mb: cache.cache_size_bytes as f64 / (1024.0 * 1024.0),
            storage_errors: stats.storage_errors,
            last_cleanup: stats.last_cleanup_time,
        }
    }
    
    // Private methods
    
    async fn create_storage_backend(config: &FeatureStoreConfig) -> Result<Box<dyn FeatureStoreTrait>> {
        match &config.storage_backend {
            StorageBackend::Memory => {
                Ok(Box::new(MemoryFeatureStore::new()))
            }
            StorageBackend::Redis { connection_string } => {
                #[cfg(feature = "events")]
                {
                    Ok(Box::new(RedisFeatureStore::new(connection_string).await?))
                }
                #[cfg(not(feature = "events"))]
                {
                    warn!("Redis backend requested but redis feature not enabled, falling back to memory");
                    Ok(Box::new(MemoryFeatureStore::new()))
                }
            }
            StorageBackend::Database { connection_string: _ } => {
                warn!("Database backend not yet implemented, falling back to memory");
                Ok(Box::new(MemoryFeatureStore::new()))
            }
            StorageBackend::FileSystem { base_path } => {
                Ok(Box::new(FileSystemFeatureStore::new(base_path)?))
            }
        }
    }
    
    async fn compress_features(&self, features: &[Feature]) -> Result<Vec<Feature>> {
        if !self.config.compression.enabled {
            return Ok(features.to_vec());
        }
        
        // For now, just return the features as-is
        // In a real implementation, this would compress feature data
        debug!("Feature compression is enabled but not yet implemented");
        Ok(features.to_vec())
    }
    
    async fn decompress_features(&self, features: &[Feature]) -> Result<Vec<Feature>> {
        if !self.config.compression.enabled {
            return Ok(features.to_vec());
        }
        
        // For now, just return the features as-is
        // In a real implementation, this would decompress feature data
        debug!("Feature decompression is enabled but not yet implemented");
        Ok(features.to_vec())
    }
    
    fn generate_cache_key(
        &self,
        namespace: &str,
        feature_names: &[String],
        time_range: &Option<(DateTime<Utc>, DateTime<Utc>)>,
        version: Option<&str>,
    ) -> String {
        let mut key = format!("{}:{}", namespace, feature_names.join(","));
        
        if let Some((start, end)) = time_range {
            key.push_str(&format!(":{}:{}", start.timestamp(), end.timestamp()));
        }
        
        if let Some(v) = version {
            key.push_str(&format!(":v{}", v));
        }
        
        key
    }
    
    async fn get_from_cache(&self, cache_key: &str) -> Option<Vec<Feature>> {
        let mut cache = self.cache.write().await;
        
        if cache.features.contains_key(cache_key) {
            let features = cache.features.get_mut(cache_key).unwrap().features.clone();
            cache.features.get_mut(cache_key).unwrap().access_count += 1;
            cache.hit_count += 1;
            Some(features)
        } else {
            cache.miss_count += 1;
            None
        }
    }
    
    async fn cache_features(&self, cache_key: &str, features: &[Feature]) {
        let mut cache = self.cache.write().await;
        
        let feature_size = self.estimate_features_size(features);
        
        // Make room if necessary
        while cache.cache_size_bytes + feature_size > cache.max_size_bytes && !cache.features.is_empty() {
            self.evict_oldest_cache_entry(&mut cache);
        }
        
        // Add to cache
        let cached_set = CachedFeatureSet {
            features: features.to_vec(),
            cached_at: Utc::now(),
            access_count: 1,
            size_bytes: feature_size,
        };
        
        cache.features.insert(cache_key.to_string(), cached_set);
        cache.cache_size_bytes += feature_size;
    }
    
    async fn update_cache(&self, namespace: &str, features: &[Feature]) {
        // Update cache with newly stored features
        let cache_key = format!("{}:latest", namespace);
        self.cache_features(&cache_key, features).await;
    }
    
    fn evict_oldest_cache_entry(&self, cache: &mut FeatureCache) {
        if let Some((oldest_key, _)) = cache.features.iter()
            .min_by_key(|(_, cached_set)| cached_set.cached_at) {
            
            let oldest_key = oldest_key.clone();
            if let Some(removed_set) = cache.features.remove(&oldest_key) {
                cache.cache_size_bytes -= removed_set.size_bytes;
                debug!("Evicted cache entry: {} (size: {} bytes)", oldest_key, removed_set.size_bytes);
            }
        }
    }
    
    fn estimate_features_size(&self, features: &[Feature]) -> usize {
        // Rough estimate: each feature ~200 bytes including metadata
        features.len() * 200
    }
    
    async fn clear_old_cache_entries(&self) {
        let cutoff = Utc::now() - Duration::hours(1);
        let mut cache = self.cache.write().await;
        
        let old_keys: Vec<String> = cache.features.iter()
            .filter(|(_, cached_set)| cached_set.cached_at < cutoff)
            .map(|(key, _)| key.clone())
            .collect();
        
        for key in old_keys {
            if let Some(removed_set) = cache.features.remove(&key) {
                cache.cache_size_bytes -= removed_set.size_bytes;
            }
        }
    }
    
    async fn calculate_namespace_checksum(&self, _namespace: &str) -> Result<String> {
        // Placeholder implementation
        // In practice, would calculate checksum based on all features in namespace
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(_namespace.as_bytes());
        Ok(format!("{:x}", hasher.finalize()))
    }
    
    async fn start_cleanup_task(&self) {
        let config = self.config.clone();
        let store_weak = Arc::downgrade(&Arc::new(self.stats.clone()));
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600)); // 1 hour
            
            loop {
                interval.tick().await;
                
                if let Some(_stats) = store_weak.upgrade() {
                    // Cleanup would be performed here
                    debug!("Background cleanup task running");
                } else {
                    break; // Store has been dropped
                }
            }
        });
    }
}

/// Store performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreMetrics {
    pub features_stored: u64,
    pub features_retrieved: u64,
    pub storage_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_hit_ratio: f64,
    pub cache_size_mb: f64,
    pub storage_errors: u64,
    pub last_cleanup: Option<DateTime<Utc>>,
}

// Storage backend implementations

/// In-memory storage backend for development and testing
struct MemoryFeatureStore {
    features: Arc<RwLock<HashMap<String, Vec<Feature>>>>,
    stats: Arc<RwLock<MemoryStats>>,
}

#[derive(Debug, Default)]
struct MemoryStats {
    total_features: usize,
    total_namespaces: usize,
}

impl MemoryFeatureStore {
    fn new() -> Self {
        Self {
            features: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(MemoryStats::default())),
        }
    }
}

#[async_trait::async_trait]
impl FeatureStoreTrait for MemoryFeatureStore {
    async fn store_features(
        &self,
        namespace: &str,
        features: &[Feature],
        _version: Option<&str>,
    ) -> Result<()> {
        let mut store = self.features.write().await;
        let mut stats = self.stats.write().await;
        
        let namespace_features = store.entry(namespace.to_string()).or_insert_with(Vec::new);
        namespace_features.extend_from_slice(features);
        
        stats.total_features += features.len();
        if namespace_features.len() == features.len() {
            stats.total_namespaces += 1; // New namespace
        }
        
        Ok(())
    }
    
    async fn retrieve_features(
        &self,
        namespace: &str,
        feature_names: &[String],
        time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
        _version: Option<&str>,
    ) -> Result<Vec<Feature>> {
        let store = self.features.read().await;
        
        if let Some(namespace_features) = store.get(namespace) {
            let mut filtered_features: Vec<Feature> = namespace_features.iter()
                .filter(|f| feature_names.is_empty() || feature_names.contains(&f.name))
                .cloned()
                .collect();
            
            if let Some((start_time, end_time)) = time_range {
                filtered_features.retain(|f| f.timestamp >= start_time && f.timestamp <= end_time);
            }
            
            Ok(filtered_features)
        } else {
            Ok(Vec::new())
        }
    }
    
    async fn list_features(&self, namespace: &str) -> Result<Vec<String>> {
        let store = self.features.read().await;
        
        if let Some(namespace_features) = store.get(namespace) {
            let feature_names: std::collections::HashSet<String> = namespace_features
                .iter()
                .map(|f| f.name.clone())
                .collect();
            Ok(feature_names.into_iter().collect())
        } else {
            Ok(Vec::new())
        }
    }
    
    async fn cleanup_old_features(&self, older_than: DateTime<Utc>) -> Result<usize> {
        let mut store = self.features.write().await;
        let mut cleaned_count = 0;
        
        for namespace_features in store.values_mut() {
            let original_len = namespace_features.len();
            namespace_features.retain(|f| f.timestamp > older_than);
            cleaned_count += original_len - namespace_features.len();
        }
        
        // Remove empty namespaces
        store.retain(|_, features| !features.is_empty());
        
        Ok(cleaned_count)
    }
    
    async fn get_stats(&self) -> Result<StorageStats> {
        let store = self.features.read().await;
        let stats = self.stats.read().await;
        
        let mut oldest_timestamp = None;
        let mut newest_timestamp = None;
        let mut total_size = 0.0;
        
        for features in store.values() {
            total_size += features.len() as f64 * 0.2; // Rough estimate: 0.2KB per feature
            
            for feature in features {
                if oldest_timestamp.is_none() || Some(feature.timestamp) < oldest_timestamp {
                    oldest_timestamp = Some(feature.timestamp);
                }
                if newest_timestamp.is_none() || Some(feature.timestamp) > newest_timestamp {
                    newest_timestamp = Some(feature.timestamp);
                }
            }
        }
        
        Ok(StorageStats {
            total_features: stats.total_features,
            total_namespaces: store.len(),
            storage_size_mb: total_size / 1024.0,
            oldest_feature: oldest_timestamp,
            newest_feature: newest_timestamp,
            cache_hit_ratio: 0.0, // Not applicable for this backend
        })
    }
}

/// File system storage backend
struct FileSystemFeatureStore {
    base_path: std::path::PathBuf,
}

impl FileSystemFeatureStore {
    fn new(base_path: &str) -> Result<Self> {
        let path = std::path::PathBuf::from(base_path);
        std::fs::create_dir_all(&path)?;
        
        Ok(Self {
            base_path: path,
        })
    }
}

#[async_trait::async_trait]
impl FeatureStoreTrait for FileSystemFeatureStore {
    async fn store_features(
        &self,
        namespace: &str,
        features: &[Feature],
        version: Option<&str>,
    ) -> Result<()> {
        let namespace_dir = self.base_path.join(namespace);
        tokio::fs::create_dir_all(&namespace_dir).await?;
        
        let filename = if let Some(v) = version {
            format!("features_v{}.json", v)
        } else {
            "features.json".to_string()
        };
        
        let file_path = namespace_dir.join(filename);
        let features_json = serde_json::to_string_pretty(features)?;
        tokio::fs::write(file_path, features_json).await?;
        
        Ok(())
    }
    
    async fn retrieve_features(
        &self,
        namespace: &str,
        feature_names: &[String],
        time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
        version: Option<&str>,
    ) -> Result<Vec<Feature>> {
        let namespace_dir = self.base_path.join(namespace);
        
        let filename = if let Some(v) = version {
            format!("features_v{}.json", v)
        } else {
            "features.json".to_string()
        };
        
        let file_path = namespace_dir.join(filename);
        
        if !file_path.exists() {
            return Ok(Vec::new());
        }
        
        let features_json = tokio::fs::read_to_string(file_path).await?;
        let all_features: Vec<Feature> = serde_json::from_str(&features_json)?;
        
        let mut filtered_features: Vec<Feature> = all_features.into_iter()
            .filter(|f| feature_names.is_empty() || feature_names.contains(&f.name))
            .collect();
        
        if let Some((start_time, end_time)) = time_range {
            filtered_features.retain(|f| f.timestamp >= start_time && f.timestamp <= end_time);
        }
        
        Ok(filtered_features)
    }
    
    async fn list_features(&self, namespace: &str) -> Result<Vec<String>> {
        let namespace_dir = self.base_path.join(namespace);
        let file_path = namespace_dir.join("features.json");
        
        if !file_path.exists() {
            return Ok(Vec::new());
        }
        
        let features_json = tokio::fs::read_to_string(file_path).await?;
        let features: Vec<Feature> = serde_json::from_str(&features_json)?;
        
        let feature_names: std::collections::HashSet<String> = features
            .iter()
            .map(|f| f.name.clone())
            .collect();
        
        Ok(feature_names.into_iter().collect())
    }
    
    async fn cleanup_old_features(&self, older_than: DateTime<Utc>) -> Result<usize> {
        let mut cleaned_count = 0;
        
        // This is a simplified implementation
        // In practice, would scan all namespace directories
        let mut entries = tokio::fs::read_dir(&self.base_path).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                let _namespace = entry.file_name().to_string_lossy().to_string();
                let file_path = entry.path().join("features.json");
                
                if file_path.exists() {
                    let features_json = tokio::fs::read_to_string(&file_path).await?;
                    let mut features: Vec<Feature> = serde_json::from_str(&features_json)?;
                    
                    let original_len = features.len();
                    features.retain(|f| f.timestamp > older_than);
                    cleaned_count += original_len - features.len();
                    
                    if original_len != features.len() {
                        let updated_json = serde_json::to_string_pretty(&features)?;
                        tokio::fs::write(&file_path, updated_json).await?;
                    }
                }
            }
        }
        
        Ok(cleaned_count)
    }
    
    async fn get_stats(&self) -> Result<StorageStats> {
        let mut total_features = 0;
        let mut total_namespaces = 0;
        let mut total_size = 0.0;
        let mut oldest_timestamp = None;
        let mut newest_timestamp = None;
        
        let mut entries = tokio::fs::read_dir(&self.base_path).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                total_namespaces += 1;
                let file_path = entry.path().join("features.json");
                
                if file_path.exists() {
                    let metadata = tokio::fs::metadata(&file_path).await?;
                    total_size += metadata.len() as f64;
                    
                    let features_json = tokio::fs::read_to_string(&file_path).await?;
                    let features: Vec<Feature> = serde_json::from_str(&features_json)?;
                    total_features += features.len();
                    
                    for feature in &features {
                        if oldest_timestamp.is_none() || Some(feature.timestamp) < oldest_timestamp {
                            oldest_timestamp = Some(feature.timestamp);
                        }
                        if newest_timestamp.is_none() || Some(feature.timestamp) > newest_timestamp {
                            newest_timestamp = Some(feature.timestamp);
                        }
                    }
                }
            }
        }
        
        Ok(StorageStats {
            total_features,
            total_namespaces,
            storage_size_mb: total_size / (1024.0 * 1024.0),
            oldest_feature: oldest_timestamp,
            newest_feature: newest_timestamp,
            cache_hit_ratio: 0.0, // Not applicable for this backend
        })
    }
}

/// Redis storage backend (optional feature)
#[cfg(feature = "events")]
struct RedisFeatureStore {
    client: redis::Client,
}

#[cfg(feature = "events")]
impl RedisFeatureStore {
    async fn new(connection_string: &str) -> Result<Self> {
        let client = redis::Client::open(connection_string)?;
        
        // Test connection
        let mut conn = client.get_multiplexed_async_connection().await?;
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;
        
        Ok(Self { client })
    }
}

#[cfg(feature = "events")]
#[async_trait::async_trait]
impl FeatureStoreTrait for RedisFeatureStore {
    async fn store_features(
        &self,
        namespace: &str,
        features: &[Feature],
        version: Option<&str>,
    ) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        
        let key = if let Some(v) = version {
            format!("features:{}:v{}", namespace, v)
        } else {
            format!("features:{}", namespace)
        };
        
        let features_json = serde_json::to_string(features)?;
        let _: () = redis::cmd("SET")
            .arg(&key)
            .arg(&features_json)
            .query_async(&mut conn)
            .await?;
        
        Ok(())
    }
    
    async fn retrieve_features(
        &self,
        namespace: &str,
        feature_names: &[String],
        time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
        version: Option<&str>,
    ) -> Result<Vec<Feature>> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        
        let key = if let Some(v) = version {
            format!("features:{}:v{}", namespace, v)
        } else {
            format!("features:{}", namespace)
        };
        
        let features_json: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut conn)
            .await?;
        
        if let Some(json) = features_json {
            let all_features: Vec<Feature> = serde_json::from_str(&json)?;
            
            let mut filtered_features: Vec<Feature> = all_features.into_iter()
                .filter(|f| feature_names.is_empty() || feature_names.contains(&f.name))
                .collect();
            
            if let Some((start_time, end_time)) = time_range {
                filtered_features.retain(|f| f.timestamp >= start_time && f.timestamp <= end_time);
            }
            
            Ok(filtered_features)
        } else {
            Ok(Vec::new())
        }
    }
    
    async fn list_features(&self, namespace: &str) -> Result<Vec<String>> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        
        let key = format!("features:{}", namespace);
        let features_json: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut conn)
            .await?;
        
        if let Some(json) = features_json {
            let features: Vec<Feature> = serde_json::from_str(&json)?;
            let feature_names: std::collections::HashSet<String> = features
                .iter()
                .map(|f| f.name.clone())
                .collect();
            Ok(feature_names.into_iter().collect())
        } else {
            Ok(Vec::new())
        }
    }
    
    async fn cleanup_old_features(&self, older_than: DateTime<Utc>) -> Result<usize> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        
        // Get all feature keys
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg("features:*")
            .query_async(&mut conn)
            .await?;
        
        let mut cleaned_count = 0;
        
        for key in keys {
            let features_json: Option<String> = redis::cmd("GET")
                .arg(&key)
                .query_async(&mut conn)
                .await?;
            
            if let Some(json) = features_json {
                let mut features: Vec<Feature> = serde_json::from_str(&json)?;
                let original_len = features.len();
                features.retain(|f| f.timestamp > older_than);
                cleaned_count += original_len - features.len();
                
                if original_len != features.len() {
                    if features.is_empty() {
                        let _: () = redis::cmd("DEL")
                            .arg(&key)
                            .query_async(&mut conn)
                            .await?;
                    } else {
                        let updated_json = serde_json::to_string(&features)?;
                        let _: () = redis::cmd("SET")
                            .arg(&key)
                            .arg(&updated_json)
                            .query_async(&mut conn)
                            .await?;
                    }
                }
            }
        }
        
        Ok(cleaned_count)
    }
    
    async fn get_stats(&self) -> Result<StorageStats> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg("features:*")
            .query_async(&mut conn)
            .await?;
        
        let mut total_features = 0;
        let total_namespaces = keys.len();
        let mut total_size = 0.0;
        let mut oldest_timestamp = None;
        let mut newest_timestamp = None;
        
        for key in keys {
            let features_json: Option<String> = redis::cmd("GET")
                .arg(&key)
                .query_async(&mut conn)
                .await?;
            
            if let Some(json) = features_json {
                total_size += json.len() as f64;
                
                let features: Vec<Feature> = serde_json::from_str(&json)?;
                total_features += features.len();
                
                for feature in &features {
                    if oldest_timestamp.is_none() || Some(feature.timestamp) < oldest_timestamp {
                        oldest_timestamp = Some(feature.timestamp);
                    }
                    if newest_timestamp.is_none() || Some(feature.timestamp) > newest_timestamp {
                        newest_timestamp = Some(feature.timestamp);
                    }
                }
            }
        }
        
        Ok(StorageStats {
            total_features,
            total_namespaces,
            storage_size_mb: total_size / (1024.0 * 1024.0),
            oldest_feature: oldest_timestamp,
            newest_feature: newest_timestamp,
            cache_hit_ratio: 0.0, // Not applicable for this backend
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_memory_feature_store() {
        let config = FeatureStoreConfig::default();
        let store = FeatureStore::new(config).await.unwrap();
        
        let features = vec![
            Feature {
                name: "test_feature".to_string(),
                value: 42.0,
                feature_type: super::super::FeatureType::Numerical,
                timestamp: Utc::now(),
                metadata: None,
            },
        ];
        
        // Store features
        store.store_features("test_namespace", &features, None).await.unwrap();
        
        // Retrieve features
        let retrieved = store
            .retrieve_features("test_namespace", &[], None, None)
            .await
            .unwrap();
        
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].name, "test_feature");
        assert_eq!(retrieved[0].value, 42.0);
    }
    
    #[tokio::test]
    async fn test_feature_filtering() {
        let config = FeatureStoreConfig::default();
        let store = FeatureStore::new(config).await.unwrap();
        
        let now = Utc::now();
        let features = vec![
            Feature {
                name: "feature1".to_string(),
                value: 1.0,
                feature_type: super::super::FeatureType::Numerical,
                timestamp: now - Duration::hours(2),
                metadata: None,
            },
            Feature {
                name: "feature2".to_string(),
                value: 2.0,
                feature_type: super::super::FeatureType::Numerical,
                timestamp: now - Duration::hours(1),
                metadata: None,
            },
        ];
        
        store.store_features("test", &features, None).await.unwrap();
        
        // Filter by name
        let filtered = store
            .retrieve_features("test", &["feature1".to_string()], None, None)
            .await
            .unwrap();
        
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "feature1");
        
        // Filter by time range
        let time_filtered = store
            .retrieve_features(
                "test",
                &[],
                Some((now - Duration::minutes(90), now)),
                None,
            )
            .await
            .unwrap();
        
        assert_eq!(time_filtered.len(), 1);
        assert_eq!(time_filtered[0].name, "feature2");
    }
}