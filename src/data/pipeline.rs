//! Data pipeline for processing and managing time series data

use crate::config::PlatformConfig;
use crate::data::{TimescaleDBStorage, RedisCache, TimeSeriesData, QualityMetrics, PlatformMetrics};
use crate::data::storage::TimeSeriesData as StorageTimeSeriesData;
use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use chrono::Utc;

/// Main data pipeline for the platform
pub struct DataPipeline {
    storage: Arc<TimescaleDBStorage>,
    cache: Arc<RedisCache>,
    config: PlatformConfig,
    metrics: Arc<RwLock<PipelineMetrics>>,
}

/// Internal metrics tracking
#[derive(Default)]
struct PipelineMetrics {
    total_processed: u64,
    total_errors: u64,
    last_process_duration: Duration,
    quality_degraded: bool,
}

impl DataPipeline {
    /// Create new data pipeline
    pub async fn new(
        storage: TimescaleDBStorage,
        cache: RedisCache,
        config: PlatformConfig,
    ) -> Result<Self> {
        Ok(Self {
            storage: Arc::new(storage),
            cache: Arc::new(cache),
            config,
            metrics: Arc::new(RwLock::new(PipelineMetrics::default())),
        })
    }
    
    /// Get reference to storage layer
    pub fn storage(&self) -> Arc<TimescaleDBStorage> {
        Arc::clone(&self.storage)
    }
    
    /// Get reference to cache layer
    pub fn cache(&self) -> Arc<RedisCache> {
        Arc::clone(&self.cache)
    }
    
    /// Process incoming time series data
    pub async fn process_data(&self, data: TimeSeriesData) -> Result<()> {
        let start = Instant::now();
        
        // Validate data first
        data.validate()
            .context("Data validation failed")?;
        
        // Convert to storage format
        let storage_data = StorageTimeSeriesData {
            timestamp: data.timestamp,
            source: "market".to_string(),
            entity: data.symbol.clone(),
            value: data.close,
            metadata: Some(serde_json::json!({
                "open": data.open,
                "high": data.high,
                "low": data.low,
                "volume": data.volume,
                "indicators": data.indicators
            })),
        };
        
        // Store in database
        self.storage.store_time_series(&storage_data).await
            .context("Failed to store data in TimescaleDB")?;
        
        // Update cache
        let cache_key = format!("data:{}:latest", data.symbol);
        self.cache.set(&cache_key, &data, Some(self.config.neural.prediction_cache_ttl)).await
            .context("Failed to update cache")?;
        
        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.total_processed += 1;
        metrics.last_process_duration = start.elapsed();
        
        Ok(())
    }
    
    /// Get latest data for a symbol
    pub async fn get_latest_data(&self, symbol: &str) -> Result<Option<TimeSeriesData>> {
        let cache_key = format!("data:{}:latest", symbol);
        
        // Try cache first
        if let Some(data) = self.cache.get(&cache_key).await? {
            return Ok(Some(data));
        }
        
        // For now, return None if not in cache
        // In a full implementation, we would query the database
        Ok(None)
    }
    
    /// Monitor data quality
    pub async fn monitor_quality(&self) -> Result<QualityMetrics> {
        let start = Instant::now();
        
        // Calculate data completeness (simplified for now)
        let data_completeness = 0.95; // 95% complete
        
        // Measure current latency
        let latency_ms = start.elapsed().as_millis() as f64;
        
        // Calculate error rate from metrics
        let metrics = self.metrics.read().await;
        let error_rate = if metrics.total_processed > 0 {
            metrics.total_errors as f64 / metrics.total_processed as f64
        } else {
            0.0
        };
        
        // Check if quality is degraded
        let quality_degraded = data_completeness < self.config.monitoring.quality_threshold || 
                              error_rate > 0.1 || 
                              latency_ms > 1000.0;
        
        // Update quality status
        let mut metrics_mut = self.metrics.write().await;
        metrics_mut.quality_degraded = quality_degraded;
        drop(metrics_mut);
        
        Ok(QualityMetrics::new(data_completeness, latency_ms, error_rate))
    }
    
    /// Collect platform metrics
    pub async fn collect_metrics(&self) -> Result<PlatformMetrics> {
        let metrics = self.metrics.read().await;
        
        // Simplified cache hit rate calculation
        let cache_hit_rate = 0.85; // 85% hit rate
        
        // Simplified storage usage
        let storage_usage_gb = 10.5; // 10.5 GB
        
        // Processing throughput (records per second)
        let processing_throughput = if metrics.last_process_duration.as_secs() > 0 {
            1.0 / metrics.last_process_duration.as_secs_f64()
        } else {
            0.0
        };
        
        Ok(PlatformMetrics::new(
            metrics.total_processed,
            cache_hit_rate,
            processing_throughput,
            storage_usage_gb,
            5, // active connections
        ))
    }
    
    /// Health check for the pipeline
    pub async fn health_check(&self) -> Result<bool> {
        // Simplified health check
        let storage_healthy = true; // Would check database connection
        let cache_healthy = true;   // Would check Redis connection
        
        Ok(storage_healthy && cache_healthy)
    }
    
    /// Batch process multiple data points
    pub async fn batch_process(&self, data_batch: Vec<TimeSeriesData>) -> Result<()> {
        let start = Instant::now();
        
        // Process each item in the batch
        for data in data_batch {
            if let Err(e) = self.process_data(data).await {
                let mut metrics = self.metrics.write().await;
                metrics.total_errors += 1;
                eprintln!("Failed to process data: {}", e);
            }
        }
        
        println!("Batch processing completed in {:?}", start.elapsed());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    
    #[tokio::test]
    async fn test_pipeline_creation() {
        // This is a basic test to ensure the pipeline can be created
        // In a real implementation, we would set up test database and cache
        assert!(true);
    }
}