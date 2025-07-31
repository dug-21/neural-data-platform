//! Market-aware training scheduler with priority queue and resource management
//!
//! This module provides a sophisticated training scheduler that:
//! - Manages training jobs with priority queues
//! - Respects market hours to minimize impact
//! - Implements resource governance during trading
//! - Handles emergency training overrides
//! - Integrates with autonomous_training.rs

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering as AtomicOrdering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock, Semaphore};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::daa::autonomous_training::{
    TrainingDecision, TrainingOutcome, TrainingPriority as AutonomousTrainingPriority,
};
use crate::utils::market_hours::{MarketHours, TrainingWindow};

/// Training job priority levels with numerical values for comparison
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JobPriority {
    /// Emergency priority - bypasses all restrictions
    Emergency = 1000,
    /// Critical priority - minimal restrictions
    Critical = 800,
    /// High priority - some restrictions apply
    High = 600,
    /// Medium priority - normal restrictions
    Medium = 400,
    /// Low priority - all restrictions apply
    Low = 200,
    /// Background priority - only runs during optimal windows
    Background = 100,
}

impl From<AutonomousTrainingPriority> for JobPriority {
    fn from(priority: AutonomousTrainingPriority) -> Self {
        match priority {
            AutonomousTrainingPriority::Emergency => JobPriority::Emergency,
            AutonomousTrainingPriority::Critical => JobPriority::Critical,
            AutonomousTrainingPriority::High => JobPriority::High,
            AutonomousTrainingPriority::Medium => JobPriority::Medium,
            AutonomousTrainingPriority::Low => JobPriority::Low,
        }
    }
}

/// Job status tracking
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JobStatus {
    /// Waiting in queue
    Queued { position: usize },
    /// Waiting for suitable market conditions
    WaitingForWindow { next_check: DateTime<Utc> },
    /// Currently executing
    Running { started_at: DateTime<Utc>, progress: f64 },
    /// Completed successfully
    Completed { finished_at: DateTime<Utc>, outcome: String },
    /// Failed during execution
    Failed { error: String, retry_count: usize },
    /// Cancelled by user or system
    Cancelled { reason: String },
}

/// Resource profile for a training job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceProfile {
    /// CPU cores required (0.0 - 1.0 as percentage of total)
    pub cpu_percentage: f64,
    /// Memory required in MB
    pub memory_mb: u64,
    /// GPU required (if any)
    pub gpu_required: bool,
    /// Network bandwidth in Mbps
    pub network_mbps: f64,
    /// Estimated duration
    pub estimated_duration: Duration,
}

/// Training job representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DAATrainingJob {
    /// Unique job identifier
    pub id: String,
    /// Job priority
    pub priority: JobPriority,
    /// Associated training decision
    pub decision: TrainingDecision,
    /// Resource requirements
    pub resources: ResourceProfile,
    /// Current status
    pub status: JobStatus,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Retry count for failures
    pub retry_count: usize,
    /// Maximum retries allowed
    pub max_retries: usize,
    /// Optional callback for completion
    #[serde(skip)]
    pub callback: Option<mpsc::UnboundedSender<TrainingOutcome>>,
}

impl DAATrainingJob {
    /// Create a new training job from a decision
    pub fn from_decision(decision: TrainingDecision) -> Self {
        let priority = JobPriority::from(decision.priority.clone());
        let resources = ResourceProfile {
            cpu_percentage: match &priority {
                JobPriority::Emergency => 0.9,
                JobPriority::Critical => 0.7,
                JobPriority::High => 0.5,
                JobPriority::Medium => 0.3,
                JobPriority::Low => 0.2,
                JobPriority::Background => 0.1,
            },
            memory_mb: (decision.resource_requirements.memory_gb * 1024.0) as u64,
            gpu_required: decision.resource_requirements.gpu_required,
            network_mbps: decision.resource_requirements.network_bandwidth_mbps,
            estimated_duration: decision.estimated_duration,
        };

        Self {
            id: Uuid::new_v4().to_string(),
            priority,
            decision,
            resources,
            status: JobStatus::Queued { position: 0 },
            created_at: Utc::now(),
            updated_at: Utc::now(),
            retry_count: 0,
            max_retries: 3,
            callback: None,
        }
    }
}

