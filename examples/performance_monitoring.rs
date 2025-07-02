//! Performance Monitoring Example
//!
//! This example demonstrates how to implement comprehensive performance monitoring
//! for the Neural Trader Autonomous Platform, including metrics collection,
//! real-time dashboards, alerting, and performance optimization techniques.

use autonomous_platform::{
    PlatformConfig, load_default_config, Result,
    data::{QualityMetrics, PlatformMetrics, TimeSeriesData},
};
use chrono::{Utc, Duration};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use tracing::{info, warn, error, debug};
use tokio::time::{interval, Duration as TokioDuration};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting Performance Monitoring Example");
    info!("=======================================");

    // Step 1: Initialize monitoring system
    let config = load_default_config()?;
    let monitor = Arc::new(PerformanceMonitor::new(config).await?);
    
    info!("✓ Performance monitor initialized");

    // Step 2: Start background monitoring tasks
    let monitor_clone = Arc::clone(&monitor);
    let metrics_task = tokio::spawn(async move {
        monitor_clone.start_metrics_collection().await
    });

    let monitor_clone = Arc::clone(&monitor);
    let alert_task = tokio::spawn(async move {
        monitor_clone.start_alert_system().await
    });

    let monitor_clone = Arc::clone(&monitor);
    let dashboard_task = tokio::spawn(async move {
        monitor_clone.start_dashboard().await
    });

    // Step 3: Simulate platform operations
    let monitor_clone = Arc::clone(&monitor);
    let simulation_task = tokio::spawn(async move {
        simulate_platform_operations(monitor_clone).await
    });

    // Step 4: Run monitoring for a specified duration
    info!("Running monitoring simulation for 60 seconds...");
    tokio::time::sleep(TokioDuration::from_secs(60)).await;

    // Step 5: Generate final performance report
    monitor.generate_performance_report().await?;

    // Cleanup tasks
    metrics_task.abort();
    alert_task.abort();
    dashboard_task.abort();
    simulation_task.abort();

    info!("Performance monitoring example completed");
    Ok(())
}

/// Main performance monitoring system
struct PerformanceMonitor {
    config: PlatformConfig,
    metrics_history: Arc<Mutex<VecDeque<PlatformMetrics>>>,
    quality_history: Arc<Mutex<VecDeque<QualityMetrics>>>,
    alerts: Arc<Mutex<Vec<Alert>>>,
    start_time: chrono::DateTime<Utc>,
    system_stats: Arc<Mutex<SystemStats>>,
}

impl PerformanceMonitor {
    async fn new(config: PlatformConfig) -> Result<Self> {
        Ok(Self {
            config,
            metrics_history: Arc::new(Mutex::new(VecDeque::with_capacity(1000))),
            quality_history: Arc::new(Mutex::new(VecDeque::with_capacity(1000))),
            alerts: Arc::new(Mutex::new(Vec::new())),
            start_time: Utc::now(),
            system_stats: Arc::new(Mutex::new(SystemStats::new())),
        })
    }

    async fn start_metrics_collection(&self) -> Result<()> {
        let mut interval = interval(TokioDuration::from_secs(self.config.monitoring.metrics_interval_secs));
        
        loop {
            interval.tick().await;
            
            // Collect platform metrics
            let platform_metrics = self.collect_platform_metrics().await?;
            {
                let mut history = self.metrics_history.lock().unwrap();
                history.push_back(platform_metrics.clone());
                if history.len() > 1000 {
                    history.pop_front();
                }
            }

            // Collect quality metrics
            let quality_metrics = self.collect_quality_metrics().await?;
            {
                let mut history = self.quality_history.lock().unwrap();
                history.push_back(quality_metrics.clone());
                if history.len() > 1000 {
                    history.pop_front();
                }
            }

            debug!("Metrics collected - Platform: {:.1}% cache hit rate, Quality: {:.1}% overall",
                   platform_metrics.cache_hit_rate * 100.0,
                   quality_metrics.overall_quality * 100.0);
        }
    }

    async fn start_alert_system(&self) -> Result<()> {
        let mut interval = interval(TokioDuration::from_secs(5));
        
        loop {
            interval.tick().await;
            self.check_alert_conditions().await?;
        }
    }

    async fn start_dashboard(&self) -> Result<()> {
        let mut interval = interval(TokioDuration::from_secs(10));
        
        loop {
            interval.tick().await;
            self.display_dashboard().await?;
        }
    }

    async fn collect_platform_metrics(&self) -> Result<PlatformMetrics> {
        // Simulate metrics collection from various system components
        let mut stats = self.system_stats.lock().unwrap();
        
        // Update simulated stats
        stats.total_records += rand::random::<u64>() % 1000 + 100;
        stats.cache_hit_rate = 0.7 + (rand::random::<f64>() * 0.3); // 70-100%
        stats.processing_throughput = 1000.0 + (rand::random::<f64>() * 4000.0); // 1000-5000 rec/sec
        stats.storage_usage_gb += (rand::random::<f64>() - 0.5) * 0.1; // Slowly growing
        stats.active_connections = 10 + (rand::random::<u32>() % 20); // 10-30 connections
        
        Ok(PlatformMetrics::new(
            stats.total_records,
            stats.cache_hit_rate,
            stats.processing_throughput,
            stats.storage_usage_gb.max(0.0),
            stats.active_connections,
        ))
    }

