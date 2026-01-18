//! Schema Generator - Derive DDL from stream configuration
//!
//! Generates TimescaleDB schema (CREATE TABLE, hypertables) directly from
//! the silver_etl configuration, making the system truly config-driven.
//!
//! ## Usage
//!
//! ```ignore
//! use silver_etl::SchemaGenerator;
//!
//! let generator = SchemaGenerator::new();
//! let ddl = generator.generate_create_table(&config)?;
//! let hypertable = generator.generate_hypertable(&config)?;
//! ```

use neural_core::config::{DeduplicationStrategy, SilverEtlConfig};
use thiserror::Error;

/// Schema generation errors
#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("Invalid target table: {0}")]
    InvalidTargetTable(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid column type '{column_type}' for field '{field}'")]
    InvalidColumnType { field: String, column_type: String },

    #[error("Database error: {0}")]
    DatabaseError(String),
}

/// Schema generator for TimescaleDB
pub struct SchemaGenerator {
    /// Whether to use IF NOT EXISTS clauses
    pub if_not_exists: bool,
}

impl Default for SchemaGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaGenerator {
    /// Create a new schema generator
    pub fn new() -> Self {
        Self {
            if_not_exists: true,
        }
    }

    /// Generate CREATE SCHEMA statement
    pub fn generate_create_schema(&self, config: &SilverEtlConfig) -> Result<String, SchemaError> {
        let schema_name = self.extract_schema_name(&config.target_table)?;

        let if_not_exists = if self.if_not_exists {
            "IF NOT EXISTS "
        } else {
            ""
        };

        Ok(format!("CREATE SCHEMA {if_not_exists}{schema_name};"))
    }

    /// Generate CREATE TABLE statement from config
    ///
    /// Derives column definitions from field_mappings, identity_fields,
    /// and timestamp configuration.
    pub fn generate_create_table(&self, config: &SilverEtlConfig) -> Result<String, SchemaError> {
        let table_name = &config.target_table;
        let if_not_exists = if self.if_not_exists {
            "IF NOT EXISTS "
        } else {
            ""
        };

        let mut columns = Vec::new();

        // 1. Add ingestion_time (always first)
        columns.push("    ingestion_time TIMESTAMPTZ NOT NULL DEFAULT NOW()".to_string());

        // 2. Add timestamp field from config
        let ts_type = self.timestamp_type(&config.timestamp.transform);
        let ts_field = &config.timestamp.target_field;
        columns.push(format!("    {ts_field} {ts_type} NOT NULL"));

        // 3. Add identity fields
        for id_field in &config.identity_fields {
            columns.push(format!("    {} TEXT NOT NULL", id_field.target));
        }

        // 3b. Add valid_timestamp if configured (used for forecasts with valid_time)
        if let Some(ref valid_ts) = config.valid_timestamp {
            let valid_ts_type = self.timestamp_type(&valid_ts.transform);
            columns.push(format!(
                "    {} {} NOT NULL",
                valid_ts.target_field, valid_ts_type
            ));
        }

        // 4. Add mapped fields from field_mappings
        for mapping in &config.field_mappings {
            let pg_type = self.to_postgres_type(&mapping.column_type)?;
            let nullable = if mapping.nullable { "" } else { " NOT NULL" };
            columns.push(format!(
                "    {} {}{}",
                mapping.target_column, pg_type, nullable
            ));
        }

        // 5. Add dq_flags column if DQ output enabled
        if config.dq_output.enabled {
            columns.push(format!("    {} TEXT[]", config.dq_output.target_column));
        }

        // Build primary key from deduplication config or defaults
        let pk_columns =
            if config.deduplication.enabled && !config.deduplication.key_columns.is_empty() {
                config.deduplication.key_columns.join(", ")
            } else {
                // Default: timestamp + identity fields
                let mut pk = vec![config.timestamp.target_field.clone()];
                for id_field in &config.identity_fields {
                    pk.push(id_field.target.clone());
                }
                pk.join(", ")
            };

        let sql = format!(
            r#"CREATE TABLE {if_not_exists}{table_name} (
{columns},
    PRIMARY KEY ({pk_columns})
);"#,
            columns = columns.join(",\n")
        );

