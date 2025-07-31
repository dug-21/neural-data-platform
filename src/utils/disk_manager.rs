/// Disk usage management for containerized environments
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tokio::fs;
use tokio::time::interval;
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskUsageConfig {
    /// Maximum disk usage in MB
    pub max_disk_usage_mb: u64,
    /// Warning threshold percentage
    pub warning_threshold: f32,
    /// Critical threshold percentage  
    pub critical_threshold: f32,
    /// Cleanup interval in seconds
    pub cleanup_interval_secs: u64,
    /// Temporary file TTL in seconds
    pub temp_file_ttl_secs: u64,
}

impl Default for DiskUsageConfig {
    fn default() -> Self {
        Self {
            max_disk_usage_mb: 1024, // 1 GB
            warning_threshold: 0.8,   // 80%
            critical_threshold: 0.95, // 95%
            cleanup_interval_secs: 300, // 5 minutes
            temp_file_ttl_secs: 3600,  // 1 hour
        }
    }
}

#[derive(Debug)]
pub struct DiskUsageStats {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub usage_percentage: f32,
}

pub struct DiskManager {
    config: DiskUsageConfig,
    monitored_paths: Vec<PathBuf>,
    temp_dir: PathBuf,
}

impl DiskManager {
    pub fn new(config: DiskUsageConfig) -> Self {
        Self {
            config,
            monitored_paths: vec![
                PathBuf::from("/tmp/neural_trader"),
                PathBuf::from("/var/log/neural_trader"),
                PathBuf::from("/app/cache"),
            ],
            temp_dir: PathBuf::from("/tmp/neural_trader"),
        }
    }

    /// Start the disk management background task
    pub async fn start(self) {
        let mut cleanup_interval = interval(Duration::from_secs(self.config.cleanup_interval_secs));
        
        loop {
            cleanup_interval.tick().await;
            
            // Check disk usage
            match self.get_disk_usage(&self.temp_dir).await {
                Ok(stats) => {
                    info!(
                        "Disk usage: {:.1}% ({} MB / {} MB)",
                        stats.usage_percentage * 100.0,
                        stats.used_bytes / 1_048_576,
                        stats.total_bytes / 1_048_576
                    );
                    
                    // Handle different usage levels
                    if stats.usage_percentage >= self.config.critical_threshold {
                        warn!("Critical disk usage detected!");
                        self.emergency_cleanup().await;
                    } else if stats.usage_percentage >= self.config.warning_threshold {
                        warn!("High disk usage detected");
                        self.aggressive_cleanup().await;
                    } else {
                        self.routine_cleanup().await;
                    }
                }
                Err(e) => error!("Failed to check disk usage: {}", e),
            }
        }
    }

    /// Get disk usage statistics for a path
    async fn get_disk_usage(&self, path: &Path) -> Result<DiskUsageStats, std::io::Error> {
        let metadata = fs::metadata(path).await?;
        
        // Calculate directory size recursively
        let used_bytes = self.calculate_directory_size(path).await?;
        
        // Simplified disk usage calculation (cross-platform)
        // Using configuration-based estimates instead of system calls
        let total_bytes = self.config.max_disk_usage_mb * 1_048_576;
        let available_bytes = total_bytes.saturating_sub(used_bytes);
        let usage_percentage = used_bytes as f32 / total_bytes as f32;
        
        Ok(DiskUsageStats {
            total_bytes,
            used_bytes,
            available_bytes,
            usage_percentage,
        })
    }

    /// Calculate total size of a directory
    async fn calculate_directory_size(&self, path: &Path) -> Result<u64, std::io::Error> {
        let mut total_size = 0u64;
        let mut entries = fs::read_dir(path).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let metadata = entry.metadata().await?;
            
            if metadata.is_file() {
                total_size += metadata.len();
            } else if metadata.is_dir() {
                // Recursively calculate subdirectory size
                if let Ok(subdir_size) = Box::pin(self.calculate_directory_size(&entry.path())).await {
                    total_size += subdir_size;
                }
            }
        }
        
