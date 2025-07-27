//! Decentralized Autonomous Agents (DAA) module
//! 
//! This module provides the core DAA coordination functionality including
//! autonomous decision making and neural training recognition.

pub mod autonomous_training;

// Re-export commonly used types
pub use autonomous_training::{
    AutonomousTrainingEngine, DAATrainingIntegration, TrainingTriggerConfig,
    PerformanceSnapshot, TrainingDecision, TrainingDecisionRecord
};