//! Memory Protection System for Neural Trader
//!
//! This module provides comprehensive memory protection strategies including:
//! - Bounded data structures with automatic eviction
//! - Time-based TTL policies
//! - Memory usage monitoring and alerting
//! - Circuit breakers for memory exhaustion
//! - Graceful degradation under resource pressure

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use tracing::{debug, error, info, warn};

pub mod bounded_structures;
pub mod circuit_breakers;
pub mod eviction_policies;
pub mod memory_monitor;
pub mod resource_limiter;

/// Memory protection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryProtectionConfig {
    /// Maximum memory usage in bytes before triggering protection
    pub max_memory_bytes: usize,
    /// Maximum number of events to keep in bounded structures
    pub max_events_per_type: usize,
    /// TTL for events in seconds
    pub event_ttl_seconds: i64,
    /// Memory check interval in milliseconds
    pub memory_check_interval_ms: u64,
    /// Enable automatic garbage collection
    pub enable_auto_gc: bool,
    /// Circuit breaker failure threshold
    pub circuit_breaker_failure_threshold: u32,
    /// Circuit breaker timeout in seconds
    pub circuit_breaker_timeout_seconds: u64,
    /// Enable memory alerts
    pub enable_memory_alerts: bool,
    /// Memory alert threshold as percentage of max memory
    pub memory_alert_threshold_percent: f64,
}

impl Default for MemoryProtectionConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: 1024 * 1024 * 1024, // 1GB default
            max_events_per_type: 10_000,           // 10k events per type
            event_ttl_seconds: 3600,               // 1 hour TTL
            memory_check_interval_ms: 30_000,      // 30 seconds
            enable_auto_gc: true,
            circuit_breaker_failure_threshold: 5,
            circuit_breaker_timeout_seconds: 300, // 5 minutes
            enable_memory_alerts: true,
            memory_alert_threshold_percent: 0.8, // Alert at 80%
        }
    }
}

/// Memory usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_allocated_bytes: u64,
    pub heap_size_bytes: u64,
    pub rss_bytes: u64,
    pub available_memory_bytes: u64,
    pub memory_usage_percent: f64,
    pub gc_count: u64,
    pub last_gc_duration_ms: u64,
    pub timestamp: DateTime<Utc>,
}

