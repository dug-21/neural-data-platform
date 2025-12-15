# AIR-004 Stream Registry Integration - Test Plan (London TDD)

## Overview

This test plan follows **London School TDD** (outside-in, mockist approach) to drive the integration of StreamRegistry into the air-quality-app startup sequence. We test behavior from the outside (application startup) and mock collaborators (StreamRegistry, ConfigClient).

**Key Principle**: Define the interface and expected behavior FIRST through tests, then implement to satisfy those tests.

---

## 1. Test Strategy

### London TDD Approach

#### Outside-In Development Flow
1. **Start from the highest level** - Application startup behavior
2. **Mock all collaborators** - StreamRegistry, ConfigClient, MQTT, Storage
3. **Define contracts through tests** - What should happen, not how
4. **Drive interface design** - Tests define the API we want
5. **Work inward** - Once high-level tests pass, implement lower layers

#### Testing Pyramid for This Feature
```
┌─────────────────────────┐
│    E2E Tests (1-2)      │  <- Full app startup with real etcd
├─────────────────────────┤
│  Integration Tests (3-5)│  <- App + StreamRegistry + mock etcd
├─────────────────────────┤
│   Unit Tests (10-15)    │  <- Individual components with mocks
└─────────────────────────┘
```

#### Mockist vs Classicist
- **London School**: Mock all collaborators, focus on interactions
- **Detroit School**: Use real objects where possible

**We choose London** because:
- StreamRegistry depends on external etcd (network calls)
- We want fast, isolated tests
- We want to specify behavior before implementation exists
- We need to test error scenarios (etcd down, invalid config)

---

## 2. Test Doubles (Mocks & Stubs)

### 2.1 Mock StreamRegistry

Using `mockall` crate (already in use - see `/workspaces/neural-data-platform/apps/air-quality-app/src/api/routes.rs` lines 83-128):

```rust
use mockall::mock;
use config_client::stream::StreamRegistry;
use neural_core::StreamConfig;

mock! {
    pub StreamRegistry {
        // Primary methods we'll mock
        async fn new(endpoints: &[&str]) -> Result<Self, ConfigError>;
        async fn load_stream(&self, stream_id: &str) -> Result<StreamConfig, ConfigError>;
        async fn stream_exists(&self, stream_id: &str) -> Result<bool, ConfigError>;
        async fn list_streams(&self) -> Result<Vec<String>, ConfigError>;
    }
}
```

### 2.2 Mock ConfigClient (Internal to StreamRegistry)

For unit testing StreamRegistry itself:

```rust
mock! {
    pub ConfigClient {
        async fn with_prefix(endpoints: &[&str], prefix: &str) -> Result<Self, ConfigError>;
        async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T, ConfigError>;
        async fn list(&self, prefix: &str) -> Result<Vec<String>, ConfigError>;
        async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<(), ConfigError>;
    }
}
```

### 2.3 Test Data Builders

```rust
/// Builder for creating test StreamConfig instances
pub struct StreamConfigBuilder {
    stream_id: String,
    description: String,
    mqtt_broker: Option<String>,
    mqtt_port: Option<u16>,
    mqtt_topic: Option<String>,
    storage_path: Option<String>,
    enabled: bool,
}

impl StreamConfigBuilder {
    pub fn new(stream_id: &str) -> Self {
        Self {
            stream_id: stream_id.to_string(),
            description: format!("Test stream: {}", stream_id),
            mqtt_broker: None,
            mqtt_port: None,
            mqtt_topic: None,
            storage_path: None,
            enabled: true,
        }
    }

    pub fn with_mqtt(mut self, broker: &str, port: u16, topic: &str) -> Self {
        self.mqtt_broker = Some(broker.to_string());
        self.mqtt_port = Some(port);
        self.mqtt_topic = Some(topic);
        self
    }

    pub fn with_storage(mut self, path: &str) -> Self {
        self.storage_path = Some(path.to_string());
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn build(self) -> StreamConfig {
        use neural_core::{FieldType, SchemaField, SourceConfig, SourceType};
        use std::collections::HashMap;

        let mut mqtt_params = HashMap::new();
        mqtt_params.insert(
            "broker_url".to_string(),
            self.mqtt_broker.unwrap_or("localhost".to_string()),
        );
        mqtt_params.insert(
            "port".to_string(),
            self.mqtt_port.unwrap_or(1883).to_string(),
        );
        mqtt_params.insert(
            "topic".to_string(),
            self.mqtt_topic.unwrap_or("test/+".to_string()),
        );

        StreamConfig {
            stream_id: self.stream_id,
            description: self.description,
            version: "1.0.0".to_string(),
            enabled: self.enabled,
            retention_days: 30,
            compression_after_days: 7,
            partitioning_strategy: "daily".to_string(),
            fields: vec![
                SchemaField::new("pm25".to_string(), FieldType::Float)
                    .required()
                    .with_unit("µg/m³".to_string()),
                SchemaField::new("temperature".to_string(), FieldType::Float)
                    .with_unit("°C".to_string()),
            ],
            sources: vec![SourceConfig {
                source_type: SourceType::Mqtt,
                enabled: true,
                params: mqtt_params,
            }],
            storage: self.storage_path.map(|path| {
                let mut storage_params = HashMap::new();
                storage_params.insert("path".to_string(), path);
                neural_core::StorageConfig {
                    storage_type: neural_core::StorageType::Parquet,
                    params: storage_params,
                }
            }),
        }
    }
}
```

