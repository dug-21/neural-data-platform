//! Dimension loader implementation
//!
//! Loads dimension (reference) data directly to Silver layer (TimescaleDB),
//! bypassing Bronze. Supports truncate_and_load and upsert strategies.
//!
//! # Architecture (DP-013)
//!
//! ```text
//! CSV File -> DimensionLoader -> Silver (TimescaleDB)
//! ```
//!
//! All behavior is configuration-driven via DimensionConfig YAML.

use async_trait::async_trait;
use std::collections::HashMap;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{debug, info, warn};

use super::ddl::DdlGenerator;
use super::error::DimensionError;
use crate::types::dimension_config::{DimensionConfig, LoadStrategy};

// FieldType is used in timescale feature and tests
#[cfg(any(feature = "timescale", test))]
#[allow(unused_imports)]
use crate::types::dimension_config::FieldType;

/// Statistics from a dimension load operation
#[derive(Debug, Clone, Default)]
pub struct DimensionLoadStats {
    /// Number of rows read from source
    pub rows_processed: usize,
    /// Number of rows successfully loaded to target
    pub rows_loaded: usize,
    /// Number of rows skipped due to validation errors
    pub rows_skipped: usize,
    /// Number of rows deleted (truncate_and_load only)
    pub rows_deleted: Option<usize>,
    /// Duration of load operation in milliseconds
    pub duration_ms: u64,
}

impl DimensionLoadStats {
    /// Create new empty stats
    pub fn new() -> Self {
        Self::default()
    }
}

/// Result type for dimension operations
pub type DimensionResult<T> = Result<T, DimensionError>;

/// Trait for loading dimension data to storage
///
/// Dimension loaders handle the complete lifecycle:
/// 1. Read from source (CSV)
/// 2. Validate data against schema
/// 3. Load to target (TimescaleDB)
#[async_trait]
pub trait DimensionLoader: Send + Sync {
    /// Get the dimension identifier
    fn dimension_id(&self) -> &str;

    /// Validate source data without loading
    ///
    /// Reads the source file and validates all rows against the schema.
    /// Returns Ok(()) if validation passes, or error with details.
    async fn validate(&self) -> DimensionResult<()>;

    /// Dry-run: validate and report what would happen
    ///
    /// Returns statistics about what the load operation would do
    /// without actually modifying the database.
    async fn dry_run(&self) -> DimensionResult<DimensionLoadStats>;

    /// Check if the loader can connect to storage (health check)
    async fn health_check(&self) -> DimensionResult<bool>;
}

/// CSV dimension loader implementation
///
/// Reads CSV files and loads them to TimescaleDB Silver layer.
/// All column mappings and types are driven by DimensionConfig.
pub struct CsvDimensionLoader {
    config: DimensionConfig,
}

impl CsvDimensionLoader {
    /// Create a new CSV dimension loader from configuration
    pub fn new(config: DimensionConfig) -> Self {
        Self { config }
    }

    /// Get the dimension configuration
    pub fn config(&self) -> &DimensionConfig {
        &self.config
    }

    /// Read and parse CSV file into validated rows
    ///
    /// Returns a vector of rows, where each row is a map of field name to parsed value.
    pub async fn read_source(&self) -> DimensionResult<Vec<HashMap<String, serde_json::Value>>> {
        let path = &self.config.source.path;
        let delimiter = self.config.source.delimiter;

        info!(
            dimension_id = %self.config.dimension_id,
            path = %path.display(),
            "Reading dimension source file"
        );

        let file = File::open(path).await.map_err(|e| {
            DimensionError::IoError(std::io::Error::new(
                e.kind(),
                format!("Failed to open '{}': {}", path.display(), e),
            ))
        })?;

        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        // Read header line
        let header_line = lines
            .next_line()
            .await?
            .ok_or_else(|| DimensionError::csv_parse(1, "CSV file is empty - no header row"))?;

        let headers: Vec<String> = parse_csv_line(&header_line, delimiter);

        // Validate header matches schema fields
        self.validate_headers(&headers)?;

        let mut rows = Vec::new();
        let mut line_number = 1; // Header is line 1

        while let Some(line) = lines.next_line().await? {
            line_number += 1;

            if line.trim().is_empty() {
                debug!(line = line_number, "Skipping empty line");
                continue;
            }

            let values = parse_csv_line(&line, delimiter);

            if values.len() != headers.len() {
                return Err(DimensionError::SchemaMismatch {
                    expected: headers.len(),
                    actual: values.len(),
                });
            }

            match self.parse_row(&headers, &values, line_number) {
                Ok(row) => rows.push(row),
                Err(e) => {
                    // Could make this configurable (skip vs abort)
                    warn!(
                        line = line_number,
                        error = %e,
                        "Skipping invalid row"
                    );
                    continue;
                }
            }
        }

        info!(
            dimension_id = %self.config.dimension_id,
            rows = rows.len(),
            "CSV parsing complete"
        );

        Ok(rows)
    }

