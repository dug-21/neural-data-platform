//! Training scheduler with market awareness
//! 
//! Schedules training jobs to minimize impact on trading operations by
//! preferring off-market hours while supporting emergency training overrides.

use crate::market_schedule::{Exchange, MarketSchedule, MarketStatus};
use crate::priority_queue::{Priority, TrainingJob, TrainingQueue};
use chrono::{DateTime, Duration, Utc};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration as StdDuration;
use tokio::sync::mpsc;
use tokio::time::{interval, Instant};

/// Training scheduler configuration
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Maximum concurrent training jobs
    pub max_concurrent_jobs: usize,
    /// Check interval for scheduling decisions (seconds)
    pub check_interval_secs: u64,
    /// Market intensity threshold for normal priority jobs (0.0-1.0)
    pub market_intensity_threshold: f64,
    /// Minimum training window duration (hours)
    pub min_training_window_hours: f64,
    /// Resource usage limit during market hours (0.0-1.0)
    pub market_hours_resource_limit: f64,
    /// Resource usage limit during off hours (0.0-1.0)
    pub off_hours_resource_limit: f64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_jobs: 4,
            check_interval_secs: 60,
            market_intensity_threshold: 0.3,
            min_training_window_hours: 2.0,
            market_hours_resource_limit: 0.25,  // Use only 25% resources during market
            off_hours_resource_limit: 0.90,     // Use up to 90% resources off hours
        }
    }
}

/// Training scheduler state
#[derive(Debug)]
pub struct TrainingScheduler {
    config: SchedulerConfig,
    market_schedule: Arc<MarketSchedule>,
    job_queue: Arc<Mutex<TrainingQueue>>,
    active_jobs: Arc<Mutex<Vec<TrainingJob>>>,
    resource_monitor: Arc<ResourceMonitor>,
}

/// Resource usage monitoring
#[derive(Debug)]
struct ResourceMonitor {
    cpu_usage: Arc<Mutex<f64>>,
    memory_usage: Arc<Mutex<f64>>,
    gpu_usage: Arc<Mutex<Option<f64>>>,
}

impl ResourceMonitor {
    fn new() -> Self {
        Self {
            cpu_usage: Arc::new(Mutex::new(0.0)),
            memory_usage: Arc::new(Mutex::new(0.0)),
            gpu_usage: Arc::new(Mutex::new(None)),
        }
    }

    /// Get current resource usage (0.0-1.0)
    fn get_usage(&self) -> f64 {
        let cpu = *self.cpu_usage.lock().unwrap();
        let memory = *self.memory_usage.lock().unwrap();
        let gpu = self.gpu_usage.lock().unwrap().unwrap_or(0.0);
        
        // Return the maximum of all resource usages
        cpu.max(memory).max(gpu)
    }

    /// Update resource usage metrics
    fn update_metrics(&self, cpu: f64, memory: f64, gpu: Option<f64>) {
        *self.cpu_usage.lock().unwrap() = cpu;
        *self.memory_usage.lock().unwrap() = memory;
        *self.gpu_usage.lock().unwrap() = gpu;
    }
}

impl TrainingScheduler {
    /// Create a new training scheduler
    pub fn new(config: SchedulerConfig) -> Self {
        Self {
            config,
            market_schedule: Arc::new(MarketSchedule::new()),
            job_queue: Arc::new(Mutex::new(TrainingQueue::new())),
            active_jobs: Arc::new(Mutex::new(Vec::new())),
            resource_monitor: Arc::new(ResourceMonitor::new()),
        }
    }

    /// Add a training job to the queue
    pub fn schedule_job(&self, job: TrainingJob) -> Result<String, String> {
        let job_id = job.id.clone();
        
        // Emergency jobs bypass all checks
        if job.priority == Priority::Emergency {
            log::warn!("Emergency training job {} scheduled for immediate execution", job_id);
        }
        
        let mut queue = self.job_queue.lock().unwrap();
        queue.push(job);
        
        log::info!("Training job {} added to queue", job_id);
        Ok(job_id)
    }

