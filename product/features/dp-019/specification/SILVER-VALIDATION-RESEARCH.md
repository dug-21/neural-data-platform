# dp-019: Silver Table Validation Research

## Executive Summary

This document provides comprehensive research on Silver table validation for the dp-019 Config Validation Pipeline. It covers how to validate that:
1. Silver tables exist in TimescaleDB
2. Table schemas match configuration
3. Type mappings are compatible
4. Hypertable configuration is correct
5. Database connections are properly managed

---

## 1. Silver Table Discovery

### 1.1 Checking Table Existence

**SQL Query to Validate Table Exists:**

```sql
-- Check if table exists in silver schema
SELECT EXISTS (
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = 'silver' AND table_name = $1
) AS exists;
```

**Example (from `timescale_silver.rs` lines 309-323):**

```rust
let exists_query = r#"
    SELECT EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'silver' AND table_name = $1
    ) AS exists
"#;
let exists_row = conn.query_one(exists_query, &[&table_name]).await?;
let exists: bool = exists_row.get("exists");
```

### 1.2 List All Silver Tables

**SQL Query (from `timescale_silver.rs` lines 212-266):**

```sql
WITH hypertable_info AS (
    SELECT
        ht.hypertable_name AS table_name,
        TRUE AS is_hypertable,
        d.column_name AS time_column,
        COALESCE(ht.compression_enabled, FALSE) AS compression_enabled,
        (SELECT COUNT(*)
         FROM timescaledb_information.chunks c
         WHERE c.hypertable_schema = 'silver'
           AND c.hypertable_name = ht.hypertable_name) AS chunk_count,
        hypertable_size(format('silver.%I', ht.hypertable_name)::regclass) AS total_bytes
    FROM timescaledb_information.hypertables ht
    LEFT JOIN timescaledb_information.dimensions d
        ON ht.hypertable_schema = d.hypertable_schema
        AND ht.hypertable_name = d.hypertable_name
        AND d.dimension_number = 1
    WHERE ht.hypertable_schema = 'silver'
)
SELECT table_name, is_hypertable, time_column, compression_enabled, chunk_count
FROM hypertable_info;
```

### 1.3 Validator Implementation Pattern

```rust
/// Check if a Silver table exists
pub async fn table_exists(
    conn: &tokio_postgres::Client,
    table_name: &str,
) -> Result<bool, ValidationError> {
    // Normalize: strip "silver." prefix if present
    let table_name = table_name.strip_prefix("silver.").unwrap_or(table_name);

    let query = r#"
        SELECT EXISTS (
            SELECT 1 FROM information_schema.tables
            WHERE table_schema = 'silver' AND table_name = $1
        ) AS exists
    "#;

    let row = conn.query_one(query, &[&table_name]).await
        .map_err(|e| ValidationError::DatabaseError(e.to_string()))?;

    Ok(row.get("exists"))
}
```

---

## 2. Table Schema Validation

### 2.1 Get Table Schema from TimescaleDB

**SQL Query for Column Information:**

```sql
-- From information_schema (standard PostgreSQL)
SELECT
    column_name,
    UPPER(udt_name) AS data_type,
    (is_nullable = 'YES') AS nullable,
    ordinal_position
FROM information_schema.columns
WHERE table_schema = 'silver' AND table_name = $1
ORDER BY ordinal_position;
```

**SQL Query with Data Dictionary (preferred, includes metadata):**

```sql
-- From data_dictionary (if populated)
SELECT
    c.column_name,
    UPPER(c.data_type) AS data_type,
    c.unit,
    c.description,
    c.nullable,
    c.is_primary_key,
    c.sort_order
FROM data_dictionary.silver_columns c
WHERE c.table_name = $1 OR c.table_name = $2  -- support both 'table' and 'silver.table'
ORDER BY c.sort_order, c.column_name;
```

### 2.2 Compare Config Columns to Table Columns

