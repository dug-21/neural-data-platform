# dp-013: CSV Source & Dimension Tables - Rust Implementation Patterns

This document defines the Rust implementation patterns for CSV source type and dimension table loading, following NDP's established Domain Adapter architecture.

---

## 1. Trait Design

### 1.1 CsvSource - Implementing RawSource Trait

CSV sources implement the existing `RawSource` trait from `core/src/traits.rs`. This follows NDP's principle that all timeseries data flows through Bronze layer regardless of transport.

```rust
// core/src/sources/csv.rs

use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Deserialize;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::BufReader;

use crate::error::{CoreError, CoreResult};
use crate::traits::RawSource;
use crate::types::RawDataPoint;

/// CSV Source adapter - reads CSV files and produces RawDataPoints for Bronze layer
///
/// Implements RawSource trait to match HTTP/MQTT sources. Each CSV row becomes
/// a RawDataPoint with the entire row stored as JSON in raw_payload.
pub struct CsvSource {
    /// Source configuration
    config: CsvSourceConfig,
    /// Generated source_id (format: "{stream_id}-Csv")
    source_id: String,
    /// Optional ndp_id from config
    ndp_id: Option<String>,
    /// Optional context metadata from config
    context: Option<serde_json::Value>,
}

impl CsvSource {
    /// Create new CsvSource from configuration
    pub fn new(
        stream_id: &str,
        config: CsvSourceConfig,
        ndp_id: Option<String>,
        context: Option<serde_json::Value>,
    ) -> Self {
        Self {
            config,
            source_id: format!("{}-Csv", stream_id),
            ndp_id,
            context,
        }
    }

    /// Parse timestamp from string value according to configured format
    fn parse_timestamp(&self, value: &str) -> CoreResult<DateTime<Utc>> {
        match &self.config.timestamp_format {
            TimestampFormat::Iso8601 => {
                DateTime::parse_from_rfc3339(value)
                    .map(|dt| dt.with_timezone(&Utc))
                    .or_else(|_| {
                        // Try without timezone
                        NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
                            .map(|ndt| ndt.and_utc())
                    })
                    .map_err(|e| CoreError::Parser(format!(
                        "Failed to parse ISO8601 timestamp '{}': {}", value, e
                    )))
            }
            TimestampFormat::EpochSeconds => {
                value.parse::<i64>()
                    .map(|secs| DateTime::from_timestamp(secs, 0).unwrap_or_else(Utc::now))
                    .map_err(|e| CoreError::Parser(format!(
                        "Failed to parse epoch timestamp '{}': {}", value, e
                    )))
            }
            TimestampFormat::EpochMillis => {
                value.parse::<i64>()
                    .map(|ms| DateTime::from_timestamp_millis(ms).unwrap_or_else(Utc::now))
                    .map_err(|e| CoreError::Parser(format!(
                        "Failed to parse epoch millis timestamp '{}': {}", value, e
                    )))
            }
            TimestampFormat::Custom(format_str) => {
                NaiveDateTime::parse_from_str(value, format_str)
                    .map(|ndt| ndt.and_utc())
                    .map_err(|e| CoreError::Parser(format!(
                        "Failed to parse timestamp '{}' with format '{}': {}",
                        value, format_str, e
                    )))
            }
        }
    }
}

#[async_trait]
impl RawSource for CsvSource {
    /// Fetch single raw data point - not typically used for CSV
    async fn fetch_raw(&self) -> CoreResult<RawDataPoint> {
        let batch = self.fetch_raw_batch().await?;
        batch.into_iter().next().ok_or_else(|| {
            CoreError::Source("CSV file is empty".to_string())
        })
    }

    /// Fetch all rows from CSV as RawDataPoints
    ///
    /// This is the primary method for CSV ingestion. Each row becomes a
    /// RawDataPoint with:
    /// - timestamp: Parsed from configured timestamp_field
    /// - source_id: "{stream_id}-Csv"
    /// - raw_payload: JSON object with all CSV columns
    async fn fetch_raw_batch(&self) -> CoreResult<Vec<RawDataPoint>> {
        let file = File::open(&self.config.path).await.map_err(|e| {
            CoreError::Source(format!(
                "Failed to open CSV file '{}': {}",
                self.config.path.display(), e
            ))
        })?;

        let reader = BufReader::new(file);
        let mut csv_reader = csv_async::AsyncReaderBuilder::new()
            .delimiter(self.config.delimiter)
            .has_headers(true)
            .create_reader(reader);

        let headers = csv_reader.headers().await.map_err(|e| {
            CoreError::Parser(format!("Failed to read CSV headers: {}", e))
        })?.clone();

        // Validate timestamp field exists
        if !headers.iter().any(|h| h == &self.config.timestamp_field) {
            return Err(CoreError::Validation(format!(
                "Timestamp field '{}' not found in CSV headers: {:?}",
                self.config.timestamp_field,
                headers.iter().collect::<Vec<_>>()
            )));
        }

        let mut points = Vec::new();
        let mut line_number = 1; // Start at 1 (header is line 0)

        let mut records = csv_reader.records();
        while let Some(result) = records.next().await {
            line_number += 1;

            match result {
                Ok(record) => {
                    match self.process_record(&headers, &record, line_number) {
                        Ok(point) => points.push(point),
                        Err(e) => {
                            match self.config.on_error {
                                OnError::Skip => {
                                    tracing::warn!(
                                        line = line_number,
                                        error = %e,
                                        "Skipping invalid CSV row"
                                    );
                                    continue;
                                }
                                OnError::Abort => return Err(e),
                            }
                        }
                    }
                }
                Err(e) => {
                    let parse_error = CsvError::Parse {
                        line: line_number,
                        message: e.to_string(),
                    };
                    match self.config.on_error {
                        OnError::Skip => {
                            tracing::warn!(
                                line = line_number,
                                error = %e,
                                "Skipping malformed CSV row"
                            );
                            continue;
                        }
                        OnError::Abort => return Err(parse_error.into()),
                    }
                }
            }
        }

        tracing::info!(
            source_id = %self.source_id,
            rows = points.len(),
            "CSV ingestion complete"
        );

        Ok(points)
    }
}

impl CsvSource {
    /// Process a single CSV record into a RawDataPoint
    fn process_record(
        &self,
        headers: &csv_async::StringRecord,
        record: &csv_async::StringRecord,
        line_number: usize,
    ) -> CoreResult<RawDataPoint> {
        // Build JSON object from CSV row
        let mut row_data = serde_json::Map::new();
        let mut timestamp_value: Option<String> = None;

        for (idx, header) in headers.iter().enumerate() {
            if let Some(value) = record.get(idx) {
                // Store raw string value - let downstream handle type conversion
                row_data.insert(
                    header.to_string(),
                    serde_json::Value::String(value.to_string())
                );

                if header == &self.config.timestamp_field {
                    timestamp_value = Some(value.to_string());
                }
            }
        }

        let timestamp = match timestamp_value {
            Some(ts) => self.parse_timestamp(&ts)?,
            None => {
                return Err(CsvError::MissingField {
                    line: line_number,
                    field: self.config.timestamp_field.clone(),
                }.into());
            }
        };

        Ok(RawDataPoint::new(&self.source_id, serde_json::Value::Object(row_data))
            .with_timestamp(timestamp)
            .with_ndp_id_opt(self.ndp_id.clone())
            .with_context_opt(self.context.clone()))
    }
}
```

