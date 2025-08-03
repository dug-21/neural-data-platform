//! Model Rollback System for Production Safety
//!
//! This module provides a comprehensive rollback mechanism for neural models with:
//! - Atomic model updates using symlinks
//! - Automatic rollback on performance degradation
//! - Manual rollback capabilities via CLI
//! - Integration with health monitoring
//! - Docker container restart safety

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::fs as async_fs;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};

use super::errors::{AdapterError, HealthCheckResult, HealthMetrics};
use super::HealthChecker;
use crate::config::NeuralConfig;

/// Rollback configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackConfig {
    /// Base directory for model storage
    pub model_base_dir: PathBuf,
    /// Maximum number of model versions to keep
    pub max_versions: usize,
    /// Performance degradation threshold for auto-rollback (%)
    pub degradation_threshold: f32,
    /// Minimum evaluation period before rollback decision (seconds)
    pub evaluation_period: u64,
    /// Number of performance samples to collect
    pub sample_count: usize,
    /// Enable automatic rollback on degradation
    pub enable_auto_rollback: bool,
    /// Health check interval for rollback monitoring
    pub health_check_interval: Duration,
    /// Grace period after deployment before monitoring starts
    pub grace_period: Duration,
    /// Backup metadata to persistent storage
    pub enable_metadata_backup: bool,
    /// Metadata backup path
    pub metadata_backup_path: PathBuf,
}

impl Default for RollbackConfig {
    fn default() -> Self {
        Self {
            model_base_dir: PathBuf::from("/opt/neural-trader/models"),
            max_versions: 5,
            degradation_threshold: 10.0, // 10% performance drop triggers rollback
            evaluation_period: 300,       // 5 minutes
            sample_count: 20,
            enable_auto_rollback: true,
            health_check_interval: Duration::from_secs(30),
            grace_period: Duration::from_secs(60),
            enable_metadata_backup: true,
            metadata_backup_path: PathBuf::from("/opt/neural-trader/metadata"),
        }
    }
}

/// Model version metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelVersion {
    /// Version identifier (timestamp-based)
    pub version_id: String,
    /// Model name
    pub model_name: String,
    /// Deployment timestamp
    pub deployed_at: DateTime<Utc>,
    /// Model file path
    pub model_path: PathBuf,
    /// Configuration used for this model
    pub config: serde_json::Value,
    /// Performance metrics at deployment
    pub baseline_metrics: ModelMetrics,
    /// Current status
    pub status: ModelStatus,
    /// Rollback count (how many times rolled back)
    pub rollback_count: u32,
    /// SHA256 checksum of model file
    pub checksum: String,
    /// Model size in bytes
    pub size_bytes: u64,
}

/// Model deployment status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelStatus {
    /// Currently active model
    Active,
    /// Previous version (available for rollback)
    Previous,
    /// Archived version
    Archived,
    /// Failed deployment
    Failed,
    /// Rolled back due to issues
    RolledBack,
}

impl std::fmt::Display for ModelStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelStatus::Active => write!(f, "Active"),
            ModelStatus::Previous => write!(f, "Previous"),
            ModelStatus::Archived => write!(f, "Archived"),
            ModelStatus::Failed => write!(f, "Failed"),
            ModelStatus::RolledBack => write!(f, "RolledBack"),
        }
    }
}

/// Model performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetrics {
    /// Prediction accuracy
    pub accuracy: f64,
    /// Average prediction latency (ms)
    pub latency_ms: f64,
    /// Error rate (%)
    pub error_rate: f64,
    /// Memory usage (MB)
    pub memory_mb: u64,
    /// CPU usage (%)
    pub cpu_percent: f32,
    /// Throughput (predictions/sec)
    pub throughput: f64,
    /// Timestamp of metrics collection
    pub timestamp: DateTime<Utc>,
}

impl Default for ModelMetrics {
    fn default() -> Self {
        Self {
            accuracy: 0.0,
            latency_ms: 0.0,
            error_rate: 0.0,
            memory_mb: 0,
            cpu_percent: 0.0,
            throughput: 0.0,
            timestamp: Utc::now(),
        }
    }
}

