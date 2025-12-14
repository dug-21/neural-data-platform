# SPARC Refinement Criteria: Air Quality Intelligence Platform (air-001)

## Executive Summary

This document defines the quality gates, testing strategy, and refinement criteria for the Neural Data Platform Air Quality feature. It ensures the platform meets production standards for reliability, performance, and maintainability across macOS (M4) and Raspberry Pi 5 deployments.

**Version:** 1.0.0
**Last Updated:** 2025-12-13
**Status:** Draft
**Feature ID:** air-001

---

## 1. Testing Strategy

### 1.1 Unit Testing

#### 1.1.1 Core Domain Logic

**Target Coverage:** 95%+

```rust
// tests/unit/aqi_calculation_test.rs
#[cfg(test)]
mod aqi_tests {
    use air_quality_core::analysis::aqi::AqiCalculator;

    #[test]
    fn test_epa_aqi_co2_ranges() {
        // Test known EPA AQI values
        let calculator = AqiCalculator::new();

        // Good (0-50)
        assert_eq!(calculator.co2_to_aqi(400.0), 25);
        assert_eq!(calculator.co2_to_aqi(600.0), 42);

        // Moderate (51-100)
        assert_eq!(calculator.co2_to_aqi(800.0), 67);
        assert_eq!(calculator.co2_to_aqi(1000.0), 92);

        // Unhealthy for Sensitive (101-150)
        assert_eq!(calculator.co2_to_aqi(1200.0), 117);
        assert_eq!(calculator.co2_to_aqi(1500.0), 142);

        // Unhealthy (151-200)
        assert_eq!(calculator.co2_to_aqi(2000.0), 175);
    }

    #[test]
    fn test_pm25_aqi_accuracy() {
        let calculator = AqiCalculator::new();

        // Test against EPA reference values
        // https://www.airnow.gov/aqi/aqi-calculator-concentration/
        assert_eq!(calculator.pm25_to_aqi(0.0), 0);
        assert_eq!(calculator.pm25_to_aqi(12.0), 50);
        assert_eq!(calculator.pm25_to_aqi(35.4), 100);
        assert_eq!(calculator.pm25_to_aqi(55.4), 150);
        assert_eq!(calculator.pm25_to_aqi(150.4), 200);
        assert_eq!(calculator.pm25_to_aqi(250.4), 300);
    }

    #[test]
    fn test_composite_aqi_max_dominates() {
        let calculator = AqiCalculator::new();
        let reading = AirQualityReading {
            co2: 500.0,    // AQI ~35
            pm25: 55.4,    // AQI 150
            pm10: 50.0,    // AQI ~45
            ..Default::default()
        };

        // Composite AQI should be the maximum
        assert_eq!(calculator.calculate_aqi(&reading), 150);
    }

    #[test]
    fn test_edge_cases() {
        let calculator = AqiCalculator::new();

        // Zero values
        assert_eq!(calculator.co2_to_aqi(0.0), 0);

        // Extremely high values
        assert!(calculator.pm25_to_aqi(999.9) >= 500);

        // NaN handling
        assert!(calculator.co2_to_aqi(f64::NAN).is_err());
    }
}
```

#### 1.1.2 Parquet Storage Layer

**Focus:** Read/Write/Query correctness

```rust
// tests/unit/parquet_storage_test.rs
#[tokio::test]
async fn test_parquet_write_and_read() {
    let temp_dir = tempdir().unwrap();
    let store = ParquetStore::new(temp_dir.path()).await.unwrap();

    // Write test data
    let readings = vec![
        AirQualityReading {
            timestamp: Utc::now(),
            co2: 650.0,
            pm25: 12.5,
            temperature: 22.3,
            ..Default::default()
        },
        // ... more test data
    ];

    store.write_batch(&readings).await.unwrap();

    // Read back
    let retrieved = store.query_range(
        Utc::now() - Duration::hours(1),
        Utc::now(),
    ).await.unwrap();

    assert_eq!(retrieved.len(), readings.len());
    assert_approx_eq!(retrieved[0].co2, 650.0, 0.01);
}

#[tokio::test]
async fn test_parquet_time_range_query() {
    let store = setup_test_store().await;

    // Write readings over 48 hours
    let base_time = Utc::now() - Duration::hours(48);
    for hour in 0..48 {
        let reading = create_test_reading(base_time + Duration::hours(hour));
        store.write(&reading).await.unwrap();
    }

    // Query last 24 hours only
    let start = Utc::now() - Duration::hours(24);
    let results = store.query_range(start, Utc::now()).await.unwrap();

    assert_eq!(results.len(), 24);
    assert!(results.iter().all(|r| r.timestamp >= start));
}

#[tokio::test]
async fn test_parquet_compression_ratio() {
    let store = setup_test_store().await;
    let readings: Vec<_> = (0..10000)
        .map(|_| create_random_reading())
        .collect();

    store.write_batch(&readings).await.unwrap();

    let file_size = fs::metadata(store.file_path()).await.unwrap().len();
    let uncompressed_estimate = readings.len() * std::mem::size_of::<AirQualityReading>();

    let compression_ratio = uncompressed_estimate as f64 / file_size as f64;
    assert!(compression_ratio > 3.0, "Compression ratio too low: {}", compression_ratio);
}
```

#### 1.1.3 Validation Rules

**Test Edge Cases:** Missing data, outliers, sensor warm-up

