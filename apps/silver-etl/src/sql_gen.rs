//! SQL generation from Silver ETL configuration
//!
//! Generates DuckDB SQL statements from config-driven field mappings,
//! transforms, and DQ rules.
//!
//! # Architecture
//!
//! The SqlGenerator translates `SilverEtlConfig` into executable DuckDB SQL:
//!
//! 1. **SELECT clause**: Field mappings with transforms and type casts
//! 2. **FROM clause**: `read_parquet()` with glob pattern for Bronze data
//! 3. **WHERE clause**: Incremental watermark filtering
//! 4. **INSERT with ON CONFLICT**: Upsert to TimescaleDB via postgres extension
//!
//! # DuckDB-Specific Syntax
//!
//! - `read_parquet('/path/**/*.parquet')` for reading Bronze Parquet files
//! - `json_extract(raw_payload, '$.path')` for JSON field extraction
//! - `json_extract_string()` for string extraction from JSON
//! - `to_timestamp()` for Unix epoch conversion
//! - `pg.silver.table_name` for PostgreSQL ATTACH writes

use neural_core::{
    ConversionFormula, DeduplicationConfig, DeduplicationStrategy, IdentityField,
    IncrementalConfig, SilverEtlConfig, SilverFieldMapping, TimestampMapping, TimestampTransform,
    TransformConfig,
};
use thiserror::Error;

// =============================================================================
// Error Types
// =============================================================================

/// SQL generation errors
#[derive(Debug, Error)]
pub enum SqlGenError {
    #[error("Configuration validation failed: {0}")]
    ConfigValidation(String),

    #[error("Unsupported transform type: {0}")]
    UnsupportedTransform(String),

    #[error("Invalid source path: {0}")]
    InvalidSourcePath(String),

    #[error("Missing required field: {0}")]
    MissingField(String),
}

// =============================================================================
// SQL Generator
// =============================================================================

/// SQL Generator for Silver ETL
///
/// Translates SilverEtlConfig into DuckDB SQL that:
/// - Reads from Bronze Parquet files
/// - Applies field mappings and transforms
/// - Generates DQ flag expressions
/// - Writes to TimescaleDB via postgres extension
pub struct SqlGenerator {
    // Configuration for SQL generation
    // Currently stateless, but may hold options in future
}

impl SqlGenerator {
    /// Create a new SQL generator
    pub fn new() -> Self {
        Self {}
    }

    // =========================================================================
    // Public API
    // =========================================================================

    /// Generate a SELECT expression for a single field mapping
    ///
    /// # Arguments
    /// * `mapping` - Field mapping configuration
    ///
    /// # Returns
    /// SQL expression like `CAST(json_extract(raw_payload, '$.pm02') AS DOUBLE) AS pm25`
    pub fn generate_select_expr(&self, mapping: &SilverFieldMapping) -> String {
        let source_ref = self.build_source_reference(&mapping.source_path);
        let type_cast = self.map_pg_type_to_duckdb(&mapping.column_type);

        match &mapping.transform {
            Some(TransformConfig::UnitConversion { formula, .. }) => {
                let converted = self.apply_formula_sql(&source_ref, type_cast, formula);
                format!("({}) AS {}", converted, mapping.target_column)
            }
            Some(TransformConfig::JsonExtract { path }) => {
                // For JsonExtract, use the explicit path provided
                let json_expr = format!("json_extract(raw_payload, '{}')", path);
                format!(
                    "CAST({} AS {}) AS {}",
                    json_expr, type_cast, mapping.target_column
                )
            }
            Some(TransformConfig::Expression { expression }) => {
                // Replace {value} placeholder with source reference
                let replaced = expression.replace("{value}", &source_ref);
                format!(
                    "CAST({} AS {}) AS {}",
                    replaced, type_cast, mapping.target_column
                )
            }
            Some(TransformConfig::Lookup { table }) => {
                let case_expr = self.generate_lookup_case(&source_ref, table);
                format!("{} AS {}", case_expr, mapping.target_column)
            }
            Some(TransformConfig::Timestamp { format }) => {
                let ts_expr = self.generate_timestamp_transform_expr(&source_ref, format);
                format!("{} AS {}", ts_expr, mapping.target_column)
            }
            Some(TransformConfig::Computed { expression, .. }) => {
                // Computed expressions are direct SQL
                format!("({}) AS {}", expression, mapping.target_column)
            }
            None => {
                // No transform, just extract and cast
                format!(
                    "CAST({} AS {}) AS {}",
                    source_ref, type_cast, mapping.target_column
                )
            }
        }
    }

    /// Generate a timestamp transformation expression
    ///
    /// # Arguments
    /// * `ts_mapping` - Timestamp mapping configuration
    ///
    /// # Returns
    /// SQL expression like `to_timestamp(timestamp / 1000000) AS observation_time`
    pub fn generate_timestamp_expr(&self, ts_mapping: &TimestampMapping) -> String {
        let source = &ts_mapping.source_field;

        // Check if source is a JSON path
        let source_ref = if source.starts_with("raw_payload.") || source.starts_with("context.") {
            self.build_source_reference(source)
        } else {
            source.clone()
        };

        match &ts_mapping.transform {
            TimestampTransform::MicrosecondsToTimestamp => {
                format!(
                    "to_timestamp({} / 1000000) AS {}",
                    source_ref, ts_mapping.target_field
                )
            }
            TimestampTransform::UnixSeconds => {
                format!(
                    "to_timestamp({}) AS {}",
                    source_ref, ts_mapping.target_field
                )
            }
            TimestampTransform::Iso8601 => {
                // For ISO8601, we need json_extract_string for string extraction
                let string_ref = if source.starts_with("raw_payload.") {
                    let json_path = source.strip_prefix("raw_payload.").unwrap();
                    format!("json_extract_string(raw_payload, '$.{}')", json_path)
                } else if source.starts_with("context.") {
                    let json_path = source.strip_prefix("context.").unwrap();
                    format!("json_extract_string(context, '$.{}')", json_path)
                } else {
                    source_ref
                };
                format!(
                    "CAST({} AS TIMESTAMPTZ) AS {}",
                    string_ref, ts_mapping.target_field
                )
            }
            TimestampTransform::NwsDuration => {
                // NWS uses ISO 8601 format with optional duration suffix
                format!(
                    "CAST({} AS TIMESTAMPTZ) AS {}",
                    source_ref, ts_mapping.target_field
                )
            }
        }
    }

    /// Generate an identity field expression
    ///
    /// # Arguments
    /// * `identity` - Identity field configuration
    ///
    /// # Returns
    /// SQL expression like `ndp_id AS ndp_id` or `json_extract_string(context, '$.location.path') AS location_path`
    pub fn generate_identity_expr(&self, identity: &IdentityField) -> String {
        let source = &identity.source;

        // Check if it's a JSON path or direct column
        if source.contains('.') {
            // It's a JSON path - determine which column
            if source.starts_with("context.") {
                let json_path = source.strip_prefix("context.").unwrap();
                format!(
                    "json_extract_string(context, '$.{}') AS {}",
                    json_path, identity.target
                )
            } else if source.starts_with("raw_payload.") {
                let json_path = source.strip_prefix("raw_payload.").unwrap();
                format!(
                    "json_extract_string(raw_payload, '$.{}') AS {}",
                    json_path, identity.target
                )
            } else {
                // Assume context if no explicit prefix
                format!(
                    "json_extract_string(context, '$.{}') AS {}",
                    source, identity.target
                )
            }
        } else {
            // Direct column reference
            format!("{} AS {}", source, identity.target)
        }
    }

    /// Generate the complete SELECT clause
    ///
    /// # Arguments
    /// * `config` - Silver ETL configuration
    ///
    /// # Returns
    /// Complete SELECT clause with all fields
    pub fn generate_select_clause(&self, config: &SilverEtlConfig) -> String {
        let mut expressions = Vec::new();

        // 1. Ingestion timestamp (always current_timestamp)
        expressions.push("current_timestamp AS ingestion_time".to_string());

        // 2. Observation timestamp
        expressions.push(self.generate_timestamp_expr(&config.timestamp));

        // 3. Identity fields
        for identity in &config.identity_fields {
            expressions.push(self.generate_identity_expr(identity));
        }

        // 4. Field mappings
        for mapping in &config.field_mappings {
            expressions.push(self.generate_select_expr(mapping));
        }

        // 5. DQ flags array (if enabled)
        if config.dq_output.enabled {
            // Placeholder for DQ flags - will be generated by DqSqlGenerator
            // Use NULL - DuckDB's []::VARCHAR[] doesn't serialize correctly
            // for PostgreSQL COPY via postgres extension
            expressions.push(format!(
                "NULL::TEXT[] AS {}",
                config.dq_output.target_column
            ));
        }

        format!("SELECT\n  {}", expressions.join(",\n  "))
    }