---

## 3. Test Cases (Specific Scenarios)

### 3.1 Unit Tests - StreamRegistry Integration Module

**File**: `/workspaces/neural-data-platform/apps/air-quality-app/tests/stream_registry_integration_test.rs`

#### Test 1: Load StreamConfig on Startup (Happy Path)
```rust
#[tokio::test]
async fn test_load_stream_config_on_startup_success() {
    // Arrange
    let mut mock_registry = MockStreamRegistry::new();
    let expected_config = StreamConfigBuilder::new("air-quality")
        .with_mqtt("mosquitto", 1883, "airgradient/readings/+")
        .with_storage("/data/parquet")
        .build();

    mock_registry
        .expect_load_stream()
        .with(eq("air-quality"))
        .times(1)
        .returning(move |_| Ok(expected_config.clone()));

    // Act
    let result = load_stream_config_from_registry(&mock_registry, "air-quality").await;

    // Assert
    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.stream_id, "air-quality");
    assert_eq!(config.enabled, true);
    assert_eq!(config.sources[0].source_type, SourceType::Mqtt);
}
```

**Expected Behavior**:
- App attempts to load stream config by stream ID
- StreamRegistry returns valid configuration
- Configuration is extracted and ready to use

---

#### Test 2: Extract MQTT Configuration from StreamConfig
```rust
#[tokio::test]
async fn test_extract_mqtt_config_from_stream_config() {
    // Arrange
    let stream_config = StreamConfigBuilder::new("air-quality")
        .with_mqtt("test-broker", 1884, "custom/topic/+")
        .build();

    // Act
    let mqtt_config = extract_mqtt_config(&stream_config);

    // Assert
    assert!(mqtt_config.is_ok());
    let config = mqtt_config.unwrap();
    assert_eq!(config.broker_url, "test-broker");
    assert_eq!(config.port, 1884);
    assert_eq!(config.topic_pattern, "custom/topic/+");
    assert_eq!(config.client_id, "air-quality-app"); // Generated
}
```

**Expected Behavior**:
- Parse StreamConfig.sources[0] (MQTT source)
- Extract MQTT parameters (broker, port, topic)
- Create neural_core::MqttConfig with proper defaults
- Return error if MQTT source not found or invalid

---

#### Test 3: Extract Storage Path from StreamConfig
```rust
#[tokio::test]
async fn test_extract_storage_path_from_stream_config() {
    // Arrange
    let stream_config = StreamConfigBuilder::new("air-quality")
        .with_storage("/custom/storage/path")
        .build();

    // Act
    let storage_path = extract_storage_path(&stream_config);

    // Assert
    assert_eq!(storage_path, "/custom/storage/path");
}
```

**Expected Behavior**:
- Extract storage path from StreamConfig.storage
- Return path string for ParquetStore initialization
- Default to "./data/parquet" if not specified

---

#### Test 4: StreamRegistry Unavailable - Fallback to AppConfig
```rust
#[tokio::test]
async fn test_stream_registry_unavailable_fallback() {
    // Arrange
    let mut mock_registry = MockStreamRegistry::new();

    mock_registry
        .expect_load_stream()
        .returning(|_| Err(ConfigError::Connection("etcd unavailable".to_string())));

    // Act
    let result = load_stream_config_with_fallback(
        Some(&mock_registry),
        "air-quality",
        &AppConfig::default_config()
    ).await;

    // Assert
    assert!(result.is_ok());
    let (mqtt_config, storage_path) = result.unwrap();
    // Should use AppConfig defaults
    assert_eq!(mqtt_config.broker_url, "localhost");
    assert_eq!(storage_path, "./data/parquet");
}
```