```rust
// tests/unit/validation_test.rs
#[test]
fn test_sensor_warmup_detection() {
    let validator = ReadingValidator::new();

    // First 3 minutes = warm-up period
    let early_reading = AirQualityReading {
        timestamp: sensor_start_time + Duration::minutes(2),
        co2: 400.0,  // Suspiciously perfect
        ..Default::default()
    };

    assert!(!validator.is_warmed_up(&early_reading));

    // After 5 minutes = ready
    let ready_reading = AirQualityReading {
        timestamp: sensor_start_time + Duration::minutes(6),
        ..Default::default()
    };

    assert!(validator.is_warmed_up(&ready_reading));
}

#[test]
fn test_outlier_detection() {
    let validator = ReadingValidator::new();

    // Physically impossible values
    assert!(validator.validate(&AirQualityReading {
        co2: 50000.0,  // CO2 can't be this high
        ..Default::default()
    }).is_err());

    assert!(validator.validate(&AirQualityReading {
        pm25: -5.0,  // Negative concentration
        ..Default::default()
    }).is_err());

    assert!(validator.validate(&AirQualityReading {
        temperature: 100.0,  // 100°C indoors
        ..Default::default()
    }).is_err());
}

#[test]
fn test_missing_data_handling() {
    let validator = ReadingValidator::new();

    let sparse_reading = AirQualityReading {
        timestamp: Utc::now(),
        co2: Some(650.0),
        pm25: None,  // Missing
        temperature: Some(22.0),
        ..Default::default()
    };

    let quality = validator.assess_quality(&sparse_reading);
    assert_eq!(quality.completeness, 0.66);  // 2/3 fields present
    assert!(quality.is_usable());
}
```

#### 1.1.4 Mock-Based Testing

**Trait Implementations:**

```rust
// tests/mocks/mod.rs
pub struct MockSensor {
    readings: VecDeque<AirQualityReading>,
}

impl MockSensor {
    pub fn with_pattern(pattern: ReadingPattern) -> Self {
        let readings = match pattern {
            ReadingPattern::Stable => Self::generate_stable(),
            ReadingPattern::Rising => Self::generate_rising_co2(),
            ReadingPattern::Spiking => Self::generate_pm25_spikes(),
        };
        Self { readings: readings.into() }
    }
}

#[async_trait]
impl AirQualitySensor for MockSensor {
    async fn read(&mut self) -> Result<AirQualityReading> {
        self.readings.pop_front()
            .ok_or_else(|| Error::NoMoreData)
    }
}

// Usage in tests
#[tokio::test]
async fn test_alert_on_rising_co2() {
    let sensor = MockSensor::with_pattern(ReadingPattern::Rising);
    let alerter = ThresholdAlerter::new(Thresholds::default());

    // Process readings
    let mut alerts = Vec::new();
    for _ in 0..20 {
        let reading = sensor.read().await.unwrap();
        alerts.extend(alerter.check(&reading).await.unwrap());
    }

    assert!(alerts.iter().any(|a| a.metric == "co2"));
}
```

---

### 1.2 Integration Testing

#### 1.2.1 MQTT → Storage Pipeline

```rust
// tests/integration/mqtt_to_storage_test.rs
#[tokio::test]
async fn test_end_to_end_mqtt_flow() {
    // Setup
    let mqtt_broker = start_test_mqtt_broker().await;
    let parquet_store = ParquetStore::new_temp().await.unwrap();

    // Start ingestion pipeline
    let pipeline = MqttIngestionPipeline::new(
        mqtt_broker.url(),
        parquet_store.clone(),
    );

    let handle = tokio::spawn(async move {
        pipeline.run().await
    });

    // Publish test message
    let test_reading = create_test_reading();
    mqtt_broker.publish("air-quality/reading", test_reading).await;

    // Wait for processing
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify storage
    let stored = parquet_store.latest().await.unwrap();
    assert_approx_eq!(stored.co2, test_reading.co2, 0.01);

    handle.abort();
}

#[tokio::test]
async fn test_mqtt_reconnection_resilience() {
    let broker = TestMqttBroker::new().await;
    let pipeline = MqttIngestionPipeline::new(broker.url(), ParquetStore::new_temp().await.unwrap());

    // Start pipeline
    let handle = tokio::spawn(async move { pipeline.run().await });

    // Kill broker
    broker.stop().await;
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Restart broker
    broker.restart().await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify reconnection
    broker.publish("air-quality/reading", create_test_reading()).await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Should have reconnected and stored the reading
    assert!(broker.has_active_subscribers());

    handle.abort();
}
```

#### 1.2.2 Storage → Query Pipeline

```rust
// tests/integration/query_pipeline_test.rs
#[tokio::test]
async fn test_historical_query_performance() {
    let store = setup_populated_store(10_000).await; // 10k readings

    let start = Instant::now();
    let results = store.query_range(
        Utc::now() - Duration::hours(24),
        Utc::now(),
    ).await.unwrap();
    let duration = start.elapsed();

    assert!(results.len() > 0);
    assert!(duration.as_millis() < 100, "Query too slow: {:?}", duration);
}

#[tokio::test]
async fn test_aggregation_accuracy() {
    let store = setup_test_store().await;

    // Write known data
    let base = Utc::now() - Duration::hours(24);
    for hour in 0..24 {
        let reading = AirQualityReading {
            timestamp: base + Duration::hours(hour),
            co2: 600.0 + (hour as f64 * 10.0), // 600, 610, 620...
            ..Default::default()
        };
        store.write(&reading).await.unwrap();
    }

    // Query aggregates
    let stats = store.query_stats(base, Utc::now()).await.unwrap();

    assert_approx_eq!(stats.co2_mean, 715.0, 1.0); // (600+830)/2 * 24/2
    assert_approx_eq!(stats.co2_min, 600.0, 0.1);
    assert_approx_eq!(stats.co2_max, 830.0, 0.1);
}
```