    /// Generate the FROM clause with parquet glob pattern
    ///
    /// # Arguments
    /// * `stream_id` - The stream identifier
    /// * `bronze_path` - Base path to Bronze data
    ///
    /// # Returns
    /// FROM clause like `FROM read_parquet('/data/raw/air-quality/**/*.parquet')`
    pub fn generate_from_clause(&self, stream_id: &str, bronze_path: &str) -> String {
        format!(
            "FROM read_parquet('{}/{}/**/*.parquet')",
            bronze_path, stream_id
        )
    }

    /// Generate the WHERE clause for incremental loading
    ///
    /// # Arguments
    /// * `incremental` - Incremental load configuration
    /// * `table` - Target table name (e.g., "silver.air_quality")
    ///
    /// # Returns
    /// WHERE clause with watermark condition, or empty string if not incremental
    pub fn generate_where_clause(&self, incremental: &IncrementalConfig, table: &str) -> String {
        if !incremental.enabled {
            return String::new();
        }

        // Generate the timestamp expression for comparison
        // We assume the timestamp is in microseconds (most common case)
        let ts_expr = "to_timestamp(timestamp / 1000000)";

        // Build the watermark subquery - use pg. prefix for PostgreSQL
        let pg_table = if table.starts_with("pg.") {
            table.to_string()
        } else {
            format!("pg.{}", table)
        };

        // Cast to TIMESTAMP for DuckDB interval arithmetic compatibility
        // DuckDB doesn't support TIMESTAMPTZ - INTERVAL directly
        format!(
            "WHERE {}::TIMESTAMP > ((\n  SELECT COALESCE(MAX({}), '1970-01-01'::TIMESTAMP)\n  FROM {}\n)::TIMESTAMP - INTERVAL '{}')\nAND {}::TIMESTAMP <= (current_timestamp::TIMESTAMP - INTERVAL '{}')",
            ts_expr,
            incremental.watermark_column,
            pg_table,
            incremental.lag_interval,
            ts_expr,
            incremental.lag_interval
        )
    }

    /// Generate the ON CONFLICT clause for upsert
    ///
    /// # Arguments
    /// * `dedup` - Deduplication configuration
    /// * `columns` - All column names in the INSERT
    ///
    /// # Returns
    /// ON CONFLICT clause like `ON CONFLICT (observation_time, ndp_id) DO UPDATE SET ...`
    pub fn generate_upsert_clause(&self, dedup: &DeduplicationConfig, columns: &[&str]) -> String {
        if !dedup.enabled {
            return String::new();
        }

        let key_cols = dedup.key_columns.join(", ");

        match dedup.strategy {
            DeduplicationStrategy::Skip => {
                format!("ON CONFLICT ({}) DO NOTHING", key_cols)
            }
            DeduplicationStrategy::Upsert | DeduplicationStrategy::Replace => {
                // Get non-key columns for UPDATE SET
                let update_cols: Vec<_> = columns
                    .iter()
                    .filter(|c| !dedup.key_columns.contains(&c.to_string()))
                    .collect();

                if update_cols.is_empty() {
                    return format!("ON CONFLICT ({}) DO NOTHING", key_cols);
                }

                let set_clauses: Vec<String> = update_cols
                    .iter()
                    .map(|c| format!("{} = EXCLUDED.{}", c, c))
                    .collect();

                format!(
                    "ON CONFLICT ({}) DO UPDATE SET\n  {}",
                    key_cols,
                    set_clauses.join(",\n  ")
                )
            }
        }
    }

    /// Generate complete ETL SQL statement
    ///
    /// # Arguments
    /// * `config` - Silver ETL configuration
    /// * `stream_id` - The stream identifier
    /// * `bronze_path` - Base path to Bronze data
    ///
    /// # Returns
    /// Complete INSERT...SELECT statement for ETL
    pub fn generate_etl_sql(
        &self,
        config: &SilverEtlConfig,
        stream_id: &str,
        bronze_path: &str,
    ) -> Result<String, SqlGenError> {
        // Validate config first
        if config.target_table.is_empty() {
            return Err(SqlGenError::MissingField("target_table".to_string()));
        }

        // Build column list
        let mut columns: Vec<&str> = vec!["ingestion_time"];
        columns.push(&config.timestamp.target_field);

        for identity in &config.identity_fields {
            columns.push(&identity.target);
        }

        for mapping in &config.field_mappings {
            columns.push(&mapping.target_column);
        }

        if config.dq_output.enabled {
            columns.push(&config.dq_output.target_column);
        }

        // Generate clauses
        let select_clause = self.generate_select_clause(config);
        let from_clause = self.generate_from_clause(stream_id, bronze_path);
        let where_clause = self.generate_where_clause(&config.incremental, &config.target_table);
        let upsert_clause = self.generate_upsert_clause(&config.deduplication, &columns);

        // Build the PostgreSQL target table with pg. prefix
        let pg_table = if config.target_table.starts_with("pg.") {
            config.target_table.clone()
        } else {
            format!("pg.{}", config.target_table)
        };

        // Assemble complete SQL
        let mut sql = format!(
            "INSERT INTO {} ({})\n{}\n{}",
            pg_table,
            columns.join(", "),
            select_clause,
            from_clause
        );

        if !where_clause.is_empty() {
            sql.push('\n');
            sql.push_str(&where_clause);
        }

        if !upsert_clause.is_empty() {
            sql.push('\n');
            sql.push_str(&upsert_clause);
        }

        Ok(sql)
    }

    // =========================================================================
    // Private Helpers
    // =========================================================================

    /// Build a source reference from a path
    ///
    /// Converts paths like `raw_payload.pm02` to `json_extract(raw_payload, '$.pm02')`
    fn build_source_reference(&self, source_path: &str) -> String {
        if source_path.starts_with("raw_payload.") {
            let json_path = source_path.strip_prefix("raw_payload.").unwrap();
            format!("json_extract(raw_payload, '$.{}')", json_path)
        } else if source_path.starts_with("context.") {
            let json_path = source_path.strip_prefix("context.").unwrap();
            format!("json_extract(context, '$.{}')", json_path)
        } else {
            // Direct column reference
            source_path.to_string()
        }
    }

    /// Map PostgreSQL types to DuckDB types
    fn map_pg_type_to_duckdb(&self, pg_type: &str) -> &'static str {
        match pg_type {
            "double_precision" | "float8" => "DOUBLE",
            "real" | "float4" => "REAL",
            "integer" | "int4" => "INTEGER",
            "smallint" | "int2" => "SMALLINT",
            "bigint" | "int8" => "BIGINT",
            "text" | "varchar" => "TEXT",
            "boolean" | "bool" => "BOOLEAN",
            "timestamptz" | "timestamp with time zone" => "TIMESTAMPTZ",
            "timestamp" | "timestamp without time zone" => "TIMESTAMP",
            "jsonb" | "json" => "JSON",
            _ => "TEXT",
        }
    }

    /// Apply a conversion formula in SQL
    fn apply_formula_sql(
        &self,
        source_ref: &str,
        type_cast: &str,
        formula: &ConversionFormula,
    ) -> String {
        match formula {
            ConversionFormula::Linear { scale, offset } => {
                // Optimize for common cases
                if (*scale - 1.0).abs() < f64::EPSILON && (*offset).abs() < f64::EPSILON {
                    // No-op conversion
                    format!("CAST({} AS {})", source_ref, type_cast)
                } else if (*scale - 1.0).abs() < f64::EPSILON {
                    // Only offset
                    format!("CAST({} AS {}) + {}", source_ref, type_cast, offset)
                } else if (*offset).abs() < f64::EPSILON {
                    // Only scale
                    format!("CAST({} AS {}) * {}", source_ref, type_cast, scale)
                } else {
                    // Full linear transform
                    format!(
                        "CAST({} AS {}) * {} + {}",
                        source_ref, type_cast, scale, offset
                    )
                }
            }
            ConversionFormula::Custom { code } => {
                // Replace {value} placeholder
                code.replace("{value}", source_ref)
            }
        }
    }

    /// Generate a CASE expression for lookup tables
    fn generate_lookup_case(
        &self,
        source_ref: &str,
        table: &std::collections::HashMap<String, String>,
    ) -> String {
        let cases: Vec<String> = table
            .iter()
            .map(|(key, value)| {
                format!(
                    "WHEN {} = '{}' THEN '{}'",
                    source_ref,
                    escape_sql_string(key),
                    escape_sql_string(value)
                )
            })
            .collect();

        format!("CASE {} ELSE NULL END", cases.join(" "))
    }

    /// Generate timestamp transform expression for a specific format
    fn generate_timestamp_transform_expr(
        &self,
        source_ref: &str,
        format: &TimestampTransform,
    ) -> String {
        match format {
            TimestampTransform::MicrosecondsToTimestamp => {
                format!("to_timestamp({} / 1000000.0)", source_ref)
            }
            TimestampTransform::UnixSeconds => {
                format!("to_timestamp({})", source_ref)
            }
            TimestampTransform::Iso8601 | TimestampTransform::NwsDuration => {
                format!("CAST({} AS TIMESTAMPTZ)", source_ref)
            }
        }
    }
}

