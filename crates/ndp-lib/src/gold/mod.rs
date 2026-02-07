//! Gold layer DDL generation for NDP
//!
//! This module provides DDL generation for:
//! - TimescaleDB continuous aggregates for individual streams
//! - Aligned materialized views for cross-stream correlation
//! - State transition views
//! - Events infrastructure
//!
//! ## Architecture
//!
//! The module follows a modular design:
//! - `config`: Configuration loading and types
//! - `generators`: SQL DDL generators
//! - `planner`: Sync planning (idempotent DDL)
//! - `registry`: Feature registry (lag, rolling, trend)
//! - `validation`: Configuration validation
//! - `error`: Structured error types
//!
//! ## Public API
//!
//! High-level convenience functions for the CLI:
//! - [`generate_stream`] - Generate DDL for a single stream (no DB needed)
//! - [`generate_domain`] - Generate DDL for a domain (aligned views, events)
//! - [`sync_stream`] - Sync DDL to DB (idempotent, needs DB connection)
//! - [`sync_domain`] - Generate domain DDL with sync action
//! - [`recreate_stream`] - Generate DDL with recreate action

pub mod config;
pub mod db;
pub mod error;
pub mod generators;
pub mod planner;
pub mod registry;
pub mod validation;

// Re-exports for convenient access
pub use config::{
    Action, AlignedStream, AlignmentConfig, ConfigLoader, DomainConfig, FileSystemConfigLoader,
    GoldEtlConfig, JoinStrategy, NullHandling, ObjectiveConfig, Priority, StreamConfig, StreamRef,
    StreamRole, StreamType, TargetConfig,
};

pub use error::{GoldDdlError, Result};

pub use generators::{
    generate_classification_sql, generate_gold_table_sql, AlignedViewGenerator,
    ClassificationSyncer, ContinuousAggregateGenerator, DefaultClassificationSyncer, EventsConfig,
    EventsGenerator, IEventsGenerator, ITransitionGenerator, RefreshPolicyGenerator,
    StateTransitionGenerator, TransitionConfig,
};

pub use registry::{FeatureConfig, FeatureGenerator, FeatureRegistry, SqlColumn};

pub use validation::{validate_gold_config, ConfigValidator};

pub use db::{CaChecker, CaInfo, PostgresCaChecker};

pub use planner::{CaAction, SyncPlan, SyncPlanner};

// ---------------------------------------------------------------------------
// Public convenience API (used by ndp-cli and ndp-gold-ddl)
// ---------------------------------------------------------------------------

/// Options for Gold DDL generation.
pub struct GenerateOptions {
    /// Include state transitions view DDL.
    pub transitions: bool,
    /// Include events infrastructure DDL.
    pub events: bool,
    /// Enable verbose diagnostic output.
    pub verbose: bool,
}

/// Generate DDL for a single stream (no database connection required).
///
/// Returns the SQL DDL as a string. If `opts.transitions` is true, generates
/// a state-transition view instead of continuous aggregates.
pub fn generate_stream(
    loader: &impl ConfigLoader,
    stream_id: &str,
    opts: &GenerateOptions,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let stream_config = loader.load_stream_config(stream_id)?;
    let gold_etl = stream_config
        .gold_etl
        .as_ref()
        .ok_or_else(|| format!("Stream '{}' has no gold_etl configuration", stream_id))?;

    if !gold_etl.enabled {
        return Err(format!("Stream '{}' has gold_etl.enabled = false", stream_id).into());
    }

    if opts.transitions {
        let transition_config = TransitionConfig::from_stream_config(&stream_config)
            .unwrap_or_else(|| TransitionConfig::new("state", "ndp_id"));
        let generator = StateTransitionGenerator::from_stream_config(&stream_config)?;
        let sql = generator.generate(&transition_config, Action::Sync)?;
        Ok(sql)
    } else {
        let generator = ContinuousAggregateGenerator::from_stream_config(&stream_config)?;
        let sql = generator.generate(gold_etl, Action::Sync)?;
        Ok(sql)
    }
}

/// Generate DDL for a domain (aligned views and optionally events).
///
/// Returns the SQL DDL as a string. If `opts.events` is true, generates
/// events infrastructure DDL instead of aligned views.
pub fn generate_domain(
    loader: &(impl ConfigLoader + Clone + 'static),
    domain_id: &str,
    opts: &GenerateOptions,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let domain_config = loader.load_domain_config(domain_id)?;

    if opts.events {
        let events_loader = loader.clone();
        let generator =
            EventsGenerator::from_domain_config(&domain_config, Box::new(events_loader));
        let sql = generator.generate(Action::Sync)?;
        Ok(sql)
    } else {
        let generator = AlignedViewGenerator::new(loader.clone());
        let sql = generator.generate(&domain_config, Action::Sync)?;
        Ok(sql)
    }
}

/// Sync Gold DDL for a stream against a real database (idempotent).
///
/// Connects to the database, checks which continuous aggregates already exist,
/// and generates DDL only for missing ones.
///
/// The caller should create a `PostgresCaChecker` from their DB client
/// and pass it as the `checker` argument.
pub async fn sync_stream(
    loader: &impl ConfigLoader,
    stream_id: &str,
    checker: &(impl CaChecker + Send + Sync),
    _opts: &crate::types::SyncOptions,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let stream_config = loader.load_stream_config(stream_id)?;
    let gold_etl = stream_config
        .gold_etl
        .as_ref()
        .ok_or_else(|| format!("Stream '{}' has no gold_etl configuration", stream_id))?;

    if !gold_etl.enabled {
        return Err(format!("Stream '{}' has gold_etl.enabled = false", stream_id).into());
    }

    let planner = SyncPlanner::new(checker, &stream_config);
    let plan = planner.plan(gold_etl).await?;

    Ok(plan.to_ddl())
}

/// Sync Gold DDL for a domain (generate aligned view with sync action).
///
/// Domain sync does not use database checks; aligned views use
/// `DO $$ IF NOT EXISTS` for idempotency.
pub fn sync_domain(
    loader: &(impl ConfigLoader + Clone + 'static),
    domain_id: &str,
    _opts: &crate::types::SyncOptions,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let domain_config = loader.load_domain_config(domain_id)?;
    let generator = AlignedViewGenerator::new(loader.clone());
    let sql = generator.generate(&domain_config, Action::Sync)?;
    Ok(sql)
}

/// Recreate Gold DDL for a stream (drop and create).
///
/// Generates DDL with Action::Recreate, which includes DROP statements.
pub fn recreate_stream(
    loader: &impl ConfigLoader,
    stream_id: &str,
    _opts: &GenerateOptions,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let stream_config = loader.load_stream_config(stream_id)?;
    let gold_etl = stream_config
        .gold_etl
        .as_ref()
        .ok_or_else(|| format!("Stream '{}' has no gold_etl configuration", stream_id))?;

    if !gold_etl.enabled {
        return Err(format!("Stream '{}' has gold_etl.enabled = false", stream_id).into());
    }

    let generator = ContinuousAggregateGenerator::from_stream_config(&stream_config)?;
    let sql = generator.generate(gold_etl, Action::Recreate)?;
    Ok(sql)
}