    /// Validate that CSV headers match schema fields
    fn validate_headers(&self, headers: &[String]) -> DimensionResult<()> {
        let schema_fields: Vec<&str> = self.config.schema.column_names();

        // Check that all schema fields exist in CSV
        for schema_field in &schema_fields {
            if !headers.iter().any(|h| h == *schema_field) {
                return Err(DimensionError::config(format!(
                    "Schema field '{}' not found in CSV headers: {:?}",
                    schema_field, headers
                )));
            }
        }

        // Warn about extra CSV columns that won't be loaded
        for header in headers {
            if !schema_fields.contains(&header.as_str()) {
                warn!(
                    column = %header,
                    "CSV column not in schema - will be ignored"
                );
            }
        }

        Ok(())
    }

    /// Parse a single CSV row into a map of field values
    fn parse_row(
        &self,
        headers: &[String],
        values: &[String],
        line_number: usize,
    ) -> DimensionResult<HashMap<String, serde_json::Value>> {
        let mut row = HashMap::new();

        for field in &self.config.schema.fields {
            // Find the column index for this field
            let col_idx = headers.iter().position(|h| h == &field.name);

            let value = match col_idx {
                Some(idx) => &values[idx],
                None => {
                    // Field not in CSV - check if required
                    if !field.nullable {
                        return Err(DimensionError::missing_field(&field.name, line_number));
                    }
                    // Use null for missing nullable fields
                    row.insert(field.name.clone(), serde_json::Value::Null);
                    continue;
                }
            };

            // Parse the value according to field type
            let parsed = if value.is_empty() {
                if !field.nullable {
                    return Err(DimensionError::missing_field(&field.name, line_number));
                }
                serde_json::Value::Null
            } else {
                field
                    .field_type
                    .parse_value(value)
                    .map_err(|e| DimensionError::invalid_type(&field.name, e))?
            };

            row.insert(field.name.clone(), parsed);
        }

        Ok(row)
    }

    /// Validate all rows against schema
    async fn validate_rows(&self) -> DimensionResult<(usize, usize)> {
        let rows = self.read_source().await?;
        let total = rows.len();

        // All rows that made it through read_source are valid
        // (invalid rows are logged and skipped during parsing)
        Ok((total, 0))
    }

    /// Generate CREATE TABLE DDL from schema
    ///
    /// Delegates to DdlGenerator - all DDL comes from config, no hardcoding.
    pub fn generate_create_table_ddl(&self) -> String {
        DdlGenerator::generate_create_table(&self.config)
    }

    /// Generate CREATE INDEX statements from schema
    ///
    /// Delegates to DdlGenerator - indexes defined in config YAML.
    pub fn generate_indexes_ddl(&self) -> Vec<String> {
        DdlGenerator::generate_indexes(&self.config)
    }

    /// Generate full DDL (CREATE TABLE + all indexes)
    ///
    /// Delegates to DdlGenerator - produces complete schema from config.
    pub fn generate_full_ddl(&self) -> String {
        DdlGenerator::generate_full_ddl(&self.config)
    }

    /// Generate INSERT statement for batch loading
    ///
    /// Delegates to DdlGenerator for consistent SQL generation.
    fn generate_insert_sql(&self) -> String {
        DdlGenerator::generate_insert(&self.config)
    }

    /// Generate UPSERT statement (INSERT ON CONFLICT DO UPDATE)
    ///
    /// Delegates to DdlGenerator for consistent SQL generation.
    fn generate_upsert_sql(&self) -> String {
        DdlGenerator::generate_upsert(&self.config)
            .expect("Upsert generation requires primary key - validated at load time")
    }

    /// Generate DELETE statement for truncate_and_load
    ///
    /// Delegates to DdlGenerator for consistent SQL generation.
    fn generate_delete_sql(&self) -> String {
        DdlGenerator::generate_delete_all(&self.config)
    }
}