    /// Check if we should start a new training job
    fn should_start_job(&self, job: &TrainingJob) -> bool {
        // Emergency jobs always start
        if job.priority == Priority::Emergency {
            return true;
        }
        
        // Check resource availability
        let current_usage = self.resource_monitor.get_usage();
        let market_intensity = self.market_schedule.market_intensity(None);
        
        let resource_limit = if market_intensity > self.config.market_intensity_threshold {
            self.config.market_hours_resource_limit
        } else {
            self.config.off_hours_resource_limit
        };
        
        if current_usage >= resource_limit {
            log::debug!(
                "Resource usage ({:.1}%) exceeds limit ({:.1}%) for priority {:?}",
                current_usage * 100.0,
                resource_limit * 100.0,
                job.priority
            );
            return false;
        }
        
        // Check market conditions for non-critical jobs
        match job.priority {
            Priority::Critical | Priority::Emergency => true,
            Priority::High => market_intensity < 0.7,
            Priority::Normal => market_intensity < self.config.market_intensity_threshold,
            Priority::Low => market_intensity < 0.1,
        }
    }

    /// Get the next suitable time to run a job
    pub fn next_execution_time(&self, job: &TrainingJob) -> DateTime<Utc> {
        // Emergency and critical jobs run immediately
        if matches!(job.priority, Priority::Emergency | Priority::Critical) {
            return Utc::now();
        }
        
        // For other jobs, find next training window
        let (window_start, _) = self.market_schedule
            .next_training_window(self.config.min_training_window_hours);
        
        window_start
    }

    /// Start the scheduler loop
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let check_interval = StdDuration::from_secs(self.config.check_interval_secs);
        let mut interval = interval(check_interval);
        
        log::info!("Training scheduler started with {} max concurrent jobs", 
                  self.config.max_concurrent_jobs);
        
        loop {
            interval.tick().await;
            
            // Update resource metrics (in production, this would query actual metrics)
            self.update_resource_metrics();
            
            // Process job queue
            self.process_queue().await?;
            
            // Clean up completed jobs
            self.cleanup_completed_jobs();
            
            // Log status
            self.log_status();
        }
    }

    /// Process the job queue and start eligible jobs
    async fn process_queue(&self) -> Result<(), Box<dyn std::error::Error>> {
        let active_count = self.active_jobs.lock().unwrap().len();
        
        if active_count >= self.config.max_concurrent_jobs {
            return Ok(());
        }
        
        let mut queue = self.job_queue.lock().unwrap();
        let mut jobs_to_start = Vec::new();
        
        // Find jobs that can start
        while active_count + jobs_to_start.len() < self.config.max_concurrent_jobs {
            if let Some(job) = queue.peek() {
                if self.should_start_job(job) {
                    if let Some(job) = queue.pop() {
                        jobs_to_start.push(job);
                    }
                } else {
                    // If the highest priority job can't start, skip checking others
                    break;
                }
            } else {
                break;
            }
        }
        
        drop(queue); // Release lock before starting jobs
        
        // Start the selected jobs
        for job in jobs_to_start {
            self.start_job(job).await?;
        }
        
        Ok(())
    }

    /// Start a training job
    async fn start_job(&self, job: TrainingJob) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Starting training job {}: {}", job.id, job.description);
        
        // Add to active jobs
        self.active_jobs.lock().unwrap().push(job.clone());
        
        // In production, this would actually start the training process
        // For now, we'll simulate it
        let job_id = job.id.clone();
        let active_jobs = self.active_jobs.clone();
        
        tokio::spawn(async move {
            // Simulate training
            tokio::time::sleep(StdDuration::from_secs(60)).await;
            
            // Mark as completed
            let mut jobs = active_jobs.lock().unwrap();
            jobs.retain(|j| j.id != job_id);
            
            log::info!("Training job {} completed", job_id);
        });
        
        Ok(())
    }

    /// Clean up completed jobs
    fn cleanup_completed_jobs(&self) {
        // In production, this would check actual job status
        // For now, completed jobs are already removed in start_job
    }

    /// Update resource metrics (mock implementation)
    fn update_resource_metrics(&self) {
        // In production, this would query actual system metrics
        // For now, we'll use mock values
        let market_intensity = self.market_schedule.market_intensity(None);
        
        // Simulate higher resource usage during market hours
        let base_usage = if market_intensity > 0.5 { 0.4 } else { 0.2 };
        let active_jobs = self.active_jobs.lock().unwrap().len() as f64;
        let job_usage = active_jobs * 0.15;
        
        self.resource_monitor.update_metrics(
            (base_usage + job_usage).min(1.0),
            (base_usage + job_usage * 0.8).min(1.0),
            Some((job_usage * 1.2).min(1.0)),
        );
    }

    /// Log current scheduler status
    fn log_status(&self) {
        let queue_size = self.job_queue.lock().unwrap().len();
        let active_count = self.active_jobs.lock().unwrap().len();
        let market_intensity = self.market_schedule.market_intensity(None);
        let resource_usage = self.resource_monitor.get_usage();
        
        log::debug!(
            "Scheduler status - Queue: {}, Active: {}/{}, Market: {:.1}%, Resources: {:.1}%",
            queue_size,
            active_count,
            self.config.max_concurrent_jobs,
            market_intensity * 100.0,
            resource_usage * 100.0
        );
    }

    /// Force immediate execution of a job (for emergencies)
    pub async fn force_execute(&self, job_id: &str) -> Result<(), String> {
        let mut queue = self.job_queue.lock().unwrap();
        
        // Find and remove the job from queue
        let job_index = queue.jobs()
            .iter()
            .position(|j| j.id == job_id)
            .ok_or_else(|| format!("Job {} not found in queue", job_id))?;
        
        let mut job = queue.jobs().remove(job_index);
        
        // Upgrade priority to emergency
        job.priority = Priority::Emergency;
        
        drop(queue);
        
        // Start immediately
        self.start_job(job).await
            .map_err(|e| format!("Failed to start job: {}", e))?;
        
        Ok(())
    }

    /// Get current queue status
    pub fn get_queue_status(&self) -> QueueStatus {
        let queue = self.job_queue.lock().unwrap();
        let active_jobs = self.active_jobs.lock().unwrap();
        
        QueueStatus {
            queued_jobs: queue.len(),
            active_jobs: active_jobs.len(),
            max_concurrent: self.config.max_concurrent_jobs,
            market_intensity: self.market_schedule.market_intensity(None),
            resource_usage: self.resource_monitor.get_usage(),
            next_window: self.market_schedule
                .next_training_window(self.config.min_training_window_hours),
        }
    }
}

