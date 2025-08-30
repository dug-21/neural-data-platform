//! Memory monitoring and usage tracking
//! 
//! Provides real-time memory monitoring capabilities with platform-specific
//! optimizations for accurate memory tracking.

use crate::memory_protection::{MemoryStats, MemoryProtectionConfig};
use chrono::Utc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

#[cfg(target_os = "linux")]
use std::fs;

#[cfg(target_os = "macos")]
use std::process::Command;

#[cfg(windows)]
use std::mem;

#[cfg(windows)]
use winapi::um::psapi::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
#[cfg(windows)]
use winapi::um::processthreadsapi::GetCurrentProcess;

/// Memory monitoring system
#[derive(Debug)]
pub struct MemoryMonitor {
    config: MemoryProtectionConfig,
    current_usage_bytes: AtomicU64,
    peak_usage_bytes: AtomicU64,
    gc_count: AtomicU64,
    last_gc_duration_ms: AtomicU64,
    monitoring_active: AtomicBool,
    sample_count: AtomicU64,
    total_samples: AtomicU64,
}

impl MemoryMonitor {
    pub fn new(config: &MemoryProtectionConfig) -> Self {
        Self {
            config: config.clone(),
            current_usage_bytes: AtomicU64::new(0),
            peak_usage_bytes: AtomicU64::new(0),
            gc_count: AtomicU64::new(0),
            last_gc_duration_ms: AtomicU64::new(0),
            monitoring_active: AtomicBool::new(false),
            sample_count: AtomicU64::new(0),
            total_samples: AtomicU64::new(0),
        }
    }

    /// Start memory monitoring
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.monitoring_active.load(Ordering::Relaxed) {
            warn!("Memory monitor already running");
            return Ok(());
        }

        info!("Starting memory monitor");
        self.monitoring_active.store(true, Ordering::Relaxed);

        // Initial memory reading
        let initial_usage = self.get_current_memory_usage().await?;
        self.current_usage_bytes.store(initial_usage, Ordering::Relaxed);
        self.peak_usage_bytes.store(initial_usage, Ordering::Relaxed);

        // Start monitoring task
        let monitor = self.clone_for_task();
        tokio::spawn(async move {
            monitor.monitoring_loop().await;
        });

