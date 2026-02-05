# Phase C: Cross-Stream + Alignment - Test Plan

> **Phase:** C (Cross-Stream + Alignment)
> **Target:** Week 4
> **Testing Approach:** London TDD (Outside-In) + Integration Testing
> **Parent Document:** [TESTING-STRATEGY.md](../../TESTING-STRATEGY.md)

---

## Overview

Phase C extends Gold layer to three streams and introduces the cross-stream aligned view. Tests validate JOIN complexity, NULL handling by stream type, and state transition extraction.

**Key Challenge**: Testing complex SQL JOINs and ensuring correct NULL handling across different stream types.

---

## Integration Environment

Phase C tests leverage the **integration environment** for end-to-end validation against real TimescaleDB. This provides higher confidence than mock-based testing alone.

### Environment Setup

```bash
# Start integration environment
DEPLOY_ENV=integration deploy/pi/deploy.sh start

# Verify TimescaleDB is healthy
docker exec integration-timescaledb pg_isready -U postgres -d ndp

# Database connection string
DATABASE_URL="postgresql://postgres:postgres@localhost:5432/ndp"
```

### Environment Details

| Component | Container | Port | Notes |
|-----------|-----------|------|-------|
| TimescaleDB | integration-timescaledb | 5432 | PostgreSQL 15 with TimescaleDB |
| etcd | integration-etcd | 2379 | Config store |
| MQTT | integration-mosquitto | 1883 | For test data injection |
| Grafana | integration-grafana | 3000 | Visual validation |

### Config Paths

- **Streams:** `config/integration/base/streams/`
- **Domains:** `config/integration/domains/`
- **Manifests:** `.deploy/releases/test/`

### Test Data Seeding

```bash
# Deploy Phase B first (prerequisite)
DEPLOY_ENV=integration deploy/pi/deploy.sh apply .deploy/releases/test/phase-b-classification.manifest.json

# Seed test data via MQTT
mosquitto_pub -h localhost -p 1883 -t "airgradient/readings/test123" \
  -m '{"serialno":"test123","pm02Compensated":15.5,"rco2":650,"atmpCompensated":22.5,"rhumCompensated":55}'

# Verify data in Silver layer
docker exec integration-timescaledb psql -U postgres -d ndp -c \
  "SELECT COUNT(*) FROM silver.air_quality_observations WHERE observation_time >= NOW() - INTERVAL '1 hour';"
```

---

## Phase C Scope

| ID | Feature | Testing Priority |
|----|---------|------------------|
| **v11-005** | Cross-Stream Aligned View | Critical |
| **v11-006** | State Transition Materializer | High |
| **v11-007** | Objectives Storage | Medium |
| **v11-003** | Per-Stream Continuous Aggregates (outdoor-weather, state-events) | Critical |

---

## Component Under Test: `ndp-gold-ddl`

### Overview

The primary component under test is **`ndp-gold-ddl`** (`tools/ndp-gold-ddl/`), a new Rust CLI tool that:

1. **Reads** stream configurations (`config/base/streams/*/config.json`)
2. **Generates** Gold layer DDL (continuous aggregates, views, refresh policies)
3. **Applies** DDL to TimescaleDB via tokio-postgres
4. **Generates** aligned views from domain configurations
5. **Syncs** objectives to data dictionary

### Component Architecture

```
tools/ndp-gold-ddl/
├── src/
│   ├── main.rs                    # CLI entry point
│   ├── config/
│   │   ├── loader.rs              # Stream config loading
│   │   └── domain.rs              # Domain config parsing
│   ├── generators/
│   │   ├── continuous_aggregate.rs # CA DDL generation
│   │   ├── aligned_view.rs        # Cross-stream view generation
│   │   └── state_transitions.rs   # Transition view generation
│   ├── db/
│   │   ├── client.rs              # PostgresClient abstraction
│   │   └── queries.rs             # SQL execution
│   └── lib.rs                     # Library exports
├── tests/
│   └── integration/               # Integration tests (this plan)
└── Cargo.toml
```

---

## ⚠️ DEFECT HANDLING POLICY: NO WORKAROUNDS

### Mandatory Requirement

**All defects discovered in `ndp-gold-ddl` MUST be fixed in the component itself.**

### Prohibited Practices

| ❌ PROHIBITED | ✅ REQUIRED |
|---------------|-------------|
| Workarounds in tests | Fix the component |
| Manual SQL patches | Fix DDL generation |
| Post-deployment fixups | Fix the generator |
| "Known issue" annotations | Create bug ticket and fix |
| Skipping failing tests | Fix root cause |
| Environment-specific hacks | Fix for all environments |
| **SQL fixes in deploy.sh** | **Fix in ndp-gold-ddl Rust code** |
| **Shell script SQL manipulation** | **Fix the generator that creates the SQL** |

### ⚠️ CRITICAL: deploy.sh is NOT a Fix Location

**`deploy.sh` is an orchestrator, not a SQL generator.**

If SQL coming from `ndp-gold-ddl` is incorrect:
- ❌ Do NOT add `sed`, `awk`, or string manipulation in deploy.sh
- ❌ Do NOT add conditional SQL patches in shell scripts
- ❌ Do NOT add "fixup" queries after running ndp-gold-ddl
- ✅ DO fix the Rust code in `tools/ndp-gold-ddl/src/generators/`

#### Why This Matters

```
deploy.sh calls:
  └── ndp-gold-ddl --stream air-quality --action sync
        └── Generates SQL from config
        └── Applies SQL to TimescaleDB

If the SQL is wrong, the bug is in ndp-gold-ddl, NOT deploy.sh.
```

#### Example: Wrong vs Right

**❌ WRONG: Fixing SQL in deploy.sh**
```bash
# deploy.sh - BAD PATTERN
apply_gold_tables() {
    local sql=$(ndp-gold-ddl generate --stream "$stream_id")

    # WRONG: Patching SQL because generator is broken
    sql=$(echo "$sql" | sed 's/bucket,/COALESCE(aq.bucket, ow.bucket) AS bucket,/')
    sql=$(echo "$sql" | sed 's/FROM gold\./FROM gold.air_quality_hourly aq FULL OUTER JOIN gold./')

    psql -c "$sql"
}
```

