# air-012: Home Assistant Integration - Validation Plan

## Document Information

| Field | Value |
|-------|-------|
| Feature | air-012 |
| Phase | Specification (SPARC S) |
| Version | 1.0.0 |
| Date | 2026-01-30 |
| Author | Specification Agent |
| Status | Draft |

---

## 1. Test Scenarios

### TS-001: MQTT Connection (FR-001)

**Objective:** Verify MQTT broker connectivity and reconnection behavior.

| Test Case | Steps | Expected Result |
|-----------|-------|-----------------|
| TC-001.1: Initial Connection | 1. Start NDP with home-assistant-state stream enabled<br>2. Check logs | Log shows `Connected to MQTT broker 192.168.52.103:1883` |
| TC-001.2: Broker Restart | 1. Restart MQTT broker<br>2. Wait 30 seconds<br>3. Check connection | Auto-reconnect within 30 seconds |
| TC-001.3: Network Interruption | 1. Disconnect network briefly<br>2. Restore network<br>3. Check logs | Reconnection with exponential backoff |

**Validation Method:** Log inspection, health endpoint check

---

### TS-002: Topic Subscription (FR-002)

**Objective:** Verify wildcard subscription captures all target sensors.

| Test Case | Steps | Expected Result |
|-----------|-------|-----------------|
| TC-002.1: Wildcard Match | 1. Publish to `homeassistant/binary_sensor/door_backslider/state`<br>2. Publish to `homeassistant/binary_sensor/door_officewindow/state`<br>3. Check Bronze | Both messages captured |
| TC-002.2: Non-Matching Topic | 1. Publish to `homeassistant/sensor/temperature/state`<br>2. Check Bronze | Message NOT captured |
| TC-002.3: New Sensor | 1. Publish to `homeassistant/binary_sensor/new_sensor/state`<br>2. Check Bronze | Message captured (wildcard works) |

**Validation Method:** Query Bronze Parquet, count messages

---

### TS-003: Message Parsing (FR-003)

**Objective:** Verify correct extraction of ndp_id, entity_id, and state.

| Test Case | Steps | Expected Result |
|-----------|-------|-----------------|
| TC-003.1: ndp_id Extraction | 1. Publish to `homeassistant/binary_sensor/door_backslider/state`<br>2. Query Bronze | `ndp_id = "door_backslider"` |
| TC-003.2: State "on" | 1. Publish payload `on`<br>2. Query Bronze | `raw_payload = "on"` |
| TC-003.3: State "off" | 1. Publish payload `off`<br>2. Query Bronze | `raw_payload = "off"` |
| TC-003.4: Timestamp Assignment | 1. Publish message<br>2. Query Bronze timestamp<br>3. Compare to wall clock | Timestamp within 1 second of publish time |

**Validation Method:** Query Bronze Parquet with DuckDB

---

### TS-004: Bronze Layer Storage (FR-004)

**Objective:** Verify Bronze layer storage format and partitioning.

| Test Case | Steps | Expected Result |
|-----------|-------|-----------------|
| TC-004.1: File Creation | 1. Publish message<br>2. Wait for batch flush<br>3. Check Bronze directory | Parquet file created |
| TC-004.2: Daily Partition | 1. Publish messages on two different days<br>2. Check partition structure | Separate directories per date |
| TC-004.3: Required Fields | 1. Query Bronze Parquet schema | Contains: `timestamp`, `ndp_id`, `topic`, `raw_payload`, `source_stream` |
| TC-004.4: Raw Preservation | 1. Publish message<br>2. Query `raw_payload` | Exact match to MQTT payload |

**Validation Method:** File system inspection, DuckDB schema query

---

### TS-005: Silver Layer ETL (FR-005)

**Objective:** Verify Bronze to Silver transformation.

| Test Case | Steps | Expected Result |
|-----------|-------|-----------------|
| TC-005.1: Table Creation | 1. Run schema migration<br>2. Check table exists | `silver.state_events` exists as hypertable |
| TC-005.2: Field Mapping | 1. Publish MQTT message<br>2. Wait for ETL<br>3. Query Silver | All fields correctly mapped |
| TC-005.3: Deduplication | 1. Insert duplicate (same event_time, ndp_id)<br>2. Query count | Only one row exists |
| TC-005.4: State Values | 1. Publish "on" and "off" messages<br>2. Query Silver | Both states stored correctly |

