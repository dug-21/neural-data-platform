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
pub mod embeddings;
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

pub use validation::{granularity_to_suffix, parse_granularity, parse_window};

pub use db::{CaChecker, CaInfo, PostgresCaChecker};

pub use planner::{CaAction, SyncPlan, SyncPlanner};

// ---------------------------------------------------------------------------
// Cross-cutting validation helper
// ---------------------------------------------------------------------------

/// Run semantic validation on a stream config before DDL generation.
///
/// Serializes the typed `StreamConfig` to `serde_json::Value` and delegates
/// to `crate::validate::validate_gold_etl`.  This re-serialization approach
/// avoids maintaining two validation code paths (one for JSON, one for structs).
///
/// Returns `Ok(())` if there are no errors (warnings are allowed).
/// Returns `Err` with a description of all validation errors otherwise.
fn run_stream_validation(
    config: &StreamConfig,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let json_value = serde_json::to_value(config)
        .map_err(|e| format!("Failed to serialize stream config for validation: {}", e))?;

    let validation_errors = crate::validate::validate_gold_etl(&json_value);

    // Filter: only block on errors, not warnings
    let blocking_errors: Vec<_> = validation_errors
        .iter()
        .filter(|e| e.severity == crate::validate::Severity::Error)
        .collect();

    if blocking_errors.is_empty() {
        return Ok(());
    }

    let mut msg = format!(
        "Validation failed for stream '{}' with {} error(s):\n",
        config.stream_id,
        blocking_errors.len()
    );
    for err in &blocking_errors {
        msg.push_str(&format!(
            "  - [{}] {}: {}\n",
            err.code, err.path, err.message
        ));
    }

    Err(msg.into())
}

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
/// When `opts.validate` is `true` (the default), semantic validation runs
/// before any DDL is generated.  If validation produces errors the function
/// returns early without touching the database.
///
/// The caller should create a `PostgresCaChecker` from their DB client
/// and pass it as the `checker` argument.
pub async fn sync_stream(
    loader: &impl ConfigLoader,
    stream_id: &str,
    checker: &impl CaChecker,
    opts: &crate::types::SyncOptions,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let stream_config = loader.load_stream_config(stream_id)?;

    // Cross-cutting validation: run semantic checks before DDL generation
    if opts.validate {
        run_stream_validation(&stream_config)?;
    }

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
///
/// When `opts.validate` is `true` (the default), semantic validation runs
/// on each stream referenced by the domain before DDL generation.
pub fn sync_domain(
    loader: &(impl ConfigLoader + Clone + 'static),
    domain_id: &str,
    opts: &crate::types::SyncOptions,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let domain_config = loader.load_domain_config(domain_id)?;

    // Cross-cutting validation: validate each stream's gold_etl config
    if opts.validate {
        for stream_ref in &domain_config.streams {
            if let Ok(stream_config) = loader.load_stream_config(&stream_ref.stream_id) {
                // Only validate streams that have gold_etl enabled
                if stream_config.gold_etl.as_ref().is_some_and(|g| g.enabled) {
                    run_stream_validation(&stream_config)?;
                }
            }
        }
    }

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

// =============================================================================
// Tests — Cross-cutting validation (ops-003 Steps C+D)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gold::config::types::{
        AggregatesConfig, FieldConfig, FieldMetricsConfig, GoldEtlConfig,
    };
    use crate::gold::db::{CaChecker, CaInfo};
    use std::collections::HashMap;

    /// A no-op CaChecker for testing sync_stream without a real database.
    struct FakeCaChecker;

    #[async_trait::async_trait]
    impl CaChecker for FakeCaChecker {
        async fn ca_exists(
            &self,
            _schema: &str,
            _name: &str,
        ) -> std::result::Result<bool, error::GoldDdlError> {
            Ok(false)
        }

        async fn get_ca_info(
            &self,
            _schema: &str,
            _name: &str,
        ) -> std::result::Result<Option<CaInfo>, error::GoldDdlError> {
            Ok(None)
        }

        async fn list_cas_in_schema(
            &self,
            _schema: &str,
        ) -> std::result::Result<Vec<CaInfo>, error::GoldDdlError> {
            Ok(vec![])
        }

        async fn refresh_policy_exists(
            &self,
            _schema: &str,
            _name: &str,
        ) -> std::result::Result<bool, error::GoldDdlError> {
            Ok(false)
        }
    }

    /// A fake ConfigLoader backed by in-memory configs.
    #[derive(Clone)]
    struct FakeConfigLoader {
        streams: HashMap<String, StreamConfig>,
    }

    impl ConfigLoader for FakeConfigLoader {
        fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig> {
            self.streams
                .get(stream_id)
                .cloned()
                .ok_or_else(|| GoldDdlError::ConfigNotFound {
                    path: stream_id.to_string(),
                })
        }

        fn load_domain_config(&self, _domain_id: &str) -> Result<config::domain::DomainConfig> {
            Err(GoldDdlError::ConfigNotFound {
                path: "test-domain".to_string(),
            })
        }
    }

    fn valid_stream_config() -> StreamConfig {
        use crate::gold::config::types::SilverEtlConfig;

        StreamConfig {
            stream_id: "test-stream".to_string(),
            stream_type: None,
            fields: vec![
                FieldConfig {
                    name: "pm25".to_string(),
                    field_type: "float".to_string(),
                },
                FieldConfig {
                    name: "co2".to_string(),
                    field_type: "int".to_string(),
                },
            ],
            silver_etl: Some(SilverEtlConfig {
                target_table: "silver.test_observations".to_string(),
                timestamp: None,
            }),
            gold_etl: Some(GoldEtlConfig {
                enabled: true,
                aggregates: Some(AggregatesConfig {
                    granularities: vec!["1 hour".to_string()],
                    fields: {
                        let mut map = HashMap::new();
                        map.insert(
                            "pm25".to_string(),
                            FieldMetricsConfig {
                                metrics: vec!["mean".to_string(), "std".to_string()],
                            },
                        );
                        map
                    },
                }),
                features: None,
                refresh_policy: None,
            }),
        }
    }

    fn invalid_stream_config() -> StreamConfig {
        StreamConfig {
            stream_id: "bad-stream".to_string(),
            stream_type: None,
            fields: vec![FieldConfig {
                name: "pm25".to_string(),
                field_type: "float".to_string(),
            }],
            silver_etl: None,
            gold_etl: Some(GoldEtlConfig {
                enabled: true,
                aggregates: Some(AggregatesConfig {
                    granularities: vec!["1 hour".to_string()],
                    fields: {
                        let mut map = HashMap::new();
                        map.insert(
                            "nonexistent_field".to_string(),
                            FieldMetricsConfig {
                                metrics: vec!["mean".to_string()],
                            },
                        );
                        map
                    },
                }),
                features: None,
                refresh_policy: None,
            }),
        }
    }

    // =========================================================================
    // Step C tests: run_stream_validation
    // =========================================================================

    #[test]
    fn test_run_stream_validation_passes_for_valid_config() {
        let config = valid_stream_config();
        let result = run_stream_validation(&config);
        assert!(
            result.is_ok(),
            "Valid config should pass validation: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_run_stream_validation_fails_for_invalid_config() {
        let config = invalid_stream_config();
        let result = run_stream_validation(&config);
        assert!(
            result.is_err(),
            "Config with nonexistent field should fail validation"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("nonexistent_field"),
            "Error message should mention the bad field, got: {}",
            msg
        );
    }

    #[test]
    fn test_run_stream_validation_skips_when_no_gold_etl() {
        let config = StreamConfig {
            stream_id: "no-gold".to_string(),
            stream_type: None,
            fields: vec![],
            silver_etl: None,
            gold_etl: None,
        };
        let result = run_stream_validation(&config);
        assert!(
            result.is_ok(),
            "Config without gold_etl should pass (nothing to validate)"
        );
    }

    // =========================================================================
    // Step D tests: sync_stream with validation wiring
    // =========================================================================

    #[tokio::test]
    async fn test_gold_sync_validates_before_ddl_generation() {
        let loader = FakeConfigLoader {
            streams: {
                let mut m = HashMap::new();
                m.insert("bad-stream".to_string(), invalid_stream_config());
                m
            },
        };
        let checker = FakeCaChecker;
        let opts = crate::types::SyncOptions {
            validate: true,
            ..Default::default()
        };

        let result = sync_stream(&loader, "bad-stream", &checker, &opts).await;
        assert!(
            result.is_err(),
            "sync_stream should fail when validate=true and config is invalid"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("nonexistent_field"),
            "Error should mention the validation failure, got: {}",
            msg
        );
    }

    #[tokio::test]
    async fn test_gold_sync_skips_validation_when_disabled() {
        let loader = FakeConfigLoader {
            streams: {
                let mut m = HashMap::new();
                m.insert("bad-stream".to_string(), invalid_stream_config());
                m
            },
        };
        let checker = FakeCaChecker;
        let opts = crate::types::SyncOptions {
            validate: false,
            ..Default::default()
        };

        // With validate=false, the sync should proceed past validation.
        // It will still fail because the field doesn't exist in gold DDL
        // generation, but the error will be from the DDL generator, not
        // from semantic validation.
        let result = sync_stream(&loader, "bad-stream", &checker, &opts).await;

        // We verify validation was skipped by checking the error is NOT
        // a "Validation failed" message.
        if let Err(e) = &result {
            let msg = e.to_string();
            assert!(
                !msg.contains("Validation failed"),
                "With validate=false, should not see validation error, got: {}",
                msg
            );
        }
        // If it succeeds, that also proves validation was skipped
    }

    #[tokio::test]
    async fn test_gold_sync_valid_config_succeeds() {
        let loader = FakeConfigLoader {
            streams: {
                let mut m = HashMap::new();
                m.insert("test-stream".to_string(), valid_stream_config());
                m
            },
        };
        let checker = FakeCaChecker;
        let opts = crate::types::SyncOptions::default(); // validate=true

        let result = sync_stream(&loader, "test-stream", &checker, &opts).await;
        assert!(
            result.is_ok(),
            "sync_stream with valid config and validate=true should succeed: {:?}",
            result.err()
        );
        let ddl = result.unwrap();
        assert!(
            !ddl.is_empty(),
            "Should generate non-empty DDL for valid config"
        );
    }
}
