/*! Multi-channel Redis subscription manager with fair processing.
 *
 * This module provides the infrastructure for subscribing to multiple
 * symbol-specific Redis channels while ensuring fair processing to prevent
 * any single symbol from monopolizing system resources.
 */

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};
// use futures::StreamExt; // Unused for now
use serde::{Deserialize, Serialize};

use crate::adapters::{AdapterError, MarketData};

/// Load enabled symbols from environment variable TRADING_SYMBOLS_PRIMARY
/// Returns None if environment variable is not set or invalid
fn load_enabled_symbols_from_env() -> Option<Vec<String>> {
    use std::env;
    
    if let Ok(symbols_env) = env::var("TRADING_SYMBOLS_PRIMARY") {
        let symbols: Vec<String> = symbols_env
            .split(',')
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty())
            .collect();
        
        if !symbols.is_empty() {
            tracing::info!("Loaded {} enabled symbols from TRADING_SYMBOLS_PRIMARY: {:?}", symbols.len(), symbols);
            return Some(symbols);
        }
    }
    
    // Try to load from sector configuration if available
    if let Ok(symbols) = load_sector_aware_symbols() {
        tracing::info!("Loaded {} symbols from sector configuration", symbols.len());
        return Some(symbols);
    }
    
    None
}

/// Load symbols from sector configuration with memory optimization
fn load_sector_aware_symbols() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    use std::fs;
    use std::path::Path;
    
    let config_path = "neural-trader-config/sector_models.toml";
    
    if !Path::new(config_path).exists() {
        return Err("Sector configuration file not found".into());
    }
    
    let content = fs::read_to_string(config_path)?;
    let config: toml::Value = content.parse()?;
    
    let mut symbols = Vec::new();
    
    // Extract high-priority symbols from sectors
    if let Some(sectors) = config.get("sectors").and_then(|s| s.as_table()) {
        // Sort sectors by weight to prioritize important ones
        let mut sector_entries: Vec<_> = sectors.iter().collect();
        sector_entries.sort_by(|a, b| {
            let weight_a = a.1.get("sector_weight").and_then(|w| w.as_float()).unwrap_or(0.0);
            let weight_b = b.1.get("sector_weight").and_then(|w| w.as_float()).unwrap_or(0.0);
            weight_b.partial_cmp(&weight_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        for (_sector_name, sector_data) in sector_entries {
            if let Some(sector_symbols) = sector_data.get("symbols").and_then(|s| s.as_array()) {
                let sector_weight = sector_data
                    .get("sector_weight")
                    .and_then(|v| v.as_float())
                    .unwrap_or(0.0);
                
                // Only include sectors with meaningful weight
                if sector_weight >= 0.05 {  // 5% minimum sector weight
                    let max_symbols = if sector_weight >= 0.15 { 3 } else { 2 }; // Top sectors get more symbols
                    
                    for (i, symbol) in sector_symbols.iter().enumerate() {
                        if i >= max_symbols { break; }
                        if let Some(symbol_str) = symbol.as_str() {
                            symbols.push(symbol_str.to_string());
                        }
                    }
                }
            }
        }
    }
    
    // Ensure reasonable symbol count for memory constraints
    symbols.sort();
    symbols.dedup();
    symbols.truncate(16);  // Limit to 16 symbols for memory efficiency
    
    if symbols.is_empty() {
        return Err("No valid symbols found in sector configuration".into());
    }
    
    Ok(symbols)
}
use crate::adapters::redis::RedisAdapter;
use crate::streaming::event_bus::EventBusIntegration;

pub mod fair_scheduler;
pub mod worker_pool;
pub mod channel_manager;

#[cfg(test)]
mod tests;

pub use fair_scheduler::*;
pub use worker_pool::*;
pub use channel_manager::*;

/// Configuration for multi-channel subscription system
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "serde")]
pub struct MultiChannelConfig {
    /// Symbols to subscribe to
    pub enabled_symbols: Vec<String>,
    /// Maximum concurrent subscriptions
    pub max_concurrent_subscriptions: usize,
    /// Worker pool size (defaults to CPU cores * 2)
    pub worker_pool_size: Option<usize>,
    /// Queue size per worker
    pub worker_queue_size: usize,
    /// Maximum percentage any symbol can consume (0.0-1.0)
    pub max_symbol_percentage: f64,
    /// Time window for fairness calculation (seconds)
    pub fairness_window_seconds: u64,
    /// Processing timeout per event (milliseconds)
    pub processing_timeout_ms: u64,
    /// Memory limit in MB
    pub memory_limit_mb: usize,
    /// Redis reconnection interval
    pub reconnect_interval_ms: u64,
}

