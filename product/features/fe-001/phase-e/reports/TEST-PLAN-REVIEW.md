# Phase E Test Plan Review

> **Reviewer:** ndp-tester
> **Date:** 2026-02-05
> **Phase:** E (Unified Event Abstraction)
> **Documents Reviewed:**
> - TEST-PLAN.md
> - V12-HANDOFF-CHECKLIST.md
> - ACCEPTANCE-CRITERIA.md
> - PHASE-E-OVERVIEW.md
> - TESTING-STRATEGY.md

---

## Executive Summary

The Phase E test plan is **well-structured** and follows London TDD principles correctly. However, this review identifies **several gaps** that must be addressed before Phase E testing can be considered complete.

### Overall Assessment

| Aspect | Rating | Notes |
|--------|--------|-------|
| Test Coverage Breadth | 3/5 | Missing v11-014 (dashboard) tests |
| London TDD Adherence | 4/5 | Good outside-in structure |
| V1.2 Contract Tests | 4/5 | Strong contract definition |
| Integration Test Completeness | 3/5 | Missing CI/CD integration |
| Mocking Strategy | 2/5 | Undefined for Phase E |
| Performance Test Coverage | 4/5 | Good Pi resource tests |

**Verdict:** CONDITIONAL PASS - Address identified gaps before implementation.

---

## 1. Test Coverage Analysis

### 1.1 Feature Coverage Matrix

| Feature ID | Feature Name | Unit Tests | Component Tests | Integration Tests | Status |
|------------|--------------|------------|-----------------|-------------------|--------|
| **v11-012** | Threshold Crossing Generator | 12 specified | 4 specified | 2 specified | COVERED |
| **v11-013** | Unified Events View | 10 specified | 5 specified | 3 specified | COVERED |
| **v11-014** | Gold Layer Dashboard | **0 specified** | **0 specified** | **0 specified** | **GAP** |
| **v11-V02** | New Feature Type Test | 0 specified | 0 specified | 0 specified | Deferred to Phase D |

### 1.2 Coverage Gap: v11-014 Dashboard Tests

**Critical Finding:** The TEST-PLAN.md does not include tests for v11-014 (Gold Layer Dashboard), despite it being listed in ACCEPTANCE-CRITERIA.md with 6 acceptance criteria (AC-E-09 through AC-E-14).

**Required Tests for v11-014:**

```rust
// Dashboard Unit Tests (config validation)
#[test]
fn test_dashboard_json_valid_syntax() { ... }

#[test]
fn test_dashboard_references_valid_datasources() { ... }

#[test]
fn test_dashboard_queries_use_gold_schema() { ... }

// Dashboard Integration Tests
#[tokio::test]
#[ignore]
async fn integration_dashboard_import_succeeds() { ... }

#[tokio::test]
#[ignore]
async fn integration_dashboard_panels_return_data() { ... }

#[tokio::test]
#[ignore]
async fn integration_dashboard_load_time_acceptable() { ... }
```

---

## 2. Alignment with Acceptance Criteria

### 2.1 Acceptance Criteria Coverage

| AC ID | Description | Test Plan Coverage | Gap |
|-------|-------------|-------------------|-----|
| AC-E-01 | Threshold crossing generator works | Section 2 | None |
| AC-E-02 | All condition types supported | Section 2.3 | None |
| AC-E-03 | Unified events view combines types | Section 3 | None |
| AC-E-04 | Event schema contract met | Section 3.2 | None |
| AC-E-05 | Hourly event aggregate available | Section 3.3 | None |
| AC-E-06 | V1.2 query patterns work | Section 3.2 | None |
| AC-E-07 | Crossing frequency observable | **NOT COVERED** | **Add observability tests** |
| AC-E-08 | Event volume monitoring | **NOT COVERED** | **Add monitoring tests** |
| AC-E-09 | Dashboard deployed | **NOT COVERED** | **Add dashboard tests** |
| AC-E-10 | Gold aggregates visible | **NOT COVERED** | **Add dashboard tests** |
| AC-E-11 | Aligned view panel works | **NOT COVERED** | **Add dashboard tests** |
| AC-E-12 | Unified events visible | **NOT COVERED** | **Add dashboard tests** |
| AC-E-13 | Objective thresholds visible | **NOT COVERED** | **Add dashboard tests** |
| AC-E-14 | Dashboard performance | **NOT COVERED** | **Add dashboard tests** |
| AC-E-PERF-01 | Events query < 100ms | Section 4 | None |
| AC-E-PERF-02 | Crossing detection < 5% overhead | **PARTIALLY COVERED** | Add specific test |
| AC-E-PERF-03 | Resource usage within budget | Section 4 | None |
| AC-E-INT-01 | Phase E deployment works | Section 4 | None |
| AC-E-INT-02 | Event schema backward compatible | Section 4 | None |
| AC-E-V12-01 | V1.2 interface contract met | Section 3.2 | None |
| AC-E-V12-02 | Documentation complete | **NOT TESTABLE** | Manual review |