/// Rollback decision data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackDecision {
    /// Reason for rollback
    pub reason: RollbackReason,
    /// Performance comparison
    pub performance_delta: PerformanceDelta,
    /// Decision timestamp
    pub timestamp: DateTime<Utc>,
    /// Was it automatic or manual
    pub automatic: bool,
    /// Additional context
    pub context: HashMap<String, serde_json::Value>,
}

/// Reasons for rollback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RollbackReason {
    /// Performance degradation detected
    PerformanceDegradation {
        metric: String,
        baseline: f64,
        current: f64,
        threshold: f64,
    },
    /// High error rate
    HighErrorRate {
        error_rate: f64,
        threshold: f64,
    },
    /// Health check failures
    HealthCheckFailure {
        consecutive_failures: u32,
    },
    /// Manual rollback requested
    ManualRequest {
        requestor: String,
        reason: String,
    },
    /// System instability
    SystemInstability {
        description: String,
    },
}

/// Performance comparison between versions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceDelta {
    /// Accuracy change (%)
    pub accuracy_delta: f64,
    /// Latency change (%)
    pub latency_delta: f64,
    /// Error rate change (%)
    pub error_rate_delta: f64,
    /// Memory usage change (%)
    pub memory_delta: f64,
    /// Overall performance score change (%)
    pub overall_delta: f64,
}

/// Model rollback manager
pub struct ModelRollbackManager {
    config: RollbackConfig,
    /// Model version history (model_name -> versions)
    version_history: Arc<RwLock<HashMap<String, VecDeque<ModelVersion>>>>,
    /// Current active versions (model_name -> version)
    active_versions: Arc<RwLock<HashMap<String, ModelVersion>>>,
    /// Performance monitor
    performance_monitor: Arc<Mutex<PerformanceMonitor>>,
    /// Rollback history
    rollback_history: Arc<RwLock<Vec<RollbackDecision>>>,
    /// Health checker integration
    health_checker: Option<Arc<dyn HealthChecker>>,
    /// Lock for atomic operations
    operation_lock: Arc<Mutex<()>>,
}

/// Performance monitoring for rollback decisions
struct PerformanceMonitor {
    /// Recent performance samples (model_name -> samples)
    performance_samples: HashMap<String, VecDeque<ModelMetrics>>,
    /// Baseline metrics for comparison
    baseline_metrics: HashMap<String, ModelMetrics>,
    /// Monitoring start times
    monitoring_start: HashMap<String, DateTime<Utc>>,
}

impl ModelRollbackManager {
    /// Create new rollback manager
    pub fn new(config: RollbackConfig) -> Result<Self> {
        // Ensure directories exist
        fs::create_dir_all(&config.model_base_dir)
            .context("Failed to create model base directory")?;
        
        if config.enable_metadata_backup {
            fs::create_dir_all(&config.metadata_backup_path)
                .context("Failed to create metadata backup directory")?;
        }

        Ok(Self {
            config,
            version_history: Arc::new(RwLock::new(HashMap::new())),
            active_versions: Arc::new(RwLock::new(HashMap::new())),
            performance_monitor: Arc::new(Mutex::new(PerformanceMonitor {
                performance_samples: HashMap::new(),
                baseline_metrics: HashMap::new(),
                monitoring_start: HashMap::new(),
            })),
            rollback_history: Arc::new(RwLock::new(Vec::new())),
            health_checker: None,
            operation_lock: Arc::new(Mutex::new(())),
        })
    }

    /// Set health checker for integration
    pub fn set_health_checker(&mut self, checker: Arc<dyn HealthChecker>) {
        self.health_checker = Some(checker);
    }

