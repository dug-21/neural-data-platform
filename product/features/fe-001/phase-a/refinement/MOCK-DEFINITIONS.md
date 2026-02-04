# Phase A: Mock Definitions for London TDD

> **Phase:** A (Architecture Foundation)
> **Purpose:** Define all mock implementations needed for Phase A unit tests
> **Testing Approach:** London School TDD (Mock Collaborators)
> **Created:** 2026-02-04

---

## Overview

This document defines all mock implementations required for Phase A testing. Following London TDD principles, we mock all collaborators (external dependencies) to isolate units under test.

**Key Principle**: Mocks should implement the same trait/interface as production code, allowing them to be swapped transparently.

---

## 1. MockConfigLoader (Existing)

**Location**: `core/src/config/mock_loader.rs`
**Status**: Already implemented in NDP codebase

### Interface

```rust
#[async_trait]
pub trait ConfigLoader: Send + Sync {
    async fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig, ConfigLoaderError>;
    async fn load_silver_etl_config(&self, stream_id: &str) -> Result<SilverEtlConfig, ConfigLoaderError>;
    async fn list_streams(&self) -> Result<Vec<String>, ConfigLoaderError>;
    async fn stream_exists(&self, stream_id: &str) -> Result<bool, ConfigLoaderError>;
    fn source_name(&self) -> &'static str;
}
```

### Builder Pattern Usage

```rust
use neural_core::config::{MockConfigLoader, ConfigLoaderError};

// Basic usage
let loader = MockConfigLoader::new()
    .with_stream(create_test_stream_config("air-quality"))
    .with_silver_config("air-quality", create_test_silver_config());

// With error simulation
let failing_loader = MockConfigLoader::new()
    .with_error(ConfigLoaderError::ConnectionError("etcd unreachable".into()));

// Multiple streams
let loader = MockConfigLoader::new()
    .with_streams(vec![
        create_test_stream_config("air-quality"),
        create_test_stream_config("outdoor-weather"),
        create_test_stream_config("home-assistant-state"),
    ]);
```

### Extension for Phase A: Gold ETL Config

The existing MockConfigLoader needs extension to support GoldEtlConfig:

```rust
// Extension to add in core/src/config/mock_loader.rs

impl MockConfigLoader {
    /// Add a Gold ETL config for testing (builder pattern)
    pub fn with_gold_config(self, stream_id: &str, config: GoldEtlConfig) -> Self {
        self.gold_configs
            .write()
            .unwrap()
            .insert(stream_id.to_string(), config);
        self
    }
}

// Add to ConfigLoader trait
#[async_trait]
pub trait ConfigLoader: Send + Sync {
    // ... existing methods ...

    async fn load_gold_etl_config(
        &self,
        stream_id: &str,
    ) -> Result<GoldEtlConfig, ConfigLoaderError>;
}
```

---

## 2. MockTimescaleDb (New)

**Location**: `tools/ndp-gold-ddl/src/mocks/timescale.rs`
**Purpose**: Mock TimescaleDB connections for testing DDL generation and execution

### Interface Definition

```rust
// tools/ndp-gold-ddl/src/db/connection.rs

use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum DbError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Query failed: {0}")]
    QueryFailed(String),

    #[error("Object already exists: {0}")]
    ObjectExists(String),

    #[error("Object not found: {0}")]
    ObjectNotFound(String),
}

#[async_trait]
pub trait TimescaleConnection: Send + Sync {
    /// Execute a SQL statement
    async fn execute(&self, sql: &str) -> Result<(), DbError>;

    /// Execute multiple SQL statements in a transaction
    async fn execute_batch(&self, statements: &[&str]) -> Result<(), DbError>;

    /// Check if a materialized view exists
    async fn view_exists(&self, schema: &str, name: &str) -> Result<bool, DbError>;

    /// Check if a continuous aggregate exists
    async fn continuous_aggregate_exists(&self, schema: &str, name: &str) -> Result<bool, DbError>;

    /// Get columns for a table/view
    async fn get_columns(&self, schema: &str, table: &str) -> Result<Vec<String>, DbError>;
}
```

### Mock Implementation