### 1.2 DimensionLoader Trait

Dimension loading is a separate concern from streaming sources. It loads reference data directly to Silver layer.

```rust
// core/src/dimensions/loader.rs

use async_trait::async_trait;
use sqlx::PgPool;

use crate::error::CoreResult;

/// Statistics from a dimension load operation
#[derive(Debug, Clone)]
pub struct DimensionLoadStats {
    /// Number of rows processed from source
    pub rows_processed: usize,
    /// Number of rows loaded to target
    pub rows_loaded: usize,
    /// Number of rows skipped (validation errors)
    pub rows_skipped: usize,
    /// Number of rows deleted (truncate_and_load only)
    pub rows_deleted: Option<usize>,
    /// Duration of load operation
    pub duration_ms: u64,
}

/// Trait for loading dimension data to target storage
///
/// Dimension loaders handle the complete lifecycle:
/// 1. Read from source (CSV, API, etc.)
/// 2. Validate data against schema
/// 3. Load to target (TimescaleDB)
#[async_trait]
pub trait DimensionLoader: Send + Sync {
    /// Dimension identifier
    fn dimension_id(&self) -> &str;

    /// Validate source data without loading
    ///
    /// Returns Ok(()) if validation passes, or error with details
    async fn validate(&self) -> CoreResult<()>;

    /// Load dimension data to target
    ///
    /// Executes the configured load strategy (truncate_and_load or upsert)
    async fn load(&self, pool: &PgPool) -> CoreResult<DimensionLoadStats>;

    /// Dry-run: validate and report what would happen
    async fn dry_run(&self) -> CoreResult<DimensionLoadStats>;
}

/// CSV dimension loader implementation
pub struct CsvDimensionLoader {
    config: DimensionConfig,
}

impl CsvDimensionLoader {
    pub fn new(config: DimensionConfig) -> Self {
        Self { config }
    }

    /// Read and parse CSV file into validated rows
    async fn read_source(&self) -> CoreResult<Vec<serde_json::Map<String, serde_json::Value>>> {
        // Implementation similar to CsvSource but returns structured data
        // for dimension loading rather than RawDataPoints
        todo!("Implementation follows CsvSource pattern")
    }

    /// Validate a row against dimension schema
    fn validate_row(
        &self,
        row: &serde_json::Map<String, serde_json::Value>,
        line: usize,
    ) -> CoreResult<()> {
        for field in &self.config.schema.fields {
            if field.required {
                if !row.contains_key(&field.name) || row[&field.name].is_null() {
                    return Err(CsvError::MissingField {
                        line,
                        field: field.name.clone(),
                    }.into());
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl DimensionLoader for CsvDimensionLoader {
    fn dimension_id(&self) -> &str {
        &self.config.dimension_id
    }

    async fn validate(&self) -> CoreResult<()> {
        let rows = self.read_source().await?;
        for (idx, row) in rows.iter().enumerate() {
            self.validate_row(row, idx + 2)?; // +2 for 1-indexed + header
        }
        Ok(())
    }

    async fn load(&self, pool: &PgPool) -> CoreResult<DimensionLoadStats> {
        let start = std::time::Instant::now();
        let rows = self.read_source().await?;

        // Validate all rows before loading
        for (idx, row) in rows.iter().enumerate() {
            self.validate_row(row, idx + 2)?;
        }

        let rows_processed = rows.len();
        let mut rows_loaded = 0;
        let mut rows_deleted = None;

        // Execute load strategy within transaction
        let mut tx = pool.begin().await.map_err(|e| {
            CoreError::DatabaseError(format!("Failed to start transaction: {}", e))
        })?;

        match self.config.load.strategy {
            LoadStrategy::TruncateAndLoad => {
                // Delete existing data
                let delete_result = sqlx::query(&format!(
                    "DELETE FROM {}",
                    &self.config.target.table
                ))
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::DatabaseError(format!("Delete failed: {}", e)))?;

                rows_deleted = Some(delete_result.rows_affected() as usize);

                // Insert all rows
                rows_loaded = self.batch_insert(&mut tx, &rows).await?;
            }
            LoadStrategy::Upsert => {
                rows_loaded = self.batch_upsert(&mut tx, &rows).await?;
            }
        }

        tx.commit().await.map_err(|e| {
            CoreError::DatabaseError(format!("Commit failed: {}", e))
        })?;

        Ok(DimensionLoadStats {
            rows_processed,
            rows_loaded,
            rows_skipped: rows_processed - rows_loaded,
            rows_deleted,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn dry_run(&self) -> CoreResult<DimensionLoadStats> {
        let rows = self.read_source().await?;
        let mut valid_rows = 0;
        let mut skipped_rows = 0;

        for (idx, row) in rows.iter().enumerate() {
            match self.validate_row(row, idx + 2) {
                Ok(_) => valid_rows += 1,
                Err(_) => skipped_rows += 1,
            }
        }

        Ok(DimensionLoadStats {
            rows_processed: rows.len(),
            rows_loaded: valid_rows,
            rows_skipped: skipped_rows,
            rows_deleted: None,
            duration_ms: 0,
        })
    }
}
```