#[async_trait]
impl DimensionLoader for CsvDimensionLoader {
    fn dimension_id(&self) -> &str {
        &self.config.dimension_id
    }

    async fn validate(&self) -> DimensionResult<()> {
        // Check file exists
        if !self.config.source.path.exists() {
            return Err(DimensionError::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "Source file not found: {}",
                    self.config.source.path.display()
                ),
            )));
        }

        // Validate primary key configuration
        self.config
            .schema
            .validate_primary_key()
            .map_err(DimensionError::config)?;

        // Read and validate all rows
        let (valid, skipped) = self.validate_rows().await?;

        info!(
            dimension_id = %self.config.dimension_id,
            valid_rows = valid,
            skipped_rows = skipped,
            "Validation complete"
        );

        Ok(())
    }

    async fn dry_run(&self) -> DimensionResult<DimensionLoadStats> {
        let start = std::time::Instant::now();

        // Validate source file exists
        self.validate().await?;

        let rows = self.read_source().await?;
        let rows_processed = rows.len();

        let stats = DimensionLoadStats {
            rows_processed,
            rows_loaded: rows_processed, // Would be loaded if not dry run
            rows_skipped: 0,
            rows_deleted: match self.config.load.strategy {
                LoadStrategy::TruncateAndLoad => Some(0), // Would delete existing
                LoadStrategy::Upsert => None,
            },
            duration_ms: start.elapsed().as_millis() as u64,
        };

        info!(
            dimension_id = %self.config.dimension_id,
            strategy = ?self.config.load.strategy,
            rows = rows_processed,
            "Dry run complete"
        );

        Ok(stats)
    }

    async fn health_check(&self) -> DimensionResult<bool> {
        // For CSV loader, just check if source file is readable
        let exists = self.config.source.path.exists();
        if !exists {
            warn!(
                path = %self.config.source.path.display(),
                "Source file does not exist"
            );
        }
        Ok(exists)
    }
}

// ============================================================================
// Feature-gated TimescaleDB implementation
// ============================================================================

#[cfg(feature = "timescale")]
pub mod timescale {
    use super::*;
    use bb8::Pool;
    use bb8_postgres::PostgresConnectionManager;
    use tokio_postgres::types::ToSql;
    use tokio_postgres::NoTls;

    type PgPool = Pool<PostgresConnectionManager<NoTls>>;

    /// Extension trait for loading dimensions to TimescaleDB
    #[async_trait]
    pub trait TimescaleDimensionLoader: DimensionLoader {
        /// Load dimension data to TimescaleDB
        async fn load(&self, pool: &PgPool) -> DimensionResult<DimensionLoadStats>;

        /// Ensure the target table exists (creates if not)
        async fn ensure_table(&self, pool: &PgPool) -> DimensionResult<()>;
    }

    #[async_trait]
    impl TimescaleDimensionLoader for CsvDimensionLoader {
        async fn load(&self, pool: &PgPool) -> DimensionResult<DimensionLoadStats> {
            let start = std::time::Instant::now();

            info!(
                dimension_id = %self.config.dimension_id,
                table = %self.config.target.qualified_name(),
                strategy = ?self.config.load.strategy,
                "Starting dimension load"
            );

            // Read source data
            let rows = self.read_source().await?;
            let rows_processed = rows.len();

            if rows.is_empty() {
                warn!(
                    dimension_id = %self.config.dimension_id,
                    "No rows to load"
                );
                return Ok(DimensionLoadStats {
                    rows_processed: 0,
                    rows_loaded: 0,
                    rows_skipped: 0,
                    rows_deleted: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                });
            }

            // Get connection from pool
            let mut conn = pool.get().await.map_err(|e| {
                DimensionError::ConnectionError(format!("Failed to get connection: {}", e))
            })?;

            // Start transaction
            let tx = conn.transaction().await.map_err(|e| {
                DimensionError::TransactionError(format!("Failed to start transaction: {}", e))
            })?;

            let mut rows_loaded = 0;
            let mut rows_deleted = None;

            match self.config.load.strategy {
                LoadStrategy::TruncateAndLoad => {
                    // Delete all existing data
                    let delete_sql = self.generate_delete_sql();
                    let deleted = tx
                        .execute(&delete_sql, &[])
                        .await
                        .map_err(|e| DimensionError::database(format!("Delete failed: {}", e)))?;
                    rows_deleted = Some(deleted as usize);

                    debug!(
                        dimension_id = %self.config.dimension_id,
                        deleted = deleted,
                        "Deleted existing rows"
                    );

                    // Insert all rows
                    rows_loaded = batch_insert(&tx, self, &rows).await?;
                }
                LoadStrategy::Upsert => {
                    rows_loaded = batch_upsert(&tx, self, &rows).await?;
                }
            }

            // Commit transaction
            tx.commit()
                .await
                .map_err(|e| DimensionError::TransactionError(format!("Commit failed: {}", e)))?;

            let stats = DimensionLoadStats {
                rows_processed,
                rows_loaded,
                rows_skipped: rows_processed - rows_loaded,
                rows_deleted,
                duration_ms: start.elapsed().as_millis() as u64,
            };

            info!(
                dimension_id = %self.config.dimension_id,
                rows_loaded = stats.rows_loaded,
                rows_deleted = ?stats.rows_deleted,
                duration_ms = stats.duration_ms,
                "Dimension load complete"
            );

            Ok(stats)
        }