```rust
// tools/ndp-gold-ddl/src/mocks/timescale.rs

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;
use async_trait::async_trait;
use crate::db::connection::{TimescaleConnection, DbError};

/// Mock TimescaleDB connection for unit testing
pub struct MockTimescaleDb {
    /// Existing views (schema.name)
    views: RwLock<HashSet<String>>,

    /// Existing continuous aggregates (schema.name)
    continuous_aggregates: RwLock<HashSet<String>>,

    /// Column definitions by table (schema.table -> columns)
    columns: RwLock<HashMap<String, Vec<String>>>,

    /// Executed SQL statements (for verification)
    executed_sql: RwLock<Vec<String>>,

    /// Error to return for all operations (if set)
    should_fail: RwLock<Option<DbError>>,
}

impl Default for MockTimescaleDb {
    fn default() -> Self {
        Self::new()
    }
}

impl MockTimescaleDb {
    /// Create a new empty mock
    pub fn new() -> Self {
        Self {
            views: RwLock::new(HashSet::new()),
            continuous_aggregates: RwLock::new(HashSet::new()),
            columns: RwLock::new(HashMap::new()),
            executed_sql: RwLock::new(Vec::new()),
            should_fail: RwLock::new(None),
        }
    }

    /// Add an existing view (builder pattern)
    pub fn with_existing_view(self, schema: &str, name: &str) -> Self {
        self.views
            .write()
            .unwrap()
            .insert(format!("{}.{}", schema, name));
        self
    }

    /// Add an existing continuous aggregate (builder pattern)
    pub fn with_existing_continuous_aggregate(self, schema: &str, name: &str) -> Self {
        self.continuous_aggregates
            .write()
            .unwrap()
            .insert(format!("{}.{}", schema, name));
        self
    }

    /// Add columns for a table (builder pattern)
    pub fn with_table_columns(self, schema: &str, table: &str, columns: Vec<String>) -> Self {
        self.columns
            .write()
            .unwrap()
            .insert(format!("{}.{}", schema, table), columns);
        self
    }

    /// Configure mock to fail with specific error (builder pattern)
    pub fn with_error(self, error: DbError) -> Self {
        *self.should_fail.write().unwrap() = Some(error);
        self
    }

    /// Get all executed SQL statements (for test assertions)
    pub fn get_executed_sql(&self) -> Vec<String> {
        self.executed_sql.read().unwrap().clone()
    }

    /// Get the last executed SQL statement
    pub fn get_last_sql(&self) -> Option<String> {
        self.executed_sql.read().unwrap().last().cloned()
    }

    /// Clear executed SQL history
    pub fn clear_executed_sql(&self) {
        self.executed_sql.write().unwrap().clear();
    }

    /// Check if a specific SQL pattern was executed
    pub fn sql_contains(&self, pattern: &str) -> bool {
        self.executed_sql
            .read()
            .unwrap()
            .iter()
            .any(|sql| sql.contains(pattern))
    }
}

#[async_trait]
impl TimescaleConnection for MockTimescaleDb {
    async fn execute(&self, sql: &str) -> Result<(), DbError> {
        // Check if we should fail
        if let Some(ref err) = *self.should_fail.read().unwrap() {
            return Err(err.clone());
        }

        // Record the SQL
        self.executed_sql.write().unwrap().push(sql.to_string());

        // Parse SQL to update internal state
        if sql.contains("CREATE MATERIALIZED VIEW") {
            if let Some(view_name) = extract_view_name(sql) {
                self.views.write().unwrap().insert(view_name.clone());

                // If it's a continuous aggregate, track that too
                if sql.contains("timescaledb.continuous") {
                    self.continuous_aggregates.write().unwrap().insert(view_name);
                }
            }
        }

        if sql.contains("DROP MATERIALIZED VIEW") {
            if let Some(view_name) = extract_view_name(sql) {
                self.views.write().unwrap().remove(&view_name);
                self.continuous_aggregates.write().unwrap().remove(&view_name);
            }
        }

        Ok(())
    }

    async fn execute_batch(&self, statements: &[&str]) -> Result<(), DbError> {
        for sql in statements {
            self.execute(sql).await?;
        }
        Ok(())
    }

    async fn view_exists(&self, schema: &str, name: &str) -> Result<bool, DbError> {
        if let Some(ref err) = *self.should_fail.read().unwrap() {
            return Err(err.clone());
        }

        let full_name = format!("{}.{}", schema, name);
        Ok(self.views.read().unwrap().contains(&full_name))
    }

    async fn continuous_aggregate_exists(&self, schema: &str, name: &str) -> Result<bool, DbError> {
        if let Some(ref err) = *self.should_fail.read().unwrap() {
            return Err(err.clone());
        }

        let full_name = format!("{}.{}", schema, name);
        Ok(self.continuous_aggregates.read().unwrap().contains(&full_name))
    }

    async fn get_columns(&self, schema: &str, table: &str) -> Result<Vec<String>, DbError> {
        if let Some(ref err) = *self.should_fail.read().unwrap() {
            return Err(err.clone());
        }

        let full_name = format!("{}.{}", schema, table);
        self.columns
            .read()
            .unwrap()
            .get(&full_name)
            .cloned()
            .ok_or_else(|| DbError::ObjectNotFound(full_name))
    }
}

/// Extract view name from CREATE/DROP MATERIALIZED VIEW statement
fn extract_view_name(sql: &str) -> Option<String> {
    // Simple regex-free extraction
    let sql_upper = sql.to_uppercase();

    let start = if sql_upper.contains("CREATE MATERIALIZED VIEW") {
        sql_upper.find("CREATE MATERIALIZED VIEW").map(|i| i + "CREATE MATERIALIZED VIEW".len())
    } else if sql_upper.contains("DROP MATERIALIZED VIEW") {
        sql_upper.find("DROP MATERIALIZED VIEW").map(|i| i + "DROP MATERIALIZED VIEW".len())
    } else {
        None
    }?;

    let remaining = sql[start..].trim();
    let end = remaining
        .find(|c: char| c.is_whitespace() || c == '(' || c == ';')
        .unwrap_or(remaining.len());

    let name = remaining[..end].trim().to_lowercase();
    if name.is_empty() { None } else { Some(name) }
}
```

