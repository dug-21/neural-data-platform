//! Events hypertable DDL generator
//!
//! Generates SQL for the Gold events hypertable and related objects:
//! - Events hypertable with TimescaleDB chunking
//! - Unified events view for V1.2 API compatibility
//! - Hourly events continuous aggregate
//! - Event detection procedure and scheduled job
//!
//! Implements v11-013 per SPEC-E02.
//! Phase 3 (ops-002): Config-driven detection procedure.

use crate::config::{Action, ConfigLoader, DomainConfig, StreamRole};
use crate::error::{GoldDdlError, Result};
use serde::{Deserialize, Serialize};

use super::constants::{GOLD_SCHEMA, NDP_ENTITY_COLUMN, SILVER_SCHEMA};
use super::state_transitions::TransitionConfig;

/// Configuration for events hypertable
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventsConfig {
    /// Whether events are enabled
    #[serde(default)]
    pub enabled: bool,

    /// Chunk interval for hypertable (e.g., "7 days")
    #[serde(default = "default_chunk_interval")]
    pub chunk_interval: String,

    /// Retention policy (e.g., "1 year")
    #[serde(default)]
    pub retention: Option<String>,

    /// Detection job schedule (e.g., "15 minutes")
    #[serde(default = "default_detection_schedule")]
    pub detection_schedule: String,

    /// Refresh window for continuous aggregates (days).
    /// The CA refresh policy re-processes data this far back each cycle.
    /// Defaults to 365 to match the typical 1-year retention.
    #[serde(default = "default_refresh_start_offset_days")]
    pub refresh_start_offset_days: u32,
}

fn default_chunk_interval() -> String {
    "7 days".to_string()
}

fn default_detection_schedule() -> String {
    "15 minutes".to_string()
}

fn default_refresh_start_offset_days() -> u32 {
    365
}

impl EventsConfig {
    /// Create a new events config with default values
    pub fn new() -> Self {
        Self {
            enabled: true,
            chunk_interval: default_chunk_interval(),
            retention: Some("1 year".to_string()),
            detection_schedule: default_detection_schedule(),
            refresh_start_offset_days: default_refresh_start_offset_days(),
        }
    }
}

/// A ConfigLoader that returns errors for every operation.
/// Used when EventsGenerator is constructed via `new()` without domain context.
struct NullConfigLoader;

impl ConfigLoader for NullConfigLoader {
    fn load_stream_config(&self, stream_id: &str) -> Result<crate::config::StreamConfig> {
        Err(GoldDdlError::ConfigNotFound {
            path: format!("null-loader:{}", stream_id),
        })
    }

    fn load_domain_config(&self, domain_id: &str) -> Result<DomainConfig> {
        Err(GoldDdlError::ConfigNotFound {
            path: format!("null-loader:{}", domain_id),
        })
    }
}

/// Generator for events hypertable DDL
pub struct EventsGenerator {
    /// Domain ID for naming conventions
    domain_id: String,

    /// Events configuration
    config: EventsConfig,

    /// Domain configuration for config-driven detection procedure
    domain_config: Option<DomainConfig>,

    /// Config loader for resolving stream configs
    config_loader: Box<dyn ConfigLoader>,
}

impl EventsGenerator {
    /// Create a new generator from domain configuration
    ///
    /// Uses events config from the domain if present, otherwise falls back to defaults.
    /// Accepts a config_loader for resolving stream-level configuration during
    /// detection procedure generation.
    pub fn from_domain_config(domain: &DomainConfig, config_loader: Box<dyn ConfigLoader>) -> Self {
        #[allow(clippy::unwrap_or_default)]
        let config = domain.events.clone().unwrap_or_else(EventsConfig::new);
        Self {
            domain_id: domain.id.clone(),
            config,
            domain_config: Some(domain.clone()),
            config_loader,
        }
    }

    /// Create a new generator with explicit configuration
    ///
    /// This constructor is used when only domain_id and EventsConfig are available
    /// (e.g., tests that only exercise hypertable/view/aggregate generation).
    /// The detection procedure will fall back to generating an empty body
    /// if no domain_config was provided.
    pub fn new(domain_id: &str, config: EventsConfig) -> Self {
        Self {
            domain_id: domain_id.to_string(),
            config,
            domain_config: None,
            config_loader: Box::new(NullConfigLoader),
        }
    }

    /// Generate complete DDL for events infrastructure
    pub fn generate(&self, action: Action) -> Result<String> {
        if !self.config.enabled {
            return Err(GoldDdlError::GenerationFailed {
                message: format!("Events not enabled for domain '{}'", self.domain_id),
            });
        }

        let mut ddl_parts = Vec::new();

        // Header comments
        ddl_parts.push(format!(
            "-- Events hypertable DDL for domain: {}",
            self.domain_id
        ));
        ddl_parts.push("-- Generated by ndp-gold-ddl".to_string());
        ddl_parts.push(format!("-- Mode: {}", action));
        ddl_parts.push(String::new());

        // Schema creation
        ddl_parts.push(format!("CREATE SCHEMA IF NOT EXISTS {};", GOLD_SCHEMA));
        ddl_parts.push(String::new());

        // Generate components based on action
        match action {
            Action::Sync => {
                ddl_parts.push(self.generate_events_hypertable_sync()?);
            }
            Action::Recreate => {
                ddl_parts.push(self.generate_events_hypertable_recreate()?);
            }
        }

        ddl_parts.push(String::new());
        ddl_parts.push(self.generate_unified_view()?);
        ddl_parts.push(String::new());
        ddl_parts.push(self.generate_hourly_aggregate(action)?);
        ddl_parts.push(String::new());
        ddl_parts.push(self.generate_hourly_by_entity_aggregate(action)?);
        ddl_parts.push(String::new());
        ddl_parts.push(self.generate_detection_procedure()?);
        ddl_parts.push(String::new());
        ddl_parts.push(self.generate_detection_job()?);

        Ok(ddl_parts.join("\n"))
    }