        async fn ensure_table(&self, pool: &PgPool) -> DimensionResult<()> {
            let conn = pool.get().await.map_err(|e| {
                DimensionError::ConnectionError(format!("Failed to get connection: {}", e))
            })?;

            // Create table
            let create_table_ddl = self.generate_create_table_ddl();
            debug!(
                dimension_id = %self.config.dimension_id,
                ddl = %create_table_ddl,
                "Creating table if not exists"
            );

            conn.execute(&create_table_ddl, &[])
                .await
                .map_err(|e| DimensionError::database(format!("Failed to create table: {}", e)))?;

            // Create indexes
            let index_ddls = self.generate_indexes_ddl();
            for index_ddl in &index_ddls {
                debug!(
                    dimension_id = %self.config.dimension_id,
                    ddl = %index_ddl,
                    "Creating index if not exists"
                );

                conn.execute(index_ddl.as_str(), &[]).await.map_err(|e| {
                    DimensionError::database(format!("Failed to create index: {}", e))
                })?;
            }

            info!(
                dimension_id = %self.config.dimension_id,
                table = %self.config.target.qualified_name(),
                indexes = index_ddls.len(),
                "Table and indexes ensured"
            );

            Ok(())
        }
    }

    /// Batch insert rows using batched statements
    async fn batch_insert(
        tx: &tokio_postgres::Transaction<'_>,
        loader: &CsvDimensionLoader,
        rows: &[HashMap<String, serde_json::Value>],
    ) -> DimensionResult<usize> {
        let insert_sql = loader.generate_insert_sql();
        let columns = loader.config.schema.column_names();
        let batch_size = loader.config.load.batch_size;

        let mut inserted = 0;

        for chunk in rows.chunks(batch_size) {
            for row in chunk {
                let params = build_params(row, &columns, &loader.config.schema.fields)?;
                let param_refs: Vec<&(dyn ToSql + Sync)> =
                    params.iter().map(|p| p.as_ref()).collect();

                tx.execute(&insert_sql, &param_refs)
                    .await
                    .map_err(|e| DimensionError::database(format!("Insert failed: {}", e)))?;

                inserted += 1;
            }
        }

        Ok(inserted)
    }

    /// Batch upsert rows using INSERT ON CONFLICT
    async fn batch_upsert(
        tx: &tokio_postgres::Transaction<'_>,
        loader: &CsvDimensionLoader,
        rows: &[HashMap<String, serde_json::Value>],
    ) -> DimensionResult<usize> {
        let upsert_sql = loader.generate_upsert_sql();
        let columns = loader.config.schema.column_names();
        let batch_size = loader.config.load.batch_size;

        let mut upserted = 0;

        for chunk in rows.chunks(batch_size) {
            for row in chunk {
                let params = build_params(row, &columns, &loader.config.schema.fields)?;
                let param_refs: Vec<&(dyn ToSql + Sync)> =
                    params.iter().map(|p| p.as_ref()).collect();

                tx.execute(&upsert_sql, &param_refs)
                    .await
                    .map_err(|e| DimensionError::database(format!("Upsert failed: {}", e)))?;

                upserted += 1;
            }
        }

        Ok(upserted)
    }