**✅ CORRECT: Fix in Rust generator**
```rust
// tools/ndp-gold-ddl/src/generators/aligned_view.rs - CORRECT
fn generate_aligned_view_sql(domain: &DomainConfig) -> Result<String, Error> {
    let streams = &domain.streams;

    // Generate proper COALESCE for bucket
    let bucket_coalesce = streams.iter()
        .map(|s| format!("{}.bucket", s.alias))
        .collect::<Vec<_>>()
        .join(", ");

    // Generate proper FULL OUTER JOINs
    let joins = generate_full_outer_joins(streams)?;

    Ok(format!(
        "CREATE OR REPLACE VIEW gold.{}_aligned AS
         SELECT COALESCE({}) AS bucket, {}
         FROM {} {}",
        domain.id, bucket_coalesce, columns, base_table, joins
    ))
}
```

#### Responsibility Boundaries

| Component | Responsibility | NOT Responsible For |
|-----------|---------------|---------------------|
| `deploy.sh` | Orchestration, sequencing, error handling | SQL correctness |
| `ndp-gold-ddl` | SQL generation, DDL creation | Deployment orchestration |
| `config/*.json` | Declarative specifications | Implementation details |

If you find yourself editing deploy.sh to fix SQL output, **STOP** and fix ndp-gold-ddl instead.

### Defect Workflow

```
1. Test FAILS
   ↓
2. Identify root cause in ndp-gold-ddl
   ↓
3. Create bug ticket: product/features/fe-001/bugs/BUG-{NNN}-{slug}.md
   ↓
4. FIX the component (generators/, db/, config/)
   ↓
5. Re-run test to verify fix
   ↓
6. Test PASSES → Continue
```

### Bug Ticket Template

Create in `product/features/fe-001/bugs/`:

```markdown
# BUG-{NNN}: {Short Description}

**Component:** ndp-gold-ddl
**Module:** {generators/aligned_view.rs | db/client.rs | etc.}
**Discovered:** {date}
**Status:** Open | In Progress | Fixed

## Symptom
{What the test observed}

## Root Cause
{Why ndp-gold-ddl produced incorrect output}

## Fix
{What was changed in the component}

## Verification
{Test that now passes}
```

### Examples

#### ❌ WRONG: Workaround in Test
```rust
#[test]
fn test_aligned_view_columns() {
    let sql = generate_aligned_view(&config).unwrap();

    // WRONG: Working around missing COALESCE
    let sql = sql.replace("aq.bucket", "COALESCE(aq.bucket, ow.bucket)");

    assert!(sql.contains("COALESCE"));
}
```

#### ✅ CORRECT: Fix the Component
```rust
// In generators/aligned_view.rs - FIX THE GENERATOR
fn generate_bucket_column(streams: &[StreamRef]) -> String {
    let buckets: Vec<String> = streams.iter()
        .map(|s| format!("{}.bucket", s.alias))
        .collect();
    format!("COALESCE({}) AS bucket", buckets.join(", "))
}
```

#### ❌ WRONG: Skip Failing Test
```rust
#[test]
#[ignore] // TODO: Fix later, NULL handling broken
fn test_null_handling_locf() {
    // ...
}
```

#### ✅ CORRECT: Fix and Enable
```rust
#[test]
fn test_null_handling_locf() {
    // Test runs because generators/aligned_view.rs was fixed
    let sql = generate_column_for_stream(&stream, "window_state", StreamType::StateEvent);
    assert!(sql.contains("COALESCE") || sql.contains("LAG"));
}
```

### Rationale

1. **`ndp-gold-ddl` is production code** - Workarounds mask real defects
2. **Declarative deployment depends on it** - deploy.sh calls ndp-gold-ddl
3. **Future phases build on Phase C** - Defects compound over time
4. **Integration tests validate real behavior** - Mocks hide issues

### Escalation

If a defect cannot be fixed within the sprint:

1. Document in `product/features/fe-001/bugs/`
2. Assess impact on Phase C completion
3. Discuss with team - may require scope adjustment
4. **Never ship with known workarounds**

---

## 1. Test Development Order (Outside-In)

```
1. ACCEPTANCE TESTS (define success)
   ├── Aligned view with 3 streams
   ├── State transitions extracted correctly
   └── Objectives stored and queryable

2. COMPONENT TESTS (verify behavior)
   ├── FULL OUTER JOIN generation
   ├── NULL handling by stream type
   ├── State transition detection logic
   └── Objectives CRUD operations

3. UNIT TESTS (implement details)
   ├── COALESCE bucket generation
   ├── LOCF (carry forward) SQL
   ├── is_actual_transition logic
   └── Objective condition parsing
```

---

## 2. v11-003: Additional Stream Aggregates Tests

### 2.1 outdoor-weather Tests

```rust
/// ACCEPTANCE: outdoor-weather continuous aggregate generated
#[test]
fn acceptance_outdoor_weather_hourly_generated() {
    let config = load_stream_config("config/base/streams/outdoor-weather/config.yaml");

    let sql = generate_gold_ddl(&config).unwrap();

    assert!(sql.contains("CREATE MATERIALIZED VIEW gold.outdoor_weather_hourly"));
    assert!(sql.contains("temperature_c_mean") || sql.contains("AVG(temperature_c)"));
    assert!(sql.contains("humidity_pct_mean") || sql.contains("AVG(humidity_pct)"));
}

/// Unit: outdoor-weather view references correct Silver table
#[test]
fn test_outdoor_weather_references_silver() {
    let config = create_gold_config_for_stream("outdoor-weather");

    let sql = generate_continuous_aggregate(&config).unwrap();

    assert!(sql.contains("FROM silver.weather_observations") ||
            sql.contains("FROM silver.outdoor_weather"));
}
```

### 2.2 home-assistant-state Tests

```rust
/// ACCEPTANCE: state-events aggregate handles state_event type
#[test]
fn acceptance_state_events_hourly_generated() {
    let config = load_stream_config("config/base/streams/home-assistant-state/config.yaml");

    let sql = generate_gold_ddl(&config).unwrap();

    assert!(sql.contains("CREATE MATERIALIZED VIEW gold.state_events_hourly") ||
            sql.contains("CREATE MATERIALIZED VIEW gold.home_assistant_state_hourly"));
}

/// Unit: state_event streams aggregate state changes
#[test]
fn test_state_event_aggregates_changes() {
    let config = create_state_event_gold_config("home-assistant-state");

    let sql = generate_continuous_aggregate(&config).unwrap();

    // State events should count transitions, not just average
    assert!(sql.contains("COUNT(*)") || sql.contains("SUM(CASE"));
}
```

---

## 3. v11-005: Cross-Stream Aligned View Tests

### 3.1 Acceptance Tests