```rust
/// Validate that config field_mappings match table columns
pub async fn validate_schema_match(
    conn: &tokio_postgres::Client,
    config: &SilverEtlConfig,
) -> Result<Vec<SchemaValidationError>, ValidationError> {
    let table_name = config.target_table
        .strip_prefix("silver.")
        .unwrap_or(&config.target_table);

    // Get actual table columns
    let query = r#"
        SELECT column_name, udt_name as data_type
        FROM information_schema.columns
        WHERE table_schema = 'silver' AND table_name = $1
    "#;

    let rows = conn.query(query, &[&table_name]).await?;
    let table_columns: HashMap<String, String> = rows.iter()
        .map(|r| (r.get::<_, String>("column_name"), r.get::<_, String>("data_type")))
        .collect();

    let mut errors = Vec::new();

    // Check each field_mapping has a corresponding column
    for mapping in &config.field_mappings {
        match table_columns.get(&mapping.target_column) {
            None => {
                errors.push(SchemaValidationError::MissingColumn {
                    column: mapping.target_column.clone(),
                    table: config.target_table.clone(),
                });
            }
            Some(actual_type) => {
                // Validate type compatibility
                if !is_type_compatible(&mapping.column_type, actual_type) {
                    errors.push(SchemaValidationError::TypeMismatch {
                        column: mapping.target_column.clone(),
                        expected: mapping.column_type.clone(),
                        actual: actual_type.clone(),
                    });
                }
            }
        }
    }

    // Check identity fields
    for identity in &config.identity_fields {
        if !table_columns.contains_key(&identity.target) {
            errors.push(SchemaValidationError::MissingColumn {
                column: identity.target.clone(),
                table: config.target_table.clone(),
            });
        }
    }

    // Check timestamp column
    if !table_columns.contains_key(&config.timestamp.target_field) {
        errors.push(SchemaValidationError::MissingColumn {
            column: config.timestamp.target_field.clone(),
            table: config.target_table.clone(),
        });
    }

    Ok(errors)
}
```

---

## 3. Type Mapping: Config to PostgreSQL

### 3.1 Valid Config Types

**From `silver_etl.rs` (lines 251-263):**

```rust
const VALID_TYPES: &[&str] = &[
    "double_precision",
    "real",
    "integer",
    "bigint",
    "smallint",
    "text",
    "varchar",
    "boolean",
    "timestamptz",
    "jsonb",
    "text[]",
];
```

### 3.2 Config Type to PostgreSQL Type Mapping

| Config Type | PostgreSQL udt_name | Notes |
|-------------|---------------------|-------|
| `double_precision` | `float8` | 8-byte floating point |
| `real` | `float4` | 4-byte floating point |
| `integer` | `int4` | 4-byte signed integer |
| `bigint` | `int8` | 8-byte signed integer |
| `smallint` | `int2` | 2-byte signed integer |
| `text` | `text` | Variable unlimited length |
| `varchar` | `varchar` | Variable with limit |
| `boolean` | `bool` | true/false |
| `timestamptz` | `timestamptz` | Timestamp with timezone |
| `jsonb` | `jsonb` | Binary JSON |
| `text[]` | `_text` | Text array |

### 3.3 Type Compatibility Check Function

```rust
/// Check if config type is compatible with PostgreSQL type
fn is_type_compatible(config_type: &str, pg_type: &str) -> bool {
    // Normalize to lowercase for comparison
    let config_type = config_type.to_lowercase();
    let pg_type = pg_type.to_lowercase();

    match config_type.as_str() {
        "double_precision" => pg_type == "float8" || pg_type == "double precision",
        "real" => pg_type == "float4" || pg_type == "real",
        "integer" => pg_type == "int4" || pg_type == "integer",
        "bigint" => pg_type == "int8" || pg_type == "bigint",
        "smallint" => pg_type == "int2" || pg_type == "smallint",
        "text" => pg_type == "text",
        "varchar" => pg_type == "varchar" || pg_type.starts_with("varchar("),
        "boolean" => pg_type == "bool" || pg_type == "boolean",
        "timestamptz" => pg_type == "timestamptz" || pg_type == "timestamp with time zone",
        "jsonb" => pg_type == "jsonb",
        "text[]" => pg_type == "_text" || pg_type == "text[]",
        _ => false, // Unknown config type
    }
}
```