---

## 2. Configuration Structs

### 2.1 CsvSourceConfig

```rust
// core/src/sources/csv.rs

use serde::Deserialize;
use std::path::PathBuf;

/// Timestamp format for CSV parsing
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimestampFormat {
    /// ISO 8601 format (default): "2024-01-15T10:30:00Z"
    Iso8601,
    /// Unix epoch seconds: "1705315800"
    EpochSeconds,
    /// Unix epoch milliseconds: "1705315800000"
    EpochMillis,
    /// Custom strftime format: "%Y-%m-%d %H:%M:%S"
    #[serde(untagged)]
    Custom(String),
}

impl Default for TimestampFormat {
    fn default() -> Self {
        TimestampFormat::Iso8601
    }
}

/// Error handling strategy for invalid rows
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OnError {
    /// Skip invalid rows and continue (default)
    #[default]
    Skip,
    /// Abort entire operation on first error
    Abort,
}

/// Configuration for CSV source type in stream configs
///
/// Example YAML:
/// ```yaml
/// source:
///   type: csv
///   path: data/imports/historical.csv
///   timestamp_field: timestamp
///   timestamp_format: iso8601
///   delimiter: ","
///   on_error: skip
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct CsvSourceConfig {
    /// Path to CSV file (relative to config root or absolute)
    pub path: PathBuf,

    /// Column name containing timestamp values
    pub timestamp_field: String,

    /// Format of timestamp values
    #[serde(default)]
    pub timestamp_format: TimestampFormat,

    /// Field delimiter character
    #[serde(default = "default_delimiter")]
    pub delimiter: u8,

    /// File encoding (currently only UTF-8 supported)
    #[serde(default = "default_encoding")]
    pub encoding: String,

    /// Error handling strategy
    #[serde(default)]
    pub on_error: OnError,
}