### Usage Examples

```rust
#[tokio::test]
async fn test_continuous_aggregate_creation() {
    // Arrange: Empty database
    let db = MockTimescaleDb::new();

    // Act: Execute DDL
    let ddl = generate_continuous_aggregate(&config).unwrap();
    db.execute(&ddl).await.unwrap();

    // Assert: View was created
    assert!(db.continuous_aggregate_exists("gold", "air_quality_hourly").await.unwrap());

    // Assert: SQL was recorded
    assert!(db.sql_contains("CREATE MATERIALIZED VIEW"));
    assert!(db.sql_contains("timescaledb.continuous"));
}

#[tokio::test]
async fn test_idempotent_creation_checks_existence() {
    // Arrange: View already exists
    let db = MockTimescaleDb::new()
        .with_existing_continuous_aggregate("gold", "air_quality_hourly");

    // Act: Check existence before creating
    let exists = db.continuous_aggregate_exists("gold", "air_quality_hourly").await.unwrap();

    // Assert
    assert!(exists, "Should report view exists");
}

#[tokio::test]
async fn test_handles_connection_error() {
    // Arrange: Database will fail
    let db = MockTimescaleDb::new()
        .with_error(DbError::ConnectionFailed("timeout".into()));

    // Act
    let result = db.execute("SELECT 1").await;

    // Assert
    assert!(matches!(result, Err(DbError::ConnectionFailed(_))));
}
```

---

## 3. MockEtcdClient (New)

**Location**: `tools/ndp-gold-ddl/src/mocks/etcd.rs`
**Purpose**: Mock etcd for testing configuration sync operations

### Interface Definition

```rust
// tools/ndp-gold-ddl/src/config/etcd_client.rs

use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum EtcdError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

#[async_trait]
pub trait EtcdClient: Send + Sync {
    /// Get a value by key
    async fn get(&self, key: &str) -> Result<Option<String>, EtcdError>;

    /// Set a value
    async fn put(&self, key: &str, value: &str) -> Result<(), EtcdError>;

    /// List keys with prefix
    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, EtcdError>;

    /// Delete a key
    async fn delete(&self, key: &str) -> Result<(), EtcdError>;
}
```

### Mock Implementation

