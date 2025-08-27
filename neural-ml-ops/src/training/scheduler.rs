//! Training Scheduler
//! 
//! Manages scheduling and queuing of training tasks, resource allocation,
//! and priority-based execution.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::Arc;
use tokio::sync::{RwLock, Notify};
use tokio::time::{interval, Duration as TokioDuration};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Scheduler configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    /// Maximum number of concurrent tasks
    pub max_concurrent_tasks: usize,
    /// Task queue capacity
    pub queue_capacity: usize,
    /// Enable priority scheduling
    pub enable_priority_scheduling: bool,
    /// Default task timeout (seconds)
    pub default_timeout_secs: u64,
    /// Cleanup interval for completed tasks (seconds)
    pub cleanup_interval_secs: u64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 4,
            queue_capacity: 100,
            enable_priority_scheduling: true,
            default_timeout_secs: 3600,
            cleanup_interval_secs: 300, // 5 minutes
        }
    }
}

/// Training task priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TaskPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl Default for TaskPriority {
    fn default() -> Self {
        TaskPriority::Normal
    }
}

/// Scheduled training task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: Uuid,
    pub name: String,
    pub workflow_id: String,
    pub priority: TaskPriority,
    pub scheduled_at: DateTime<Utc>,
    pub deadline: Option<DateTime<Utc>>,
    pub estimated_duration: Option<Duration>,
    pub resource_requirements: ResourceRequirements,
    pub retry_count: u32,
    pub max_retries: u32,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

/// Resource requirements for a task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirements {
    /// CPU cores required
    pub cpu_cores: u32,
    /// Memory required in MB
    pub memory_mb: u32,
    /// GPU required
    pub gpu_required: bool,
    /// Disk space required in MB
    pub disk_mb: u32,
}

impl Default for ResourceRequirements {
    fn default() -> Self {
        Self {
            cpu_cores: 1,
            memory_mb: 1024, // 1 GB
            gpu_required: false,
            disk_mb: 1024,   // 1 GB
        }
    }
}

/// Task execution status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Queued,
    Scheduled,
    Running,
    Completed,
    Failed,
    Cancelled,
    Retrying,
}

/// Training scheduler implementation
pub struct TrainingScheduler {
    config: SchedulerConfig,
    task_queue: Arc<RwLock<BinaryHeap<PriorityTask>>>,
    running_tasks: Arc<RwLock<Vec<ScheduledTask>>>,
    completed_tasks: Arc<RwLock<Vec<ScheduledTask>>>,
    resource_monitor: Arc<ResourceMonitor>,
    task_notify: Arc<Notify>,
}

/// Task wrapper for priority queue
#[derive(Debug, Clone)]
struct PriorityTask {
    task: ScheduledTask,
    priority_score: i64,
}

impl PartialEq for PriorityTask {
    fn eq(&self, other: &Self) -> bool {
        self.priority_score == other.priority_score
    }
}

impl Eq for PriorityTask {}

impl PartialOrd for PriorityTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriorityTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority scores come first (max heap)
        self.priority_score.cmp(&other.priority_score)
    }
}

/// System resource monitor
#[derive(Debug)]
pub struct ResourceMonitor {
    available_cpu_cores: Arc<RwLock<u32>>,
    available_memory_mb: Arc<RwLock<u32>>,
    available_disk_mb: Arc<RwLock<u32>>,
    gpu_available: Arc<RwLock<bool>>,
}

impl TrainingScheduler {
    /// Create a new training scheduler
    pub async fn new(config: SchedulerConfig) -> Result<Self> {
        info!("Initializing Training Scheduler");
        
        let scheduler = Self {
            config: config.clone(),
            task_queue: Arc::new(RwLock::new(BinaryHeap::new())),
            running_tasks: Arc::new(RwLock::new(Vec::new())),
            completed_tasks: Arc::new(RwLock::new(Vec::new())),
            resource_monitor: Arc::new(ResourceMonitor::new().await?),
            task_notify: Arc::new(Notify::new()),
        };
        
        // Start background scheduler
        scheduler.start_scheduler_loop().await;
        
        // Start cleanup task
        scheduler.start_cleanup_task().await;
        
        info!("Training Scheduler initialized with {} max concurrent tasks", 
              config.max_concurrent_tasks);
        
        Ok(scheduler)
    }
    