fn default_delimiter() -> u8 {
    b','
}

fn default_encoding() -> String {
    "utf-8".to_string()
}

impl Default for CsvSourceConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            timestamp_field: "timestamp".to_string(),
            timestamp_format: TimestampFormat::default(),
            delimiter: default_delimiter(),
            encoding: default_encoding(),
            on_error: OnError::default(),
        }
    }
}
```

### 2.2 DimensionConfig

```rust
// core/src/dimensions/config.rs

use serde::Deserialize;
use std::path::PathBuf;

/// Dimension table configuration
///
/// Example YAML (config/base/dimensions/entity_context.yaml):
/// ```yaml
/// dimension_id: entity-context
/// target:
///   table: silver.entity_context
///   primary_key: [ndp_id]
/// source:
///   type: csv
///   path: config/dimensions/entity_context.csv
/// schema:
///   fields:
///     - name: ndp_id
///       data_type: text
///       required: true
///     - name: category
///       data_type: text
///       required: true
/// load:
///   strategy: truncate_and_load
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct DimensionConfig {
    /// Unique dimension identifier (kebab-case)
    pub dimension_id: String,

    /// Target table configuration
    pub target: DimensionTarget,

    /// Source data configuration
    pub source: DimensionSource,

    /// Schema definition
    pub schema: DimensionSchema,

    /// Load behavior configuration
    #[serde(default)]
    pub load: LoadConfig,
}

/// Target table specification
#[derive(Debug, Clone, Deserialize)]
pub struct DimensionTarget {
    /// Fully-qualified table name (e.g., "silver.entity_context")
    pub table: String,

    /// Primary key columns for upsert operations
    pub primary_key: Vec<String>,
}

/// Dimension source configuration
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DimensionSource {
    /// CSV file source
    Csv {
        /// Path to CSV file
        path: PathBuf,
        /// Optional delimiter override
        #[serde(default = "default_delimiter_char")]
        delimiter: char,
        /// Optional encoding override
        #[serde(default = "default_encoding")]
        encoding: String,
    },
    // Future: Api, Database, etc.
}

fn default_delimiter_char() -> char {
    ','
}