        info!("Memory monitor started");
        Ok(())
    }

    /// Stop memory monitoring
    pub async fn stop(&self) {
        info!("Stopping memory monitor");
        self.monitoring_active.store(false, Ordering::Relaxed);
    }

    /// Get current memory statistics
    pub async fn get_stats(&self) -> MemoryStats {
        let current_usage = self.current_usage_bytes.load(Ordering::Relaxed);
        let available_memory = self.get_available_memory().await.unwrap_or(0);
        
        MemoryStats {
            total_allocated_bytes: current_usage,
            heap_size_bytes: self.get_heap_size().await.unwrap_or(current_usage),
            rss_bytes: current_usage,
            available_memory_bytes: available_memory,
            memory_usage_percent: if available_memory > 0 {
                (current_usage as f64 / (current_usage + available_memory) as f64) * 100.0
            } else {
                (current_usage as f64 / self.config.max_memory_bytes as f64) * 100.0
            },
            gc_count: self.gc_count.load(Ordering::Relaxed),
            last_gc_duration_ms: self.last_gc_duration_ms.load(Ordering::Relaxed),
            timestamp: Utc::now(),
        }
    }

    /// Get current memory usage in bytes
    pub async fn get_current_usage(&self) -> u64 {
        self.current_usage_bytes.load(Ordering::Relaxed)
    }

    /// Get peak memory usage since startup
    pub async fn get_peak_usage(&self) -> u64 {
        self.peak_usage_bytes.load(Ordering::Relaxed)
    }

    /// Record a garbage collection event
    pub fn record_gc(&self, duration_ms: u64) {
        self.gc_count.fetch_add(1, Ordering::Relaxed);
        self.last_gc_duration_ms.store(duration_ms, Ordering::Relaxed);
        debug!("Recorded GC event: {}ms", duration_ms);
    }

    /// Check if memory usage exceeds threshold
    pub async fn is_memory_threshold_exceeded(&self, threshold_percent: f64) -> bool {
        let stats = self.get_stats().await;
        stats.memory_usage_percent > threshold_percent
    }

    /// Main monitoring loop
    async fn monitoring_loop(&self) {
        let mut interval = interval(Duration::from_millis(self.config.memory_check_interval_ms));

        while self.monitoring_active.load(Ordering::Relaxed) {
            interval.tick().await;

            match self.update_memory_stats().await {
                Ok(_) => {
                    self.total_samples.fetch_add(1, Ordering::Relaxed);
                },
                Err(e) => {
                    error!("Failed to update memory stats: {}", e);
                }
            }
        }

        debug!("Memory monitoring loop exited");
    }

    /// Update memory statistics
    async fn update_memory_stats(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let current_usage = self.get_current_memory_usage().await?;
        
        // Update current usage
        self.current_usage_bytes.store(current_usage, Ordering::Relaxed);
        
        // Update peak if necessary
        let current_peak = self.peak_usage_bytes.load(Ordering::Relaxed);
        if current_usage > current_peak {
            self.peak_usage_bytes.store(current_usage, Ordering::Relaxed);
            debug!("New peak memory usage: {} MB", current_usage / 1024 / 1024);
        }

        self.sample_count.fetch_add(1, Ordering::Relaxed);

        // Log every 100 samples at debug level
        if self.sample_count.load(Ordering::Relaxed) % 100 == 0 {
            let usage_mb = current_usage / 1024 / 1024;
            let peak_mb = self.peak_usage_bytes.load(Ordering::Relaxed) / 1024 / 1024;
            debug!("Memory usage: {} MB (peak: {} MB)", usage_mb, peak_mb);
        }

        Ok(())
    }

    /// Get current memory usage (platform-specific implementation)
    async fn get_current_memory_usage(&self) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        #[cfg(target_os = "linux")]
        {
            self.get_memory_usage_linux().await
        }

        #[cfg(target_os = "macos")]
        {
            self.get_memory_usage_macos().await
        }

        #[cfg(windows)]
        {
            self.get_memory_usage_windows().await
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            // Fallback for other platforms
            self.get_memory_usage_fallback().await
        }
    }

    /// Get available memory (platform-specific)
    async fn get_available_memory(&self) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        #[cfg(target_os = "linux")]
        {
            self.get_available_memory_linux().await
        }

        #[cfg(target_os = "macos")]
        {
            self.get_available_memory_macos().await
        }

        #[cfg(windows)]
        {
            self.get_available_memory_windows().await
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            Ok(self.config.max_memory_bytes as u64)
        }
    }

    /// Get heap size estimate
    async fn get_heap_size(&self) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        // For Rust, heap size is not directly available
        // We'll estimate it as current memory usage
        Ok(self.current_usage_bytes.load(Ordering::Relaxed))
    }

    #[cfg(target_os = "linux")]
    async fn get_memory_usage_linux(&self) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        let status = fs::read_to_string("/proc/self/status")?;
        
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let kb: u64 = parts[1].parse()?;
                    return Ok(kb * 1024); // Convert KB to bytes
                }
            }
        }
        
        Err("Could not parse VmRSS from /proc/self/status".into())
    }

    #[cfg(target_os = "linux")]
    async fn get_available_memory_linux(&self) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        let meminfo = fs::read_to_string("/proc/meminfo")?;
        
        for line in meminfo.lines() {
            if line.starts_with("MemAvailable:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let kb: u64 = parts[1].parse()?;
                    return Ok(kb * 1024); // Convert KB to bytes
                }
            }
        }
        
        Err("Could not parse MemAvailable from /proc/meminfo".into())
    }

    #[cfg(target_os = "macos")]
    async fn get_memory_usage_macos(&self) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        // Use `ps` command to get RSS
        let output = Command::new("ps")
            .args(&["-o", "rss=", "-p"])
            .arg(std::process::id().to_string())
            .output()?;
        
        let rss_str = String::from_utf8(output.stdout)?;
        let rss_kb: u64 = rss_str.trim().parse()?;
        Ok(rss_kb * 1024) // Convert KB to bytes
    }

    #[cfg(target_os = "macos")]
    async fn get_available_memory_macos(&self) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        // Use `vm_stat` to get memory statistics
        let output = Command::new("vm_stat")
            .output()?;
        
        let vm_stat = String::from_utf8(output.stdout)?;
        let mut free_pages = 0u64;
        let mut page_size = 4096u64; // Default page size
        
        for line in vm_stat.lines() {
            if line.starts_with("Pages free:") {
                if let Some(count_str) = line.split_whitespace().nth(2) {
                    free_pages = count_str.trim_end_matches('.').parse().unwrap_or(0);
                }
            } else if line.starts_with("Mach Virtual Memory Statistics:") {
                // Extract page size if available
                if line.contains("page size of ") {
                    // Parse page size from the line
                    // This is a simplified approach
                }
            }
        }
        
        Ok(free_pages * page_size)
    }

    #[cfg(windows)]
    async fn get_memory_usage_windows(&self) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        unsafe {
            let mut pmc: PROCESS_MEMORY_COUNTERS = mem::zeroed();
            pmc.cb = mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
            
            let result = GetProcessMemoryInfo(
                GetCurrentProcess(),
                &mut pmc,
                pmc.cb
            );
            
            if result != 0 {
                Ok(pmc.WorkingSetSize as u64)
            } else {
                Err("Failed to get process memory info".into())
            }
        }
    }

    #[cfg(windows)]
    async fn get_available_memory_windows(&self) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        // Windows implementation would use GlobalMemoryStatusEx
        // For now, return a reasonable estimate
        Ok(self.config.max_memory_bytes as u64)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    async fn get_memory_usage_fallback(&self) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        // Fallback implementation - estimate based on heap allocator if available
        #[cfg(feature = "jemalloc")]
        {
            use tikv_jemalloc_ctl::{epoch, stats};
            epoch::advance().unwrap();
            Ok(stats::allocated::read().unwrap())
        }
        
        #[cfg(not(feature = "jemalloc"))]
        {
            // Very basic estimation - not accurate but prevents crashes
            // In a real implementation, you might want to track allocations manually
            Ok(self.config.max_memory_bytes as u64 / 4) // Assume 25% usage
        }
    }

    fn clone_for_task(&self) -> Self {
        Self {
            config: self.config.clone(),
            current_usage_bytes: AtomicU64::new(self.current_usage_bytes.load(Ordering::Relaxed)),
            peak_usage_bytes: AtomicU64::new(self.peak_usage_bytes.load(Ordering::Relaxed)),
            gc_count: AtomicU64::new(self.gc_count.load(Ordering::Relaxed)),
            last_gc_duration_ms: AtomicU64::new(self.last_gc_duration_ms.load(Ordering::Relaxed)),
            monitoring_active: AtomicBool::new(self.monitoring_active.load(Ordering::Relaxed)),
            sample_count: AtomicU64::new(0),
            total_samples: AtomicU64::new(self.total_samples.load(Ordering::Relaxed)),
        }
    }
}

