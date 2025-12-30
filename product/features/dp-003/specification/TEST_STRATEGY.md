# DP-003: MQTT Multi-Subscription Test Strategy

## Overview

This document defines the comprehensive test strategy for the MQTT multi-subscription feature (DP-003). The testing approach follows London School TDD principles established in AIR-005, with focus on behavior verification and component interaction testing.

---

## 1. Test Pyramid

```
                    /\
                   /  \
                  / E2E \          2-3 tests
                 /  Tests \        (Full pipeline, Grafana verification)
                /----------\
               / Integration \     8-10 tests
              /    Tests      \    (Real MQTT broker, real Parquet storage)
             /----------------\
            /    Unit Tests    \   40-50 tests
           /  (Fast, Isolated)  \  (Mocked dependencies, behavior verification)
          /______________________\
```

### Distribution

| Layer | Count | Time | Focus |
|-------|-------|------|-------|
| Unit Tests | 40-50 | < 1s each | Component behavior, edge cases |
| Integration Tests | 8-10 | 1-10s each | Component interactions, real MQTT |
| E2E Tests | 2-3 | 30-60s each | Full pipeline verification |

---

## 2. Test Categories

### 2.1 Unit Tests

**Location**: Inline `#[cfg(test)]` modules in implementation files

**Coverage Areas**:

#### A. Configuration Parsing (10-12 tests)
- Multi-subscription config loading
- Backward-compatible single topic config
- Invalid config rejection
- Duplicate stream_id detection
- Parser configuration per subscription
- Default value handling

