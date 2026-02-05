//! Aligned view generator for cross-stream correlation
//!
//! Generates MATERIALIZED VIEW SQL that joins multiple Gold hourly views
//! based on domain configuration. Implements:
//! - Multi-stream JOINs with configurable strategy
//! - NULL handling per stream type (ADR-FE001-004)
//! - Forecast alignment on issued_at (ADR-FE001-003)
//! - Column aliasing with stream prefixes

use crate::config::{
    Action, AlignedStream, AlignmentConfig, ConfigLoader, DomainConfig, StreamRef, StreamRole,
    StreamType,
};
use crate::error::{GoldDdlError, Result};
use crate::generators::column_builder::{ColumnBuilder, DefaultColumnBuilder};
use crate::generators::join_builder::{DefaultJoinBuilder, JoinBuilder};

/// Generator for aligned views
pub struct AlignedViewGenerator<L: ConfigLoader> {
    config_loader: L,
    join_builder: Box<dyn JoinBuilder>,
    column_builder: Box<dyn ColumnBuilder>,
}

impl<L: ConfigLoader> AlignedViewGenerator<L> {
    /// Create a new generator with the given config loader
    pub fn new(config_loader: L) -> Self {
        Self {
            config_loader,
            join_builder: Box::new(DefaultJoinBuilder),
            column_builder: Box::new(DefaultColumnBuilder),
        }
    }

    /// Generate aligned view DDL for a domain
    pub fn generate(&self, domain_config: &DomainConfig, action: Action) -> Result<String> {
        // Validate minimum streams
        if domain_config.streams.len() < 2 {
            return Err(GoldDdlError::GenerationFailed {
                message: format!(
                    "Domain '{}' requires at least 2 streams for alignment, found {}",
                    domain_config.id,
                    domain_config.streams.len()
                ),
            });
        }

        // Build aligned stream metadata
        let aligned_streams = self.build_aligned_streams(domain_config)?;

        // Validate primary stream exists
        if aligned_streams.is_empty()
            || aligned_streams[0].role != StreamRole::Primary
        {
            return Err(GoldDdlError::GenerationFailed {
                message: format!(
                    "Domain '{}' requires a stream with role 'primary'",
                    domain_config.id
                ),
            });
        }

        // Generate SQL based on action
        match action {
            Action::Sync => self.generate_sync_sql(domain_config, &aligned_streams),
            Action::Recreate => self.generate_recreate_sql(domain_config, &aligned_streams),
        }
    }

    /// Build aligned stream metadata from domain config and stream configs
    fn build_aligned_streams(&self, domain_config: &DomainConfig) -> Result<Vec<AlignedStream>> {
        let mut streams = Vec::new();

        for stream_ref in &domain_config.streams {
            let aligned = self.build_aligned_stream(stream_ref, &domain_config.alignment)?;
            streams.push(aligned);
        }

        // Sort streams by role (primary first)
        streams.sort_by_key(|s| s.role.sort_order());

        Ok(streams)
    }

    /// Build aligned stream metadata for a single stream reference
    fn build_aligned_stream(
        &self,
        stream_ref: &StreamRef,
        alignment: &AlignmentConfig,
    ) -> Result<AlignedStream> {
        // Load stream config to get type and columns
        let stream_config = self.config_loader.load_stream_config(&stream_ref.stream_id)?;

        // Determine stream type (default to observation if not specified)
        let stream_type = self.determine_stream_type(&stream_ref.stream_id);

        // Determine null handling (override > stream type default)
        let null_handling = stream_ref
            .null_handling
            .unwrap_or_else(|| stream_type.default_null_handling());

        // Derive Gold table name
        let gold_table = self.derive_gold_table_name(&stream_ref.stream_id, &alignment.granularity);

        // Get Gold columns from stream config
        let columns = self.derive_gold_columns(&stream_config)?;

        Ok(AlignedStream {
            stream_id: stream_ref.stream_id.clone(),
            alias: stream_ref.alias.clone(),
            role: stream_ref.role,
            stream_type,
            gold_table,
            columns,
            null_handling,
        })
    }