impl Default for SqlGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Utility Functions
// =============================================================================

/// Escape single quotes in SQL strings
fn escape_sql_string(s: &str) -> String {
    s.replace('\'', "''")
}

// =============================================================================
// PIVOT SQL Generation for Pre-Transformed Data
// =============================================================================

/// Metric mapping for PIVOT SQL generation
///
/// Maps a metric name (from pre_transformed.metric_name) to a target column
#[derive(Debug, Clone, PartialEq)]
pub struct MetricMapping {
    /// The metric name as stored in pre_transformed.metric_name column
    pub metric_name: String,
    /// The target column name in the Silver table
    pub target_column: String,
}

impl MetricMapping {
    /// Create a new metric mapping
    pub fn new(metric_name: impl Into<String>, target_column: impl Into<String>) -> Self {
        Self {
            metric_name: metric_name.into(),
            target_column: target_column.into(),
        }
    }
}

impl SqlGenerator {
    /// Generate SQL for pre-transformed data (PIVOT pattern)
    ///
    /// Instead of reading from Parquet with json_extract, this reads from the
    /// pre_transformed temp table and pivots metric rows into columns.
    ///
    /// # Pre-Transformed Table Schema
    ///
    /// ```sql
    /// pre_transformed (
    ///     issue_time TIMESTAMPTZ,
    ///     valid_time TIMESTAMPTZ,
    ///     ndp_id VARCHAR,
    ///     location_id VARCHAR,
    ///     metric_name VARCHAR,
    ///     value DOUBLE
    /// )
    /// ```
    ///
    /// # Target Output
    ///
    /// ```sql
    /// INSERT INTO pg.silver.nws_forecasts (
    ///     ingestion_time, issue_time, valid_time, ndp_id, temperature_c, wind_speed_kmh
    /// )
    /// SELECT
    ///     current_timestamp AS ingestion_time,
    ///     issue_time,
    ///     valid_time,
    ///     ndp_id,
    ///     MAX(CASE WHEN metric_name = 'temperature' THEN value END) AS temperature_c,
    ///     MAX(CASE WHEN metric_name = 'wind_speed' THEN value END) AS wind_speed_kmh
    /// FROM pre_transformed
    /// GROUP BY issue_time, valid_time, ndp_id
    /// ON CONFLICT (issue_time, valid_time, ndp_id) DO UPDATE SET ...
    /// ```
    ///
    /// # Arguments
    ///
    /// * `config` - Silver ETL configuration
    /// * `metric_mappings` - List of (metric_name, target_column) pairs
    ///
    /// # Returns
    ///
    /// Complete INSERT...SELECT statement with PIVOT pattern
    pub fn generate_pivot_sql(
        &self,
        config: &SilverEtlConfig,
        metric_mappings: &[MetricMapping],
    ) -> Result<String, SqlGenError> {
        // Validate config
        if config.target_table.is_empty() {
            return Err(SqlGenError::MissingField("target_table".to_string()));
        }

        // Build column list for INSERT
        let mut columns: Vec<String> = vec![
            "ingestion_time".to_string(),
            config.timestamp.target_field.clone(), // issue_time
        ];

        // Add valid_time if configured
        if let Some(ref valid_ts) = config.valid_timestamp {
            columns.push(valid_ts.target_field.clone());
        }

        // Add identity fields (ndp_id, location_id, etc.)
        for identity in &config.identity_fields {
            columns.push(identity.target.clone());
        }

        // Add metric columns
        for mapping in metric_mappings {
            columns.push(mapping.target_column.clone());
        }

        // Add DQ flags column if enabled
        if config.dq_output.enabled {
            columns.push(config.dq_output.target_column.clone());
        }

        // Build SELECT expressions
        let mut select_exprs: Vec<String> = vec![
            "current_timestamp AS ingestion_time".to_string(),
            format!("{} AS {}", "issue_time", config.timestamp.target_field),
        ];

        // Add valid_time expression
        if let Some(ref valid_ts) = config.valid_timestamp {
            select_exprs.push(format!("valid_time AS {}", valid_ts.target_field));
        }

        // Identity fields from pre_transformed
        for identity in &config.identity_fields {
            let source_col = self.identity_source_to_pre_transformed_column(&identity.source);
            if source_col == identity.target {
                select_exprs.push(source_col);
            } else {
                select_exprs.push(format!("{} AS {}", source_col, identity.target));
            }
        }

        // Pivot expressions for each metric
        for mapping in metric_mappings {
            select_exprs.push(format!(
                "MAX(CASE WHEN metric_name = '{}' THEN value END) AS {}",
                escape_sql_string(&mapping.metric_name),
                mapping.target_column
            ));
        }

        // Add empty DQ flags if enabled (placeholder - DQ processing happens elsewhere)
        // Use NULL - DuckDB's []::VARCHAR[] doesn't serialize correctly for PostgreSQL COPY
        if config.dq_output.enabled {
            select_exprs.push(format!(
                "NULL::TEXT[] AS {}",
                config.dq_output.target_column
            ));
        }

        // Build GROUP BY columns (timestamps + identity fields)
        let mut group_by_cols: Vec<String> = vec!["issue_time".to_string()];
        if config.valid_timestamp.is_some() {
            group_by_cols.push("valid_time".to_string());
        }
        for identity in &config.identity_fields {
            let source_col = self.identity_source_to_pre_transformed_column(&identity.source);
            group_by_cols.push(source_col);
        }

        // Build the PostgreSQL target table with pg. prefix
        let pg_table = if config.target_table.starts_with("pg.") {
            config.target_table.clone()
        } else {
            format!("pg.{}", config.target_table)
        };

        // Build upsert clause
        let column_refs: Vec<&str> = columns.iter().map(|s| s.as_str()).collect();
        let upsert_clause = self.generate_upsert_clause(&config.deduplication, &column_refs);

        // Assemble complete SQL
        let mut sql = format!(
            "INSERT INTO {} ({})\nSELECT\n  {}\nFROM pre_transformed\nGROUP BY {}",
            pg_table,
            columns.join(", "),
            select_exprs.join(",\n  "),
            group_by_cols.join(", ")
        );

        if !upsert_clause.is_empty() {
            sql.push('\n');
            sql.push_str(&upsert_clause);
        }

        Ok(sql)
    }

    /// Extract metric mappings from field_mappings for PIVOT
    ///
    /// For pre-transformed data, the source_path typically contains the metric path
    /// (e.g., "raw_payload.properties.temperature"). This extracts the metric name
    /// (the last segment) for use in PIVOT SQL.
    ///
    /// # Arguments
    ///
    /// * `config` - Silver ETL configuration
    ///
    /// # Returns
    ///
    /// Vector of MetricMapping with metric_name and target_column pairs
    pub fn extract_metric_mappings(config: &SilverEtlConfig) -> Vec<MetricMapping> {
        config
            .field_mappings
            .iter()
            .map(|m| {
                // Extract metric name from source_path
                // e.g., "raw_payload.properties.temperature" -> "temperature"
                let metric_name = m
                    .source_path
                    .rsplit('.')
                    .next()
                    .unwrap_or(&m.source_path)
                    .to_string();

                MetricMapping::new(metric_name, m.target_column.clone())
            })
            .collect()
    }

