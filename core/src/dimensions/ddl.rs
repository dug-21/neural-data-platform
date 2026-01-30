//! DDL Generator for Dimension Tables
//!
//! Generates PostgreSQL DDL statements from DimensionConfig.
//! This ensures all table definitions come from configuration YAML,
//! not hardcoded SQL files.
//!
//! # Architecture (DP-013)
//!
//! The config YAML is the single source of truth for dimension schemas.
//! This generator reads the config and produces:
//! - CREATE TABLE statements
//! - CREATE INDEX statements
//! - TRUNCATE statements (for truncate_and_load strategy)
//! - INSERT ON CONFLICT statements (for upsert strategy)
//!
//! # Example
//!
//! ```ignore
//! use platform_core::dimensions::DdlGenerator;
//! use platform_core::types::DimensionConfig;
//!
//! let config: DimensionConfig = load_from_yaml("entity_context.yaml");
//! let create_ddl = DdlGenerator::generate_create_table(&config);
//! let indexes = DdlGenerator::generate_indexes(&config);
//! ```

use crate::types::dimension_config::{DimensionConfig, DimensionField, FieldType, LoadStrategy};

/// DDL Generator for dimension tables
///
/// Generates PostgreSQL-compatible DDL from DimensionConfig.
/// All table structure comes from config YAML - no hardcoded SQL.
pub struct DdlGenerator;

impl DdlGenerator {
    /// Generate CREATE TABLE IF NOT EXISTS statement from config
    ///
    /// The generated DDL includes:
    /// - All fields with proper PostgreSQL types
    /// - NOT NULL constraints where applicable
    /// - PRIMARY KEY constraint if primary_key is specified
    ///
    /// # Example Output
    ///
    /// ```sql
    /// CREATE TABLE IF NOT EXISTS silver.entity_context (
    ///     ndp_id TEXT NOT NULL,
    ///     category TEXT NOT NULL,
    ///     friendly_name TEXT NOT NULL,
    ///     location_path TEXT,
    ///     PRIMARY KEY (ndp_id)
    /// );
    /// ```
    pub fn generate_create_table(config: &DimensionConfig) -> String {
        let schema = &config.target.schema;
        let table = &config.target.table;

        let columns: Vec<String> = config
            .schema
            .fields
            .iter()
            .map(|f| Self::field_to_column_def(f))
            .collect();

        let pk_constraint = if !config.schema.primary_key.is_empty() {
            format!(
                ",\n    PRIMARY KEY ({})",
                config.schema.primary_key.join(", ")
            )
        } else {
            String::new()
        };

        format!(
            "CREATE TABLE IF NOT EXISTS {schema}.{table} (\n    {columns}{pk}\n);",
            schema = schema,
            table = table,
            columns = columns.join(",\n    "),
            pk = pk_constraint
        )
    }

    /// Generate column definition from field config
    fn field_to_column_def(field: &DimensionField) -> String {
        let sql_type = Self::field_type_to_sql(&field.field_type);
        let nullable = if field.nullable { "" } else { " NOT NULL" };
        format!("{} {}{}", field.name, sql_type, nullable)
    }