#### B. Topic Router (12-15 tests)
- MQTT wildcard pattern matching (+, #)
- First-match routing for overlapping patterns
- Unmatched topic handling
- Stream ID tag injection
- Topic-to-subscription mapping

#### C. Subscription Management (8-10 tests)
- Subscription lifecycle (add/remove)
- Concurrent subscription access
- Subscription health tracking
- Message counter per subscription

#### D. Parser Integration (8-10 tests)
- Per-subscription parser selection
- Parser error isolation
- Schema consistency across parsers
- Default location ID handling

### 2.2 Integration Tests

**Location**: `tests/integration/mqtt_multi_subscription_test.rs`

**Infrastructure**: Docker Compose with Mosquitto broker

**Coverage Areas**:

#### A. Connection Management (3-4 tests)
- Single connection for multiple subscriptions
- Reconnection with all topic re-subscription
- QoS handling per subscription

#### B. Message Flow (4-5 tests)
- Message routing to correct stream
- Concurrent message handling
- High-throughput stress test
- Parser error recovery

### 2.3 End-to-End Tests

**Location**: `tests/e2e/mqtt_pipeline_test.rs`

**Infrastructure**: Full deployment (MQTT + Parquet + DuckDB)

**Coverage Areas**:

#### A. Pipeline Verification (2-3 tests)
- Air-quality data to correct Parquet partition
- HomeAssistant data to separate partition
- Grafana dashboard data availability

---

## 3. Coverage Targets

### 3.1 Code Coverage by Component

| Component | Target | Priority | Notes |
|-----------|--------|----------|-------|
| `MqttConfig` (new) | 95% | High | All config variants |
| `SubscriptionConfig` (new) | 95% | High | All fields tested |
| `TopicRouter` (new) | 90% | High | Pattern matching critical |
| `MqttSource` (modified) | 85% | High | Core functionality |
| Config parsing | 90% | High | Backward compat critical |
| Error handling | 80% | Medium | All error paths |

### 3.2 Functional Coverage

| Requirement | Test Category | Coverage Target |
|-------------|---------------|-----------------|
| FR-2.1 (Multi-sub config) | Unit | 100% |
| FR-2.2 (Topic routing) | Unit + Integration | 100% |
| FR-2.3 (Message parsing) | Unit | 95% |
| FR-2.4 (Connection mgmt) | Integration | 90% |
| FR-2.5 (Hot-reload) | Integration | 80% (Phase 2) |
| NFR-3.1 (Performance) | Load tests | Benchmarked |
| NFR-3.4 (Backward compat) | Unit + E2E | 100% |

---

## 4. Mock Strategy

### 4.1 Unit Test Mocks

Following AIR-005 patterns, use trait-based mocking:

```rust
// Mock MQTT client for unit tests
#[cfg(test)]
mod tests {
    use mockall::automock;

    #[automock]
    #[async_trait]
    trait MqttClient: Send + Sync {
        async fn subscribe(&self, topic: &str, qos: QoS) -> Result<(), Error>;
        async fn disconnect(&self) -> Result<(), Error>;
    }

    // Mock parser for topic router tests
    #[automock]
    trait Parser: Send + Sync {
        fn parse(&self, json: &Value, ts: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>>;
    }
}
```

### 4.2 Integration Test Infrastructure

```yaml
# docker-compose.test.yml
services:
  mosquitto-test:
    image: eclipse-mosquitto:2.0
    ports:
      - "11883:1883"  # Avoid conflict with production
    volumes:
      - ./tests/fixtures/mosquitto.conf:/mosquitto/config/mosquitto.conf
```

### 4.3 Test Data Fixtures

**Location**: `tests/fixtures/mqtt/`

```
tests/fixtures/mqtt/
  airgradient_message.json     # Standard air quality payload
  homeassistant_state.json     # HomeAssistant state message
  malformed_json.txt           # Invalid JSON for error tests
  large_payload.json           # Stress test payload
```

---

## 5. Test Environment

### 5.1 Unit Test Environment

- No external dependencies
- In-memory channels
- Mocked MQTT client
- Fast execution (< 100ms total)

### 5.2 Integration Test Environment

```bash
# Start test infrastructure
docker-compose -f docker-compose.test.yml up -d

# Run integration tests
cargo test --package air-quality-app --test '*integration*' -- --ignored
```

### 5.3 E2E Test Environment

```bash
# Full deployment required
./deploy/pi/deploy.sh start

# Run E2E tests
cargo test --package air-quality-app --test '*e2e*' -- --ignored
```

---

## 6. CI/CD Integration

### 6.1 GitHub Actions Workflow

```yaml
# .github/workflows/test.yml
jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run unit tests
        run: cargo test --workspace --lib

  integration-tests:
    runs-on: ubuntu-latest
    services:
      mosquitto:
        image: eclipse-mosquitto:2.0
        ports:
          - 11883:1883
    steps:
      - uses: actions/checkout@v4
      - name: Run integration tests
        run: cargo test --workspace --test '*integration*' -- --ignored
```

### 6.2 Test Gates

| Stage | Tests Run | Pass Criteria |
|-------|-----------|---------------|
| PR Check | Unit tests | 100% pass |
| Pre-merge | Unit + Integration | 100% pass |
| Nightly | Unit + Integration + E2E | 100% pass |
| Release | All + Performance | All pass + NFR met |

---

## 7. Test Execution Commands

### 7.1 Development Workflow

```bash
# Run all unit tests (fast feedback loop)
cargo test --package neural-core --lib
cargo test --package air-quality-app --lib

# Run specific test module
cargo test --package neural-core sources::mqtt::tests

# Run with output
cargo test mqtt -- --nocapture

# Run integration tests (requires Docker)
cargo test --test mqtt_multi_subscription_integration -- --ignored

# Run performance benchmarks
cargo bench mqtt_throughput
```

### 7.2 Full Test Suite

```bash
# All tests including integration
cargo test --workspace

# With coverage report
cargo tarpaulin --out Html --output-dir coverage/
```

---

## 8. Performance Testing

### 8.1 Throughput Benchmarks

**Requirement**: NFR-3.1.2 (>= 1000 msg/sec)

```rust
#[bench]
fn bench_mqtt_message_throughput(b: &mut Bencher) {
    // Setup: Multi-subscription MQTT source
    // Measure: Messages processed per second
    // Target: >= 1000 msg/sec
}
```

### 8.2 Latency Benchmarks

**Requirement**: NFR-3.1.1 (< 100ms p95)

```rust
#[bench]
fn bench_mqtt_processing_latency(b: &mut Bencher) {
    // Measure: Time from message receive to channel send
    // Target: < 100ms p95
}
```

### 8.3 Memory Benchmarks

**Requirement**: NFR-3.1.3 (< 10MB per subscription)

```rust
#[test]
fn test_memory_overhead_per_subscription() {
    // Measure: Memory delta per added subscription
    // Target: < 10MB
}
```

---

## 9. Regression Prevention

### 9.1 Backward Compatibility Tests

Every PR must pass these tests:

```rust
#[test]
fn test_legacy_single_topic_config_still_works() {
    // Load existing air-quality config (single topic_pattern)
    // Verify MQTT source starts and subscribes correctly
}

#[test]
fn test_existing_mqtt_tests_unchanged() {
    // All 15 existing mqtt.rs tests must pass unchanged
}
```

### 9.2 Contract Tests

```rust
#[test]
fn test_source_trait_contract_preserved() {
    // MqttSource still implements Source trait
    // fetch() and health_check() behavior unchanged
}

#[test]
fn test_parquet_schema_unchanged() {
    // Output points conform to Bronze layer schema
    // timestamp, location_id, metric, value, tags
}
```

---

## 10. Test Documentation

### 10.1 Test Naming Convention

```
test_<component>_<scenario>_<expected_outcome>
```

Examples:
- `test_topic_router_wildcard_plus_matches_single_level`
- `test_config_parser_duplicate_stream_id_returns_error`
- `test_mqtt_source_reconnect_resubscribes_all_topics`

### 10.2 Test Structure (AAA Pattern)

```rust
#[tokio::test]
async fn test_topic_router_routes_to_correct_stream() {
    // ARRANGE: Setup router with two subscriptions
    let router = TopicRouter::new(vec![
        SubscriptionConfig { stream_id: "air-quality", topic_pattern: "airgradient/+" },
        SubscriptionConfig { stream_id: "homeassistant", topic_pattern: "homeassistant/+/+/state" },
    ]);

    // ACT: Route a message
    let stream_id = router.route("airgradient/abc123", &payload);

    // ASSERT: Verify correct routing
    assert_eq!(stream_id, Some("air-quality"));
}
```

---

## 11. Risk Mitigation

### 11.1 Testing Risks

| Risk | Mitigation |
|------|------------|
| MQTT broker unavailable | Use mock client for unit tests |
| Flaky async tests | Use tokio test utilities, deterministic delays |
| Integration test pollution | Isolated test topics, cleanup between tests |
| Performance regression | Automated benchmarks in CI |

### 11.2 Coverage Gaps

| Gap | Resolution |
|-----|------------|
| Hot-reload testing | Defer to Phase 2, manual verification for MVP |
| Network partition | Mock network failures in unit tests |
| Large-scale load | Separate load test environment |

---

## 12. Success Criteria

The test suite is complete when:

- [ ] All 23 acceptance criteria have corresponding tests
- [ ] Unit test coverage >= 85% for new code
- [ ] All 15 existing MQTT tests pass unchanged
- [ ] Integration tests pass in CI environment
- [ ] Performance benchmarks meet NFR targets
- [ ] No flaky tests (100% deterministic)
- [ ] Test execution time < 5 minutes (unit + integration)

---

## 13. References

- `docs/testing/AIR-005-TEST-DESIGN.md` - London School TDD patterns
- `core/src/sources/mqtt.rs` - Existing MQTT implementation and tests
- `apps/air-quality-app/tests/mqtt_routing_integration_test.rs` - Existing integration tests
- REQUIREMENTS.md - Functional requirements
- ACCEPTANCE_CRITERIA.md - Acceptance criteria (23 scenarios)
