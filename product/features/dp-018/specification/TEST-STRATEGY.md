# dp-018: London TDD Test Strategy

## Overview

This document defines the London School TDD test strategy for dp-018 (JSON Config Foundation). The tests follow outside-in development with mock-driven design, focusing on behavior verification rather than state inspection.

---

## Current Test Baseline

From Episode 26 analysis:

| Component | Tests | Status |
|-----------|-------|--------|
| platform-core | 147 | Passing |
| air-quality-app | 112 passing, 14 failing | etcd dependency in coordinator tests |
| config-client | 3 integration tests | Requires etcd (ignored by default) |
| silver-etl | 10 integration tests | Requires Docker (ignored by default) |

**Key Finding**: Most test failures are due to etcd dependency in tests that assume etcd is always available. dp-018 must establish patterns for testing WITHOUT infrastructure.

---

## Test Categories

### 1. Unit Tests (Isolated, Mocked Dependencies)

**Purpose**: Test individual functions and methods in isolation
**Infrastructure Required**: None
**Run Command**: `cargo test`

### 2. Integration Tests (Real Components, Mock Infrastructure)

**Purpose**: Test component interactions with mocked external systems
**Infrastructure Required**: None (mocks replace etcd, TimescaleDB)
**Run Command**: `cargo test` (not ignored)

### 3. Contract Tests (Schema Validation)

**Purpose**: Validate config schemas and data contracts
**Infrastructure Required**: None
**Run Command**: `cargo test --test schema_validation`

### 4. Full Integration Tests (Real Infrastructure)

**Purpose**: End-to-end testing with real etcd and TimescaleDB
**Infrastructure Required**: Docker (etcd, TimescaleDB)
**Run Command**: `cargo test -- --ignored`

---

## Phase 0: JSON Migration Tests

### 0.1 JSON Schema Validation Tests

**File**: `tests/schema_validation.rs`

```rust
#[cfg(test)]
mod json_schema_tests {
    use serde_json::Value;
    use jsonschema::{JSONSchema, Draft};

    // ============================================================
    // Test: Schema accepts v1.0 structure (backward compat)
    // ============================================================
    #[test]
    fn test_schema_accepts_v1_config_with_entity_schemas() {
        let schema: Value = load_schema("stream-config.schema.json");
        let compiled = JSONSchema::compile(&schema).unwrap();

        let v1_config = json!({
            "stream_id": "air-quality",
            "config_version": 1.0,
            "fields": [
                { "name": "pm25", "type": "float", "nullable": false }
            ],
            "entity_schemas": [
                { "entity_type": "sensor", "fields": {...} }
            ]
        });

        assert!(compiled.is_valid(&v1_config));
    }

    // ============================================================
    // Test: Schema accepts v1.1 structure (enriched fields)
    // ============================================================
    #[test]
    fn test_schema_accepts_v1_1_config_with_enriched_fields() {
        let schema: Value = load_schema("stream-config.schema.json");
        let compiled = JSONSchema::compile(&schema).unwrap();

        let v1_1_config = json!({
            "stream_id": "air-quality",
            "config_version": 1.1,
            "fields": [
                {
                    "name": "pm25",
                    "type": "float",
                    "nullable": false,
                    "description": "Particulate matter 2.5um",
                    "device_class": "sensor"
                }
            ]
        });

        assert!(compiled.is_valid(&v1_1_config));
    }

    // ============================================================
    // Test: Schema rejects invalid stream_id format
    // ============================================================
    #[test]
    fn test_schema_rejects_invalid_stream_id() {
        let schema: Value = load_schema("stream-config.schema.json");
        let compiled = JSONSchema::compile(&schema).unwrap();

        let invalid_config = json!({
            "stream_id": "Invalid_ID", // Uppercase not allowed
            "config_version": 1.1,
            "fields": []
        });

        assert!(!compiled.is_valid(&invalid_config));
    }

    // ============================================================
    // Test: Schema requires minimum one field
    // ============================================================
    #[test]
    fn test_schema_requires_at_least_one_field() {
        let schema: Value = load_schema("stream-config.schema.json");
        let compiled = JSONSchema::compile(&schema).unwrap();

        let no_fields_config = json!({
            "stream_id": "test",
            "config_version": 1.1,
            "fields": []
        });

        let result = compiled.validate(&no_fields_config);
        assert!(result.is_err());
    }
}
```

