# SPARC Refinement Criteria: Neural Data Platform - Air Quality (air-001)

**Feature ID**: air-001
**Document Version**: 1.1.0
**Date**: 2025-12-13
**Phase**: SPARC - Refinement & Testing
**Revision**: Docker Deployment + Complete AirGradient Fields

---

## Table of Contents

1. [Test-Driven Development Strategy](#1-test-driven-development-strategy)
2. [Unit Test Specifications](#2-unit-test-specifications)
3. [Integration Test Specifications](#3-integration-test-specifications)
4. [Docker Container Testing](#4-docker-container-testing)
5. [Performance Benchmarks](#5-performance-benchmarks)
6. [Acceptance Criteria](#6-acceptance-criteria)
7. [Quality Gates](#7-quality-gates)
8. [Continuous Integration Pipeline](#8-continuous-integration-pipeline)

---

## 1. Test-Driven Development Strategy

### 1.1 TDD Workflow (Red-Green-Refactor)

```
┌─────────────────────────────────────────────────────────────────────┐
│                    TDD CYCLE FOR AIR QUALITY PLATFORM               │
│                                                                     │
│     ┌─────────┐                                                     │
│     │  RED    │ Write failing test first                           │
│     │  PHASE  │ • Define expected behavior                          │
│     └────┬────┘ • Test should fail (no implementation)              │
│          │                                                          │
│          ▼                                                          │
│     ┌─────────┐                                                     │
│     │  GREEN  │ Write minimal code to pass                         │
│     │  PHASE  │ • Implement just enough                             │
│     └────┬────┘ • Test must pass                                    │
│          │                                                          │
│          ▼                                                          │
│     ┌─────────┐                                                     │
│     │REFACTOR │ Improve without breaking                           │
│     │  PHASE  │ • Clean up code                                     │
│     └────┬────┘ • All tests still pass                              │
│          │                                                          │
│          └──────── REPEAT ──────────┘                               │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 Test Categories

| Category | Scope | Framework | Location |
|----------|-------|-----------|----------|
| Unit Tests | Single function/struct | `#[test]`, `proptest` | `src/*/tests.rs` |
| Integration Tests | Cross-module | `#[tokio::test]` | `tests/` |
| Container Tests | Docker deployment | `testcontainers` | `tests/container/` |
| E2E Tests | Full system | Custom harness | `tests/e2e/` |
| Property Tests | Edge cases | `proptest` | Inline with unit tests |
| Benchmark Tests | Performance | `criterion` | `benches/` |

### 1.3 Test Coverage Requirements

| Component | Minimum Coverage | Target Coverage |
|-----------|-----------------|-----------------|
| Core Traits | 90% | 95% |
| AirGradient Parser | 95% | 100% |
| Parquet Storage | 85% | 90% |
| MQTT Source | 80% | 90% |
| Alert Engine | 90% | 95% |
| AQI Calculations | 100% | 100% |
| Overall | 85% | 90% |

---

## 2. Unit Test Specifications

### 2.1 AirGradient Message Parser Tests

```rust
// tests/unit/parser_tests.rs

mod airgradient_parser_tests {
    use super::*;

    /// TEST: Parse complete Local API payload (29+ fields)
    #[test]
    fn test_parse_complete_local_api_payload() {
        let payload = r#"{
            "wifi": -46,
            "serialno": "ecda3b1eaaaf",
            "rco2": 447,
            "pm01": 3,
            "pm02": 7,
            "pm10": 8,
            "pm02Compensated": 6,
            "pm01Standard": 3,
            "pm02Standard": 7,
            "pm10Standard": 8,
            "pm003Count": 442,
            "pm005Count": 380,
            "pm01Count": 98,
            "pm02Count": 12,
            "pm50Count": 2,
            "pm10Count": 1,
            "atmp": 25.87,
            "atmpCompensated": 24.47,
            "rhum": 43,
            "rhumCompensated": 49,
            "tvocIndex": 100,
            "tvocRaw": 33051,
            "noxIndex": 1,
            "noxRaw": 16307,
            "boot": 6,
            "bootCount": 6,
            "ledMode": "pm",
            "firmware": "3.1.4",
            "model": "I-9PSL"
        }"#;

        let reading = parse_airgradient_message(payload.as_bytes(), DataSource::LocalAPI)
            .expect("Should parse successfully");

        // Verify all fields
        assert_eq!(reading.location_id, "ecda3b1eaaaf");
        assert_eq!(reading.co2, Some(447));
        assert_eq!(reading.pm25, Some(7));
        assert_eq!(reading.pm25_compensated, Some(6));
        assert_eq!(reading.pm003_count, Some(442));
        assert_eq!(reading.temperature, Some(25.87));
        assert_eq!(reading.temperature_compensated, Some(24.47));
        assert_eq!(reading.tvoc_index, Some(100));
        assert_eq!(reading.tvoc_raw, Some(33051));
        assert_eq!(reading.firmware, Some("3.1.4".to_string()));
        assert_eq!(reading.model, Some("I-9PSL".to_string()));
    }

    /// TEST: Parse MQTT payload (12 fields - subset)
    #[test]
    fn test_parse_mqtt_payload_subset() {
        let payload = r#"{
            "wifi": -42,
            "serialno": "ecda3b1eaaaf",
            "rco2": 825,
            "pm02": 7,
            "pm01": 4,
            "pm10": 10,
            "atmp": 23.45,
            "rhum": 55,
            "tvocIndex": 120,
            "noxIndex": 50
        }"#;

        let reading = parse_airgradient_message(payload.as_bytes(), DataSource::MQTT)
            .expect("Should parse MQTT payload");

        // Verify MQTT fields present
        assert_eq!(reading.co2, Some(825));
        assert_eq!(reading.pm25, Some(7));

        // Verify Local-API-only fields are None
        assert_eq!(reading.pm25_compensated, None);
        assert_eq!(reading.pm003_count, None);
        assert_eq!(reading.firmware, None);
    }

    /// TEST: Missing required field (serialno) should fail
    #[test]
    fn test_missing_serialno_fails() {
        let payload = r#"{"rco2": 500, "pm02": 10}"#;

        let result = parse_airgradient_message(payload.as_bytes(), DataSource::MQTT);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("serialno"));
    }

    /// TEST: Invalid JSON should fail gracefully
    #[test]
    fn test_invalid_json_fails() {
        let payload = b"not valid json {";

        let result = parse_airgradient_message(payload, DataSource::MQTT);

        assert!(result.is_err());
    }

    /// TEST: Out-of-range values trigger validation warnings
    #[test]
    fn test_out_of_range_pm25_warning() {
        let payload = r#"{
            "serialno": "test123",
            "pm02": 999999
        }"#;

        let result = parse_airgradient_message(payload.as_bytes(), DataSource::MQTT);

        // Should parse but with validation warning
        let reading = result.expect("Should parse");
        assert!(reading.quality_score < 1.0, "Quality score should be penalized");
    }

    /// PROPERTY TEST: Any valid PM2.5 value produces valid AQI
    #[proptest]
    fn prop_valid_pm25_produces_valid_aqi(pm25 in 0.0f64..500.0) {
        let aqi = calculate_aqi_from_pm25(pm25);
        prop_assert!(aqi.is_some());
        prop_assert!(aqi.unwrap() <= 500);
    }
}
```

### 2.2 AQI Calculation Tests

```rust
// tests/unit/aqi_tests.rs

mod aqi_calculation_tests {
    /// TEST: EPA AQI breakpoint calculations
    #[test]
    fn test_aqi_breakpoints_pm25() {
        // Good (0-50)
        assert_eq!(calculate_aqi_from_pm25(0.0), 0);
        assert_eq!(calculate_aqi_from_pm25(12.0), 50);

        // Moderate (51-100)
        assert_eq!(calculate_aqi_from_pm25(12.1), 51);
        assert_eq!(calculate_aqi_from_pm25(35.4), 100);

        // Unhealthy for Sensitive (101-150)
        assert_eq!(calculate_aqi_from_pm25(35.5), 101);
        assert_eq!(calculate_aqi_from_pm25(55.4), 150);

        // Unhealthy (151-200)
        assert_eq!(calculate_aqi_from_pm25(55.5), 151);
        assert_eq!(calculate_aqi_from_pm25(150.4), 200);

        // Very Unhealthy (201-300)
        assert_eq!(calculate_aqi_from_pm25(150.5), 201);
        assert_eq!(calculate_aqi_from_pm25(250.4), 300);

        // Hazardous (301-500)
        assert_eq!(calculate_aqi_from_pm25(250.5), 301);
        assert_eq!(calculate_aqi_from_pm25(500.4), 500);
    }

    /// TEST: AQI category determination
    #[test]
    fn test_aqi_category() {
        assert_eq!(aqi_to_category(25), AQICategory::Good);
        assert_eq!(aqi_to_category(75), AQICategory::Moderate);
        assert_eq!(aqi_to_category(125), AQICategory::UnhealthyForSensitive);
        assert_eq!(aqi_to_category(175), AQICategory::Unhealthy);
        assert_eq!(aqi_to_category(250), AQICategory::VeryUnhealthy);
        assert_eq!(aqi_to_category(400), AQICategory::Hazardous);
    }

    /// TEST: CO2 health thresholds
    #[test]
    fn test_co2_health_thresholds() {
        assert_eq!(co2_health_level(400), CO2Level::Excellent);
        assert_eq!(co2_health_level(600), CO2Level::Good);
        assert_eq!(co2_health_level(1000), CO2Level::Moderate);
        assert_eq!(co2_health_level(1500), CO2Level::Poor);
        assert_eq!(co2_health_level(2500), CO2Level::Unhealthy);
        assert_eq!(co2_health_level(5000), CO2Level::Dangerous);
    }
}
```

### 2.3 Parquet Storage Tests

```rust
// tests/unit/storage_tests.rs

mod parquet_storage_tests {
    use tempfile::TempDir;

    /// TEST: Write single reading to Parquet
    #[tokio::test]
    async fn test_write_single_reading() {
        let temp_dir = TempDir::new().unwrap();
        let store = ParquetStore::new(temp_dir.path()).await.unwrap();

        let reading = create_test_reading();
        store.append(reading.clone()).await.unwrap();
        store.flush().await.unwrap();

        // Verify file created
        let partition_path = temp_dir.path()
            .join(&reading.location_id)
            .join("2025/12/13.parquet");
        assert!(partition_path.exists());
    }

    /// TEST: Query range returns correct results
    #[tokio::test]
    async fn test_query_range() {
        let temp_dir = TempDir::new().unwrap();
        let store = ParquetStore::new(temp_dir.path()).await.unwrap();

        // Insert 100 readings over 24 hours
        let readings = create_test_readings(100, Duration::hours(24));
        for reading in &readings {
            store.append(reading.clone()).await.unwrap();
        }
        store.flush().await.unwrap();

        // Query middle 50%
        let start = readings[25].timestamp;
        let end = readings[75].timestamp;

        let results = store.query_range("test-sensor", start, end, QueryFilters::default())
            .await.unwrap();

        assert!(results.len() >= 49);
        assert!(results.len() <= 51);
    }

    /// TEST: Aggregation produces correct statistics
    #[tokio::test]
    async fn test_aggregation() {
        let temp_dir = TempDir::new().unwrap();
        let store = ParquetStore::new(temp_dir.path()).await.unwrap();

        // Insert readings with known values
        let readings = vec![
            create_reading_with_pm25(10.0),
            create_reading_with_pm25(20.0),
            create_reading_with_pm25(30.0),
        ];
        for reading in &readings {
            store.append(reading.clone()).await.unwrap();
        }
        store.flush().await.unwrap();

        let aggregated = store.aggregate(
            "test-sensor",
            readings[0].timestamp - Duration::minutes(1),
            readings[2].timestamp + Duration::minutes(1),
            AggregationType::Mean,
            Duration::hours(1),
        ).await.unwrap();

        assert!((aggregated[0].value - 20.0).abs() < 0.01);
    }

    /// TEST: Compaction merges files correctly
    #[tokio::test]
    async fn test_compaction() {
        let temp_dir = TempDir::new().unwrap();
        let store = ParquetStore::new(temp_dir.path()).await.unwrap();

        // Insert in multiple batches (creates multiple files)
        for batch in 0..5 {
            let reading = create_test_reading();
            store.append(reading).await.unwrap();
            store.flush().await.unwrap();
        }

        // Run compaction
        let stats = store.compact().await.unwrap();

        assert!(stats.files_merged >= 4);
        assert_eq!(stats.files_created, 1);
        assert!(stats.compression_ratio > 1.0);
    }
}
```

### 2.4 Validation Tests

```rust
// tests/unit/validation_tests.rs

mod validation_tests {
    /// TEST: Valid reading passes all checks
    #[test]
    fn test_valid_reading_passes() {
        let reading = AirQualityReading {
            location_id: "ecda3b1eaaaf".to_string(),
            timestamp: Utc::now(),
            pm25: Some(15.0),
            pm10: Some(25.0),
            co2: Some(600),
            temperature: Some(22.5),
            humidity: Some(45.0),
            ..Default::default()
        };

        let result = validate_reading(&reading);

        assert!(result.is_valid);
        assert!(result.errors.is_empty());
        assert!(result.quality_score > 0.8);
    }

    /// TEST: PM2.5 > PM10 is physically inconsistent
    #[test]
    fn test_pm25_greater_than_pm10_warning() {
        let reading = AirQualityReading {
            location_id: "test".to_string(),
            timestamp: Utc::now(),
            pm25: Some(50.0),
            pm10: Some(30.0),  // PM2.5 > PM10 is impossible
            ..Default::default()
        };

        let result = validate_reading(&reading);

        assert!(result.is_valid);  // Still valid but...
        assert!(!result.warnings.is_empty());  // Has warning
        assert!(result.quality_score < 1.0);  // Quality penalized
    }

    /// TEST: CO2 below atmospheric minimum is invalid
    #[test]
    fn test_co2_below_atmospheric_minimum() {
        let reading = AirQualityReading {
            location_id: "test".to_string(),
            timestamp: Utc::now(),
            co2: Some(200),  // Atmospheric CO2 is ~420ppm
            ..Default::default()
        };

        let result = validate_reading(&reading);

        assert!(!result.warnings.is_empty());
    }

    /// TEST: Future timestamp is rejected
    #[test]
    fn test_future_timestamp_rejected() {
        let reading = AirQualityReading {
            location_id: "test".to_string(),
            timestamp: Utc::now() + Duration::hours(1),
            pm25: Some(10.0),
            ..Default::default()
        };

        let result = validate_reading(&reading);

        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("future")));
    }
}
```

### 2.5 Configuration Management Tests (config-store)

```rust
// tests/unit/config_tests.rs

mod config_store_tests {
    use config_store::{
        ConfigStore, ConfigValue, ConfigError, ConfigNode,
        InMemoryConfigStore, SecureInMemoryConfigStore,
        loaders::GitOpsLoader,
        validation::SchemaValidator,
        security::{SecretBlocker, InputValidator},
    };

    /// TEST: Load YAML configuration files
    #[tokio::test]
    async fn test_load_yaml_configuration() {
        let yaml_content = r#"
air_quality:
  sensors:
    - serial: "ecda3b1eaaaf"
      name: "Living Room"
      location_id: "living-room"
      data_source: "both"
  thresholds:
    co2:
      good: 800
      moderate: 1000
      poor: 1500
"#;

        let config_value = serde_yaml::from_str::<serde_yaml::Value>(yaml_content)
            .expect("Should parse YAML");

        assert!(config_value["air_quality"]["sensors"].is_sequence());
        assert_eq!(
            config_value["air_quality"]["thresholds"]["co2"]["good"].as_u64(),
            Some(800)
        );
    }

    /// TEST: Environment variable substitution
    #[test]
    fn test_env_var_substitution() {
        std::env::set_var("TEST_MQTT_URL", "mqtt://testbroker:1883");

        let input = "${TEST_MQTT_URL}";
        let result = substitute_env_vars(input);

        assert_eq!(result, "mqtt://testbroker:1883");

        std::env::remove_var("TEST_MQTT_URL");
    }

    /// TEST: Environment variable with default value
    #[test]
    fn test_env_var_with_default() {
        // Ensure variable is not set
        std::env::remove_var("NONEXISTENT_VAR");

        let input = "${NONEXISTENT_VAR:default_value}";
        let result = substitute_env_vars(input);

        assert_eq!(result, "default_value");
    }

    /// TEST: GitOps base + overlay merging
    #[tokio::test]
    async fn test_gitops_base_overlay_merge() {
        let temp_dir = TempDir::new().unwrap();

        // Create base config
        let base_path = temp_dir.path().join("base");
        std::fs::create_dir_all(&base_path).unwrap();
        std::fs::write(
            base_path.join("air-quality.yaml"),
            r#"
air_quality:
  thresholds:
    co2:
      good: 800
      poor: 1500
  alerting:
    enabled: true
"#,
        ).unwrap();

        // Create overlay config
        let overlay_path = temp_dir.path().join("overlays/production");
        std::fs::create_dir_all(&overlay_path).unwrap();
        std::fs::write(
            overlay_path.join("overrides.yaml"),
            r#"
air_quality:
  thresholds:
    co2:
      good: 600  # Stricter threshold for production
"#,
        ).unwrap();

        let gitops_loader = GitOpsLoader::new(temp_dir.path(), "production");
        let base = gitops_loader.load_base_configs().await.unwrap();
        let overlay = gitops_loader.load_overlay_configs().await.unwrap();

        let merged = deep_merge(base, overlay);

        // Overlay should override base
        assert_eq!(merged["air_quality"]["thresholds"]["co2"]["good"], 600);
        // Base values should remain if not overridden
        assert_eq!(merged["air_quality"]["thresholds"]["co2"]["poor"], 1500);
        // Non-overridden sections should remain
        assert_eq!(merged["air_quality"]["alerting"]["enabled"], true);
    }

    /// TEST: SecretBlocker prevents password storage
    #[tokio::test]
    async fn test_secret_blocker_blocks_password() {
        let store = SecureInMemoryConfigStore::new();

        let result = store.set(
            "/air-quality/mqtt/password",
            ConfigValue::String("secret123".to_string())
        ).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConfigError::SecurityViolation(_)
        ));
    }

    /// TEST: SecretBlocker blocks API key patterns
    #[tokio::test]
    async fn test_secret_blocker_blocks_api_key() {
        let store = SecureInMemoryConfigStore::new();

        // GitHub token pattern
        let result = store.set(
            "/air-quality/github_token",
            ConfigValue::String("ghp_abcdefghijklmnopqrstuvwxyz123456".to_string())
        ).await;

        assert!(result.is_err());
    }

    /// TEST: InputValidator blocks path traversal
    #[test]
    fn test_input_validator_blocks_path_traversal() {
        let validator = InputValidator::new();

        let result = validator.validate_key("../../../etc/passwd");

        assert!(result.is_err());
    }

    /// TEST: Schema validation catches invalid config
    #[tokio::test]
    async fn test_schema_validation_invalid_config() {
        let schema = r#"{
            "type": "object",
            "properties": {
                "thresholds": {
                    "type": "object",
                    "properties": {
                        "co2": {
                            "type": "object",
                            "properties": {
                                "good": { "type": "integer", "minimum": 0, "maximum": 10000 }
                            },
                            "required": ["good"]
                        }
                    }
                }
            }
        }"#;

        let validator = SchemaValidator::from_string(schema).unwrap();

        // Invalid: co2.good is negative
        let invalid_config = serde_json::json!({
            "thresholds": {
                "co2": {
                    "good": -100
                }
            }
        });

        let result = validator.validate(&invalid_config);
        assert!(result.is_err());
    }

    /// TEST: Hierarchical path storage and retrieval
    #[tokio::test]
    async fn test_hierarchical_config_paths() {
        let store = InMemoryConfigStore::new();

        // Store nested configuration
        store.set("/air-quality/thresholds/co2/good", ConfigValue::Integer(800)).await.unwrap();
        store.set("/air-quality/thresholds/co2/poor", ConfigValue::Integer(1500)).await.unwrap();
        store.set("/air-quality/thresholds/pm25/good", ConfigValue::Float(12.0)).await.unwrap();

        // Retrieve individual values
        let co2_good = store.get("/air-quality/thresholds/co2/good").await.unwrap();
        assert_eq!(co2_good, ConfigValue::Integer(800));

        // List keys under prefix
        let co2_keys = store.list_keys("/air-quality/thresholds/co2").await.unwrap();
        assert!(co2_keys.contains(&"/air-quality/thresholds/co2/good".to_string()));
        assert!(co2_keys.contains(&"/air-quality/thresholds/co2/poor".to_string()));

        // Get tree structure
        let thresholds_tree = store.get_tree("/air-quality/thresholds").await.unwrap();
        assert!(thresholds_tree.contains_key("co2"));
        assert!(thresholds_tree.contains_key("pm25"));
    }

    /// TEST: Configuration versioning
    #[tokio::test]
    async fn test_config_versioning() {
        let store = InMemoryConfigStore::new();

        // Set initial value
        store.set("/air-quality/thresholds/co2/good", ConfigValue::Integer(800)).await.unwrap();

        // Update value
        store.set("/air-quality/thresholds/co2/good", ConfigValue::Integer(700)).await.unwrap();

        // Update again
        store.set("/air-quality/thresholds/co2/good", ConfigValue::Integer(600)).await.unwrap();

        // Retrieve version history
        let history = store.get_history("/air-quality/thresholds/co2/good").await.unwrap();
        assert!(history.len() >= 2);

        // Get specific version
        let v1 = store.get_version("/air-quality/thresholds/co2/good", 1).await.unwrap();
        assert_eq!(v1.value, ConfigValue::Integer(800));
    }

    /// TEST: Configuration hot-reload detection
    #[tokio::test]
    async fn test_config_watch_detects_changes() {
        let store = Arc::new(InMemoryConfigStore::new());
        let changes_detected = Arc::new(AtomicUsize::new(0));
        let changes_clone = changes_detected.clone();

        // Start watching
        let (tx, mut rx) = tokio::sync::mpsc::channel(10);

        // Simulate file change notification
        store.set("/air-quality/thresholds/co2/good", ConfigValue::Integer(800)).await.unwrap();

        // In real implementation, this would be triggered by file watcher
        tx.send(("/air-quality/thresholds/co2/good".to_string(), ConfigValue::Integer(600))).await.unwrap();

        // Verify change detected
        let change = rx.recv().await.unwrap();
        assert_eq!(change.0, "/air-quality/thresholds/co2/good");
    }

    /// PROPERTY TEST: Any valid YAML config can be loaded and retrieved
    #[proptest]
    fn prop_yaml_roundtrip(
        #[strategy(any::<u16>())] co2_threshold: u16,
        #[strategy("[a-z]{8}")] sensor_name: String,
    ) {
        let yaml = format!(r#"
air_quality:
  thresholds:
    co2:
      good: {}
  sensors:
    - name: "{}"
"#, co2_threshold, sensor_name);

        let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();

        prop_assert_eq!(
            parsed["air_quality"]["thresholds"]["co2"]["good"].as_u64(),
            Some(co2_threshold as u64)
        );
        prop_assert_eq!(
            parsed["air_quality"]["sensors"][0]["name"].as_str(),
            Some(sensor_name.as_str())
        );
    }
}
```

### 2.6 Configuration Integration Test

```rust
// tests/integration/config_integration.rs

#[tokio::test]
async fn test_full_config_loading_pipeline() {
    // Create test configuration directory structure
    let temp_dir = TempDir::new().unwrap();
    setup_test_config_structure(&temp_dir);

    // Set environment variables
    std::env::set_var("MQTT_BROKER_URL", "mqtt://test:1883");
    std::env::set_var("ENVIRONMENT", "test");

    // Load configuration using ConfigManager
    let config_manager = ConfigManager::new(
        temp_dir.path().to_str().unwrap(),
        "test"
    ).await.expect("Should load configuration");

    // Verify typed access works
    let thresholds: ThresholdConfig = config_manager
        .get("/air-quality/thresholds")
        .await
        .expect("Should get thresholds");

    assert!(thresholds.co2.good > 0);
    assert!(thresholds.pm25.good > 0.0);

    // Verify sensor config
    let sensors: Vec<SensorConfig> = config_manager
        .get("/air-quality/sensors")
        .await
        .expect("Should get sensors");

    assert!(!sensors.is_empty());

    // Cleanup
    std::env::remove_var("MQTT_BROKER_URL");
    std::env::remove_var("ENVIRONMENT");
}

fn setup_test_config_structure(temp_dir: &TempDir) {
    let base_path = temp_dir.path().join("base");
    std::fs::create_dir_all(&base_path).unwrap();

    std::fs::write(
        base_path.join("air-quality.yaml"),
        r#"
air_quality:
  sensors:
    - serial: "test123"
      name: "Test Sensor"
      location_id: "test-location"
      data_source: "mqtt"
      enabled: true

  ingestion:
    mqtt:
      broker_url: "${MQTT_BROKER_URL}"
      topic_pattern: "airgradient/readings/{serial}"

  thresholds:
    co2:
      good: 800
      moderate: 1000
      poor: 1500
    pm25:
      good: 12.0
      moderate: 35.4

  alerting:
    enabled: true
    channels:
      - type: log
        level: warn
"#,
    ).unwrap();

    // Create overlay directory
    let overlay_path = temp_dir.path().join("overlays/test");
    std::fs::create_dir_all(&overlay_path).unwrap();

    std::fs::write(
        overlay_path.join("overrides.yaml"),
        r#"
air_quality:
  alerting:
    enabled: false  # Disable alerts in test
"#,
    ).unwrap();
}
```

---

## 3. Integration Test Specifications

### 3.1 MQTT to Storage Pipeline Test

```rust
// tests/integration/mqtt_to_storage.rs

#[tokio::test]
async fn test_mqtt_ingestion_pipeline() {
    // Start test MQTT broker
    let broker = TestMqttBroker::start().await;

    // Start storage
    let temp_dir = TempDir::new().unwrap();
    let storage = ParquetStore::new(temp_dir.path()).await.unwrap();

    // Start ingestion pipeline
    let pipeline = IngestionPipeline::new(
        MqttSource::new(&broker.url(), "airgradient/+/readings"),
        storage.clone(),
    );
    let _handle = pipeline.start().await.unwrap();

    // Publish test message
    let test_payload = create_complete_airgradient_json();
    broker.publish("airgradient/ecda3b1eaaaf/readings", &test_payload).await;

    // Wait for processing
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify stored
    let results = storage.query_range(
        "ecda3b1eaaaf",
        Utc::now() - Duration::minutes(1),
        Utc::now() + Duration::minutes(1),
        QueryFilters::default(),
    ).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].pm25, Some(7));
}
```

### 3.2 Alert System Integration Test

```rust
// tests/integration/alert_system.rs

#[tokio::test]
async fn test_threshold_alert_triggers() {
    let alert_receiver = TestAlertReceiver::new();
    let alert_engine = AlertEngine::new(
        vec![
            ThresholdRule {
                id: "pm25-unhealthy".to_string(),
                field: "pm25".to_string(),
                operator: Operator::GreaterThan,
                value: 35.5,
                severity: AlertSeverity::High,
            },
        ],
        alert_receiver.clone(),
    );

    // Send reading below threshold - no alert
    let normal_reading = create_reading_with_pm25(20.0);
    alert_engine.check(normal_reading).await;
    assert!(alert_receiver.alerts().is_empty());

    // Send reading above threshold - should alert
    let unhealthy_reading = create_reading_with_pm25(50.0);
    alert_engine.check(unhealthy_reading).await;

    let alerts = alert_receiver.alerts();
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].severity, AlertSeverity::High);
    assert!(alerts[0].message.contains("PM2.5"));
}
```

### 3.3 Forecasting Pipeline Test

```rust
// tests/integration/forecasting.rs

#[tokio::test]
async fn test_forecast_generation() {
    // Create historical data (7 days of hourly readings)
    let temp_dir = TempDir::new().unwrap();
    let storage = ParquetStore::new(temp_dir.path()).await.unwrap();

    let historical_readings = generate_synthetic_air_quality_data(
        Duration::days(7),
        Duration::hours(1),
    );
    for reading in &historical_readings {
        storage.append(reading.clone()).await.unwrap();
    }
    storage.flush().await.unwrap();

    // Load model (or train if needed)
    let model = FannForecaster::load_or_train("pm25", &storage).await.unwrap();

    // Generate forecast
    let forecast = model.predict(
        "ecda3b1eaaaf",
        &storage,
        24,  // 24-hour forecast
    ).await.unwrap();

    assert_eq!(forecast.len(), 24);
    for point in &forecast {
        assert!(point.confidence > 0.0);
        assert!(point.confidence <= 1.0);
        assert!(point.predicted_value >= 0.0);
        assert!(point.predicted_value <= 500.0);
    }
}
```

---

## 4. Docker Container Testing

### 4.1 Container Build Test

```rust
// tests/container/build_test.rs

use testcontainers::{clients, images::generic::GenericImage};

#[tokio::test]
async fn test_container_builds_successfully() {
    let docker = clients::Cli::default();

    // Build image
    let build_output = std::process::Command::new("docker")
        .args(&["build", "-t", "neural-air-quality:test", "."])
        .output()
        .expect("Failed to build Docker image");

    assert!(build_output.status.success(),
        "Docker build failed: {}",
        String::from_utf8_lossy(&build_output.stderr)
    );
}

#[tokio::test]
async fn test_container_starts_and_healthy() {
    let docker = clients::Cli::default();

    // Start container
    let image = GenericImage::new("neural-air-quality", "test")
        .with_env_var("LOG_LEVEL", "debug")
        .with_env_var("DATA_SOURCE", "none")  // No external deps for test
        .with_exposed_port(8080);

    let container = docker.run(image);
    let port = container.get_host_port_ipv4(8080);

    // Wait for startup
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Check health endpoint
    let client = reqwest::Client::new();
    let response = client
        .get(&format!("http://localhost:{}/health", port))
        .send()
        .await
        .expect("Failed to reach health endpoint");

    assert!(response.status().is_success());

    let health: HealthResponse = response.json().await.unwrap();
    assert_eq!(health.status, "healthy");
}
```

### 4.2 Docker Compose Integration Test

```rust
// tests/container/compose_test.rs

#[tokio::test]
async fn test_docker_compose_stack() {
    // Start compose stack
    let compose_output = std::process::Command::new("docker")
        .args(&["compose", "-f", "docker-compose.test.yml", "up", "-d"])
        .output()
        .expect("Failed to start compose stack");

    assert!(compose_output.status.success());

    // Wait for services to be ready
    tokio::time::sleep(Duration::from_secs(30)).await;

    // Test MQTT connectivity
    let mqtt_client = MqttClient::connect("mqtt://localhost:1883").await.unwrap();

    // Test application API
    let api_response = reqwest::get("http://localhost:8080/health")
        .await
        .expect("Failed to reach API");
    assert!(api_response.status().is_success());

    // Cleanup
    std::process::Command::new("docker")
        .args(&["compose", "-f", "docker-compose.test.yml", "down", "-v"])
        .output()
        .expect("Failed to stop compose stack");
}
```

### 4.3 Multi-Architecture Build Test

```bash
#!/bin/bash
# tests/container/multiarch_test.sh

set -e

echo "=== Testing Multi-Architecture Build ==="

# Setup buildx
docker buildx create --name multiarch-test --use || true

# Build for both architectures
docker buildx build \
    --platform linux/amd64,linux/arm64 \
    --tag neural-air-quality:multiarch-test \
    --load \
    .

# Verify amd64 manifest
docker buildx imagetools inspect neural-air-quality:multiarch-test | grep -q "linux/amd64"
echo "✓ amd64 build verified"

# Verify arm64 manifest
docker buildx imagetools inspect neural-air-quality:multiarch-test | grep -q "linux/arm64"
echo "✓ arm64 build verified"

echo "=== Multi-Architecture Build Test PASSED ==="
```

---

## 5. Performance Benchmarks

### 5.1 Benchmark Configuration

```toml
# Cargo.toml
[dev-dependencies]
criterion = { version = "0.5", features = ["async_tokio"] }

[[bench]]
name = "air_quality_benchmarks"
harness = false
```

### 5.2 Benchmark Implementations

```rust
// benches/air_quality_benchmarks.rs

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};

fn parser_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser");

    // Complete payload (29 fields)
    let complete_payload = create_complete_airgradient_json();
    group.throughput(Throughput::Bytes(complete_payload.len() as u64));

    group.bench_function("parse_complete_payload", |b| {
        b.iter(|| {
            parse_airgradient_message(
                complete_payload.as_bytes(),
                DataSource::LocalAPI
            ).unwrap()
        })
    });

    // MQTT payload (12 fields)
    let mqtt_payload = create_mqtt_airgradient_json();
    group.bench_function("parse_mqtt_payload", |b| {
        b.iter(|| {
            parse_airgradient_message(
                mqtt_payload.as_bytes(),
                DataSource::MQTT
            ).unwrap()
        })
    });

    group.finish();
}

fn storage_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage");
    let rt = tokio::runtime::Runtime::new().unwrap();

    // Benchmark batch sizes
    for batch_size in [10, 100, 1000, 10000] {
        group.throughput(Throughput::Elements(batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("append_batch", batch_size),
            &batch_size,
            |b, &size| {
                let temp_dir = TempDir::new().unwrap();
                let store = rt.block_on(async {
                    ParquetStore::new(temp_dir.path()).await.unwrap()
                });
                let readings: Vec<_> = (0..size).map(|_| create_test_reading()).collect();

                b.iter(|| {
                    rt.block_on(async {
                        store.append_batch(readings.clone()).await.unwrap()
                    })
                })
            },
        );
    }

    group.finish();
}

fn aqi_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("aqi");

    group.bench_function("calculate_aqi_full", |b| {
        let reading = create_complete_reading();
        b.iter(|| calculate_aqi(&reading))
    });

    group.finish();
}

criterion_group!(benches, parser_benchmarks, storage_benchmarks, aqi_benchmarks);
criterion_main!(benches);
```

### 5.3 Performance Targets

| Operation | Target Latency (p99) | Target Throughput |
|-----------|---------------------|-------------------|
| Parse complete message | < 100μs | > 10,000/sec |
| Parse MQTT message | < 50μs | > 20,000/sec |
| Calculate AQI | < 10μs | > 100,000/sec |
| Append single reading | < 1ms | > 1,000/sec |
| Append batch (1000) | < 100ms | > 10,000/sec |
| Query range (1 day) | < 100ms | N/A |
| Query range (7 days) | < 500ms | N/A |
| Forecast generation (24h) | < 5s | N/A |
| Container startup | < 30s | N/A |
| Health check | < 10ms | N/A |

### 5.4 Resource Usage Targets (Raspberry Pi 5)

| Resource | Idle | Active | Maximum |
|----------|------|--------|---------|
| CPU | < 5% | < 30% | < 80% |
| Memory | < 256MB | < 512MB | < 1.5GB |
| Disk I/O | < 1MB/s | < 10MB/s | < 50MB/s |
| Network | < 100KB/s | < 500KB/s | < 5MB/s |

---

## 6. Acceptance Criteria

### 6.1 Functional Acceptance Criteria

| ID | Criteria | Test Method |
|----|----------|-------------|
| FA-01 | System ingests AirGradient messages via MQTT | Integration test |
| FA-02 | System ingests AirGradient messages via Local HTTP API | Integration test |
| FA-03 | All 29+ sensor fields are captured when available | Unit test |
| FA-04 | AQI calculated correctly per EPA breakpoints | Unit test |
| FA-05 | Data persisted to Parquet files | Integration test |
| FA-06 | Historical queries return correct results | Integration test |
| FA-07 | Alerts triggered on threshold exceedance | Integration test |
| FA-08 | Forecasts generated with confidence intervals | Integration test |
| FA-09 | REST API returns correct responses | E2E test |
| FA-10 | Health endpoint reports accurate status | E2E test |

### 6.2 Non-Functional Acceptance Criteria

| ID | Criteria | Test Method |
|----|----------|-------------|
| NF-01 | Docker image builds for amd64 and arm64 | CI pipeline |
| NF-02 | Container starts within 30 seconds | Container test |
| NF-03 | System handles 1 reading/second sustained | Load test |
| NF-04 | Memory usage < 2GB under normal load | Benchmark |
| NF-05 | Storage efficient (< 1KB per reading) | Unit test |
| NF-06 | Graceful degradation on MQTT disconnect | Integration test |
| NF-07 | No data loss on container restart | Integration test |
| NF-08 | 85%+ test coverage | CI pipeline |

### 6.3 Definition of Done

A feature is considered "done" when:

1. **Code Complete**
   - [ ] Implementation matches pseudocode specification
   - [ ] All unit tests pass
   - [ ] All integration tests pass
   - [ ] Code reviewed and approved

2. **Quality Gates Passed**
   - [ ] Test coverage >= 85%
   - [ ] No critical/high severity bugs
   - [ ] No security vulnerabilities (cargo audit)
   - [ ] Clippy warnings resolved

3. **Documentation**
   - [ ] API documentation updated
   - [ ] README updated if applicable
   - [ ] Configuration examples provided

4. **Deployment Ready**
   - [ ] Docker image builds successfully
   - [ ] Multi-architecture verified
   - [ ] docker-compose.yml updated if needed
   - [ ] Configuration validated

---

## 7. Quality Gates

### 7.1 Pre-Commit Gates

```yaml
# .pre-commit-config.yaml
repos:
  - repo: local
    hooks:
      - id: cargo-fmt
        name: cargo fmt
        entry: cargo fmt --all -- --check
        language: system
        pass_filenames: false

      - id: cargo-clippy
        name: cargo clippy
        entry: cargo clippy --all-targets -- -D warnings
        language: system
        pass_filenames: false

      - id: cargo-test
        name: cargo test (fast)
        entry: cargo test --lib
        language: system
        pass_filenames: false
```

### 7.2 CI Quality Gates

| Gate | Tool | Threshold | Block on Failure |
|------|------|-----------|------------------|
| Compilation | `cargo build` | Success | Yes |
| Formatting | `cargo fmt --check` | No changes | Yes |
| Linting | `cargo clippy` | No warnings | Yes |
| Unit Tests | `cargo test` | 100% pass | Yes |
| Coverage | `cargo llvm-cov` | >= 85% | Yes |
| Security | `cargo audit` | No vulnerabilities | Yes |
| Docker Build | `docker build` | Success | Yes |
| Integration Tests | `cargo test --test '*'` | 100% pass | Yes |

### 7.3 Release Gates

| Gate | Criteria | Verification |
|------|----------|--------------|
| Version Tagged | Semantic version in Cargo.toml | Automated |
| Changelog Updated | Entry for version | Manual review |
| Multi-arch Images | amd64 + arm64 built | CI verification |
| Performance | Benchmarks within targets | CI verification |
| Documentation | API docs generated | CI verification |

---

## 8. Continuous Integration Pipeline

### 8.1 GitHub Actions Workflow

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  check:
    name: Check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo check --all-targets

  fmt:
    name: Format
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt --all -- --check

  clippy:
    name: Clippy
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo clippy --all-targets -- -D warnings

  test:
    name: Test
    runs-on: ubuntu-latest
    services:
      mosquitto:
        image: eclipse-mosquitto:2
        ports:
          - 1883:1883
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Run tests
        run: cargo test --all
      - name: Run integration tests
        run: cargo test --test '*' -- --test-threads=1

  coverage:
    name: Coverage
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: llvm-tools-preview
      - uses: Swatinem/rust-cache@v2
      - uses: taiki-e/install-action@cargo-llvm-cov
      - name: Generate coverage
        run: cargo llvm-cov --all-features --lcov --output-path lcov.info
      - name: Check coverage threshold
        run: |
          COVERAGE=$(cargo llvm-cov --all-features --json | jq '.data[0].totals.lines.percent')
          if (( $(echo "$COVERAGE < 85" | bc -l) )); then
            echo "Coverage $COVERAGE% is below threshold of 85%"
            exit 1
          fi

  security:
    name: Security Audit
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: rustsec/audit-check@v1.4.1
        with:
          token: ${{ secrets.GITHUB_TOKEN }}

  docker:
    name: Docker Build
    runs-on: ubuntu-latest
    needs: [check, test]
    steps:
      - uses: actions/checkout@v4
      - uses: docker/setup-buildx-action@v3
      - name: Build Docker image
        uses: docker/build-push-action@v5
        with:
          context: .
          push: false
          tags: neural-air-quality:test
          cache-from: type=gha
          cache-to: type=gha,mode=max
      - name: Test container
        run: |
          docker run -d --name test-container -p 8080:8080 neural-air-quality:test
          sleep 10
          curl -f http://localhost:8080/health
          docker stop test-container

  docker-multiarch:
    name: Multi-arch Build
    runs-on: ubuntu-latest
    needs: [docker]
    if: github.ref == 'refs/heads/main'
    steps:
      - uses: actions/checkout@v4
      - uses: docker/setup-qemu-action@v3
      - uses: docker/setup-buildx-action@v3
      - name: Build multi-arch
        uses: docker/build-push-action@v5
        with:
          context: .
          platforms: linux/amd64,linux/arm64
          push: false
          tags: neural-air-quality:latest
```

---

## Summary

This refinement criteria document establishes comprehensive testing standards for the Neural Data Platform Air Quality feature:

1. **TDD Strategy**: Red-Green-Refactor cycle with 85%+ coverage requirement
2. **Unit Tests**: Complete coverage of parser, AQI calculations, storage, and validation
3. **Integration Tests**: MQTT pipeline, alert system, and forecasting verification
4. **Container Tests**: Docker build, health checks, and multi-architecture validation
5. **Performance**: Benchmarks with specific latency and throughput targets
6. **Quality Gates**: Pre-commit, CI, and release gates ensuring code quality
7. **CI Pipeline**: Automated GitHub Actions workflow for continuous verification

All tests are designed to support Docker-based deployment and validate the complete 29+ field AirGradient sensor data set.
