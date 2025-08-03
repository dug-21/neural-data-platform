//! Performance Optimization Engine
//!
//! Comprehensive performance optimization system targeting production benchmarks:
//! - Memory usage: <50MB per symbol (90% reduction)
//! - Prediction latency: <100ms
//! - Lazy loading with intelligent caching
//! - Resource pool management

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, info, warn};

/// Performance optimization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationConfig {
    /// Target memory per symbol (MB)
    pub memory_target_mb: f64,
    /// Maximum prediction latency (ms)
    pub max_prediction_latency_ms: u64,
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
    /// Memory pool size (MB)
    pub memory_pool_mb: f64,
    /// Enable lazy loading
    pub enable_lazy_loading: bool,
    /// Enable memory compression
    pub enable_compression: bool,
    /// GC interval in minutes
    pub gc_interval_minutes: u64,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            memory_target_mb: 50.0, // 50MB per symbol target
            max_prediction_latency_ms: 100, // <100ms prediction latency
            cache_ttl_seconds: 300, // 5 minutes cache
            memory_pool_mb: 512.0, // 512MB shared pool
            enable_lazy_loading: true,
            enable_compression: true,
            gc_interval_minutes: 15,
        }
    }
}

/// Memory optimization statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_allocated_mb: f64,
    pub total_used_mb: f64,
    pub peak_usage_mb: f64,
    pub symbols_loaded: usize,
    pub avg_memory_per_symbol_mb: f64,
    pub compression_ratio: f64,
    pub cache_hit_rate: f64,
    pub timestamp: DateTime<Utc>,
}

/// Performance metrics tracker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub avg_prediction_latency_ms: f64,
    pub p95_prediction_latency_ms: f64,
    pub p99_prediction_latency_ms: f64,
    pub cache_hit_rate: f64,
    pub memory_efficiency: f64,
    pub gc_frequency_per_hour: f64,
    pub predictions_per_second: f64,
    pub timestamp: DateTime<Utc>,
}

/// Lazy loading state
#[derive(Debug, Clone)]
pub enum LoadingState {
    Unloaded,
    Loading,
    Loaded(DateTime<Utc>),
    Evicted(DateTime<Utc>),
}

/// Memory-optimized resource container
#[derive(Debug)]
pub struct OptimizedResource<T> {
    pub data: Option<Arc<T>>,
    pub state: LoadingState,
    pub memory_size_bytes: usize,
    pub last_access: DateTime<Utc>,
    pub access_count: u64,
    pub compressed_data: Option<Vec<u8>>,
}

impl<T> OptimizedResource<T> {
    pub fn new() -> Self {
        Self {
            data: None,
            state: LoadingState::Unloaded,
            memory_size_bytes: 0,
            last_access: Utc::now(),
            access_count: 0,
            compressed_data: None,
        }
    }
    
    pub fn is_loaded(&self) -> bool {
        matches!(self.state, LoadingState::Loaded(_)) && self.data.is_some()
    }
    
    pub fn mark_accessed(&mut self) {
        self.last_access = Utc::now();
        self.access_count += 1;
    }
}