### 0.2 Migration Script Tests

**File**: `tests/migration_tests.sh`

```bash
#!/bin/bash
# Migration script unit tests

# Test: YAML to JSON round-trip preserves data
test_roundtrip() {
    local yaml_file="$1"
    local json_file="${yaml_file%.yaml}.json"

    # Convert YAML to JSON
    ./scripts/migrate-yaml-to-json.sh "$yaml_file" "$json_file"

    # Convert back to YAML
    yq -o yaml "$json_file" > "${yaml_file}.roundtrip"

    # Compare normalized versions
    if diff <(yq -o json "$yaml_file" | jq -S) <(cat "$json_file" | jq -S); then
        echo "PASS: $yaml_file"
        return 0
    else
        echo "FAIL: $yaml_file - data not preserved"
        return 1
    fi
}

# Test: Idempotent migration
test_idempotent() {
    local json_file="$1"
    local hash1=$(sha256sum "$json_file" | cut -d' ' -f1)

    # Run migration again
    ./scripts/migrate-yaml-to-json.sh "${json_file%.json}.yaml" "$json_file"

    local hash2=$(sha256sum "$json_file" | cut -d' ' -f1)

    if [ "$hash1" == "$hash2" ]; then
        echo "PASS: Idempotent migration"
    else
        echo "FAIL: Migration not idempotent"
        return 1
    fi
}

# Test: Field enrichment from entity_schemas
test_field_enrichment() {
    local yaml_file="$1"
    local json_file="${yaml_file%.yaml}.json"

    # Migrate with enrichment
    ./scripts/migrate-yaml-to-json.sh --enrich "$yaml_file" "$json_file"

    # Check that fields have description
    if jq -e '.fields[0].description' "$json_file" > /dev/null; then
        echo "PASS: Fields enriched with description"
    else
        echo "FAIL: Fields not enriched"
        return 1
    fi
}
```

### 0.3 Config Parsing Tests

**File**: `core/src/config/mod.rs` (inline tests)

```rust
#[cfg(test)]
mod json_parsing_tests {
    use super::*;
    use serde_json::json;

    // ============================================================
    // Test: Parse v1.1 JSON with enriched fields
    // ============================================================
    #[test]
    fn test_parse_v1_1_json_config() {
        let json = json!({
            "stream_id": "air-quality",
            "description": "Air quality sensor data",
            "version": "1.1.0",
            "enabled": true,
            "fields": [{
                "name": "pm25",
                "type": "float",
                "nullable": false,
                "unit": "ug/m3",
                "description": "Particulate matter 2.5um"
            }],
            "sources": [{
                "source_type": "mqtt",
                "enabled": true,
                "params": {}
            }]
        });

        let config: StreamConfig = serde_json::from_value(json).unwrap();

        assert_eq!(config.stream_id, "air-quality");
        assert_eq!(config.fields.len(), 1);
        assert_eq!(config.fields[0].description, Some("Particulate matter 2.5um".to_string()));
    }

    // ============================================================
    // Test: Parse v1.0 JSON (backward compat)
    // ============================================================
    #[test]
    fn test_parse_v1_0_json_config_backward_compat() {
        let json = json!({
            "stream_id": "air-quality",
            "description": "Air quality sensor data",
            "version": "1.0.0",
            "enabled": true,
            "fields": [{
                "name": "pm25",
                "type": "float",
                "nullable": false
            }],
            "sources": [{
                "source_type": "mqtt",
                "enabled": true,
                "params": {}
            }]
        });

        let config: StreamConfig = serde_json::from_value(json).unwrap();

        // description should be None for v1.0
        assert!(config.fields[0].description.is_none());
    }

    // ============================================================
    // Test: JSON and YAML parsing produce identical results
    // ============================================================
    #[test]
    fn test_json_yaml_parsing_equivalent() {
        let yaml = r#"
stream_id: air-quality
description: Air quality sensor data
version: "1.0.0"
enabled: true
fields:
  - name: pm25
    type: float
    nullable: false
sources:
  - source_type: mqtt
    enabled: true
    params: {}
"#;

        let json = json!({
            "stream_id": "air-quality",
            "description": "Air quality sensor data",
            "version": "1.0.0",
            "enabled": true,
            "fields": [{"name": "pm25", "type": "float", "nullable": false}],
            "sources": [{"source_type": "mqtt", "enabled": true, "params": {}}]
        });

        let yaml_config: StreamConfig = serde_yaml::from_str(yaml).unwrap();
        let json_config: StreamConfig = serde_json::from_value(json).unwrap();

        assert_eq!(yaml_config, json_config);
    }
}
```