    async fn collect_quality_metrics(&self) -> Result<QualityMetrics> {
        // Simulate quality metrics from data pipeline
        let data_completeness = 0.85 + (rand::random::<f64>() * 0.15); // 85-100%
        let latency_ms = 50.0 + (rand::random::<f64>() * 200.0); // 50-250ms
        let error_rate = rand::random::<f64>() * 0.05; // 0-5%
        
        Ok(QualityMetrics::new(data_completeness, latency_ms, error_rate))
    }

    async fn check_alert_conditions(&self) -> Result<()> {
        let latest_quality = {
            let history = self.quality_history.lock().unwrap();
            history.back().cloned()
        };

        let latest_platform = {
            let history = self.metrics_history.lock().unwrap();
            history.back().cloned()
        };

        if let (Some(quality), Some(platform)) = (latest_quality, latest_platform) {
            let mut alerts = self.alerts.lock().unwrap();
            
            // Check quality threshold
            if quality.overall_quality < self.config.monitoring.quality_threshold {
                let alert = Alert::new(
                    AlertLevel::Warning,
                    format!("Quality below threshold: {:.2}% < {:.2}%", 
                           quality.overall_quality * 100.0,
                           self.config.monitoring.quality_threshold * 100.0),
                );
                alerts.push(alert.clone());
                warn!("🚨 {}", alert.message);
            }

            // Check high latency
            if quality.latency_ms > 500.0 {
                let alert = Alert::new(
                    AlertLevel::Critical,
                    format!("High latency detected: {:.0}ms", quality.latency_ms),
                );
                alerts.push(alert.clone());
                error!("🚨 {}", alert.message);
            }

            // Check cache hit rate
            if platform.cache_hit_rate < 0.5 {
                let alert = Alert::new(
                    AlertLevel::Warning,
                    format!("Low cache hit rate: {:.1}%", platform.cache_hit_rate * 100.0),
                );
                alerts.push(alert.clone());
                warn!("🚨 {}", alert.message);
            }

            // Check storage usage
            if platform.storage_usage_gb > 10.0 {
                let alert = Alert::new(
                    AlertLevel::Info,
                    format!("High storage usage: {:.1} GB", platform.storage_usage_gb),
                );
                alerts.push(alert.clone());
                info!("ℹ️ {}", alert.message);
            }

            // Keep only recent alerts (last 100)
            if alerts.len() > 100 {
                alerts.drain(0..alerts.len() - 100);
            }
        }

        Ok(())
    }

    async fn display_dashboard(&self) -> Result<()> {
        let (latest_quality, latest_platform) = {
            let quality_history = self.quality_history.lock().unwrap();
            let platform_history = self.metrics_history.lock().unwrap();
            (quality_history.back().cloned(), platform_history.back().cloned())
        };

        if let (Some(quality), Some(platform)) = (latest_quality, latest_platform) {
            let uptime = Utc::now() - self.start_time;
            let recent_alerts = {
                let alerts = self.alerts.lock().unwrap();
                alerts.iter().rev().take(3).cloned().collect::<Vec<_>>()
            };

            info!("📊 PERFORMANCE DASHBOARD");
            info!("========================");
            info!("⏱️  Uptime: {} minutes", uptime.num_minutes());
            info!("📈 Processing: {:.0} records/sec", platform.processing_throughput);
            info!("💾 Storage: {:.1} GB", platform.storage_usage_gb);
            info!("🔗 Connections: {}", platform.active_connections);
            info!("⚡ Cache Hit Rate: {:.1}%", platform.cache_hit_rate * 100.0);
            info!("📊 Data Quality: {:.1}%", quality.overall_quality * 100.0);
            info!("⏰ Latency: {:.0}ms", quality.latency_ms);
            info!("❌ Error Rate: {:.2}%", quality.error_rate * 100.0);
            
            if !recent_alerts.is_empty() {
                info!("🚨 Recent Alerts:");
                for alert in recent_alerts {
                    let age = Utc::now() - alert.timestamp;
                    info!("   {} - {} ({}m ago)", 
                          alert.level.emoji(), 
                          alert.message,
                          age.num_minutes());
                }
            }
            info!("========================");
        }

        Ok(())
    }