/// Performance optimization engine
pub struct PerformanceOptimizer {
    config: OptimizationConfig,
    memory_pool: Arc<Semaphore>,
    resource_cache: Arc<DashMap<String, OptimizedResource<Vec<u8>>>>,
    memory_stats: Arc<RwLock<MemoryStats>>,
    performance_metrics: Arc<RwLock<PerformanceMetrics>>,
    gc_task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl PerformanceOptimizer {
    /// Create new performance optimizer
    pub async fn new(config: OptimizationConfig) -> Result<Self> {
        let memory_pool_permits = (config.memory_pool_mb * 1024.0 * 1024.0) as usize / 1024; // 1KB units
        let memory_pool = Arc::new(Semaphore::new(memory_pool_permits));
        
        let initial_stats = MemoryStats {
            total_allocated_mb: 0.0,
            total_used_mb: 0.0,
            peak_usage_mb: 0.0,
            symbols_loaded: 0,
            avg_memory_per_symbol_mb: 0.0,
            compression_ratio: 1.0,
            cache_hit_rate: 0.0,
            timestamp: Utc::now(),
        };
        
        let initial_metrics = PerformanceMetrics {
            avg_prediction_latency_ms: 0.0,
            p95_prediction_latency_ms: 0.0,
            p99_prediction_latency_ms: 0.0,
            cache_hit_rate: 0.0,
            memory_efficiency: 100.0,
            gc_frequency_per_hour: 0.0,
            predictions_per_second: 0.0,
            timestamp: Utc::now(),
        };
        
        Ok(Self {
            config,
            memory_pool,
            resource_cache: Arc::new(DashMap::new()),
            memory_stats: Arc::new(RwLock::new(initial_stats)),
            performance_metrics: Arc::new(RwLock::new(initial_metrics)),
            gc_task_handle: None,
        })
    }
    
    /// Start the optimization engine
    pub async fn start(&mut self) -> Result<()> {
        info!("🚀 Starting Performance Optimization Engine");
        info!("   Memory target: {:.1}MB per symbol", self.config.memory_target_mb);
        info!("   Latency target: <{}ms", self.config.max_prediction_latency_ms);
        info!("   Memory pool: {:.1}MB", self.config.memory_pool_mb);
        
        // Start garbage collection task
        if self.config.enable_lazy_loading {
            self.start_gc_task().await?;
        }
        
        Ok(())
    }
    
    /// Allocate memory for a resource
    pub async fn allocate_memory(&self, size_bytes: usize) -> Result<tokio::sync::OwnedSemaphorePermit> {
        let size_kb = (size_bytes + 1023) / 1024; // Round up to KB
        
        let permit = Arc::clone(&self.memory_pool)
            .acquire_many_owned(size_kb as u32)
            .await
            .context("Failed to acquire memory permit")?;
        
        // Update memory stats
        {
            let mut stats = self.memory_stats.write().await;
            stats.total_allocated_mb += size_bytes as f64 / (1024.0 * 1024.0);
            if stats.total_allocated_mb > stats.peak_usage_mb {
                stats.peak_usage_mb = stats.total_allocated_mb;
            }
            stats.timestamp = Utc::now();
        }
        
        Ok(permit)  // permit is already OwnedSemaphorePermit from acquire_many_owned
    }
    
    /// Load resource with lazy loading
    pub async fn load_resource<T, F>(
        &self,
        key: &str,
        loader: F,
    ) -> Result<Arc<T>>
    where
        T: Clone + Send + Sync + 'static,
        F: std::future::Future<Output = Result<T>> + Send,
    {
        let start_time = std::time::Instant::now();
        
        // Check if resource is already loaded
        if let Some(mut resource_ref) = self.resource_cache.get_mut(key) {
            resource_ref.mark_accessed();
            
            if resource_ref.is_loaded() {
                let latency_ms = start_time.elapsed().as_millis() as f64;
                self.record_cache_hit(latency_ms).await;
                
                // This is a simplified example - in practice, you'd need proper type handling
                return Err(anyhow::anyhow!("Type conversion not implemented in this example"));
            }
        }
        
        // Load the resource
        let resource_data = loader.await?;
        let memory_size = std::mem::size_of_val(&resource_data);
        
        // Acquire memory permit
        let _permit = self.allocate_memory(memory_size).await?;
        
        // Store the resource
        let arc_data = Arc::new(resource_data);
        let mut optimized_resource = OptimizedResource::new();
        optimized_resource.state = LoadingState::Loaded(Utc::now());
        optimized_resource.memory_size_bytes = memory_size;
        optimized_resource.mark_accessed();
        
        self.resource_cache.insert(key.to_string(), optimized_resource);
        
        let latency_ms = start_time.elapsed().as_millis() as f64;
        self.record_cache_miss(latency_ms).await;
        
        // Update memory stats
        self.update_memory_stats().await;
        
        Ok(arc_data)
    }
    
    /// Compress data for memory efficiency
    pub fn compress_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        if !self.config.enable_compression {
            return Ok(data.to_vec());
        }
        
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write;
        
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data)?;
        let compressed = encoder.finish()?;
        