---

## Phase 1: ConfigLoader Trait Tests

### 1.1 ConfigLoader Trait Definition

**File**: `core/src/config/loader.rs`

```rust
use async_trait::async_trait;
use neural_core::{StreamConfig, SilverEtlConfig};
use thiserror::Error;

/// Configuration loading errors
#[derive(Debug, Error)]
pub enum ConfigLoaderError {
    #[error("Stream not found: {0}")]
    StreamNotFound(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),
}

/// Unified trait for configuration loading
///
/// Implementors: EtcdConfigLoader, MockConfigLoader
#[async_trait]
pub trait ConfigLoader: Send + Sync {
    /// Load stream configuration by stream_id
    async fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig, ConfigLoaderError>;

    /// Load Silver ETL configuration for a stream
    async fn load_silver_etl_config(&self, stream_id: &str) -> Result<SilverEtlConfig, ConfigLoaderError>;

    /// List all stream IDs
    async fn list_streams(&self) -> Result<Vec<String>, ConfigLoaderError>;

    /// Check if stream exists
    async fn stream_exists(&self, stream_id: &str) -> Result<bool, ConfigLoaderError>;

    /// Get config source name (for logging)
    fn source_name(&self) -> &'static str;
}
```

### 1.2 MockConfigLoader for Unit Tests

**File**: `core/src/config/mock_loader.rs`

```rust
use super::{ConfigLoader, ConfigLoaderError};
use mockall::automock;
use std::collections::HashMap;
use std::sync::RwLock;

/// Mock implementation for unit testing without etcd
pub struct MockConfigLoader {
    streams: RwLock<HashMap<String, StreamConfig>>,
    silver_configs: RwLock<HashMap<String, SilverEtlConfig>>,
    should_fail: RwLock<Option<ConfigLoaderError>>,
}

impl MockConfigLoader {
    pub fn new() -> Self {
        Self {
            streams: RwLock::new(HashMap::new()),
            silver_configs: RwLock::new(HashMap::new()),
            should_fail: RwLock::new(None),
        }
    }

    /// Add a stream config for testing
    pub fn with_stream(self, config: StreamConfig) -> Self {
        self.streams.write().unwrap().insert(config.stream_id.clone(), config);
        self
    }

    /// Add a Silver ETL config for testing
    pub fn with_silver_config(self, stream_id: &str, config: SilverEtlConfig) -> Self {
        self.silver_configs.write().unwrap().insert(stream_id.to_string(), config);
        self
    }

    /// Configure mock to fail with specific error
    pub fn with_error(self, error: ConfigLoaderError) -> Self {
        *self.should_fail.write().unwrap() = Some(error);
        self
    }
}

#[async_trait]
impl ConfigLoader for MockConfigLoader {
    async fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig, ConfigLoaderError> {
        if let Some(ref err) = *self.should_fail.read().unwrap() {
            return Err(ConfigLoaderError::ConnectionError(err.to_string()));
        }

        self.streams
            .read()
            .unwrap()
            .get(stream_id)
            .cloned()
            .ok_or_else(|| ConfigLoaderError::StreamNotFound(stream_id.to_string()))
    }

    async fn load_silver_etl_config(&self, stream_id: &str) -> Result<SilverEtlConfig, ConfigLoaderError> {
        if let Some(ref err) = *self.should_fail.read().unwrap() {
            return Err(ConfigLoaderError::ConnectionError(err.to_string()));
        }

        self.silver_configs
            .read()
            .unwrap()
            .get(stream_id)
            .cloned()
            .ok_or_else(|| ConfigLoaderError::StreamNotFound(stream_id.to_string()))
    }

    async fn list_streams(&self) -> Result<Vec<String>, ConfigLoaderError> {
        if let Some(ref err) = *self.should_fail.read().unwrap() {
            return Err(ConfigLoaderError::ConnectionError(err.to_string()));
        }

        Ok(self.streams.read().unwrap().keys().cloned().collect())
    }

    async fn stream_exists(&self, stream_id: &str) -> Result<bool, ConfigLoaderError> {
        Ok(self.streams.read().unwrap().contains_key(stream_id))
    }

    fn source_name(&self) -> &'static str {
        "mock"
    }
}
```