    /// Build parameter values from row data
    fn build_params(
        row: &HashMap<String, serde_json::Value>,
        columns: &[&str],
        fields: &[crate::types::dimension_config::DimensionField],
    ) -> DimensionResult<Vec<Box<dyn ToSql + Sync + Send>>> {
        let mut params: Vec<Box<dyn ToSql + Sync + Send>> = Vec::new();

        for col in columns {
            let field = fields
                .iter()
                .find(|f| f.name == *col)
                .ok_or_else(|| DimensionError::config(format!("Field '{}' not in schema", col)))?;

            let value = row.get(*col).cloned().unwrap_or(serde_json::Value::Null);

            let param: Box<dyn ToSql + Sync + Send> = match (&field.field_type, value) {
                (_, serde_json::Value::Null) => Box::new(None::<String>),
                (FieldType::Text, serde_json::Value::String(s)) => Box::new(s),
                (FieldType::Integer, serde_json::Value::Number(n)) => {
                    Box::new(n.as_i64().unwrap_or(0))
                }
                (FieldType::Float, serde_json::Value::Number(n)) => {
                    Box::new(n.as_f64().unwrap_or(0.0))
                }
                (FieldType::Boolean, serde_json::Value::Bool(b)) => Box::new(b),
                (FieldType::Timestamp, serde_json::Value::String(s)) => {
                    // Parse timestamp string
                    match chrono::DateTime::parse_from_rfc3339(&s) {
                        Ok(dt) => Box::new(dt.with_timezone(&chrono::Utc)),
                        Err(_) => {
                            // Try parsing as naive datetime
                            match chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S") {
                                Ok(ndt) => Box::new(ndt.and_utc()),
                                Err(_) => Box::new(None::<chrono::DateTime<chrono::Utc>>),
                            }
                        }
                    }
                }
                (FieldType::Jsonb, v) => Box::new(serde_json::to_string(&v).unwrap_or_default()),
                _ => Box::new(None::<String>), // Fallback for unexpected combinations
            };

            params.push(param);
        }

        Ok(params)
    }
}

// Re-export timescale types when feature is enabled
#[cfg(feature = "timescale")]
pub use timescale::TimescaleDimensionLoader;

// ============================================================================
// Helper functions
// ============================================================================