### 3.4 Expanded Type Mapping (for dp-020 DDL Generation)

| JSON Config Type | PostgreSQL DDL | Information Schema udt_name |
|------------------|----------------|----------------------------|
| `string` | `TEXT` | `text` |
| `float` | `DOUBLE PRECISION` | `float8` |
| `integer` | `INTEGER` | `int4` |
| `bigint` | `BIGINT` | `int8` |
| `boolean` | `BOOLEAN` | `bool` |
| `timestamp` | `TIMESTAMPTZ` | `timestamptz` |
| `json` | `JSONB` | `jsonb` |

---

## 4. Hypertable Validation

### 4.1 Check if Table is a Hypertable

```sql
SELECT EXISTS (
    SELECT 1 FROM timescaledb_information.hypertables
    WHERE hypertable_schema = 'silver' AND hypertable_name = $1
) AS is_hypertable;
```

### 4.2 Get Hypertable Configuration

```sql
SELECT
    d.column_name AS time_column,
    ht.compression_enabled,
    (SELECT COUNT(*) FROM timescaledb_information.chunks c
     WHERE c.hypertable_schema = 'silver' AND c.hypertable_name = $1) AS chunk_count,
    hypertable_size(format('silver.%I', $1)::regclass) AS total_bytes
FROM timescaledb_information.hypertables ht
JOIN timescaledb_information.dimensions d
    ON ht.hypertable_schema = d.hypertable_schema
    AND ht.hypertable_name = d.hypertable_name
    AND d.dimension_number = 1
WHERE ht.hypertable_schema = 'silver' AND ht.hypertable_name = $1;
```

### 4.3 Validate Hypertable Time Column

```rust
/// Validate that config timestamp field matches hypertable time column
pub async fn validate_hypertable_config(
    conn: &tokio_postgres::Client,
    config: &SilverEtlConfig,
) -> Result<Option<HypertableValidationError>, ValidationError> {
    let table_name = config.target_table
        .strip_prefix("silver.")
        .unwrap_or(&config.target_table);

    let query = r#"
        SELECT d.column_name AS time_column
        FROM timescaledb_information.hypertables ht
        JOIN timescaledb_information.dimensions d
            ON ht.hypertable_schema = d.hypertable_schema
            AND ht.hypertable_name = d.hypertable_name
            AND d.dimension_number = 1
        WHERE ht.hypertable_schema = 'silver' AND ht.hypertable_name = $1
    "#;

    match conn.query_opt(query, &[&table_name]).await? {
        Some(row) => {
            let time_column: String = row.get("time_column");

            // Check if config timestamp matches hypertable time column
            if time_column != config.timestamp.target_field {
                return Ok(Some(HypertableValidationError::TimeColumnMismatch {
                    config_column: config.timestamp.target_field.clone(),
                    hypertable_column: time_column,
                }));
            }
            Ok(None)
        }
        None => {
            // Not a hypertable - might be intentional
            Ok(Some(HypertableValidationError::NotAHypertable {
                table: config.target_table.clone(),
            }))
        }
    }
}
```

---

## 5. Connection Management

### 5.1 Current Connection Pattern

**From `timescale.rs` (lines 127-148):**

```rust
use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;
use tokio_postgres::NoTls;

type PgPool = Pool<PostgresConnectionManager<NoTls>>;

pub struct TimescaleOutput {
    config: TimescaleConfig,
    pool: PgPool,
}

impl TimescaleOutput {
    pub async fn new(config: TimescaleConfig) -> Result<Self, SilverOutputError> {
        let manager = PostgresConnectionManager::new_from_stringlike(
            &config.connection_string, NoTls
        )?;

        let pool = Pool::builder()
            .max_size(config.max_connections)
            .connection_timeout(Duration::from_secs(config.connection_timeout_secs))
            .build(manager)
            .await?;

        Ok(Self { config, pool })
    }
}
```

### 5.2 Environment Variables for Connection