fn default_encoding() -> String {
    "utf-8".to_string()
}

/// Dimension schema definition
#[derive(Debug, Clone, Deserialize)]
pub struct DimensionSchema {
    /// Field definitions
    pub fields: Vec<DimensionField>,
}

/// Field definition within dimension schema
#[derive(Debug, Clone, Deserialize)]
pub struct DimensionField {
    /// Column name
    pub name: String,

    /// Data type for Silver table
    pub data_type: DimensionDataType,

    /// Whether field is required (non-nullable)
    #[serde(default)]
    pub required: bool,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Data types for dimension fields
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DimensionDataType {
    Text,
    Integer,
    Float,
    Boolean,
    Timestamp,
    Json,
}

impl DimensionDataType {
    /// Convert to PostgreSQL type string
    pub fn to_pg_type(&self) -> &'static str {
        match self {
            DimensionDataType::Text => "TEXT",
            DimensionDataType::Integer => "BIGINT",
            DimensionDataType::Float => "DOUBLE PRECISION",
            DimensionDataType::Boolean => "BOOLEAN",
            DimensionDataType::Timestamp => "TIMESTAMPTZ",
            DimensionDataType::Json => "JSONB",
        }
    }
}

/// Load strategy configuration
#[derive(Debug, Clone, Deserialize)]
pub struct LoadConfig {
    /// Load strategy
    #[serde(default)]
    pub strategy: LoadStrategy,
}

impl Default for LoadConfig {
    fn default() -> Self {
        Self {
            strategy: LoadStrategy::default(),
        }
    }
}

/// Load strategy for dimension updates
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LoadStrategy {
    /// Delete all existing data, insert new (default for dimensions)
    #[default]
    TruncateAndLoad,
    /// Insert new rows, update existing based on primary_key
    Upsert,
}
```

---

## 3. Error Types

Following NDP's `thiserror` pattern from `core/src/error.rs`.

```rust
// core/src/sources/csv.rs (or core/src/error.rs extension)

use thiserror::Error;

/// CSV-specific errors with context
#[derive(Debug, Error)]
pub enum CsvError {
    /// CSV parsing error with line number
    #[error("CSV parse error at line {line}: {message}")]
    Parse {
        line: usize,
        message: String,
    },

    /// Required field missing
    #[error("Missing required field '{field}' at line {line}")]
    MissingField {
        line: usize,
        field: String,
    },

    /// Type conversion error
    #[error("Type conversion failed for field '{field}' at line {line}: {message}")]
    TypeConversion {
        line: usize,
        field: String,
        message: String,
    },

    /// File I/O error
    #[error("File error for '{path}': {message}")]
    FileError {
        path: String,
        message: String,
    },

    /// Validation error
    #[error("Validation error: {0}")]
    Validation(String),
}

impl From<CsvError> for CoreError {
    fn from(err: CsvError) -> Self {
        match err {
            CsvError::Parse { .. } | CsvError::TypeConversion { .. } => {
                CoreError::Parser(err.to_string())
            }
            CsvError::MissingField { .. } | CsvError::Validation(_) => {
                CoreError::Validation(err.to_string())
            }
            CsvError::FileError { .. } => {
                CoreError::Source(err.to_string())
            }
        }
    }
}

/// Dimension-specific errors
#[derive(Debug, Error)]
pub enum DimensionError {
    /// Schema validation failed
    #[error("Schema validation failed for dimension '{dimension_id}': {message}")]
    SchemaValidation {
        dimension_id: String,
        message: String,
    },

    /// Load operation failed
    #[error("Load failed for dimension '{dimension_id}': {message}")]
    LoadFailed {
        dimension_id: String,
        message: String,
    },

    /// Table creation failed
    #[error("Failed to create table '{table}': {message}")]
    TableCreation {
        table: String,
        message: String,
    },
}

