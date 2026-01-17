//! Storage abstraction module for Bronze, Silver, Dictionary, and ETL layers.
//!
//! Provides storage traits and implementations following the NDP Domain Adapter
//! pattern (hexagonal architecture) as defined in ADR-002.
//!
//! # Architecture
//!
//! ## Traits (Ports)
//!
//! - `BronzeStorage` - Access Bronze layer Parquet files
//! - `SilverStorage` - Access Silver layer TimescaleDB tables (dp-010)
//! - `DictionaryStore` - Access cross-layer data dictionary (dp-010)
//! - `EtlRunStore` - Access ETL run history (dp-010)
//!
//! ## Implementations (Adapters)
//!
//! - `LocalParquetStorage` - Local filesystem adapter for Bronze
//! - `TimescaleSilverStorage` - TimescaleDB adapter for Silver (dp-010)
//! - `TimescaleDictionaryStore` - TimescaleDB adapter for data dictionary (dp-010)
//! - `TimescaleEtlRunStore` - TimescaleDB adapter for ETL history (dp-010)
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
mod timescale_dictionary;
mod timescale_etl;
mod timescale_silver;
mod traits;
mod types;

// Bronze layer exports
pub use local::LocalParquetStorage;
pub use traits::BronzeStorage;
pub use types::{ParquetSchemaInfo, StreamStorageInfo};

// TimescaleDB adapter exports (dp-010)
pub use timescale_dictionary::TimescaleDictionaryStore;
pub use timescale_etl::TimescaleEtlRunStore;
pub use timescale_silver::{TimescalePoolConfig, TimescaleSilverStorage};

// Silver layer exports (dp-010)
pub use traits::SilverStorage;
pub use types::{
    DqSummary, HypertableInfo, SampleFilters, SilverColumnInfo, SilverTableDescription,
    SilverTableInfo, SilverTableStats, TimeRange,
};

// Dictionary exports (dp-010)
pub use traits::DictionaryStore;
pub use types::{
    ColumnDescription, DictionaryEntry, DqRuleInfo, LineageSource, LineageTrace, SourceInfo,
    ValidationRange,
};

// ETL exports (dp-010)
pub use traits::EtlRunStore;
pub use types::{
    EtlHistoryResult, EtlRunDetail, EtlRunInfo, EtlStreamStatus, FreshnessEntry, FreshnessReport,
    FreshnessSummary, HistorySummary, RunStats,
};

// Re-export mocks for testing
#[cfg(test)]
pub use traits::MockBronzeStorage;

#[cfg(test)]
pub use traits::MockSilverStorage;

#[cfg(test)]
pub use traits::MockDictionaryStore;

#[cfg(test)]
pub use traits::MockEtlRunStore;