impl Default for MultiChannelConfig {
    fn default() -> Self {
        // Load symbols from environment or configuration, fallback to primary symbols
        let enabled_symbols = load_enabled_symbols_from_env().unwrap_or_else(|| {
            vec![
                "AAPL".to_string(), 
                "NVDA".to_string(), 
                "MSFT".to_string(),
                "GOOGL".to_string(), 
                "TSLA".to_string(),
                "AMZN".to_string(),
                "META".to_string(),
                "DDOG".to_string()
            ]
        });
        
        Self {
            enabled_symbols,
            max_concurrent_subscriptions: 100,
            worker_pool_size: None, // Auto-detect CPU cores
            worker_queue_size: 1000,
            max_symbol_percentage: 0.20, // 20% maximum per symbol
            fairness_window_seconds: 60,
            processing_timeout_ms: 200,
            memory_limit_mb: 500,
            reconnect_interval_ms: 5000,
        }
    }
}

/// Work item for processing queue
#[derive(Debug, Clone)]
pub struct WorkItem {
    pub symbol: String,
    pub channel: String,
    pub market_data: MarketData,
    pub received_at: Instant,
    pub priority: f64,
}

// Manual Send + Sync implementation for WorkItem
unsafe impl Send for WorkItem {}
unsafe impl Sync for WorkItem {}

/// Processing priority levels
#[derive(Debug, Clone, Copy)]
pub enum ProcessingPriority {
    High = 3,
    Normal = 2,
    Low = 1,
    Throttled = 0,
}

impl From<ProcessingPriority> for f64 {
    fn from(priority: ProcessingPriority) -> f64 {
        priority as i32 as f64
    }
}

/// Per-symbol processing statistics
#[derive(Debug, Clone, Default)]
pub struct SymbolStats {
    pub messages_processed: u64,
    pub total_processing_time: Duration,
    pub average_latency: Duration,
    pub last_processed: Option<Instant>,
    pub throttle_count: u32,
}

/// System-wide processing metrics
#[derive(Debug, Clone)]
pub struct ProcessingMetrics {
    pub symbol_stats: HashMap<String, SymbolStats>,
    pub total_messages: u64,
    pub total_processing_time: Duration,
    pub fairness_violations: u32,
    pub system_start_time: Instant,
}

impl Default for ProcessingMetrics {
    fn default() -> Self {
        Self {
            symbol_stats: HashMap::new(),
            total_messages: 0,
            total_processing_time: Duration::ZERO,
            fairness_violations: 0,
            system_start_time: Instant::now(),
        }
    }
}

/// Main multi-channel subscription manager
pub struct MultiChannelSubscriptionManager {
    config: MultiChannelConfig,
    redis_adapter: Arc<RedisAdapter>,
    event_bus: Arc<EventBusIntegration>,
    worker_pool: Arc<RwLock<WorkerPool>>,
    fair_scheduler: Arc<RwLock<FairProcessingScheduler>>,
    channel_manager: Arc<RwLock<ChannelSubscriptionManager>>,
    metrics: Arc<RwLock<ProcessingMetrics>>,
    shutdown_signal: Arc<AtomicBool>,
}

impl MultiChannelSubscriptionManager {
    /// Create new multi-channel subscription manager
    pub fn new(
        config: MultiChannelConfig,
        redis_adapter: Arc<RedisAdapter>,
        event_bus: Arc<EventBusIntegration>,
    ) -> Self {
        let worker_pool_size = config.worker_pool_size
            .unwrap_or_else(|| num_cpus::get() * 2);
            
        let fair_scheduler = Arc::new(RwLock::new(
            FairProcessingScheduler::new(
                Duration::from_secs(config.fairness_window_seconds),
                config.max_symbol_percentage,
            )
        ));
        
        let worker_pool = Arc::new(RwLock::new(WorkerPool::new(
            worker_pool_size,
            config.worker_queue_size,
            event_bus.clone(),
            fair_scheduler.clone(),
        )));
        
        let channel_manager = Arc::new(RwLock::new(
            ChannelSubscriptionManager::new(
                redis_adapter.clone(),
                worker_pool.clone(),
            )
        ));
        
        Self {
            config,
            redis_adapter,
            event_bus,
            worker_pool,
            fair_scheduler,
            channel_manager,
            metrics: Arc::new(RwLock::new(ProcessingMetrics::default())),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
        }
    }
    