impl From<DimensionError> for CoreError {
    fn from(err: DimensionError) -> Self {
        CoreError::DatabaseError(err.to_string())
    }
}
```

---

## 4. Module Structure

Proposed locations within `/core` following existing patterns:

```
core/
└── src/
    ├── sources/
    │   ├── mod.rs              # Add: pub mod csv; pub use csv::*;
    │   ├── csv.rs              # NEW: CsvSource, CsvSourceConfig
    │   ├── http_poll.rs        # Existing
    │   └── mqtt/               # Existing
    │
    ├── dimensions/
    │   ├── mod.rs              # NEW: Module exports
    │   ├── config.rs           # NEW: DimensionConfig structs
    │   ├── loader.rs           # NEW: DimensionLoader trait + CsvDimensionLoader
    │   └── schema.rs           # NEW: DDL generation for auto-creating tables
    │
    ├── types/
    │   └── stream_config.rs    # UPDATE: Add SourceType::Csv variant
    │
    └── error.rs                # UPDATE: Add CsvError, DimensionError
```

**Rationale:**
- CSV source goes in `sources/` alongside HTTP and MQTT adapters
- Dimensions are a new concept - separate module for clarity
- Both follow the Domain Adapter pattern (trait + implementation)

---

## 5. Key Dependencies

Add to `core/Cargo.toml`:

```toml
[dependencies]
# Existing
async-trait = "0.1"
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "1.0"
tokio = { version = "1", features = ["fs", "io-util"] }
tracing = "0.1"
sqlx = { version = "0.8", features = ["runtime-tokio-rustls", "postgres"] }

# NEW for CSV support
csv-async = { version = "1.2", features = ["tokio"] }
```

**Why csv-async?**
- Async/await compatible for tokio runtime
- Streaming parser - doesn't load entire file into memory
- Handles common edge cases (quotes, escapes, different delimiters)

---

## 6. Code Patterns

### 6.1 Streaming CSV Parsing (Memory Efficiency)

**Pattern:** Process rows as a stream rather than loading entire file.

```rust
/// Stream-based CSV processing for large files
///
/// Uses AsyncReaderBuilder for memory-efficient processing.
/// Each row is processed and forwarded immediately rather than
/// accumulating all rows in memory.
pub async fn process_csv_stream<F, Fut>(
    path: &PathBuf,
    delimiter: u8,
    mut processor: F,
) -> CoreResult<usize>
where
    F: FnMut(csv_async::StringRecord, usize) -> Fut,
    Fut: std::future::Future<Output = CoreResult<()>>,
{
    let file = File::open(path).await?;
    let reader = BufReader::new(file);
    let mut csv_reader = csv_async::AsyncReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(true)
        .create_reader(reader);

    let mut line_number = 1;
    let mut processed = 0;

    let mut records = csv_reader.records();
    while let Some(result) = records.next().await {
        line_number += 1;
        let record = result.map_err(|e| CsvError::Parse {
            line: line_number,
            message: e.to_string(),
        })?;

        processor(record, line_number).await?;
        processed += 1;
    }

    Ok(processed)
}
```

### 6.2 Batch Inserts for Dimension Loading

**Pattern:** Use batched inserts within a transaction for efficient dimension loading.

```rust
impl CsvDimensionLoader {
    /// Batch insert rows into target table
    ///
    /// Uses PostgreSQL COPY for large batches or multi-value INSERT for smaller ones.
    async fn batch_insert(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        rows: &[serde_json::Map<String, serde_json::Value>],
    ) -> CoreResult<usize> {
        if rows.is_empty() {
            return Ok(0);
        }

        // Build column list from schema
        let columns: Vec<&str> = self.config.schema.fields
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        let column_list = columns.join(", ");
        let placeholders: Vec<String> = (1..=columns.len())
            .map(|i| format!("${}", i))
            .collect();

        // Batch size for multi-value INSERT
        const BATCH_SIZE: usize = 100;
        let mut inserted = 0;

        for chunk in rows.chunks(BATCH_SIZE) {
            // Build multi-value INSERT
            let values_clauses: Vec<String> = chunk.iter().enumerate()
                .map(|(row_idx, _)| {
                    let start = row_idx * columns.len() + 1;
                    let row_placeholders: Vec<String> = (start..start + columns.len())
                        .map(|i| format!("${}", i))
                        .collect();
                    format!("({})", row_placeholders.join(", "))
                })
                .collect();

            let sql = format!(
                "INSERT INTO {} ({}) VALUES {}",
                &self.config.target.table,
                column_list,
                values_clauses.join(", ")
            );

            let mut query = sqlx::query(&sql);

            // Bind all values
            for row in chunk {
                for col in &columns {
                    let value = row.get(*col);
                    query = bind_json_value(query, value);
                }
            }

            query.execute(&mut **tx).await.map_err(|e| {
                CoreError::DatabaseError(format!("Insert failed: {}", e))
            })?;

            inserted += chunk.len();
        }

        Ok(inserted)
    }