/// Memory protection alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAlert {
    pub alert_type: MemoryAlertType,
    pub message: String,
    pub current_usage_bytes: u64,
    pub threshold_bytes: u64,
    pub severity: AlertSeverity,
    pub timestamp: DateTime<Utc>,
    pub recovery_suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryAlertType {
    HighMemoryUsage,
    MemoryLeak,
    CircuitBreakerTripped,
    EvictionTriggered,
    GarbageCollectionTimeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

/// Event with TTL metadata
#[derive(Debug, Clone)]
pub struct TtlEvent<T> {
    pub data: T,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub access_count: u32,
    pub last_accessed: DateTime<Utc>,
}

impl<T> TtlEvent<T> {
    pub fn new(data: T, ttl: Duration) -> Self {
        let now = Utc::now();
        Self {
            data,
            created_at: now,
            expires_at: now + ttl,
            access_count: 0,
            last_accessed: now,
        }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    pub fn access(&mut self) -> &T {
        self.access_count += 1;
        self.last_accessed = Utc::now();
        &self.data
    }
}

/// Bounded collection with TTL and LRU eviction
#[derive(Debug)]
pub struct BoundedTtlCollection<T> {
    data: VecDeque<TtlEvent<T>>,
    max_size: usize,
    ttl: Duration,
    total_insertions: AtomicU64,
    total_evictions: AtomicU64,
    expired_evictions: AtomicU64,
    size_evictions: AtomicU64,
}

impl<T> BoundedTtlCollection<T> {
    pub fn new(max_size: usize, ttl_seconds: i64) -> Self {
        Self {
            data: VecDeque::with_capacity(max_size),
            max_size,
            ttl: Duration::seconds(ttl_seconds),
            total_insertions: AtomicU64::new(0),
            total_evictions: AtomicU64::new(0),
            expired_evictions: AtomicU64::new(0),
            size_evictions: AtomicU64::new(0),
        }
    }

    pub fn push(&mut self, item: T) {
        self.total_insertions.fetch_add(1, Ordering::Relaxed);
        
        // Remove expired items first
        self.evict_expired();
        
        // If at capacity, remove oldest item
        if self.data.len() >= self.max_size {
            self.data.pop_front();
            self.size_evictions.fetch_add(1, Ordering::Relaxed);
            self.total_evictions.fetch_add(1, Ordering::Relaxed);
        }
        
        // Add new item
        let ttl_event = TtlEvent::new(item, self.ttl);
        self.data.push_back(ttl_event);
    }

    pub fn get_valid_items(&mut self) -> Vec<&T> {
        self.evict_expired();
        self.data.iter_mut().map(|event| event.access()).collect()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn evict_expired(&mut self) {
        let original_len = self.data.len();
        self.data.retain(|event| !event.is_expired());
        let evicted = original_len - self.data.len();
        
        if evicted > 0 {
            self.expired_evictions.fetch_add(evicted as u64, Ordering::Relaxed);
            self.total_evictions.fetch_add(evicted as u64, Ordering::Relaxed);
        }
    }

    pub fn stats(&self) -> BoundedCollectionStats {
        BoundedCollectionStats {
            current_size: self.data.len(),
            max_size: self.max_size,
            total_insertions: self.total_insertions.load(Ordering::Relaxed),
            total_evictions: self.total_evictions.load(Ordering::Relaxed),
            expired_evictions: self.expired_evictions.load(Ordering::Relaxed),
            size_evictions: self.size_evictions.load(Ordering::Relaxed),
            utilization_percent: (self.data.len() as f64 / self.max_size as f64) * 100.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundedCollectionStats {
    pub current_size: usize,
    pub max_size: usize,
    pub total_insertions: u64,
    pub total_evictions: u64,
    pub expired_evictions: u64,
    pub size_evictions: u64,
    pub utilization_percent: f64,
}

/// Main memory protection coordinator
#[derive(Debug)]
pub struct MemoryProtectionSystem {
    config: MemoryProtectionConfig,
    memory_monitor: Arc<memory_monitor::MemoryMonitor>,
    circuit_breakers: Arc<RwLock<HashMap<String, circuit_breakers::MemoryCircuitBreaker>>>,
    resource_limiter: Arc<resource_limiter::ResourceLimiter>,
    alert_history: Arc<Mutex<VecDeque<MemoryAlert>>>,
    protection_active: AtomicBool,
    last_gc_timestamp: AtomicU64,
    gc_count: AtomicU64,
}

impl MemoryProtectionSystem {
    pub fn new(config: MemoryProtectionConfig) -> Self {
        Self {
            memory_monitor: Arc::new(memory_monitor::MemoryMonitor::new(&config)),
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            resource_limiter: Arc::new(resource_limiter::ResourceLimiter::new(&config)),
            alert_history: Arc::new(Mutex::new(VecDeque::with_capacity(1000))),
            protection_active: AtomicBool::new(false),
            last_gc_timestamp: AtomicU64::new(0),
            gc_count: AtomicU64::new(0),
            config,
        }
    }

    /// Start the memory protection system
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting memory protection system");
        
        self.protection_active.store(true, Ordering::Relaxed);
        
        // Start memory monitoring
        self.memory_monitor.start().await?;
        
        // Start resource limiting
        self.resource_limiter.start().await?;
        
        // Start periodic cleanup task
        self.start_cleanup_task().await;
        
        info!("Memory protection system started successfully");
        Ok(())
    }

    /// Stop the memory protection system
    pub async fn stop(&self) {
        info!("Stopping memory protection system");
        
        self.protection_active.store(false, Ordering::Relaxed);
        self.memory_monitor.stop().await;
        self.resource_limiter.stop().await;
        
        info!("Memory protection system stopped");
    }

    /// Check if system is under memory pressure
    pub async fn is_under_memory_pressure(&self) -> bool {
        let stats = self.memory_monitor.get_stats().await;
        stats.memory_usage_percent > self.config.memory_alert_threshold_percent
    }

    /// Trigger emergency memory cleanup
    pub async fn emergency_cleanup(&self) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        warn!("Triggering emergency memory cleanup");
        
        let mut cleanup_count = 0;
        
        // Trigger garbage collection
        if self.config.enable_auto_gc {
            self.trigger_gc().await;
            cleanup_count += 1;
        }
        
        // Trip all circuit breakers temporarily
        {
            let mut breakers = self.circuit_breakers.write().await;
            for (name, breaker) in breakers.iter_mut() {
                if !breaker.is_open() {
                    breaker.trip().await;
                    warn!("Tripped circuit breaker: {}", name);
                    cleanup_count += 1;
                }
            }
        }
        
        // Alert about emergency cleanup
        let alert = MemoryAlert {
            alert_type: MemoryAlertType::MemoryLeak,
            message: format!("Emergency cleanup performed, {} actions taken", cleanup_count),
            current_usage_bytes: self.memory_monitor.get_current_usage().await,
            threshold_bytes: self.config.max_memory_bytes as u64,
            severity: AlertSeverity::Emergency,
            timestamp: Utc::now(),
            recovery_suggestions: vec![
                "Check for memory leaks in event processing".to_string(),
                "Reduce batch sizes".to_string(),
                "Increase memory limits if needed".to_string(),
                "Review TTL settings".to_string(),
            ],
        };
        
        self.add_alert(alert).await;
        
        Ok(cleanup_count)
    }

    /// Get or create a circuit breaker for a component
    pub async fn get_circuit_breaker(&self, component: &str) -> Arc<circuit_breakers::MemoryCircuitBreaker> {
        let mut breakers = self.circuit_breakers.write().await;
        if let Some(breaker) = breakers.get(component) {
            Arc::new(breaker.clone())
        } else {
            let breaker = circuit_breakers::MemoryCircuitBreaker::new(
                component.to_string(),
                self.config.circuit_breaker_failure_threshold,
                std::time::Duration::from_secs(self.config.circuit_breaker_timeout_seconds),
            );
            let breaker_arc = Arc::new(breaker.clone());
            breakers.insert(component.to_string(), breaker);
            breaker_arc
        }
    }

    /// Add an alert to the history
    pub async fn add_alert(&self, alert: MemoryAlert) {
        let mut history = self.alert_history.lock().await;
        
        // Log the alert
        match alert.severity {
            AlertSeverity::Info => info!("{}", alert.message),
            AlertSeverity::Warning => warn!("{}", alert.message),
            AlertSeverity::Critical => error!("{}", alert.message),
            AlertSeverity::Emergency => error!("EMERGENCY: {}", alert.message),
        }
        
        // Keep only the last 1000 alerts
        if history.len() >= 1000 {
            history.pop_front();
        }
        
        history.push_back(alert);
    }

    /// Get recent alerts
    pub async fn get_recent_alerts(&self, limit: usize) -> Vec<MemoryAlert> {
        let history = self.alert_history.lock().await;
        history.iter().rev().take(limit).cloned().collect()
    }

    /// Get memory statistics
    pub async fn get_memory_stats(&self) -> MemoryStats {
        self.memory_monitor.get_stats().await
    }

    /// Trigger garbage collection
    async fn trigger_gc(&self) {
        let start = std::time::Instant::now();
        
        // Force a garbage collection (platform-specific)
        #[cfg(feature = "jemalloc")]
        {
            use tikv_jemalloc_ctl::{epoch, stats};
            epoch::advance().unwrap();
        }
        
        // For standard allocator, there's no direct GC trigger
        // But we can still track the attempt
        let duration = start.elapsed();
        
        self.last_gc_timestamp.store(
            Utc::now().timestamp() as u64,
            Ordering::Relaxed
        );
        self.gc_count.fetch_add(1, Ordering::Relaxed);
        
        debug!("Garbage collection completed in {:?}", duration);
    }

    /// Start the periodic cleanup task
    async fn start_cleanup_task(&self) {
        let protection_system = self.clone_for_task();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_millis(protection_system.config.memory_check_interval_ms)
            );
            
            while protection_system.protection_active.load(Ordering::Relaxed) {
                interval.tick().await;
                
                if let Err(e) = protection_system.periodic_cleanup().await {
                    error!("Error in periodic cleanup: {}", e);
                }
            }
        });
    }

    /// Perform periodic cleanup and monitoring
    async fn periodic_cleanup(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let stats = self.memory_monitor.get_stats().await;
        
        // Check if we're approaching memory limits
        if stats.memory_usage_percent > self.config.memory_alert_threshold_percent {
            let alert = MemoryAlert {
                alert_type: MemoryAlertType::HighMemoryUsage,
                message: format!(
                    "High memory usage detected: {:.1}% ({} MB used)",
                    stats.memory_usage_percent,
                    stats.total_allocated_bytes / 1024 / 1024
                ),
                current_usage_bytes: stats.total_allocated_bytes,
                threshold_bytes: (self.config.max_memory_bytes as f64 * self.config.memory_alert_threshold_percent) as u64,
                severity: if stats.memory_usage_percent > 0.95 {
                    AlertSeverity::Emergency
                } else if stats.memory_usage_percent > 0.9 {
                    AlertSeverity::Critical
                } else {
                    AlertSeverity::Warning
                },
                timestamp: Utc::now(),
                recovery_suggestions: vec![
                    "Check for event bus memory leaks".to_string(),
                    "Reduce event TTL settings".to_string(),
                    "Implement more aggressive eviction policies".to_string(),
                ],
            };
            
            self.add_alert(alert).await;
            
            // If memory usage is critical, trigger emergency cleanup
            if stats.memory_usage_percent > 0.95 {
                self.emergency_cleanup().await?;
            }
        }
        
        // Periodic GC if enabled
        if self.config.enable_auto_gc {
            let last_gc = self.last_gc_timestamp.load(Ordering::Relaxed);
            let now = Utc::now().timestamp() as u64;
            
            // Trigger GC every 5 minutes or if memory usage is high
            if now - last_gc > 300 || stats.memory_usage_percent > 0.8 {
                self.trigger_gc().await;
            }
        }
        
        Ok(())
    }

    fn clone_for_task(&self) -> Self {
        Self {
            config: self.config.clone(),
            memory_monitor: Arc::clone(&self.memory_monitor),
            circuit_breakers: Arc::clone(&self.circuit_breakers),
            resource_limiter: Arc::clone(&self.resource_limiter),
            alert_history: Arc::clone(&self.alert_history),
            protection_active: AtomicBool::new(self.protection_active.load(Ordering::Relaxed)),
            last_gc_timestamp: AtomicU64::new(self.last_gc_timestamp.load(Ordering::Relaxed)),
            gc_count: AtomicU64::new(self.gc_count.load(Ordering::Relaxed)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration as TokioDuration};

    #[test]
    fn test_ttl_event_expiration() {
        let ttl = Duration::seconds(1);
        let event = TtlEvent::new("test_data", ttl);
        
        // Should not be expired immediately
        assert!(!event.is_expired());
        
        // Should be expired after TTL
        std::thread::sleep(std::time::Duration::from_secs(2));
        assert!(event.is_expired());
    }

    #[test]
    fn test_bounded_collection_size_limit() {
        let mut collection = BoundedTtlCollection::new(3, 3600);
        
        // Add more items than the limit
        for i in 0..5 {
            collection.push(format!("item_{}", i));
        }
        
        // Should only keep the maximum number
        assert_eq!(collection.len(), 3);
        
        // Should have evicted 2 items due to size
        let stats = collection.stats();
        assert_eq!(stats.size_evictions, 2);
    }

    #[tokio::test]
    async fn test_memory_protection_system_alerts() {
        let mut config = MemoryProtectionConfig::default();
        config.memory_alert_threshold_percent = 0.1; // Very low threshold for testing
        
        let system = MemoryProtectionSystem::new(config);
        
        let alert = MemoryAlert {
            alert_type: MemoryAlertType::HighMemoryUsage,
            message: "Test alert".to_string(),
            current_usage_bytes: 1000,
            threshold_bytes: 800,
            severity: AlertSeverity::Warning,
            timestamp: Utc::now(),
            recovery_suggestions: vec!["Test suggestion".to_string()],
        };
        
        system.add_alert(alert).await;
        
        let recent_alerts = system.get_recent_alerts(10).await;
        assert_eq!(recent_alerts.len(), 1);
        assert_eq!(recent_alerts[0].message, "Test alert");
    }
}