// Implement ordering for priority queue
impl Ord for DAATrainingJob {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first
        let priority_cmp = (self.priority as i32).cmp(&(other.priority as i32)).reverse();
        if priority_cmp != Ordering::Equal {
            return priority_cmp;
        }
        // Then by creation time (older first)
        other.created_at.cmp(&self.created_at)
    }
}

impl PartialOrd for DAATrainingJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for DAATrainingJob {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for DAATrainingJob {}

/// Resource limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimitConfig {
    /// Maximum CPU usage during trading hours (0.0 - 1.0)
    pub trading_cpu_limit: f64,
    /// Maximum CPU usage during off hours (0.0 - 1.0)
    pub off_hours_cpu_limit: f64,
    /// Maximum memory usage in MB
    pub max_memory_mb: u64,
    /// Whether to allow GPU usage during trading
    pub gpu_during_trading: bool,
    /// Maximum concurrent jobs
    pub max_concurrent_jobs: usize,
}

impl Default for ResourceLimitConfig {
    fn default() -> Self {
        Self {
            trading_cpu_limit: 0.3,      // 30% during trading
            off_hours_cpu_limit: 0.9,    // 90% during off hours
            max_memory_mb: 16384,        // 16GB max
            gpu_during_trading: false,
            max_concurrent_jobs: 4,
        }
    }
}

/// Scheduler configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DAASchedulerConfig {
    /// Resource limits
    pub resource_limits: ResourceLimitConfig,
    /// Check interval for market conditions
    pub check_interval: Duration,
    /// Maximum queue size
    pub max_queue_size: usize,
    /// Enable emergency override
    pub emergency_override: bool,
    /// Market hours tracker update interval
    pub market_update_interval: Duration,
}

impl Default for DAASchedulerConfig {
    fn default() -> Self {
        Self {
            resource_limits: ResourceLimitConfig::default(),
            check_interval: Duration::minutes(1),
            max_queue_size: 1000,
            emergency_override: true,
            market_update_interval: Duration::minutes(5),
        }
    }
}

/// Resource usage tracking
#[derive(Debug, Clone)]
struct ResourceUsage {
    cpu_used: Arc<AtomicU64>,         // Represented as percentage * 1000 for precision
    memory_used_mb: Arc<AtomicU64>,
    gpu_in_use: Arc<AtomicBool>,
    active_jobs: Arc<AtomicUsize>,
}