| Variable | Purpose | Example |
|----------|---------|---------|
| `NDP_TIMESCALE_URL` | Full connection string | `postgresql://user:pass@localhost:5432/ndp` |
| `TIMESCALE_HOST` | Database host | `localhost` |
| `TIMESCALE_PORT` | Database port | `5432` |
| `TIMESCALE_USER` | Database user | `ndp` |
| `TIMESCALE_PASSWORD` | Database password | `secret` |
| `TIMESCALE_DB` | Database name | `neural_data` |

### 5.3 Connection String Construction

```rust
fn build_connection_string() -> Option<String> {
    // Try full connection string first
    if let Ok(url) = std::env::var("NDP_TIMESCALE_URL") {
        return Some(url);
    }

    // Build from components
    let host = std::env::var("TIMESCALE_HOST").ok()?;
    let port = std::env::var("TIMESCALE_PORT").unwrap_or_else(|_| "5432".to_string());
    let user = std::env::var("TIMESCALE_USER").ok()?;
    let password = std::env::var("TIMESCALE_PASSWORD").ok()?;
    let db = std::env::var("TIMESCALE_DB").ok()?;

    Some(format!("postgresql://{}:{}@{}:{}/{}", user, password, host, port, db))
}
```

### 5.4 Graceful Handling When DB Unavailable

```rust
/// Validator that handles missing database gracefully
pub struct SilverValidator {
    pool: Option<PgPool>,
}

impl SilverValidator {
    pub async fn new() -> Self {
        let pool = match build_connection_string() {
            Some(conn_str) => {
                match create_pool(&conn_str).await {
                    Ok(pool) => Some(pool),
                    Err(e) => {
                        tracing::warn!("TimescaleDB unavailable: {}. Silver table validation disabled.", e);
                        None
                    }
                }
            }
            None => {
                tracing::info!("No TimescaleDB connection configured. Silver table validation disabled.");
                None
            }
        };

        Self { pool }
    }

    pub async fn validate_table_exists(&self, table_name: &str) -> ValidationResult {
        match &self.pool {
            Some(pool) => {
                let conn = pool.get().await?;
                // ... actual validation
            }
            None => {
                ValidationResult::Skipped {
                    reason: "Database connection not available".to_string(),
                }
            }
        }
    }
}
```

---

## 6. Index Validation

### 6.1 Check Expected Indexes

**Standard Silver Table Indexes:**

```sql
-- Primary key index (automatic)
-- (observation_time, ndp_id)

-- Common query patterns
CREATE INDEX idx_{table}_ndp_time ON silver.{table} (ndp_id, observation_time DESC);
CREATE INDEX idx_{table}_stream ON silver.{table} (source_stream, observation_time DESC);
```

**SQL to List Indexes:**

```sql
SELECT
    i.relname AS index_name,
    a.attname AS column_name,
    ix.indisunique AS is_unique,
    ix.indisprimary AS is_primary
FROM pg_class t
JOIN pg_index ix ON t.oid = ix.indrelid
JOIN pg_class i ON i.oid = ix.indexrelid
JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY(ix.indkey)
JOIN pg_namespace n ON n.oid = t.relnamespace
WHERE n.nspname = 'silver' AND t.relname = $1
ORDER BY i.relname, a.attnum;
```

### 6.2 Validate Deduplication Key Indexes

```rust
/// Validate that deduplication key columns are indexed
pub async fn validate_dedup_indexes(
    conn: &tokio_postgres::Client,
    config: &SilverEtlConfig,
) -> Result<Vec<IndexValidationWarning>, ValidationError> {
    if !config.deduplication.enabled {
        return Ok(vec![]);
    }

    let table_name = config.target_table
        .strip_prefix("silver.")
        .unwrap_or(&config.target_table);

    // Get indexed columns
    let query = r#"
        SELECT DISTINCT a.attname AS column_name
        FROM pg_class t
        JOIN pg_index ix ON t.oid = ix.indrelid
        JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY(ix.indkey)
        JOIN pg_namespace n ON n.oid = t.relnamespace
        WHERE n.nspname = 'silver' AND t.relname = $1
    "#;

    let rows = conn.query(query, &[&table_name]).await?;
    let indexed_columns: HashSet<String> = rows.iter()
        .map(|r| r.get::<_, String>("column_name"))
        .collect();

    let mut warnings = Vec::new();
    for key_col in &config.deduplication.key_columns {
        if !indexed_columns.contains(key_col) {
            warnings.push(IndexValidationWarning::MissingIndex {
                column: key_col.clone(),
                reason: "Deduplication key column not indexed".to_string(),
            });
        }
    }

    Ok(warnings)
}
```