        Ok(sql)
    }

    /// Generate SELECT create_hypertable statement
    pub fn generate_hypertable(&self, config: &SilverEtlConfig) -> Result<String, SchemaError> {
        let table_name = &config.target_table;
        let time_column = &config.timestamp.target_field;

        // Use 1 day chunk interval for most observation data
        let chunk_interval = "INTERVAL '1 day'";

        let sql = format!(
            r#"SELECT create_hypertable(
    '{table_name}',
    '{time_column}',
    chunk_time_interval => {chunk_interval},
    if_not_exists => TRUE
);"#
        );

        Ok(sql)
    }

    /// Generate ALTER TABLE statements to add missing columns (schema evolution)
    pub fn generate_add_columns(
        &self,
        config: &SilverEtlConfig,
        existing_columns: &[String],
    ) -> Result<Vec<String>, SchemaError> {
        let table_name = &config.target_table;
        let mut statements = Vec::new();

        // Check each field mapping
        for mapping in &config.field_mappings {
            if !existing_columns.contains(&mapping.target_column) {
                let pg_type = self.to_postgres_type(&mapping.column_type)?;
                statements.push(format!(
                    "ALTER TABLE {table_name} ADD COLUMN IF NOT EXISTS {} {};",
                    mapping.target_column, pg_type
                ));
            }
        }

        // Check dq_flags column
        if config.dq_output.enabled && !existing_columns.contains(&config.dq_output.target_column) {
            statements.push(format!(
                "ALTER TABLE {table_name} ADD COLUMN IF NOT EXISTS {} TEXT[];",
                config.dq_output.target_column
            ));
        }

        Ok(statements)
    }

    /// Generate complete migration SQL for a stream
    ///
    /// Includes: schema, table, hypertable, indexes
    pub fn generate_full_migration(&self, config: &SilverEtlConfig) -> Result<String, SchemaError> {
        let mut statements = Vec::new();

        // 1. Create schema
        statements.push(self.generate_create_schema(config)?);
        statements.push(String::new()); // blank line

        // 2. Create table
        statements.push(self.generate_create_table(config)?);
        statements.push(String::new());

        // 3. Create hypertable
        statements.push(self.generate_hypertable(config)?);
        statements.push(String::new());

        // 4. Create standard indexes
        statements.extend(self.generate_indexes(config)?);

        Ok(statements.join("\n"))
    }

    /// Generate standard indexes for a Silver table
    pub fn generate_indexes(&self, config: &SilverEtlConfig) -> Result<Vec<String>, SchemaError> {
        let table_name = &config.target_table;
        let table_short = self.extract_table_name(table_name)?;
        let time_column = &config.timestamp.target_field;

        let mut indexes = Vec::new();

        // Index on identity fields (for lookups)
        for id_field in &config.identity_fields {
            indexes.push(format!(
                "CREATE INDEX IF NOT EXISTS idx_{table_short}_{} ON {table_name} ({});",
                id_field.target, id_field.target
            ));
        }

        // Composite index on time + identity (for time-series queries)
        if !config.identity_fields.is_empty() {
            let id_cols: Vec<_> = config
                .identity_fields
                .iter()
                .map(|f| f.target.as_str())
                .collect();
            indexes.push(format!(
                "CREATE INDEX IF NOT EXISTS idx_{table_short}_time_id ON {table_name} ({}, {});",
                time_column,
                id_cols.join(", ")
            ));
        }

        // Index on ingestion_time (for incremental processing)
        indexes.push(format!(
            "CREATE INDEX IF NOT EXISTS idx_{table_short}_ingestion ON {table_name} (ingestion_time);"
        ));

        Ok(indexes)
    }

    /// Convert config column type to PostgreSQL type
    fn to_postgres_type(&self, column_type: &str) -> Result<String, SchemaError> {
        match column_type.to_lowercase().as_str() {
            "double_precision" | "double" | "float8" => Ok("DOUBLE PRECISION".to_string()),
            "real" | "float4" | "float" => Ok("REAL".to_string()),
            "integer" | "int" | "int4" => Ok("INTEGER".to_string()),
            "bigint" | "int8" => Ok("BIGINT".to_string()),
            "smallint" | "int2" => Ok("SMALLINT".to_string()),
            "text" | "string" | "varchar" => Ok("TEXT".to_string()),
            "boolean" | "bool" => Ok("BOOLEAN".to_string()),
            "timestamptz" | "timestamp" => Ok("TIMESTAMPTZ".to_string()),
            "date" => Ok("DATE".to_string()),
            "jsonb" | "json" => Ok("JSONB".to_string()),
            other => Err(SchemaError::InvalidColumnType {
                field: "unknown".to_string(),
                column_type: other.to_string(),
            }),
        }
    }

    /// Get PostgreSQL type for timestamp based on transform
    fn timestamp_type(&self, transform: &neural_core::config::TimestampTransform) -> &'static str {
        // All transforms output TIMESTAMPTZ
        match transform {
            _ => "TIMESTAMPTZ",
        }
    }

    /// Extract schema name from qualified table name
    fn extract_schema_name(&self, table_name: &str) -> Result<String, SchemaError> {
        if let Some(dot_pos) = table_name.find('.') {
            Ok(table_name[..dot_pos].to_string())
        } else {
            Err(SchemaError::InvalidTargetTable(format!(
                "Table '{}' must be schema-qualified (e.g., 'silver.table_name')",
                table_name
            )))
        }
    }

    /// Extract table name without schema
    fn extract_table_name(&self, table_name: &str) -> Result<String, SchemaError> {
        if let Some(dot_pos) = table_name.find('.') {
            Ok(table_name[dot_pos + 1..].to_string())
        } else {
            Ok(table_name.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neural_core::config::{
        DeduplicationConfig, DqOutputConfig, IdentityField, IncrementalConfig, SilverFieldMapping,
        TimestampMapping, TimestampTransform,
    };

    fn test_config() -> SilverEtlConfig {
        SilverEtlConfig {
            enabled: true,
            target_table: "silver.air_quality_observations".to_string(),
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
            field_mappings: vec![
                SilverFieldMapping {
                    source_path: "raw_payload.pm02".to_string(),
                    target_column: "pm25".to_string(),
                    column_type: "double_precision".to_string(),
                    nullable: false,
                    transform: None,
                    dq_rules: vec![],
                },
                SilverFieldMapping {
                    source_path: "raw_payload.temperature".to_string(),
                    target_column: "temperature_c".to_string(),
                    column_type: "double_precision".to_string(),
                    nullable: true,
                    transform: None,
                    dq_rules: vec![],
                },
            ],
            dq_rules: vec![],
            dq_output: DqOutputConfig {
                enabled: true,
                target_column: "dq_flags".to_string(),
                include_rules: true,
                include_values: false,
            },
            deduplication: DeduplicationConfig {
                enabled: true,
                key_columns: vec!["observation_time".to_string(), "ndp_id".to_string()],
                strategy: DeduplicationStrategy::Upsert,
            },
            incremental: IncrementalConfig::default(),
        }
    }

    #[test]
    fn test_generate_create_schema() {
        let gen = SchemaGenerator::new();
        let config = test_config();
        let sql = gen.generate_create_schema(&config).unwrap();
        assert_eq!(sql, "CREATE SCHEMA IF NOT EXISTS silver;");
    }

    #[test]
    fn test_generate_create_table() {
        let gen = SchemaGenerator::new();
        let config = test_config();
        let sql = gen.generate_create_table(&config).unwrap();

        assert!(sql.contains("CREATE TABLE IF NOT EXISTS silver.air_quality_observations"));
        assert!(sql.contains("ingestion_time TIMESTAMPTZ NOT NULL DEFAULT NOW()"));
        assert!(sql.contains("observation_time TIMESTAMPTZ NOT NULL"));
        assert!(sql.contains("ndp_id TEXT NOT NULL"));
        assert!(sql.contains("pm25 DOUBLE PRECISION NOT NULL"));
        assert!(sql.contains("temperature_c DOUBLE PRECISION"));
        assert!(sql.contains("dq_flags TEXT[]"));
        assert!(sql.contains("PRIMARY KEY (observation_time, ndp_id)"));
    }

    #[test]
    fn test_generate_hypertable() {
        let gen = SchemaGenerator::new();
        let config = test_config();
        let sql = gen.generate_hypertable(&config).unwrap();

        assert!(sql.contains("create_hypertable"));
        assert!(sql.contains("silver.air_quality_observations"));
        assert!(sql.contains("observation_time"));
        assert!(sql.contains("if_not_exists => TRUE"));
    }

    #[test]
    fn test_generate_indexes() {
        let gen = SchemaGenerator::new();
        let config = test_config();
        let indexes = gen.generate_indexes(&config).unwrap();

        assert!(indexes.len() >= 2);
        assert!(indexes
            .iter()
            .any(|s| s.contains("idx_air_quality_observations_ndp_id")));
        assert!(indexes
            .iter()
            .any(|s| s.contains("idx_air_quality_observations_ingestion")));
    }

    #[test]
    fn test_generate_full_migration() {
        let gen = SchemaGenerator::new();
        let config = test_config();
        let sql = gen.generate_full_migration(&config).unwrap();

        assert!(sql.contains("CREATE SCHEMA"));
        assert!(sql.contains("CREATE TABLE"));
        assert!(sql.contains("create_hypertable"));
        assert!(sql.contains("CREATE INDEX"));
    }

    #[test]
    fn test_generate_add_columns() {
        let gen = SchemaGenerator::new();
        let config = test_config();

        // Simulate existing columns missing pm25
        let existing = vec![
            "observation_time".to_string(),
            "ndp_id".to_string(),
            "temperature_c".to_string(),
        ];
        let stmts = gen.generate_add_columns(&config, &existing).unwrap();

        assert_eq!(stmts.len(), 2); // pm25 and dq_flags
        assert!(stmts.iter().any(|s| s.contains("pm25")));
        assert!(stmts.iter().any(|s| s.contains("dq_flags")));
    }

    #[test]
    fn test_to_postgres_type() {
        let gen = SchemaGenerator::new();

        assert_eq!(
            gen.to_postgres_type("double_precision").unwrap(),
            "DOUBLE PRECISION"
        );
        assert_eq!(gen.to_postgres_type("smallint").unwrap(), "SMALLINT");
        assert_eq!(gen.to_postgres_type("text").unwrap(), "TEXT");
        assert_eq!(gen.to_postgres_type("boolean").unwrap(), "BOOLEAN");
        assert!(gen.to_postgres_type("unknown_type").is_err());
    }

    #[test]
    fn test_invalid_table_name() {
        let gen = SchemaGenerator::new();
        let result = gen.extract_schema_name("no_schema_table");
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_create_table_with_valid_timestamp() {
        use neural_core::config::{ValidTimestampMapping, ValidTimestampSource};

        let gen = SchemaGenerator::new();
        let mut config = test_config();

        // Add valid_timestamp (used for forecasts with issue_time + valid_time)
        config.target_table = "silver.weather_forecasts".to_string();
        config.timestamp.target_field = "issue_time".to_string();
        config.valid_timestamp = Some(ValidTimestampMapping {
            target_field: "valid_time".to_string(),
            transform: TimestampTransform::NwsDuration,
            source: ValidTimestampSource::ArrayExplosion,
        });
        config.deduplication.key_columns = vec![
            "issue_time".to_string(),
            "valid_time".to_string(),
            "ndp_id".to_string(),
        ];

        let sql = gen.generate_create_table(&config).unwrap();

        // Should have both timestamp columns
        assert!(
            sql.contains("issue_time TIMESTAMPTZ NOT NULL"),
            "Missing issue_time column"
        );
        assert!(
            sql.contains("valid_time TIMESTAMPTZ NOT NULL"),
            "Missing valid_time column"
        );

        // Primary key should include all three columns
        assert!(
            sql.contains("PRIMARY KEY (issue_time, valid_time, ndp_id)"),
            "Wrong PRIMARY KEY"
        );
    }
}