**Expected Behavior**:
- Attempt to load from StreamRegistry
- On connection error, log warning
- Fall back to AppConfig (existing behavior)
- App continues running

---

#### Test 5: Invalid StreamConfig - Missing Required Fields
```rust
#[tokio::test]
async fn test_invalid_stream_config_no_mqtt_source() {
    // Arrange
    let mut invalid_config = StreamConfigBuilder::new("invalid").build();
    invalid_config.sources.clear(); // Remove MQTT source

    // Act
    let result = extract_mqtt_config(&invalid_config);

    // Assert
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("No MQTT source found"));
}
```

**Expected Behavior**:
- Validation fails for incomplete config
- Returns descriptive error
- App logs error and falls back to AppConfig

---

#### Test 6: Stream Disabled in Registry
```rust
#[tokio::test]
async fn test_stream_disabled_in_registry() {
    // Arrange
    let mut mock_registry = MockStreamRegistry::new();
    let disabled_config = StreamConfigBuilder::new("air-quality")
        .disabled()
        .build();

    mock_registry
        .expect_load_stream()
        .returning(move |_| Ok(disabled_config.clone()));

    // Act
    let result = load_stream_config_from_registry(&mock_registry, "air-quality").await;

    // Assert
    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.enabled, false);
}
```

**Expected Behavior**:
- Stream config loads successfully
- Application checks `enabled` flag
- If disabled, skip stream setup or log warning
- Behavior defined by application policy

---

### 3.2 Integration Tests - Application Startup

**File**: `/workspaces/neural-data-platform/apps/air-quality-app/tests/app_startup_integration_test.rs`

#### Test 7: Full App Startup with StreamRegistry
```rust
#[tokio::test]
async fn test_app_startup_with_stream_registry_integration() {
    // Arrange
    let mut mock_registry = MockStreamRegistry::new();
    let stream_config = StreamConfigBuilder::new("air-quality")
        .with_mqtt("mosquitto", 1883, "airgradient/readings/+")
        .with_storage("/data/test-parquet")
        .build();

    mock_registry
        .expect_load_stream()
        .returning(move |_| Ok(stream_config.clone()));

    // Act - Initialize app components with registry
    let (mqtt_handler, storage_writer, api_router) = initialize_app_with_registry(
        Some(mock_registry),
        "air-quality"
    ).await.unwrap();

    // Assert
    assert!(mqtt_handler.is_some());
    assert!(storage_writer.is_some());
    assert!(api_router.is_some());

    // Verify MQTT handler uses correct config
    let health = mqtt_handler.unwrap().health_check().await;
    assert!(health.is_ok());
}
```

**Expected Behavior**:
- App startup loads stream config from registry
- MQTT handler initialized with registry config
- Storage writer initialized with registry path
- All components operational

---

#### Test 8: App Startup - Registry Connection Fails
```rust
#[tokio::test]
async fn test_app_startup_registry_connection_failure() {
    // Arrange - No registry available

    // Act
    let result = initialize_app_with_registry(None, "air-quality").await;

    // Assert
    assert!(result.is_ok());
    let (mqtt_handler, storage_writer, _) = result.unwrap();

    // Should fall back to AppConfig
    assert!(mqtt_handler.is_some()); // With default config
    assert!(storage_writer.is_some()); // With default path
}
```

**Expected Behavior**:
- App detects registry unavailable
- Logs warning about fallback
- Uses AppConfig defaults
- App starts successfully (degraded mode)

---

#### Test 9: App Startup - Invalid Stream ID
```rust
#[tokio::test]
async fn test_app_startup_nonexistent_stream() {
    // Arrange
    let mut mock_registry = MockStreamRegistry::new();

    mock_registry
        .expect_load_stream()
        .returning(|_| Err(ConfigError::NotFound("Stream not found".to_string())));

    // Act
    let result = initialize_app_with_registry(
        Some(mock_registry),
        "nonexistent-stream"
    ).await;

    // Assert
    assert!(result.is_ok());
    // Should fall back to AppConfig when stream not found
}
```

**Expected Behavior**:
- Registry returns NotFound error
- App logs warning
- Falls back to AppConfig
- App continues running

---