impl ResourceUsage {
    fn new() -> Self {
        Self {
            cpu_used: Arc::new(AtomicU64::new(0)),
            memory_used_mb: Arc::new(AtomicU64::new(0)),
            gpu_in_use: Arc::new(AtomicBool::new(false)),
            active_jobs: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn allocate(&self, resources: &ResourceProfile) -> bool {
        // Try to allocate CPU
        let cpu_units = (resources.cpu_percentage * 1000.0) as u64;
        let current_cpu = self.cpu_used.load(AtomicOrdering::Acquire);
        if current_cpu + cpu_units > 1000 {
            return false;
        }

        // Try to allocate memory
        let current_memory = self.memory_used_mb.load(AtomicOrdering::Acquire);
        if current_memory + resources.memory_mb > 16384 {
            return false;
        }

        // Check GPU
        if resources.gpu_required && self.gpu_in_use.load(AtomicOrdering::Acquire) {
            return false;
        }

        // Allocate resources
        self.cpu_used.fetch_add(cpu_units, AtomicOrdering::AcqRel);
        self.memory_used_mb.fetch_add(resources.memory_mb, AtomicOrdering::AcqRel);
        if resources.gpu_required {
            self.gpu_in_use.store(true, AtomicOrdering::Release);
        }
        self.active_jobs.fetch_add(1, AtomicOrdering::AcqRel);

        true
    }

    fn release(&self, resources: &ResourceProfile) {
        let cpu_units = (resources.cpu_percentage * 1000.0) as u64;
        self.cpu_used.fetch_sub(cpu_units, AtomicOrdering::AcqRel);
        self.memory_used_mb.fetch_sub(resources.memory_mb, AtomicOrdering::AcqRel);
        if resources.gpu_required {
            self.gpu_in_use.store(false, AtomicOrdering::Release);
        }
        self.active_jobs.fetch_sub(1, AtomicOrdering::AcqRel);
    }

    fn get_cpu_percentage(&self) -> f64 {
        self.cpu_used.load(AtomicOrdering::Acquire) as f64 / 1000.0
    }
}

/// Market-aware training scheduler
pub struct DAATrainingScheduler {
    /// Configuration
    config: DAASchedulerConfig,
    /// Priority queue of pending jobs
    job_queue: Arc<Mutex<BinaryHeap<DAATrainingJob>>>,
    /// Active jobs tracking
    active_jobs: Arc<RwLock<HashMap<String, DAATrainingJob>>>,
    /// Completed jobs history
    completed_jobs: Arc<RwLock<VecDeque<DAATrainingJob>>>,
    /// Market hours tracker
    market_hours: Arc<MarketHours>,
    /// Resource usage tracking
    resource_usage: ResourceUsage,
    /// Semaphore for concurrent job limiting
    job_semaphore: Arc<Semaphore>,
    /// Shutdown signal
    shutdown: Arc<AtomicBool>,
    /// Scheduler task handle
    scheduler_handle: Option<JoinHandle<()>>,
    /// Job executor channel
    executor_tx: mpsc::UnboundedSender<DAATrainingJob>,
    executor_rx: Option<mpsc::UnboundedReceiver<DAATrainingJob>>,
}

impl DAATrainingScheduler {
    /// Create a new training scheduler
    pub fn new(config: DAASchedulerConfig) -> Result<Self> {
        let (executor_tx, executor_rx) = mpsc::unbounded_channel();
        let job_semaphore = Arc::new(Semaphore::new(config.resource_limits.max_concurrent_jobs));

        Ok(Self {
            config,
            job_queue: Arc::new(Mutex::new(BinaryHeap::new())),
            active_jobs: Arc::new(RwLock::new(HashMap::new())),
            completed_jobs: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            market_hours: Arc::new(MarketHours::new()),
            resource_usage: ResourceUsage::new(),
            job_semaphore,
            shutdown: Arc::new(AtomicBool::new(false)),
            scheduler_handle: None,
            executor_tx,
            executor_rx: Some(executor_rx),
        })
    }

    /// Start the scheduler
    pub async fn start(&mut self) -> Result<()> {
        info!("🚀 Starting DAA Training Scheduler");

        // Take the receiver
        let mut executor_rx = self.executor_rx.take()
            .ok_or_else(|| anyhow::anyhow!("Scheduler already started"))?;

        // Start the scheduler loop
        let scheduler_handle = {
            let job_queue = Arc::clone(&self.job_queue);
            let active_jobs = Arc::clone(&self.active_jobs);
            let completed_jobs = Arc::clone(&self.completed_jobs);
            let market_hours = Arc::clone(&self.market_hours);
            let resource_usage = self.resource_usage.clone();
            let job_semaphore = Arc::clone(&self.job_semaphore);
            let shutdown = Arc::clone(&self.shutdown);
            let config = self.config.clone();
            let executor_tx = self.executor_tx.clone();

            tokio::spawn(async move {
                let mut interval = tokio::time::interval(config.check_interval.to_std().unwrap());

                while !shutdown.load(AtomicOrdering::Acquire) {
                    interval.tick().await;

                    // Process the queue
                    if let Err(e) = Self::process_queue(
                        &job_queue,
                        &active_jobs,
                        &market_hours,
                        &resource_usage,
                        &config,
                        &executor_tx,
                    ).await {
                        error!("Error processing job queue: {}", e);
                    }
                }

                info!("📛 Scheduler loop shutting down");
            })
        };

        // Start the executor loop
        let executor_handle = {
            let active_jobs = Arc::clone(&self.active_jobs);
            let completed_jobs = Arc::clone(&self.completed_jobs);
            let resource_usage = self.resource_usage.clone();
            let job_semaphore = Arc::clone(&self.job_semaphore);
            let shutdown = Arc::clone(&self.shutdown);

            tokio::spawn(async move {
                while !shutdown.load(AtomicOrdering::Acquire) {
                    match executor_rx.recv().await {
                        Some(job) => {
                            let active_jobs = Arc::clone(&active_jobs);
                            let completed_jobs = Arc::clone(&completed_jobs);
                            let resource_usage = resource_usage.clone();
                            let job_semaphore = Arc::clone(&job_semaphore);

                            tokio::spawn(async move {
                                let _permit = job_semaphore.acquire().await.unwrap();
                                Self::execute_job(
                                    job,
                                    &active_jobs,
                                    &completed_jobs,
                                    &resource_usage,
                                ).await;
                            });
                        }
                        None => break,
                    }
                }

                info!("📛 Executor loop shutting down");
            })
        };

        self.scheduler_handle = Some(scheduler_handle);
        
        info!("✅ DAA Training Scheduler started successfully");
        Ok(())
    }

    /// Submit a new training job
    pub async fn submit_job(&self, mut job: DAATrainingJob) -> Result<String> {
        // Check queue size
        {
            let queue = self.job_queue.lock().await;
            if queue.len() >= self.config.max_queue_size {
                return Err(anyhow::anyhow!("Job queue is full"));
            }
        }

        let job_id = job.id.clone();
        job.status = JobStatus::Queued { position: 0 };
        job.updated_at = Utc::now();

        // Add to queue
        {
            let mut queue = self.job_queue.lock().await;
            queue.push(job.clone());
            
            // Update positions
            let mut jobs: Vec<_> = queue.drain().collect();
            for (i, j) in jobs.iter_mut().enumerate() {
                if let JobStatus::Queued { ref mut position } = j.status {
                    *position = i;
                    j.updated_at = Utc::now();
                }
            }
            for j in jobs {
                queue.push(j);
            }
        }

        info!("📥 Job {} submitted with priority {:?}", job_id, job.priority);
        Ok(job_id)
    }

    /// Cancel a job
    pub async fn cancel_job(&self, job_id: &str, reason: &str) -> Result<()> {
        // Check active jobs first
        {
            let mut active = self.active_jobs.write().await;
            if let Some(mut job) = active.remove(job_id) {
                job.status = JobStatus::Cancelled { reason: reason.to_string() };
                job.updated_at = Utc::now();
                
                // Release resources
                self.resource_usage.release(&job.resources);
                
                // Add to completed
                let mut completed = self.completed_jobs.write().await;
                completed.push_back(job);
                
                info!("🚫 Cancelled active job {}", job_id);
                return Ok(());
            }
        }

        // Check queue
        {
            let mut queue = self.job_queue.lock().await;
            let jobs: Vec<_> = queue.drain().collect();
            let mut found = false;
            
            for mut job in jobs {
                if job.id == job_id {
                    job.status = JobStatus::Cancelled { reason: reason.to_string() };
                    job.updated_at = Utc::now();
                    
                    let mut completed = self.completed_jobs.write().await;
                    completed.push_back(job);
                    found = true;
                    
                    info!("🚫 Cancelled queued job {}", job_id);
                } else {
                    queue.push(job);
                }
            }
            
            if found {
                return Ok(());
            }
        }

        Err(anyhow::anyhow!("Job {} not found", job_id))
    }

    /// Get job status
    pub async fn get_job_status(&self, job_id: &str) -> Option<JobStatus> {
        // Check active jobs
        if let Some(job) = self.active_jobs.read().await.get(job_id) {
            return Some(job.status.clone());
        }

        // Check queue
        {
            let queue = self.job_queue.lock().await;
            for job in queue.iter() {
                if job.id == job_id {
                    return Some(job.status.clone());
                }
            }
        }

        // Check completed
        let completed = self.completed_jobs.read().await;
        for job in completed.iter() {
            if job.id == job_id {
                return Some(job.status.clone());
            }
        }

        None
    }

    /// Get current resource usage
    pub async fn get_resource_usage(&self) -> (f64, u64, bool, usize) {
        (
            self.resource_usage.get_cpu_percentage(),
            self.resource_usage.memory_used_mb.load(AtomicOrdering::Acquire),
            self.resource_usage.gpu_in_use.load(AtomicOrdering::Acquire),
            self.resource_usage.active_jobs.load(AtomicOrdering::Acquire),
        )
    }

    /// Process the job queue
    async fn process_queue(
        job_queue: &Arc<Mutex<BinaryHeap<DAATrainingJob>>>,
        active_jobs: &Arc<RwLock<HashMap<String, DAATrainingJob>>>,
        market_hours: &Arc<MarketHours>,
        resource_usage: &ResourceUsage,
        config: &DAASchedulerConfig,
        executor_tx: &mpsc::UnboundedSender<DAATrainingJob>,
    ) -> Result<()> {
        let now = Utc::now();
        let mut jobs_to_execute = Vec::new();

        // Get current market conditions
        let training_window = market_hours.get_training_window(now).await;
        let resource_limit = market_hours.get_resource_limit(now).await;
        let market_intensity = market_hours.get_market_intensity(now).await;

        debug!("🕐 Processing queue - Window: {:?}, Resource limit: {:.2}, Intensity: {:.2}", 
               training_window, resource_limit, market_intensity.score);

        // Process queue
        {
            let mut queue = job_queue.lock().await;
            let mut temp_jobs = Vec::new();

            while let Some(mut job) = queue.pop() {
                let should_execute = Self::should_execute_job(
                    &job,
                    &training_window,
                    resource_limit,
                    resource_usage,
                    config,
                );

                match should_execute {
                    Ok(true) => {
                        // Try to allocate resources
                        if resource_usage.allocate(&job.resources) {
                            job.status = JobStatus::Running { 
                                started_at: now, 
                                progress: 0.0 
                            };
                            job.updated_at = now;
                            jobs_to_execute.push(job);
                        } else {
                            // Resources not available, keep in queue
                            temp_jobs.push(job);
                        }
                    }
                    Ok(false) => {
                        // Update status to waiting
                        if let JobStatus::Queued { .. } = job.status {
                            job.status = JobStatus::WaitingForWindow { 
                                next_check: now + config.check_interval 
                            };
                            job.updated_at = now;
                        }
                        temp_jobs.push(job);
                    }
                    Err(e) => {
                        warn!("Failed to evaluate job {}: {}", job.id, e);
                        temp_jobs.push(job);
                    }
                }

                // Stop if we've hit the concurrent job limit
                if jobs_to_execute.len() >= config.resource_limits.max_concurrent_jobs {
                    break;
                }
            }

            // Put jobs back in queue
            for job in temp_jobs {
                queue.push(job);
            }
        }

        // Execute selected jobs
        for job in jobs_to_execute {
            let job_id = job.id.clone();
            let job_priority = job.priority.clone(); // Store priority before moving job
            
            // Add to active jobs
            active_jobs.write().await.insert(job_id.clone(), job.clone());
            
            // Send to executor
            if let Err(e) = executor_tx.send(job) {
                error!("Failed to send job {} to executor: {}", job_id, e);
                
                // Remove from active and release resources
                if let Some(job) = active_jobs.write().await.remove(&job_id) {
                    resource_usage.release(&job.resources);
                }
            } else {
                info!("🏃 Started executing job {} with priority {:?}", job_id, job_priority);
            }
        }

        Ok(())
    }

    /// Check if a job should be executed now
    fn should_execute_job(
        job: &DAATrainingJob,
        training_window: &TrainingWindow,
        resource_limit: f64,
        resource_usage: &ResourceUsage,
        config: &DAASchedulerConfig,
    ) -> Result<bool> {
        // Emergency jobs always execute
        if job.priority == JobPriority::Emergency && config.emergency_override {
            return Ok(true);
        }

        // Check resource availability
        let current_cpu = resource_usage.get_cpu_percentage();
        if current_cpu + job.resources.cpu_percentage > resource_limit {
            debug!("Job {} exceeds resource limit: {:.2} + {:.2} > {:.2}", 
                   job.id, current_cpu, job.resources.cpu_percentage, resource_limit);
            return Ok(false);
        }

        // Check training window requirements
        match (job.priority, training_window) {
            // Critical jobs run in all but restricted windows
            (JobPriority::Critical, TrainingWindow::Restricted) => Ok(false),
            (JobPriority::Critical, _) => Ok(true),
            
            // High priority jobs need at least acceptable window
            (JobPriority::High, TrainingWindow::Poor) | 
            (JobPriority::High, TrainingWindow::Restricted) => Ok(false),
            (JobPriority::High, _) => Ok(true),
            
            // Medium priority needs good window
            (JobPriority::Medium, TrainingWindow::Optimal) |
            (JobPriority::Medium, TrainingWindow::Good) => Ok(true),
            (JobPriority::Medium, _) => Ok(false),
            
            // Low and background only in optimal windows
            (JobPriority::Low, TrainingWindow::Optimal) => Ok(true),
            (JobPriority::Background, TrainingWindow::Optimal) => Ok(true),
            _ => Ok(false),
        }
    }

    /// Execute a training job
    async fn execute_job(
        mut job: DAATrainingJob,
        active_jobs: &Arc<RwLock<HashMap<String, DAATrainingJob>>>,
        completed_jobs: &Arc<RwLock<VecDeque<DAATrainingJob>>>,
        resource_usage: &ResourceUsage,
    ) {
        let job_id = job.id.clone();
        info!("🎯 Executing training job {} with priority {:?}", job_id, job.priority);

        // Simulate training execution with progress updates
        let start_time = Utc::now();
        let duration_secs = job.resources.estimated_duration.num_seconds() as u64;
        let update_interval = std::time::Duration::from_secs(5);
        
        for elapsed in (0..duration_secs).step_by(5) {
            // Update progress
            let progress = (elapsed as f64 / duration_secs as f64).min(1.0);
            
            // Update job status
            {
                let mut active = active_jobs.write().await;
                if let Some(active_job) = active.get_mut(&job_id) {
                    active_job.status = JobStatus::Running { 
                        started_at: start_time, 
                        progress 
                    };
                    active_job.updated_at = Utc::now();
                }
            }
            
            // Simulate work
            tokio::time::sleep(update_interval).await;
            
            // Check if we should continue (could add cancellation check here)
            if elapsed + 5 >= duration_secs {
                break;
            }
        }

        // Complete the job
        let outcome = TrainingOutcome::Success {
            improvement: 15.0, // Simulated improvement
            new_accuracy: 0.85,
        };

        job.status = JobStatus::Completed {
            finished_at: Utc::now(),
            outcome: format!("{:?}", outcome),
        };
        job.updated_at = Utc::now();

        // Send callback if provided
        if let Some(callback) = &job.callback {
            let _ = callback.send(outcome);
        }

        // Remove from active and add to completed
        active_jobs.write().await.remove(&job_id);
        
        {
            let mut completed = completed_jobs.write().await;
            completed.push_back(job.clone());
            
            // Keep only last 1000 completed jobs
            while completed.len() > 1000 {
                completed.pop_front();
            }
        }

        // Release resources
        resource_usage.release(&job.resources);

        info!("✅ Completed training job {} successfully", job_id);
    }

    /// Shutdown the scheduler
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("🛑 Shutting down DAA Training Scheduler");
        
        self.shutdown.store(true, AtomicOrdering::Release);
        
        if let Some(handle) = self.scheduler_handle.take() {
            handle.await?;
        }
        
        info!("✅ DAA Training Scheduler shut down successfully");
        Ok(())
    }

    /// Get queue statistics
    pub async fn get_queue_stats(&self) -> QueueStats {
        let queue = self.job_queue.lock().await;
        let active = self.active_jobs.read().await;
        let completed = self.completed_jobs.read().await;
        
        let mut priority_counts = HashMap::new();
        for job in queue.iter() {
            *priority_counts.entry(job.priority).or_insert(0) += 1;
        }
        
        QueueStats {
            queued_jobs: queue.len(),
            active_jobs: active.len(),
            completed_jobs: completed.len(),
            priority_breakdown: priority_counts,
            cpu_usage: self.resource_usage.get_cpu_percentage(),
            memory_usage_mb: self.resource_usage.memory_used_mb.load(AtomicOrdering::Acquire),
            gpu_in_use: self.resource_usage.gpu_in_use.load(AtomicOrdering::Acquire),
        }
    }
}

/// Queue statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct QueueStats {
    pub queued_jobs: usize,
    pub active_jobs: usize,
    pub completed_jobs: usize,
    pub priority_breakdown: HashMap<JobPriority, usize>,
    pub cpu_usage: f64,
    pub memory_usage_mb: u64,
    pub gpu_in_use: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_scheduler_creation() {
        let config = DAASchedulerConfig::default();
        let scheduler = DAATrainingScheduler::new(config).unwrap();
        
        let (cpu, memory, gpu, active) = scheduler.get_resource_usage().await;
        assert_eq!(cpu, 0.0);
        assert_eq!(memory, 0);
        assert!(!gpu);
        assert_eq!(active, 0);
    }

