//! Bronze layer storage abstraction module.
//!
//! Provides the `BronzeStorage` trait and implementations for accessing
//! Bronze layer Parquet files. Follows the NDP Domain Adapter pattern
//! (hexagonal architecture) as defined in ADR-002.
//!
//! # Architecture
//!
//! - **Port**: `BronzeStorage` trait defines the interface
//! - **Adapter**: `LocalParquetStorage` implements for local filesystem
//! - **Future Adapter**: `S3ParquetStorage` for cloud deployment
//!
//! # Directory Structure
//!
//! Bronze layer files are organized in Hive-style partitions:
//! ```text
//! /data/raw/{stream_id}/
//!     year=YYYY/
//!         month=MM/
//!             day=DD/
//!                 data.parquet
//! ```

mod local;
mod traits;
mod types;

pub use local::LocalParquetStorage;
pub use traits::BronzeStorage;
pub use types::{FieldInfo, JsonStructure, ParquetSchemaInfo, RawPayloadStructure, StreamStorageInfo};

// Re-export mock for testing
#[cfg(test)]
pub use traits::MockBronzeStorage;