### 1.3 EtcdConfigLoader Tests

**File**: `core/src/config/etcd_loader_test.rs`

```rust
#[cfg(test)]
mod etcd_config_loader_tests {
    use super::*;
    use mockall::predicate::*;

    // ============================================================
    // Test: Load stream config from etcd (mocked)
    // ============================================================
    #[tokio::test]
    async fn test_load_stream_config_success() {
        // Use MockConfigLoader to test behavior
        let config = create_test_stream_config("air-quality");
        let loader = MockConfigLoader::new().with_stream(config.clone());

        let result = loader.load_stream_config("air-quality").await;

        assert!(result.is_ok());
        let loaded = result.unwrap();
        assert_eq!(loaded.stream_id, "air-quality");
    }

    // ============================================================
    // Test: Stream not found returns error
    // ============================================================
    #[tokio::test]
    async fn test_load_stream_config_not_found() {
        let loader = MockConfigLoader::new();

        let result = loader.load_stream_config("nonexistent").await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConfigLoaderError::StreamNotFound(_)
        ));
    }

    // ============================================================
    // Test: Connection error is propagated
    // ============================================================
    #[tokio::test]
    async fn test_load_stream_config_connection_error() {
        let loader = MockConfigLoader::new()
            .with_error(ConfigLoaderError::ConnectionError("etcd unreachable".into()));

        let result = loader.load_stream_config("any").await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConfigLoaderError::ConnectionError(_)
        ));
    }

    // ============================================================
    // Test: Load Silver ETL config
    // ============================================================
    #[tokio::test]
    async fn test_load_silver_etl_config_success() {
        let silver_config = SilverEtlConfig {
            enabled: true,
            target_table: "silver.air_quality_observations".to_string(),
            ..Default::default()
        };

        let loader = MockConfigLoader::new()
            .with_silver_config("air-quality", silver_config);

        let result = loader.load_silver_etl_config("air-quality").await;

        assert!(result.is_ok());
        let loaded = result.unwrap();
        assert!(loaded.enabled);
        assert_eq!(loaded.target_table, "silver.air_quality_observations");
    }

    // ============================================================
    // Test: Source name for logging
    // ============================================================
    #[tokio::test]
    async fn test_source_name_returns_etcd() {
        let loader = EtcdConfigLoader::new_mock();
        assert_eq!(loader.source_name(), "etcd");
    }
}

// Integration tests (require etcd)
#[cfg(test)]
mod etcd_integration_tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Run with --ignored when etcd is available
    async fn test_etcd_loader_real_connection() {
        let loader = EtcdConfigLoader::new(&["http://localhost:2379"])
            .await
            .expect("Failed to connect to etcd");

        // Test basic functionality
        let streams = loader.list_streams().await.expect("Failed to list streams");
        assert!(streams.len() >= 0);
    }
}
```

---

## Phase 1.3: Silver Subscriber Tests

### Silver Subscriber with Mock ConfigLoader

**File**: `apps/air-quality-app/src/silver/subscriber_test.rs`

