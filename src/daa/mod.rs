//! Decentralized Autonomous Agents (DAA) module
//!
//! This module provides the core DAA coordination functionality including
//! autonomous decision making and neural training recognition.

pub mod autonomous_training;
pub mod compatibility_adapter;
pub mod enhanced_performance_snapshot;
pub mod test_compatibility;
pub mod training_scheduler;

// Phase 3: Real-time training integration
pub mod realtime_training_integration;

// Re-export commonly used types
pub use autonomous_training::{
    AutonomousTrainingEngine, DAATrainingIntegration, PerformanceSnapshot, TrainingDecision,
    TrainingDecisionRecord, TrainingTriggerConfig,
};

// Re-export enhanced types for the DAA extension
pub use enhanced_performance_snapshot::{
    EnhancedPerformanceSnapshot, DataTypeMetrics, DataTypePattern, DataQualityIssue,
    QualityIssueType, DistributionType, EnhancementMetadata,
};

// Re-export compatibility adapter
pub use compatibility_adapter::{
    EnhancedTrainingEngineAdapter, SnapshotType,
};

pub use training_scheduler::{
    DAASchedulerConfig, DAATrainingJob, DAATrainingScheduler, JobStatus, ResourceLimitConfig,
    ResourceProfile,
};

// Re-export real-time training integration
pub use realtime_training_integration::{
    DAATrainingScheduler as EnhancedDAATrainingScheduler, 
    CoordinationConfig, 
    TrainingSystemFactory,
};