### 2.2 Missing Test Categories

1. **Observability Tests (AC-E-07, AC-E-08)**
   - Crossing frequency query validation
   - Event volume alerting threshold tests
   - Oscillation pattern detection tests

2. **Dashboard Tests (AC-E-09 through AC-E-14)**
   - Dashboard JSON validation
   - Panel data availability
   - Performance on Pi 5

---

## 3. London TDD Approach Review

### 3.1 Outside-In Structure

**Strength:** The test plan correctly specifies the outside-in order:

```
1. ACCEPTANCE TESTS (define V1.2 interface)
2. COMPONENT TESTS (verify behavior)
3. UNIT TESTS (implement details)
```

This aligns with the project's TESTING-STRATEGY.md.

### 3.2 Test Naming Convention

**Strength:** Tests follow the established pattern:
- Acceptance: `acceptance_<feature>_<behavior>`
- Component: `test_<component>_<behavior>`
- Unit: `test_<function>_<scenario>`

### 3.3 Arrange-Act-Assert Structure

**Strength:** All test examples follow AAA pattern correctly.

---

## 4. Mocking Strategy Gaps

### 4.1 Missing Mock Definitions

The TEST-PLAN.md references functions that would need mocks but doesn't define them:

| Function | Required Mock | Status |
|----------|---------------|--------|
| `load_domain_config()` | MockConfigLoader | **Exists** in core |
| `generate_threshold_crossings()` | None needed | N/A |
| `generate_unified_events_view()` | None needed | N/A |
| `check_view_exists()` | MockTimescaleDb | **NOT DEFINED** |
| `deploy_manifest()` | MockDeployHandler | **NOT DEFINED** |

### 4.2 Recommended Mock Additions

```rust
// tools/ndp-gold-ddl/src/mocks/events.rs

pub struct MockEventStore {
    events: RwLock<Vec<Event>>,
}

impl MockEventStore {
    pub fn new() -> Self { ... }

    pub fn with_events(self, events: Vec<Event>) -> Self { ... }

    pub fn with_crossing_events(self, count: usize) -> Self {
        // Generate test crossing events
    }

    pub fn with_transition_events(self, count: usize) -> Self {
        // Generate test transition events
    }
}
```

---

## 5. Integration Test Gaps

### 5.1 CI/CD Integration

**Gap:** The TEST-PLAN.md does not specify GitHub Actions workflow integration.

**Required Addition:**

```yaml
# .github/workflows/phase-e-tests.yml
name: Phase E Tests

on:
  pull_request:
    paths:
      - 'tools/ndp-gold-ddl/src/generators/events.rs'
      - 'config/domains/**/objectives*'

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - name: Run Phase E unit tests
        run: |
          cargo test -p ndp-gold-ddl threshold_crossing
          cargo test -p ndp-gold-ddl unified_events

  integration-tests:
    runs-on: ubuntu-latest
    services:
      timescaledb:
        image: timescale/timescaledb:latest-pg15
    steps:
      - name: Run Phase E integration tests
        env:
          DEPLOY_ENV: integration
        run: cargo test -p ndp-gold-ddl --test integration -- phase_e --ignored
```

### 5.2 Test Data Setup

**Gap:** The integration tests reference `setup_threshold_crossing_test_data()` but this helper is not defined.

**Required Test Helper:**

```rust
// tools/ndp-gold-ddl/tests/helpers/setup.rs

pub async fn setup_threshold_crossing_test_data() -> Result<(), TestError> {
    // Insert Silver layer data that will generate crossings
    let sql = r#"
        INSERT INTO silver.air_quality (event_time, ndp_id, co2)
        VALUES
            (NOW() - INTERVAL '2 hours', 'sensor-1', 750),  -- Below threshold
            (NOW() - INTERVAL '1 hour', 'sensor-1', 850),   -- Above threshold (crossing!)
            (NOW(), 'sensor-1', 780);                        -- Below threshold (crossing!)
    "#;
    execute_sql(sql).await
}
```