```rust
// tools/ndp-gold-ddl/src/mocks/etcd.rs

use std::collections::HashMap;
use std::sync::RwLock;
use async_trait::async_trait;
use crate::config::etcd_client::{EtcdClient, EtcdError};

/// Mock etcd client for unit testing
pub struct MockEtcdClient {
    /// Key-value store
    data: RwLock<HashMap<String, String>>,

    /// Error to return for all operations (if set)
    should_fail: RwLock<Option<EtcdError>>,
}

impl Default for MockEtcdClient {
    fn default() -> Self {
        Self::new()
    }
}

impl MockEtcdClient {
    /// Create a new empty mock
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
            should_fail: RwLock::new(None),
        }
    }

    /// Add a key-value pair (builder pattern)
    pub fn with_key(self, key: &str, value: &str) -> Self {
        self.data
            .write()
            .unwrap()
            .insert(key.to_string(), value.to_string());
        self
    }

    /// Add stream config (builder pattern)
    pub fn with_stream_config(self, stream_id: &str, config: &StreamConfig) -> Self {
        let key = format!("/streams/{}/config", stream_id);
        let value = serde_json::to_string(config).unwrap();
        self.with_key(&key, &value)
    }

    /// Configure mock to fail (builder pattern)
    pub fn with_error(self, error: EtcdError) -> Self {
        *self.should_fail.write().unwrap() = Some(error);
        self
    }

    /// Get all stored keys
    pub fn get_all_keys(&self) -> Vec<String> {
        self.data.read().unwrap().keys().cloned().collect()
    }
}

#[async_trait]
impl EtcdClient for MockEtcdClient {
    async fn get(&self, key: &str) -> Result<Option<String>, EtcdError> {
        if let Some(ref err) = *self.should_fail.read().unwrap() {
            return Err(err.clone());
        }

        Ok(self.data.read().unwrap().get(key).cloned())
    }

    async fn put(&self, key: &str, value: &str) -> Result<(), EtcdError> {
        if let Some(ref err) = *self.should_fail.read().unwrap() {
            return Err(err.clone());
        }

        self.data
            .write()
            .unwrap()
            .insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, EtcdError> {
        if let Some(ref err) = *self.should_fail.read().unwrap() {
            return Err(err.clone());
        }

        Ok(self.data
            .read()
            .unwrap()
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect())
    }

    async fn delete(&self, key: &str) -> Result<(), EtcdError> {
        if let Some(ref err) = *self.should_fail.read().unwrap() {
            return Err(err.clone());
        }

        self.data.write().unwrap().remove(key);
        Ok(())
    }
}
```

---

## 4. MockSchemaValidator (New)

**Location**: `tools/ndp-gold-ddl/src/mocks/validator.rs`
**Purpose**: Mock JSON Schema validation for testing config handling

### Implementation

```rust
// tools/ndp-gold-ddl/src/mocks/validator.rs

use std::collections::HashMap;
use std::sync::RwLock;
use crate::validation::{SchemaValidator, ValidationError, ValidationResult};

/// Mock schema validator for unit testing
pub struct MockSchemaValidator {
    /// Schemas that should validate successfully (schema_name -> always_valid)
    valid_schemas: RwLock<HashMap<String, bool>>,

    /// Specific configs that should fail (hash -> error message)
    failing_configs: RwLock<HashMap<String, String>>,
}

impl Default for MockSchemaValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl MockSchemaValidator {
    pub fn new() -> Self {
        Self {
            valid_schemas: RwLock::new(HashMap::new()),
            failing_configs: RwLock::new(HashMap::new()),
        }
    }

    /// Mark a schema as always validating (builder pattern)
    pub fn with_valid_schema(self, schema_name: &str) -> Self {
        self.valid_schemas
            .write()
            .unwrap()
            .insert(schema_name.to_string(), true);
        self
    }

    /// Configure a specific config to fail validation (builder pattern)
    pub fn with_failing_config(self, config_hash: &str, error: &str) -> Self {
        self.failing_configs
            .write()
            .unwrap()
            .insert(config_hash.to_string(), error.to_string());
        self
    }

    /// Default: all schemas valid, no configs fail
    pub fn permissive() -> Self {
        Self::new()
    }
}

impl SchemaValidator for MockSchemaValidator {
    fn validate(&self, config: &serde_json::Value, schema_name: &str) -> ValidationResult {
        // Check if this specific config should fail
        let config_str = serde_json::to_string(config).unwrap_or_default();
        if let Some(error) = self.failing_configs.read().unwrap().get(&config_str) {
            return Err(ValidationError::SchemaViolation(error.clone()));
        }

        // Check if schema is marked as valid
        if self.valid_schemas.read().unwrap().contains_key(schema_name) {
            return Ok(());
        }

        // Default: validation passes
        Ok(())
    }
}
```