        Ok(total_size)
    }

    /// Routine cleanup - remove old temporary files
    async fn routine_cleanup(&self) {
        info!("Performing routine disk cleanup");
        
        let ttl = Duration::from_secs(self.config.temp_file_ttl_secs);
        let now = SystemTime::now();
        
        for path in &self.monitored_paths {
            if let Err(e) = self.cleanup_old_files(path, ttl, now).await {
                error!("Failed to cleanup {}: {}", path.display(), e);
            }
        }
    }

    /// Aggressive cleanup - reduce TTL and remove caches
    async fn aggressive_cleanup(&self) {
        warn!("Performing aggressive disk cleanup");
        
        // Use shorter TTL for aggressive cleanup
        let ttl = Duration::from_secs(self.config.temp_file_ttl_secs / 4);
        let now = SystemTime::now();
        
        for path in &self.monitored_paths {
            if let Err(e) = self.cleanup_old_files(path, ttl, now).await {
                error!("Failed to cleanup {}: {}", path.display(), e);
            }
        }
        
        // Clear caches
        self.clear_caches().await;
    }

    /// Emergency cleanup - remove everything non-essential
    async fn emergency_cleanup(&self) {
        error!("Performing emergency disk cleanup!");
        
        // Remove all temporary files
        for path in &self.monitored_paths {
            if path.starts_with("/tmp") || path.to_string_lossy().contains("cache") {
                if let Err(e) = self.remove_directory_contents(path).await {
                    error!("Failed to clear {}: {}", path.display(), e);
                }
            }
        }
        
        // Truncate logs
        self.truncate_logs().await;
    }

    /// Remove files older than TTL
    async fn cleanup_old_files(
        &self,
        path: &Path,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<(), std::io::Error> {
        if !path.exists() {
            return Ok(());
        }
        
        let mut entries = fs::read_dir(path).await?;
        let mut removed_count = 0;
        let mut removed_bytes = 0u64;
        
        while let Some(entry) = entries.next_entry().await? {
            let metadata = entry.metadata().await?;
            
            if metadata.is_file() {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(age) = now.duration_since(modified) {
                        if age > ttl {
                            removed_bytes += metadata.len();
                            fs::remove_file(entry.path()).await?;
                            removed_count += 1;
                        }
                    }
                }
            }
        }
        
        if removed_count > 0 {
            info!(
                "Removed {} files ({} MB) from {}",
                removed_count,
                removed_bytes / 1_048_576,
                path.display()
            );
        }
        
        Ok(())
    }

    /// Remove all contents of a directory
    async fn remove_directory_contents(&self, path: &Path) -> Result<(), std::io::Error> {
        if !path.exists() {
            return Ok(());
        }
        
        let mut entries = fs::read_dir(path).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();
            
            if entry.metadata().await?.is_dir() {
                fs::remove_dir_all(&entry_path).await?;
            } else {
                fs::remove_file(&entry_path).await?;
            }
        }
        
        Ok(())
    }

    /// Clear application caches
    async fn clear_caches(&self) {
        info!("Clearing application caches");
        
        // Clear specific cache directories
        let cache_dirs = vec![
            "/app/cache/indicators",
            "/app/cache/predictions",
            "/tmp/neural_trader/cache",
        ];
        
        for dir in cache_dirs {
            let path = Path::new(dir);
            if let Err(e) = self.remove_directory_contents(path).await {
                error!("Failed to clear cache {}: {}", dir, e);
            }
        }
    }

    /// Truncate log files to save space
    async fn truncate_logs(&self) {
        info!("Truncating log files");
        
        let log_dir = Path::new("/var/log/neural_trader");
        if !log_dir.exists() {
            return;
        }
        
        if let Ok(mut entries) = fs::read_dir(log_dir).await {
            while let Some(entry) = entries.next_entry().await.ok().flatten() {
                if entry.path().extension().map_or(false, |ext| ext == "log") {
                    // Keep only last 1000 lines of each log
                    if let Err(e) = self.truncate_log_file(&entry.path(), 1000).await {
                        error!("Failed to truncate {}: {}", entry.path().display(), e);
                    }
                }
            }
        }
    }

    /// Truncate a log file to keep only the last N lines
    async fn truncate_log_file(&self, path: &Path, keep_lines: usize) -> Result<(), std::io::Error> {
        let content = fs::read_to_string(path).await?;
        let lines: Vec<&str> = content.lines().collect();
        
        if lines.len() > keep_lines {
            let truncated: Vec<&str> = lines[lines.len() - keep_lines..].to_vec();
            let new_content = truncated.join("\n");
            fs::write(path, new_content).await?;
            
            info!(
                "Truncated {} from {} to {} lines",
                path.display(),
                lines.len(),
                keep_lines
            );
        }
        
        Ok(())
    }
}

/// Streaming data processor that minimizes disk usage
pub struct StreamingDataProcessor {
    buffer_size: usize,
    compression_enabled: bool,
}

impl StreamingDataProcessor {
    pub fn new(buffer_size: usize) -> Self {
        Self {
            buffer_size,
            compression_enabled: true,
        }
    }

    /// Process data in streaming fashion without disk storage
    pub async fn process_stream<T, F>(
        &self,
        mut data_stream: impl futures::Stream<Item = T> + Unpin,
        mut processor: F,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        F: FnMut(Vec<T>) -> Result<(), Box<dyn std::error::Error>>,
        T: Send + 'static,
    {
        use futures::StreamExt;
        
        let mut buffer = Vec::with_capacity(self.buffer_size);
        
        while let Some(item) = data_stream.next().await {
            buffer.push(item);
            
            // Process when buffer is full
            if buffer.len() >= self.buffer_size {
                processor(std::mem::replace(&mut buffer, Vec::with_capacity(self.buffer_size)))?;
            }
        }
        
        // Process remaining items
        if !buffer.is_empty() {
            processor(buffer)?;
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_disk_usage_calculation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = DiskManager::new(DiskUsageConfig::default());
        
        // Create some test files
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "test content").await.unwrap();
        
        let size = manager.calculate_directory_size(temp_dir.path()).await.unwrap();
        assert!(size > 0);
    }

    #[tokio::test]
    async fn test_cleanup_old_files() {
        let temp_dir = TempDir::new().unwrap();
        let manager = DiskManager::new(DiskUsageConfig {
            temp_file_ttl_secs: 0, // Immediate expiry for testing
            ..Default::default()
        });
        
        // Create a test file
        let file_path = temp_dir.path().join("old_file.txt");
        fs::write(&file_path, "old content").await.unwrap();
        
        // Sleep to ensure file is "old"
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Run cleanup
        manager
            .cleanup_old_files(temp_dir.path(), Duration::from_secs(0), SystemTime::now())
            .await
            .unwrap();
        
        // File should be removed
        assert!(!file_path.exists());
    }
}