---

## 6. Performance Test Analysis

### 6.1 Specified Performance Tests

| Test | Target | Method | Coverage |
|------|--------|--------|----------|
| 30-day events query | < 100ms | `std::time::Instant` | GOOD |
| Crossing detection overhead | < 5% | Comparative timing | PARTIAL |
| Events storage | < 50 MB | Size query | GOOD |

### 6.2 Missing Performance Tests

1. **Dashboard Load Time (AC-E-14)**
   ```rust
   #[tokio::test]
   #[ignore]
   async fn test_dashboard_load_time_under_3_seconds() {
       let start = Instant::now();
       // Load dashboard via API
       let duration = start.elapsed();
       assert!(duration.as_secs() < 3);
   }
   ```

2. **Pi 5 Resource Constraints**
   ```rust
   #[tokio::test]
   #[ignore]
   async fn test_events_memory_usage_within_budget() {
       // Memory should not exceed 512MB during event processing
       let memory_before = get_process_memory();
       // Run event processing
       let memory_after = get_process_memory();
       assert!(memory_after - memory_before < 100_000_000); // 100 MB headroom
   }
   ```

---

## 7. V1.2 Contract Test Review

### 7.1 Strengths

- Clear V1.2 query patterns defined
- Schema contract explicitly stated
- JSONB details structure validated

### 7.2 Potential Issues

1. **event_id Generation**
   - Test assumes UUID generation but doesn't validate uniqueness
   - Add: `test_event_id_unique_across_types()`

2. **Time Zone Handling**
   - Tests don't verify TIMESTAMPTZ behavior across time zones
   - Add: `test_event_time_timezone_consistent()`

---

## 8. Exit Criteria Alignment

### 8.1 Test Plan Exit Criteria vs Handoff Checklist

| Exit Criterion | Test Plan | Handoff Checklist | Aligned |
|----------------|-----------|-------------------|---------|
| Threshold crossing tests pass | Yes | Yes | YES |
| Unified events tests pass | Yes | Yes | YES |
| V1.2 contract tests pass | Yes | Yes | YES |
| All condition types supported | Yes | No explicit check | PARTIAL |
| JSONB details validated | Yes | Yes | YES |
| Events hourly aggregate tests | Yes | Yes | YES |
| Integration tests pass | Yes | Yes | YES |
| Performance requirements met | Yes | Yes | YES |
| **V1.2 handoff checklist complete** | No | Yes | **GAP** |

---

## 9. Recommendations

### 9.1 Critical (Must Fix Before Implementation)

1. **Add v11-014 Dashboard Test Section**
   - Create Section 5 in TEST-PLAN.md for dashboard tests
   - Cover AC-E-09 through AC-E-14
   - Include JSON validation, panel tests, performance tests

2. **Add Observability Test Section**
   - Create tests for AC-E-07 (crossing frequency)
   - Create tests for AC-E-08 (event volume monitoring)

3. **Define Mocking Strategy for Events**
   - Add MockEventStore to testing infrastructure
   - Document mock usage patterns

### 9.2 Important (Should Fix)

4. **Add CI/CD Workflow Specification**
   - Define GitHub Actions workflow for Phase E
   - Include both unit and integration test jobs

5. **Add Test Data Setup Helpers**
   - Define `setup_threshold_crossing_test_data()`
   - Define `setup_unified_events_test_data()`

6. **Add V1.2 Handoff Verification Test**
   - Automated test that runs handoff checklist queries
   - Outputs pass/fail for each checklist item

### 9.3 Nice to Have (Consider)

7. **Add Property-Based Tests**
   - Use `proptest` for threshold crossing edge cases
   - Test all condition type combinations

8. **Add Mutation Testing**
   - Use `cargo-mutants` to verify test effectiveness
   - Target 80%+ mutation score

---

## 10. Test Metrics Summary

### Current State (from TEST-PLAN.md)