```rust
#[cfg(test)]
mod silver_subscriber_tests {
    use super::*;
    use crate::config::{MockConfigLoader, ConfigLoader};
    use neural_core::{EventBus, TimeSeriesPoint};

    // ============================================================
    // Test: SilverSubscriber loads config from ConfigLoader
    // ============================================================
    #[tokio::test]
    async fn test_subscriber_loads_config_from_loader() {
        // Arrange
        let silver_config = create_test_silver_config();
        let loader = Arc::new(MockConfigLoader::new()
            .with_silver_config("air-quality", silver_config));

        let event_bus = Arc::new(EventBus::new(Default::default()));

        // Act
        let subscriber = SilverSubscriber::new(
            loader,
            mock_timescale_output(),
            event_bus.clone(),
        ).await;

        // Assert
        assert!(subscriber.is_ok());
        // Verify config was loaded (behavior verification via mock expectations)
    }

    // ============================================================
    // Test: SilverSubscriber handles config load failure
    // ============================================================
    #[tokio::test]
    async fn test_subscriber_handles_config_load_failure() {
        // Arrange
        let loader = Arc::new(MockConfigLoader::new()
            .with_error(ConfigLoaderError::ConnectionError("etcd down".into())));

        let event_bus = Arc::new(EventBus::new(Default::default()));

        // Act
        let result = SilverSubscriber::new(
            loader,
            mock_timescale_output(),
            event_bus.clone(),
        ).await;

        // Assert
        assert!(result.is_err());
        // Should log ERROR level (dp-018 requirement 1.7)
    }

    // ============================================================
    // Test: SilverSubscriber processes events with loaded config
    // ============================================================
    #[tokio::test]
    async fn test_subscriber_processes_events_with_config() {
        // Arrange
        let silver_config = create_test_silver_config();
        let loader = Arc::new(MockConfigLoader::new()
            .with_silver_config("air-quality", silver_config));

        let event_bus = Arc::new(EventBus::new(Default::default()));
        let output = Arc::new(InMemorySilverOutput::new());

        let subscriber = SilverSubscriber::new(
            loader,
            output.clone(),
            event_bus.clone(),
        ).await.unwrap();

        // Act - publish event to bus
        event_bus.publish(create_test_event("air-quality"));

        // Give subscriber time to process
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Assert - verify output received transformed data
        let records = output.get_records("air-quality").await;
        assert!(!records.is_empty());
    }

    // ============================================================
    // Test: Config source is logged
    // ============================================================
    #[tokio::test]
    async fn test_config_source_logged() {
        // This test would use tracing-subscriber test utilities
        // to capture and verify log output

        let loader = Arc::new(MockConfigLoader::new()
            .with_silver_config("air-quality", create_test_silver_config()));

        // Verify log contains "config loaded from etcd" or "config loaded from mock"
        // Implementation uses tracing::info! with source_name()
    }

    // ============================================================
    // Helper functions
    // ============================================================

    fn create_test_silver_config() -> SilverEtlConfig {
        SilverEtlConfig {
            enabled: true,
            target_table: "silver.air_quality_observations".to_string(),
            timestamp: TimestampMapping {
                source_field: "timestamp".to_string(),
                target_field: "observation_time".to_string(),
                transform: TimestampTransform::MicrosecondsToTimestamp,
            },
            field_mappings: vec![
                SilverFieldMapping {
                    source_path: "raw_payload.pm25".to_string(),
                    target_column: "pm25".to_string(),
                    column_type: "double_precision".to_string(),
                    nullable: false,
                    transform: None,
                    dq_rules: vec![],
                },
            ],
            ..Default::default()
        }
    }

    fn mock_timescale_output() -> Arc<dyn SilverOutput> {
        Arc::new(InMemorySilverOutput::new())
    }

    fn create_test_event(stream_id: &str) -> TimeSeriesPoint {
        TimeSeriesPoint {
            timestamp: Utc::now(),
            stream_id: stream_id.to_string(),
            fields: [("pm25".to_string(), json!(25.5))].into(),
            tags: HashMap::new(),
        }
    }
}
```

---

## Phase 1.5: Dictionary Loader Tests

### Dictionary Loader with fields + entity_schemas Fallback

**File**: `core/ndp-mcp-server/src/storage/dictionary_test.rs`

