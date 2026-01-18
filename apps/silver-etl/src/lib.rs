//! Silver ETL Library
//!
//! Config-driven ETL for transforming Bronze layer Parquet data
//! into Silver layer TimescaleDB tables.
//!
//! ## Architecture
//!
//! The Silver ETL follows the config-driven pattern established in dp-006:
//!
//! 1. **Configuration Loading**: Stream configs with `silver_etl` sections
//!    define field mappings, transforms, and DQ rules
//!
//! 2. **SQL Generation**: Config is translated to DuckDB SQL that:
//!    - Reads from Bronze Parquet files via `read_parquet()`
//!    - Applies transforms and DQ checks
//!    - Writes to TimescaleDB via postgres extension
//!
//! 3. **DQ Transparency**: All quality issues are flagged, not hidden.
//!    The `dq_flags TEXT[]` column captures rule violations.
//!
//! ## Modules
//!
//! - `config`: Configuration loading from etcd/files
//! - `sql_gen`: SQL generation from config
//! - `dq`: DQ rule evaluation and flag generation
//! - `etl`: ETL execution engine
//! - `metrics`: Prometheus metrics
//! - `persistence`: ETL run statistics persistence (dp-011)
//! - `pre_transform`: Pre-transform stage for columnar array data (dp-007)

// Module declarations
pub mod config;
pub mod daemon;
pub mod dq;
pub mod etl;
pub mod metrics;
pub mod persistence;
pub mod pre_transform;
pub mod schema_gen;
pub mod sql_gen;

// Re-export main types for library consumers
pub use config::ConfigLoader;
pub use daemon::{DaemonConfig, DaemonError, DaemonRunner, EtlExecutor, RealEtlExecutor};
pub use dq::DqSqlGenerator;
pub use etl::{BronzeRawData, EtlError, EtlRunner, EtlStats};
pub use metrics::EtlMetrics;
pub use persistence::{
    DuckDbRunPersistence, EtlRunMode, EtlRunPersistence, EtlRunRecord, EtlRunStatus,
    NoOpPersistence, PersistenceError,
};
pub use pre_transform::{
    apply_pre_transform, build_parser, build_parser_from_config, create_temp_table,
    PreTransformError, PreTransformResult,
};
pub use schema_gen::{SchemaError, SchemaGenerator};
pub use sql_gen::{SqlGenError, SqlGenerator};