| Category | Specified Count | Target |
|----------|-----------------|--------|
| Threshold Crossing Tests | 12-15 | Met |
| Unified Events Tests | 10-12 | Met |
| V1.2 Contract Tests | 5-8 | Met |
| Integration Tests | 3-5 | Met |
| **Dashboard Tests** | **0** | **6+ needed** |
| **Observability Tests** | **0** | **4+ needed** |

### Revised Targets

| Category | Current | Revised Target | Gap |
|----------|---------|----------------|-----|
| Unit Tests | ~35 | 45 | +10 |
| Component Tests | ~12 | 16 | +4 |
| Integration Tests | ~5 | 10 | +5 |
| **Total** | **~52** | **71** | **+19** |

---

## 11. Conclusion

The Phase E TEST-PLAN.md provides a solid foundation for testing the core event functionality (v11-012, v11-013). However, significant gaps exist in:

1. Dashboard testing (v11-014) - **Not covered at all**
2. Observability testing (AC-E-07, AC-E-08) - **Not covered**
3. Mocking strategy for events - **Not defined**
4. CI/CD integration - **Not specified**

**Recommendation:** Address the critical gaps identified in Section 9.1 before beginning Phase E implementation. The test plan should be updated to include dashboard tests and observability tests to ensure full coverage of the acceptance criteria.

---

## Appendix A: Missing Test Templates

### A.1 Dashboard Unit Test Template

```rust
// tools/ndp-gold-ddl/tests/dashboard_tests.rs

/// ACCEPTANCE: Dashboard JSON is valid
#[test]
fn test_dashboard_json_valid() {
    let dashboard_path = "config/grafana/dashboards/gold-layer-overview.json";
    let content = std::fs::read_to_string(dashboard_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert!(parsed.get("title").is_some(), "Dashboard should have title");
    assert!(parsed.get("panels").is_some(), "Dashboard should have panels");
}

/// ACCEPTANCE: Dashboard queries reference Gold schema
#[test]
fn test_dashboard_queries_gold_schema() {
    let dashboard = load_dashboard_json();

    for panel in dashboard.panels() {
        if let Some(query) = panel.query() {
            assert!(
                query.contains("gold.") || query.contains("FROM gold"),
                "Panel '{}' should query Gold schema", panel.title
            );
        }
    }
}
```

### A.2 Observability Test Template

```rust
// tools/ndp-gold-ddl/tests/observability_tests.rs

/// ACCEPTANCE: Crossing frequency queryable (AC-E-07)
#[tokio::test]
#[ignore]
async fn test_crossing_frequency_queryable() {
    let sql = r#"
        SELECT
            DATE_TRUNC('day', event_time) as day,
            details->>'objective_id' as objective,
            COUNT(*) as crossing_count
        FROM gold.events_unified
        WHERE event_type = 'threshold_crossing'
        AND event_time >= NOW() - INTERVAL '7 days'
        GROUP BY DATE_TRUNC('day', event_time), details->>'objective_id'
    "#;

    let result = query_sql(sql).await;
    assert!(result.is_ok(), "Crossing frequency query should execute");
}

/// ACCEPTANCE: Event volume monitoring works (AC-E-08)
#[tokio::test]
#[ignore]
async fn test_event_volume_monitoring() {
    let sql = r#"
        SELECT
            DATE_TRUNC('hour', event_time) as hour,
            COUNT(*) as event_count
        FROM gold.events_unified
        WHERE event_time >= NOW() - INTERVAL '24 hours'
        GROUP BY DATE_TRUNC('hour', event_time)
    "#;

    let result = query_sql(sql).await;
    assert!(result.is_ok(), "Event volume query should execute");
}
```

---

## Appendix B: Test Plan Update Checklist

Before Phase E implementation begins, the TEST-PLAN.md should be updated to include:

- [ ] Section 5: Dashboard Tests (v11-014)
  - [ ] Unit tests for JSON validation
  - [ ] Component tests for query correctness
  - [ ] Integration tests for panel data availability
  - [ ] Performance tests for load time

- [ ] Section 6: Observability Tests
  - [ ] Crossing frequency tests
  - [ ] Event volume monitoring tests
  - [ ] Oscillation detection tests

- [ ] Section 7: Mocking Strategy
  - [ ] MockEventStore definition
  - [ ] Test data generators

- [ ] Section 8: CI/CD Integration
  - [ ] GitHub Actions workflow
  - [ ] Test isolation requirements

---

*Review completed: 2026-02-05 by ndp-tester*
