//! AIR-005: Multi-Stream Ingestion Coordinator
//!
//! This module coordinates multiple data sources, validates incoming data,
//! and routes it to appropriate storage layers.
//!
//! ## Components
//!
//! - **IngestionCoordinator**: Main coordinator that receives data from sources
//!   and routes to storage channels
//! - **SourceManager**: Manages lifecycle of multiple data sources (MQTT, HTTP, Webhook)
//! - **IngestionRouter**: Routes and validates time series points against schemas
//! - **ConfigWatcher**: DP-021 hot-reload support via etcd watch

pub mod config_watcher;
pub mod ingestion_coordinator;
pub mod router;
pub mod source_manager;

pub use config_watcher::{ConfigWatchError, ConfigWatchHandle, ConfigWatcher};
pub use ingestion_coordinator::{CoordinatorError, IngestionCoordinator};
pub use router::{DeadLetterItem, IngestionRouter, ValidationError};
pub use source_manager::{
    ConfigChangeType, HotReloadResult, SourceHealth, SourceManager, SourceManagerError,
};