    /// Batch upsert rows (INSERT ON CONFLICT UPDATE)
    async fn batch_upsert(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        rows: &[serde_json::Map<String, serde_json::Value>],
    ) -> CoreResult<usize> {
        if rows.is_empty() {
            return Ok(0);
        }

        let columns: Vec<&str> = self.config.schema.fields
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        let column_list = columns.join(", ");
        let pk_columns = self.config.target.primary_key.join(", ");

        // Build excluded columns for UPDATE
        let update_columns: Vec<String> = columns.iter()
            .filter(|c| !self.config.target.primary_key.contains(&c.to_string()))
            .map(|c| format!("{} = EXCLUDED.{}", c, c))
            .collect();

        let mut upserted = 0;

        for row in rows {
            let placeholders: Vec<String> = (1..=columns.len())
                .map(|i| format!("${}", i))
                .collect();

            let sql = format!(
                "INSERT INTO {} ({}) VALUES ({}) \
                 ON CONFLICT ({}) DO UPDATE SET {}",
                &self.config.target.table,
                column_list,
                placeholders.join(", "),
                pk_columns,
                update_columns.join(", ")
            );

            let mut query = sqlx::query(&sql);
            for col in &columns {
                let value = row.get(*col);
                query = bind_json_value(query, value);
            }

            query.execute(&mut **tx).await.map_err(|e| {
                CoreError::DatabaseError(format!("Upsert failed: {}", e))
            })?;

            upserted += 1;
        }

        Ok(upserted)
    }
}

/// Bind JSON value to sqlx query with type conversion
fn bind_json_value<'q>(
    query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    value: Option<&serde_json::Value>,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    match value {
        Some(serde_json::Value::String(s)) => query.bind(s.clone()),
        Some(serde_json::Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                query.bind(i)
            } else if let Some(f) = n.as_f64() {
                query.bind(f)
            } else {
                query.bind(Option::<String>::None)
            }
        }
        Some(serde_json::Value::Bool(b)) => query.bind(*b),
        Some(serde_json::Value::Null) | None => query.bind(Option::<String>::None),
        Some(other) => query.bind(other.to_string()),
    }
}
```

### 6.3 Transaction Handling

**Pattern:** All dimension loads must be atomic - use PostgreSQL transactions.

```rust
/// Execute dimension load within a transaction
///
/// Ensures atomicity: either all rows load or none do.
/// On error, transaction rolls back automatically.
pub async fn load_with_transaction(
    pool: &PgPool,
    loader: &dyn DimensionLoader,
) -> CoreResult<DimensionLoadStats> {
    // Begin transaction
    let mut tx = pool.begin().await.map_err(|e| {
        CoreError::DatabaseError(format!("Failed to begin transaction: {}", e))
    })?;

    // Perform load operations
    let stats = match loader.load_within_tx(&mut tx).await {
        Ok(stats) => stats,
        Err(e) => {
            // Transaction will auto-rollback on drop
            tracing::error!(
                dimension_id = %loader.dimension_id(),
                error = %e,
                "Dimension load failed, rolling back"
            );
            return Err(e);
        }
    };

    // Commit on success
    tx.commit().await.map_err(|e| {
        CoreError::DatabaseError(format!("Failed to commit transaction: {}", e))
    })?;

    tracing::info!(
        dimension_id = %loader.dimension_id(),
        rows_loaded = stats.rows_loaded,
        duration_ms = stats.duration_ms,
        "Dimension load committed"
    );

    Ok(stats)
}
```

---

## 7. Integration with Existing Patterns

### 7.1 Source Type Extension

Update `SourceType` enum in `core/src/types/stream_config.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Mqtt,
    HttpPoll,
    Webhook,
    FileWatch,
    Csv,  // NEW
}
```

Update `source_type_suffix` in `core/src/sources/mod.rs`:

```rust
fn source_type_suffix(source_type: &SourceType) -> &'static str {
    match source_type {
        SourceType::HttpPoll => "Http",
        SourceType::Mqtt => "Mqtt",
        SourceType::Webhook => "Webhook",
        SourceType::FileWatch => "FileWatch",
        SourceType::Csv => "Csv",  // NEW
    }
}
```

### 7.2 Coordinator Integration

CSV sources are triggered manually (not polling), so they integrate differently:

```rust
// apps/air-quality-app/src/coordinator/source_manager.rs