---

## 7. Complete Validator Implementation

### 7.1 Validation Error Types

```rust
#[derive(Debug, Clone, Serialize)]
pub enum SilverValidationError {
    // Table errors
    TableNotFound { table: String },
    NotAHypertable { table: String },

    // Schema errors
    MissingColumn { table: String, column: String },
    TypeMismatch { column: String, expected: String, actual: String },

    // Hypertable errors
    TimeColumnMismatch { config_column: String, hypertable_column: String },

    // Connection errors
    DatabaseUnavailable { reason: String },

    // Index warnings (not errors, but warnings)
    MissingRecommendedIndex { column: String, reason: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct SilverValidationResult {
    pub table: String,
    pub exists: bool,
    pub is_hypertable: bool,
    pub errors: Vec<SilverValidationError>,
    pub warnings: Vec<String>,
    pub skipped: bool,
    pub skip_reason: Option<String>,
}
```

### 7.2 Full Validator

```rust
pub struct SilverTableValidator {
    pool: Option<PgPool>,
}

impl SilverTableValidator {
    pub async fn new() -> Self {
        let pool = Self::try_connect().await;
        Self { pool }
    }

    async fn try_connect() -> Option<PgPool> {
        let conn_str = std::env::var("NDP_TIMESCALE_URL").ok()?;

        let manager = PostgresConnectionManager::new_from_stringlike(&conn_str, NoTls).ok()?;
        Pool::builder()
            .max_size(2)  // Low for validation
            .connection_timeout(Duration::from_secs(5))
            .build(manager)
            .await
            .ok()
    }

    pub fn is_connected(&self) -> bool {
        self.pool.is_some()
    }

    pub async fn validate(&self, config: &SilverEtlConfig) -> SilverValidationResult {
        let table = config.target_table.clone();

        // Handle no connection
        let pool = match &self.pool {
            Some(p) => p,
            None => {
                return SilverValidationResult {
                    table,
                    exists: false,
                    is_hypertable: false,
                    errors: vec![],
                    warnings: vec![],
                    skipped: true,
                    skip_reason: Some("Database connection not available".to_string()),
                };
            }
        };

        // Get connection
        let conn = match pool.get().await {
            Ok(c) => c,
            Err(e) => {
                return SilverValidationResult {
                    table,
                    exists: false,
                    is_hypertable: false,
                    errors: vec![SilverValidationError::DatabaseUnavailable {
                        reason: e.to_string(),
                    }],
                    warnings: vec![],
                    skipped: false,
                    skip_reason: None,
                };
            }
        };

        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // 1. Check table exists
        let table_name = table.strip_prefix("silver.").unwrap_or(&table);
        let exists = self.table_exists(&conn, table_name).await.unwrap_or(false);

        if !exists {
            return SilverValidationResult {
                table,
                exists: false,
                is_hypertable: false,
                errors: vec![SilverValidationError::TableNotFound { table: table.clone() }],
                warnings: vec![],
                skipped: false,
                skip_reason: None,
            };
        }

        // 2. Check is hypertable
        let is_hypertable = self.is_hypertable(&conn, table_name).await.unwrap_or(false);
        if !is_hypertable {
            warnings.push(format!("Table {} is not a hypertable", table));
        }

        // 3. Validate schema match
        if let Ok(schema_errors) = self.validate_schema(&conn, config).await {
            errors.extend(schema_errors);
        }

        // 4. Validate hypertable time column
        if is_hypertable {
            if let Ok(Some(err)) = self.validate_time_column(&conn, config).await {
                errors.push(err);
            }
        }

        SilverValidationResult {
            table,
            exists,
            is_hypertable,
            errors,
            warnings,
            skipped: false,
            skip_reason: None,
        }
    }

    // ... helper methods
}
```