    /// Deploy a new model version atomically
    pub async fn deploy_model(
        &self,
        model_name: &str,
        model_path: &Path,
        config: serde_json::Value,
        initial_metrics: ModelMetrics,
    ) -> Result<ModelVersion> {
        // Acquire lock for atomic operation
        let _lock = self.operation_lock.lock().await;

        info!("Deploying new model version for: {}", model_name);

        // Generate version ID
        let version_id = format!("{}-{}", model_name, Utc::now().timestamp_millis());

        // Calculate model checksum
        let checksum = self.calculate_checksum(model_path).await?;
        let size_bytes = async_fs::metadata(model_path).await?.len();

        // Create version directory
        let version_dir = self.config.model_base_dir
            .join(model_name)
            .join(&version_id);
        async_fs::create_dir_all(&version_dir).await?;

        // Copy model to version directory
        let versioned_model_path = version_dir.join("model.bin");
        async_fs::copy(model_path, &versioned_model_path).await?;

        // Save configuration
        let config_path = version_dir.join("config.json");
        let config_content = serde_json::to_string_pretty(&config)?;
        async_fs::write(&config_path, config_content).await?;

        // Create model version metadata
        let model_version = ModelVersion {
            version_id: version_id.clone(),
            model_name: model_name.to_string(),
            deployed_at: Utc::now(),
            model_path: versioned_model_path,
            config,
            baseline_metrics: initial_metrics.clone(),
            status: ModelStatus::Active,
            rollback_count: 0,
            checksum,
            size_bytes,
        };

        // Update symlink atomically
        self.update_current_symlink(model_name, &version_dir).await?;

        // Update version history
        {
            let mut history = self.version_history.write().await;
            let versions = history.entry(model_name.to_string())
                .or_insert_with(|| VecDeque::with_capacity(self.config.max_versions + 1));
            
            // Mark previous version as "Previous"
            if let Some(prev) = versions.back_mut() {
                if prev.status == ModelStatus::Active {
                    prev.status = ModelStatus::Previous;
                }
            }

            versions.push_back(model_version.clone());

            // Clean up old versions
            while versions.len() > self.config.max_versions {
                if let Some(old_version) = versions.pop_front() {
                    self.archive_version(&old_version).await?;
                }
            }
        }

        // Update active version
        {
            let mut active = self.active_versions.write().await;
            active.insert(model_name.to_string(), model_version.clone());
        }

        // Set baseline metrics for monitoring
        {
            let mut monitor = self.performance_monitor.lock().await;
            monitor.baseline_metrics.insert(model_name.to_string(), initial_metrics);
            monitor.monitoring_start.insert(model_name.to_string(), Utc::now());
            monitor.performance_samples.insert(
                model_name.to_string(),
                VecDeque::with_capacity(self.config.sample_count),
            );
        }

        // Backup metadata if enabled
        if self.config.enable_metadata_backup {
            self.backup_metadata(&model_version).await?;
        }

        // Start monitoring if auto-rollback is enabled
        if self.config.enable_auto_rollback {
            self.start_performance_monitoring(model_name).await;
        }

        info!("Successfully deployed model version: {}", version_id);
        Ok(model_version)
    }

    /// Rollback to previous model version
    pub async fn rollback_model(
        &self,
        model_name: &str,
        reason: RollbackReason,
        automatic: bool,
    ) -> Result<ModelVersion> {
        // Acquire lock for atomic operation
        let _lock = self.operation_lock.lock().await;

        info!("Rolling back model: {} (automatic: {})", model_name, automatic);

        // Get version history
        let history = self.version_history.read().await;
        let versions = history.get(model_name)
            .ok_or_else(|| anyhow!("No version history for model: {}", model_name))?;

        // Find previous version
        let previous_version = versions.iter()
            .rev()
            .find(|v| v.status == ModelStatus::Previous)
            .ok_or_else(|| anyhow!("No previous version available for rollback"))?
            .clone();

        drop(history);

        // Calculate performance delta
        let performance_delta = self.calculate_performance_delta(model_name).await?;

        // Record rollback decision
        let decision = RollbackDecision {
            reason,
            performance_delta,
            timestamp: Utc::now(),
            automatic,
            context: HashMap::new(),
        };

        {
            let mut rollback_history = self.rollback_history.write().await;
            rollback_history.push(decision);
        }

        // Update symlink to previous version
        let version_dir = self.config.model_base_dir
            .join(model_name)
            .join(&previous_version.version_id);
        self.update_current_symlink(model_name, &version_dir).await?;

        // Update version statuses
        {
            let mut history = self.version_history.write().await;
            if let Some(versions) = history.get_mut(model_name) {
                for version in versions.iter_mut() {
                    if version.status == ModelStatus::Active {
                        version.status = ModelStatus::RolledBack;
                        version.rollback_count += 1;
                    } else if version.version_id == previous_version.version_id {
                        version.status = ModelStatus::Active;
                    }
                }
            }
        }

        // Update active version
        let mut updated_version = previous_version.clone();
        updated_version.status = ModelStatus::Active;
        {
            let mut active = self.active_versions.write().await;
            active.insert(model_name.to_string(), updated_version.clone());
        }

        // Reset performance monitoring
        {
            let mut monitor = self.performance_monitor.lock().await;
            monitor.baseline_metrics.insert(
                model_name.to_string(),
                updated_version.baseline_metrics.clone(),
            );
            monitor.monitoring_start.insert(model_name.to_string(), Utc::now());
            monitor.performance_samples.get_mut(model_name)
                .map(|samples| samples.clear());
        }

        info!("Successfully rolled back to version: {}", updated_version.version_id);
        Ok(updated_version)
    }