#### 1.2.3 Forecast Pipeline

```rust
// tests/integration/forecast_pipeline_test.rs
#[tokio::test]
async fn test_forecast_generation_latency() {
    let store = setup_populated_store(2880).await; // 48 hours @ 1 min intervals
    let forecaster = AirQualityForecaster::new();

    // Train on historical data
    let history = store.query_range(
        Utc::now() - Duration::hours(48),
        Utc::now(),
    ).await.unwrap();

    forecaster.fit("co2", &history).await.unwrap();

    // Measure forecast generation time
    let start = Instant::now();
    let forecast = forecaster.forecast("co2", 48).await.unwrap(); // 24h ahead
    let duration = start.elapsed();

    assert_eq!(forecast.predictions.len(), 48);
    assert!(duration.as_millis() < 1000, "Forecast too slow: {:?}", duration);
}

#[tokio::test]
async fn test_forecast_confidence_intervals() {
    let forecaster = setup_trained_forecaster().await;
    let forecast = forecaster.forecast("co2", 24).await.unwrap();

    // Check confidence intervals are reasonable
    for (i, (&point, (&lower, &upper))) in forecast.predictions.iter()
        .zip(forecast.lower_95.iter().zip(forecast.upper_95.iter()))
        .enumerate()
    {
        assert!(lower <= point, "Lower bound violation at step {}", i);
        assert!(point <= upper, "Upper bound violation at step {}", i);

        // Interval should widen with time
        if i > 0 {
            let prev_width = forecast.upper_95[i-1] - forecast.lower_95[i-1];
            let curr_width = upper - lower;
            assert!(curr_width >= prev_width * 0.95, "Confidence interval shrinking");
        }
    }
}
```

---

### 1.3 End-to-End Testing

#### 1.3.1 Full Data Flow with Mock Sensor

```rust
// tests/e2e/full_system_test.rs
#[tokio::test]
async fn test_complete_data_flow() {
    // 1. Setup entire system
    let system = TestSystem::new().await;
    system.start_all_services().await;

    // 2. Mock sensor produces readings
    let mock_sensor = MockSensor::with_pattern(ReadingPattern::Realistic);

    // 3. Readings flow through system
    for _ in 0..100 {
        let reading = mock_sensor.read().await.unwrap();
        system.mqtt_broker.publish("air-quality/reading", &reading).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // 4. Verify each stage
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Storage received all
    let stored = system.parquet_store.count().await.unwrap();
    assert_eq!(stored, 100);

    // MCP server can query
    let current = system.mcp_client.call("get_current_readings").await.unwrap();
    assert!(current.co2 > 0.0);

    // Forecasts available
    let forecast = system.mcp_client.call("forecast_air_quality", json!({"hours": 4})).await.unwrap();
    assert_eq!(forecast.predictions.len(), 8); // 30-min intervals

    system.shutdown().await;
}
```

#### 1.3.2 Alert Triggering

```rust
// tests/e2e/alert_system_test.rs
#[tokio::test]
async fn test_alert_end_to_end() {
    let system = TestSystem::new().await;
    let alert_receiver = system.subscribe_alerts().await;

    // Publish high CO2 reading
    let high_co2 = AirQualityReading {
        timestamp: Utc::now(),
        co2: 1800.0,  // Above threshold
        ..Default::default()
    };

    system.mqtt_broker.publish("air-quality/reading", &high_co2).await;

    // Wait for alert
    let alert = timeout(Duration::from_secs(5), alert_receiver.recv())
        .await
        .expect("Alert timeout")
        .expect("No alert received");

    assert_eq!(alert.metric, "co2");
    assert_eq!(alert.severity, Severity::Warning);
    assert!(alert.message.contains("1800"));
}

#[tokio::test]
async fn test_predictive_alert() {
    let system = TestSystem::new().await;
    let alert_receiver = system.subscribe_alerts().await;

    // Feed pattern that will exceed threshold in 2 hours
    let pattern = MockSensor::with_pattern(ReadingPattern::GradualRise {
        start: 800.0,
        end: 1600.0,
        duration: Duration::hours(3),
    });

    system.ingest_pattern(&pattern).await;

    // Should get predictive alert
    let alert = timeout(Duration::from_secs(10), alert_receiver.recv())
        .await
        .expect("No predictive alert")
        .expect("Alert error");

    assert_eq!(alert.alert_type, AlertType::Predictive);
    assert!(alert.time_until.unwrap().as_secs() > 3600); // >1 hour warning
}
```

#### 1.3.3 MCP Tool Responses