**Validation Method:** SQL queries against TimescaleDB

**Sample Validation Query:**
```sql
SELECT
    event_time,
    ndp_id,
    source_entity_id,
    state,
    dq_flags
FROM silver.state_events
WHERE source_stream = 'home-assistant-state'
ORDER BY event_time DESC
LIMIT 10;
```

---

### TS-006: Dimension Table (FR-006)

**Objective:** Verify sensor registration in entity_context dimension.

| Test Case | Steps | Expected Result |
|-----------|-------|-----------------|
| TC-006.1: CSV Content | 1. Check `entity_context.csv`<br>2. Verify 3 entries | All 3 sensors present with correct attributes |
| TC-006.2: Sync Command | 1. Run `./deploy.sh sync-dimensions`<br>2. Check status | Successful sync reported |
| TC-006.3: Silver Join | 1. Query state_events JOIN entity_context<br>2. Check enrichment | Category, friendly_name returned |

**Validation Method:** CSV inspection, SQL join query

**Sample Validation Query:**
```sql
SELECT
    s.event_time,
    s.ndp_id,
    s.state,
    e.category,
    e.friendly_name,
    e.orientation
FROM silver.state_events s
LEFT JOIN silver.entity_context e ON s.ndp_id = e.ndp_id
WHERE s.source_stream = 'home-assistant-state';
```

---

### TS-007: Pipeline Health (FR-007)

**Objective:** Verify sparse-data-appropriate freshness monitoring.

| Test Case | Steps | Expected Result |
|-----------|-------|-----------------|
| TC-007.1: Fresh Status | 1. Publish message<br>2. Wait 1 hour<br>3. Check dashboard | Green (Fresh) status |
| TC-007.2: Stale Status | 1. No messages for 20 hours<br>2. Check dashboard | Yellow (Stale) status |
| TC-007.3: Critical Status | 1. No messages for 40 hours<br>2. Check dashboard | Red (Critical) status |
| TC-007.4: No False Alarm | 1. Window stays closed 12 hours<br>2. Check dashboard | Still Green (< 18h threshold) |

**Validation Method:** Dashboard visual inspection, health query

**Sample Health Query:**
```sql
WITH latest AS (
    SELECT
        ndp_id,
        MAX(event_time) AS last_event,
        NOW() - MAX(event_time) AS age
    FROM silver.state_events
    WHERE source_stream = 'home-assistant-state'
    GROUP BY ndp_id
)
SELECT
    ndp_id,
    last_event,
    age,
    CASE
        WHEN age < INTERVAL '18 hours' THEN 'FRESH'
        WHEN age < INTERVAL '36 hours' THEN 'STALE'
        ELSE 'CRITICAL'
    END AS status
FROM latest;
```

---

## 2. Integration Test Approach

### 2.1 Test Environment

| Component | Configuration |
|-----------|---------------|
| MQTT Broker | Local mosquitto or 192.168.52.103 |
| TimescaleDB | Docker container (test instance) |
| Bronze Storage | Temp directory |
| Test Framework | Rust integration tests |

### 2.2 Integration Test Suite

**Location:** `core/tests/integration/home_assistant_state.rs`

```rust
// Pseudo-code for integration test structure

#[tokio::test]
async fn test_mqtt_to_bronze_flow() {
    // 1. Setup: Start MQTT broker, Bronze storage
    // 2. Publish test message
    // 3. Wait for batch flush
    // 4. Assert: Bronze contains expected record
}

#[tokio::test]
async fn test_bronze_to_silver_etl() {
    // 1. Setup: Insert test Bronze record
    // 2. Run ETL
    // 3. Assert: Silver state_events contains transformed record
}

#[tokio::test]
async fn test_end_to_end() {
    // 1. Setup: Full pipeline
    // 2. Publish MQTT message
    // 3. Wait for ETL cycle
    // 4. Assert: Queryable in Silver
}
```

### 2.3 Test Data

**Test Messages:**

| ndp_id | Payload | Expected State |
|--------|---------|----------------|
| `door_backslider` | `on` | open |
| `door_backslider` | `off` | closed |
| `door_officewindow` | `on` | open |
| `door_dinettewindow` | `off` | closed |