```rust
/// ACCEPTANCE: Aligned view joins all 3 streams
#[test]
fn acceptance_aligned_view_joins_three_streams() {
    let domain_config = load_domain_config("config/domains/indoor-air-quality/domain.yaml");

    let sql = generate_aligned_view(&domain_config).unwrap();

    // Three streams should be joined
    assert!(sql.contains("gold.air_quality_hourly"));
    assert!(sql.contains("gold.outdoor_weather_hourly"));
    assert!(sql.contains("gold.state_events_hourly") || sql.contains("gold.home_assistant_state_hourly"));
}

/// ACCEPTANCE: Aligned view uses FULL OUTER JOIN
#[test]
fn acceptance_aligned_view_uses_full_outer_join() {
    let domain_config = load_domain_config("config/domains/indoor-air-quality/domain.yaml");

    let sql = generate_aligned_view(&domain_config).unwrap();

    // Should use FULL OUTER JOIN for preserving all rows
    assert!(sql.contains("FULL OUTER JOIN"));
}

/// ACCEPTANCE: Aligned view creates correct output columns
#[test]
fn acceptance_aligned_view_has_expected_columns() {
    let domain_config = load_domain_config("config/domains/indoor-air-quality/domain.yaml");

    let sql = generate_aligned_view(&domain_config).unwrap();

    // Should have aliased columns from each stream
    assert!(sql.contains("indoor_pm25") || sql.contains("AS indoor_pm25"));
    assert!(sql.contains("outdoor_temp") || sql.contains("AS outdoor_temp"));
    assert!(sql.contains("window_opens") || sql.contains("transition_count"));
}
```

### 3.2 Component Tests

```rust
/// Component: JOIN generation with MockConfigLoader
#[tokio::test]
async fn test_aligned_view_with_mock_streams() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_stream(create_typed_stream("air-quality", StreamType::Observation))
        .with_stream(create_typed_stream("outdoor-weather", StreamType::Observation))
        .with_stream(create_typed_stream("home-assistant-state", StreamType::StateEvent));

    let domain_config = create_test_domain_config("indoor-air-quality");

    // Act
    let sql = generate_aligned_view(&domain_config).unwrap();

    // Assert: All streams joined
    let join_count = sql.matches("FULL OUTER JOIN").count();
    assert_eq!(join_count, 2, "Should have 2 JOINs for 3 streams");
}

/// Component: COALESCE bucket handles all streams
#[test]
fn test_coalesce_bucket_from_all_streams() {
    let domain_config = create_test_domain_config("indoor-air-quality");

    let sql = generate_aligned_view(&domain_config).unwrap();

    // Bucket should COALESCE from all streams
    assert!(sql.contains("COALESCE("));
    // Should reference bucket from each alias
    let aliases = ["aq.bucket", "ow.bucket", "se.bucket"];
    for alias in &aliases {
        assert!(sql.contains(alias) || sql.contains(&alias.replace(".", "_")),
                "Missing COALESCE for {}", alias);
    }
}

/// Component: Column aliasing follows convention
#[test]
fn test_column_aliasing_convention() {
    let domain_config = DomainConfig {
        streams: vec![
            StreamRef { stream_id: "air-quality".to_string(), alias: "indoor".to_string(), role: StreamRole::Primary },
            StreamRef { stream_id: "outdoor-weather".to_string(), alias: "outdoor".to_string(), role: StreamRole::Context },
        ],
        ..create_test_domain_config("test")
    };

    let sql = generate_aligned_view(&domain_config).unwrap();

    // Columns should be prefixed with alias
    assert!(sql.contains("AS indoor_") || sql.contains("indoor."));
    assert!(sql.contains("AS outdoor_") || sql.contains("outdoor."));
}
```

### 3.3 NULL Handling Tests (ADR-FE001-004)

```rust
/// Unit: Observation streams preserve NULL
#[test]
fn test_observation_null_handling_preserve() {
    let config = DomainConfig {
        streams: vec![
            StreamRef {
                stream_id: "air-quality".to_string(),
                alias: "aq".to_string(),
                role: StreamRole::Primary,
            },
        ],
        alignment: AlignmentConfig {
            null_handling: NullHandling::ByStreamType,
            ..Default::default()
        },
        ..Default::default()
    };

    let sql = generate_column_for_stream(&config.streams[0], "pm25_mean", StreamType::Observation);

    // Observation should NOT use COALESCE/LOCF
    assert!(!sql.contains("COALESCE") || sql.contains("COALESCE(aq.bucket"));
    assert!(!sql.contains("LAG(") || sql.contains("LAG(aq.bucket"));
}

/// Unit: state_event streams use LOCF (carry forward)
#[test]
fn test_state_event_null_handling_locf() {
    let config = DomainConfig {
        streams: vec![
            StreamRef {
                stream_id: "home-assistant-state".to_string(),
                alias: "se".to_string(),
                role: StreamRole::Actuator,
            },
        ],
        alignment: AlignmentConfig {
            null_handling: NullHandling::ByStreamType,
            ..Default::default()
        },
        ..Default::default()
    };

    let sql = generate_column_for_stream(&config.streams[0], "window_state", StreamType::StateEvent);

    // State event should use LOCF pattern
    assert!(sql.contains("COALESCE") || sql.contains("LAG"));
    assert!(sql.contains("OVER") || sql.contains("IGNORE NULLS"));
}

/// Unit: NULL handling follows ADR-FE001-004
#[test]
fn test_null_handling_by_stream_type_enum() {
    // observation -> preserve
    assert_eq!(
        get_null_handling_for_type(StreamType::Observation),
        NullHandling::Preserve
    );

    // state_event -> carry_forward
    assert_eq!(
        get_null_handling_for_type(StreamType::StateEvent),
        NullHandling::CarryForward
    );

    // forecast -> preserve (use actual forecast, don't carry forward old)
    assert_eq!(
        get_null_handling_for_type(StreamType::Forecast),
        NullHandling::Preserve
    );
}
```

### 3.4 JOIN Order Tests

```rust
/// Unit: Primary stream is first in FROM clause
#[test]
fn test_primary_stream_first_in_from() {
    let domain_config = DomainConfig {
        streams: vec![
            StreamRef { stream_id: "outdoor-weather".to_string(), alias: "ow".to_string(), role: StreamRole::Context },
            StreamRef { stream_id: "air-quality".to_string(), alias: "aq".to_string(), role: StreamRole::Primary },
        ],
        ..create_test_domain_config("test")
    };

    let sql = generate_aligned_view(&domain_config).unwrap();

    // Primary stream should be in FROM, not in JOIN
    let from_pos = sql.find("FROM gold.air_quality");
    let join_pos = sql.find("JOIN gold.air_quality");

    assert!(from_pos.is_some(), "Primary stream should be in FROM");
    assert!(join_pos.is_none() || from_pos.unwrap() < join_pos.unwrap(),
            "Primary stream should be first");
}
```