    async fn generate_performance_report(&self) -> Result<()> {
        let uptime = Utc::now() - self.start_time;
        
        let (avg_quality, avg_latency, avg_throughput, avg_cache_hit) = {
            let quality_history = self.quality_history.lock().unwrap();
            let platform_history = self.metrics_history.lock().unwrap();
            
            let quality_avg = if !quality_history.is_empty() {
                quality_history.iter().map(|q| q.overall_quality).sum::<f64>() / quality_history.len() as f64
            } else { 0.0 };
            
            let latency_avg = if !quality_history.is_empty() {
                quality_history.iter().map(|q| q.latency_ms).sum::<f64>() / quality_history.len() as f64
            } else { 0.0 };
            
            let throughput_avg = if !platform_history.is_empty() {
                platform_history.iter().map(|p| p.processing_throughput).sum::<f64>() / platform_history.len() as f64
            } else { 0.0 };
            
            let cache_avg = if !platform_history.is_empty() {
                platform_history.iter().map(|p| p.cache_hit_rate).sum::<f64>() / platform_history.len() as f64
            } else { 0.0 };
            
            (quality_avg, latency_avg, throughput_avg, cache_avg)
        };

        let alert_counts = {
            let alerts = self.alerts.lock().unwrap();
            let mut counts = HashMap::new();
            for alert in alerts.iter() {
                *counts.entry(alert.level.clone()).or_insert(0) += 1;
            }
            counts
        };

        info!("📋 FINAL PERFORMANCE REPORT");
        info!("===========================");
        info!("⏱️  Total uptime: {} minutes", uptime.num_minutes());
        info!("📊 Average data quality: {:.1}%", avg_quality * 100.0);
        info!("⏰ Average latency: {:.0}ms", avg_latency);
        info!("📈 Average throughput: {:.0} records/sec", avg_throughput);
        info!("⚡ Average cache hit rate: {:.1}%", avg_cache_hit * 100.0);
        info!("");
        info!("🚨 Alert Summary:");
        for (level, count) in alert_counts {
            info!("   {} {}: {}", level.emoji(), level, count);
        }
        info!("");
        
        // Performance recommendations
        info!("💡 Performance Recommendations:");
        if avg_cache_hit < 0.8 {
            info!("   • Consider increasing cache size or optimizing cache keys");
        }
        if avg_latency > 200.0 {
            info!("   • Investigate high latency sources and optimize data pipeline");
        }
        if avg_quality < 0.9 {
            info!("   • Review data quality checks and improve error handling");
        }
        if avg_throughput < 2000.0 {
            info!("   • Consider scaling processing workers or optimizing algorithms");
        }
        info!("===========================");

        Ok(())
    }
}

/// Simulate platform operations that generate metrics
async fn simulate_platform_operations(monitor: Arc<PerformanceMonitor>) -> Result<()> {
    let mut interval = interval(TokioDuration::from_millis(500));
    
    loop {
        interval.tick().await;
        
        // Simulate various operations that affect performance
        simulate_data_processing().await?;
        simulate_cache_operations().await?;
        simulate_database_operations().await?;
        
        // Occasionally simulate performance issues
        if rand::random::<f64>() < 0.05 { // 5% chance
            simulate_performance_issue().await?;
        }
    }
}

async fn simulate_data_processing() -> Result<()> {
    // Simulate CPU-intensive data processing
    let _work = (0..1000).map(|i| i * i).collect::<Vec<_>>();
    tokio::time::sleep(TokioDuration::from_millis(1)).await;
    Ok(())
}

async fn simulate_cache_operations() -> Result<()> {
    // Simulate cache read/write operations
    tokio::time::sleep(TokioDuration::from_millis(2)).await;
    Ok(())
}

async fn simulate_database_operations() -> Result<()> {
    // Simulate database queries
    tokio::time::sleep(TokioDuration::from_millis(5)).await;
    Ok(())
}

async fn simulate_performance_issue() -> Result<()> {
    // Simulate occasional performance spikes
    tokio::time::sleep(TokioDuration::from_millis(100)).await;
    Ok(())
}

/// System statistics tracking
#[derive(Debug, Clone)]
struct SystemStats {
    total_records: u64,
    cache_hit_rate: f64,
    processing_throughput: f64,
    storage_usage_gb: f64,
    active_connections: u32,
}

impl SystemStats {
    fn new() -> Self {
        Self {
            total_records: 0,
            cache_hit_rate: 0.8,
            processing_throughput: 2500.0,
            storage_usage_gb: 1.0,
            active_connections: 15,
        }
    }
}

/// Alert system
#[derive(Debug, Clone)]
struct Alert {
    level: AlertLevel,
    message: String,
    timestamp: chrono::DateTime<Utc>,
}

impl Alert {
    fn new(level: AlertLevel, message: String) -> Self {
        Self {
            level,
            message,
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum AlertLevel {
    Info,
    Warning,
    Critical,
}

impl AlertLevel {
    fn emoji(&self) -> &'static str {
        match self {
            AlertLevel::Info => "ℹ️",
            AlertLevel::Warning => "⚠️",
            AlertLevel::Critical => "🚨",
        }
    }
}

impl std::fmt::Display for AlertLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertLevel::Info => write!(f, "INFO"),
            AlertLevel::Warning => write!(f, "WARNING"),
            AlertLevel::Critical => write!(f, "CRITICAL"),
        }
    }
}