/// Memory usage sampling for detailed analysis
pub struct MemoryUsageSampler {
    samples: std::collections::VecDeque<MemoryUsageSample>,
    max_samples: usize,
}

#[derive(Debug, Clone)]
pub struct MemoryUsageSample {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub rss_bytes: u64,
    pub heap_bytes: u64,
    pub available_bytes: u64,
    pub gc_count: u64,
}

impl MemoryUsageSampler {
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: std::collections::VecDeque::with_capacity(max_samples),
            max_samples,
        }
    }

    pub fn add_sample(&mut self, sample: MemoryUsageSample) {
        if self.samples.len() >= self.max_samples {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    pub fn get_recent_samples(&self, count: usize) -> Vec<&MemoryUsageSample> {
        self.samples.iter().rev().take(count).collect()
    }

    pub fn detect_memory_leak(&self, threshold_mb: f64) -> bool {
        if self.samples.len() < 10 {
            return false;
        }

        // Check if memory usage has consistently increased over the last 10 samples
        let recent: Vec<_> = self.samples.iter().rev().take(10).collect();
        let oldest_mb = recent.last().unwrap().rss_bytes as f64 / 1024.0 / 1024.0;
        let newest_mb = recent.first().unwrap().rss_bytes as f64 / 1024.0 / 1024.0;

        newest_mb - oldest_mb > threshold_mb
    }

    pub fn get_memory_growth_rate_mb_per_minute(&self) -> f64 {
        if self.samples.len() < 2 {
            return 0.0;
        }

        let first = self.samples.front().unwrap();
        let last = self.samples.back().unwrap();

        let duration_minutes = (last.timestamp - first.timestamp).num_minutes() as f64;
        if duration_minutes <= 0.0 {
            return 0.0;
        }

        let memory_diff_mb = (last.rss_bytes as f64 - first.rss_bytes as f64) / 1024.0 / 1024.0;
        memory_diff_mb / duration_minutes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_protection::MemoryProtectionConfig;

    #[tokio::test]
    async fn test_memory_monitor_creation() {
        let config = MemoryProtectionConfig::default();
        let monitor = MemoryMonitor::new(&config);
        
        assert!(!monitor.monitoring_active.load(Ordering::Relaxed));
        assert_eq!(monitor.gc_count.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn test_memory_monitor_stats() {
        let config = MemoryProtectionConfig::default();
        let monitor = MemoryMonitor::new(&config);
        
        // Start monitoring
        monitor.start().await.unwrap();
        
        // Wait a bit for initial reading
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let stats = monitor.get_stats().await;
        assert!(stats.total_allocated_bytes > 0);
        
        monitor.stop().await;
    }

    #[test]
    fn test_memory_usage_sampler() {
        let mut sampler = MemoryUsageSampler::new(5);
        
        // Add samples
        for i in 0..3 {
            sampler.add_sample(MemoryUsageSample {
                timestamp: Utc::now(),
                rss_bytes: (100 + i * 10) * 1024 * 1024, // MB
                heap_bytes: (80 + i * 8) * 1024 * 1024,
                available_bytes: 1024 * 1024 * 1024,
                gc_count: i as u64,
            });
        }
        
        assert_eq!(sampler.samples.len(), 3);
        
        let recent = sampler.get_recent_samples(2);
        assert_eq!(recent.len(), 2);
        
        // Test memory leak detection (should not detect with only 3 samples)
        assert!(!sampler.detect_memory_leak(50.0));
    }
}