    /// Generate SQL for events hypertable (sync mode)
    fn generate_events_hypertable_sync(&self) -> Result<String> {
        let create_table = self.generate_create_table_sql()?;
        let indexes = self.generate_indexes_sql()?;
        let retention = self.generate_retention_policy_sql()?;

        Ok(format!(
            r#"-- Events hypertable (create if not exists)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_tables
        WHERE schemaname = '{gold_schema}'
          AND tablename = 'events'
    ) THEN
{create_table_indented}

        RAISE NOTICE 'Created events hypertable: {gold_schema}.events';
    ELSE
        RAISE NOTICE '{gold_schema}.events already exists, skipping';
    END IF;
END $$;

{indexes}

{retention}"#,
            gold_schema = GOLD_SCHEMA,
            create_table_indented = Self::indent(&create_table, 8),
            indexes = indexes,
            retention = retention,
        ))
    }

    /// Generate SQL for events hypertable (recreate mode)
    fn generate_events_hypertable_recreate(&self) -> Result<String> {
        let create_table = self.generate_create_table_sql()?;
        let indexes = self.generate_indexes_sql()?;
        let retention = self.generate_retention_policy_sql()?;

        Ok(format!(
            r#"-- Drop existing events infrastructure
DROP VIEW IF EXISTS {gold_schema}.events_unified CASCADE;
DROP MATERIALIZED VIEW IF EXISTS {gold_schema}.events_hourly_by_entity CASCADE;
DROP MATERIALIZED VIEW IF EXISTS {gold_schema}.events_hourly CASCADE;
DROP TABLE IF EXISTS {gold_schema}.events CASCADE;

-- Events hypertable
{create_table}

{indexes}

{retention}"#,
            gold_schema = GOLD_SCHEMA,
            create_table = create_table,
            indexes = indexes,
            retention = retention,
        ))
    }

    /// Generate CREATE TABLE SQL for events
    fn generate_create_table_sql(&self) -> Result<String> {
        Ok(format!(
            r#"CREATE TABLE {gold_schema}.events (
    -- Identity
    event_id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    event_time TIMESTAMPTZ NOT NULL,

    -- Event classification
    stream_id TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    event_type TEXT NOT NULL,

    -- State transition fields (NULL for threshold crossings)
    from_state TEXT,
    to_state TEXT,
    duration_in_state_ms BIGINT,

    -- Threshold crossing fields (NULL for state transitions)
    metric TEXT,
    threshold_value DOUBLE PRECISION,
    crossing_direction TEXT,
    metric_value DOUBLE PRECISION,
    previous_metric_value DOUBLE PRECISION,
    objective_id TEXT,

    -- Context snapshot at event time (for correlation)
    context JSONB NOT NULL DEFAULT '{{}}'::JSONB,

    -- Extensible details
    details JSONB NOT NULL DEFAULT '{{}}'::JSONB
);

-- Convert to hypertable
SELECT create_hypertable('{gold_schema}.events', 'event_time',
    chunk_time_interval => INTERVAL '{chunk_interval}',
    if_not_exists => TRUE
);

COMMENT ON TABLE {gold_schema}.events IS
    'Events hypertable: state transitions and threshold crossings with context snapshots. For V1.2 Pattern Detection.';"#,
            gold_schema = GOLD_SCHEMA,
            chunk_interval = self.config.chunk_interval,
        ))
    }

    /// Generate index creation SQL
    fn generate_indexes_sql(&self) -> Result<String> {
        Ok(format!(
            r#"-- Indexes for V1.2 query patterns
CREATE INDEX IF NOT EXISTS idx_events_time
    ON {gold_schema}.events (event_time DESC);

CREATE INDEX IF NOT EXISTS idx_events_type_time
    ON {gold_schema}.events (event_type, event_time DESC);

CREATE INDEX IF NOT EXISTS idx_events_entity_time
    ON {gold_schema}.events (entity_id, event_time DESC);

CREATE INDEX IF NOT EXISTS idx_events_objective
    ON {gold_schema}.events (objective_id, event_time DESC)
    WHERE event_type = 'threshold_crossing';

CREATE INDEX IF NOT EXISTS idx_events_context
    ON {gold_schema}.events USING GIN (context);

CREATE INDEX IF NOT EXISTS idx_events_details
    ON {gold_schema}.events USING GIN (details);"#,
            gold_schema = GOLD_SCHEMA,
        ))
    }

    /// Generate retention policy SQL
    fn generate_retention_policy_sql(&self) -> Result<String> {
        match &self.config.retention {
            Some(retention) => Ok(format!(
                r#"-- Retention policy ({retention})
SELECT add_retention_policy('{gold_schema}.events', INTERVAL '{retention}', if_not_exists => TRUE);"#,
                gold_schema = GOLD_SCHEMA,
                retention = retention,
            )),
            None => Ok("-- No retention policy configured".to_string()),
        }
    }

    /// Generate unified events view SQL
    pub fn generate_unified_view(&self) -> Result<String> {
        Ok(format!(
            r#"-- Unified events view for V1.2 API compatibility
CREATE OR REPLACE VIEW {gold_schema}.events_unified AS
SELECT
    event_id,
    event_time,
    stream_id,
    entity_id,
    event_type,
    -- Build details JSONB for backward compatibility
    CASE event_type
        WHEN 'state_transition' THEN
            jsonb_build_object(
                'from_state', from_state,
                'to_state', to_state,
                'duration_in_previous_ms', duration_in_state_ms
            )
        WHEN 'threshold_crossing' THEN
            jsonb_build_object(
                'metric', metric,
                'threshold', threshold_value,
                'direction', crossing_direction,
                'value', metric_value,
                'previous_value', previous_metric_value,
                'objective_id', objective_id
            )
        ELSE details
    END AS details,
    context
FROM {gold_schema}.events
ORDER BY event_time, event_type, event_id;

COMMENT ON VIEW {gold_schema}.events_unified IS
    'V1.2 API view on events hypertable. Provides backward-compatible schema.';"#,
            gold_schema = GOLD_SCHEMA,
        ))
    }

    /// Generate hourly events continuous aggregate SQL
    pub fn generate_hourly_aggregate(&self, action: Action) -> Result<String> {
        let create_ca = format!(
            r#"CREATE MATERIALIZED VIEW {gold_schema}.events_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', event_time) AS bucket,
    COUNT(*) AS total_events,
    COUNT(*) FILTER (WHERE event_type = 'state_transition') AS state_transition_count,
    COUNT(*) FILTER (WHERE event_type = 'threshold_crossing') AS threshold_crossing_count,
    COUNT(DISTINCT entity_id) AS distinct_entities_with_events
FROM {gold_schema}.events
GROUP BY bucket
WITH NO DATA"#,
            gold_schema = GOLD_SCHEMA,
        );

        let refresh_start_offset_days = self.config.refresh_start_offset_days;
        let refresh_policy = format!(
            r#"-- Refresh policy for events hourly aggregate (remove+add ensures correct offsets on redeploy)
SELECT remove_continuous_aggregate_policy('{gold_schema}.events_hourly', if_exists => TRUE);
SELECT add_continuous_aggregate_policy('{gold_schema}.events_hourly',
    start_offset => INTERVAL '{refresh_start_offset_days} days',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '15 minutes'
);

-- Index for time range queries
CREATE INDEX IF NOT EXISTS idx_events_hourly_bucket
    ON {gold_schema}.events_hourly (bucket DESC);

-- Backfill: materialize any events outside the rolling refresh window
CALL refresh_continuous_aggregate('{gold_schema}.events_hourly', NULL, NULL);"#,
            gold_schema = GOLD_SCHEMA,
        );

        match action {
            Action::Sync => Ok(format!(
                r#"-- Hourly events continuous aggregate (create if not exists)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM timescaledb_information.continuous_aggregates
        WHERE view_schema = '{gold_schema}'
          AND view_name = 'events_hourly'
    ) THEN
        {create_ca_indented};
        RAISE NOTICE 'Created continuous aggregate: {gold_schema}.events_hourly';
    ELSE
        RAISE NOTICE '{gold_schema}.events_hourly already exists, skipping';
    END IF;
END $$;

{refresh_policy}"#,
                gold_schema = GOLD_SCHEMA,
                create_ca_indented = Self::indent(&create_ca, 8),
                refresh_policy = refresh_policy,
            )),
            Action::Recreate => Ok(format!(
                r#"-- Hourly events continuous aggregate (recreate)
DROP MATERIALIZED VIEW IF EXISTS {gold_schema}.events_hourly CASCADE;

{create_ca};

{refresh_policy}"#,
                gold_schema = GOLD_SCHEMA,
                create_ca = create_ca,
                refresh_policy = refresh_policy,
            )),
        }
    }

    /// Generate hourly events by entity continuous aggregate SQL
    pub fn generate_hourly_by_entity_aggregate(&self, action: Action) -> Result<String> {
        let create_ca = format!(
            r#"CREATE MATERIALIZED VIEW {gold_schema}.events_hourly_by_entity
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', event_time) AS bucket,
    entity_id,
    stream_id,
    COUNT(*) AS total_events,
    COUNT(*) FILTER (WHERE event_type = 'state_transition') AS state_transition_count,
    COUNT(*) FILTER (WHERE event_type = 'threshold_crossing') AS threshold_crossing_count
FROM {gold_schema}.events
GROUP BY bucket, entity_id, stream_id
WITH NO DATA"#,
            gold_schema = GOLD_SCHEMA,
        );

        let refresh_start_offset_days = self.config.refresh_start_offset_days;
        let refresh_policy = format!(
            r#"-- Refresh policy for events hourly by entity aggregate (remove+add ensures correct offsets on redeploy)
SELECT remove_continuous_aggregate_policy('{gold_schema}.events_hourly_by_entity', if_exists => TRUE);
SELECT add_continuous_aggregate_policy('{gold_schema}.events_hourly_by_entity',
    start_offset => INTERVAL '{refresh_start_offset_days} days',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '15 minutes'
);

-- Indexes for events hourly by entity
CREATE INDEX IF NOT EXISTS idx_events_hourly_by_entity_bucket
    ON {gold_schema}.events_hourly_by_entity (bucket DESC);

CREATE INDEX IF NOT EXISTS idx_events_hourly_by_entity_entity_bucket
    ON {gold_schema}.events_hourly_by_entity (entity_id, bucket DESC);

-- Backfill: materialize any events outside the rolling refresh window
CALL refresh_continuous_aggregate('{gold_schema}.events_hourly_by_entity', NULL, NULL);"#,
            gold_schema = GOLD_SCHEMA,
        );

        match action {
            Action::Sync => Ok(format!(
                r#"-- Hourly events by entity continuous aggregate (create if not exists)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM timescaledb_information.continuous_aggregates
        WHERE view_schema = '{gold_schema}'
          AND view_name = 'events_hourly_by_entity'
    ) THEN
        {create_ca_indented};
        RAISE NOTICE 'Created continuous aggregate: {gold_schema}.events_hourly_by_entity';
    ELSE
        RAISE NOTICE '{gold_schema}.events_hourly_by_entity already exists, skipping';
    END IF;
END $$;

{refresh_policy}"#,
                gold_schema = GOLD_SCHEMA,
                create_ca_indented = Self::indent(&create_ca, 8),
                refresh_policy = refresh_policy,
            )),
            Action::Recreate => Ok(format!(
                r#"-- Hourly events by entity continuous aggregate (recreate)
DROP MATERIALIZED VIEW IF EXISTS {gold_schema}.events_hourly_by_entity CASCADE;

{create_ca};

{refresh_policy}"#,
                gold_schema = GOLD_SCHEMA,
                create_ca = create_ca,
                refresh_policy = refresh_policy,
            )),
        }
    }

    /// Derive the Gold CA table name from a silver target_table.
    ///
    /// Convention: strip the silver schema prefix, use the remainder with `_hourly`
    /// suffix in the gold schema.
    ///
    /// Examples:
    /// - `silver.air_quality` -> `gold.air_quality_hourly`
    /// - `silver.state_events` -> `gold.state_events_hourly`
    /// - `air_quality` (no prefix) -> `gold.air_quality_hourly`
    fn derive_gold_ca_table(silver_table: &str) -> String {
        let table_id = silver_table
            .strip_prefix(&format!("{}.", SILVER_SCHEMA))
            .unwrap_or(silver_table);
        format!("{}.{}_hourly", GOLD_SCHEMA, table_id)
    }

    /// Build a list of context columns from the domain's aligned streams.
    ///
    /// Derives column names from each stream's gold_etl aggregates using the
    /// `{alias}_{field}_{metric}` naming convention. The columns are sorted
    /// for deterministic output.
    fn build_context_columns(&self, domain_config: &DomainConfig) -> Vec<(String, String)> {
        let mut context_cols: Vec<(String, String)> = Vec::new();

        for stream_ref in &domain_config.streams {
            if let Ok(stream_config) = self.config_loader.load_stream_config(&stream_ref.stream_id)
            {
                if let Some(ref gold_etl) = stream_config.gold_etl {
                    if let Some(ref aggregates) = gold_etl.aggregates {
                        let mut field_names: Vec<_> = aggregates.fields.keys().collect();
                        field_names.sort();
                        for field_name in field_names {
                            if let Some(field_config) = aggregates.fields.get(field_name) {
                                for metric in &field_config.metrics {
                                    let col_name =
                                        format!("{}_{}_{}", stream_ref.alias, field_name, metric);
                                    // Use a short label for the JSONB key
                                    let label = format!("{}_{}", stream_ref.alias, field_name);
                                    // Only add unique labels (first metric wins for label)
                                    if !context_cols.iter().any(|(l, _)| l == &label) {
                                        context_cols.push((label, col_name));
                                    }
                                }
                            }
                        }
                    }
                }

                // For state_event streams, add the state_last column
                if stream_ref.role == StreamRole::Actuator {
                    if let Some(ref gold_etl) = stream_config.gold_etl {
                        if let Some(ref features) = gold_etl.features {
                            if let Some(ref transitions) = features.transitions {
                                if transitions.enabled {
                                    let col_name =
                                        format!("{}_{}_last", stream_ref.alias, transitions.field);
                                    let label =
                                        format!("{}_{}", stream_ref.alias, transitions.field);
                                    if !context_cols.iter().any(|(l, _)| l == &label) {
                                        context_cols.push((label, col_name));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        context_cols.sort();
        context_cols
    }

    /// Build the context enrichment JSONB SQL expression.
    ///
    /// Returns a SQL fragment that produces JSONB context from the aligned view
    /// at a given bucket time. If no context columns are available, returns
    /// a static empty JSONB expression.
    fn build_context_sql(&self, domain_config: &DomainConfig, time_expr: &str) -> String {
        let _domain_id_snake = self.domain_id.replace('-', "_");
        let aligned_view = &domain_config.alignment.view_name;
        let context_cols = self.build_context_columns(domain_config);

        if context_cols.is_empty() {
            return "'{}'::JSONB".to_string();
        }

        let jsonb_args: Vec<String> = context_cols
            .iter()
            .map(|(label, col)| format!("                '{}', a.{}", label, col))
            .collect();

        format!(
            r#"COALESCE(
            (SELECT jsonb_build_object(
{}
            ) FROM {gold_schema}.{aligned_view} a
            WHERE a.bucket = {time_expr}),
            '{{}}'::JSONB
        )"#,
            jsonb_args.join(",\n"),
            gold_schema = GOLD_SCHEMA,
            aligned_view = aligned_view,
            time_expr = time_expr,
        )
    }

    /// Generate the state transitions section of the detection procedure.
    ///
    /// Finds the actuator stream from domain config, loads its StreamConfig to
    /// get the silver table, entity field, and state field. Returns None if no
    /// actuator stream is found.
    fn generate_state_transitions_section(&self, domain_config: &DomainConfig) -> Option<String> {
        // Find the actuator stream
        let actuator_ref = domain_config
            .streams
            .iter()
            .find(|s| s.role == StreamRole::Actuator)?;

        // Load the actuator's stream config
        let stream_config = self
            .config_loader
            .load_stream_config(&actuator_ref.stream_id)
            .ok()?;

        // Get silver table
        let silver_table = stream_config
            .silver_etl
            .as_ref()
            .map(|s| s.target_table.clone())
            .unwrap_or_else(|| {
                format!(
                    "{}.{}",
                    SILVER_SCHEMA,
                    actuator_ref.stream_id.replace('-', "_")
                )
            });

        // Get transition config from the stream's gold_etl
        let transition_config = TransitionConfig::from_stream_config(&stream_config)
            .unwrap_or_else(|| TransitionConfig::new("state", NDP_ENTITY_COLUMN));

        let entity_field = if transition_config.entity_field.is_empty() {
            NDP_ENTITY_COLUMN.to_string()
        } else {
            transition_config.entity_field.clone()
        };

        let state_field = if transition_config.state_field.is_empty() {
            "state".to_string()
        } else {
            transition_config.state_field.clone()
        };

        let stream_id = &actuator_ref.stream_id;
        let context_sql =
            self.build_context_sql(domain_config, "time_bucket('1 hour', t.event_time)");

        Some(format!(
            r#"    -- =========================================================
    -- STATE TRANSITIONS
    -- =========================================================
    -- Insert new state transition events from Silver layer
    WITH new_transitions AS (
        SELECT
            s.event_time AS event_time,
            '{stream_id}' AS stream_id,
            s.{entity_field} AS entity_id,
            'state_transition' AS event_type,
            LAG(s.{state_field}) OVER (PARTITION BY s.{entity_field} ORDER BY s.event_time) AS from_state,
            s.{state_field} AS to_state,
            EXTRACT(EPOCH FROM (s.event_time - LAG(s.event_time) OVER (PARTITION BY s.{entity_field} ORDER BY s.event_time))) * 1000 AS duration_ms
        FROM {silver_table} s
        WHERE s.event_time > last_run
    ),
    actual_transitions AS (
        SELECT * FROM new_transitions
        WHERE from_state IS NOT NULL
          AND from_state IS DISTINCT FROM to_state
    )
    INSERT INTO {gold_schema}.events (
        event_time, stream_id, entity_id, event_type,
        from_state, to_state, duration_in_state_ms,
        context, details
    )
    SELECT
        t.event_time,
        t.stream_id,
        t.entity_id,
        t.event_type,
        t.from_state,
        t.to_state,
        t.duration_ms::BIGINT,
        -- Context from aligned view at event's hourly bucket
        {context_sql},
        '{{}}'::JSONB
    FROM actual_transitions t
    ON CONFLICT DO NOTHING;

    GET DIAGNOSTICS state_events_inserted = ROW_COUNT;"#,
            stream_id = stream_id,
            entity_field = entity_field,
            state_field = state_field,
            silver_table = silver_table,
            gold_schema = GOLD_SCHEMA,
            context_sql = context_sql,
        ))
    }

    /// Generate a single objective's crossing CTE.
    ///
    /// Produces a CTE like `{metric}_crossings AS (...)` that detects when
    /// the metric crosses the threshold in either direction.
    fn generate_crossing_cte(
        &self,
        objective_id: &str,
        metric: &str,
        threshold: f64,
        _condition: &str,
        _unit: Option<&str>,
    ) -> String {
        // Format the threshold value: use integer format if it's a whole number
        let threshold_str = if threshold == threshold.floor() && threshold.abs() < 1e15 {
            format!("{:.1}", threshold)
        } else {
            format!("{}", threshold)
        };

        // Determine crossing direction based on condition
        // For "<" condition: rising = value goes above threshold, falling = drops below
        // For ">" condition: rising = value goes above threshold, falling = drops below
        // Both use the same detection: crossing above or below the threshold
        let threshold_f = &threshold_str;

        format!(
            r#"    {objective_id}_crossings AS (
        SELECT
            bucket AS event_time,
            stream_id,
            entity_id,
            'threshold_crossing' AS event_type,
            '{metric}' AS metric,
            {threshold_f} AS threshold_value,
            CASE
                WHEN {metric}_prev < {threshold_f} AND {metric}_value >= {threshold_f} THEN 'rising'
                WHEN {metric}_prev >= {threshold_f} AND {metric}_value < {threshold_f} THEN 'falling'
            END AS crossing_direction,
            {metric}_value AS metric_value,
            {metric}_prev AS previous_metric_value,
            '{objective_id}' AS objective_id
        FROM hourly_obs
        WHERE bucket > last_run
          AND {metric}_prev IS NOT NULL
          AND {metric}_value IS NOT NULL
          AND (
              ({metric}_prev < {threshold_f} AND {metric}_value >= {threshold_f})
              OR ({metric}_prev >= {threshold_f} AND {metric}_value < {threshold_f})
          )
    )"#,
            metric = metric,
            threshold_f = threshold_f,
            objective_id = objective_id,
        )
    }

    /// Generate the threshold crossings section of the detection procedure.
    ///
    /// Iterates over domain objectives, resolving each objective's target stream
    /// to derive the Gold CA table name. Generates a crossing CTE per objective
    /// and unions them together. Returns None if no objectives are defined.
    fn generate_threshold_crossings_section(&self, domain_config: &DomainConfig) -> Option<String> {
        if domain_config.objectives.is_empty() {
            return None;
        }

        // Find the primary observation stream for stream_id in the crossings
        let primary_ref = domain_config
            .streams
            .iter()
            .find(|s| s.role == StreamRole::Primary);

        let primary_stream_id = primary_ref
            .map(|r| r.stream_id.as_str())
            .unwrap_or(&self.domain_id);

        // Determine Gold CA table: load primary stream config and derive from silver table
        let gold_ca_table = if let Some(pref) = primary_ref {
            if let Ok(stream_config) = self.config_loader.load_stream_config(&pref.stream_id) {
                if let Some(ref silver_etl) = stream_config.silver_etl {
                    Self::derive_gold_ca_table(&silver_etl.target_table)
                } else {
                    // Fallback: derive from stream_id
                    format!(
                        "{}.{}_hourly",
                        GOLD_SCHEMA,
                        pref.stream_id.replace('-', "_")
                    )
                }
            } else {
                format!(
                    "{}.{}_hourly",
                    GOLD_SCHEMA,
                    pref.stream_id.replace('-', "_")
                )
            }
        } else {
            format!(
                "{}.{}_hourly",
                GOLD_SCHEMA,
                self.domain_id.replace('-', "_")
            )
        };

        // Build the hourly_obs CTE columns: for each objective, include a value and prev column
        let mut obs_columns = Vec::new();
        let mut metrics_seen = Vec::new();
        for obj in &domain_config.objectives {
            let metric = &obj.target.metric;
            if !metrics_seen.contains(metric) {
                obs_columns.push(format!(
                    "            {metric}_mean AS {metric}_value,\n            LAG({metric}_mean) OVER (PARTITION BY {entity} ORDER BY bucket) AS {metric}_prev",
                    metric = metric,
                    entity = NDP_ENTITY_COLUMN,
                ));
                metrics_seen.push(metric.clone());
            }
        }

        let obs_columns_sql = obs_columns.join(",\n");

        // Build crossing CTEs
        let mut crossing_ctes = Vec::new();
        for obj in &domain_config.objectives {
            crossing_ctes.push(self.generate_crossing_cte(
                &obj.id,
                &obj.target.metric,
                obj.target.threshold,
                &obj.target.condition,
                obj.target.unit.as_deref(),
            ));
        }

        let crossing_ctes_sql = crossing_ctes.join(",\n");

        // Build union of all crossings — one per objective (unique by objective_id)
        let union_parts: Vec<String> = domain_config
            .objectives
            .iter()
            .map(|obj| format!("        SELECT * FROM {}_crossings", obj.id))
            .collect();
        let union_sql = union_parts.join("\n        UNION ALL\n");

        // Build the details JSONB expression per metric with unit from objectives
        let mut unit_cases = Vec::new();
        for obj in &domain_config.objectives {
            if let Some(ref unit) = obj.target.unit {
                unit_cases.push(format!("WHEN '{}' THEN '{}'", obj.target.metric, unit));
            }
        }

        let details_sql = if unit_cases.is_empty() {
            "'{}'::JSONB".to_string()
        } else {
            format!(
                "jsonb_build_object('condition', '{}', 'unit', CASE c.metric {} ELSE '' END)",
                domain_config.objectives[0].target.condition,
                unit_cases.join(" "),
            )
        };

        let context_sql = self.build_context_sql(domain_config, "c.event_time");

        Some(format!(
            r#"
    -- =========================================================
    -- THRESHOLD CROSSINGS
    -- =========================================================
    -- Insert new threshold crossing events
    -- (Uses objectives from domain config and compares consecutive observations)
    WITH hourly_obs AS (
        SELECT
            bucket,
            {entity} AS entity_id,
            '{primary_stream_id}'::TEXT AS stream_id,
{obs_columns_sql}
        FROM {gold_ca_table}
        WHERE bucket > last_run - INTERVAL '1 hour'
    ),
{crossing_ctes_sql},
    all_crossings AS (
{union_sql}
    )
    INSERT INTO {gold_schema}.events (
        event_time, stream_id, entity_id, event_type,
        metric, threshold_value, crossing_direction,
        metric_value, previous_metric_value, objective_id,
        context, details
    )
    SELECT
        c.event_time,
        c.stream_id,
        c.entity_id,
        c.event_type,
        c.metric,
        c.threshold_value,
        c.crossing_direction,
        c.metric_value,
        c.previous_metric_value,
        c.objective_id,
        -- Context from aligned view at crossing time
        {context_sql},
        {details_sql}
    FROM all_crossings c
    ON CONFLICT DO NOTHING;

    GET DIAGNOSTICS crossing_events_inserted = ROW_COUNT;"#,
            entity = NDP_ENTITY_COLUMN,
            primary_stream_id = primary_stream_id,
            obs_columns_sql = obs_columns_sql,
            gold_ca_table = gold_ca_table,
            crossing_ctes_sql = crossing_ctes_sql,
            union_sql = union_sql,
            gold_schema = GOLD_SCHEMA,
            context_sql = context_sql,
            details_sql = details_sql,
        ))
    }

    /// Generate event detection procedure SQL
    ///
    /// This is now fully config-driven. It reads actuator streams, objectives,
    /// and context columns from the domain configuration and stream configs
    /// loaded via config_loader.
    pub fn generate_detection_procedure(&self) -> Result<String> {
        let domain_config = self.domain_config.as_ref();

        // If we have domain config, generate config-driven procedure
        if let Some(dc) = domain_config {
            return self.generate_detection_procedure_from_config(dc);
        }

        // Fallback for generators created via new() without domain config:
        // Generate the same procedure shell with no sections.
        // This preserves backward compatibility for tests that use new().
        self.generate_detection_procedure_minimal()
    }

    /// Generate a config-driven detection procedure from DomainConfig.
    fn generate_detection_procedure_from_config(
        &self,
        domain_config: &DomainConfig,
    ) -> Result<String> {
        let state_section = self.generate_state_transitions_section(domain_config);
        let crossings_section = self.generate_threshold_crossings_section(domain_config);

        let has_transitions = state_section.is_some();
        let has_crossings = crossings_section.is_some();

        // Build variable declarations based on what sections exist
        let mut var_decls = vec!["    last_run TIMESTAMPTZ;".to_string()];
        if has_transitions {
            var_decls.push("    state_events_inserted INT := 0;".to_string());
        }
        if has_crossings {
            var_decls.push("    crossing_events_inserted INT := 0;".to_string());
        }
        let var_decls_sql = var_decls.join("\n");

        // Build body sections
        let mut body_parts = Vec::new();

        if let Some(transitions) = state_section {
            body_parts.push(transitions);
        }

        if let Some(crossings) = crossings_section {
            body_parts.push(crossings);
        }

        // Build RAISE NOTICE
        let notice = if has_transitions && has_crossings {
            "    RAISE NOTICE 'Event detection: % state transitions, % threshold crossings',\n        state_events_inserted, crossing_events_inserted;".to_string()
        } else if has_transitions {
            "    RAISE NOTICE 'Event detection: % state transitions', state_events_inserted;"
                .to_string()
        } else if has_crossings {
            "    RAISE NOTICE 'Event detection: % threshold crossings', crossing_events_inserted;"
                .to_string()
        } else {
            "    RAISE NOTICE 'Event detection: no sections configured';".to_string()
        };

        let body_sql = body_parts.join("\n");

        Ok(format!(
            r#"-- Event detection procedure (runs as TimescaleDB job)
-- Delete dependent jobs and DROP procedure to avoid "cannot remove parameter defaults" error
DO $$
DECLARE
    _job_id INTEGER;
BEGIN
    FOR _job_id IN
        SELECT job_id FROM timescaledb_information.jobs
        WHERE proc_schema = '{gold_schema}' AND proc_name = 'detect_events'
    LOOP
        PERFORM delete_job(_job_id);
        RAISE NOTICE 'Deleted job % ({gold_schema}.detect_events) before procedure replacement', _job_id;
    END LOOP;
END $$;

DROP PROCEDURE IF EXISTS {gold_schema}.detect_events(integer, jsonb);

CREATE OR REPLACE PROCEDURE {gold_schema}.detect_events(job_id INT, config JSONB)
LANGUAGE plpgsql AS $$
DECLARE
{var_decls_sql}
BEGIN
    -- Get last successful run time
    SELECT last_successful_finish INTO last_run
    FROM timescaledb_information.job_stats
    WHERE job_id = detect_events.job_id;

    -- Default to 2 hours ago if first run
    last_run := COALESCE(last_run, NOW() - INTERVAL '2 hours');

{body_sql}

{notice}

    COMMIT;
END;
$$;

COMMENT ON PROCEDURE {gold_schema}.detect_events IS
    'Detects state transitions and threshold crossings, inserts into {gold_schema}.events with context snapshots.';"#,
            gold_schema = GOLD_SCHEMA,
            var_decls_sql = var_decls_sql,
            body_sql = body_sql,
            notice = notice,
        ))
    }

    /// Generate a minimal detection procedure for backward compatibility.
    ///
    /// Used when the generator is constructed via `new()` without a DomainConfig.
    /// Produces a valid procedure with no detection sections.
    fn generate_detection_procedure_minimal(&self) -> Result<String> {
        Ok(format!(
            r#"-- Event detection procedure (runs as TimescaleDB job)
-- Delete dependent jobs and DROP procedure to avoid "cannot remove parameter defaults" error
DO $$
DECLARE
    _job_id INTEGER;
BEGIN
    FOR _job_id IN
        SELECT job_id FROM timescaledb_information.jobs
        WHERE proc_schema = '{gold_schema}' AND proc_name = 'detect_events'
    LOOP
        PERFORM delete_job(_job_id);
        RAISE NOTICE 'Deleted job % ({gold_schema}.detect_events) before procedure replacement', _job_id;
    END LOOP;
END $$;

DROP PROCEDURE IF EXISTS {gold_schema}.detect_events(integer, jsonb);

CREATE OR REPLACE PROCEDURE {gold_schema}.detect_events(job_id INT, config JSONB)
LANGUAGE plpgsql AS $$
DECLARE
    last_run TIMESTAMPTZ;
BEGIN
    -- Get last successful run time
    SELECT last_successful_finish INTO last_run
    FROM timescaledb_information.job_stats
    WHERE job_id = detect_events.job_id;

    -- Default to 2 hours ago if first run
    last_run := COALESCE(last_run, NOW() - INTERVAL '2 hours');

    RAISE NOTICE 'Event detection: no sections configured';

    COMMIT;
END;
$$;

COMMENT ON PROCEDURE {gold_schema}.detect_events IS
    'Detects state transitions and threshold crossings, inserts into {gold_schema}.events with context snapshots.';"#,
            gold_schema = GOLD_SCHEMA,
        ))
    }

    /// Generate detection job scheduling SQL
    pub fn generate_detection_job(&self) -> Result<String> {
        Ok(format!(
            r#"-- Schedule the detection job (every {schedule})
SELECT add_job(
    '{gold_schema}.detect_events'::regproc,
    '{schedule}'::INTERVAL,
    config => '{{}}'::JSONB
);"#,
            gold_schema = GOLD_SCHEMA,
            schedule = self.config.detection_schedule,
        ))
    }

    /// Indent text by a given number of spaces
    fn indent(text: &str, spaces: usize) -> String {
        let indent = " ".repeat(spaces);
        text.lines()
            .map(|line| format!("{}{}", indent, line))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Trait for generating events DDL
pub trait IEventsGenerator {
    /// Generate SQL for events hypertable
    fn generate_events_hypertable(&self, domain: &DomainConfig) -> Result<String>;

    /// Generate SQL for unified events view
    fn generate_unified_view(&self) -> Result<String>;

    /// Generate SQL for hourly events continuous aggregate
    fn generate_hourly_aggregate(&self) -> Result<String>;

    /// Generate SQL for event detection procedure
    fn generate_detection_procedure(&self, domain: &DomainConfig) -> Result<String>;

    /// Generate SQL for detection job
    fn generate_detection_job(&self, schedule: &str) -> Result<String>;
}

impl IEventsGenerator for EventsGenerator {
    fn generate_events_hypertable(&self, _domain: &DomainConfig) -> Result<String> {
        let create_table = self.generate_create_table_sql()?;
        let indexes = self.generate_indexes_sql()?;
        let retention = self.generate_retention_policy_sql()?;

        Ok(format!("{}\n\n{}\n\n{}", create_table, indexes, retention))
    }

    fn generate_unified_view(&self) -> Result<String> {
        EventsGenerator::generate_unified_view(self)
    }

    fn generate_hourly_aggregate(&self) -> Result<String> {
        EventsGenerator::generate_hourly_aggregate(self, Action::Sync)
    }

    fn generate_detection_procedure(&self, _domain: &DomainConfig) -> Result<String> {
        EventsGenerator::generate_detection_procedure(self)
    }

    fn generate_detection_job(&self, _schedule: &str) -> Result<String> {
        EventsGenerator::generate_detection_job(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        AggregatesConfig, AlignmentConfig, ConfigLoader, DomainConfig, FeaturesConfig,
        FieldMetricsConfig, GoldEtlConfig, JoinStrategy, NullHandling, ObjectiveConfig,
        SilverEtlConfig, StreamConfig, StreamRef, StreamRole, TargetConfig, TransitionsConfig,
    };
    use crate::error::{GoldDdlError, Result};
    use std::collections::HashMap;

    /// Mock config loader that returns pre-configured stream configs.
    /// Mirrors the air-quality domain for backward-compatible test output.
    struct MockConfigLoader {
        stream_configs: HashMap<String, StreamConfig>,
    }

    impl MockConfigLoader {
        fn air_quality_loader() -> Self {
            let mut configs = HashMap::new();

            // air-quality stream (primary observation)
            configs.insert(
                "air-quality".to_string(),
                StreamConfig {
                    stream_id: "air-quality".to_string(),
                    stream_type: None,
                    fields: vec![],
                    silver_etl: Some(SilverEtlConfig {
                        target_table: "silver.air_quality".to_string(),
                        timestamp: None,
                    }),
                    gold_etl: Some(GoldEtlConfig {
                        enabled: true,
                        aggregates: Some(AggregatesConfig {
                            granularities: vec!["1 hour".to_string()],
                            fields: {
                                let mut fields = HashMap::new();
                                fields.insert(
                                    "co2".to_string(),
                                    FieldMetricsConfig {
                                        metrics: vec!["mean".to_string()],
                                    },
                                );
                                fields.insert(
                                    "pm25".to_string(),
                                    FieldMetricsConfig {
                                        metrics: vec!["mean".to_string()],
                                    },
                                );
                                fields.insert(
                                    "temperature_c".to_string(),
                                    FieldMetricsConfig {
                                        metrics: vec!["mean".to_string()],
                                    },
                                );
                                fields
                            },
                        }),
                        features: None,
                        refresh_policy: None,
                    }),
                },
            );

            // home-assistant-state (actuator)
            configs.insert(
                "home-assistant-state".to_string(),
                StreamConfig {
                    stream_id: "home-assistant-state".to_string(),
                    stream_type: None,
                    fields: vec![],
                    silver_etl: Some(SilverEtlConfig {
                        target_table: "silver.state_events".to_string(),
                        timestamp: None,
                    }),
                    gold_etl: Some(GoldEtlConfig {
                        enabled: true,
                        aggregates: None,
                        features: Some(FeaturesConfig {
                            lag: None,
                            rolling: None,
                            trend: None,
                            transitions: Some(TransitionsConfig {
                                enabled: true,
                                field: "state".to_string(),
                                states: vec!["on".to_string(), "off".to_string()],
                            }),
                        }),
                        refresh_policy: None,
                    }),
                },
            );

            // outdoor-aqi stream (context)
            configs.insert(
                "outdoor-aqi".to_string(),
                StreamConfig {
                    stream_id: "outdoor-aqi".to_string(),
                    stream_type: None,
                    fields: vec![],
                    silver_etl: Some(SilverEtlConfig {
                        target_table: "silver.outdoor_aqi".to_string(),
                        timestamp: None,
                    }),
                    gold_etl: Some(GoldEtlConfig {
                        enabled: true,
                        aggregates: Some(AggregatesConfig {
                            granularities: vec!["1 hour".to_string()],
                            fields: {
                                let mut fields = HashMap::new();
                                fields.insert(
                                    "aqi_pm25".to_string(),
                                    FieldMetricsConfig {
                                        metrics: vec!["mean".to_string()],
                                    },
                                );
                                fields.insert(
                                    "temperature_c".to_string(),
                                    FieldMetricsConfig {
                                        metrics: vec!["mean".to_string()],
                                    },
                                );
                                fields
                            },
                        }),
                        features: None,
                        refresh_policy: None,
                    }),
                },
            );

            Self {
                stream_configs: configs,
            }
        }

        fn empty_loader() -> Self {
            Self {
                stream_configs: HashMap::new(),
            }
        }
    }

    impl ConfigLoader for MockConfigLoader {
        fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig> {
            self.stream_configs.get(stream_id).cloned().ok_or_else(|| {
                GoldDdlError::ConfigNotFound {
                    path: format!("mock:{}", stream_id),
                }
            })
        }

        fn load_domain_config(&self, _domain_id: &str) -> Result<DomainConfig> {
            Err(GoldDdlError::ConfigNotFound {
                path: "mock domain not implemented".to_string(),
            })
        }
    }

    fn create_test_domain() -> DomainConfig {
        DomainConfig {
            id: "indoor-air-quality".to_string(),
            description: "Indoor air quality monitoring domain".to_string(),
            streams: vec![
                StreamRef {
                    stream_id: "air-quality".to_string(),
                    alias: "indoor".to_string(),
                    role: StreamRole::Primary,
                    null_handling: None,
                },
                StreamRef {
                    stream_id: "home-assistant-state".to_string(),
                    alias: "state".to_string(),
                    role: StreamRole::Actuator,
                    null_handling: Some(NullHandling::CarryForward),
                },
            ],
            alignment: AlignmentConfig {
                view_name: "indoor_air_quality_aligned".to_string(),
                granularity: "1 hour".to_string(),
                join_strategy: JoinStrategy::FullOuter,
                null_handling: NullHandling::Preserve,
            },
            objectives: vec![],
            events: None,
        }
    }

    /// Create a domain with objectives for threshold crossing tests
    fn create_test_domain_with_objectives() -> DomainConfig {
        DomainConfig {
            id: "indoor-air-quality".to_string(),
            description: "Indoor air quality monitoring domain".to_string(),
            streams: vec![
                StreamRef {
                    stream_id: "air-quality".to_string(),
                    alias: "indoor".to_string(),
                    role: StreamRole::Primary,
                    null_handling: None,
                },
                StreamRef {
                    stream_id: "outdoor-aqi".to_string(),
                    alias: "outdoor".to_string(),
                    role: StreamRole::Context,
                    null_handling: None,
                },
                StreamRef {
                    stream_id: "home-assistant-state".to_string(),
                    alias: "state".to_string(),
                    role: StreamRole::Actuator,
                    null_handling: Some(NullHandling::CarryForward),
                },
            ],
            alignment: AlignmentConfig {
                view_name: "indoor_air_quality_aligned".to_string(),
                granularity: "1 hour".to_string(),
                join_strategy: JoinStrategy::FullOuter,
                null_handling: NullHandling::Preserve,
            },
            objectives: vec![
                ObjectiveConfig {
                    id: "healthy_co2".to_string(),
                    description: "Keep CO2 below healthy threshold".to_string(),
                    target: TargetConfig {
                        stream: "air-quality".to_string(),
                        metric: "co2".to_string(),
                        condition: "<".to_string(),
                        threshold: 800.0,
                        unit: Some("ppm".to_string()),
                    },
                    priority: crate::config::Priority::High,
                },
                ObjectiveConfig {
                    id: "healthy_pm25".to_string(),
                    description: "Keep PM2.5 below WHO guideline".to_string(),
                    target: TargetConfig {
                        stream: "air-quality".to_string(),
                        metric: "pm25".to_string(),
                        condition: "<".to_string(),
                        threshold: 12.0,
                        unit: Some("ug/m3".to_string()),
                    },
                    priority: crate::config::Priority::High,
                },
            ],
            events: None,
        }
    }

    fn create_test_config() -> EventsConfig {
        EventsConfig {
            enabled: true,
            chunk_interval: "7 days".to_string(),
            retention: Some("1 year".to_string()),
            detection_schedule: "15 minutes".to_string(),
            refresh_start_offset_days: 365,
        }
    }

    // =========================================================================
    // TDD Cycle 1: Events Hypertable Generation
    // =========================================================================

    #[test]
    fn test_generates_create_table() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("CREATE TABLE"));
        assert!(sql.contains("gold.events"));
        assert!(sql.contains("event_id UUID"));
        assert!(sql.contains("event_time TIMESTAMPTZ"));
    }

    #[test]
    fn test_generates_hypertable() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("create_hypertable"));
        assert!(sql.contains("gold.events"));
        assert!(sql.contains("event_time"));
    }

    #[test]
    fn test_generates_chunk_interval() {
        let config = create_test_config();
        let generator = EventsGenerator::new("test-domain", config);

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("chunk_time_interval => INTERVAL '7 days'"));
    }

    // =========================================================================
    // TDD Cycle 2: Schema Columns
    // =========================================================================

    #[test]
    fn test_generates_state_transition_columns() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("from_state TEXT"));
        assert!(sql.contains("to_state TEXT"));
        assert!(sql.contains("duration_in_state_ms BIGINT"));
    }

    #[test]
    fn test_generates_threshold_crossing_columns() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("metric TEXT"));
        assert!(sql.contains("threshold_value DOUBLE PRECISION"));
        assert!(sql.contains("crossing_direction TEXT"));
        assert!(sql.contains("metric_value DOUBLE PRECISION"));
        assert!(sql.contains("previous_metric_value DOUBLE PRECISION"));
        assert!(sql.contains("objective_id TEXT"));
    }

    #[test]
    fn test_generates_context_and_details_columns() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("context JSONB NOT NULL"));
        assert!(sql.contains("details JSONB NOT NULL"));
    }

    // =========================================================================
    // TDD Cycle 3: Indexes
    // =========================================================================

    #[test]
    fn test_generates_time_index() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("CREATE INDEX"));
        assert!(sql.contains("idx_events_time"));
        assert!(sql.contains("event_time DESC"));
    }

    #[test]
    fn test_generates_type_time_index() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("idx_events_type_time"));
        assert!(sql.contains("event_type, event_time DESC"));
    }

    #[test]
    fn test_generates_gin_indexes_for_jsonb() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("USING GIN (context)"));
        assert!(sql.contains("USING GIN (details)"));
    }

    #[test]
    fn test_generates_objective_partial_index() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("idx_events_objective"));
        assert!(sql.contains("WHERE event_type = 'threshold_crossing'"));
    }

    // =========================================================================
    // TDD Cycle 4: Unified Events View
    // =========================================================================

    #[test]
    fn test_generates_unified_view() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("CREATE OR REPLACE VIEW gold.events_unified"));
        assert!(sql.contains("SELECT") && sql.contains("FROM gold.events"));
    }

    #[test]
    fn test_unified_view_builds_details_jsonb() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("CASE event_type"));
        assert!(sql.contains("WHEN 'state_transition'"));
        assert!(sql.contains("WHEN 'threshold_crossing'"));
        assert!(sql.contains("jsonb_build_object"));
    }

    // =========================================================================
    // TDD Cycle 5: Hourly Continuous Aggregate
    // =========================================================================

    #[test]
    fn test_generates_hourly_aggregate() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("CREATE MATERIALIZED VIEW gold.events_hourly"));
        assert!(sql.contains("timescaledb.continuous"));
    }

    #[test]
    fn test_hourly_aggregate_counts_events() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("COUNT(*) AS total_events"));
        assert!(sql.contains("state_transition_count"));
        assert!(sql.contains("threshold_crossing_count"));
    }

    #[test]
    fn test_hourly_aggregate_refresh_policy() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        // Remove old policy before adding (idempotent redeploy)
        assert!(sql.contains("remove_continuous_aggregate_policy('gold.events_hourly'"));
        assert!(sql.contains("add_continuous_aggregate_policy"));
        assert!(sql.contains("gold.events_hourly"));
        assert!(sql.contains("start_offset => INTERVAL '365 days'"));
        assert!(sql.contains("schedule_interval"));
        // Backfill after creation
        assert!(sql.contains("CALL refresh_continuous_aggregate('gold.events_hourly'"));
    }

    // =========================================================================
    // TDD Cycle 6: Detection Procedure
    // =========================================================================

    #[test]
    fn test_generates_detection_procedure() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("CREATE OR REPLACE PROCEDURE gold.detect_events"));
        assert!(sql.contains("LANGUAGE plpgsql"));
    }

    #[test]
    fn test_detection_procedure_handles_state_transitions() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("STATE TRANSITIONS"));
        assert!(sql.contains("LAG(s.state) OVER"));
        assert!(sql.contains("from_state IS DISTINCT FROM to_state"));
    }

    #[test]
    fn test_detection_procedure_handles_threshold_crossings() {
        let domain = create_test_domain_with_objectives();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("THRESHOLD CROSSINGS"));
        assert!(sql.contains("co2_crossings"));
        assert!(sql.contains("pm25_crossings"));
    }

    #[test]
    fn test_detection_procedure_captures_context() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("indoor_air_quality_aligned"));
        assert!(sql.contains("indoor_co2"));
        assert!(sql.contains("indoor_pm25"));
    }

    #[test]
    fn test_detection_procedure_uses_last_run() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("last_successful_finish"));
        assert!(sql.contains("last_run"));
        assert!(sql.contains("job_stats"));
    }

    // =========================================================================
    // TDD Cycle 7: Detection Job Scheduling
    // =========================================================================

    #[test]
    fn test_generates_detection_job() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("add_job"));
        assert!(sql.contains("gold.detect_events"));
        assert!(sql.contains("15 minutes"));
    }

    #[test]
    fn test_detection_job_configurable_schedule() {
        let config = EventsConfig {
            enabled: true,
            detection_schedule: "30 minutes".to_string(),
            ..EventsConfig::default()
        };
        let generator = EventsGenerator::new("test-domain", config);

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("'30 minutes'"));
    }

    // =========================================================================
    // TDD Cycle 8: Retention Policy
    // =========================================================================

    #[test]
    fn test_generates_retention_policy() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("add_retention_policy"));
        assert!(sql.contains("gold.events"));
        assert!(sql.contains("1 year"));
    }

    #[test]
    fn test_no_retention_policy_when_not_configured() {
        let config = EventsConfig {
            enabled: true,
            retention: None,
            ..EventsConfig::default()
        };
        let generator = EventsGenerator::new("test-domain", config);

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("No retention policy configured"));
    }

    // =========================================================================
    // TDD Cycle 9: Idempotency
    // =========================================================================

    #[test]
    fn test_sync_mode_checks_existence() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Sync).unwrap();

        assert!(sql.contains("IF NOT EXISTS"));
        assert!(sql.contains("pg_tables"));
        assert!(sql.contains("schemaname = 'gold'"));
    }

    #[test]
    fn test_recreate_mode_drops_first() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("DROP TABLE IF EXISTS gold.events CASCADE"));
        assert!(sql.contains("DROP VIEW IF EXISTS gold.events_unified CASCADE"));
        assert!(sql.contains("DROP MATERIALIZED VIEW IF EXISTS gold.events_hourly CASCADE"));
    }

    // =========================================================================
    // TDD Cycle 10: Error Handling
    // =========================================================================

    #[test]
    fn test_disabled_events_returns_error() {
        let config = EventsConfig {
            enabled: false,
            ..EventsConfig::default()
        };
        let generator = EventsGenerator::new("test-domain", config);

        let result = generator.generate(Action::Recreate);

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not enabled"));
    }

    // =========================================================================
    // TDD Cycle 11: Header Comments
    // =========================================================================

    #[test]
    fn test_generates_header_comments() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("-- Events hypertable DDL for domain: indoor-air-quality"));
        assert!(sql.contains("-- Generated by ndp-gold-ddl"));
        assert!(sql.contains("-- Mode: recreate"));
    }

    #[test]
    fn test_generates_table_comments() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("COMMENT ON TABLE gold.events"));
        assert!(sql.contains("COMMENT ON VIEW gold.events_unified"));
        assert!(sql.contains("COMMENT ON PROCEDURE gold.detect_events"));
    }

    // =========================================================================
    // Trait Implementation Tests
    // =========================================================================

    #[test]
    fn test_trait_generate_events_hypertable() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql =
            <EventsGenerator as IEventsGenerator>::generate_events_hypertable(&generator, &domain)
                .unwrap();

        assert!(sql.contains("CREATE TABLE gold.events"));
        assert!(sql.contains("CREATE INDEX"));
    }

    #[test]
    fn test_trait_generate_unified_view() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = <EventsGenerator as IEventsGenerator>::generate_unified_view(&generator).unwrap();

        assert!(sql.contains("CREATE OR REPLACE VIEW gold.events_unified"));
    }

    #[test]
    fn test_trait_generate_hourly_aggregate() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql =
            <EventsGenerator as IEventsGenerator>::generate_hourly_aggregate(&generator).unwrap();

        assert!(sql.contains("CREATE MATERIALIZED VIEW gold.events_hourly"));
    }

    // =========================================================================
    // TDD Cycle 12: from_domain_config reads events config
    // =========================================================================

    #[test]
    fn test_from_domain_config_uses_config_events_when_present() {
        let mut domain = create_test_domain();
        domain.events = Some(EventsConfig {
            enabled: true,
            chunk_interval: "14 days".to_string(),
            retention: Some("2 years".to_string()),
            detection_schedule: "30 minutes".to_string(),
            refresh_start_offset_days: 730,
        });

        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );
        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("INTERVAL '14 days'"));
        assert!(sql.contains("INTERVAL '2 years'"));
        assert!(sql.contains("'30 minutes'"));
    }

    #[test]
    fn test_from_domain_config_uses_defaults_when_no_events_config() {
        let domain = create_test_domain();
        assert!(domain.events.is_none());

        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );
        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("INTERVAL '7 days'"));
        assert!(sql.contains("INTERVAL '1 year'"));
        assert!(sql.contains("'15 minutes'"));
    }

    // =========================================================================
    // TDD Cycle 13: events_hourly_by_entity CA
    // =========================================================================

    #[test]
    fn test_generates_hourly_by_entity_aggregate() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("gold.events_hourly_by_entity"));
        assert!(sql.contains("entity_id"));
        assert!(sql.contains("stream_id"));
        assert!(sql.contains("GROUP BY bucket, entity_id, stream_id"));
    }

    #[test]
    fn test_hourly_by_entity_refresh_policy() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        // Remove old policy before adding (idempotent redeploy)
        assert!(sql.contains("remove_continuous_aggregate_policy('gold.events_hourly_by_entity'"));
        assert!(sql.contains("add_continuous_aggregate_policy('gold.events_hourly_by_entity'"));
        assert!(sql.contains("start_offset => INTERVAL '365 days'"));
        // Backfill after creation
        assert!(sql.contains("CALL refresh_continuous_aggregate('gold.events_hourly_by_entity'"));
    }

    #[test]
    fn test_hourly_by_entity_indexes() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("idx_events_hourly_by_entity_entity_bucket"));
        assert!(sql.contains("(entity_id, bucket DESC)"));
    }

    #[test]
    fn test_recreate_drops_hourly_by_entity() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(
            sql.contains("DROP MATERIALIZED VIEW IF EXISTS gold.events_hourly_by_entity CASCADE")
        );
    }

    // =========================================================================
    // TDD Cycle 14: Config-driven detection procedure (Phase 3)
    // =========================================================================

    #[test]
    fn test_detection_reads_actuator_stream_id_from_config() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        // The actuator stream_id should come from config, not hardcoded
        assert!(
            sql.contains("'home-assistant-state' AS stream_id"),
            "Should use actuator stream_id from domain config"
        );
    }

    #[test]
    fn test_detection_reads_silver_table_from_config() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        // Silver table should come from the actuator's stream config
        assert!(
            sql.contains("FROM silver.state_events s"),
            "Should use silver table from stream config"
        );
    }

    #[test]
    fn test_detection_reads_entity_field_from_config() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        // Entity field should come from TransitionConfig, defaulting to ndp_id
        assert!(
            sql.contains("s.ndp_id AS entity_id"),
            "Should use entity_field from transition config"
        );
    }

    #[test]
    fn test_detection_reads_state_field_from_config() {
        let domain = create_test_domain();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        // State field should come from TransitionConfig
        assert!(
            sql.contains("LAG(s.state) OVER"),
            "Should use state_field from transition config"
        );
        assert!(
            sql.contains("s.state AS to_state"),
            "Should use state_field for to_state"
        );
    }

    #[test]
    fn test_detection_reads_objectives_for_crossings() {
        let domain = create_test_domain_with_objectives();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        // Objective-derived values
        assert!(
            sql.contains("800.0 AS threshold_value"),
            "co2 threshold from config"
        );
        assert!(
            sql.contains("12.0 AS threshold_value"),
            "pm25 threshold from config"
        );
        assert!(
            sql.contains("'healthy_co2' AS objective_id"),
            "co2 objective_id from config"
        );
        assert!(
            sql.contains("'healthy_pm25' AS objective_id"),
            "pm25 objective_id from config"
        );
    }

    #[test]
    fn test_detection_derives_gold_ca_table() {
        let domain = create_test_domain_with_objectives();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        // Gold CA table derived from silver.air_quality -> gold.air_quality_hourly
        assert!(
            sql.contains("gold.air_quality_hourly"),
            "Should derive gold CA table from silver table"
        );
    }

    #[test]
    fn test_detection_reads_metric_from_objectives() {
        let domain = create_test_domain_with_objectives();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(
            sql.contains("co2_mean AS co2_value"),
            "metric column from config"
        );
        assert!(
            sql.contains("pm25_mean AS pm25_value"),
            "metric column from config"
        );
        assert!(sql.contains("'co2' AS metric"), "metric name from config");
        assert!(sql.contains("'pm25' AS metric"), "metric name from config");
    }

    #[test]
    fn test_detection_reads_units_from_objectives() {
        let domain = create_test_domain_with_objectives();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("ppm"), "unit from co2 objective");
        assert!(sql.contains("ug/m3"), "unit from pm25 objective");
    }

    #[test]
    fn test_detection_reads_primary_stream_id_for_crossings() {
        let domain = create_test_domain_with_objectives();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(
            sql.contains("'air-quality'::TEXT AS stream_id"),
            "Crossing stream_id from primary stream config"
        );
    }

    #[test]
    fn test_detection_no_actuator_skips_transitions() {
        let mut domain = create_test_domain_with_objectives();
        // Remove the actuator stream
        domain.streams.retain(|s| s.role != StreamRole::Actuator);

        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        // Should still have crossings but NOT state transitions
        assert!(
            sql.contains("THRESHOLD CROSSINGS"),
            "Should still have crossings"
        );
        assert!(
            !sql.contains("STATE TRANSITIONS"),
            "Should skip transitions when no actuator"
        );
    }

    #[test]
    fn test_detection_no_objectives_skips_crossings() {
        let domain = create_test_domain(); // No objectives
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        // Should still have state transitions but NOT crossings
        assert!(
            sql.contains("STATE TRANSITIONS"),
            "Should still have transitions"
        );
        assert!(
            !sql.contains("THRESHOLD CROSSINGS"),
            "Should skip crossings when no objectives"
        );
    }

    #[test]
    fn test_detection_context_from_aligned_view() {
        let domain = create_test_domain_with_objectives();
        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::air_quality_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(
            sql.contains("indoor_air_quality_aligned"),
            "Context should reference the aligned view"
        );
        assert!(
            sql.contains("jsonb_build_object"),
            "Context should build JSONB from aligned columns"
        );
    }

    #[test]
    fn test_derive_gold_ca_table_strips_silver_prefix() {
        assert_eq!(
            EventsGenerator::derive_gold_ca_table("silver.air_quality"),
            "gold.air_quality_hourly"
        );
    }

    #[test]
    fn test_derive_gold_ca_table_handles_no_prefix() {
        assert_eq!(
            EventsGenerator::derive_gold_ca_table("weather_forecast"),
            "gold.weather_forecast_hourly"
        );
    }

    #[test]
    fn test_detection_empty_domain_no_crash() {
        // Domain with no streams and no objectives
        let domain = DomainConfig {
            id: "empty-domain".to_string(),
            description: "".to_string(),
            streams: vec![],
            alignment: AlignmentConfig {
                view_name: "empty_aligned".to_string(),
                granularity: "1 hour".to_string(),
                join_strategy: JoinStrategy::FullOuter,
                null_handling: NullHandling::Preserve,
            },
            objectives: vec![],
            events: Some(EventsConfig::new()),
        };

        let generator = EventsGenerator::from_domain_config(
            &domain,
            Box::new(MockConfigLoader::empty_loader()),
        );

        let sql = generator.generate(Action::Recreate).unwrap();

        // Should produce a valid procedure with no sections
        assert!(sql.contains("CREATE OR REPLACE PROCEDURE gold.detect_events"));
        assert!(sql.contains("no sections configured"));
    }

    #[test]
    fn test_new_constructor_still_works_for_simple_tests() {
        // The new() constructor without domain config should still work
        let config = EventsConfig::new();
        let generator = EventsGenerator::new("test-domain", config);

        let sql = generator.generate(Action::Recreate).unwrap();

        assert!(sql.contains("CREATE TABLE gold.events"));
        assert!(sql.contains("CREATE OR REPLACE PROCEDURE gold.detect_events"));
    }
}