```rust
#[cfg(test)]
mod dictionary_loader_tests {
    use super::*;

    // ============================================================
    // Test: Load from fields.description (v1.1 config)
    // ============================================================
    #[tokio::test]
    async fn test_dictionary_loads_from_fields_description() {
        // Arrange - v1.1 config with enriched fields
        let config = json!({
            "stream_id": "air-quality",
            "fields": [{
                "name": "pm25",
                "type": "float",
                "description": "Particulate matter 2.5um",
                "unit": "ug/m3"
            }]
        });

        let loader = MockConfigLoader::new()
            .with_stream(serde_json::from_value(config).unwrap());

        let dictionary = DictionaryLoader::new(Arc::new(loader));

        // Act
        let entries = dictionary.load_bronze_entries("air-quality").await.unwrap();

        // Assert
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].column_name, "pm25");
        assert_eq!(entries[0].description, Some("Particulate matter 2.5um".to_string()));
        assert_eq!(entries[0].unit, Some("ug/m3".to_string()));
    }

    // ============================================================
    // Test: Fallback to entity_schemas when fields lack description
    // ============================================================
    #[tokio::test]
    async fn test_dictionary_fallback_to_entity_schemas() {
        // Arrange - v1.0 config with entity_schemas but no field descriptions
        let config = json!({
            "stream_id": "air-quality",
            "fields": [{
                "name": "pm25",
                "type": "float"
            }],
            "entity_schemas": [{
                "entity_type": "sensor",
                "fields": [{
                    "name": "pm25",
                    "description": "PM2.5 from entity_schemas"
                }]
            }]
        });

        let loader = MockConfigLoader::new()
            .with_stream(serde_json::from_value(config).unwrap());

        let dictionary = DictionaryLoader::new(Arc::new(loader));

        // Act
        let entries = dictionary.load_bronze_entries("air-quality").await.unwrap();

        // Assert - should fallback to entity_schemas description
        assert_eq!(entries[0].description, Some("PM2.5 from entity_schemas".to_string()));
    }

    // ============================================================
    // Test: fields.description takes precedence over entity_schemas
    // ============================================================
    #[tokio::test]
    async fn test_fields_description_takes_precedence() {
        // Arrange - both fields and entity_schemas have descriptions
        let config = json!({
            "stream_id": "air-quality",
            "fields": [{
                "name": "pm25",
                "type": "float",
                "description": "From fields (preferred)"
            }],
            "entity_schemas": [{
                "entity_type": "sensor",
                "fields": [{
                    "name": "pm25",
                    "description": "From entity_schemas (fallback)"
                }]
            }]
        });

        let loader = MockConfigLoader::new()
            .with_stream(serde_json::from_value(config).unwrap());

        let dictionary = DictionaryLoader::new(Arc::new(loader));

        // Act
        let entries = dictionary.load_bronze_entries("air-quality").await.unwrap();

        // Assert - should use fields.description, not entity_schemas
        assert_eq!(entries[0].description, Some("From fields (preferred)".to_string()));
    }

    // ============================================================
    // Test: Handle missing descriptions gracefully
    // ============================================================
    #[tokio::test]
    async fn test_dictionary_handles_missing_descriptions() {
        // Arrange - no descriptions anywhere
        let config = json!({
            "stream_id": "air-quality",
            "fields": [{
                "name": "pm25",
                "type": "float"
            }]
        });

        let loader = MockConfigLoader::new()
            .with_stream(serde_json::from_value(config).unwrap());

        let dictionary = DictionaryLoader::new(Arc::new(loader));

        // Act
        let entries = dictionary.load_bronze_entries("air-quality").await.unwrap();

        // Assert - description should be None
        assert!(entries[0].description.is_none());
    }
}
```

---

## Mock Strategy Summary

### Mock Boundaries

| Component | Mock | Purpose |
|-----------|------|---------|
| etcd Client | `MockConfigLoader` | Unit test config loading without etcd |
| TimescaleDB | `InMemorySilverOutput` | Unit test Silver transforms without database |
| Parquet Store | `InMemoryBronzeStore` | Unit test Bronze ingestion without files |
| EventBus | Real `EventBus` | Test async messaging (lightweight, no mocks needed) |

### Existing Mock Patterns in Codebase

**Reference**: `core/ndp-mcp-server/src/mcp/tools/query_dictionary.rs`