    /// Schedule a new training task
    pub async fn schedule_task(&self, mut task: ScheduledTask) -> Result<()> {
        info!("Scheduling task: {} ({})", task.name, task.id);
        
        // Check queue capacity
        let queue_size = self.task_queue.read().await.len();
        if queue_size >= self.config.queue_capacity {
            warn!("Task queue is full, rejecting task: {}", task.id);
            return Err(anyhow::anyhow!("Task queue is full"));
        }
        
        // Set status and timestamps
        task.status = TaskStatus::Queued;
        task.created_at = Utc::now();
        
        // Calculate priority score
        let priority_score = self.calculate_priority_score(&task);
        
        // Add to queue
        let priority_task = PriorityTask {
            task,
            priority_score,
        };
        
        self.task_queue.write().await.push(priority_task);
        
        // Notify scheduler
        self.task_notify.notify_one();
        
        Ok(())
    }
    
    /// Get task status by ID
    pub async fn get_task_status(&self, task_id: Uuid) -> Option<TaskStatus> {
        // Check running tasks
        let running_tasks = self.running_tasks.read().await;
        if let Some(task) = running_tasks.iter().find(|t| t.id == task_id) {
            return Some(task.status.clone());
        }
        
        // Check completed tasks
        let completed_tasks = self.completed_tasks.read().await;
        if let Some(task) = completed_tasks.iter().find(|t| t.id == task_id) {
            return Some(task.status.clone());
        }
        
        // Check queued tasks
        let queue = self.task_queue.read().await;
        if queue.iter().any(|pt| pt.task.id == task_id) {
            return Some(TaskStatus::Queued);
        }
        
        None
    }
    
    /// Cancel a scheduled task
    pub async fn cancel_task(&self, task_id: Uuid) -> Result<bool> {
        info!("Attempting to cancel task: {}", task_id);
        
        // Try to remove from queue first
        {
            let mut queue = self.task_queue.write().await;
            let original_len = queue.len();
            let queue_vec: Vec<PriorityTask> = queue.drain().collect();
            
            for priority_task in queue_vec {
                if priority_task.task.id != task_id {
                    queue.push(priority_task);
                }
            }
            
            if queue.len() < original_len {
                info!("Task {} cancelled (was in queue)", task_id);
                return Ok(true);
            }
        }
        
        // Try to cancel running task
        {
            let mut running_tasks = self.running_tasks.write().await;
            if let Some(pos) = running_tasks.iter().position(|t| t.id == task_id) {
                let mut task = running_tasks.remove(pos);
                task.status = TaskStatus::Cancelled;
                task.completed_at = Some(Utc::now());
                
                // Move to completed tasks
                self.completed_tasks.write().await.push(task);
                
                info!("Task {} cancelled (was running)", task_id);
                return Ok(true);
            }
        }
        
        warn!("Task {} not found for cancellation", task_id);
        Ok(false)
    }
    
    /// List all tasks with their current status
    pub async fn list_tasks(&self) -> Vec<ScheduledTask> {
        let mut all_tasks = Vec::new();
        
        // Add queued tasks
        let queue = self.task_queue.read().await;
        for priority_task in queue.iter() {
            all_tasks.push(priority_task.task.clone());
        }
        
        // Add running tasks
        let running_tasks = self.running_tasks.read().await;
        all_tasks.extend(running_tasks.iter().cloned());
        
        // Add completed tasks (limited to recent ones)
        let completed_tasks = self.completed_tasks.read().await;
        let recent_cutoff = Utc::now() - Duration::hours(24);
        all_tasks.extend(
            completed_tasks.iter()
                .filter(|t| t.completed_at.map_or(false, |ct| ct > recent_cutoff))
                .cloned()
        );
        
        all_tasks
    }
    
    /// Get scheduler statistics
    pub async fn get_statistics(&self) -> SchedulerStatistics {
        let queued_count = self.task_queue.read().await.len();
        let running_count = self.running_tasks.read().await.len();
        let completed_count = self.completed_tasks.read().await.len();
        
        let resource_utilization = self.resource_monitor.get_utilization().await;
        
        SchedulerStatistics {
            queued_tasks: queued_count,
            running_tasks: running_count,
            completed_tasks: completed_count,
            max_concurrent_tasks: self.config.max_concurrent_tasks,
            queue_capacity: self.config.queue_capacity,
            resource_utilization,
        }
    }
    
    // Private methods
    