### 3.3 Behavior Specification Tests

**File**: `/workspaces/neural-data-platform/apps/air-quality-app/tests/stream_registry_behavior_test.rs`

#### Test 10: MQTT Config Priority - Registry > AppConfig
```rust
#[tokio::test]
async fn test_mqtt_config_priority_registry_over_appconfig() {
    // Arrange
    let registry_broker = "registry-broker";
    let appconfig_broker = "appconfig-broker";

    let stream_config = StreamConfigBuilder::new("air-quality")
        .with_mqtt(registry_broker, 1884, "registry/topic/+")
        .build();

    let app_config = AppConfig {
        mqtt: MqttConfig {
            broker_url: appconfig_broker.to_string(),
            port: 1883,
            ..Default::default()
        },
        ..Default::default()
    };

    // Act
    let final_mqtt_config = merge_mqtt_config(Some(&stream_config), &app_config);

    // Assert
    assert_eq!(final_mqtt_config.broker_url, registry_broker);
    assert_eq!(final_mqtt_config.port, 1884);
    assert_eq!(final_mqtt_config.topic_pattern, "registry/topic/+");
}
```

**Expected Behavior**:
- When StreamConfig available, use its MQTT settings
- AppConfig serves as fallback only
- No environment variable override at this level (already applied in AppConfig)

---

#### Test 11: Storage Path Priority - Registry > AppConfig
```rust
#[tokio::test]
async fn test_storage_path_priority_registry_over_appconfig() {
    // Arrange
    let registry_path = "/registry/storage/path";
    let appconfig_path = "/appconfig/storage/path";

    let stream_config = StreamConfigBuilder::new("air-quality")
        .with_storage(registry_path)
        .build();

    let app_config = AppConfig {
        storage: StorageConfig {
            base_path: appconfig_path.to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    // Act
    let final_storage_path = merge_storage_path(Some(&stream_config), &app_config);

    // Assert
    assert_eq!(final_storage_path, registry_path);
}
```

**Expected Behavior**:
- Registry storage path takes priority
- AppConfig used only when registry unavailable

---

#### Test 12: Graceful Degradation - Partial Config
```rust
#[tokio::test]
async fn test_graceful_degradation_partial_stream_config() {
    // Arrange - StreamConfig has MQTT but no storage
    let stream_config = StreamConfigBuilder::new("air-quality")
        .with_mqtt("registry-broker", 1883, "topic/+")
        // NO storage configured
        .build();

    let app_config = AppConfig::default_config();

    // Act
    let mqtt_config = extract_mqtt_config(&stream_config).unwrap();
    let storage_path = merge_storage_path(Some(&stream_config), &app_config);

    // Assert
    assert_eq!(mqtt_config.broker_url, "registry-broker");
    // Storage falls back to AppConfig
    assert_eq!(storage_path, app_config.storage.base_path);
}
```

**Expected Behavior**:
- Use registry values where available
- Fall back to AppConfig for missing fields
- Hybrid configuration approach

---

### 3.4 E2E Tests (Require Running etcd)

**File**: `/workspaces/neural-data-platform/apps/air-quality-app/tests/stream_registry_e2e_test.rs`

#### Test 13: Full E2E - Real etcd, Real StreamRegistry
```rust
#[tokio::test]
#[ignore] // Run with: cargo test --ignored
async fn test_e2e_app_startup_with_real_etcd() {
    // Prerequisites: etcd running on localhost:2379

    // Arrange - Set up real stream config in etcd
    let registry = StreamRegistry::new(&["http://localhost:2379"])
        .await
        .expect("etcd must be running for E2E test");

    let test_config = StreamConfigBuilder::new("air-quality-e2e")
        .with_mqtt("localhost", 1883, "test/e2e/+")
        .with_storage("/tmp/test-parquet")
        .build();

    registry.save_stream(&test_config).await.expect("Failed to save test config");

    // Act - Start application components
    let result = initialize_app_with_stream_id("air-quality-e2e").await;

    // Assert
    assert!(result.is_ok());

    // Cleanup
    registry.delete_stream("air-quality-e2e").await.ok();
}
```

**Expected Behavior**:
- Full integration with real etcd
- Load, parse, and apply real config
- All components start successfully
- Validates entire flow end-to-end

---

## 4. Test File Organization