```rust
// tests/e2e/mcp_tools_test.rs
#[tokio::test]
async fn test_mcp_get_current_readings() {
    let system = TestSystem::new().await;

    // Publish known reading
    let reading = AirQualityReading {
        timestamp: Utc::now(),
        co2: 742.0,
        pm25: 15.3,
        temperature: 21.5,
        humidity: 48.2,
        ..Default::default()
    };
    system.publish_reading(&reading).await;

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Query via MCP
    let response = system.mcp_client
        .call_tool("get_current_readings", json!({}))
        .await
        .unwrap();

    assert_approx_eq!(response.co2_ppm, 742.0, 1.0);
    assert_approx_eq!(response.pm25_ugm3, 15.3, 0.5);
    assert_eq!(response.aqi_category, "Good");
}

#[tokio::test]
async fn test_mcp_analyze_ventilation() {
    let system = TestSystem::new().await;

    // Load 24h of historical data
    system.load_test_history("tests/fixtures/high_co2_24h.parquet").await;

    // Call ventilation analysis
    let analysis = system.mcp_client
        .call_tool("analyze_ventilation", json!({}))
        .await
        .unwrap();

    assert!(analysis.estimated_ach > 0.0);
    assert!(analysis.estimated_ach < 2.0); // Typical residential
    assert!(analysis.decay_time_minutes > 0);
    assert!(!analysis.recommendations.is_empty());
}

#[tokio::test]
async fn test_mcp_forecast_accuracy() {
    let system = TestSystem::new().await;
    system.load_test_history("tests/fixtures/stable_pattern_48h.parquet").await;

    // Get 4-hour forecast
    let forecast = system.mcp_client
        .call_tool("forecast_air_quality", json!({"hours": 4}))
        .await
        .unwrap();

    // Verify structure
    assert_eq!(forecast.predictions.len(), 8);
    assert!(forecast.confidence > 0.7);

    // Continue feeding data and check accuracy
    let actual_future = system.load_actual_future_data().await;
    let mape = calculate_mape(&forecast.predictions, &actual_future);

    assert!(mape < 15.0, "Forecast MAPE too high: {:.2}%", mape);
}
```

---

## 2. Quality Gates

Each gate must pass before proceeding to the next development phase.

| Gate | Criteria | Measurement | Automation |
|------|----------|-------------|------------|
| **G1: Compilation** | Code compiles without errors | `cargo build --release` exits 0 | CI required |
| **G2: Tests Pass** | All tests pass with >95% coverage | `cargo test && cargo tarpaulin` | CI required |
| **G3: Linting** | No clippy warnings (pedantic mode) | `cargo clippy -- -D warnings` | CI required |
| **G4: Documentation** | All public APIs documented | `cargo doc --no-deps` no warnings | CI required |
| **G5: Performance** | Ingestion <10ms/point, query <100ms | `cargo bench` comparison | CI optional |
| **G6: Memory** | <500MB RSS on Pi5 under load | `systemd-cgtop` monitoring | Manual |
| **G7: Formatting** | Consistent code style | `cargo fmt --check` | CI required |
| **G8: Security** | No known vulnerabilities | `cargo audit` clean | CI required |

### G1: Compilation Gate

```bash
# Must pass on both platforms
cargo build --release --target x86_64-apple-darwin     # M4 Mac
cargo build --release --target aarch64-unknown-linux-gnu  # Pi5

# All features enabled
cargo build --all-features

# No warnings
cargo build --release 2>&1 | grep -i warning && exit 1
```

### G2: Test Coverage Gate

```bash
# Run all test suites
cargo test --all-features

# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage

# Enforce minimum coverage
cargo tarpaulin --fail-under 95
```

**Coverage Requirements by Module:**

| Module | Minimum Coverage | Rationale |
|--------|------------------|-----------|
| `aqi.rs` | 100% | Core domain logic |
| `parquet.rs` | 95% | Data integrity critical |
| `validation.rs` | 100% | Safety critical |
| `mqtt.rs` | 85% | External I/O, hard to mock |
| `forecast.rs` | 90% | ML accuracy critical |
| `mcp_server.rs` | 80% | Integration layer |

### G3: Clippy Linting Gate

```toml
# .cargo/config.toml
[target.'cfg(all())']
rustflags = ["-D", "warnings"]

# clippy.toml
msrv = "1.70.0"
cognitive-complexity-threshold = 15
```

```bash
# Must pass pedantic lints
cargo clippy --all-features -- \
    -D warnings \
    -W clippy::pedantic \
    -W clippy::nursery \
    -A clippy::module_name_repetitions
```

### G4: Documentation Gate

```bash
# All public items documented
cargo doc --no-deps --document-private-items 2>&1 | grep warning && exit 1

# Documentation tests pass
cargo test --doc
```

**Required Documentation:**

- Module-level docs with examples
- All public structs/enums with descriptions
- All public functions with:
  - Summary line
  - Parameter descriptions
  - Return value description
  - Example usage (where non-trivial)
  - Error conditions

Example:

```rust
/// Calculates the EPA Air Quality Index from raw PM2.5 concentration.
///
/// Uses the EPA AQI breakpoint table to convert particulate matter
/// concentration (µg/m³) to the 0-500 AQI scale.
///
/// # Arguments
///
/// * `pm25_ugm3` - PM2.5 concentration in micrograms per cubic meter
///
/// # Returns
///
/// AQI value (0-500+) or error if concentration is invalid.
///
/// # Example
///
/// ```
/// use air_quality_core::analysis::aqi::pm25_to_aqi;
///
/// let aqi = pm25_to_aqi(12.0)?;
/// assert_eq!(aqi, 50); // EPA breakpoint
/// ```
///
/// # Errors
///
/// Returns `Error::InvalidConcentration` if `pm25_ugm3` is negative or NaN.
///
/// # References
///
/// - [EPA AQI Calculator](https://www.airnow.gov/aqi/aqi-calculator-concentration/)
pub fn pm25_to_aqi(pm25_ugm3: f64) -> Result<u32> {
    // ...
}
```

### G5: Performance Benchmarks Gate

```rust
// benches/ingestion_benchmark.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_parquet_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("parquet_write");

    for batch_size in [1, 10, 100, 1000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            &batch_size,
            |b, &size| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let store = rt.block_on(ParquetStore::new_temp()).unwrap();
                let readings: Vec<_> = (0..size)
                    .map(|_| create_test_reading())
                    .collect();

                b.iter(|| {
                    rt.block_on(store.write_batch(black_box(&readings)))
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_parquet_write);
criterion_main!(benches);
```

**Performance Targets:**

| Operation | Target | Measurement |
|-----------|--------|-------------|
| Parquet write (single) | <10ms | 99th percentile |
| Parquet write (batch 100) | <50ms | Mean |
| Parquet query (24h range) | <100ms | 99th percentile |
| AQI calculation | <1µs | Mean |
| Forecast generation | <500ms | 99th percentile |
| MQTT publish latency | <5ms | 99th percentile |

### G6: Memory Constraints Gate

**Raspberry Pi 5 Targets:**

- **Idle:** <100MB RSS
- **Active ingestion:** <200MB RSS
- **Forecast training:** <400MB RSS
- **Peak (worst case):** <500MB RSS

**Monitoring:**

```bash
# During test run
systemd-cgtop -m -n 1 --order=memory | grep air-quality

# Valgrind leak check (x86 only)
valgrind --leak-check=full --show-leak-kinds=all ./target/release/air-quality-daemon

# Continuous monitoring
cargo run --release --bin memory-profiler -- --duration 3600
```

---

## 3. Performance Benchmarks

### 3.1 Ingestion Performance

**Setup:** Raspberry Pi 5 (8GB), 64-bit OS

| Metric | Target | Baseline | Optimized | Test Command |
|--------|--------|----------|-----------|--------------|
| **Parquet write throughput** | >100 points/sec | TBD | TBD | `cargo bench parquet_write` |
| **MQTT message processing** | >200 msg/sec | TBD | TBD | `cargo bench mqtt_ingestion` |
| **End-to-end latency** | <5s (p99) | TBD | TBD | `cargo test --release e2e_latency` |

### 3.2 Query Performance

**Setup:** M4 Mac, QuestDB with 1M rows

| Query Type | Target | Measurement |
|------------|--------|-------------|
| Latest reading | <10ms | p99 |
| 24-hour range (1-min resolution) | <100ms | p99 |
| 7-day aggregates | <500ms | p99 |
| 30-day statistics | <2s | p99 |

```rust
// benches/query_benchmark.rs
fn bench_query_24h(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = rt.block_on(setup_populated_store(100_000)); // ~1 week @ 1/min

    c.bench_function("query_24h_range", |b| {
        b.iter(|| {
            let start = Utc::now() - Duration::hours(24);
            rt.block_on(store.query_range(black_box(start), Utc::now()))
        });
    });
}
```

### 3.3 Forecast Performance

**Setup:** M4 Mac with 48h training data

| Forecast Horizon | Target Latency | Target MAPE | Model |
|------------------|----------------|-------------|-------|
| 1 hour | <100ms | <10% | ETS |
| 4 hours | <300ms | <15% | MSTL |
| 24 hours | <1s | <25% | Ensemble |

```rust
// tests/performance/forecast_accuracy_test.rs
#[tokio::test]
async fn test_forecast_mape() {
    let store = load_historical_data("tests/fixtures/2_weeks.parquet").await;

    // Split: train on first 80%, test on last 20%
    let (train, test) = store.split_at_timestamp(split_point).await;

    let forecaster = AirQualityForecaster::new();
    forecaster.fit("co2", &train).await.unwrap();

    let forecast = forecaster.forecast("co2", test.len()).await.unwrap();
    let actual: Vec<f64> = test.iter().map(|r| r.co2).collect();

    let mape = calculate_mape(&forecast.predictions, &actual);
    assert!(mape < 15.0, "MAPE {:.2}% exceeds target", mape);
}

fn calculate_mape(predictions: &[f64], actuals: &[f64]) -> f64 {
    predictions.iter()
        .zip(actuals.iter())
        .map(|(pred, actual)| ((actual - pred).abs() / actual) * 100.0)
        .sum::<f64>() / predictions.len() as f64
}
```

### 3.4 Memory Usage Under Load

```rust
// tests/performance/memory_test.rs
#[tokio::test]
async fn test_memory_leak() {
    let initial = current_memory_usage();

    let system = TestSystem::new().await;

    // Run for 1 hour, ingesting every 30 seconds
    for _ in 0..120 {
        system.ingest_reading(create_test_reading()).await;
        tokio::time::sleep(Duration::from_secs(30)).await;
    }

    let final_usage = current_memory_usage();
    let growth = final_usage - initial;

    // Allow 10% growth max
    assert!(growth < initial * 0.1, "Memory grew by {:.1}%", (growth as f64 / initial as f64) * 100.0);
}
```

---

## 4. Code Quality Standards

### 4.1 File Organization

**Maximum file sizes:**

- `lib.rs` / `main.rs`: 300 lines
- Module files: 500 lines
- Test files: 1000 lines (exception)

If exceeded, split into submodules:

```
air-quality-core/src/
├── analysis/
│   ├── mod.rs          # Re-exports, <100 lines
│   ├── aqi.rs          # AQI calculation, <500 lines
│   ├── ventilation.rs  # ACH estimation, <500 lines
│   └── events.rs       # Event detection, <500 lines
```

### 4.2 Function Complexity

**Maximum function sizes:**

- Public API functions: 50 lines
- Internal functions: 80 lines
- Test functions: 100 lines (exception)

**Cyclomatic complexity:**

- Target: <10
- Maximum: 15
- Measured by: `cargo clippy -W clippy::cognitive-complexity`

**Example of refactoring for complexity:**

```rust
// BAD: Complexity = 18
pub fn process_reading(reading: &AirQualityReading) -> Result<Vec<Action>> {
    let mut actions = Vec::new();

    if reading.co2 > 1000.0 {
        if reading.pm25 > 35.0 {
            if reading.voc_index > 200 {
                actions.push(Action::AlertHighPollution);
            } else {
                actions.push(Action::AlertCO2);
            }
        } else {
            if reading.temperature > 25.0 {
                actions.push(Action::VentilateWithCooling);
            } else {
                actions.push(Action::Ventilate);
            }
        }
    } else if reading.pm25 > 25.0 {
        // ... more nesting
    }
    // ...
}

// GOOD: Complexity = 4
pub fn process_reading(reading: &AirQualityReading) -> Result<Vec<Action>> {
    let mut actions = Vec::new();

    actions.extend(check_air_quality(reading)?);
    actions.extend(check_ventilation_needs(reading)?);
    actions.extend(check_comfort(reading)?);

    Ok(actions)
}

fn check_air_quality(reading: &AirQualityReading) -> Result<Vec<Action>> {
    match (reading.co2, reading.pm25, reading.voc_index) {
        (co2, pm25, voc) if co2 > 1000.0 && pm25 > 35.0 && voc > 200 => {
            Ok(vec![Action::AlertHighPollution])
        }
        (co2, _, _) if co2 > 1000.0 => Ok(vec![Action::AlertCO2]),
        // ...
    }
}
```

### 4.3 Documentation Requirements

**All public APIs must have:**

1. **Summary** (1-2 sentences)
2. **Arguments** (if any)
3. **Returns** (what and when)
4. **Errors** (all error conditions)
5. **Examples** (non-trivial functions)
6. **Panics** (if applicable)
7. **Safety** (if unsafe)

```rust
/// Estimates air changes per hour (ACH) from CO2 decay rate.
///
/// Analyzes CO2 concentration decay when indoor sources are removed
/// (e.g., after opening windows). Uses exponential decay fitting to
/// estimate natural ventilation rate.
///
/// # Arguments
///
/// * `readings` - Time-series of CO2 readings during ventilation event
/// * `baseline_co2` - Outdoor/target CO2 level (typically 400-450 ppm)
///
/// # Returns
///
/// Estimated ACH (0.1 to 5.0 typical range) or error if insufficient data.
///
/// # Errors
///
/// - `Error::InsufficientData` if <10 readings provided
/// - `Error::InvalidDecay` if CO2 is increasing instead of decreasing
/// - `Error::FitFailed` if exponential fit doesn't converge
///
/// # Example
///
/// ```
/// use air_quality_core::analysis::ventilation::estimate_ach;
///
/// let readings = load_ventilation_event()?;
/// let ach = estimate_ach(&readings, 420.0)?;
/// println!("Estimated ventilation: {:.2} ACH", ach);
/// ```
///
/// # References
///
/// - ASHRAE Standard 62.2-2019 (residential ventilation)
/// - "CO2 as a tracer gas" - Sherman & Dickerhoff (1998)
pub fn estimate_ach(readings: &[AirQualityReading], baseline_co2: f64) -> Result<f64> {
    // ...
}
```

### 4.4 Error Handling

**No `unwrap()` or `expect()` in production code.**

Use:

```rust
// GOOD: Propagate errors
pub fn read_config(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path)?;
    let config = toml::from_str(&content)?;
    Ok(config)
}

// GOOD: Provide context
pub fn read_config(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config from {}", path.display()))?;

    toml::from_str(&content)
        .with_context(|| "Invalid TOML syntax in config file")
}

// BAD: panic in library code
pub fn read_config(path: &Path) -> Config {
    let content = fs::read_to_string(path).expect("Config file must exist");
    toml::from_str(&content).unwrap()
}
```

**Exception:** `unwrap()` is allowed in:

- Test code
- Examples/documentation
- After explicit checks: `if some.is_some() { some.unwrap() }`

---

## 5. Refinement Iterations

### Iteration 1: Core Storage & Validation (Week 1)

**Focus:** Data correctness and reliability

**Entry Criteria:**

- Phase 1 MVP features implemented
- Basic tests written

**Tasks:**

1. Achieve 100% test coverage on:
   - AQI calculation
   - Validation rules
   - Parquet read/write
2. Add property-based tests (proptest)
3. Benchmark Parquet compression ratios
4. Document all storage APIs

**Exit Criteria:**

- ✅ All storage tests pass
- ✅ Parquet compression ratio >3.0x
- ✅ No data loss over 24h continuous test
- ✅ All G1-G4 gates pass

**Metrics:**

- Test coverage: >95%
- Parquet write latency: <10ms p99
- Documentation completeness: 100%

---

### Iteration 2: Domain Accuracy (Week 2)

**Focus:** Air quality calculations match EPA references

**Entry Criteria:**

- Iteration 1 complete
- Reference test data collected

**Tasks:**

1. Validate AQI against EPA calculator
2. Test all EPA breakpoints (PM2.5, PM10, CO2)
3. Compare with reference implementations
4. Add edge case tests (extreme values)

**Exit Criteria:**

- ✅ AQI matches EPA within ±1 AQI point
- ✅ All EPA breakpoints tested
- ✅ VOC/NOx index calculations validated
- ✅ Health category thresholds correct

**Validation Data:**

```rust
// tests/validation/epa_reference_test.rs
#[test]
fn test_epa_aqi_breakpoints() {
    let test_cases = [
        // (PM2.5 µg/m³, Expected AQI)
        (0.0, 0),
        (12.0, 50),
        (35.4, 100),
        (55.4, 150),
        (150.4, 200),
        (250.4, 300),
        (350.4, 400),
        (500.4, 500),
    ];

    for (pm25, expected_aqi) in test_cases {
        let calculated = pm25_to_aqi(pm25).unwrap();
        assert_eq!(
            calculated, expected_aqi,
            "PM2.5 {:.1} µg/m³ should be AQI {}, got {}",
            pm25, expected_aqi, calculated
        );
    }
}
```

---

### Iteration 3: Integration Reliability (Week 3)

**Focus:** End-to-end pipeline stability

**Entry Criteria:**

- Iteration 2 complete
- All components individually tested

**Tasks:**

1. 24-hour soak test on Pi5
2. Network failure recovery tests
3. MQTT broker restart handling
4. Disk space exhaustion handling

**Exit Criteria:**

- ✅ 24h run without crashes
- ✅ Automatic reconnection on network loss
- ✅ Graceful degradation on storage full
- ✅ All data recovered after restart

**Test Scenarios:**

```rust
// tests/integration/reliability_test.rs
#[tokio::test]
async fn test_24h_continuous_operation() {
    let system = TestSystem::new().await;
    let start = Instant::now();

    // Run for 24 hours
    while start.elapsed() < Duration::from_secs(86400) {
        // Inject random failures
        if rand::random::<f64>() < 0.01 {  // 1% failure rate
            system.mqtt_broker.inject_failure().await;
        }

        system.ingest_reading(create_test_reading()).await;
        tokio::time::sleep(Duration::from_secs(30)).await;
    }

    // Verify data completeness
    let stored = system.parquet_store.count().await.unwrap();
    let expected = 86400 / 30;  // ~2880 readings

    let completeness = stored as f64 / expected as f64;
    assert!(completeness > 0.99, "Only {:.1}% data captured", completeness * 100.0);
}
```

---

### Iteration 4: Performance Optimization (Week 4)

**Focus:** Meet Pi5 resource constraints

**Entry Criteria:**

- Iteration 3 complete
- Baseline performance measured

**Tasks:**

1. Profile hot paths with flamegraph
2. Optimize Parquet write batching
3. Reduce memory allocations
4. Tune async runtime

**Exit Criteria:**

- ✅ Memory <500MB RSS on Pi5
- ✅ CPU <30% average on Pi5
- ✅ Ingestion latency <10ms p99
- ✅ Query latency <100ms p99

**Optimization Techniques:**

```rust
// Before: Allocating on every read
pub async fn read(&self) -> Result<AirQualityReading> {
    let response = self.client.get(&self.url).send().await?;
    let json: serde_json::Value = response.json().await?;
    Ok(parse_reading(&json)?)
}

// After: Reuse buffer
pub struct AirGradientSource {
    client: Client,
    url: String,
    buffer: Mutex<Vec<u8>>,  // Reusable buffer
}

pub async fn read(&self) -> Result<AirQualityReading> {
    let mut buffer = self.buffer.lock().await;
    buffer.clear();

    self.client.get(&self.url)
        .send().await?
        .copy_to(&mut *buffer).await?;

    Ok(serde_json::from_slice(&buffer)?)
}
```

**Profiling Commands:**

```bash
# CPU profiling
cargo flamegraph --bin air-quality-daemon -- --duration 60

# Memory profiling
heaptrack ./target/release/air-quality-daemon --duration 60

# Perf analysis
cargo build --release
perf record -F 99 -g ./target/release/air-quality-daemon
perf script | stackcollapse-perf | flamegraph > perf.svg
```

---

## 6. Definition of Done

A feature is considered **DONE** when ALL of the following are met:

### Code Completion Checklist

- [ ] **Implementation complete** per specification
- [ ] **All quality gates pass** (G1-G8)
- [ ] **Test coverage ≥95%** (per module requirements)
- [ ] **All tests pass** (unit, integration, e2e)
- [ ] **Documentation complete**:
  - [ ] All public APIs documented
  - [ ] Module-level documentation
  - [ ] README with examples
  - [ ] Architecture diagrams
- [ ] **Code reviewed** by another developer
- [ ] **No linting warnings** (`cargo clippy` clean)
- [ ] **Security audit clean** (`cargo audit`)

### Performance Checklist

- [ ] **Benchmarks run** and results recorded
- [ ] **Performance targets met**:
  - [ ] Ingestion <10ms/point (p99)
  - [ ] Query 24h <100ms (p99)
  - [ ] Forecast <500ms (p99)
- [ ] **Memory constraints met**:
  - [ ] <500MB RSS on Pi5
  - [ ] No memory leaks detected
- [ ] **CPU usage acceptable**:
  - [ ] <30% average on Pi5
  - [ ] <10% idle on M4

### Platform Compatibility Checklist

- [ ] **Compiles on macOS (M4)**
- [ ] **Compiles on Pi5 (aarch64)**
- [ ] **Tests pass on both platforms**
- [ ] **Performance acceptable on Pi5**
- [ ] **Cross-compilation working**:
  ```bash
  cargo build --release --target aarch64-unknown-linux-gnu
  ```

### Integration Checklist

- [ ] **MCP tools functional**:
  - [ ] `get_current_readings` works
  - [ ] `forecast_air_quality` works
  - [ ] `analyze_ventilation` works
  - [ ] `get_health_recommendations` works
- [ ] **MQTT integration working**:
  - [ ] Publishes to MQTT broker
  - [ ] Home Assistant auto-discovery
- [ ] **HomeKit integration working**:
  - [ ] Visible in Apple Home
  - [ ] Real-time updates
- [ ] **Grafana dashboards working**:
  - [ ] All panels display data
  - [ ] Alerts configured

### Operational Readiness Checklist

- [ ] **Deployment tested**:
  - [ ] systemd service file created
  - [ ] Service starts on boot
  - [ ] Log rotation configured
- [ ] **Monitoring configured**:
  - [ ] Prometheus metrics exposed
  - [ ] Health check endpoint working
  - [ ] Alerts configured
- [ ] **Backup strategy defined**:
  - [ ] Parquet files backed up
  - [ ] Configuration backed up
  - [ ] Restore procedure tested
- [ ] **Error handling verified**:
  - [ ] Network failures handled
  - [ ] Disk full handled
  - [ ] Sensor disconnection handled

---

## 7. Technical Debt Tracking

### Known Shortcuts

| Item | Description | Impact | Remediation Plan | Target |
|------|-------------|--------|------------------|--------|
| **SQLite on Pi** | Using SQLite instead of Parquet on Pi | Memory inefficient | Migrate to Parquet | Phase 3 |
| **Single-threaded ingestion** | MQTT ingestion not parallelized | Limited throughput | Add tokio task pool | Phase 4 |
| **No model versioning** | Forecasting models not versioned | Rollback impossible | Add MLflow integration | Phase 6 |
| **Hardcoded thresholds** | Alert thresholds in code, not config | Inflexible | Move to TOML config | Phase 2 |

### Future Optimization Opportunities

| Opportunity | Benefit | Effort | Priority |
|-------------|---------|--------|----------|
| **Parquet predicate pushdown** | 10x faster queries | Medium | High |
| **SIMD AQI calculation** | 5x faster AQI | Low | Low |
| **Zero-copy deserialization** | 30% less memory | High | Medium |
| **Forecast model ensemble** | 20% better accuracy | High | Medium |
| **Adaptive sampling rate** | 50% less storage | Medium | Low |

### Deferred Features

| Feature | Reason Deferred | Target Phase |
|---------|----------------|--------------|
| **Multi-sensor aggregation** | Not needed for MVP | Phase 5 |
| **Mobile app** | Out of scope | Post-launch |
| **Cloud sync** | Local-first design | Phase 7 |
| **Historical re-training** | Manual training sufficient for now | Phase 6 |
| **Custom alert rules DSL** | TOML sufficient | Phase 8 |

### Technical Debt Review Cadence

- **Weekly:** Review during sprint planning
- **Monthly:** Update remediation targets
- **Quarterly:** Prioritize top 3 items for resolution

---

## 8. Appendix: Testing Tools & Frameworks

### Test Frameworks

```toml
[dev-dependencies]
# Unit testing
tokio-test = "0.4"
proptest = "1.4"  # Property-based testing
rstest = "0.18"   # Parameterized tests

# Integration testing
testcontainers = "0.15"  # Docker containers for tests
wiremock = "0.5"         # Mock HTTP servers

# Performance testing
criterion = "0.5"
flamegraph = "0.6"

# Coverage
tarpaulin = "0.27"
```

### Continuous Integration

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]

jobs:
  test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest]
        rust: [stable, nightly]

    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: ${{ matrix.rust }}

      - name: Run tests
        run: cargo test --all-features

      - name: Run clippy
        run: cargo clippy --all-features -- -D warnings

      - name: Check formatting
        run: cargo fmt --check

      - name: Run benchmarks
        run: cargo bench --no-run

  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable

      - name: Install tarpaulin
        run: cargo install cargo-tarpaulin

      - name: Generate coverage
        run: cargo tarpaulin --out Xml --fail-under 95

      - name: Upload to codecov
        uses: codecov/codecov-action@v3
```

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-13 | System | Initial refinement criteria |

---

**Next Steps:**

1. Review this document with the team
2. Set up CI/CD pipeline with quality gates
3. Begin Iteration 1: Core Storage & Validation
4. Establish baseline performance metrics

**Related Documents:**

- `/product/features/air-001/specs/01-specification.md` (when created)
- `/product/research/08-air-quality-domain-spec.md`
- `/product/research/09-implementation-roadmap.md`