    /// Update current symlink atomically
    async fn update_current_symlink(
        &self,
        model_name: &str,
        target_dir: &Path,
    ) -> Result<()> {
        let model_dir = self.config.model_base_dir.join(model_name);
        let current_link = model_dir.join("current");
        let temp_link = model_dir.join(format!("current.tmp.{}", std::process::id()));

        // Create temporary symlink
        if temp_link.exists() {
            async_fs::remove_file(&temp_link).await?;
        }
        tokio::task::spawn_blocking({
            let target = target_dir.to_path_buf();
            let link = temp_link.clone();
            move || symlink(target, link)
        }).await??;

        // Atomically rename to current
        async_fs::rename(&temp_link, &current_link).await?;

        debug!("Updated current symlink for {} to {:?}", model_name, target_dir);
        Ok(())
    }

    /// Archive old model version
    async fn archive_version(&self, version: &ModelVersion) -> Result<()> {
        let version_dir = self.config.model_base_dir
            .join(&version.model_name)
            .join(&version.version_id);

        if version_dir.exists() {
            let archive_dir = self.config.model_base_dir
                .join(&version.model_name)
                .join("archive");
            async_fs::create_dir_all(&archive_dir).await?;

            let archive_path = archive_dir.join(&version.version_id);
            async_fs::rename(&version_dir, &archive_path).await?;

            info!("Archived model version: {}", version.version_id);
        }

        Ok(())
    }

    /// Calculate SHA256 checksum of model file
    async fn calculate_checksum(&self, path: &Path) -> Result<String> {
        use sha2::{Sha256, Digest};
        
        let content = async_fs::read(path).await?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let result = hasher.finalize();
        Ok(format!("{:x}", result))
    }

    /// Backup model metadata
    async fn backup_metadata(&self, version: &ModelVersion) -> Result<()> {
        let backup_file = self.config.metadata_backup_path
            .join(format!("{}-{}.json", version.model_name, version.version_id));

        let metadata_json = serde_json::to_string_pretty(version)?;
        async_fs::write(&backup_file, metadata_json).await?;

        debug!("Backed up metadata for version: {}", version.version_id);
        Ok(())
    }

    /// Start performance monitoring for auto-rollback
    async fn start_performance_monitoring(&self, model_name: &str) {
        let model_name = model_name.to_string();
        let config = self.config.clone();
        let performance_monitor = Arc::clone(&self.performance_monitor);
        let health_checker = self.health_checker.clone();
        let manager = self.clone_for_monitoring();

        tokio::spawn(async move {
            // Wait for grace period
            sleep(config.grace_period).await;

            info!("Starting performance monitoring for model: {}", model_name);

            let mut interval = tokio::time::interval(config.health_check_interval);
            let mut consecutive_failures = 0;

            loop {
                interval.tick().await;

                // Collect performance metrics
                let metrics = if let Some(checker) = &health_checker {
                    checker.get_metrics(&model_name).await
                } else {
                    // Fallback to basic metrics
                    HealthMetrics {
                        memory_usage_mb: 100,
                        cpu_usage_percent: 25.0,
                        request_count: 1000,
                        error_rate: 1.0,
                        average_response_time: Duration::from_millis(100),
                    }
                };

                let current_metrics = ModelMetrics {
                    accuracy: 100.0 - metrics.error_rate as f64,
                    latency_ms: metrics.average_response_time.as_millis() as f64,
                    error_rate: metrics.error_rate as f64,
                    memory_mb: metrics.memory_usage_mb,
                    cpu_percent: metrics.cpu_usage_percent,
                    throughput: if metrics.average_response_time.as_millis() > 0 {
                        1000.0 / metrics.average_response_time.as_millis() as f64
                    } else {
                        0.0
                    },
                    timestamp: Utc::now(),
                };

                // Record sample
                {
                    let mut monitor = performance_monitor.lock().await;
                    if let Some(samples) = monitor.performance_samples.get_mut(&model_name) {
                        if samples.len() >= config.sample_count {
                            samples.pop_front();
                        }
                        samples.push_back(current_metrics.clone());
                    }
                }

                // Check for degradation
                if let Ok(should_rollback) = manager.check_for_degradation(&model_name).await {
                    if should_rollback {
                        consecutive_failures += 1;
                        
                        if consecutive_failures >= 3 {
                            warn!("Performance degradation detected for model: {}", model_name);
                            
                            // Perform automatic rollback
                            if let Err(e) = manager.perform_auto_rollback(&model_name).await {
                                error!("Failed to perform automatic rollback: {}", e);
                            } else {
                                info!("Automatic rollback completed for model: {}", model_name);
                                break; // Stop monitoring after rollback
                            }
                        }
                    } else {
                        consecutive_failures = 0;
                    }
                }
            }
        });
    }