    /// Determine stream type based on stream ID naming conventions
    /// In production, this would read from stream config
    fn determine_stream_type(&self, stream_id: &str) -> StreamType {
        if stream_id.contains("forecast") {
            StreamType::Forecast
        } else if stream_id.contains("state") || stream_id.contains("event") {
            StreamType::StateEvent
        } else if stream_id.contains("dimension") || stream_id.contains("ref") {
            StreamType::Dimension
        } else {
            StreamType::Observation
        }
    }

    /// Derive Gold table name from stream ID and granularity
    fn derive_gold_table_name(&self, stream_id: &str, granularity: &str) -> String {
        let normalized_id = stream_id.replace('-', "_");
        let suffix = self.granularity_to_suffix(granularity);
        format!("gold.{}_{}", normalized_id, suffix)
    }

    /// Convert granularity to table suffix
    fn granularity_to_suffix(&self, granularity: &str) -> &'static str {
        match granularity.to_lowercase().as_str() {
            "1 hour" | "1 hours" => "hourly",
            "1 day" | "1 days" => "daily",
            "15 minutes" | "15 minute" => "15min",
            "5 minutes" | "5 minute" => "5min",
            "1 minute" | "1 minutes" => "1min",
            _ => "hourly", // Default to hourly
        }
    }

    /// Derive Gold columns from stream config
    fn derive_gold_columns(
        &self,
        stream_config: &crate::config::StreamConfig,
    ) -> Result<Vec<String>> {
        let mut columns = vec!["bucket".to_string()];

        // Get gold_etl config to determine which fields are aggregated
        if let Some(ref gold_etl) = stream_config.gold_etl {
            if let Some(ref aggregates) = gold_etl.aggregates {
                for (field_name, field_config) in &aggregates.fields {
                    for metric in &field_config.metrics {
                        columns.push(format!("{}_{}", field_name, metric));
                    }
                }
            }
        } else {
            // Fallback: use field names from stream config with common aggregates
            for field in &stream_config.fields {
                if self.is_numeric_field(&field.field_type) {
                    columns.push(format!("{}_mean", field.name));
                }
            }
        }

        // Always include sample_count
        columns.push("sample_count".to_string());

        Ok(columns)
    }

    /// Check if a field type is numeric
    fn is_numeric_field(&self, field_type: &str) -> bool {
        matches!(
            field_type.to_lowercase().as_str(),
            "float" | "double" | "int" | "integer" | "smallint" | "bigint" | "numeric" | "decimal"
        )
    }

    /// Generate SQL for sync mode (create if not exists)
    fn generate_sync_sql(
        &self,
        domain_config: &DomainConfig,
        streams: &[AlignedStream],
    ) -> Result<String> {
        let view_name = &domain_config.alignment.view_name;
        let stream_list = self.get_stream_list(streams);
        let columns = self.column_builder.build_select_columns(streams);
        let joins = self.join_builder.build_joins(streams, domain_config.alignment.join_strategy);
        let bucket_coalesce = self.get_bucket_coalesce(streams);

        // Format columns - join with commas but not for comment-only lines
        let column_list = self.format_column_list(&columns);

        let sql = format!(
            r#"-- Aligned view for domain: {}
-- Streams: {}
-- Mode: SYNC (create if not exists)

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_matviews
        WHERE schemaname = 'gold'
          AND matviewname = '{view_name}'
    ) THEN
        CREATE MATERIALIZED VIEW gold.{view_name} AS
        SELECT
{column_list}
        {joins}
        WHERE {bucket_coalesce} >= NOW() - INTERVAL '90 days';

        RAISE NOTICE 'Created aligned view: gold.{view_name}';
    ELSE
        RAISE NOTICE 'gold.{view_name} already exists, skipping';
    END IF;
END $$;

-- Index for efficient bucket queries
CREATE INDEX IF NOT EXISTS idx_{view_name}_bucket
    ON gold.{view_name} (bucket);

-- Refresh command (run manually or via scheduler)
-- REFRESH MATERIALIZED VIEW gold.{view_name};
"#,
            domain_config.id,
            stream_list,
            view_name = view_name,
            column_list = column_list,
            joins = joins,
            bucket_coalesce = bucket_coalesce,
        );

        Ok(sql)
    }

    /// Generate SQL for recreate mode (drop and create)
    fn generate_recreate_sql(
        &self,
        domain_config: &DomainConfig,
        streams: &[AlignedStream],
    ) -> Result<String> {
        let view_name = &domain_config.alignment.view_name;
        let stream_list = self.get_stream_list(streams);
        let columns = self.column_builder.build_select_columns(streams);
        let joins = self.join_builder.build_joins(streams, domain_config.alignment.join_strategy);
        let bucket_coalesce = self.get_bucket_coalesce(streams);

        // Format columns - join with commas but not for comment-only lines
        let column_list = self.format_column_list(&columns);

        let sql = format!(
            r#"-- Aligned view for domain: {}
-- Streams: {}
-- Mode: RECREATE (drop and create)

-- Drop existing view
DROP MATERIALIZED VIEW IF EXISTS gold.{view_name} CASCADE;

-- Create aligned view
CREATE MATERIALIZED VIEW gold.{view_name} AS
SELECT
{column_list}
{joins}
WHERE {bucket_coalesce} >= NOW() - INTERVAL '90 days';

-- Index for efficient bucket queries
CREATE INDEX IF NOT EXISTS idx_{view_name}_bucket
    ON gold.{view_name} (bucket);

-- Refresh command (run manually or via scheduler)
-- REFRESH MATERIALIZED VIEW gold.{view_name};
"#,
            domain_config.id,
            stream_list,
            view_name = view_name,
            column_list = column_list,
            joins = joins,
            bucket_coalesce = bucket_coalesce,
        );

        Ok(sql)
    }

    /// Get comma-separated list of stream aliases
    fn get_stream_list(&self, streams: &[AlignedStream]) -> String {
        streams.iter().map(|s| s.alias.as_str()).collect::<Vec<_>>().join(", ")
    }

    /// Get bucket COALESCE expression for WHERE clause
    fn get_bucket_coalesce(&self, streams: &[AlignedStream]) -> String {
        if streams.len() == 1 {
            format!("{}.bucket", streams[0].alias)
        } else {
            let buckets: Vec<String> = streams
                .iter()
                .map(|s| format!("{}.bucket", s.alias))
                .collect();
            format!("COALESCE({})", buckets.join(", "))
        }
    }

    /// Format column list for SQL, handling indentation and comments properly
    ///
    /// SQL expressions get commas after them (except the last one).
    /// Comment lines never have commas.
    fn format_column_list(&self, columns: &[String]) -> String {
        // Separate SQL expressions from comments
        let mut entries: Vec<(String, bool)> = Vec::new(); // (text, is_sql_expression)

        for col in columns {
            let trimmed = col.trim();
            let is_comment_or_empty = trimmed.is_empty()
                || trimmed.starts_with("--")
                || (col.contains('\n')
                    && col
                        .trim_start_matches('\n')
                        .trim()
                        .starts_with("--"));

            entries.push((col.clone(), !is_comment_or_empty));
        }

        // Find the last SQL expression index
        let last_sql_idx = entries
            .iter()
            .enumerate()
            .rev()
            .find(|(_, (_, is_sql))| *is_sql)
            .map(|(idx, _)| idx);

        // Format each entry
        let mut result = Vec::new();
        for (idx, (col, is_sql)) in entries.iter().enumerate() {
            if *is_sql {
                // SQL expression
                let needs_comma = last_sql_idx.is_some_and(|last| idx < last);
                if needs_comma {
                    result.push(format!("            {},", col.trim()));
                } else {
                    result.push(format!("            {}", col.trim()));
                }
            } else {
                // Comment or empty - preserve newline prefix for spacing
                if col.starts_with('\n') {
                    result.push(col.clone());
                } else {
                    result.push(format!("            {}", col));
                }
            }
        }

        result.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Action, AlignmentConfig, DomainConfig, JoinStrategy, NullHandling, StreamRef};
    use std::collections::HashMap;

    /// Mock config loader for testing
    struct MockConfigLoader {
        stream_configs: HashMap<String, crate::config::StreamConfig>,
    }

    impl MockConfigLoader {
        fn new() -> Self {
            Self {
                stream_configs: HashMap::new(),
            }
        }

        fn with_stream(mut self, stream_id: &str, gold_enabled: bool) -> Self {
            let config = crate::config::StreamConfig {
                stream_id: stream_id.to_string(),
                fields: vec![
                    crate::config::FieldConfig {
                        name: "pm25".to_string(),
                        field_type: "float".to_string(),
                    },
                    crate::config::FieldConfig {
                        name: "co2".to_string(),
                        field_type: "int".to_string(),
                    },
                ],
                silver_etl: Some(crate::config::SilverEtlConfig {
                    target_table: format!("silver.{}_observations", stream_id.replace('-', "_")),
                    timestamp: None,
                }),
                gold_etl: if gold_enabled {
                    Some(crate::config::GoldEtlConfig {
                        enabled: true,
                        aggregates: Some(crate::config::AggregatesConfig {
                            granularities: vec!["1 hour".to_string()],
                            fields: {
                                let mut fields = HashMap::new();
                                fields.insert(
                                    "pm25".to_string(),
                                    crate::config::FieldMetricsConfig {
                                        metrics: vec!["mean".to_string(), "std".to_string()],
                                    },
                                );
                                fields
                            },
                        }),
                        features: None,
                        refresh_policy: None,
                    })
                } else {
                    None
                },
            };
            self.stream_configs.insert(stream_id.to_string(), config);
            self
        }
    }

    impl ConfigLoader for MockConfigLoader {
        fn load_stream_config(&self, stream_id: &str) -> Result<crate::config::StreamConfig> {
            self.stream_configs
                .get(stream_id)
                .cloned()
                .ok_or_else(|| GoldDdlError::ConfigNotFound {
                    path: format!("mock:{}", stream_id),
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
            description: "Test domain".to_string(),
            streams: vec![
                StreamRef {
                    stream_id: "air-quality".to_string(),
                    alias: "indoor".to_string(),
                    role: StreamRole::Primary,
                    null_handling: None,
                },
                StreamRef {
                    stream_id: "outdoor-weather".to_string(),
                    alias: "outdoor".to_string(),
                    role: StreamRole::Context,
                    null_handling: None,
                },
            ],
            alignment: AlignmentConfig {
                view_name: "indoor_air_quality_aligned".to_string(),
                granularity: "1 hour".to_string(),
                join_strategy: JoinStrategy::FullOuter,
                null_handling: NullHandling::Preserve,
            },
            objectives: vec![],
        }
    }

    #[test]
    fn test_generates_full_outer_join_sql() {
        let loader = MockConfigLoader::new()
            .with_stream("air-quality", true)
            .with_stream("outdoor-weather", true);

        let generator = AlignedViewGenerator::new(loader);
        let domain = create_test_domain();

        let sql = generator.generate(&domain, Action::Sync).unwrap();

        assert!(sql.contains("FULL OUTER JOIN"));
        assert!(sql.contains("gold.indoor_air_quality_aligned"));
    }

    #[test]
    fn test_aligned_view_joins_on_bucket() {
        let loader = MockConfigLoader::new()
            .with_stream("air-quality", true)
            .with_stream("outdoor-weather", true);

        let generator = AlignedViewGenerator::new(loader);
        let domain = create_test_domain();

        let sql = generator.generate(&domain, Action::Sync).unwrap();

        // First join uses simple equality
        assert!(sql.contains("indoor.bucket = outdoor.bucket"));
    }

    #[test]
    fn test_column_aliasing_uses_stream_alias() {
        let loader = MockConfigLoader::new()
            .with_stream("air-quality", true)
            .with_stream("outdoor-weather", true);

        let generator = AlignedViewGenerator::new(loader);
        let domain = create_test_domain();

        let sql = generator.generate(&domain, Action::Sync).unwrap();

        assert!(sql.contains("indoor_pm25_mean"));
        assert!(sql.contains("outdoor_pm25_mean"));
    }

    #[test]
    fn test_null_handling_for_state_events() {
        let loader = MockConfigLoader::new()
            .with_stream("air-quality", true)
            .with_stream("home-assistant-state", true);

        let generator = AlignedViewGenerator::new(loader);
        let mut domain = create_test_domain();
        domain.streams[1] = StreamRef {
            stream_id: "home-assistant-state".to_string(),
            alias: "state".to_string(),
            role: StreamRole::Actuator,
            null_handling: Some(NullHandling::CarryForward),
        };

        let sql = generator.generate(&domain, Action::Sync).unwrap();

        // State event columns should use LAG IGNORE NULLS
        assert!(sql.contains("LAG(state.pm25_mean) IGNORE NULLS"));
    }

    #[test]
    fn test_forecast_stream_lateral_join() {
        let loader = MockConfigLoader::new()
            .with_stream("air-quality", true)
            .with_stream("nws-forecast-hourly", true);

        let generator = AlignedViewGenerator::new(loader);
        let mut domain = create_test_domain();
        domain.streams[1] = StreamRef {
            stream_id: "nws-forecast-hourly".to_string(),
            alias: "forecast".to_string(),
            role: StreamRole::Context,
            null_handling: None,
        };

        let sql = generator.generate(&domain, Action::Sync).unwrap();

        assert!(sql.contains("LEFT JOIN LATERAL"));
        assert!(sql.contains("WHERE f.issued_at <="));
        assert!(sql.contains("ORDER BY f.issued_at DESC"));
    }

    #[test]
    fn test_sync_mode_checks_existence() {
        let loader = MockConfigLoader::new()
            .with_stream("air-quality", true)
            .with_stream("outdoor-weather", true);

        let generator = AlignedViewGenerator::new(loader);
        let domain = create_test_domain();

        let sql = generator.generate(&domain, Action::Sync).unwrap();

        assert!(sql.contains("IF NOT EXISTS"));
        assert!(sql.contains("pg_matviews"));
    }

    #[test]
    fn test_recreate_mode_drops_first() {
        let loader = MockConfigLoader::new()
            .with_stream("air-quality", true)
            .with_stream("outdoor-weather", true);

        let generator = AlignedViewGenerator::new(loader);
        let domain = create_test_domain();

        let sql = generator.generate(&domain, Action::Recreate).unwrap();

        assert!(sql.contains("DROP MATERIALIZED VIEW IF EXISTS"));
        assert!(sql.contains("CASCADE"));
    }

    #[test]
    fn test_insufficient_streams_error() {
        let loader = MockConfigLoader::new().with_stream("air-quality", true);

        let generator = AlignedViewGenerator::new(loader);
        let mut domain = create_test_domain();
        domain.streams = vec![StreamRef {
            stream_id: "air-quality".to_string(),
            alias: "indoor".to_string(),
            role: StreamRole::Primary,
            null_handling: None,
        }];

        let result = generator.generate(&domain, Action::Sync);

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("at least 2 streams"));
    }

    #[test]
    fn test_index_generation() {
        let loader = MockConfigLoader::new()
            .with_stream("air-quality", true)
            .with_stream("outdoor-weather", true);

        let generator = AlignedViewGenerator::new(loader);
        let domain = create_test_domain();

        let sql = generator.generate(&domain, Action::Sync).unwrap();

        assert!(sql.contains("CREATE INDEX"));
        assert!(sql.contains("idx_indoor_air_quality_aligned_bucket"));
        assert!(sql.contains("(bucket)"));
    }

    #[test]
    fn test_left_join_strategy() {
        let loader = MockConfigLoader::new()
            .with_stream("air-quality", true)
            .with_stream("outdoor-weather", true);

        let generator = AlignedViewGenerator::new(loader);
        let mut domain = create_test_domain();
        domain.alignment.join_strategy = JoinStrategy::Left;

        let sql = generator.generate(&domain, Action::Sync).unwrap();

        assert!(sql.contains("LEFT JOIN"));
        assert!(!sql.contains("FULL OUTER JOIN"));
    }

    #[test]
    fn test_primary_stream_first_in_from() {
        let loader = MockConfigLoader::new()
            .with_stream("air-quality", true)
            .with_stream("outdoor-weather", true);

        let generator = AlignedViewGenerator::new(loader);
        let mut domain = create_test_domain();
        // Reverse the order - context first, then primary
        domain.streams = vec![
            StreamRef {
                stream_id: "outdoor-weather".to_string(),
                alias: "outdoor".to_string(),
                role: StreamRole::Context,
                null_handling: None,
            },
            StreamRef {
                stream_id: "air-quality".to_string(),
                alias: "indoor".to_string(),
                role: StreamRole::Primary,
                null_handling: None,
            },
        ];

        let sql = generator.generate(&domain, Action::Sync).unwrap();

        // Despite reversed input order, primary should be first in FROM
        assert!(sql.contains("FROM gold.air_quality_hourly indoor"));
    }

    // ========== v11-005: London TDD Tests per SPEC-C01 ==========

    /// Test: FULL OUTER JOIN generates correctly for two streams
    /// Per SPEC-C01 FR-C01-002: All streams joined using FULL OUTER JOIN
    #[test]
    fn test_generates_full_outer_join_for_two_streams() {
        let loader = MockConfigLoader::new()
            .with_stream("air-quality", true)
            .with_stream("outdoor-weather", true);

        let generator = AlignedViewGenerator::new(loader);
        let domain = create_test_domain();

        let sql = generator.generate(&domain, Action::Sync).unwrap();

        // Should contain FULL OUTER JOIN keyword
        assert!(
            sql.contains("FULL OUTER JOIN"),
            "Expected FULL OUTER JOIN in SQL:\n{}",
            sql
        );

        // Both streams should be present
        assert!(
            sql.contains("gold.air_quality_hourly"),
            "Missing air_quality_hourly table"
        );
        assert!(
            sql.contains("gold.outdoor_weather_hourly"),
            "Missing outdoor_weather_hourly table"
        );

        // Join condition should reference buckets
        assert!(
            sql.contains(".bucket"),
            "Join should reference bucket columns"
        );
    }

    /// Test: Bucket COALESCE includes all streams per SPEC-C01 FR-C01-003
    /// The aligned view must use COALESCE(aq.bucket, ow.bucket, se.bucket) AS bucket
    #[test]
    fn test_bucket_coalesces_from_all_streams() {
        let loader = MockConfigLoader::new()
            .with_stream("air-quality", true)
            .with_stream("outdoor-weather", true)
            .with_stream("home-assistant-state", true);

        let generator = AlignedViewGenerator::new(loader);
        let mut domain = create_test_domain();
        domain.streams.push(StreamRef {
            stream_id: "home-assistant-state".to_string(),
            alias: "state".to_string(),
            role: StreamRole::Actuator,
            null_handling: Some(NullHandling::CarryForward),
        });

        let sql = generator.generate(&domain, Action::Sync).unwrap();

        // Check for COALESCE with bucket references
        assert!(
            sql.contains("COALESCE("),
            "Expected COALESCE in bucket expression:\n{}",
            sql
        );

        // Should have bucket references from all streams
        assert!(
            sql.contains("indoor.bucket") || sql.contains("AS bucket"),
            "Expected bucket column from indoor stream"
        );
        assert!(
            sql.contains("outdoor.bucket") || sql.contains("AS bucket"),
            "Expected bucket column from outdoor stream"
        );
    }

    /// Test: Observation streams preserve NULL per ADR-FE001-004
    /// Observation columns should NOT use LOCF - NULL means "no data"
    #[test]
    fn test_observation_preserves_null() {
        // Set up loader with both streams
        let loader = MockConfigLoader::new()
            .with_stream("air-quality", true)
            .with_stream("outdoor-weather", true);

        let generator = AlignedViewGenerator::new(loader);
        let mut domain = create_test_domain();
        // Configure with observation streams
        domain.streams = vec![
            StreamRef {
                stream_id: "air-quality".to_string(),
                alias: "indoor".to_string(),
                role: StreamRole::Primary,
                null_handling: None, // Default for observation is Preserve
            },
            StreamRef {
                stream_id: "outdoor-weather".to_string(),
                alias: "outdoor".to_string(),
                role: StreamRole::Context,
                null_handling: None,
            },
        ];

        let sql = generator.generate(&domain, Action::Sync).unwrap();

        // Observation columns should be simple passthrough (no LAG IGNORE NULLS)
        // They should appear as: indoor.pm25_mean AS indoor_pm25_mean
        assert!(
            sql.contains("indoor.pm25_mean AS indoor_pm25_mean")
                || sql.contains("indoor.pm25_std AS indoor_pm25_std"),
            "Observation columns should use simple passthrough, not LOCF:\n{}",
            sql
        );

        // Verify the column doesn't have LAG in same line (not LOCF)
        let lines: Vec<&str> = sql.lines().collect();
        for line in lines {
            if line.contains("indoor.pm25_mean AS") {
                assert!(
                    !line.contains("LAG("),
                    "Observation column should NOT use LAG (LOCF):\n{}",
                    line
                );
            }
        }
    }

    /// Test: State event streams use LOCF (carry forward) per ADR-FE001-004
    /// State columns should use COALESCE(current, LAG(...) IGNORE NULLS)
    #[test]
    fn test_state_event_carries_forward() {
        let loader = MockConfigLoader::new()
            .with_stream("air-quality", true)
            .with_stream("home-assistant-state", true);

        let generator = AlignedViewGenerator::new(loader);
        let mut domain = create_test_domain();
        domain.streams[1] = StreamRef {
            stream_id: "home-assistant-state".to_string(),
            alias: "state".to_string(),
            role: StreamRole::Actuator,
            null_handling: Some(NullHandling::CarryForward), // Explicit LOCF
        };

        let sql = generator.generate(&domain, Action::Sync).unwrap();

        // State event columns should use LOCF pattern:
        // COALESCE(state.column, LAG(state.column) IGNORE NULLS OVER ...)
        assert!(
            sql.contains("LAG(state.") && sql.contains("IGNORE NULLS"),
            "State event columns should use LAG IGNORE NULLS (LOCF):\n{}",
            sql
        );

        // Should have COALESCE wrapper
        assert!(
            sql.contains("COALESCE"),
            "State event columns should be wrapped in COALESCE:\n{}",
            sql
        );
    }

    /// Test: Column aliasing follows {alias}_{metric} convention per SPEC-C01 FR-C01-005
    #[test]
    fn test_column_aliasing_convention() {
        let loader = MockConfigLoader::new()
            .with_stream("air-quality", true)
            .with_stream("outdoor-weather", true);

        let generator = AlignedViewGenerator::new(loader);
        let domain = create_test_domain();

        let sql = generator.generate(&domain, Action::Sync).unwrap();

        // Columns should follow {alias}_{metric} pattern
        // indoor stream uses alias "indoor"
        assert!(
            sql.contains("indoor_pm25_mean") || sql.contains("AS indoor_pm25_mean"),
            "Expected indoor_pm25_mean column alias:\n{}",
            sql
        );

        // outdoor stream uses alias "outdoor"
        assert!(
            sql.contains("outdoor_pm25_mean") || sql.contains("AS outdoor_pm25_mean"),
            "Expected outdoor_pm25_mean column alias:\n{}",
            sql
        );

        // Should NOT have duplicate prefixing (not indoor_indoor_pm25)
        assert!(
            !sql.contains("indoor_indoor_"),
            "Should not have duplicate alias prefixing"
        );
        assert!(
            !sql.contains("outdoor_outdoor_"),
            "Should not have duplicate alias prefixing"
        );
    }
}