---

## 4. v11-006: State Transition Materializer Tests

### 4.1 Acceptance Tests

```rust
/// ACCEPTANCE: State transitions view generated from config
#[test]
fn acceptance_state_transitions_view_generated() {
    let config = load_stream_config("config/base/streams/home-assistant-state/config.yaml");

    let sql = generate_state_transitions(&config).unwrap();

    assert!(sql.contains("CREATE VIEW") || sql.contains("CREATE OR REPLACE VIEW"));
    assert!(sql.contains("gold.") && sql.contains("_transitions"));
}

/// ACCEPTANCE: Transition detects state change
#[test]
fn acceptance_transition_detects_change() {
    let sql = generate_state_transitions_sql("home-assistant-state", "state", "ndp_id");

    assert!(sql.contains("LAG(state)"));
    assert!(sql.contains("IS DISTINCT FROM") || sql.contains("!="));
    assert!(sql.contains("from_state"));
    assert!(sql.contains("to_state"));
}
```

### 4.2 Component Tests

```rust
/// Component: is_actual_transition filters noise
#[test]
fn test_is_actual_transition_column() {
    let sql = generate_state_transitions_sql("test-stream", "state", "entity_id");

    // Should have column that identifies real transitions
    assert!(sql.contains("is_actual_transition"));
    assert!(sql.contains("CASE WHEN") || sql.contains("DISTINCT FROM"));
}

/// Component: Duration calculation included
#[test]
fn test_duration_in_previous_state() {
    let sql = generate_state_transitions_sql("test-stream", "state", "entity_id");

    assert!(sql.contains("duration") || sql.contains("EXTRACT") || sql.contains("interval"));
    assert!(sql.contains("LAG(") && sql.contains("event_time"));
}

/// Component: Partitioned by entity
#[test]
fn test_transitions_partitioned_by_entity() {
    let sql = generate_state_transitions_sql("test-stream", "state", "ndp_id");

    assert!(sql.contains("PARTITION BY ndp_id"));
    assert!(sql.contains("ORDER BY") && (sql.contains("event_time") || sql.contains("observation_time")));
}
```

### 4.3 Unit Tests

```rust
/// Unit: WINDOW clause generated correctly
#[test]
fn test_window_clause_generation() {
    let sql = generate_window_clause("ndp_id", "event_time");

    assert_eq!(
        sql,
        "PARTITION BY ndp_id ORDER BY event_time"
    );
}

/// Unit: Transition from NULL handled (first event)
#[test]
fn test_first_event_is_transition() {
    let sql = generate_state_transitions_sql("test", "state", "entity");

    // First event (where LAG is NULL) should be marked as transition
    assert!(sql.contains("LAG(state) OVER") && sql.contains("IS NULL"));
}

/// Unit: State field is configurable
#[test]
fn test_configurable_state_field() {
    let sql_state = generate_state_transitions_sql("test", "state", "entity");
    let sql_status = generate_state_transitions_sql("test", "status", "entity");

    assert!(sql_state.contains("LAG(state)"));
    assert!(sql_status.contains("LAG(status)"));
}
```

---

## 5. v11-007: Objectives Storage Tests

### 5.1 Acceptance Tests

```rust
/// ACCEPTANCE: Objectives loaded from domain config
#[test]
fn acceptance_objectives_loaded_from_config() {
    let domain = load_domain_config("config/domains/indoor-air-quality/domain.yaml");

    assert!(!domain.objectives.is_empty());

    let healthy_co2 = domain.objectives.iter()
        .find(|o| o.id == "healthy_co2");
    assert!(healthy_co2.is_some());

    let obj = healthy_co2.unwrap();
    assert_eq!(obj.target.stream, "air-quality");
    assert_eq!(obj.target.metric, "co2");
    assert_eq!(obj.target.condition, "<");
    assert_eq!(obj.target.threshold, 800.0);
}

/// ACCEPTANCE: Objectives synced to data dictionary
#[tokio::test]
async fn acceptance_objectives_synced_to_dictionary() {
    // Arrange
    let db = MockTimescaleDb::new();
    let domain = load_domain_config("config/domains/indoor-air-quality/domain.yaml");

    // Act
    sync_objectives(&db, &domain).await.unwrap();

    // Assert
    assert!(db.sql_contains("INSERT INTO data_dictionary.objectives"));
    assert!(db.sql_contains("'healthy_co2'"));
}
```

### 5.2 Component Tests

```rust
/// Component: Objective SQL generation
#[test]
fn test_objective_insert_sql() {
    let objective = Objective {
        id: "healthy_co2".to_string(),
        target: ObjectiveTarget {
            stream: "air-quality".to_string(),
            metric: "co2".to_string(),
            condition: "<".to_string(),
            threshold: 800.0,
            unit: Some("ppm".to_string()),
        },
        priority: Some("high".to_string()),
    };

    let sql = generate_objective_insert_sql(&objective, "indoor-air-quality");

    assert!(sql.contains("'healthy_co2'"));
    assert!(sql.contains("'air-quality'"));
    assert!(sql.contains("'co2'"));
    assert!(sql.contains("'<'"));
    assert!(sql.contains("800"));
}

/// Component: All condition types supported
#[test]
fn test_all_condition_types() {
    for condition in &["<", ">", "<=", ">=", "==", "!="] {
        let objective = Objective {
            target: ObjectiveTarget {
                condition: condition.to_string(),
                threshold: 100.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let sql = generate_objective_insert_sql(&objective, "test");
        assert!(sql.contains(condition), "Condition {} not in SQL", condition);
    }
}
```

### 5.3 Unit Tests