    /// Check for performance degradation
    async fn check_for_degradation(&self, model_name: &str) -> Result<bool> {
        let monitor = self.performance_monitor.lock().await;
        
        let baseline = monitor.baseline_metrics.get(model_name)
            .ok_or_else(|| anyhow!("No baseline metrics for model: {}", model_name))?;
        
        let samples = monitor.performance_samples.get(model_name)
            .ok_or_else(|| anyhow!("No performance samples for model: {}", model_name))?;

        if samples.len() < self.config.sample_count / 2 {
            return Ok(false); // Not enough samples yet
        }

        // Calculate average metrics
        let avg_metrics = self.calculate_average_metrics(samples);

        // Check for degradation
        let accuracy_degradation = ((baseline.accuracy - avg_metrics.accuracy) / baseline.accuracy) * 100.0;
        let latency_increase = ((avg_metrics.latency_ms - baseline.latency_ms) / baseline.latency_ms) * 100.0;
        let error_rate_increase = avg_metrics.error_rate - baseline.error_rate;

        if accuracy_degradation > self.config.degradation_threshold as f64 ||
           latency_increase > self.config.degradation_threshold as f64 ||
           error_rate_increase > self.config.degradation_threshold as f64 {
            warn!(
                "Performance degradation detected - Accuracy: {:.2}%, Latency: {:.2}%, Error Rate: {:.2}%",
                accuracy_degradation, latency_increase, error_rate_increase
            );
            return Ok(true);
        }

        Ok(false)
    }

    /// Calculate average metrics from samples
    fn calculate_average_metrics(&self, samples: &VecDeque<ModelMetrics>) -> ModelMetrics {
        let count = samples.len() as f64;
        
        let mut avg = ModelMetrics::default();
        for sample in samples {
            avg.accuracy += sample.accuracy;
            avg.latency_ms += sample.latency_ms;
            avg.error_rate += sample.error_rate;
            avg.memory_mb += sample.memory_mb;
            avg.cpu_percent += sample.cpu_percent;
            avg.throughput += sample.throughput;
        }

        avg.accuracy /= count;
        avg.latency_ms /= count;
        avg.error_rate /= count;
        avg.memory_mb = (avg.memory_mb as f64 / count) as u64;
        avg.cpu_percent /= count as f32;
        avg.throughput /= count;

        avg
    }

    /// Calculate performance delta between current and baseline
    async fn calculate_performance_delta(&self, model_name: &str) -> Result<PerformanceDelta> {
        let monitor = self.performance_monitor.lock().await;
        
        let baseline = monitor.baseline_metrics.get(model_name)
            .ok_or_else(|| anyhow!("No baseline metrics"))?;
        
        let samples = monitor.performance_samples.get(model_name)
            .ok_or_else(|| anyhow!("No performance samples"))?;

        let current = if samples.is_empty() {
            baseline.clone()
        } else {
            self.calculate_average_metrics(samples)
        };

        Ok(PerformanceDelta {
            accuracy_delta: ((current.accuracy - baseline.accuracy) / baseline.accuracy) * 100.0,
            latency_delta: ((current.latency_ms - baseline.latency_ms) / baseline.latency_ms) * 100.0,
            error_rate_delta: current.error_rate - baseline.error_rate,
            memory_delta: ((current.memory_mb as f64 - baseline.memory_mb as f64) / baseline.memory_mb as f64) * 100.0,
            overall_delta: ((current.accuracy - baseline.accuracy) / baseline.accuracy) * 100.0,
        })
    }

