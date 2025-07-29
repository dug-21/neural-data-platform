//! Real-time autonomous training system with market awareness
//! 
//! This crate provides a complete solution for scheduling and managing
//! neural network training jobs with awareness of market hours to minimize
//! impact on trading operations.

pub mod market_schedule;
pub mod priority_queue;
pub mod training_scheduler;

// Re-export main types
pub use market_schedule::{Exchange, MarketSchedule, MarketStatus};
pub use priority_queue::{Priority, TrainingJob, TrainingQueue, ModelType};
pub use training_scheduler::{TrainingScheduler, SchedulerConfig, QueueStatus};

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::market_schedule::{Exchange, MarketSchedule, MarketStatus};
    pub use crate::priority_queue::{Priority, TrainingJob, ModelType};
    pub use crate::training_scheduler::{TrainingScheduler, SchedulerConfig};
}