```
apps/air-quality-app/
├── src/
│   ├── main.rs                          # Modified to use StreamRegistry
│   ├── stream_integration.rs            # NEW: StreamRegistry integration logic
│   └── lib.rs                           # Export stream_integration module
├── tests/
│   ├── stream_registry_integration_test.rs   # NEW: Tests 1-6
│   ├── app_startup_integration_test.rs       # NEW: Tests 7-9
│   ├── stream_registry_behavior_test.rs      # NEW: Tests 10-12
│   └── stream_registry_e2e_test.rs           # NEW: Test 13
└── Cargo.toml                           # Add mockall to dev-dependencies
```

### Cargo.toml Updates
```toml
[dev-dependencies]
mockall = "0.12"
axum-test = "14.1"
tokio-test = "0.4"
```

---

## 5. Mock Trait Definitions (Rust Implementation)

### 5.1 StreamRegistry Mock with mockall

```rust
// In tests/common/mocks.rs or inline in test files

use mockall::{predicate::*, mock};
use config_client::ConfigError;
use neural_core::StreamConfig;

mock! {
    pub StreamRegistry {
        // Constructor (note: mockall doesn't mock constructors directly,
        // so we test the behavior of methods on an already-created mock)

        pub async fn load_stream(&self, stream_id: &str) -> Result<StreamConfig, ConfigError>;
        pub async fn stream_exists(&self, stream_id: &str) -> Result<bool, ConfigError>;
        pub async fn list_streams(&self) -> Result<Vec<String>, ConfigError>;
        pub async fn load_all_streams(&self) -> Result<std::collections::HashMap<String, StreamConfig>, ConfigError>;
        pub async fn save_stream(&self, config: &StreamConfig) -> Result<(), ConfigError>;
        pub async fn delete_stream(&self, stream_id: &str) -> Result<(), ConfigError>;
        pub async fn clear_cache(&self);
        pub async fn cache_size(&self) -> usize;
    }
}
```

### 5.2 Usage Example

```rust
#[tokio::test]
async fn example_using_mock_registry() {
    // Create mock
    let mut mock_registry = MockStreamRegistry::new();

    // Set expectations
    mock_registry
        .expect_load_stream()
        .with(eq("test-stream"))
        .times(1)
        .returning(|_| {
            Ok(StreamConfigBuilder::new("test-stream").build())
        });

    // Use in test
    let result = mock_registry.load_stream("test-stream").await;
    assert!(result.is_ok());
}
```

---

## 6. Implementation Order (TDD Red-Green-Refactor)

Following London TDD, we implement in this order:

### Phase 1: Define Top-Level Behavior (Outside-In)
1. **Write Test 7** (app startup integration) - **RED** ❌
2. **Create stub function** `initialize_app_with_registry()` - **GREEN** ✅
3. **Write Test 1** (load stream config) - **RED** ❌
4. **Implement** `load_stream_config_from_registry()` - **GREEN** ✅
5. **Write Test 2** (extract MQTT config) - **RED** ❌
6. **Implement** `extract_mqtt_config()` - **GREEN** ✅
7. **Write Test 3** (extract storage path) - **RED** ❌
8. **Implement** `extract_storage_path()` - **GREEN** ✅

### Phase 2: Handle Error Cases
9. **Write Test 4** (registry unavailable) - **RED** ❌
10. **Implement** fallback logic - **GREEN** ✅
11. **Write Test 5** (invalid config) - **RED** ❌
12. **Implement** validation - **GREEN** ✅
13. **Write Test 6** (disabled stream) - **RED** ❌
14. **Implement** enabled check - **GREEN** ✅

### Phase 3: Verify Integration
15. **Write Tests 8-9** (startup failures) - **RED** ❌
16. **Refactor** error handling - **GREEN** ✅
17. **Write Tests 10-12** (behavior specs) - **RED** ❌
18. **Refactor** config merging - **GREEN** ✅

### Phase 4: E2E Validation
19. **Write Test 13** (E2E with real etcd) - **RED** ❌
20. **Debug** integration issues - **GREEN** ✅
21. **Refactor** for production readiness - **REFACTOR** ♻️

---

## 7. Success Criteria

### Tests Must Pass
- All unit tests pass without real etcd
- Integration tests pass with mocked dependencies
- E2E tests pass with real etcd (run with `--ignored`)

### Code Coverage
- **Minimum 85%** line coverage for new code
- **100%** branch coverage for error handling paths

### Behavior Validation
- ✅ App loads config from StreamRegistry on startup
- ✅ MQTT handler uses registry config (broker, port, topic)
- ✅ Storage writer uses registry path
- ✅ Graceful fallback when registry unavailable
- ✅ Validation of StreamConfig before use
- ✅ Logging of configuration decisions