/// Parse a CSV line into fields, handling quoted values
fn parse_csv_line(line: &str, delimiter: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '"' {
            if in_quotes {
                // Check for escaped quote
                if chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                in_quotes = true;
            }
        } else if c == delimiter && !in_quotes {
            fields.push(current.trim().to_string());
            current = String::new();
        } else {
            current.push(c);
        }
    }

    // Don't forget the last field
    fields.push(current.trim().to_string());

    fields
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::dimension_config::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_config(path: std::path::PathBuf) -> DimensionConfig {
        DimensionConfig {
            dimension_id: "test-dimension".to_string(),
            target: DimensionTarget {
                table: "test_dimension".to_string(),
                schema: "silver".to_string(),
            },
            source: DimensionSource {
                source_type: DimensionSourceType::Csv,
                path,
                delimiter: ',',
            },
            schema: DimensionSchema {
                fields: vec![
                    DimensionField::new("id", FieldType::Text).required(),
                    DimensionField::new("name", FieldType::Text),
                    DimensionField::new("value", FieldType::Integer),
                ],
                primary_key: vec!["id".to_string()],
                indexes: vec![],
            },
            load: LoadConfig {
                strategy: LoadStrategy::TruncateAndLoad,
                batch_size: 100,
            },
        }
    }

    #[test]
    fn test_parse_csv_line_simple() {
        let line = "a,b,c";
        let result = parse_csv_line(line, ',');
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_parse_csv_line_quoted() {
        let line = r#""hello, world",b,c"#;
        let result = parse_csv_line(line, ',');
        assert_eq!(result, vec!["hello, world", "b", "c"]);
    }

    #[test]
    fn test_parse_csv_line_escaped_quotes() {
        let line = r#""say ""hello""",b"#;
        let result = parse_csv_line(line, ',');
        assert_eq!(result, vec![r#"say "hello""#, "b"]);
    }

    #[test]
    fn test_parse_csv_line_whitespace() {
        let line = " a , b , c ";
        let result = parse_csv_line(line, ',');
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[tokio::test]
    async fn test_csv_loader_read_source() {
        // Create temp CSV file
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "id,name,value").unwrap();
        writeln!(file, "1,Alice,100").unwrap();
        writeln!(file, "2,Bob,200").unwrap();
        file.flush().unwrap();

        let config = create_test_config(file.path().to_path_buf());
        let loader = CsvDimensionLoader::new(config);

        let rows = loader.read_source().await.unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["id"], serde_json::json!("1"));
        assert_eq!(rows[0]["name"], serde_json::json!("Alice"));
        assert_eq!(rows[0]["value"], serde_json::json!(100));
    }

    #[tokio::test]
    async fn test_csv_loader_validate_headers() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "id,name,value,extra").unwrap();
        writeln!(file, "1,Test,42,ignored").unwrap();
        file.flush().unwrap();

        let config = create_test_config(file.path().to_path_buf());
        let loader = CsvDimensionLoader::new(config);

        // Should succeed - extra columns are warned but allowed
        let result = loader.validate().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_csv_loader_missing_header() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "id,name").unwrap(); // Missing 'value'
        writeln!(file, "1,Test").unwrap();
        file.flush().unwrap();

        let config = create_test_config(file.path().to_path_buf());
        let loader = CsvDimensionLoader::new(config);

        // Should fail - required schema field missing from CSV
        let result = loader.read_source().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_csv_loader_dry_run() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "id,name,value").unwrap();
        writeln!(file, "1,Alice,100").unwrap();
        writeln!(file, "2,Bob,200").unwrap();
        writeln!(file, "3,Charlie,300").unwrap();
        file.flush().unwrap();

        let config = create_test_config(file.path().to_path_buf());
        let loader = CsvDimensionLoader::new(config);

        let stats = loader.dry_run().await.unwrap();

        assert_eq!(stats.rows_processed, 3);
        assert_eq!(stats.rows_loaded, 3);
        assert_eq!(stats.rows_skipped, 0);
        assert!(stats.rows_deleted.is_some()); // truncate_and_load
    }

    #[test]
    fn test_generate_create_table_ddl() {
        let config = DimensionConfig {
            dimension_id: "entity-context".to_string(),
            target: DimensionTarget {
                table: "entity_context".to_string(),
                schema: "silver".to_string(),
            },
            source: DimensionSource {
                source_type: DimensionSourceType::Csv,
                path: "test.csv".into(),
                delimiter: ',',
            },
            schema: DimensionSchema {
                fields: vec![
                    DimensionField::new("ndp_id", FieldType::Text).required(),
                    DimensionField::new("category", FieldType::Text).required(),
                    DimensionField::new("friendly_name", FieldType::Text),
                ],
                primary_key: vec!["ndp_id".to_string()],
                indexes: vec![],
            },
            load: LoadConfig::default(),
        };

        let loader = CsvDimensionLoader::new(config);
        let ddl = loader.generate_create_table_ddl();

        assert!(ddl.contains("CREATE TABLE IF NOT EXISTS silver.entity_context"));
        assert!(ddl.contains("ndp_id TEXT NOT NULL"));
        assert!(ddl.contains("category TEXT NOT NULL"));
        assert!(ddl.contains("friendly_name TEXT"));
        assert!(ddl.contains("PRIMARY KEY (ndp_id)"));
    }

    #[test]
    fn test_generate_insert_sql() {
        let config = create_test_config("test.csv".into());
        let loader = CsvDimensionLoader::new(config);
        let sql = loader.generate_insert_sql();

        // Check key components (format may vary)
        assert!(sql.contains("INSERT INTO silver.test_dimension"));
        assert!(sql.contains("(id, name, value)"));
        assert!(sql.contains("VALUES ($1, $2, $3)"));
    }

    #[test]
    fn test_generate_upsert_sql() {
        let config = create_test_config("test.csv".into());
        let loader = CsvDimensionLoader::new(config);
        let sql = loader.generate_upsert_sql();

        assert!(sql.contains("INSERT INTO silver.test_dimension"));
        assert!(sql.contains("ON CONFLICT (id) DO UPDATE SET"));
        assert!(sql.contains("name = EXCLUDED.name"));
        assert!(sql.contains("value = EXCLUDED.value"));
    }

    #[tokio::test]
    async fn test_csv_loader_health_check() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "id,name,value").unwrap();
        file.flush().unwrap();

        let config = create_test_config(file.path().to_path_buf());
        let loader = CsvDimensionLoader::new(config);

        // File exists - healthy
        assert!(loader.health_check().await.unwrap());

        // Non-existent file
        let bad_config = create_test_config("/nonexistent/file.csv".into());
        let bad_loader = CsvDimensionLoader::new(bad_config);
        assert!(!bad_loader.health_check().await.unwrap());
    }

    #[test]
    fn test_dimension_id() {
        let config = create_test_config("test.csv".into());
        let loader = CsvDimensionLoader::new(config);
        assert_eq!(loader.dimension_id(), "test-dimension");
    }
}