/// Queue status information
#[derive(Debug, Clone)]
pub struct QueueStatus {
    pub queued_jobs: usize,
    pub active_jobs: usize,
    pub max_concurrent: usize,
    pub market_intensity: f64,
    pub resource_usage: f64,
    pub next_window: (DateTime<Utc>, DateTime<Utc>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::priority_queue::ModelType;

    #[test]
    fn test_scheduler_creation() {
        let config = SchedulerConfig::default();
        let scheduler = TrainingScheduler::new(config);
        
        let status = scheduler.get_queue_status();
        assert_eq!(status.queued_jobs, 0);
        assert_eq!(status.active_jobs, 0);
    }

    #[test]
    fn test_job_scheduling() {
        let scheduler = TrainingScheduler::new(SchedulerConfig::default());
        
        let job = TrainingJob {
            id: "test-job-1".to_string(),
            model_type: ModelType::LSTM,
            description: "Test training job".to_string(),
            priority: Priority::Normal,
            created_at: Utc::now(),
            estimated_duration_secs: 3600,
            resource_requirements: Default::default(),
        };
        
        let result = scheduler.schedule_job(job);
        assert!(result.is_ok());
        
        let status = scheduler.get_queue_status();
        assert_eq!(status.queued_jobs, 1);
    }

    #[test]
    fn test_emergency_priority() {
        let scheduler = TrainingScheduler::new(SchedulerConfig::default());
        
        let emergency_job = TrainingJob {
            id: "emergency-1".to_string(),
            model_type: ModelType::MLP,
            description: "Emergency retraining".to_string(),
            priority: Priority::Emergency,
            created_at: Utc::now(),
            estimated_duration_secs: 1800,
            resource_requirements: Default::default(),
        };
        
        // Emergency jobs should always be eligible to start
        assert!(scheduler.should_start_job(&emergency_job));
    }
}