---

## 5. Test Fixtures Module

**Location**: `tools/ndp-gold-ddl/tests/fixtures/mod.rs`
**Purpose**: Shared test data creation helpers

### Implementation

```rust
// tools/ndp-gold-ddl/tests/fixtures/mod.rs

use neural_core::types::{StreamConfig, SchemaField, FieldType, SourceConfig, SourceType};
use neural_core::config::silver_etl::{SilverEtlConfig, TimestampMapping, TimestampTransform};
use crate::config::{GoldEtlConfig, AggregatesConfig, FieldConfig, FeatureConfig};
use std::collections::HashMap;

/// Create a minimal valid stream config for testing
pub fn create_test_stream_config(stream_id: &str) -> StreamConfig {
    StreamConfig {
        stream_id: stream_id.to_string(),
        description: format!("{} test stream", stream_id),
        version: "1.0.0".to_string(),
        enabled: true,
        retention_days: 365,
        compression_after_days: 7,
        partitioning_strategy: "daily".to_string(),
        fields: vec![
            SchemaField::new("pm25".to_string(), FieldType::Float)
                .required()
                .with_unit("ug/m3".to_string()),
            SchemaField::new("co2".to_string(), FieldType::Float)
                .with_unit("ppm".to_string()),
            SchemaField::new("temperature_c".to_string(), FieldType::Float)
                .with_unit("celsius".to_string()),
        ],
        sources: vec![SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            ndp_id: Some(format!("{}-sensor-001", stream_id)),
            context: None,
            params: HashMap::new(),
        }],
        storage: None,
        silver_etl: None,
        entity_schemas: None,
    }
}

/// Create a stream config with custom fields
pub fn create_stream_with_fields(stream_id: &str, field_names: &[&str]) -> StreamConfig {
    let mut config = create_test_stream_config(stream_id);
    config.fields = field_names
        .iter()
        .map(|name| SchemaField::new(name.to_string(), FieldType::Float))
        .collect();
    config
}

/// Create a minimal valid Silver ETL config
pub fn create_test_silver_config(stream_id: &str) -> SilverEtlConfig {
    SilverEtlConfig {
        enabled: true,
        target_table: format!("silver.{}_observations", stream_id.replace("-", "_")),
        target_schema: None,
        timestamp: TimestampMapping {
            source_field: "timestamp".to_string(),
            target_field: "observation_time".to_string(),
            transform: TimestampTransform::MicrosecondsToTimestamp,
        },
        valid_timestamp: None,
        pre_transform: None,
        identity_fields: vec!["ndp_id".to_string()],
        field_mappings: vec![],
        dq_rules: vec![],
        dq_output: Default::default(),
        deduplication: Default::default(),
        incremental: Default::default(),
    }
}

/// Create a minimal valid Gold ETL config
pub fn create_test_gold_config(stream_id: &str) -> GoldEtlConfig {
    GoldEtlConfig {
        enabled: true,
        stream_id: stream_id.to_string(),
        description: Some(format!("{} Gold layer", stream_id)),
        aggregates: AggregatesConfig {
            granularities: vec!["1 hour".to_string()],
            default_metrics: Some(vec!["mean".to_string(), "count".to_string()]),
            fields: hashmap! {
                "pm25".to_string() => FieldConfig {
                    metrics: vec!["mean".to_string(), "std".to_string(), "min".to_string(), "max".to_string()]
                }
            },
        },
        features: None,
        refresh_policy: None,
    }
}

/// Create a Gold ETL config with specific fields and metrics
pub fn create_gold_config_with_metrics(
    stream_id: &str,
    granularity: &str,
    fields: &[(&str, Vec<&str>)]
) -> GoldEtlConfig {
    let mut config = create_test_gold_config(stream_id);
    config.aggregates.granularities = vec![granularity.to_string()];
    config.aggregates.fields = fields
        .iter()
        .map(|(name, metrics)| {
            (
                name.to_string(),
                FieldConfig {
                    metrics: metrics.iter().map(|m| m.to_string()).collect(),
                },
            )
        })
        .collect();
    config
}

/// Create a domain config for alignment testing
pub fn create_test_domain_config(domain_id: &str) -> DomainConfig {
    DomainConfig {
        id: domain_id.to_string(),
        description: Some(format!("{} domain", domain_id)),
        streams: vec![
            StreamRef {
                stream_id: "air-quality".to_string(),
                alias: "aq".to_string(),
                role: StreamRole::Primary,
            },
            StreamRef {
                stream_id: "outdoor-weather".to_string(),
                alias: "ow".to_string(),
                role: StreamRole::Context,
            },
        ],
        alignment: AlignmentConfig {
            view_name: format!("{}_aligned", domain_id.replace("-", "_")),
            granularity: "1 hour".to_string(),
            join_strategy: JoinStrategy::FullOuter,
            null_handling: NullHandling::ByStreamType,
        },
        objectives: vec![],
    }
}

/// Load fixture config from JSON file
pub fn load_fixture_config(name: &str) -> serde_json::Value {
    let path = format!("tests/fixtures/configs/valid/{}.json", name);
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read fixture: {}", path));
    serde_json::from_str(&content)
        .expect(&format!("Failed to parse fixture: {}", path))
}

/// Load expected SQL fixture
pub fn load_expected_sql(name: &str) -> String {
    let path = format!("tests/fixtures/expected_sql/{}.sql", name);
    std::fs::read_to_string(&path)
        .expect(&format!("Failed to read expected SQL: {}", path))
}

/// Macro for creating HashMap in tests
#[macro_export]
macro_rules! hashmap {
    ($($key:expr => $value:expr),* $(,)?) => {{
        let mut map = std::collections::HashMap::new();
        $(map.insert($key, $value);)*
        map
    }};
}
```