### Non-Functional
- Tests run in < 5 seconds (unit + integration)
- No flaky tests (consistent pass/fail)
- Clear error messages on failures

---

## 8. Testing Commands

```bash
# Run all tests (except E2E)
cargo test --package air-quality-app

# Run specific test module
cargo test --package air-quality-app --test stream_registry_integration_test

# Run with output
cargo test --package air-quality-app -- --nocapture

# Run E2E tests (requires etcd)
cargo test --package air-quality-app --ignored

# Coverage report
cargo tarpaulin --package air-quality-app --out Html

# Watch mode (requires cargo-watch)
cargo watch -x "test --package air-quality-app"
```

---

## 9. Example Test Output (Expected)

```
running 13 tests
test test_load_stream_config_on_startup_success ... ok
test test_extract_mqtt_config_from_stream_config ... ok
test test_extract_storage_path_from_stream_config ... ok
test test_stream_registry_unavailable_fallback ... ok
test test_invalid_stream_config_no_mqtt_source ... ok
test test_stream_disabled_in_registry ... ok
test test_app_startup_with_stream_registry_integration ... ok
test test_app_startup_registry_connection_failure ... ok
test test_app_startup_nonexistent_stream ... ok
test test_mqtt_config_priority_registry_over_appconfig ... ok
test test_storage_path_priority_registry_over_appconfig ... ok
test test_graceful_degradation_partial_stream_config ... ok
test test_e2e_app_startup_with_real_etcd ... ignored

test result: ok. 12 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.34s
```

---

## 10. Dependencies & Setup

### Required Crates
```toml
[dependencies]
config-client = { path = "../../config-client" }
neural-core = { path = "../../core" }
tokio = { version = "1", features = ["full"] }
tracing = "0.1"

[dev-dependencies]
mockall = "0.12"           # For mocking
axum-test = "14.1"         # For HTTP testing
tokio-test = "0.4"         # For tokio testing utilities
```

### Test Fixtures
Create `/workspaces/neural-data-platform/apps/air-quality-app/tests/common/mod.rs`:
```rust
pub mod fixtures;
pub mod mocks;
pub mod builders;
```

---

## 11. Open Questions for Implementation

1. **Stream ID Source**: Where does the app get the stream ID to load?
   - Hardcoded to "air-quality"?
   - Environment variable `STREAM_ID`?
   - Command-line argument?

2. **Multiple Streams**: Will the app support multiple streams in the future?
   - Current plan: Single stream per app instance
   - Future: Load all streams and route by topic?

3. **Config Refresh**: Should the app watch for config changes?
   - Current: Load once at startup
   - Future: Watch etcd for updates and reload?

4. **Partial Failure**: If MQTT config invalid but storage valid, should app start?
   - Proposal: Start in "degraded mode" (storage only, no ingestion)

---

## 12. Notes for Implementer

### London TDD Reminders
- **Write the test first** - Define the interface you want
- **Make it pass** - Simplest implementation that works
- **Refactor** - Clean up while tests remain green
- **Mock collaborators** - Focus on the unit under test
- **Test behavior, not implementation** - Tests should survive refactoring

### Common Pitfalls
- ❌ Testing implementation details (private methods)
- ❌ Over-mocking (mocking things that aren't collaborators)
- ❌ Integration tests disguised as unit tests
- ❌ Tests that depend on external state (file system, network)
- ❌ Tests that don't clean up after themselves

### Best Practices
- ✅ One assertion per test (logical assertion, not physical)
- ✅ Test names describe the behavior being tested
- ✅ Arrange-Act-Assert structure
- ✅ Test both happy path and error cases
- ✅ Use builders for complex test data

---

## Summary

This test plan provides:
- **13 comprehensive tests** covering startup, config loading, fallbacks, and E2E
- **London TDD approach** with outside-in development
- **Mock definitions** using mockall for fast, isolated tests
- **Clear success criteria** and expected behaviors
- **Implementation roadmap** following TDD red-green-refactor

**Next Steps**:
1. Review and approve this test plan
2. Begin Phase 1: Write Test 7 (app startup) and make it RED
3. Implement minimum code to make it GREEN
4. Continue TDD cycle through all 13 tests
5. Run E2E test with real etcd to validate integration

**Estimated Development Time**: 2-3 days following this plan