    /// Map identity field source to pre_transformed column name
    ///
    /// The pre_transformed table has specific column names (ndp_id, location_id).
    /// This maps config source paths to the appropriate column.
    fn identity_source_to_pre_transformed_column(&self, source: &str) -> String {
        if source == "ndp_id" || source.ends_with(".ndp_id") {
            "ndp_id".to_string()
        } else if source.contains("location") {
            "location_id".to_string()
        } else {
            // For unknown sources, use the last segment as column name
            source.rsplit('.').next().unwrap_or(source).to_string()
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use neural_core::config::silver_etl::{ValidTimestampMapping, ValidTimestampSource};
    use neural_core::{DqOutputConfig, IdentityField};

    // ============================================================
    // Test 1: Generate SELECT for simple field (no transform)
    // ============================================================
    #[test]
    fn test_generate_select_simple_field() {
        let mapping = SilverFieldMapping {
            source_path: "raw_payload.pm02".to_string(),
            target_column: "pm25".to_string(),
            column_type: "double_precision".to_string(),
            nullable: true,
            transform: None,
            dq_rules: vec![],
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_select_expr(&mapping);

        assert_eq!(
            sql,
            "CAST(json_extract(raw_payload, '$.pm02') AS DOUBLE) AS pm25"
        );
    }

    // ============================================================
    // Test 2: Generate SELECT with unit conversion transform
    // ============================================================
    #[test]
    fn test_generate_select_with_unit_conversion() {
        let mapping = SilverFieldMapping {
            source_path: "raw_payload.main.temp".to_string(),
            target_column: "temperature_c".to_string(),
            column_type: "double_precision".to_string(),
            nullable: true,
            transform: Some(TransformConfig::UnitConversion {
                from: "kelvin".to_string(),
                to: "celsius".to_string(),
                formula: ConversionFormula::Linear {
                    scale: 1.0,
                    offset: -273.15,
                },
            }),
            dq_rules: vec![],
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_select_expr(&mapping);

        assert_eq!(
            sql,
            "(CAST(json_extract(raw_payload, '$.main.temp') AS DOUBLE) + -273.15) AS temperature_c"
        );
    }

    // ============================================================
    // Test 3: Generate SELECT with json_extract transform
    // ============================================================
    #[test]
    fn test_generate_select_with_json_extract() {
        let mapping = SilverFieldMapping {
            source_path: "raw_payload".to_string(),
            target_column: "aqi".to_string(),
            column_type: "integer".to_string(),
            nullable: true,
            transform: Some(TransformConfig::JsonExtract {
                path: "$.list[0].main.aqi".to_string(),
            }),
            dq_rules: vec![],
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_select_expr(&mapping);

        assert_eq!(
            sql,
            "CAST(json_extract(raw_payload, '$.list[0].main.aqi') AS INTEGER) AS aqi"
        );
    }

    // ============================================================
    // Test 4: Generate timestamp transform expression
    // ============================================================
    #[test]
    fn test_generate_timestamp_microseconds() {
        let ts_mapping = TimestampMapping {
            source_field: "timestamp".to_string(),
            target_field: "observation_time".to_string(),
            transform: TimestampTransform::MicrosecondsToTimestamp,
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_timestamp_expr(&ts_mapping);

        assert_eq!(sql, "to_timestamp(timestamp / 1000000) AS observation_time");
    }

    #[test]
    fn test_generate_timestamp_iso8601() {
        let ts_mapping = TimestampMapping {
            source_field: "raw_payload.properties.timestamp".to_string(),
            target_field: "observation_time".to_string(),
            transform: TimestampTransform::Iso8601,
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_timestamp_expr(&ts_mapping);

        assert_eq!(
            sql,
            "CAST(json_extract_string(raw_payload, '$.properties.timestamp') AS TIMESTAMPTZ) AS observation_time"
        );
    }

    // ============================================================
    // Test 5: Generate identity field expressions
    // ============================================================
    #[test]
    fn test_generate_identity_field_simple() {
        let identity = IdentityField {
            source: "ndp_id".to_string(),
            target: "ndp_id".to_string(),
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_identity_expr(&identity);

        assert_eq!(sql, "ndp_id AS ndp_id");
    }

    #[test]
    fn test_generate_identity_field_json_path() {
        let identity = IdentityField {
            source: "context.location.path".to_string(),
            target: "location_path".to_string(),
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_identity_expr(&identity);

        assert_eq!(
            sql,
            "json_extract_string(context, '$.location.path') AS location_path"
        );
    }

    // ============================================================
    // Test 6: Generate complete SELECT clause
    // ============================================================
    #[test]
    fn test_generate_select_clause_complete() {
        let config = SilverEtlConfig {
            enabled: true,
            target_table: "silver.air_quality".to_string(),
            target_schema: None,
            timestamp: TimestampMapping {
                source_field: "timestamp".to_string(),
                target_field: "observation_time".to_string(),
                transform: TimestampTransform::MicrosecondsToTimestamp,
            },
            valid_timestamp: None,
            pre_transform: None,
            identity_fields: vec![IdentityField {
                source: "ndp_id".to_string(),
                target: "ndp_id".to_string(),
            }],
            field_mappings: vec![SilverFieldMapping {
                source_path: "raw_payload.pm02".to_string(),
                target_column: "pm25".to_string(),
                column_type: "double_precision".to_string(),
                nullable: true,
                transform: None,
                dq_rules: vec![],
            }],
            dq_rules: vec![],
            dq_output: DqOutputConfig::default(),
            deduplication: DeduplicationConfig::default(),
            incremental: IncrementalConfig::default(),
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_select_clause(&config);

        assert!(sql.contains("to_timestamp(timestamp / 1000000) AS observation_time"));
        assert!(sql.contains("ndp_id AS ndp_id"));
        assert!(sql.contains("json_extract(raw_payload, '$.pm02')"));
    }

    // ============================================================
    // Test 7: Generate FROM clause with parquet glob
    // ============================================================
    #[test]
    fn test_generate_from_clause() {
        let gen = SqlGenerator::new();
        let sql = gen.generate_from_clause("air-quality", "/data/raw");

        assert_eq!(
            sql,
            "FROM read_parquet('/data/raw/air-quality/**/*.parquet')"
        );
    }

    // ============================================================
    // Test 8: Generate WHERE clause for incremental
    // ============================================================
    #[test]
    fn test_generate_where_clause_incremental() {
        let incremental = IncrementalConfig {
            enabled: true,
            watermark_column: "observation_time".to_string(),
            lag_interval: "5 minutes".to_string(),
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_where_clause(&incremental, "silver.air_quality");

        // Check timestamp expression with DuckDB-compatible TIMESTAMP casts
        assert!(sql.contains("::TIMESTAMP >"));
        assert!(sql.contains("SELECT COALESCE(MAX(observation_time)"));
        assert!(sql.contains("FROM pg.silver.air_quality"));
        assert!(sql.contains("INTERVAL '5 minutes'"));
        // DuckDB requires casting to TIMESTAMP for interval arithmetic
        assert!(sql.contains("::TIMESTAMP - INTERVAL"));
        assert!(sql.contains("current_timestamp::TIMESTAMP"));
    }

    #[test]
    fn test_generate_where_clause_disabled() {
        let incremental = IncrementalConfig {
            enabled: false,
            watermark_column: "observation_time".to_string(),
            lag_interval: "5 minutes".to_string(),
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_where_clause(&incremental, "silver.air_quality");

        assert!(sql.is_empty());
    }

    // ============================================================
    // Test 9: Generate INSERT with ON CONFLICT (upsert)
    // ============================================================
    #[test]
    fn test_generate_upsert_clause() {
        let dedup = DeduplicationConfig {
            enabled: true,
            key_columns: vec!["observation_time".to_string(), "ndp_id".to_string()],
            strategy: DeduplicationStrategy::Upsert,
        };
        let columns = vec!["observation_time", "ndp_id", "pm25", "dq_flags"];

        let gen = SqlGenerator::new();
        let sql = gen.generate_upsert_clause(&dedup, &columns);

        assert!(sql.contains("ON CONFLICT (observation_time, ndp_id)"));
        assert!(sql.contains("DO UPDATE SET"));
        assert!(sql.contains("pm25 = EXCLUDED.pm25"));
        assert!(sql.contains("dq_flags = EXCLUDED.dq_flags"));
    }

    #[test]
    fn test_generate_skip_clause() {
        let dedup = DeduplicationConfig {
            enabled: true,
            key_columns: vec!["observation_time".to_string(), "ndp_id".to_string()],
            strategy: DeduplicationStrategy::Skip,
        };
        let columns = vec!["observation_time", "ndp_id", "pm25"];

        let gen = SqlGenerator::new();
        let sql = gen.generate_upsert_clause(&dedup, &columns);

        assert!(sql.contains("ON CONFLICT (observation_time, ndp_id) DO NOTHING"));
    }

    // ============================================================
    // Test 10: Generate complete ETL SQL
    // ============================================================
    #[test]
    fn test_generate_complete_etl_sql() {
        let config = create_test_config();

        let gen = SqlGenerator::new();
        let sql = gen
            .generate_etl_sql(&config, "air-quality", "/data/raw")
            .expect("Should generate ETL SQL");

        // Verify structure
        assert!(sql.contains("INSERT INTO pg.silver.air_quality"));
        assert!(sql.contains("SELECT"));
        assert!(sql.contains("FROM read_parquet"));
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("ON CONFLICT"));
    }

    #[test]
    fn test_generate_etl_sql_no_dedup() {
        let mut config = create_test_config();
        config.deduplication.enabled = false;
        config.incremental.enabled = false;

        let gen = SqlGenerator::new();
        let sql = gen
            .generate_etl_sql(&config, "air-quality", "/data/raw")
            .expect("Should generate ETL SQL");

        assert!(sql.contains("INSERT INTO pg.silver.air_quality"));
        assert!(sql.contains("FROM read_parquet"));
        assert!(!sql.contains("ON CONFLICT"));
        assert!(!sql.contains("WHERE"));
    }

    // ============================================================
    // Test 11: Unit conversion with scale only
    // ============================================================
    #[test]
    fn test_unit_conversion_scale_only() {
        let mapping = SilverFieldMapping {
            source_path: "raw_payload.wind.speed".to_string(),
            target_column: "wind_speed_kmh".to_string(),
            column_type: "double_precision".to_string(),
            nullable: true,
            transform: Some(TransformConfig::UnitConversion {
                from: "m_s".to_string(),
                to: "km_h".to_string(),
                formula: ConversionFormula::Linear {
                    scale: 3.6,
                    offset: 0.0,
                },
            }),
            dq_rules: vec![],
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_select_expr(&mapping);

        assert!(sql.contains("* 3.6"));
        assert!(!sql.contains("+ 0"));
    }

    // ============================================================
    // Test 12: Expression transform
    // ============================================================
    #[test]
    fn test_expression_transform() {
        let mapping = SilverFieldMapping {
            source_path: "raw_payload.temp_f".to_string(),
            target_column: "temperature_c".to_string(),
            column_type: "double_precision".to_string(),
            nullable: true,
            transform: Some(TransformConfig::Expression {
                expression: "({value} - 32) * 5 / 9".to_string(),
            }),
            dq_rules: vec![],
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_select_expr(&mapping);

        assert!(sql.contains("json_extract(raw_payload, '$.temp_f')"));
        assert!(sql.contains("- 32"));
        assert!(sql.contains("* 5 / 9"));
    }

    // ============================================================
    // Test 13: Lookup transform
    // ============================================================
    #[test]
    fn test_lookup_transform() {
        let mut table = std::collections::HashMap::new();
        table.insert("1".to_string(), "Good".to_string());
        table.insert("2".to_string(), "Fair".to_string());

        let mapping = SilverFieldMapping {
            source_path: "raw_payload.aqi_level".to_string(),
            target_column: "aqi_category".to_string(),
            column_type: "text".to_string(),
            nullable: true,
            transform: Some(TransformConfig::Lookup { table }),
            dq_rules: vec![],
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_select_expr(&mapping);

        assert!(sql.contains("CASE"));
        assert!(sql.contains("WHEN"));
        assert!(sql.contains("ELSE NULL END"));
    }

    // ============================================================
    // Test 14: Computed transform
    // ============================================================
    #[test]
    fn test_computed_transform() {
        let mapping = SilverFieldMapping {
            source_path: "".to_string(),
            target_column: "forecast_horizon_hours".to_string(),
            column_type: "double_precision".to_string(),
            nullable: true,
            transform: Some(TransformConfig::Computed {
                depends_on: vec!["issue_time".to_string(), "valid_time".to_string()],
                expression: "EXTRACT(EPOCH FROM valid_time - issue_time) / 3600".to_string(),
            }),
            dq_rules: vec![],
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_select_expr(&mapping);

        assert!(sql.contains("EXTRACT(EPOCH FROM valid_time - issue_time) / 3600"));
        assert!(sql.contains("AS forecast_horizon_hours"));
    }

    // ============================================================
    // Test 15: Error handling - missing target_table
    // ============================================================
    #[test]
    fn test_error_missing_target_table() {
        let config = SilverEtlConfig {
            enabled: true,
            target_table: "".to_string(),
            target_schema: None,
            timestamp: TimestampMapping {
                source_field: "timestamp".to_string(),
                target_field: "observation_time".to_string(),
                transform: TimestampTransform::MicrosecondsToTimestamp,
            },
            valid_timestamp: None,
            pre_transform: None,
            identity_fields: vec![],
            field_mappings: vec![],
            dq_rules: vec![],
            dq_output: DqOutputConfig::default(),
            deduplication: DeduplicationConfig::default(),
            incremental: IncrementalConfig::default(),
        };

        let gen = SqlGenerator::new();
        let result = gen.generate_etl_sql(&config, "test", "/data");

        assert!(result.is_err());
        match result {
            Err(SqlGenError::MissingField(field)) => {
                assert_eq!(field, "target_table");
            }
            _ => panic!("Expected MissingField error"),
        }
    }

    // ============================================================
    // Test 16: SELECT clause with DQ output enabled
    // ============================================================
    #[test]
    fn test_select_clause_with_dq_output() {
        let config = SilverEtlConfig {
            enabled: true,
            target_table: "silver.test".to_string(),
            target_schema: None,
            timestamp: TimestampMapping {
                source_field: "timestamp".to_string(),
                target_field: "observation_time".to_string(),
                transform: TimestampTransform::MicrosecondsToTimestamp,
            },
            valid_timestamp: None,
            pre_transform: None,
            identity_fields: vec![],
            field_mappings: vec![],
            dq_rules: vec![],
            dq_output: DqOutputConfig {
                enabled: true,
                target_column: "dq_flags".to_string(),
                include_rules: true,
                include_values: false,
            },
            deduplication: DeduplicationConfig::default(),
            incremental: IncrementalConfig::default(),
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_select_clause(&config);

        assert!(sql.contains("AS dq_flags"));
    }

    // ============================================================
    // Test 17: Identity field with raw_payload prefix
    // ============================================================
    #[test]
    fn test_identity_field_raw_payload_path() {
        let identity = IdentityField {
            source: "raw_payload.serialno".to_string(),
            target: "device_serial".to_string(),
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_identity_expr(&identity);

        assert_eq!(
            sql,
            "json_extract_string(raw_payload, '$.serialno') AS device_serial"
        );
    }

    // ============================================================
    // Test 18: Timestamp with unix seconds
    // ============================================================
    #[test]
    fn test_generate_timestamp_unix_seconds() {
        let ts_mapping = TimestampMapping {
            source_field: "timestamp".to_string(),
            target_field: "observation_time".to_string(),
            transform: TimestampTransform::UnixSeconds,
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_timestamp_expr(&ts_mapping);

        assert_eq!(sql, "to_timestamp(timestamp) AS observation_time");
    }

    // ============================================================
    // Helper: Create test config
    // ============================================================
    fn create_test_config() -> SilverEtlConfig {
        SilverEtlConfig {
            enabled: true,
            target_table: "silver.air_quality".to_string(),
            target_schema: None,
            timestamp: TimestampMapping {
                source_field: "timestamp".to_string(),
                target_field: "observation_time".to_string(),
                transform: TimestampTransform::MicrosecondsToTimestamp,
            },
            valid_timestamp: None,
            pre_transform: None,
            identity_fields: vec![IdentityField {
                source: "ndp_id".to_string(),
                target: "ndp_id".to_string(),
            }],
            field_mappings: vec![SilverFieldMapping {
                source_path: "raw_payload.pm02".to_string(),
                target_column: "pm25".to_string(),
                column_type: "double_precision".to_string(),
                nullable: true,
                transform: None,
                dq_rules: vec![],
            }],
            dq_rules: vec![],
            dq_output: DqOutputConfig {
                enabled: true,
                ..Default::default()
            },
            deduplication: DeduplicationConfig {
                enabled: true,
                key_columns: vec!["observation_time".to_string(), "ndp_id".to_string()],
                strategy: DeduplicationStrategy::Upsert,
            },
            incremental: IncrementalConfig {
                enabled: true,
                watermark_column: "observation_time".to_string(),
                lag_interval: "5 minutes".to_string(),
            },
        }
    }

    // ============================================================
    // PIVOT SQL Generation Tests (DP-007)
    // ============================================================

    /// Create test config for PIVOT SQL tests (forecast data)
    fn create_pivot_test_config() -> SilverEtlConfig {
        SilverEtlConfig {
            enabled: true,
            target_table: "silver.nws_forecasts".to_string(),
            target_schema: None,
            timestamp: TimestampMapping {
                source_field: "timestamp".to_string(),
                target_field: "issue_time".to_string(),
                transform: TimestampTransform::MicrosecondsToTimestamp,
            },
            valid_timestamp: Some(ValidTimestampMapping {
                target_field: "valid_time".to_string(),
                transform: TimestampTransform::NwsDuration,
                source: ValidTimestampSource::ArrayExplosion,
            }),
            pre_transform: None,
            identity_fields: vec![IdentityField {
                source: "ndp_id".to_string(),
                target: "ndp_id".to_string(),
            }],
            field_mappings: vec![
                SilverFieldMapping {
                    source_path: "raw_payload.properties.temperature".to_string(),
                    target_column: "temperature_c".to_string(),
                    column_type: "double_precision".to_string(),
                    nullable: true,
                    transform: None,
                    dq_rules: vec![],
                },
                SilverFieldMapping {
                    source_path: "raw_payload.properties.windSpeed".to_string(),
                    target_column: "wind_speed_kmh".to_string(),
                    column_type: "double_precision".to_string(),
                    nullable: true,
                    transform: None,
                    dq_rules: vec![],
                },
            ],
            dq_rules: vec![],
            dq_output: DqOutputConfig::default(),
            deduplication: DeduplicationConfig {
                enabled: true,
                key_columns: vec![
                    "issue_time".to_string(),
                    "valid_time".to_string(),
                    "ndp_id".to_string(),
                ],
                strategy: DeduplicationStrategy::Upsert,
            },
            incremental: IncrementalConfig {
                enabled: true,
                watermark_column: "issue_time".to_string(),
                lag_interval: "1 hour".to_string(),
            },
        }
    }

    // ============================================================
    // Test 19: Generate basic PIVOT SQL from pre_transformed
    // ============================================================
    #[test]
    fn test_generate_pivot_sql_basic() {
        let config = create_pivot_test_config();
        let mappings = vec![
            MetricMapping::new("temperature", "temperature_c"),
            MetricMapping::new("wind_speed", "wind_speed_kmh"),
        ];

        let gen = SqlGenerator::new();
        let sql = gen.generate_pivot_sql(&config, &mappings).unwrap();

        // Verify FROM pre_transformed (not read_parquet)
        assert!(
            sql.contains("FROM pre_transformed"),
            "Should read from pre_transformed table, got: {}",
            sql
        );

        // Verify GROUP BY clause
        assert!(
            sql.contains("GROUP BY issue_time, valid_time, ndp_id"),
            "Should GROUP BY key columns, got: {}",
            sql
        );

        // Verify PIVOT pattern for temperature
        assert!(
            sql.contains(
                "MAX(CASE WHEN metric_name = 'temperature' THEN value END) AS temperature_c"
            ),
            "Should contain PIVOT expression for temperature, got: {}",
            sql
        );

        // Verify PIVOT pattern for wind_speed
        assert!(
            sql.contains(
                "MAX(CASE WHEN metric_name = 'wind_speed' THEN value END) AS wind_speed_kmh"
            ),
            "Should contain PIVOT expression for wind_speed, got: {}",
            sql
        );
    }

    // ============================================================
    // Test 20: Extract metric mappings from field_mappings
    // ============================================================
    #[test]
    fn test_extract_metric_mappings() {
        let config = create_pivot_test_config();

        let mappings = SqlGenerator::extract_metric_mappings(&config);

        assert_eq!(mappings.len(), 2);

        // First mapping: raw_payload.properties.temperature -> temperature_c
        assert_eq!(mappings[0].metric_name, "temperature");
        assert_eq!(mappings[0].target_column, "temperature_c");

        // Second mapping: raw_payload.properties.windSpeed -> wind_speed_kmh
        assert_eq!(mappings[1].metric_name, "windSpeed");
        assert_eq!(mappings[1].target_column, "wind_speed_kmh");
    }

    // ============================================================
    // Test 21: PIVOT SQL includes ON CONFLICT clause for upsert
    // ============================================================
    #[test]
    fn test_pivot_sql_with_upsert() {
        let config = create_pivot_test_config();
        let mappings = vec![MetricMapping::new("temperature", "temperature_c")];

        let gen = SqlGenerator::new();
        let sql = gen.generate_pivot_sql(&config, &mappings).unwrap();

        // Verify ON CONFLICT clause is present
        assert!(
            sql.contains("ON CONFLICT (issue_time, valid_time, ndp_id)"),
            "Should include ON CONFLICT with key columns, got: {}",
            sql
        );

        // Verify DO UPDATE SET clause
        assert!(
            sql.contains("DO UPDATE SET"),
            "Should include DO UPDATE SET, got: {}",
            sql
        );

        // Verify temperature column is in the update
        assert!(
            sql.contains("temperature_c = EXCLUDED.temperature_c"),
            "Should update temperature_c on conflict, got: {}",
            sql
        );
    }

    // ============================================================
    // Test 22: PIVOT SQL includes ingestion_time
    // ============================================================
    #[test]
    fn test_pivot_sql_includes_ingestion_time() {
        let config = create_pivot_test_config();
        let mappings = vec![MetricMapping::new("temperature", "temperature_c")];

        let gen = SqlGenerator::new();
        let sql = gen.generate_pivot_sql(&config, &mappings).unwrap();

        // Verify ingestion_time is in INSERT columns
        assert!(
            sql.contains("INSERT INTO pg.silver.nws_forecasts (ingestion_time,"),
            "Should include ingestion_time in INSERT columns, got: {}",
            sql
        );

        // Verify current_timestamp for ingestion_time
        assert!(
            sql.contains("current_timestamp AS ingestion_time"),
            "Should use current_timestamp for ingestion_time, got: {}",
            sql
        );
    }

    // ============================================================
    // Test 23: PIVOT SQL handles missing target_table
    // ============================================================
    #[test]
    fn test_pivot_sql_error_missing_target_table() {
        let mut config = create_pivot_test_config();
        config.target_table = "".to_string();
        let mappings = vec![MetricMapping::new("temperature", "temperature_c")];

        let gen = SqlGenerator::new();
        let result = gen.generate_pivot_sql(&config, &mappings);

        assert!(result.is_err());
        match result {
            Err(SqlGenError::MissingField(field)) => {
                assert_eq!(field, "target_table");
            }
            _ => panic!("Expected MissingField error"),
        }
    }

    // ============================================================
    // Test 24: PIVOT SQL with DQ output enabled
    // ============================================================
    #[test]
    fn test_pivot_sql_with_dq_output() {
        let mut config = create_pivot_test_config();
        config.dq_output = DqOutputConfig {
            enabled: true,
            target_column: "dq_flags".to_string(),
            include_rules: true,
            include_values: false,
        };
        let mappings = vec![MetricMapping::new("temperature", "temperature_c")];

        let gen = SqlGenerator::new();
        let sql = gen.generate_pivot_sql(&config, &mappings).unwrap();

        // Verify dq_flags column is in INSERT
        assert!(
            sql.contains(", dq_flags)"),
            "Should include dq_flags column, got: {}",
            sql
        );

        // Verify NULL array placeholder (NULL used for COPY compatibility)
        assert!(
            sql.contains("NULL::TEXT[] AS dq_flags"),
            "Should include NULL DQ flags array, got: {}",
            sql
        );
    }

    // ============================================================
    // Test 25: PIVOT SQL without valid_timestamp (observations)
    // ============================================================
    #[test]
    fn test_pivot_sql_without_valid_timestamp() {
        let mut config = create_pivot_test_config();
        config.valid_timestamp = None;
        config.target_table = "silver.observations".to_string();
        config.timestamp.target_field = "observation_time".to_string();
        config.deduplication.key_columns =
            vec!["observation_time".to_string(), "ndp_id".to_string()];

        let mappings = vec![MetricMapping::new("temperature", "temperature_c")];

        let gen = SqlGenerator::new();
        let sql = gen.generate_pivot_sql(&config, &mappings).unwrap();

        // Should NOT have valid_time in columns
        assert!(
            !sql.contains("valid_time"),
            "Should NOT include valid_time without valid_timestamp config, got: {}",
            sql
        );

        // Should GROUP BY observation_time (the timestamp.target_field maps to issue_time in pre_transformed)
        assert!(
            sql.contains("GROUP BY issue_time, ndp_id"),
            "Should GROUP BY issue_time and ndp_id only, got: {}",
            sql
        );
    }

    // ============================================================
    // Test 26: PIVOT SQL with multiple identity fields
    // ============================================================
    #[test]
    fn test_pivot_sql_multiple_identity_fields() {
        let mut config = create_pivot_test_config();
        config.identity_fields = vec![
            IdentityField {
                source: "ndp_id".to_string(),
                target: "ndp_id".to_string(),
            },
            IdentityField {
                source: "context.location.id".to_string(),
                target: "location_id".to_string(),
            },
        ];
        config.deduplication.key_columns = vec![
            "issue_time".to_string(),
            "valid_time".to_string(),
            "ndp_id".to_string(),
            "location_id".to_string(),
        ];

        let mappings = vec![MetricMapping::new("temperature", "temperature_c")];

        let gen = SqlGenerator::new();
        let sql = gen.generate_pivot_sql(&config, &mappings).unwrap();

        // Verify both identity fields in INSERT columns
        assert!(
            sql.contains("valid_time, ndp_id, location_id"),
            "Should include both identity fields, got: {}",
            sql
        );

        // Verify GROUP BY includes location_id
        assert!(
            sql.contains("GROUP BY issue_time, valid_time, ndp_id, location_id"),
            "Should GROUP BY all identity fields, got: {}",
            sql
        );
    }

    // ============================================================
    // Test 27: PIVOT SQL generates correct pg. prefix
    // ============================================================
    #[test]
    fn test_pivot_sql_pg_prefix() {
        let config = create_pivot_test_config();
        let mappings = vec![MetricMapping::new("temperature", "temperature_c")];

        let gen = SqlGenerator::new();
        let sql = gen.generate_pivot_sql(&config, &mappings).unwrap();

        // Should have pg. prefix
        assert!(
            sql.contains("INSERT INTO pg.silver.nws_forecasts"),
            "Should have pg. prefix on target table, got: {}",
            sql
        );
    }

    // ============================================================
    // Test 28: PIVOT SQL with deduplication disabled
    // ============================================================
    #[test]
    fn test_pivot_sql_dedup_disabled() {
        let mut config = create_pivot_test_config();
        config.deduplication.enabled = false;

        let mappings = vec![MetricMapping::new("temperature", "temperature_c")];

        let gen = SqlGenerator::new();
        let sql = gen.generate_pivot_sql(&config, &mappings).unwrap();

        // Should NOT have ON CONFLICT
        assert!(
            !sql.contains("ON CONFLICT"),
            "Should NOT include ON CONFLICT when dedup disabled, got: {}",
            sql
        );
    }

    // ============================================================
    // Test 29: MetricMapping creation
    // ============================================================
    #[test]
    fn test_metric_mapping_new() {
        let mapping = MetricMapping::new("temperature", "temp_c");

        assert_eq!(mapping.metric_name, "temperature");
        assert_eq!(mapping.target_column, "temp_c");
    }

    // ============================================================
    // Test 30: Extract metric mappings with single field
    // ============================================================
    #[test]
    fn test_extract_metric_mappings_single_field() {
        let config = SilverEtlConfig {
            enabled: true,
            target_table: "silver.test".to_string(),
            target_schema: None,
            timestamp: TimestampMapping {
                source_field: "timestamp".to_string(),
                target_field: "observation_time".to_string(),
                transform: TimestampTransform::MicrosecondsToTimestamp,
            },
            valid_timestamp: None,
            pre_transform: None,
            identity_fields: vec![],
            field_mappings: vec![SilverFieldMapping {
                source_path: "humidity".to_string(), // No dot path
                target_column: "humidity_pct".to_string(),
                column_type: "double_precision".to_string(),
                nullable: true,
                transform: None,
                dq_rules: vec![],
            }],
            dq_rules: vec![],
            dq_output: DqOutputConfig::default(),
            deduplication: DeduplicationConfig::default(),
            incremental: IncrementalConfig::default(),
        };

        let mappings = SqlGenerator::extract_metric_mappings(&config);

        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].metric_name, "humidity");
        assert_eq!(mappings[0].target_column, "humidity_pct");
    }

    // ============================================================
    // Test 31: PIVOT SQL escapes metric names with quotes
    // ============================================================
    #[test]
    fn test_pivot_sql_escapes_special_characters() {
        let config = create_pivot_test_config();
        let mappings = vec![
            MetricMapping::new("temp's", "temperature_c"), // Single quote in name
        ];

        let gen = SqlGenerator::new();
        let sql = gen.generate_pivot_sql(&config, &mappings).unwrap();

        // Should escape single quote
        assert!(
            sql.contains("metric_name = 'temp''s'"),
            "Should escape single quotes in metric names, got: {}",
            sql
        );
    }

    // ============================================================
    // Test: Execute pivot SQL in DuckDB and verify column count
    // ============================================================
    #[test]
    fn test_pivot_sql_execution_column_count() {
        use duckdb::Connection;

        let conn = Connection::open_in_memory().unwrap();

        // Create pre_transformed table (same schema as ETL creates)
        conn.execute_batch(
            r#"
            CREATE TABLE pre_transformed (
                issue_time TIMESTAMPTZ,
                valid_time TIMESTAMPTZ,
                ndp_id VARCHAR,
                location_id VARCHAR,
                metric_name VARCHAR,
                value DOUBLE
            );

            INSERT INTO pre_transformed VALUES
                ('2026-01-15 12:00:00+00', '2026-01-15 13:00:00+00', 'weather-nws-002', 'loc1', 'temperature', 15.5),
                ('2026-01-15 12:00:00+00', '2026-01-15 13:00:00+00', 'weather-nws-002', 'loc1', 'dewpoint', 10.2),
                ('2026-01-15 12:00:00+00', '2026-01-15 13:00:00+00', 'weather-nws-002', 'loc1', 'wind_speed', 25.0);
            "#,
        )
        .unwrap();

        // Run the pivot query (simplified version with fewer metrics)
        let mut stmt = conn
            .prepare(
                r#"
                SELECT
                  current_timestamp AS ingestion_time,
                  issue_time,
                  ndp_id,
                  valid_time,
                  MAX(CASE WHEN metric_name = 'temperature' THEN value END) AS temperature_c,
                  MAX(CASE WHEN metric_name = 'dewpoint' THEN value END) AS dewpoint_c,
                  MAX(CASE WHEN metric_name = 'wind_speed' THEN value END) AS wind_speed_kmh,
                  NULL::TEXT[] AS dq_flags
                FROM pre_transformed
                GROUP BY issue_time, valid_time, ndp_id
                "#,
            )
            .unwrap();

        let column_count = stmt.column_count();
        println!("Pivot query produces {} columns", column_count);

        // Get column names
        let column_names: Vec<String> = (0..column_count)
            .map(|i| stmt.column_name(i).unwrap().to_string())
            .collect();
        println!("Column names: {:?}", column_names);

        assert_eq!(column_count, 8, "Pivot query should produce 8 columns");

        // Execute and check we get results
        let rows: Vec<_> = stmt
            .query_map([], |row| {
                let ndp_id: String = row.get(2)?;
                Ok(ndp_id)
            })
            .unwrap()
            .collect();

        assert_eq!(rows.len(), 1, "Should have 1 pivoted row");
        println!("Test passed: pivot produces correct column count");
    }

    // ============================================================
    // Test: Export pivot result to CSV and count tab-separated fields
    // ============================================================
    #[test]
    fn test_pivot_sql_csv_export_field_count() {
        use duckdb::Connection;
        use std::fs;

        let conn = Connection::open_in_memory().unwrap();

        conn.execute_batch(
            r#"
            CREATE TABLE pre_transformed (
                issue_time TIMESTAMPTZ,
                valid_time TIMESTAMPTZ,
                ndp_id VARCHAR,
                location_id VARCHAR,
                metric_name VARCHAR,
                value DOUBLE
            );

            INSERT INTO pre_transformed VALUES
                ('2026-01-15 12:00:00+00', '2026-01-15 13:00:00+00', 'weather-nws-002', 'loc1', 'temperature', 15.5),
                ('2026-01-15 12:00:00+00', '2026-01-15 13:00:00+00', 'weather-nws-002', 'loc1', 'dewpoint', 10.2);
            "#,
        )
        .unwrap();

        // Export pivot query to CSV with tab delimiter (like COPY uses)
        conn.execute_batch(
            r#"
            COPY (
                SELECT
                  current_timestamp AS ingestion_time,
                  issue_time,
                  ndp_id,
                  valid_time,
                  MAX(CASE WHEN metric_name = 'temperature' THEN value END) AS temperature_c,
                  MAX(CASE WHEN metric_name = 'dewpoint' THEN value END) AS dewpoint_c,
                  NULL::TEXT[] AS dq_flags
                FROM pre_transformed
                GROUP BY issue_time, valid_time, ndp_id
            ) TO '/tmp/pivot_test_output.csv' (DELIMITER E'\t');
            "#,
        )
        .unwrap();

        // Read the output and count fields
        let content = fs::read_to_string("/tmp/pivot_test_output.csv").unwrap();
        println!("CSV output:\n{}", content);

        let lines: Vec<&str> = content.trim().lines().collect();
        assert!(!lines.is_empty(), "Should have output");

        let field_count = lines[0].split('\t').count();
        println!("Tab-separated field count: {}", field_count);

        // Print each field for debugging
        for (i, field) in lines[0].split('\t').enumerate() {
            println!("  Field {}: '{}'", i, field);
        }

        assert_eq!(field_count, 7, "CSV should have 7 tab-separated fields");
    }

    // ============================================================
    // Test: Full 18-column pivot SQL matching real ETL
    // ============================================================
    #[test]
    fn test_pivot_sql_full_18_columns_csv_export() {
        use duckdb::Connection;
        use std::fs;

        let conn = Connection::open_in_memory().unwrap();

        conn.execute_batch(
            r#"
            CREATE TABLE pre_transformed (
                issue_time TIMESTAMPTZ,
                valid_time TIMESTAMPTZ,
                ndp_id VARCHAR,
                location_id VARCHAR,
                metric_name VARCHAR,
                value DOUBLE
            );

            -- Insert metrics matching actual config
            INSERT INTO pre_transformed VALUES
                ('2026-01-15 12:00:00+00', '2026-01-15 13:00:00+00', 'weather-nws-002', 'loc1', 'temperature', 15.5),
                ('2026-01-15 12:00:00+00', '2026-01-15 13:00:00+00', 'weather-nws-002', 'loc1', 'dewpoint', 10.2),
                ('2026-01-15 12:00:00+00', '2026-01-15 13:00:00+00', 'weather-nws-002', 'loc1', 'apparent_temperature', 14.0),
                ('2026-01-15 12:00:00+00', '2026-01-15 13:00:00+00', 'weather-nws-002', 'loc1', 'heat_index', 16.0),
                ('2026-01-15 12:00:00+00', '2026-01-15 13:00:00+00', 'weather-nws-002', 'loc1', 'wind_chill', 13.0),
                ('2026-01-15 12:00:00+00', '2026-01-15 13:00:00+00', 'weather-nws-002', 'loc1', 'wind_speed', 25.0),
                ('2026-01-15 12:00:00+00', '2026-01-15 13:00:00+00', 'weather-nws-002', 'loc1', 'wind_direction', 180.0),
                ('2026-01-15 12:00:00+00', '2026-01-15 13:00:00+00', 'weather-nws-002', 'loc1', 'wind_gust', 35.0),
                ('2026-01-15 12:00:00+00', '2026-01-15 13:00:00+00', 'weather-nws-002', 'loc1', 'probability_of_precipitation', 20.0),
                ('2026-01-15 12:00:00+00', '2026-01-15 13:00:00+00', 'weather-nws-002', 'loc1', 'quantitative_precipitation', 0.5),
                ('2026-01-15 12:00:00+00', '2026-01-15 13:00:00+00', 'weather-nws-002', 'loc1', 'relative_humidity', 65.0),
                ('2026-01-15 12:00:00+00', '2026-01-15 13:00:00+00', 'weather-nws-002', 'loc1', 'sky_cover', 50.0),
                ('2026-01-15 12:00:00+00', '2026-01-15 13:00:00+00', 'weather-nws-002', 'loc1', 'visibility', 10000.0);
            "#,
        )
        .unwrap();

        // Export EXACT pivot query from dry-run (18 columns)
        conn.execute_batch(
            r#"
            COPY (
                SELECT
                  current_timestamp AS ingestion_time,
                  issue_time AS issue_time,
                  ndp_id,
                  valid_time AS valid_time,
                  MAX(CASE WHEN metric_name = 'temperature' THEN value END) AS temperature_c,
                  MAX(CASE WHEN metric_name = 'dewpoint' THEN value END) AS dewpoint_c,
                  MAX(CASE WHEN metric_name = 'apparent_temperature' THEN value END) AS apparent_temp_c,
                  MAX(CASE WHEN metric_name = 'heat_index' THEN value END) AS heat_index_c,
                  MAX(CASE WHEN metric_name = 'wind_chill' THEN value END) AS wind_chill_c,
                  MAX(CASE WHEN metric_name = 'wind_speed' THEN value END) AS wind_speed_kmh,
                  MAX(CASE WHEN metric_name = 'wind_direction' THEN value END) AS wind_direction_deg,
                  MAX(CASE WHEN metric_name = 'wind_gust' THEN value END) AS wind_gust_kmh,
                  MAX(CASE WHEN metric_name = 'probability_of_precipitation' THEN value END) AS precip_probability_pct,
                  MAX(CASE WHEN metric_name = 'quantitative_precipitation' THEN value END) AS precip_amount_mm,
                  MAX(CASE WHEN metric_name = 'relative_humidity' THEN value END) AS humidity_pct,
                  MAX(CASE WHEN metric_name = 'sky_cover' THEN value END) AS sky_cover_pct,
                  MAX(CASE WHEN metric_name = 'visibility' THEN value END) AS visibility_m,
                  NULL::TEXT[] AS dq_flags
                FROM pre_transformed
                GROUP BY issue_time, valid_time, ndp_id
            ) TO '/tmp/pivot_full_18_output.csv' (DELIMITER E'\t');
            "#,
        )
        .unwrap();

        // Read the output and count fields - DON'T use trim() as it removes trailing tabs
        let content = fs::read_to_string("/tmp/pivot_full_18_output.csv").unwrap();
        println!("Full 18-column CSV output (escaped):\n{}", content.escape_debug());

        // Split by newline manually to preserve trailing tabs
        let lines: Vec<&str> = content.split('\n').collect();
        assert!(lines.len() >= 2, "Should have header and data lines");

        // Data line (second line, index 1)
        let data_line = lines[1];
        println!("Data line raw: '{}'", data_line.escape_debug());

        let field_count = data_line.split('\t').count();
        println!("Data line tab-separated field count: {}", field_count);

        // Print each field
        for (i, field) in data_line.split('\t').enumerate() {
            println!("  Field {}: '{}'", i, field.escape_debug());
        }

        assert_eq!(field_count, 18, "CSV should have exactly 18 tab-separated fields");
    }
}