---

## Mock Module Organization

### File Structure

```
tools/ndp-gold-ddl/src/
├── mocks/
│   ├── mod.rs              # Re-export all mocks
│   ├── timescale.rs        # MockTimescaleDb
│   ├── etcd.rs             # MockEtcdClient
│   └── validator.rs        # MockSchemaValidator
├── db/
│   └── connection.rs       # TimescaleConnection trait
├── config/
│   └── etcd_client.rs      # EtcdClient trait
└── validation/
    └── mod.rs              # SchemaValidator trait

tests/
├── fixtures/
│   ├── mod.rs              # Test helpers
│   ├── configs/
│   │   ├── valid/
│   │   │   └── air_quality_basic.json
│   │   └── invalid/
│   │       └── unknown_metric.json
│   └── expected_sql/
│       └── air_quality_hourly.sql
└── helpers/
    └── sql_compare.rs      # SQL comparison utilities
```

### Module Re-exports

```rust
// tools/ndp-gold-ddl/src/mocks/mod.rs

pub mod timescale;
pub mod etcd;
pub mod validator;

pub use timescale::MockTimescaleDb;
pub use etcd::MockEtcdClient;
pub use validator::MockSchemaValidator;

// Re-export the existing MockConfigLoader
pub use neural_core::config::mock_loader::MockConfigLoader;
```

---

## Usage in Tests

```rust
// Example test file: tools/ndp-gold-ddl/tests/continuous_aggregate_test.rs

use ndp_gold_ddl::mocks::{MockConfigLoader, MockTimescaleDb};
use ndp_gold_ddl::generators::continuous_aggregate::generate_continuous_aggregate;

mod fixtures;
use fixtures::{create_test_gold_config, create_test_stream_config};

#[tokio::test]
async fn test_full_generation_pipeline() {
    // Arrange: Set up all mocks
    let loader = MockConfigLoader::new()
        .with_stream(create_test_stream_config("air-quality"))
        .with_gold_config("air-quality", create_test_gold_config("air-quality"));

    let db = MockTimescaleDb::new();

    // Act: Generate and execute DDL
    let gold_config = loader.load_gold_etl_config("air-quality").await.unwrap();
    let sql = generate_continuous_aggregate(&gold_config).unwrap();
    db.execute(&sql).await.unwrap();

    // Assert: Verify behavior
    assert!(db.continuous_aggregate_exists("gold", "air_quality_hourly").await.unwrap());
    assert!(db.sql_contains("WITH (timescaledb.continuous)"));
}
```

---

## References

- [TDD-GUIDE.md](./TDD-GUIDE.md) - Step-by-step TDD instructions
- [TEST-PLAN.md](./TEST-PLAN.md) - Complete Phase A test plan
- [mock_loader.rs](/workspaces/neural-data-platform/core/src/config/mock_loader.rs) - Existing mock pattern

---

*Mock Definitions created: 2026-02-04*
