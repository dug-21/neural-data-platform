//! Multi-stream ingestion coordination
//!
//! This module provides coordination for multiple ingestion sources:
//! - IngestionCoordinator: Owns the mpsc channel and orchestrates sources
//! - SourceManager: Spawns and manages source lifecycles

pub mod ingestion_coordinator;
pub mod source_manager;

pub use ingestion_coordinator::IngestionCoordinator;
pub use source_manager::SourceManager;
