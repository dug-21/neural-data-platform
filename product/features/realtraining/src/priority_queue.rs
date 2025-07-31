//! Priority queue for training job management
//! 
//! Implements a priority-based queue system for managing training jobs
//! with support for different priority levels and resource requirements.

use chrono::{DateTime, Utc};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Training job priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Lowest priority - can wait for optimal conditions
    Low = 0,
    /// Normal priority - scheduled during off-hours
    Normal = 1,
    /// High priority - runs with relaxed constraints
    High = 2,
    /// Critical priority - runs as soon as resources allow
    Critical = 3,
    /// Emergency priority - runs immediately regardless of conditions
    Emergency = 4,
}

impl Priority {
    /// Parse priority from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "low" => Some(Priority::Low),
            "normal" => Some(Priority::Normal),
            "high" => Some(Priority::High),
            "critical" => Some(Priority::Critical),
            "emergency" => Some(Priority::Emergency),
            _ => None,
        }
    }
}

/// Model types that can be trained
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelType {
    /// Multi-Layer Perceptron
    MLP,
    /// Long Short-Term Memory
    LSTM,
    /// Transformer
    Transformer,
    /// Ensemble model
    Ensemble,
    /// Custom model type
    Custom(String),
}

/// Resource requirements for a training job
#[derive(Debug, Clone)]
pub struct ResourceRequirements {
    /// Minimum CPU cores required
    pub min_cpu_cores: usize,
    /// Minimum memory in GB
    pub min_memory_gb: f64,
    /// Whether GPU is required
    pub requires_gpu: bool,
    /// Minimum GPU memory in GB (if GPU required)
    pub min_gpu_memory_gb: Option<f64>,
    /// Estimated disk space in GB
    pub disk_space_gb: f64,
}

impl Default for ResourceRequirements {
    fn default() -> Self {
        Self {
            min_cpu_cores: 2,
            min_memory_gb: 4.0,
            requires_gpu: false,
            min_gpu_memory_gb: None,
            disk_space_gb: 1.0,
        }
    }
}

/// Training job representation
#[derive(Debug, Clone)]
pub struct TrainingJob {
    /// Unique job identifier
    pub id: String,
    /// Model type being trained
    pub model_type: ModelType,
    /// Job description
    pub description: String,
    /// Job priority
    pub priority: Priority,
    /// When the job was created
    pub created_at: DateTime<Utc>,
    /// Estimated training duration in seconds
    pub estimated_duration_secs: u64,
    /// Resource requirements
    pub resource_requirements: ResourceRequirements,
}

impl TrainingJob {
    /// Create a new training job
    pub fn new(
        id: String,
        model_type: ModelType,
        description: String,
        priority: Priority,
    ) -> Self {
        Self {
            id,
            model_type,
            description,
            priority,
            created_at: Utc::now(),
            estimated_duration_secs: 3600, // Default 1 hour
            resource_requirements: ResourceRequirements::default(),
        }
    }

    /// Set resource requirements
    pub fn with_resources(mut self, requirements: ResourceRequirements) -> Self {
        self.resource_requirements = requirements;
        self
    }

    /// Set estimated duration
    pub fn with_duration(mut self, duration_secs: u64) -> Self {
        self.estimated_duration_secs = duration_secs;
        self
    }

    /// Get age of the job
    pub fn age(&self) -> chrono::Duration {
        Utc::now() - self.created_at
    }

    /// Calculate job score for prioritization
    /// Higher score = higher priority
    pub fn priority_score(&self) -> i64 {
        let base_priority = self.priority as i64 * 1_000_000;
        let age_bonus = self.age().num_seconds().min(86400); // Cap at 1 day
        base_priority + age_bonus
    }
}

/// Wrapper for heap ordering (higher priority first)
#[derive(Debug, Clone)]
struct HeapJob(TrainingJob);

impl PartialEq for HeapJob {
    fn eq(&self, other: &Self) -> bool {
        self.0.priority_score() == other.0.priority_score()
    }
}

impl Eq for HeapJob {}

impl PartialOrd for HeapJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HeapJob {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher scores should come first
        self.0.priority_score().cmp(&other.0.priority_score())
    }
}

/// Priority queue for training jobs
#[derive(Debug)]
pub struct TrainingQueue {
    heap: BinaryHeap<HeapJob>,
}

