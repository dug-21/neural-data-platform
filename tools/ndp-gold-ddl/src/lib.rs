//! ndp-gold-ddl - Gold layer DDL generation for NDP
//!
//! This crate is a thin wrapper that re-exports from `ndp_lib::gold`.
//! All Gold DDL generation logic now lives in `crates/ndp-lib/src/gold/`.
//!
//! The standalone binary (`ndp-gold-ddl`) is preserved for backward compatibility.
//! Production deployment uses `ndp gold` subcommands instead (v1.1.14+).

// Re-export the gold module's public API
pub use ndp_lib::gold::config;
pub use ndp_lib::gold::error;
pub use ndp_lib::gold::generators;
pub use ndp_lib::gold::planner;
pub use ndp_lib::gold::registry;
pub use ndp_lib::gold::validation;

// Re-export top-level types for backward compatibility
pub use ndp_lib::gold::config::{
    Action, AlignedStream, AlignmentConfig, ConfigLoader, DomainConfig, FileSystemConfigLoader,
    GoldEtlConfig, JoinStrategy, NullHandling, ObjectiveConfig, Priority, StreamConfig, StreamRef,
    StreamRole, StreamType, TargetConfig,
};

pub use ndp_lib::gold::error::{GoldDdlError, Result};

pub use ndp_lib::gold::generators::{
    generate_classification_sql, generate_gold_table_sql, AlignedViewGenerator,
    ClassificationSyncer, ContinuousAggregateGenerator, DefaultClassificationSyncer, EventsConfig,
    EventsGenerator, IEventsGenerator, ITransitionGenerator, RefreshPolicyGenerator,
    StateTransitionGenerator, TransitionConfig,
};

pub use ndp_lib::gold::registry::{FeatureConfig, FeatureGenerator, FeatureRegistry, SqlColumn};

pub use ndp_lib::gold::validation::{granularity_to_suffix, parse_granularity, parse_window};

pub use ndp_lib::gold::planner::{CaAction, SyncPlan, SyncPlanner};

/// Backward-compatible db module re-exports.
///
/// The original ndp-gold-ddl had its own DbClient and PostgresClient.
/// These now come from ndp_lib::db (shared) and ndp_lib::gold::db (CaChecker).
pub mod db {
    pub use ndp_lib::db::{DbClient, PostgresClient};
    pub use ndp_lib::gold::db::{CaChecker, CaInfo, PostgresCaChecker};
    pub use ndp_lib::gold::error::GoldDdlError as DbError;
}