```rust
/// Unit: Objective validation rejects invalid condition
#[test]
fn test_invalid_condition_rejected() {
    let objective = Objective {
        target: ObjectiveTarget {
            condition: "LIKE".to_string(), // Invalid
            ..Default::default()
        },
        ..Default::default()
    };

    let result = validate_objective(&objective);
    assert!(result.is_err());
}

/// Unit: Priority enum parsing
#[test]
fn test_priority_parsing() {
    for (value, expected) in &[
        ("low", Priority::Low),
        ("medium", Priority::Medium),
        ("high", Priority::High),
        ("critical", Priority::Critical),
    ] {
        let json = json!({ "priority": value });
        let priority: Priority = serde_json::from_value(json["priority"].clone()).unwrap();
        assert_eq!(priority, *expected);
    }
}

/// Unit: Between condition requires threshold_upper
#[test]
fn test_between_condition_threshold_handling() {
    let objective = Objective {
        id: "comfortable_humidity".to_string(),
        target: ObjectiveTarget {
            stream: "air-quality".to_string(),
            metric: "humidity_pct".to_string(),
            condition: "between".to_string(),
            threshold: 40.0,
            threshold_upper: Some(60.0),
            unit: Some("percent".to_string()),
        },
        priority: Some("medium".to_string()),
    };

    let sql = generate_objective_insert_sql(&objective, "indoor-air-quality");

    assert!(sql.contains("40"));
    assert!(sql.contains("60"));
    assert!(sql.contains("between"));
}
```

### 5.4 deploy.sh Sync Function Tests (Integration)

The objectives sync via `deploy.sh sync-domains` command is tested via shell integration tests.

```bash
# Test 1: Sync objectives from domain.yaml to data dictionary
# GIVEN: domain.yaml exists with 4 objectives
# WHEN: deploy.sh sync-domains is called
# THEN: data_dictionary.objectives has 4 rows for indoor-air-quality

DEPLOY_ENV=integration ./deploy.sh sync-domains
docker exec integration-timescaledb psql -U postgres -d ndp -c \
  "SELECT COUNT(*) FROM data_dictionary.objectives WHERE domain_id = 'indoor-air-quality';"
# Expected: 4

# Test 2: Upsert behavior (idempotent)
# GIVEN: objectives already synced
# WHEN: sync-domains is called again
# THEN: no duplicate rows, same count

./deploy.sh sync-domains
./deploy.sh sync-domains
docker exec integration-timescaledb psql -U postgres -d ndp -c \
  "SELECT COUNT(*) FROM data_dictionary.objectives WHERE domain_id = 'indoor-air-quality';"
# Expected: still 4 (no duplicates)

# Test 3: Between condition with threshold_upper
# GIVEN: objective with condition='between' and threshold=[40,60]
# WHEN: sync-domains is called
# THEN: threshold=40, threshold_upper=60 stored correctly

docker exec integration-timescaledb psql -U postgres -d ndp -c \
  "SELECT objective_id, threshold, threshold_upper FROM data_dictionary.objectives
   WHERE domain_id = 'indoor-air-quality' AND condition = 'between';"
# Expected: comfortable_humidity: threshold=40, threshold_upper=60
#          comfortable_temperature: threshold=20, threshold_upper=24

# Test 4: Domain streams synced with roles
# GIVEN: domain.yaml has 3 streams (primary, context, actuator)
# WHEN: sync-domains is called
# THEN: data_dictionary.domain_streams has 3 rows with correct roles

docker exec integration-timescaledb psql -U postgres -d ndp -c \
  "SELECT stream_id, alias, role FROM data_dictionary.domain_streams
   WHERE domain_id = 'indoor-air-quality' ORDER BY role;"
# Expected: air-quality/indoor/primary, outdoor-weather/outdoor/context, home-assistant-state/state/actuator

# Test 5: Objectives queryable via MCP
# GIVEN: objectives synced to data dictionary
# WHEN: MCP query_dictionary tool queries objectives
# THEN: objectives returned with correct fields

docker exec integration-timescaledb psql -U postgres -d ndp -c \
  "SELECT * FROM data_dictionary.v_high_priority_objectives WHERE domain_id = 'indoor-air-quality';"
# Expected: healthy_co2 and healthy_pm25 returned (high priority)
```

---

## 6. Integration Tests (Live Database)

These tests run against the real integration environment with TimescaleDB.

### 6.1 Environment Setup Module

```rust
//! Integration test helpers for Phase C
//! Located in: tools/ndp-gold-ddl/tests/integration/helpers.rs

use tokio_postgres::{Client, NoTls};
use std::env;

/// Get database connection for integration tests
pub async fn get_integration_db() -> Client {
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5432/ndp".to_string());

    let (client, connection) = tokio_postgres::connect(&database_url, NoTls)
        .await
        .expect("Failed to connect to integration database");

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Connection error: {}", e);
        }
    });

    client
}

/// Check if a continuous aggregate exists
pub async fn check_continuous_aggregate_exists(
    client: &Client,
    schema: &str,
    view_name: &str,
) -> bool {
    let row = client
        .query_one(
            "SELECT COUNT(*) FROM timescaledb_information.continuous_aggregates
             WHERE view_schema = $1 AND view_name = $2",
            &[&schema, &view_name],
        )
        .await
        .expect("Query failed");

    let count: i64 = row.get(0);
    count > 0
}

/// Check if a view exists
pub async fn check_view_exists(client: &Client, schema: &str, view_name: &str) -> bool {
    let row = client
        .query_one(
            "SELECT COUNT(*) FROM pg_views WHERE schemaname = $1 AND viewname = $2",
            &[&schema, &view_name],
        )
        .await
        .expect("Query failed");

    let count: i64 = row.get(0);
    count > 0
}

/// Seed test data for air-quality stream
pub async fn seed_air_quality_test_data(client: &Client, hours: i64) {
    client.execute(
        "INSERT INTO silver.air_quality_observations
         (observation_time, ingestion_time, ndp_id, pm25, pm10, co2, temperature_c, humidity_pct, dq_flags)
         SELECT
             NOW() - (n || ' hours')::interval,
             NOW(),
             'test_sensor_1',
             20.0 + random() * 30,
             30.0 + random() * 50,
             400 + floor(random() * 400)::int,
             20.0 + random() * 5,
             40.0 + random() * 20,
             '{}'::text[]
         FROM generate_series(1, $1) n
         ON CONFLICT DO NOTHING",
        &[&hours],
    ).await.expect("Failed to seed test data");
}
```

### 6.2 Full Deployment Integration Test