---

## 3. Manual Validation Steps

### 3.1 Pre-Deployment Checklist

- [ ] MQTT broker accessible at 192.168.52.103:1883
- [ ] Test subscription with `mosquitto_sub`:
  ```bash
  mosquitto_sub -h 192.168.52.103 -p 1883 \
    -t "homeassistant/binary_sensor/+/state" -v
  ```
- [ ] Trigger sensor state change in Home Assistant
- [ ] Verify message received

### 3.2 Bronze Layer Validation

```bash
# List Bronze files
ls -la data/bronze/home-assistant-state/

# Query Bronze with DuckDB
duckdb <<EOF
SELECT *
FROM parquet_scan('data/bronze/home-assistant-state/**/*.parquet')
ORDER BY timestamp DESC
LIMIT 10;
EOF
```

### 3.3 Silver Layer Validation

```bash
# Connect to TimescaleDB
psql -h localhost -U postgres -d ndp

# Check table exists
\dt silver.state_events

# Query recent events
SELECT * FROM silver.state_events ORDER BY event_time DESC LIMIT 10;

# Check hypertable status
SELECT * FROM timescaledb_information.hypertables
WHERE hypertable_name = 'state_events';
```

### 3.4 Dimension Table Validation

```bash
# Check CSV content
cat data/dimensions/entity_context.csv | grep door

# Sync dimensions
./deploy.sh sync-dimensions

# Query enriched data
psql -h localhost -U postgres -d ndp -c "
SELECT s.ndp_id, s.state, e.friendly_name, e.category
FROM silver.state_events s
JOIN silver.entity_context e ON s.ndp_id = e.ndp_id
LIMIT 5;
"
```

### 3.5 Pipeline Health Validation

```bash
# Check dashboard freshness panel
# Navigate to Grafana > Pipeline Health > State Events

# Manual freshness query
psql -h localhost -U postgres -d ndp -c "
SELECT ndp_id, MAX(event_time) AS last_event,
       NOW() - MAX(event_time) AS age
FROM silver.state_events
GROUP BY ndp_id;
"
```

---

## 4. Success Criteria

### 4.1 Acceptance Gate

| Criterion | Requirement | Validation |
|-----------|-------------|------------|
| Data Flow | MQTT -> Bronze -> Silver | End-to-end test passes |
| Latency | < 35 seconds total | Timing test |
| Reliability | Zero event loss | Message count match |
| Schema | Matches specification | Schema inspection |
| DQ | Invalid states flagged | DQ flag test |

### 4.2 Definition of Done

- [ ] All test cases in TS-001 through TS-007 pass
- [ ] Integration tests pass in CI
- [ ] Manual validation steps complete
- [ ] Pipeline health shows green for all sensors
- [ ] No regressions to existing streams (air-quality)

### 4.3 Rollback Criteria

| Condition | Action |
|-----------|--------|
| Silver table creation fails | Revert migration |
| MQTT connection unstable | Disable stream, investigate |
| Data corruption detected | Stop ETL, restore from Bronze |
| Performance degradation | Disable stream, profile |

---

## 5. Test Execution Schedule

| Phase | Duration | Activities |
|-------|----------|------------|
| Unit Tests | 1 hour | Run `cargo test` |
| Integration Tests | 2 hours | Run integration suite |
| Manual Validation | 1 hour | Execute manual steps |
| Soak Test | 24 hours | Monitor pipeline health overnight |
| Sign-off | 30 min | Review results, approve |

---

## 6. Known Limitations

1. **Timestamp Precision:** Using ingestion time, not Home Assistant event time. Acceptable for correlation analysis (MQTT latency < 100ms).

2. **Sparse Data:** May go hours without events. Requires adjusted monitoring thresholds.

3. **No Historical Backfill:** Only captures events after deployment. Historical state not available.

4. **Single Broker:** No broker failover (internal network acceptable).

---

## 7. References

- SPECIFICATION.md: `/workspaces/neural-data-platform/product/features/air-012/specification/SPECIFICATION.md`
- SCOPE.md: `/workspaces/neural-data-platform/product/features/air-012/SCOPE.md`
- Existing Integration Tests: `/workspaces/neural-data-platform/core/tests/`