    /// Map FieldType to PostgreSQL type string
    fn field_type_to_sql(ft: &FieldType) -> &'static str {
        ft.to_pg_type()
    }

    /// Generate CREATE INDEX statements from config
    ///
    /// Returns a vector of index creation statements.
    /// Supports both regular and unique indexes.
    ///
    /// # Example Output
    ///
    /// ```sql
    /// CREATE INDEX IF NOT EXISTS idx_entity_context_category
    ///     ON silver.entity_context (category);
    /// CREATE UNIQUE INDEX IF NOT EXISTS idx_entity_context_location
    ///     ON silver.entity_context (location_path);
    /// ```
    pub fn generate_indexes(config: &DimensionConfig) -> Vec<String> {
        let qualified_table = config.target.qualified_name();

        config
            .schema
            .indexes
            .iter()
            .map(|idx| {
                let unique = if idx.unique { "UNIQUE " } else { "" };
                let columns = idx.columns.join(", ");
                format!(
                    "CREATE {unique}INDEX IF NOT EXISTS {name}\n    ON {table} ({columns});",
                    unique = unique,
                    name = idx.name,
                    table = qualified_table,
                    columns = columns
                )
            })
            .collect()
    }

    /// Generate TRUNCATE statement for truncate_and_load strategy
    ///
    /// Used before bulk loading to clear existing data.
    pub fn generate_truncate(config: &DimensionConfig) -> String {
        format!("TRUNCATE TABLE {};", config.target.qualified_name())
    }

    /// Generate DELETE statement (alternative to TRUNCATE for non-superuser)
    ///
    /// Some environments don't allow TRUNCATE, so DELETE is a fallback.
    pub fn generate_delete_all(config: &DimensionConfig) -> String {
        format!("DELETE FROM {};", config.target.qualified_name())
    }

    /// Generate parameterized INSERT statement for batch loading
    ///
    /// Returns the SQL with $1, $2, etc. parameter placeholders.
    /// The number of placeholders matches the number of fields in config.
    ///
    /// # Example Output
    ///
    /// ```sql
    /// INSERT INTO silver.entity_context (ndp_id, category, friendly_name)
    /// VALUES ($1, $2, $3)
    /// ```
    pub fn generate_insert(config: &DimensionConfig) -> String {
        let qualified_table = config.target.qualified_name();
        let columns: Vec<&str> = config.schema.column_names();
        let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("${}", i)).collect();

        format!(
            "INSERT INTO {} ({})\nVALUES ({})",
            qualified_table,
            columns.join(", "),
            placeholders.join(", ")
        )
    }

    /// Generate INSERT ON CONFLICT (upsert) statement
    ///
    /// Used for upsert strategy. Updates all non-PK columns on conflict.
    ///
    /// # Example Output
    ///
    /// ```sql
    /// INSERT INTO silver.entity_context (ndp_id, category, friendly_name)
    /// VALUES ($1, $2, $3)
    /// ON CONFLICT (ndp_id) DO UPDATE SET
    ///     category = EXCLUDED.category,
    ///     friendly_name = EXCLUDED.friendly_name
    /// ```
    pub fn generate_upsert(config: &DimensionConfig) -> Result<String, String> {
        if config.schema.primary_key.is_empty() {
            return Err("Upsert requires primary_key to be defined".to_string());
        }

        let qualified_table = config.target.qualified_name();
        let columns: Vec<&str> = config.schema.column_names();
        let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("${}", i)).collect();

        // Columns to update (all except primary key)
        let pk_set: std::collections::HashSet<&str> = config
            .schema
            .primary_key
            .iter()
            .map(|s| s.as_str())
            .collect();

        let update_columns: Vec<String> = columns
            .iter()
            .filter(|col| !pk_set.contains(*col))
            .map(|col| format!("{} = EXCLUDED.{}", col, col))
            .collect();

        if update_columns.is_empty() {
            // All columns are in PK, use DO NOTHING
            return Ok(format!(
                "INSERT INTO {} ({})\nVALUES ({})\nON CONFLICT ({}) DO NOTHING",
                qualified_table,
                columns.join(", "),
                placeholders.join(", "),
                config.schema.primary_key.join(", ")
            ));
        }

        Ok(format!(
            "INSERT INTO {} ({})\nVALUES ({})\nON CONFLICT ({}) DO UPDATE SET\n    {}",
            qualified_table,
            columns.join(", "),
            placeholders.join(", "),
            config.schema.primary_key.join(", "),
            update_columns.join(",\n    ")
        ))
    }

    /// Generate the appropriate load statement based on strategy
    ///
    /// For TruncateAndLoad: returns simple INSERT
    /// For Upsert: returns INSERT ON CONFLICT
    pub fn generate_load_statement(config: &DimensionConfig) -> Result<String, String> {
        match config.load.strategy {
            LoadStrategy::TruncateAndLoad => Ok(Self::generate_insert(config)),
            LoadStrategy::Upsert => Self::generate_upsert(config),
        }
    }

    /// Generate DROP TABLE statement (for migrations/testing)
    pub fn generate_drop_table(config: &DimensionConfig) -> String {
        format!("DROP TABLE IF EXISTS {};", config.target.qualified_name())
    }

    /// Generate DROP INDEX statements (for migrations)
    pub fn generate_drop_indexes(config: &DimensionConfig) -> Vec<String> {
        config
            .schema
            .indexes
            .iter()
            .map(|idx| {
                format!(
                    "DROP INDEX IF EXISTS {}.{};",
                    config.target.schema, idx.name
                )
            })
            .collect()
    }

    /// Generate full DDL script (CREATE TABLE + all indexes)
    ///
    /// Returns a complete script that can be executed to create
    /// the table and all its indexes.
    pub fn generate_full_ddl(config: &DimensionConfig) -> String {
        let mut statements = vec![Self::generate_create_table(config)];
        statements.extend(Self::generate_indexes(config));
        statements.join("\n\n")
    }

    /// Generate schema creation statement
    ///
    /// Ensures the target schema exists before creating tables.
    pub fn generate_create_schema(config: &DimensionConfig) -> String {
        format!("CREATE SCHEMA IF NOT EXISTS {};", config.target.schema)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::dimension_config::{
        DimensionSchema, DimensionSource, DimensionSourceType, DimensionTarget, IndexConfig,
        LoadConfig,
    };
    use std::path::PathBuf;

    fn create_test_config() -> DimensionConfig {
        DimensionConfig {
            dimension_id: "entity-context".to_string(),
            target: DimensionTarget {
                table: "entity_context".to_string(),
                schema: "silver".to_string(),
            },
            source: DimensionSource {
                source_type: DimensionSourceType::Csv,
                path: PathBuf::from("data/dimensions/entity_context.csv"),
                delimiter: ',',
            },
            schema: DimensionSchema {
                fields: vec![
                    DimensionField::new("ndp_id", FieldType::Text).required(),
                    DimensionField::new("category", FieldType::Text).required(),
                    DimensionField::new("friendly_name", FieldType::Text).required(),
                    DimensionField::new("location_path", FieldType::Text),
                    DimensionField::new("correlates_with", FieldType::Text),
                    DimensionField::new("orientation", FieldType::Text),
                ],
                primary_key: vec!["ndp_id".to_string()],
                indexes: vec![
                    IndexConfig::new("idx_entity_context_category", vec!["category".to_string()]),
                    IndexConfig::new(
                        "idx_entity_context_location",
                        vec!["location_path".to_string()],
                    ),
                ],
            },
            load: LoadConfig::default(),
        }
    }

    #[test]
    fn test_generate_create_table() {
        let config = create_test_config();
        let ddl = DdlGenerator::generate_create_table(&config);

        assert!(ddl.contains("CREATE TABLE IF NOT EXISTS silver.entity_context"));
        assert!(ddl.contains("ndp_id TEXT NOT NULL"));
        assert!(ddl.contains("category TEXT NOT NULL"));
        assert!(ddl.contains("friendly_name TEXT NOT NULL"));
        assert!(ddl.contains("location_path TEXT"));
        assert!(!ddl.contains("location_path TEXT NOT NULL"));
        assert!(ddl.contains("PRIMARY KEY (ndp_id)"));
    }

    #[test]
    fn test_generate_create_table_no_pk() {
        let mut config = create_test_config();
        config.schema.primary_key.clear();

        let ddl = DdlGenerator::generate_create_table(&config);

        assert!(!ddl.contains("PRIMARY KEY"));
    }

    #[test]
    fn test_generate_indexes() {
        let config = create_test_config();
        let indexes = DdlGenerator::generate_indexes(&config);

        assert_eq!(indexes.len(), 2);
        assert!(indexes[0].contains("CREATE INDEX IF NOT EXISTS idx_entity_context_category"));
        assert!(indexes[0].contains("ON silver.entity_context (category)"));
        assert!(indexes[1].contains("idx_entity_context_location"));
    }

    #[test]
    fn test_generate_unique_index() {
        let mut config = create_test_config();
        config.schema.indexes = vec![IndexConfig {
            name: "idx_unique_test".to_string(),
            columns: vec!["category".to_string()],
            unique: true,
        }];

        let indexes = DdlGenerator::generate_indexes(&config);

        assert_eq!(indexes.len(), 1);
        assert!(indexes[0].contains("CREATE UNIQUE INDEX IF NOT EXISTS idx_unique_test"));
    }

    #[test]
    fn test_generate_truncate() {
        let config = create_test_config();
        let truncate = DdlGenerator::generate_truncate(&config);

        assert_eq!(truncate, "TRUNCATE TABLE silver.entity_context;");
    }

    #[test]
    fn test_generate_insert() {
        let config = create_test_config();
        let insert = DdlGenerator::generate_insert(&config);

        assert!(insert.contains("INSERT INTO silver.entity_context"));
        assert!(insert.contains("ndp_id, category, friendly_name"));
        assert!(insert.contains("VALUES ($1, $2, $3, $4, $5, $6)"));
    }

    #[test]
    fn test_generate_upsert() {
        let config = create_test_config();
        let upsert = DdlGenerator::generate_upsert(&config).unwrap();

        assert!(upsert.contains("INSERT INTO silver.entity_context"));
        assert!(upsert.contains("ON CONFLICT (ndp_id) DO UPDATE SET"));
        assert!(upsert.contains("category = EXCLUDED.category"));
        assert!(upsert.contains("friendly_name = EXCLUDED.friendly_name"));
        // Primary key should NOT be in update set
        assert!(!upsert.contains("ndp_id = EXCLUDED.ndp_id"));
    }

    #[test]
    fn test_generate_upsert_no_pk_fails() {
        let mut config = create_test_config();
        config.schema.primary_key.clear();

        let result = DdlGenerator::generate_upsert(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_load_statement_truncate() {
        let config = create_test_config();
        let stmt = DdlGenerator::generate_load_statement(&config).unwrap();

        // TruncateAndLoad uses simple INSERT
        assert!(stmt.contains("INSERT INTO"));
        assert!(!stmt.contains("ON CONFLICT"));
    }

    #[test]
    fn test_generate_load_statement_upsert() {
        let mut config = create_test_config();
        config.load.strategy = LoadStrategy::Upsert;

        let stmt = DdlGenerator::generate_load_statement(&config).unwrap();

        assert!(stmt.contains("ON CONFLICT"));
    }

    #[test]
    fn test_generate_full_ddl() {
        let config = create_test_config();
        let full_ddl = DdlGenerator::generate_full_ddl(&config);

        // Should contain CREATE TABLE
        assert!(full_ddl.contains("CREATE TABLE IF NOT EXISTS"));
        // Should contain both indexes
        assert!(full_ddl.contains("idx_entity_context_category"));
        assert!(full_ddl.contains("idx_entity_context_location"));
    }

    #[test]
    fn test_generate_drop_table() {
        let config = create_test_config();
        let drop = DdlGenerator::generate_drop_table(&config);

        assert_eq!(drop, "DROP TABLE IF EXISTS silver.entity_context;");
    }

    #[test]
    fn test_generate_drop_indexes() {
        let config = create_test_config();
        let drops = DdlGenerator::generate_drop_indexes(&config);

        assert_eq!(drops.len(), 2);
        assert!(drops[0].contains("DROP INDEX IF EXISTS silver.idx_entity_context_category"));
    }

    #[test]
    fn test_generate_create_schema() {
        let config = create_test_config();
        let schema_ddl = DdlGenerator::generate_create_schema(&config);

        assert_eq!(schema_ddl, "CREATE SCHEMA IF NOT EXISTS silver;");
    }

    #[test]
    fn test_field_types_mapping() {
        let mut config = create_test_config();
        config.schema.fields = vec![
            DimensionField::new("text_col", FieldType::Text),
            DimensionField::new("int_col", FieldType::Integer),
            DimensionField::new("float_col", FieldType::Float),
            DimensionField::new("bool_col", FieldType::Boolean),
            DimensionField::new("ts_col", FieldType::Timestamp),
            DimensionField::new("json_col", FieldType::Jsonb),
        ];
        config.schema.primary_key.clear();

        let ddl = DdlGenerator::generate_create_table(&config);

        assert!(ddl.contains("text_col TEXT"));
        assert!(ddl.contains("int_col BIGINT"));
        assert!(ddl.contains("float_col DOUBLE PRECISION"));
        assert!(ddl.contains("bool_col BOOLEAN"));
        assert!(ddl.contains("ts_col TIMESTAMPTZ"));
        assert!(ddl.contains("json_col JSONB"));
    }

    #[test]
    fn test_composite_primary_key() {
        let mut config = create_test_config();
        config.schema.primary_key = vec!["ndp_id".to_string(), "category".to_string()];

        let ddl = DdlGenerator::generate_create_table(&config);

        assert!(ddl.contains("PRIMARY KEY (ndp_id, category)"));
    }

    #[test]
    fn test_composite_index() {
        let mut config = create_test_config();
        config.schema.indexes = vec![IndexConfig::new(
            "idx_composite",
            vec!["category".to_string(), "location_path".to_string()],
        )];

        let indexes = DdlGenerator::generate_indexes(&config);

        assert_eq!(indexes.len(), 1);
        assert!(indexes[0].contains("(category, location_path)"));
    }
}