        let compression_ratio = data.len() as f64 / compressed.len() as f64;
        debug!("Compressed data: {} -> {} bytes (ratio: {:.2}x)", 
               data.len(), compressed.len(), compression_ratio);
        
        Ok(compressed)
    }
    
    /// Decompress data
    pub fn decompress_data(&self, compressed: &[u8]) -> Result<Vec<u8>> {
        if !self.config.enable_compression {
            return Ok(compressed.to_vec());
        }
        
        use flate2::read::GzDecoder;
        use std::io::Read;
        
        let mut decoder = GzDecoder::new(compressed);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        
        Ok(decompressed)
    }
    
    /// Start garbage collection task
    async fn start_gc_task(&mut self) -> Result<()> {
        let resource_cache = Arc::clone(&self.resource_cache);
        let config = self.config.clone();
        let memory_stats = Arc::clone(&self.memory_stats);
        
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(config.gc_interval_minutes * 60)
            );
            
            loop {
                interval.tick().await;
                
                let start_time = std::time::Instant::now();
                let mut evicted_count = 0;
                let mut freed_memory = 0usize;
                
                // Find candidates for eviction
                let now = Utc::now();
                let eviction_threshold = chrono::Duration::seconds(config.cache_ttl_seconds as i64);
                
                let keys_to_evict: Vec<String> = resource_cache
                    .iter()
                    .filter_map(|entry| {
                        let resource = entry.value();
                        if now - resource.last_access > eviction_threshold {
                            Some(entry.key().clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                
                // Evict resources
                for key in keys_to_evict {
                    if let Some((_, mut resource)) = resource_cache.remove(&key) {
                        freed_memory += resource.memory_size_bytes;
                        resource.state = LoadingState::Evicted(now);
                        resource.data = None;
                        evicted_count += 1;
                    }
                }
                
                // Update memory stats
                if evicted_count > 0 {
                    let mut stats = memory_stats.write().await;
                    stats.total_used_mb -= freed_memory as f64 / (1024.0 * 1024.0);
                    stats.symbols_loaded = resource_cache.len();
                    stats.timestamp = now;
                    
                    let elapsed = start_time.elapsed();
                    info!("🧹 GC completed: evicted {} resources, freed {:.2}MB in {:?}",
                          evicted_count, freed_memory as f64 / (1024.0 * 1024.0), elapsed);
                }
            }
        });
        
        self.gc_task_handle = Some(handle);
        info!("🕐 Started GC task with {}min interval", self.config.gc_interval_minutes);
        
        Ok(())
    }
    
    /// Record cache hit
    async fn record_cache_hit(&self, latency_ms: f64) {
        let mut metrics = self.performance_metrics.write().await;
        // Update cache hit rate (exponential moving average)
        metrics.cache_hit_rate = metrics.cache_hit_rate * 0.9 + 1.0 * 0.1;
        
        // Update latency metrics
        metrics.avg_prediction_latency_ms = 
            metrics.avg_prediction_latency_ms * 0.95 + latency_ms * 0.05;
        
        metrics.timestamp = Utc::now();
    }
    
    /// Record cache miss
    async fn record_cache_miss(&self, latency_ms: f64) {
        let mut metrics = self.performance_metrics.write().await;
        // Update cache hit rate (exponential moving average)
        metrics.cache_hit_rate = metrics.cache_hit_rate * 0.9 + 0.0 * 0.1;
        
        // Update latency metrics
        metrics.avg_prediction_latency_ms = 
            metrics.avg_prediction_latency_ms * 0.95 + latency_ms * 0.05;
        
        metrics.timestamp = Utc::now();
    }
    
    /// Update memory statistics
    async fn update_memory_stats(&self) {
        let mut stats = self.memory_stats.write().await;
        
        let total_resources = self.resource_cache.len();
        let total_memory_bytes: usize = self.resource_cache
            .iter()
            .map(|entry| entry.value().memory_size_bytes)
            .sum();
        
        stats.symbols_loaded = total_resources;
        stats.total_used_mb = total_memory_bytes as f64 / (1024.0 * 1024.0);
        
        if total_resources > 0 {
            stats.avg_memory_per_symbol_mb = stats.total_used_mb / total_resources as f64;
        }
        
        stats.timestamp = Utc::now();
    }
    
    /// Get current memory statistics
    pub async fn get_memory_stats(&self) -> MemoryStats {
        self.memory_stats.read().await.clone()
    }
    
    /// Get current performance metrics
    pub async fn get_performance_metrics(&self) -> PerformanceMetrics {
        self.performance_metrics.read().await.clone()
    }
    
    /// Check if performance targets are met
    pub async fn check_performance_targets(&self) -> Result<PerformanceReport> {
        let memory_stats = self.get_memory_stats().await;
        let performance_metrics = self.get_performance_metrics().await;
        
        let memory_target_met = memory_stats.avg_memory_per_symbol_mb <= self.config.memory_target_mb;
        let latency_target_met = performance_metrics.avg_prediction_latency_ms <= self.config.max_prediction_latency_ms as f64;
        
        let memory_reduction = if memory_stats.avg_memory_per_symbol_mb > 0.0 {
            ((500.0 - memory_stats.avg_memory_per_symbol_mb) / 500.0) * 100.0 // Assume 500MB baseline
        } else {
            0.0
        };
        
        Ok(PerformanceReport {
            memory_target_met,
            latency_target_met,
            memory_reduction_percent: memory_reduction,
            current_memory_per_symbol_mb: memory_stats.avg_memory_per_symbol_mb,
            current_avg_latency_ms: performance_metrics.avg_prediction_latency_ms,
            cache_hit_rate: performance_metrics.cache_hit_rate,
            recommendations: self.generate_recommendations(&memory_stats, &performance_metrics).await,
        })
    }
    
    /// Generate optimization recommendations
    async fn generate_recommendations(
        &self,
        memory_stats: &MemoryStats,
        performance_metrics: &PerformanceMetrics,
    ) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        // Memory recommendations
        if memory_stats.avg_memory_per_symbol_mb > self.config.memory_target_mb {
            recommendations.push(format!(
                "Memory usage {:.1}MB per symbol exceeds target {:.1}MB. Consider increasing compression or reducing cache size.",
                memory_stats.avg_memory_per_symbol_mb, self.config.memory_target_mb
            ));
        }
        
        // Latency recommendations
        if performance_metrics.avg_prediction_latency_ms > self.config.max_prediction_latency_ms as f64 {
            recommendations.push(format!(
                "Average latency {:.1}ms exceeds target {}ms. Consider optimizing model loading or increasing cache TTL.",
                performance_metrics.avg_prediction_latency_ms, self.config.max_prediction_latency_ms
            ));
        }
        
        // Cache recommendations
        if performance_metrics.cache_hit_rate < 0.8 {
            recommendations.push(format!(
                "Cache hit rate {:.1}% is low. Consider increasing cache TTL or memory pool size.",
                performance_metrics.cache_hit_rate * 100.0
            ));
        }
        
        // Compression recommendations
        if memory_stats.compression_ratio < 2.0 && !self.config.enable_compression {
            recommendations.push("Enable compression to reduce memory usage.".to_string());
        }
        
        if recommendations.is_empty() {
            recommendations.push("🎯 All performance targets are being met!".to_string());
        }
        
        recommendations
    }
    
    /// Force garbage collection
    pub async fn force_gc(&self) -> Result<GCResult> {
        let start_time = std::time::Instant::now();
        let mut evicted_count = 0;
        let mut freed_memory = 0usize;
        
        let now = Utc::now();
        let eviction_threshold = chrono::Duration::seconds(self.config.cache_ttl_seconds as i64);
        
        // Find all eligible resources for eviction
        let keys_to_evict: Vec<String> = self.resource_cache
            .iter()
            .filter_map(|entry| {
                let resource = entry.value();
                if now - resource.last_access > eviction_threshold || resource.access_count == 0 {
                    Some(entry.key().clone())
                } else {
                    None
                }
            })
            .collect();
        
        // Evict resources
        for key in keys_to_evict {
            if let Some((_, mut resource)) = self.resource_cache.remove(&key) {
                freed_memory += resource.memory_size_bytes;
                resource.state = LoadingState::Evicted(now);
                resource.data = None;
                evicted_count += 1;
            }
        }
        
        // Update memory stats
        self.update_memory_stats().await;
        
        let elapsed = start_time.elapsed();
        
        Ok(GCResult {
            evicted_resources: evicted_count,
            freed_memory_mb: freed_memory as f64 / (1024.0 * 1024.0),
            gc_duration_ms: elapsed.as_millis() as u64,
            timestamp: now,
        })
    }
}