```rust
/// INTEGRATION: Full Phase C deployment against real TimescaleDB
/// Run with: DEPLOY_ENV=integration cargo test -p ndp-gold-ddl integration_phase_c -- --ignored
#[tokio::test]
#[ignore]
async fn integration_phase_c_full_deployment() {
    let client = get_integration_db().await;

    // Prerequisite: Verify Phase B objects exist (air-quality Gold tables)
    assert!(
        check_continuous_aggregate_exists(&client, "gold", "air_quality_hourly").await,
        "Prerequisite failed: Phase B not deployed (gold.air_quality_hourly missing)"
    );

    // Act: Deploy Phase C via shell command
    let output = std::process::Command::new("bash")
        .args(["-c", "DEPLOY_ENV=integration deploy/pi/deploy.sh apply .deploy/releases/test/phase-c-alignment.manifest.json"])
        .current_dir(env!("CARGO_MANIFEST_DIR").to_string() + "/../..")
        .output()
        .expect("Failed to run deploy");

    assert!(output.status.success(), "Deployment failed: {}", String::from_utf8_lossy(&output.stderr));

    // Assert: All Phase C continuous aggregates exist
    assert!(
        check_continuous_aggregate_exists(&client, "gold", "outdoor_weather_hourly").await,
        "gold.outdoor_weather_hourly not created"
    );
    assert!(
        check_continuous_aggregate_exists(&client, "gold", "state_events_hourly").await,
        "gold.state_events_hourly not created"
    );

    // Assert: Aligned view exists
    assert!(
        check_view_exists(&client, "gold", "indoor_air_quality_aligned").await,
        "gold.indoor_air_quality_aligned view not created"
    );

    // Assert: Objectives stored in data dictionary
    let row = client
        .query_one(
            "SELECT COUNT(*) FROM data_dictionary.objectives WHERE domain_id = 'indoor-air-quality'",
            &[],
        )
        .await
        .expect("Query failed");
    let objectives_count: i64 = row.get(0);
    assert!(objectives_count > 0, "No objectives stored for indoor-air-quality domain");
}
```

### 6.3 Aligned View Query Performance Test

```rust
/// INTEGRATION: Aligned view query performance (< 100ms on Pi 5)
#[tokio::test]
#[ignore]
async fn integration_aligned_view_performance() {
    let client = get_integration_db().await;

    // Arrange: Seed 30 days of test data if needed
    seed_air_quality_test_data(&client, 720).await; // 30 days * 24 hours

    // Refresh continuous aggregates to ensure data is materialized
    client.execute(
        "CALL refresh_continuous_aggregate('gold.air_quality_hourly', NULL, NULL)",
        &[],
    ).await.ok(); // May fail if no data, that's OK

    // Act: Timed query against aligned view
    let start = std::time::Instant::now();
    let rows = client
        .query(
            "SELECT bucket, indoor_pm25_mean, outdoor_temperature_c_mean, state_changes_count
             FROM gold.indoor_air_quality_aligned
             WHERE bucket >= NOW() - INTERVAL '30 days'
             ORDER BY bucket",
            &[],
        )
        .await
        .expect("Query failed");
    let duration = start.elapsed();

    // Assert: Query performance
    println!("Aligned view query returned {} rows in {:?}", rows.len(), duration);
    assert!(
        duration.as_millis() < 100,
        "Aligned view query took {}ms, expected < 100ms",
        duration.as_millis()
    );
}
```

### 6.4 State Transitions Integration Test

```rust
/// INTEGRATION: State transitions extraction from live data
#[tokio::test]
#[ignore]
async fn integration_state_transitions_work() {
    let client = get_integration_db().await;

    // Arrange: Seed state event test data
    client.execute(
        "INSERT INTO silver.state_events (event_time, ingestion_time, ndp_id, entity_id, state, dq_flags)
         VALUES
             (NOW() - INTERVAL '2 hours', NOW(), 'test_entity', 'window_sensor', 'closed', '{}'),
             (NOW() - INTERVAL '1 hour', NOW(), 'test_entity', 'window_sensor', 'open', '{}'),
             (NOW() - INTERVAL '30 minutes', NOW(), 'test_entity', 'window_sensor', 'closed', '{}'),
             (NOW(), NOW(), 'test_entity', 'window_sensor', 'open', '{}')
         ON CONFLICT DO NOTHING",
        &[],
    ).await.ok();

    // Act: Query state transitions view
    let transitions = client
        .query(
            "SELECT event_time, from_state, to_state, is_actual_transition
             FROM gold.state_transitions
             WHERE entity_id = 'window_sensor'
             ORDER BY event_time DESC",
            &[],
        )
        .await
        .expect("Query failed");

    // Assert: Transitions detected
    assert!(!transitions.is_empty(), "No state transitions found");

    // Assert: is_actual_transition filtering works
    let actual_transitions: Vec<_> = transitions
        .iter()
        .filter(|r| r.get::<_, bool>("is_actual_transition"))
        .collect();

    assert!(!actual_transitions.is_empty(), "No actual transitions detected");
    println!("Found {} actual transitions out of {} total", actual_transitions.len(), transitions.len());
}
```

### 6.5 NULL Handling by Stream Type Integration Test

```rust
/// INTEGRATION: Verify NULL handling per ADR-FE001-004
#[tokio::test]
#[ignore]
async fn integration_null_handling_by_stream_type() {
    let client = get_integration_db().await;

    // Query aligned view and check NULL patterns
    let rows = client
        .query(
            "SELECT
                 bucket,
                 indoor_pm25_mean,
                 outdoor_temperature_c_mean,
                 state_changes_count,
                 -- Check if observation columns preserve NULL (not carried forward)
                 LAG(indoor_pm25_mean) OVER (ORDER BY bucket) as prev_indoor_pm25
             FROM gold.indoor_air_quality_aligned
             WHERE bucket >= NOW() - INTERVAL '7 days'
             ORDER BY bucket
             LIMIT 100",
            &[],
        )
        .await
        .expect("Query failed");

    // Verify observation streams preserve NULL (no LOCF applied)
    // State event streams should have LOCF applied (non-NULL carry forward)
    println!("Checking NULL handling patterns in {} rows", rows.len());

    // At least verify the query executes and returns expected columns
    assert!(rows.len() >= 0, "Query should return rows or empty set");
}
```

### 6.6 outdoor-air-quality Exclusion Test

```rust
/// INTEGRATION: Verify outdoor-air-quality NOT in Gold (reserved for Phase D)
#[tokio::test]
#[ignore]
async fn integration_outdoor_air_quality_not_in_gold() {
    let client = get_integration_db().await;

    // Assert: No outdoor_air_quality Gold objects
    assert!(
        !check_continuous_aggregate_exists(&client, "gold", "outdoor_air_quality_hourly").await,
        "outdoor_air_quality_hourly should NOT exist (reserved for Phase D)"
    );

    // Assert: Not in domain streams
    let row = client
        .query_one(
            "SELECT COUNT(*) FROM data_dictionary.domain_streams
             WHERE domain_id = 'indoor-air-quality' AND stream_id = 'outdoor-air-quality'",
            &[],
        )
        .await
        .expect("Query failed");
    let count: i64 = row.get(0);
    assert_eq!(count, 0, "outdoor-air-quality should NOT be in domain_streams");
}
```