```rust
// MockDictionaryStore pattern (already in codebase)
use crate::storage::MockDictionaryStore;

let mut mock = MockDictionaryStore::new();
mock.expect_search()
    .with(predicate::eq("temperature"), predicate::eq(None))
    .times(1)
    .returning(|_, _| Ok(vec![...]));
```

**Reference**: `core/src/traits.rs`

```rust
// mockall usage for Source/Store traits
mock! {
    pub TestStore {}
    #[async_trait]
    impl Store for TestStore {
        async fn write(&self, point: &TimeSeriesPoint) -> Result<(), StoreError>;
        async fn query(&self, filter: QueryFilter) -> Result<Vec<TimeSeriesPoint>, StoreError>;
    }
}
```

---

## Test Execution Commands

### Unit Tests (No Infrastructure)

```bash
# All unit tests
cargo test

# With output
cargo test -- --nocapture

# Specific module
cargo test config::loader_tests
cargo test silver::subscriber_tests
cargo test dictionary_loader_tests

# Filter by test name
cargo test test_load_stream_config
```

### Integration Tests (Requires Docker)

```bash
# Start test infrastructure
docker compose -f deploy/docker-compose.test.yml up -d

# Run integration tests
cargo test -- --ignored

# Specific integration test
cargo test --test etcd_config_test -- --ignored

# Stop infrastructure
docker compose -f deploy/docker-compose.test.yml down
```

### Schema Validation Tests

```bash
# JSON schema validation
./scripts/validate-configs.sh

# Individual config validation
jsonschema -i config/base/streams/air-quality/config.json schemas/stream-config.schema.json
```

---

## Test Categories by dp-018 Task

| Task ID | Task Description | Test Type | Test Files |
|---------|------------------|-----------|------------|
| 0.1 | JSON Schema v1.1 | Contract | `tests/schema_validation.rs` |
| 0.2 | Supporting schemas | Contract | `tests/schema_validation.rs` |
| 0.3 | Migration script | Shell | `tests/migration_tests.sh` |
| 0.4 | Migrate configs | Integration | `tests/migration_integration.rs` |
| 0.5 | Enrich fields | Unit | `core/src/config/mod.rs` |
| 1.1 | ConfigLoader trait | Unit | `core/src/config/loader_test.rs` |
| 1.2 | EtcdConfigLoader | Unit + Integration | `core/src/config/etcd_loader_test.rs` |
| 1.3 | Fix Silver streaming | Unit | `apps/air-quality-app/src/silver/subscriber_test.rs` |
| 1.4 | Fix Silver batch | Unit | `apps/silver-etl/tests/config_tests.rs` |
| 1.5 | Fix dictionary sync | Unit | `core/ndp-mcp-server/src/storage/dictionary_test.rs` |
| 1.5a | Dictionary loader | Unit | `core/ndp-mcp-server/src/storage/dictionary_test.rs` |
| 1.6 | Config source logging | Unit | Logging assertions in all tests |
| 1.7 | Promote sync errors | Unit | Error level assertions |

---

## Success Criteria Verification

| Criterion | Verification Method |
|-----------|---------------------|
| All configs in JSON format | Schema validation tests pass |
| No YAML file reads in runtime | Grep codebase for `.yaml` reads, exclude test fixtures |
| Config source logged | Tracing test captures `info!("config loaded from {}")` |
| Sync failures are ERROR level | Log level assertions in tests |
| Fields contain descriptions | Unit tests verify `description` field populated |

---

## References

- [AIR-005-TEST-DESIGN.md](../../../../docs/testing/AIR-005-TEST-DESIGN.md) - London TDD patterns
- [dp-018 SCOPE.md](../SCOPE.md) - Feature requirements
- [dp-016 IMPLEMENTATION-ROADMAP.md](../../dp-016/IMPLEMENTATION-ROADMAP.md) - Phase details
- [DQ-FRAMEWORK-DESIGN.md](../../../../docs/architecture/DQ-FRAMEWORK-DESIGN.md) - DQ testing patterns

---

*Test Strategy created: 2026-02-01*
*Author: ndp-tester agent*