    #[tokio::test]
    async fn test_job_priority_ordering() {
        let decision = TrainingDecision {
            decision_id: "test".to_string(),
            timestamp: Utc::now(),
            decision_type: crate::daa::autonomous_training::TrainingDecisionType::FullRetraining {
                reason: "test".to_string(),
                expected_improvement: 0.1,
            },
            confidence: 0.9,
            reasoning: vec![],
            performance_snapshot: crate::daa::autonomous_training::PerformanceSnapshot {
                timestamp: Utc::now(),
                accuracy: 0.8,
                confidence: 0.9,
                price_error: 0.1,
                sharpe_ratio: 0.5,
                max_drawdown: 0.1,
                volatility: 0.02,
                model_agreement: 0.9,
                consecutive_failures: 0,
                trading_volume: 1000000.0,
                profit_loss: 0.0,
            },
            resource_requirements: crate::daa::autonomous_training::ResourceRequirements::minimal(),
            estimated_duration: Duration::hours(1),
            priority: crate::daa::autonomous_training::TrainingPriority::High,
            affected_models: vec!["test".to_string()],
        };

        let mut job1 = DAATrainingJob::from_decision(decision.clone());
        job1.priority = JobPriority::Low;
        
        let mut job2 = DAATrainingJob::from_decision(decision);
        job2.priority = JobPriority::Emergency;
        
        assert!(job2 > job1);
    }

    #[tokio::test]
    async fn test_resource_allocation() {
        let usage = ResourceUsage::new();
        
        let profile = ResourceProfile {
            cpu_percentage: 0.5,
            memory_mb: 1024,
            gpu_required: false,
            network_mbps: 100.0,
            estimated_duration: Duration::hours(1),
        };
        
        assert!(usage.allocate(&profile));
        assert_eq!(usage.get_cpu_percentage(), 0.5);
        
        // Try to allocate more than available
        let profile2 = ResourceProfile {
            cpu_percentage: 0.6,
            memory_mb: 1024,
            gpu_required: false,
            network_mbps: 100.0,
            estimated_duration: Duration::hours(1),
        };
        
        assert!(!usage.allocate(&profile2));
        
        // Release and try again
        usage.release(&profile);
        assert_eq!(usage.get_cpu_percentage(), 0.0);
        assert!(usage.allocate(&profile2));
    }
}