//! ML Enhancement Module for Phase 3
//!
//! This module contains advanced ML enhancements including:
//! - High-performance model checkpointing system
//! - Automatic rollback with Byzantine consensus
//! - Performance monitoring integration
//! - Training pipeline optimization

pub mod checkpoint_system;

pub use checkpoint_system::{
    CheckpointManager, CheckpointConfig, CheckpointId, CheckpointMetadata,
    PerformanceMetrics, RollbackDecision, ByzantineConsensus,
    DefaultByzantineConsensus,
};

use anyhow::Result;
use std::sync::Arc;
use crate::daa::autonomous_training::AutonomousTrainingEngine;

/// ML Enhancement facade for easy integration
pub struct MLEnhancementSystem {
    checkpoint_manager: Arc<CheckpointManager>,
    training_engine: Arc<AutonomousTrainingEngine>,
}

impl MLEnhancementSystem {
    /// Create new ML enhancement system
    pub fn new(
        checkpoint_config: CheckpointConfig,
        training_engine: AutonomousTrainingEngine,
    ) -> Result<Self> {
        let consensus = Arc::new(DefaultByzantineConsensus::new("main_node".to_string()));
        let checkpoint_manager = Arc::new(CheckpointManager::new(checkpoint_config, consensus)?);

        Ok(Self {
            checkpoint_manager,
            training_engine: Arc::new(training_engine),
        })
    }

    /// Get checkpoint manager
    pub fn checkpoint_manager(&self) -> Arc<CheckpointManager> {
        Arc::clone(&self.checkpoint_manager)
    }

    /// Get training engine
    pub fn training_engine(&self) -> Arc<AutonomousTrainingEngine> {
        Arc::clone(&self.training_engine)
    }

    /// Initialize the ML enhancement system
    pub async fn initialize(&self) -> Result<()> {
        tracing::info!("Initializing ML Enhancement System");
        
        // System is ready to handle checkpoints and rollbacks
        tracing::info!("ML Enhancement System initialized successfully");
        
        Ok(())
    }
}