    /// Perform automatic rollback
    async fn perform_auto_rollback(&self, model_name: &str) -> Result<()> {
        let monitor = self.performance_monitor.lock().await;
        let baseline = monitor.baseline_metrics.get(model_name).cloned();
        let samples = monitor.performance_samples.get(model_name).cloned();
        drop(monitor);

        let current_metrics = if let Some(samples) = samples {
            self.calculate_average_metrics(&samples)
        } else {
            return Err(anyhow!("No performance samples available"));
        };

        let baseline = baseline.ok_or_else(|| anyhow!("No baseline metrics available"))?;

        let reason = RollbackReason::PerformanceDegradation {
            metric: "accuracy".to_string(),
            baseline: baseline.accuracy,
            current: current_metrics.accuracy,
            threshold: self.config.degradation_threshold as f64,
        };

        self.rollback_model(model_name, reason, true).await?;
        Ok(())
    }

    /// Clone manager for monitoring tasks
    fn clone_for_monitoring(&self) -> Self {
        Self {
            config: self.config.clone(),
            version_history: Arc::clone(&self.version_history),
            active_versions: Arc::clone(&self.active_versions),
            performance_monitor: Arc::clone(&self.performance_monitor),
            rollback_history: Arc::clone(&self.rollback_history),
            health_checker: self.health_checker.clone(),
            operation_lock: Arc::clone(&self.operation_lock),
        }
    }

    /// Get current active version for a model
    pub async fn get_active_version(&self, model_name: &str) -> Option<ModelVersion> {
        let active = self.active_versions.read().await;
        active.get(model_name).cloned()
    }

    /// Get version history for a model
    pub async fn get_version_history(&self, model_name: &str) -> Vec<ModelVersion> {
        let history = self.version_history.read().await;
        history.get(model_name)
            .map(|versions| versions.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get rollback history
    pub async fn get_rollback_history(&self) -> Vec<RollbackDecision> {
        let history = self.rollback_history.read().await;
        history.clone()
    }

    /// Manually trigger rollback
    pub async fn manual_rollback(
        &self,
        model_name: &str,
        requestor: &str,
        reason: &str,
    ) -> Result<ModelVersion> {
        let rollback_reason = RollbackReason::ManualRequest {
            requestor: requestor.to_string(),
            reason: reason.to_string(),
        };

        self.rollback_model(model_name, rollback_reason, false).await
    }

    /// Get model path for current version
    pub async fn get_current_model_path(&self, model_name: &str) -> Result<PathBuf> {
        let model_dir = self.config.model_base_dir.join(model_name);
        let current_link = model_dir.join("current");

        if !current_link.exists() {
            return Err(anyhow!("No current version deployed for model: {}", model_name));
        }

        let target = async_fs::read_link(&current_link).await?;
        let model_path = target.join("model.bin");

        if !model_path.exists() {
            return Err(anyhow!("Model file not found at: {:?}", model_path));
        }

        Ok(model_path)
    }

    /// Verify model integrity
    pub async fn verify_model_integrity(&self, model_name: &str) -> Result<bool> {
        let active = self.active_versions.read().await;
        let version = active.get(model_name)
            .ok_or_else(|| anyhow!("No active version for model: {}", model_name))?;

        let model_path = &version.model_path;
        if !model_path.exists() {
            return Ok(false);
        }

        let checksum = self.calculate_checksum(model_path).await?;
        Ok(checksum == version.checksum)
    }

    /// Clean up old archived versions
    pub async fn cleanup_archives(&self, model_name: &str, keep_count: usize) -> Result<u32> {
        let archive_dir = self.config.model_base_dir
            .join(model_name)
            .join("archive");

        if !archive_dir.exists() {
            return Ok(0);
        }

        let mut entries = async_fs::read_dir(&archive_dir).await?;
        let mut archives = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            if let Ok(metadata) = entry.metadata().await {
                if metadata.is_dir() {
                    archives.push((entry.path(), metadata.modified()?));
                }
            }
        }

        // Sort by modification time (oldest first)
        archives.sort_by_key(|&(_, time)| time);

        let mut removed_count = 0;
        if archives.len() > keep_count {
            let to_remove = archives.len() - keep_count;
            for (path, _) in archives.into_iter().take(to_remove) {
                async_fs::remove_dir_all(&path).await?;
                removed_count += 1;
                info!("Removed archived version: {:?}", path);
            }
        }

        Ok(removed_count)
    }
}

/// CLI tool support for rollback operations
pub mod cli {
    use super::*;
    use clap::{Parser, Subcommand};