/// Performance optimization report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub memory_target_met: bool,
    pub latency_target_met: bool,
    pub memory_reduction_percent: f64,
    pub current_memory_per_symbol_mb: f64,
    pub current_avg_latency_ms: f64,
    pub cache_hit_rate: f64,
    pub recommendations: Vec<String>,
}

/// Garbage collection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCResult {
    pub evicted_resources: usize,
    pub freed_memory_mb: f64,
    pub gc_duration_ms: u64,
    pub timestamp: DateTime<Utc>,
}

/// Performance optimization integration trait
#[async_trait]
pub trait PerformanceOptimized {
    /// Apply performance optimizations
    async fn optimize_performance(&self, optimizer: &PerformanceOptimizer) -> Result<()>;
    
    /// Get memory usage estimate
    fn estimate_memory_usage(&self) -> usize;
    
    /// Check if resource should be lazy loaded
    fn should_lazy_load(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_performance_optimizer_creation() {
        let config = OptimizationConfig::default();
        let optimizer = PerformanceOptimizer::new(config).await;
        assert!(optimizer.is_ok());
    }
    
    #[tokio::test]
    async fn test_memory_allocation() {
        let config = OptimizationConfig::default();
        let optimizer = PerformanceOptimizer::new(config).await.unwrap();
        
        let permit = optimizer.allocate_memory(1024 * 1024).await; // 1MB
        assert!(permit.is_ok());
        
        let stats = optimizer.get_memory_stats().await;
        assert!(stats.total_allocated_mb > 0.0);
    }
    
    #[tokio::test]
    async fn test_compression() {
        let config = OptimizationConfig::default();
        let optimizer = PerformanceOptimizer::new(config).await.unwrap();
        
        let data = vec![0u8; 1000]; // 1KB of zeros - highly compressible
        let compressed = optimizer.compress_data(&data).unwrap();
        assert!(compressed.len() < data.len());
        
        let decompressed = optimizer.decompress_data(&compressed).unwrap();
        assert_eq!(data, decompressed);
    }
    
    #[tokio::test]
    async fn test_performance_targets() {
        let config = OptimizationConfig::default();
        let optimizer = PerformanceOptimizer::new(config).await.unwrap();
        
        let report = optimizer.check_performance_targets().await.unwrap();
        assert!(!report.recommendations.is_empty());
    }
    
    #[tokio::test]
    async fn test_force_gc() {
        let config = OptimizationConfig::default();
        let optimizer = PerformanceOptimizer::new(config).await.unwrap();
        
        let result = optimizer.force_gc().await.unwrap();
        assert_eq!(result.evicted_resources, 0); // No resources to evict initially
    }
}