    async fn start_scheduler_loop(&self) {
        let task_queue = self.task_queue.clone();
        let running_tasks = self.running_tasks.clone();
        let completed_tasks = self.completed_tasks.clone();
        let resource_monitor = self.resource_monitor.clone();
        let task_notify = self.task_notify.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            info!("Starting scheduler loop");
            
            loop {
                // Wait for notification or timeout
                tokio::select! {
                    _ = task_notify.notified() => {},
                    _ = tokio::time::sleep(TokioDuration::from_secs(5)) => {},
                }
                
                // Try to schedule next task
                if let Err(e) = Self::schedule_next_task(
                    &task_queue,
                    &running_tasks,
                    &completed_tasks,
                    &resource_monitor,
                    &config,
                ).await {
                    warn!("Error in scheduler loop: {}", e);
                }
            }
        });
    }
    
    async fn schedule_next_task(
        task_queue: &Arc<RwLock<BinaryHeap<PriorityTask>>>,
        running_tasks: &Arc<RwLock<Vec<ScheduledTask>>>,
        completed_tasks: &Arc<RwLock<Vec<ScheduledTask>>>,
        resource_monitor: &Arc<ResourceMonitor>,
        config: &SchedulerConfig,
    ) -> Result<()> {
        // Check if we can run more tasks
        let running_count = running_tasks.read().await.len();
        if running_count >= config.max_concurrent_tasks {
            return Ok(());
        }
        
        // Get next task from queue
        let next_task = {
            let mut queue = task_queue.write().await;
            queue.pop()
        };
        
        if let Some(mut priority_task) = next_task {
            // Check resource availability
            if resource_monitor.can_allocate(&priority_task.task.resource_requirements).await {
                let mut task = priority_task.task;
                // Start the task
                task.status = TaskStatus::Running;
                task.started_at = Some(Utc::now());
                
                info!("Starting task: {} ({})", task.name, task.id);
                
                // Allocate resources
                resource_monitor.allocate_resources(&task.resource_requirements).await?;
                
                // Add to running tasks
                running_tasks.write().await.push(task.clone());
                
                // Spawn task execution
                let task_clone = task.clone();
                let running_tasks_clone = running_tasks.clone();
                let completed_tasks_clone = completed_tasks.clone();
                let resource_monitor_clone = resource_monitor.clone();
                
                tokio::spawn(async move {
                    // Simulate task execution
                    let result = Self::execute_task(task_clone.clone()).await;
                    
                    // Remove from running tasks
                    let mut running = running_tasks_clone.write().await;
                    if let Some(pos) = running.iter().position(|t| t.id == task_clone.id) {
                        let mut completed_task = running.remove(pos);
                        
                        // Update task status based on result
                        match result {
                            Ok(_) => {
                                completed_task.status = TaskStatus::Completed;
                                info!("Task completed successfully: {}", completed_task.id);
                            }
                            Err(e) => {
                                completed_task.status = TaskStatus::Failed;
                                completed_task.error = Some(e.to_string());
                                warn!("Task failed: {} - {}", completed_task.id, e);
                            }
                        }
                        
                        completed_task.completed_at = Some(Utc::now());
                        
                        // Release resources
                        if let Err(e) = resource_monitor_clone.release_resources(&completed_task.resource_requirements).await {
                            warn!("Failed to release resources for task {}: {}", completed_task.id, e);
                        }
                        
                        // Move to completed tasks
                        completed_tasks_clone.write().await.push(completed_task);
                    }
                });
            } else {
                // Not enough resources, put task back in queue
                debug!("Insufficient resources for task: {}, returning to queue", priority_task.task.id);
                task_queue.write().await.push(priority_task);
            }
        }
        
        Ok(())
    }
    
    async fn execute_task(task: ScheduledTask) -> Result<()> {
        info!("Executing task: {} ({})", task.name, task.id);
        
        // Simulate task execution time
        let execution_time = task.estimated_duration
            .unwrap_or_else(|| Duration::minutes(5))
            .num_seconds()
            .max(1) as u64;
        
        // Simulate work in smaller chunks to allow for cancellation
        let chunks = execution_time.min(60); // Max 60 second chunks
        let chunk_duration = TokioDuration::from_secs(execution_time / chunks);
        
        for _ in 0..chunks {
            tokio::time::sleep(chunk_duration).await;
            // In a real implementation, this would check for cancellation
        }
        
        info!("Task execution completed: {}", task.id);
        Ok(())
    }
    
    fn calculate_priority_score(&self, task: &ScheduledTask) -> i64 {
        if !self.config.enable_priority_scheduling {
            return Utc::now().timestamp(); // FIFO ordering
        }
        
        let mut score = (task.priority as i64) * 1000;
        
        // Add urgency based on deadline
        if let Some(deadline) = task.deadline {
            let time_to_deadline = (deadline - Utc::now()).num_minutes();
            if time_to_deadline > 0 {
                // Closer deadline = higher priority
                score += 1000 - time_to_deadline.min(1000);
            } else {
                // Past deadline = highest priority
                score += 2000;
            }
        }
        
        // Add age factor (older tasks get higher priority)
        let age_minutes = (Utc::now() - task.created_at).num_minutes();
        score += age_minutes.min(100); // Cap at 100 minutes
        
        score
    }
    
    async fn start_cleanup_task(&self) {
        let completed_tasks = self.completed_tasks.clone();
        let cleanup_interval = self.config.cleanup_interval_secs;
        
        tokio::spawn(async move {
            let mut interval = interval(TokioDuration::from_secs(cleanup_interval));
            
            loop {
                interval.tick().await;
                
                // Clean up old completed tasks
                let cutoff = Utc::now() - Duration::days(7);
                let mut completed = completed_tasks.write().await;
                let original_len = completed.len();
                
                completed.retain(|task| {
                    task.completed_at.map_or(true, |ct| ct > cutoff)
                });
                
                let removed_count = original_len - completed.len();
                if removed_count > 0 {
                    info!("Cleaned up {} old completed tasks", removed_count);
                }
            }
        });
    }
}

