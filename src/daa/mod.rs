//! Decentralized Autonomous Agents (DAA) module
//!
//! This module provides the core DAA coordination functionality including
//! autonomous decision making and neural training recognition.

pub mod autonomous_training;
pub mod training_scheduler;

// Re-export commonly used types
pub use autonomous_training::{
    AutonomousTrainingEngine, DAATrainingIntegration, PerformanceSnapshot, TrainingDecision,
    TrainingDecisionRecord, TrainingTriggerConfig,
};

pub use training_scheduler::{
    DAASchedulerConfig, DAATrainingJob, DAATrainingScheduler, JobStatus, ResourceLimitConfig,
    ResourceProfile,
};