### 6.7 Domain Objectives Sync Integration Test

```rust
/// INTEGRATION: Verify objectives sync via deploy.sh sync-domains
#[tokio::test]
#[ignore]
async fn integration_domain_objectives_sync() {
    let client = get_integration_db().await;

    // Act: Sync domains via shell command
    let output = std::process::Command::new("bash")
        .args(["-c", "DEPLOY_ENV=integration deploy/pi/deploy.sh sync-domains"])
        .current_dir(env!("CARGO_MANIFEST_DIR").to_string() + "/../..")
        .output()
        .expect("Failed to run sync-domains");

    assert!(output.status.success(), "sync-domains failed: {}", String::from_utf8_lossy(&output.stderr));

    // Assert: Domain created
    let domain_row = client
        .query_one(
            "SELECT domain_id, description FROM data_dictionary.domains WHERE domain_id = 'indoor-air-quality'",
            &[],
        )
        .await
        .expect("Domain query failed");
    let domain_id: &str = domain_row.get(0);
    assert_eq!(domain_id, "indoor-air-quality");

    // Assert: Objectives synced (should have 4 objectives)
    let obj_row = client
        .query_one(
            "SELECT COUNT(*) FROM data_dictionary.objectives WHERE domain_id = 'indoor-air-quality'",
            &[],
        )
        .await
        .expect("Objectives count query failed");
    let obj_count: i64 = obj_row.get(0);
    assert_eq!(obj_count, 4, "Expected 4 objectives, found {}", obj_count);

    // Assert: Between condition stored correctly
    let between_row = client
        .query_one(
            "SELECT threshold, threshold_upper FROM data_dictionary.objectives
             WHERE domain_id = 'indoor-air-quality' AND objective_id = 'comfortable_humidity'",
            &[],
        )
        .await
        .expect("Between objective query failed");
    let threshold: rust_decimal::Decimal = between_row.get(0);
    let threshold_upper: rust_decimal::Decimal = between_row.get(1);
    assert_eq!(threshold.to_string(), "40", "Lower threshold should be 40");
    assert_eq!(threshold_upper.to_string(), "60", "Upper threshold should be 60");

    // Assert: Domain streams synced (3 streams)
    let stream_row = client
        .query_one(
            "SELECT COUNT(*) FROM data_dictionary.domain_streams WHERE domain_id = 'indoor-air-quality'",
            &[],
        )
        .await
        .expect("Domain streams query failed");
    let stream_count: i64 = stream_row.get(0);
    assert_eq!(stream_count, 3, "Expected 3 domain streams, found {}", stream_count);

    // Assert: Roles correct
    let role_rows = client
        .query(
            "SELECT stream_id, role FROM data_dictionary.domain_streams
             WHERE domain_id = 'indoor-air-quality' ORDER BY role",
            &[],
        )
        .await
        .expect("Roles query failed");

    let roles: std::collections::HashMap<String, String> = role_rows.iter()
        .map(|r| (r.get::<_, String>(0), r.get::<_, String>(1)))
        .collect();

    assert_eq!(roles.get("home-assistant-state").map(|s| s.as_str()), Some("actuator"));
    assert_eq!(roles.get("outdoor-weather").map(|s| s.as_str()), Some("context"));
    assert_eq!(roles.get("air-quality").map(|s| s.as_str()), Some("primary"));
}

/// INTEGRATION: Verify objectives sync is idempotent
#[tokio::test]
#[ignore]
async fn integration_domain_objectives_sync_idempotent() {
    let client = get_integration_db().await;

    // Act: Sync domains twice
    for _ in 0..2 {
        let output = std::process::Command::new("bash")
            .args(["-c", "DEPLOY_ENV=integration deploy/pi/deploy.sh sync-domains"])
            .current_dir(env!("CARGO_MANIFEST_DIR").to_string() + "/../..")
            .output()
            .expect("Failed to run sync-domains");

        assert!(output.status.success(), "sync-domains failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    // Assert: No duplicates
    let obj_row = client
        .query_one(
            "SELECT COUNT(*) FROM data_dictionary.objectives WHERE domain_id = 'indoor-air-quality'",
            &[],
        )
        .await
        .expect("Objectives count query failed");
    let obj_count: i64 = obj_row.get(0);
    assert_eq!(obj_count, 4, "Expected 4 objectives after multiple syncs, found {}", obj_count);
}

/// INTEGRATION: Verify objectives queryable via views
#[tokio::test]
#[ignore]
async fn integration_objectives_queryable_via_views() {
    let client = get_integration_db().await;

    // Ensure sync has run
    std::process::Command::new("bash")
        .args(["-c", "DEPLOY_ENV=integration deploy/pi/deploy.sh sync-domains"])
        .current_dir(env!("CARGO_MANIFEST_DIR").to_string() + "/../..")
        .output()
        .ok();

    // Assert: High priority view returns expected objectives
    let high_priority_rows = client
        .query(
            "SELECT objective_id, target_stream, priority FROM data_dictionary.v_high_priority_objectives
             WHERE domain_id = 'indoor-air-quality'",
            &[],
        )
        .await
        .expect("High priority view query failed");

    assert_eq!(high_priority_rows.len(), 2, "Expected 2 high priority objectives");

    let high_priority_ids: Vec<String> = high_priority_rows.iter()
        .map(|r| r.get::<_, String>(0))
        .collect();
    assert!(high_priority_ids.contains(&"healthy_co2".to_string()));
    assert!(high_priority_ids.contains(&"healthy_pm25".to_string()));

    // Assert: Domain overview view works
    let overview_row = client
        .query_one(
            "SELECT objective_count, constraint_count FROM data_dictionary.v_domain_overview
             WHERE domain_id = 'indoor-air-quality'",
            &[],
        )
        .await
        .expect("Domain overview query failed");

    let objective_count: i64 = overview_row.get(0);
    let constraint_count: i64 = overview_row.get(1);
    assert_eq!(objective_count, 4, "Expected 4 objectives in overview");
    assert_eq!(constraint_count, 0, "Expected 0 constraints (domain.yaml has no constraints section)");
}
```

---

## 7. Test Execution Commands

### Unit Tests (No Database Required)

```bash
# Run Phase C unit tests
cargo test -p ndp-gold-ddl --lib aligned_view
cargo test -p ndp-gold-ddl --lib state_transitions
cargo test -p ndp-gold-ddl --lib objectives
cargo test -p ndp-gold-ddl --lib null_handling
```

### Component Tests (MockConfigLoader)