impl ResourceMonitor {
    async fn new() -> Result<Self> {
        // Get system resources (simplified)
        let cpu_cores = num_cpus::get() as u32;
        let available_memory_mb = 8192; // 8 GB - would query actual system memory
        let available_disk_mb = 102400; // 100 GB - would query actual disk space
        let gpu_available = false; // Would detect actual GPU
        
        Ok(Self {
            available_cpu_cores: Arc::new(RwLock::new(cpu_cores)),
            available_memory_mb: Arc::new(RwLock::new(available_memory_mb)),
            available_disk_mb: Arc::new(RwLock::new(available_disk_mb)),
            gpu_available: Arc::new(RwLock::new(gpu_available)),
        })
    }
    
    async fn can_allocate(&self, requirements: &ResourceRequirements) -> bool {
        let cpu_available = *self.available_cpu_cores.read().await;
        let memory_available = *self.available_memory_mb.read().await;
        let disk_available = *self.available_disk_mb.read().await;
        let gpu_available = *self.gpu_available.read().await;
        
        cpu_available >= requirements.cpu_cores
            && memory_available >= requirements.memory_mb
            && disk_available >= requirements.disk_mb
            && (!requirements.gpu_required || gpu_available)
    }
    
    async fn allocate_resources(&self, requirements: &ResourceRequirements) -> Result<()> {
        *self.available_cpu_cores.write().await -= requirements.cpu_cores;
        *self.available_memory_mb.write().await -= requirements.memory_mb;
        *self.available_disk_mb.write().await -= requirements.disk_mb;
        
        if requirements.gpu_required {
            *self.gpu_available.write().await = false;
        }
        
        debug!("Resources allocated: CPU: {}, Memory: {}MB, Disk: {}MB, GPU: {}", 
               requirements.cpu_cores, requirements.memory_mb, requirements.disk_mb, requirements.gpu_required);
        
        Ok(())
    }
    
    async fn release_resources(&self, requirements: &ResourceRequirements) -> Result<()> {
        *self.available_cpu_cores.write().await += requirements.cpu_cores;
        *self.available_memory_mb.write().await += requirements.memory_mb;
        *self.available_disk_mb.write().await += requirements.disk_mb;
        
        if requirements.gpu_required {
            *self.gpu_available.write().await = true;
        }
        
        debug!("Resources released: CPU: {}, Memory: {}MB, Disk: {}MB, GPU: {}", 
               requirements.cpu_cores, requirements.memory_mb, requirements.disk_mb, requirements.gpu_required);
        
        Ok(())
    }
    
    async fn get_utilization(&self) -> ResourceUtilization {
        ResourceUtilization {
            cpu_cores_used: num_cpus::get() as u32 - *self.available_cpu_cores.read().await,
            cpu_cores_total: num_cpus::get() as u32,
            memory_mb_used: 8192 - *self.available_memory_mb.read().await,
            memory_mb_total: 8192,
            disk_mb_used: 102400 - *self.available_disk_mb.read().await,
            disk_mb_total: 102400,
            gpu_in_use: !*self.gpu_available.read().await,
        }
    }
}

/// Scheduler statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct SchedulerStatistics {
    pub queued_tasks: usize,
    pub running_tasks: usize,
    pub completed_tasks: usize,
    pub max_concurrent_tasks: usize,
    pub queue_capacity: usize,
    pub resource_utilization: ResourceUtilization,
}

/// Resource utilization information
#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceUtilization {
    pub cpu_cores_used: u32,
    pub cpu_cores_total: u32,
    pub memory_mb_used: u32,
    pub memory_mb_total: u32,
    pub disk_mb_used: u32,
    pub disk_mb_total: u32,
    pub gpu_in_use: bool,
}