---

## 8. Integration with dp-019 Validator

### 8.1 Semantic Validation Layer

The Silver table validation fits into dp-019's two-layer validation:

1. **Schema Validation (Layer 1)**: JSON Schema checks
   - `target_table` format (must start with `silver.`)
   - `field_mappings[].type` is a valid type
   - Required fields present

2. **Semantic Validation (Layer 2)**: Silver table checks
   - Table exists in TimescaleDB
   - Columns match config
   - Types are compatible
   - Hypertable configuration is correct

### 8.2 CLI Integration

```bash
# Schema validation only (fast, no DB)
ndp-validate --schema-only config.json

# Full validation with Silver table checks
ndp-validate --check-tables config.json

# Skip Silver validation when DB unavailable
ndp-validate --skip-silver-validation config.json
```

### 8.3 Validation Output

```json
{
  "valid": false,
  "errors": [
    {
      "layer": "semantic",
      "path": "$.silver_etl.target_table",
      "message": "Table 'silver.air_quality_readings' not found in database",
      "severity": "error",
      "code": "SILVER_TABLE_NOT_FOUND"
    },
    {
      "layer": "semantic",
      "path": "$.silver_etl.field_mappings[2].target_column",
      "message": "Column 'tvoc_reading' not found in table. Did you mean 'tvoc_index'?",
      "severity": "error",
      "code": "COLUMN_NOT_FOUND"
    }
  ],
  "warnings": [
    {
      "layer": "semantic",
      "path": "$.silver_etl.target_table",
      "message": "Table is not a hypertable. Consider running create_hypertable() for time-series performance.",
      "severity": "warning",
      "code": "NOT_HYPERTABLE"
    }
  ],
  "silver_validation": {
    "connected": true,
    "table_exists": false,
    "is_hypertable": false
  }
}
```

---

## 9. Key File References

| Component | File | Key Lines |
|-----------|------|-----------|
| TimescaleOutput | `core/src/silver/outputs/timescale.rs` | 114-148 |
| TimescaleSilverStorage | `core/ndp-mcp-server/src/storage/timescale_silver.rs` | 110-168, 304-328 |
| SilverEtlConfig | `core/src/config/silver_etl.rs` | 59-162 |
| Valid Column Types | `core/src/config/silver_etl.rs` | 251-263 |
| Silver Schema DDL | `deploy/timescaledb/init/001_silver_schema.sql` | 101-148 |
| list_silver_tables MCP | `core/ndp-mcp-server/src/mcp/tools/list_silver_tables.rs` | 93-122 |
| describe_silver_table MCP | `core/ndp-mcp-server/src/mcp/tools/describe_silver_table.rs` | Full |

---

## 10. Recommendations for dp-019

### 10.1 Implementation Priority

1. **Table Existence Check** (High Priority)
   - Catches the most critical error: missing tables
   - Simple SQL query
   - Clear error message

2. **Column Name Validation** (High Priority)
   - Catches typos in `target_column`
   - Prevents silent NULL values

3. **Type Compatibility Check** (Medium Priority)
   - Catches type mismatches
   - Requires type mapping table

4. **Hypertable Validation** (Low Priority)
   - Nice-to-have warning
   - Not a blocking error

### 10.2 Graceful Degradation

When TimescaleDB is unavailable:
- **Do not fail validation** - Schema validation can proceed
- **Report as skipped** - Clear indication in output
- **Log warning** - Operator knows Silver checks were skipped
- **Provide hint** - Suggest setting `NDP_TIMESCALE_URL`

### 10.3 Performance Considerations

- Use connection pool with small size (2 connections) for validation
- Cache schema metadata during batch validation
- Short connection timeout (5 seconds) for validation

---

*Research completed: 2026-02-02*
*Feature: dp-019 Config Validation Pipeline*
*Phase: Specification*
*Author: NDP TimescaleDB Developer Agent*