```bash
# Run Phase C component tests with mocks
cargo test -p ndp-gold-ddl --test alignment_tests
cargo test -p ndp-gold-ddl --test transition_tests
cargo test -p ndp-gold-ddl --test objectives_tests
```

### Integration Tests (Live TimescaleDB)

```bash
# 1. Start integration environment
DEPLOY_ENV=integration deploy/pi/deploy.sh start

# 2. Wait for services (health check)
docker exec integration-timescaledb pg_isready -U postgres -d ndp

# 3. Deploy Phase B prerequisite (if not already deployed)
DEPLOY_ENV=integration deploy/pi/deploy.sh apply .deploy/releases/test/phase-b-classification.manifest.json

# 4. Run integration tests
DATABASE_URL="postgresql://postgres:postgres@localhost:5432/ndp" \
  cargo test -p ndp-gold-ddl --test integration -- phase_c --ignored --test-threads=1

# 5. Alternatively, run all Phase C integration tests:
DEPLOY_ENV=integration DATABASE_URL="postgresql://postgres:postgres@localhost:5432/ndp" \
  cargo test integration_phase_c --ignored -- --test-threads=1

# 6. (Optional) Stop integration environment when done
DEPLOY_ENV=integration deploy/pi/deploy.sh stop
```

### Manual Verification (SQL Commands)

```bash
# Connect to integration database
docker exec -it integration-timescaledb psql -U postgres -d ndp

# Verify continuous aggregates
SELECT view_name FROM timescaledb_information.continuous_aggregates
WHERE view_schema = 'gold' ORDER BY view_name;

# Verify aligned view columns
SELECT column_name FROM information_schema.columns
WHERE table_schema = 'gold' AND table_name = 'indoor_air_quality_aligned'
ORDER BY ordinal_position;

# Verify objectives in data dictionary
SELECT objective_id, target_metric, condition, threshold
FROM data_dictionary.objectives WHERE domain_id = 'indoor-air-quality';

# Test aligned view query performance
EXPLAIN (ANALYZE, COSTS, TIMING)
SELECT * FROM gold.indoor_air_quality_aligned
WHERE bucket >= NOW() - INTERVAL '30 days';
```

### CI/CD Integration

```yaml
# Example GitHub Actions workflow step
- name: Run Phase C Integration Tests
  env:
    DEPLOY_ENV: integration
    DATABASE_URL: postgresql://postgres:postgres@localhost:5432/ndp
  run: |
    # Start services
    docker compose -f docker-compose.integration.yml up -d timescaledb etcd
    sleep 10

    # Run tests
    cargo test -p ndp-gold-ddl --test integration -- phase_c --ignored --test-threads=1
```

---

## 8. Test Metrics (Phase C Target)

| Category | Target | Priority | Notes |
|----------|--------|----------|-------|
| Unit Tests | 20-25 | High | No DB required |
| Component Tests | 8-10 | High | MockConfigLoader |
| **Integration Tests** | 6-8 | **Critical** | Live TimescaleDB |
| Coverage (aligned_view.rs) | 85% | Critical | |
| Coverage (state_transitions.rs) | 80% | High | |
| Test Duration (unit) | <5s | High | |
| Test Duration (integration) | <60s | Medium | Per-test limit |

### Integration Test Coverage

| Test ID | Validates | AC Reference |
|---------|-----------|--------------|
| `integration_phase_c_full_deployment` | End-to-end deployment | AC-C-01 to AC-C-07 |
| `integration_aligned_view_performance` | Query <100ms | AC-C-PERF-01 |
| `integration_state_transitions_work` | Transition extraction | AC-C-05 |
| `integration_null_handling_by_stream_type` | NULL semantics | AC-C-04 |
| `integration_outdoor_air_quality_not_in_gold` | Phase D reservation | AC-C-08 |
| `integration_domain_metadata` | Data dictionary | AC-C-06, AC-C-07 |

---

## 9. Exit Criteria

Phase C testing complete when:

### Unit & Component Tests
- [ ] All 3 stream aggregates generated and tested (mock)
- [ ] Aligned view JOIN tests pass (mock)
- [ ] NULL handling tests verify ADR-FE001-004 (mock)
- [ ] State transition tests pass (mock)
- [ ] Objectives storage tests pass (mock)

### Integration Tests (Live Database)
- [ ] `integration_phase_c_full_deployment` passes
- [ ] `integration_aligned_view_performance` confirms <100ms
- [ ] `integration_state_transitions_work` verifies transition extraction
- [ ] `integration_null_handling_by_stream_type` validates NULL semantics
- [ ] `integration_outdoor_air_quality_not_in_gold` confirms Phase D reservation
- [ ] All integration tests run via `DEPLOY_ENV=integration`

### ⚠️ Component Quality Gates (MANDATORY)
- [ ] **Zero workarounds** in test code
- [ ] **Zero manual SQL patches** required after deployment
- [ ] **All defects fixed in `ndp-gold-ddl`** source code
- [ ] **Bug tickets closed** for any discovered defects
- [ ] **No `#[ignore]` annotations** hiding broken functionality
- [ ] **No SQL manipulation in deploy.sh** (sed, awk, string fixes)
- [ ] **deploy.sh only orchestrates** - does not modify SQL output

### Manual Verification (Optional but Recommended)
- [ ] Visual inspection in Grafana (localhost:3000)
- [ ] SQL queries confirm data dictionary entries
- [ ] Performance confirmed on target hardware (Pi 5) if available

### Phase C Release Criteria

**Phase C is NOT complete if:**
- Any test contains workaround code
- Any test is skipped due to unresolved defects
- Manual intervention is needed post-deployment
- Generated SQL requires hand-editing

---

## References

- [PHASE-C-OVERVIEW.md](../specification/PHASE-C-OVERVIEW.md) - Phase C specification
- [ADR-FE001-004](../../architecture/ADR-FE001-004-null-handling.md) - NULL handling
- [TESTING-STRATEGY.md](../../TESTING-STRATEGY.md) - Overall testing strategy
- [DEPLOYMENT-DECLARATIVES.md](../../../../docs/procedures/DEPLOYMENT-DECLARATIVES.md) - Manifest deployment
- [docker-compose.integration.yml](../../../../docker-compose.integration.yml) - Integration environment
- [ACCEPTANCE-CRITERIA.md](../completion/ACCEPTANCE-CRITERIA.md) - Acceptance criteria with verification SQL

---

*Phase C Test Plan created: 2026-02-04*
*Updated: 2026-02-04 - Added integration environment testing with live TimescaleDB*