impl SourceManager {
    /// Trigger one-time CSV ingest for a stream
    pub async fn ingest_csv(
        &self,
        stream_id: &str,
        config: &CsvSourceConfig,
    ) -> CoreResult<usize> {
        let source = CsvSource::new(
            stream_id,
            config.clone(),
            None, // ndp_id from source config if present
            None, // context from source config if present
        );

        let points = source.fetch_raw_batch().await?;
        let count = points.len();

        // Send to Bronze storage via existing channel
        self.raw_store.write_raw_batch(points).await?;

        Ok(count)
    }
}
```

---

## 8. Testing Approach

Following NDP's London School TDD with mockall:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[tokio::test]
    async fn test_csv_source_parses_basic_file() {
        // Create temporary CSV
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "timestamp,value,sensor_id").unwrap();
        writeln!(file, "2024-01-15T10:00:00Z,23.5,sensor-001").unwrap();
        writeln!(file, "2024-01-15T10:01:00Z,24.0,sensor-001").unwrap();

        let config = CsvSourceConfig {
            path: file.path().to_path_buf(),
            timestamp_field: "timestamp".to_string(),
            ..Default::default()
        };

        let source = CsvSource::new("test-stream", config, None, None);
        let points = source.fetch_raw_batch().await.unwrap();

        assert_eq!(points.len(), 2);
        assert_eq!(points[0].source_id, "test-stream-Csv");
        assert_eq!(points[0].raw_payload["value"], "23.5");
    }

    #[tokio::test]
    async fn test_csv_source_skips_invalid_rows() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "timestamp,value").unwrap();
        writeln!(file, "2024-01-15T10:00:00Z,23.5").unwrap();
        writeln!(file, "invalid-timestamp,24.0").unwrap(); // Invalid
        writeln!(file, "2024-01-15T10:02:00Z,25.0").unwrap();

        let config = CsvSourceConfig {
            path: file.path().to_path_buf(),
            timestamp_field: "timestamp".to_string(),
            on_error: OnError::Skip,
            ..Default::default()
        };

        let source = CsvSource::new("test-stream", config, None, None);
        let points = source.fetch_raw_batch().await.unwrap();

        assert_eq!(points.len(), 2); // Skipped invalid row
    }

    #[tokio::test]
    async fn test_csv_source_aborts_on_error() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "timestamp,value").unwrap();
        writeln!(file, "invalid-timestamp,23.5").unwrap();

        let config = CsvSourceConfig {
            path: file.path().to_path_buf(),
            timestamp_field: "timestamp".to_string(),
            on_error: OnError::Abort,
            ..Default::default()
        };

        let source = CsvSource::new("test-stream", config, None, None);
        let result = source.fetch_raw_batch().await;

        assert!(result.is_err());
    }
}
```

---

## References

- **Patterns Retrieved:**
  - `arch-dual-trait-source`: RawSource trait for Bronze layer
  - `arch-domain-adapter-pattern`: Hexagonal architecture
  - `sample-rust-error-handling`: thiserror pattern
  - `config-stream-files`: Stream config structure

- **Existing Code:**
  - `/core/src/traits.rs`: RawSource trait definition
  - `/core/src/error.rs`: CoreError enum
  - `/core/src/sources/http_poll.rs`: HTTP source pattern
  - `/core/src/types/stream_config.rs`: SourceType enum

- **Feature Scope:**
  - `/product/features/dp-013/SCOPE.md`
