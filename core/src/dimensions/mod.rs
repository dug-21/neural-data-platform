//! Dimension loading module
//!
//! This module provides functionality for loading dimension (reference) data
//! directly to Silver layer, bypassing Bronze.
//!
//! # Architecture (DP-013)
//!
//! Dimensions are metadata/lookup tables that enrich timeseries observations:
//! - entity_context: Maps ndp_id to friendly names, locations, correlations
//! - sensor_metadata: Calibration data, installation dates
//! - location_hierarchy: Geographic/organizational structure
//!
//! Unlike timeseries, dimensions:
//! - Load via TRUNCATE+INSERT or UPSERT (not append-only)
//! - Don't need raw payload preservation (Bronze)
//! - Change infrequently (config-managed, not streamed)
//!
//! # Configuration-Driven DDL (DP-013)
//!
//! All table structures come from YAML config files. There are NO hardcoded
//! SQL files for specific dimension tables. The `DdlGenerator` reads
//! `DimensionConfig` and produces:
//!
//! - CREATE TABLE statements with proper types and constraints
//! - CREATE INDEX statements (regular and unique)
//! - INSERT and UPSERT statements for loading
//! - TRUNCATE statements for the truncate_and_load strategy
//!
//! # Usage
//!
//! ```rust,ignore
//! use platform_core::dimensions::{CsvDimensionLoader, DimensionLoader, DdlGenerator};
//! use platform_core::types::dimension_config::DimensionConfig;
//!
//! // Load config (single source of truth)
//! let config: DimensionConfig = serde_yaml::from_str(yaml)?;
//!
//! // Generate DDL from config
//! let create_table = DdlGenerator::generate_create_table(&config);
//! let indexes = DdlGenerator::generate_indexes(&config);
//!
//! // Create loader
//! let loader = CsvDimensionLoader::new(config);
//!
//! // Validate (dry run)
//! let stats = loader.dry_run().await?;
//! println!("Would load {} rows", stats.rows_loaded);
//!
//! // Load to TimescaleDB (requires 'timescale' feature)
//! #[cfg(feature = "timescale")]
//! {
//!     use platform_core::dimensions::TimescaleDimensionLoader;
//!     loader.load(&pool).await?;
//! }
//! ```

pub mod ddl;
pub mod error;
pub mod loader;

// Re-export main types
pub use ddl::DdlGenerator;
pub use error::DimensionError;
pub use loader::{CsvDimensionLoader, DimensionLoadStats, DimensionLoader, DimensionResult};

// Re-export timescale extension when feature is enabled
#[cfg(feature = "timescale")]
pub use loader::TimescaleDimensionLoader;
