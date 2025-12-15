//! AIR-004: Multi-Stream Ingestion Coordinator
//!
//! This module coordinates multiple data sources, validates incoming data,
//! and routes it to appropriate storage layers.

pub mod router;
pub mod source_manager;
pub mod ingestion_coordinator;

pub use router::IngestionRouter;
pub use source_manager::SourceManager;
pub use ingestion_coordinator::IngestionCoordinator;