    /// Start multi-channel subscriptions
    pub async fn start(&self) -> Result<(), AdapterError> {
        tracing::info!("Starting multi-channel subscription manager");
        
        // Start worker pool
        {
            let mut worker_pool = self.worker_pool.write().await;
            worker_pool.start().await?;
        }
        
        // Subscribe to all configured symbols
        for symbol in &self.config.enabled_symbols {
            self.subscribe_to_symbol(symbol.clone()).await?;
        }
        
        // Start fairness monitoring
        self.start_fairness_monitor().await;
        
        // Start metrics collection
        self.start_metrics_collector().await;
        
        tracing::info!("Multi-channel subscription manager started successfully");
        Ok(())
    }
    
    /// Subscribe to a specific symbol channel
    pub async fn subscribe_to_symbol(&self, symbol: String) -> Result<(), AdapterError> {
        let channel = format!("market:{}", symbol);
        tracing::info!("Subscribing to channel: {}", channel);
        
        let channel_manager = self.channel_manager.read().await;
        channel_manager.add_subscription(symbol, channel).await?;
        
        Ok(())
    }
    
    /// Unsubscribe from a symbol channel
    pub async fn unsubscribe_from_symbol(&self, symbol: &str) -> Result<(), AdapterError> {
        let channel_manager = self.channel_manager.read().await;
        channel_manager.remove_subscription(symbol).await?;
        Ok(())
    }
    
    /// Get current processing statistics
    pub async fn get_processing_stats(&self) -> ProcessingMetrics {
        self.metrics.read().await.clone()
    }
    
    /// Get fair processing compliance rate
    pub async fn get_fairness_compliance(&self) -> f64 {
        let scheduler = self.fair_scheduler.read().await;
        scheduler.get_compliance_rate()
    }
    
    /// Shutdown the subscription manager
    pub async fn shutdown(&self) -> Result<(), AdapterError> {
        tracing::info!("Shutting down multi-channel subscription manager");
        
        // Signal shutdown
        self.shutdown_signal.store(true, Ordering::Relaxed);
        
        // Shutdown components
        {
            let channel_manager = self.channel_manager.read().await;
            channel_manager.shutdown().await?;
        }
        
        {
            let worker_pool = self.worker_pool.read().await;
            worker_pool.shutdown().await?;
        }
        
        tracing::info!("Multi-channel subscription manager shutdown complete");
        Ok(())
    }
    
    /// Start fairness monitoring task
    async fn start_fairness_monitor(&self) {
        let scheduler = self.fair_scheduler.clone();
        let metrics = self.metrics.clone();
        let shutdown = self.shutdown_signal.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            
            while !shutdown.load(Ordering::Relaxed) {
                interval.tick().await;
                
                let compliance_rate = {
                    let scheduler_guard = scheduler.read().await;
                    scheduler_guard.get_compliance_rate()
                };
                
                if compliance_rate < 0.99 {
                    tracing::warn!("Fairness compliance below threshold: {:.2}%", compliance_rate * 100.0);
                    
                    let mut metrics_guard = metrics.write().await;
                    metrics_guard.fairness_violations += 1;
                }
                
                // Log processing statistics
                let stats = {
                    let scheduler_guard = scheduler.read().await;
                    scheduler_guard.get_processing_stats()
                };
                
                tracing::debug!("Processing stats: {:?}", stats);
            }
        });
    }
    
    /// Start metrics collection task
    async fn start_metrics_collector(&self) {
        let metrics = self.metrics.clone();
        let worker_pool = self.worker_pool.clone();
        let shutdown = self.shutdown_signal.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            
            while !shutdown.load(Ordering::Relaxed) {
                interval.tick().await;
                
                let worker_stats = {
                    let worker_pool = worker_pool.read().await;
                    worker_pool.get_worker_statistics().await
                };
                
                let mut metrics_guard = metrics.write().await;
                
                // Update system metrics
                metrics_guard.total_messages = worker_stats.total_processed;
                
                // Update per-symbol stats
                for (symbol, stats) in worker_stats.symbol_stats {
                    metrics_guard.symbol_stats.insert(symbol, stats);
                }
                
                tracing::info!("Metrics updated: {} total messages processed", metrics_guard.total_messages);
            }
        });
    }
}