impl TrainingQueue {
    /// Create a new empty queue
    pub fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
        }
    }

    /// Add a job to the queue
    pub fn push(&mut self, job: TrainingJob) {
        self.heap.push(HeapJob(job));
    }

    /// Remove and return the highest priority job
    pub fn pop(&mut self) -> Option<TrainingJob> {
        self.heap.pop().map(|h| h.0)
    }

    /// Peek at the highest priority job without removing it
    pub fn peek(&self) -> Option<&TrainingJob> {
        self.heap.peek().map(|h| &h.0)
    }

    /// Get the number of jobs in the queue
    pub fn len(&self) -> usize {
        self.heap.len()
    }

    /// Check if the queue is empty
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    /// Get all jobs in priority order (does not remove them)
    pub fn jobs(&self) -> Vec<&TrainingJob> {
        let mut jobs: Vec<_> = self.heap.iter().map(|h| &h.0).collect();
        jobs.sort_by(|a, b| b.priority_score().cmp(&a.priority_score()));
        jobs
    }

    /// Remove a specific job by ID
    pub fn remove_by_id(&mut self, job_id: &str) -> Option<TrainingJob> {
        let jobs: Vec<_> = self.heap.drain().collect();
        let mut removed = None;
        
        for heap_job in jobs {
            if heap_job.0.id == job_id {
                removed = Some(heap_job.0);
            } else {
                self.heap.push(heap_job);
            }
        }
        
        removed
    }

    /// Update job priority
    pub fn update_priority(&mut self, job_id: &str, new_priority: Priority) -> bool {
        if let Some(mut job) = self.remove_by_id(job_id) {
            job.priority = new_priority;
            self.push(job);
            true
        } else {
            false
        }
    }

    /// Get jobs by priority level
    pub fn jobs_by_priority(&self, priority: Priority) -> Vec<&TrainingJob> {
        self.heap
            .iter()
            .map(|h| &h.0)
            .filter(|job| job.priority == priority)
            .collect()
    }

    /// Clear all jobs from the queue
    pub fn clear(&mut self) {
        self.heap.clear();
    }

    /// Get queue statistics
    pub fn stats(&self) -> QueueStats {
        let mut stats = QueueStats::default();
        
        for heap_job in &self.heap {
            let job = &heap_job.0;
            match job.priority {
                Priority::Low => stats.low_priority += 1,
                Priority::Normal => stats.normal_priority += 1,
                Priority::High => stats.high_priority += 1,
                Priority::Critical => stats.critical_priority += 1,
                Priority::Emergency => stats.emergency_priority += 1,
            }
            
            if job.resource_requirements.requires_gpu {
                stats.gpu_required += 1;
            }
        }
        
        stats.total = self.heap.len();
        stats
    }
}

impl Default for TrainingQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Queue statistics
#[derive(Debug, Default, Clone)]
pub struct QueueStats {
    pub total: usize,
    pub low_priority: usize,
    pub normal_priority: usize,
    pub high_priority: usize,
    pub critical_priority: usize,
    pub emergency_priority: usize,
    pub gpu_required: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Emergency > Priority::Critical);
        assert!(Priority::Critical > Priority::High);
        assert!(Priority::High > Priority::Normal);
        assert!(Priority::Normal > Priority::Low);
    }

    #[test]
    fn test_priority_queue() {
        let mut queue = TrainingQueue::new();
        
        // Add jobs with different priorities
        queue.push(TrainingJob::new(
            "low-1".to_string(),
            ModelType::MLP,
            "Low priority job".to_string(),
            Priority::Low,
        ));
        
        queue.push(TrainingJob::new(
            "high-1".to_string(),
            ModelType::LSTM,
            "High priority job".to_string(),
            Priority::High,
        ));
        
        queue.push(TrainingJob::new(
            "normal-1".to_string(),
            ModelType::Transformer,
            "Normal priority job".to_string(),
            Priority::Normal,
        ));
        
        // Should get high priority first
        let first = queue.pop().unwrap();
        assert_eq!(first.id, "high-1");
        
        // Then normal
        let second = queue.pop().unwrap();
        assert_eq!(second.id, "normal-1");
        
        // Then low
        let third = queue.pop().unwrap();
        assert_eq!(third.id, "low-1");
    }

    #[test]
    fn test_job_aging() {
        let job = TrainingJob::new(
            "test-1".to_string(),
            ModelType::MLP,
            "Test job".to_string(),
            Priority::Normal,
        );
        
        // Job should have non-zero age immediately
        assert!(job.age().num_seconds() >= 0);
        
        // Priority score should include age
        let initial_score = job.priority_score();
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(job.priority_score() >= initial_score);
    }

    #[test]
    fn test_remove_by_id() {
        let mut queue = TrainingQueue::new();
        
        queue.push(TrainingJob::new("job-1".to_string(), ModelType::MLP, "Job 1".to_string(), Priority::Normal));
        queue.push(TrainingJob::new("job-2".to_string(), ModelType::LSTM, "Job 2".to_string(), Priority::High));
        queue.push(TrainingJob::new("job-3".to_string(), ModelType::Transformer, "Job 3".to_string(), Priority::Low));
        
        assert_eq!(queue.len(), 3);
        
        let removed = queue.remove_by_id("job-2");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, "job-2");
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn test_queue_stats() {
        let mut queue = TrainingQueue::new();
        
        queue.push(TrainingJob::new("1".to_string(), ModelType::MLP, "".to_string(), Priority::Low));
        queue.push(TrainingJob::new("2".to_string(), ModelType::MLP, "".to_string(), Priority::Low));
        queue.push(TrainingJob::new("3".to_string(), ModelType::MLP, "".to_string(), Priority::Normal));
        queue.push(TrainingJob::new("4".to_string(), ModelType::MLP, "".to_string(), Priority::High));
        
        let mut gpu_job = TrainingJob::new("5".to_string(), ModelType::Transformer, "".to_string(), Priority::Critical);
        gpu_job.resource_requirements.requires_gpu = true;
        queue.push(gpu_job);
        
        let stats = queue.stats();
        assert_eq!(stats.total, 5);
        assert_eq!(stats.low_priority, 2);
        assert_eq!(stats.normal_priority, 1);
        assert_eq!(stats.high_priority, 1);
        assert_eq!(stats.critical_priority, 1);
        assert_eq!(stats.gpu_required, 1);
    }
}