    #[derive(Parser)]
    #[clap(name = "model-rollback")]
    #[clap(about = "Neural model rollback management tool")]
    pub struct Cli {
        #[clap(subcommand)]
        pub command: Commands,
    }

    #[derive(Subcommand)]
    pub enum Commands {
        /// List model versions
        List {
            /// Model name
            model: String,
        },
        /// Show current active version
        Current {
            /// Model name
            model: String,
        },
        /// Rollback to previous version
        Rollback {
            /// Model name
            model: String,
            /// Reason for rollback
            #[clap(short, long)]
            reason: String,
            /// Requestor name
            #[clap(short = 'u', long)]
            user: String,
        },
        /// Show rollback history
        History {
            /// Model name (optional, shows all if not specified)
            model: Option<String>,
        },
        /// Verify model integrity
        Verify {
            /// Model name
            model: String,
        },
        /// Clean up old archives
        Cleanup {
            /// Model name
            model: String,
            /// Number of archives to keep
            #[clap(short, long, default_value = "3")]
            keep: usize,
        },
    }

    /// Execute CLI command
    pub async fn execute_command(
        manager: &ModelRollbackManager,
        command: Commands,
    ) -> Result<()> {
        match command {
            Commands::List { model } => {
                let versions = manager.get_version_history(&model).await;
                println!("Model versions for '{}':", model);
                println!("{:-<80}", "");
                for version in versions {
                    println!(
                        "{} | {} | {} | {} rollbacks",
                        version.version_id,
                        version.deployed_at.format("%Y-%m-%d %H:%M:%S"),
                        version.status,
                        version.rollback_count
                    );
                }
            }
            Commands::Current { model } => {
                if let Some(version) = manager.get_active_version(&model).await {
                    println!("Current active version for '{}':", model);
                    println!("Version ID: {}", version.version_id);
                    println!("Deployed at: {}", version.deployed_at);
                    println!("Status: {:?}", version.status);
                    println!("Checksum: {}", version.checksum);
                    println!("Size: {} bytes", version.size_bytes);
                } else {
                    println!("No active version found for model: {}", model);
                }
            }
            Commands::Rollback { model, reason, user } => {
                match manager.manual_rollback(&model, &user, &reason).await {
                    Ok(version) => {
                        println!("Successfully rolled back to version: {}", version.version_id);
                    }
                    Err(e) => {
                        eprintln!("Rollback failed: {}", e);
                    }
                }
            }
            Commands::History { model } => {
                let history = manager.get_rollback_history().await;
                let filtered = if let Some(model_name) = model {
                    history.into_iter()
                        .filter(|d| match &d.reason {
                            RollbackReason::PerformanceDegradation { .. } => true,
                            _ => true, // Show all for now
                        })
                        .collect()
                } else {
                    history
                };

                println!("Rollback history:");
                println!("{:-<80}", "");
                for decision in filtered {
                    println!("Timestamp: {}", decision.timestamp);
                    println!("Automatic: {}", decision.automatic);
                    println!("Reason: {:?}", decision.reason);
                    println!("Performance Delta: {:?}", decision.performance_delta);
                    println!("{:-<80}", "");
                }
            }
            Commands::Verify { model } => {
                match manager.verify_model_integrity(&model).await {
                    Ok(valid) => {
                        if valid {
                            println!("Model '{}' integrity verified: OK", model);
                        } else {
                            println!("Model '{}' integrity check: FAILED", model);
                        }
                    }
                    Err(e) => {
                        eprintln!("Verification failed: {}", e);
                    }
                }
            }
            Commands::Cleanup { model, keep } => {
                match manager.cleanup_archives(&model, keep).await {
                    Ok(removed) => {
                        println!("Cleaned up {} archived versions for model '{}'", removed, model);
                    }
                    Err(e) => {
                        eprintln!("Cleanup failed: {}", e);
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_manager() -> (ModelRollbackManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = RollbackConfig {
            model_base_dir: temp_dir.path().join("models"),
            metadata_backup_path: temp_dir.path().join("metadata"),
            max_versions: 3,
            enable_auto_rollback: false, // Disable for tests
            ..Default::default()
        };

        let manager = ModelRollbackManager::new(config).unwrap();
        (manager, temp_dir)
    }

    async fn create_test_model(path: &Path) -> Result<()> {
        let content = b"test model content";
        async_fs::write(path, content).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_model_deployment() {
        let (manager, _temp_dir) = create_test_manager().await;
        let model_path = _temp_dir.path().join("test_model.bin");
        create_test_model(&model_path).await.unwrap();

        let config = serde_json::json!({
            "learning_rate": 0.001,
            "epochs": 100
        });

        let metrics = ModelMetrics {
            accuracy: 95.0,
            latency_ms: 50.0,
            error_rate: 5.0,
            memory_mb: 100,
            cpu_percent: 25.0,
            throughput: 20.0,
            timestamp: Utc::now(),
        };

        let version = manager.deploy_model(
            "test_model",
            &model_path,
            config,
            metrics,
        ).await.unwrap();

        assert_eq!(version.model_name, "test_model");
        assert_eq!(version.status, ModelStatus::Active);
        assert!(version.model_path.exists());

        // Verify symlink
        let current_path = manager.get_current_model_path("test_model").await.unwrap();
        assert!(current_path.exists());
    }

    #[tokio::test]
    async fn test_rollback() {
        let (manager, _temp_dir) = create_test_manager().await;
        
        // Deploy v1
        let model_path_v1 = _temp_dir.path().join("model_v1.bin");
        create_test_model(&model_path_v1).await.unwrap();
        
        let v1 = manager.deploy_model(
            "test_model",
            &model_path_v1,
            serde_json::json!({"version": 1}),
            ModelMetrics::default(),
        ).await.unwrap();

        // Deploy v2
        let model_path_v2 = _temp_dir.path().join("model_v2.bin");
        create_test_model(&model_path_v2).await.unwrap();
        
        let _v2 = manager.deploy_model(
            "test_model",
            &model_path_v2,
            serde_json::json!({"version": 2}),
            ModelMetrics::default(),
        ).await.unwrap();

        // Rollback
        let rolled_back = manager.manual_rollback(
            "test_model",
            "test_user",
            "Test rollback",
        ).await.unwrap();

        assert_eq!(rolled_back.version_id, v1.version_id);
        assert_eq!(rolled_back.status, ModelStatus::Active);

        // Check history
        let history = manager.get_version_history("test_model").await;
        assert_eq!(history.len(), 2);
    }

    #[tokio::test]
    async fn test_version_limit() {
        let (manager, _temp_dir) = create_test_manager().await;
        
        // Deploy 4 versions (limit is 3)
        for i in 1..=4 {
            let model_path = _temp_dir.path().join(format!("model_v{}.bin", i));
            create_test_model(&model_path).await.unwrap();
            
            manager.deploy_model(
                "test_model",
                &model_path,
                serde_json::json!({"version": i}),
                ModelMetrics::default(),
            ).await.unwrap();
        }

        // Check that only 3 versions are kept
        let history = manager.get_version_history("test_model").await;
        assert_eq!(history.len(), 3);
    }

    #[tokio::test]
    async fn test_integrity_verification() {
        let (manager, _temp_dir) = create_test_manager().await;
        let model_path = _temp_dir.path().join("test_model.bin");
        create_test_model(&model_path).await.unwrap();

        manager.deploy_model(
            "test_model",
            &model_path,
            serde_json::json!({}),
            ModelMetrics::default(),
        ).await.unwrap();

        // Verify integrity
        let is_valid = manager.verify_model_integrity("test_model").await.unwrap();
        assert!(is_valid);
    }
}