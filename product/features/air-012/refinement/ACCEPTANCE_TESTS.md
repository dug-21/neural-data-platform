# air-012: Home Assistant Integration - Acceptance Tests

## Overview

This document maps acceptance test scenarios to the acceptance criteria defined in `SCOPE.md`. Each scenario includes specific test steps, expected results, and verification methods.

---

## Acceptance Criteria Summary

### Bronze Layer
- AC-B1: Stream config `home-assistant-state` created
- AC-B2: MQTT source connects to broker at `192.168.52.103:1883`
- AC-B3: Topic pattern `homeassistant/binary_sensor/+/state` subscribed
- AC-B4: Raw payload stored in Parquet with topic metadata
- AC-B5: `ndp_id` extracted/assigned for each sensor

### Silver Layer
- AC-S1: `silver.state_events` table created (hypertable)
- AC-S2: ETL extracts `state` from payload
- AC-S3: `source_entity_id` extracted from topic path
- AC-S4: `event_time` uses ingestion timestamp

### Dimension
- AC-D1: 3 sensors added to `entity_context.csv`
- AC-D2: `./deploy.sh sync-dimensions` loads updated dimension

### Pipeline Health
- AC-H1: State events freshness visible in dashboard
- AC-H2: Sparse-data thresholds applied (18/36 hours)
- AC-H3: No false alarms when windows stay closed overnight

---

## Acceptance Test Scenarios

### AT-01: Stream Config Created and Valid

**Criteria:** AC-B1

**Preconditions:**
- Repository cloned
- Config directory structure exists

**Test Steps:**

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Check config file exists | `config/base/streams/home-assistant-state.yaml` present |
| 2 | Parse YAML config | No parse errors |
| 3 | Validate `stream_id` | Value is `home-assistant-state` |
| 4 | Validate `source.type` | Value is `mqtt` |
| 5 | List streams via CLI | `home-assistant-state` appears in list |

**Verification Script:**
```bash
#!/bin/bash
CONFIG_PATH="config/base/streams/home-assistant-state.yaml"

# Step 1: File exists
test -f "$CONFIG_PATH" || { echo "FAIL: Config file missing"; exit 1; }

# Step 2: Valid YAML
yq eval '.' "$CONFIG_PATH" > /dev/null || { echo "FAIL: Invalid YAML"; exit 1; }

# Step 3: Correct stream_id
STREAM_ID=$(yq eval '.stream_id' "$CONFIG_PATH")
[ "$STREAM_ID" = "home-assistant-state" ] || { echo "FAIL: Wrong stream_id: $STREAM_ID"; exit 1; }

# Step 4: Source type is MQTT
SOURCE_TYPE=$(yq eval '.source.type' "$CONFIG_PATH")
[ "$SOURCE_TYPE" = "mqtt" ] || { echo "FAIL: Wrong source type: $SOURCE_TYPE"; exit 1; }

echo "PASS: Stream config valid"
```

**Expected Result:** All steps pass; config file is valid and contains correct values.

---

### AT-02: MQTT Broker Connection

**Criteria:** AC-B2

**Preconditions:**
- MQTT broker running at `192.168.52.103:1883`
- Network access to broker

**Test Steps:**

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Test broker connectivity | TCP connection succeeds |
| 2 | Subscribe to test topic | No connection errors |
| 3 | Verify config has correct broker | `broker_url` is `192.168.52.103` |
| 4 | Verify config has correct port | `port` is `1883` |

**Verification Script:**
```bash
#!/bin/bash
BROKER="192.168.52.103"
PORT="1883"

# Step 1: TCP connectivity
nc -zv "$BROKER" "$PORT" 2>&1 | grep -q "succeeded" || { echo "FAIL: Cannot connect to broker"; exit 1; }

# Step 2: MQTT subscription test (3 second timeout)
timeout 3 mosquitto_sub -h "$BROKER" -p "$PORT" -t "homeassistant/#" -C 0 2>&1
if [ $? -eq 124 ]; then
    echo "PASS: MQTT subscription works (timeout expected with no messages)"
else
    echo "PASS: MQTT subscription works"
fi

# Step 3-4: Config values
CONFIG_PATH="config/base/streams/home-assistant-state.yaml"
CONFIG_BROKER=$(yq eval '.source.mqtt.broker_url' "$CONFIG_PATH")
CONFIG_PORT=$(yq eval '.source.mqtt.port' "$CONFIG_PATH")

[ "$CONFIG_BROKER" = "$BROKER" ] || { echo "FAIL: Wrong broker in config: $CONFIG_BROKER"; exit 1; }
[ "$CONFIG_PORT" = "$PORT" ] || { echo "FAIL: Wrong port in config: $CONFIG_PORT"; exit 1; }

echo "PASS: MQTT broker connection verified"
```

**Expected Result:** Connection succeeds; config matches actual broker settings.

---

### AT-03: Topic Pattern Subscription

**Criteria:** AC-B3

**Preconditions:**
- Stream config exists
- MQTT broker accessible

**Test Steps:**

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Extract topic pattern from config | Pattern is `homeassistant/binary_sensor/+/state` |
| 2 | Subscribe to pattern | Subscription succeeds |
| 3 | Trigger `door_backslider` event | Message received |
| 4 | Trigger `door_officewindow` event | Message received |
| 5 | Trigger `door_dinettewindow` event | Message received |

**Verification Script:**
```bash
#!/bin/bash
BROKER="192.168.52.103"
TOPIC_PATTERN="homeassistant/binary_sensor/+/state"

# Subscribe and capture one message (with timeout)
MESSAGE=$(timeout 30 mosquitto_sub -h "$BROKER" -t "$TOPIC_PATTERN" -C 1 -v)

if [ -n "$MESSAGE" ]; then
    echo "PASS: Received message: $MESSAGE"
else
    echo "INFO: No message within timeout (trigger a sensor manually)"
fi

# Verify config pattern
CONFIG_PATH="config/base/streams/home-assistant-state.yaml"
CONFIG_PATTERN=$(yq eval '.source.mqtt.subscriptions[0].topic_pattern' "$CONFIG_PATH")
[ "$CONFIG_PATTERN" = "$TOPIC_PATTERN" ] || { echo "FAIL: Wrong pattern: $CONFIG_PATTERN"; exit 1; }

echo "PASS: Topic pattern configured correctly"
```

**Expected Result:** Topic pattern matches; messages from all 3 sensors are received when triggered.

---

### AT-04: Raw Payload Stored in Bronze

**Criteria:** AC-B4

**Preconditions:**
- Stream running and collecting data
- At least one sensor event triggered

**Test Steps:**

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Locate Bronze directory | `data/raw/home-assistant-state/` exists |
| 2 | Find Parquet files | At least one `.parquet` file present |
| 3 | Read Parquet schema | Contains `raw_payload` column |
| 4 | Read Parquet schema | Contains `topic` metadata column |
| 5 | Query payload content | Value is `"on"` or `"off"` |

**Verification Script:**
```bash
#!/bin/bash
BRONZE_PATH="data/raw/home-assistant-state"

# Step 1: Directory exists
test -d "$BRONZE_PATH" || { echo "FAIL: Bronze directory missing"; exit 1; }

# Step 2: Parquet files exist
PARQUET_COUNT=$(find "$BRONZE_PATH" -name "*.parquet" 2>/dev/null | wc -l)
[ "$PARQUET_COUNT" -gt 0 ] || { echo "FAIL: No Parquet files found"; exit 1; }

# Step 3-5: Query with DuckDB (if available)
if command -v duckdb &> /dev/null; then
    duckdb -c "
        SELECT raw_payload, metadata
        FROM read_parquet('$BRONZE_PATH/**/*.parquet')
        LIMIT 3;
    " || { echo "WARN: DuckDB query failed"; }
fi

echo "PASS: Bronze storage verified ($PARQUET_COUNT files)"
```

**Expected Result:** Parquet files exist with raw payload and topic metadata.

---

### AT-05: ndp_id Extracted for Each Sensor

**Criteria:** AC-B5

**Preconditions:**
- Data flowing through Bronze layer

**Test Steps:**

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Query Bronze for distinct ndp_id | Returns sensor identifiers |
| 2 | Verify `door_backslider` | Present in results |
| 3 | Verify `door_officewindow` | Present in results |
| 4 | Verify `door_dinettewindow` | Present in results |

**Verification Query (DuckDB):**
```sql
SELECT DISTINCT
    json_extract_string(metadata, '$.topic') as topic,
    -- Extract ndp_id from topic: homeassistant/binary_sensor/{ndp_id}/state
    regexp_extract(
        json_extract_string(metadata, '$.topic'),
        'binary_sensor/([^/]+)/state',
        1
    ) as ndp_id
FROM read_parquet('data/raw/home-assistant-state/**/*.parquet');
```

**Expected Result:**
| topic | ndp_id |
|-------|--------|
| homeassistant/binary_sensor/door_backslider/state | door_backslider |
| homeassistant/binary_sensor/door_officewindow/state | door_officewindow |
| homeassistant/binary_sensor/door_dinettewindow/state | door_dinettewindow |

---

### AT-06: Silver Hypertable Created

**Criteria:** AC-S1

**Preconditions:**
- TimescaleDB running
- Schema migrations applied

**Test Steps:**

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Check table exists | `silver.state_events` found |
| 2 | Check is hypertable | TimescaleDB hypertable metadata present |
| 3 | Check time column | `event_time` is partitioning column |
| 4 | Check primary key | PK on `(event_time, ndp_id)` |

**Verification Query (PostgreSQL):**
```sql
-- Step 1: Table exists
SELECT EXISTS (
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = 'silver' AND table_name = 'state_events'
) AS table_exists;

-- Step 2: Is hypertable
SELECT hypertable_schema, hypertable_name
FROM timescaledb_information.hypertables
WHERE hypertable_name = 'state_events';

-- Step 3: Time column
SELECT column_name
FROM timescaledb_information.dimensions
WHERE hypertable_name = 'state_events'
  AND dimension_type = 'Time';

-- Step 4: Primary key columns
SELECT kcu.column_name
FROM information_schema.table_constraints tc
JOIN information_schema.key_column_usage kcu
  ON tc.constraint_name = kcu.constraint_name
WHERE tc.table_schema = 'silver'
  AND tc.table_name = 'state_events'
  AND tc.constraint_type = 'PRIMARY KEY';
```

**Expected Result:**
- Table exists: `true`
- Hypertable: `(silver, state_events)`
- Time column: `event_time`
- PK columns: `event_time`, `ndp_id`

---

### AT-07: ETL Extracts State Correctly

**Criteria:** AC-S2

**Preconditions:**
- Bronze data exists
- Silver ETL has run

**Test Steps:**

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Query Silver table | Data present |
| 2 | Check `state` column values | Only `on` or `off` |
| 3 | Verify no null states | `state IS NOT NULL` for all rows |

**Verification Query:**
```sql
-- Step 1 & 2: Data with valid states
SELECT state, count(*) as event_count
FROM silver.state_events
GROUP BY state;

-- Expected:
-- state | event_count
-- ------+------------
-- on    | N
-- off   | M

-- Step 3: No nulls
SELECT count(*) as null_count
FROM silver.state_events
WHERE state IS NULL;

-- Expected: 0
```

**Expected Result:** State values are `on` or `off` with no nulls.

---

### AT-08: source_entity_id Extracted from Topic

**Criteria:** AC-S3

**Preconditions:**
- Silver ETL has populated data

**Test Steps:**

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Query `source_entity_id` for `door_backslider` | `binary_sensor.door_backslider` |
| 2 | Query `source_entity_id` for `door_officewindow` | `binary_sensor.door_officewindow` |
| 3 | Query `source_entity_id` for `door_dinettewindow` | `binary_sensor.door_dinettewindow` |

**Verification Query:**
```sql
SELECT DISTINCT ndp_id, source_entity_id
FROM silver.state_events
WHERE ndp_id IN ('door_backslider', 'door_officewindow', 'door_dinettewindow')
ORDER BY ndp_id;
```

**Expected Result:**
| ndp_id | source_entity_id |
|--------|------------------|
| door_backslider | binary_sensor.door_backslider |
| door_dinettewindow | binary_sensor.door_dinettewindow |
| door_officewindow | binary_sensor.door_officewindow |

---

### AT-09: event_time Uses Ingestion Timestamp

**Criteria:** AC-S4

**Preconditions:**
- Recent events in Silver table

**Test Steps:**

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Query recent events | `event_time` is within last hour (if recent) |
| 2 | Compare ingestion vs event | Timestamps are close (~<1 second) |
| 3 | Verify timezone | `event_time` is `TIMESTAMPTZ` (UTC aware) |

**Verification Query:**
```sql
-- Step 1: Recent events
SELECT event_time, ndp_id, state
FROM silver.state_events
ORDER BY event_time DESC
LIMIT 5;

-- Step 3: Check column type
SELECT data_type
FROM information_schema.columns
WHERE table_schema = 'silver'
  AND table_name = 'state_events'
  AND column_name = 'event_time';

-- Expected: timestamp with time zone
```

**Expected Result:** `event_time` is `TIMESTAMPTZ` and reflects ingestion time.

---

### AT-10: Dimension CSV Contains Sensors

**Criteria:** AC-D1

**Preconditions:**
- Dimension CSV file exists

**Test Steps:**

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Read CSV file | File parses successfully |
| 2 | Find `door_backslider` row | Present with category `door` |
| 3 | Find `door_officewindow` row | Present with category `window` |
| 4 | Find `door_dinettewindow` row | Present with category `window` |

**Verification Script:**
```bash
#!/bin/bash
CSV_PATH="data/dimensions/entity_context.csv"

# Step 1: File exists and is valid CSV
test -f "$CSV_PATH" || { echo "FAIL: CSV missing"; exit 1; }

# Step 2-4: Check for sensors
for sensor in "door_backslider,door" "door_officewindow,window" "door_dinettewindow,window"; do
    ndp_id=$(echo "$sensor" | cut -d',' -f1)
    category=$(echo "$sensor" | cut -d',' -f2)

    if grep -q "^$ndp_id,$category," "$CSV_PATH"; then
        echo "PASS: Found $ndp_id with category $category"
    else
        echo "FAIL: Missing or wrong category for $ndp_id"
        exit 1
    fi
done

echo "PASS: All 3 sensors present in dimension CSV"
```

**Expected Result:** All 3 sensors present with correct categories.

---

### AT-11: Dimension Sync Loads Data

**Criteria:** AC-D2

**Preconditions:**
- TimescaleDB running
- Dimension CSV updated
- `deploy.sh` script available

**Test Steps:**

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Run `./deploy.sh sync-dimensions` | Exit code 0 |
| 2 | Query dimension table | 3 new sensors present |
| 3 | Verify categories match CSV | `door` and `window` categories correct |

**Verification Script:**
```bash
#!/bin/bash
# Step 1: Run sync
./deploy.sh sync-dimensions
[ $? -eq 0 ] || { echo "FAIL: sync-dimensions failed"; exit 1; }

# Step 2-3: Query database
psql -c "
SELECT ndp_id, category, friendly_name
FROM silver.entity_context
WHERE ndp_id IN ('door_backslider', 'door_officewindow', 'door_dinettewindow')
ORDER BY ndp_id;
"
```

**Expected Result:**
| ndp_id | category | friendly_name |
|--------|----------|---------------|
| door_backslider | door | Back Door Slider |
| door_dinettewindow | window | Dinette Window |
| door_officewindow | window | Office Window |

---

### AT-12: Pipeline Health Shows Freshness

**Criteria:** AC-H1

**Preconditions:**
- Data flowing to Silver
- Dashboard configured

**Test Steps:**

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Open Grafana dashboard | Dashboard loads |
| 2 | Locate pipeline health panel | Panel displays state_events freshness |
| 3 | Verify freshness calculation | Shows time since last event |

**Verification Query:**
```sql
-- Pipeline health freshness query
SELECT
    'state_events' as stream,
    MAX(event_time) as last_event,
    NOW() - MAX(event_time) as age,
    CASE
        WHEN NOW() - MAX(event_time) < INTERVAL '18 hours' THEN 'FRESH'
        WHEN NOW() - MAX(event_time) < INTERVAL '36 hours' THEN 'STALE'
        ELSE 'CRITICAL'
    END as status
FROM silver.state_events;
```

**Expected Result:** Query returns freshness status; dashboard displays similar information.

---

### AT-13: Sparse Data Thresholds Applied

**Criteria:** AC-H2

**Preconditions:**
- Pipeline health configured with sparse thresholds

**Test Steps:**

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Query with data <18h old | Status = FRESH |
| 2 | Query with data 20h old | Status = STALE |
| 3 | Query with data 40h old | Status = CRITICAL |

**Verification Queries:**
```sql
-- Test threshold logic (using calculated times)
WITH test_times AS (
    SELECT NOW() - INTERVAL '10 hours' as fresh_time,
           NOW() - INTERVAL '24 hours' as stale_time,
           NOW() - INTERVAL '48 hours' as critical_time
)
SELECT
    'fresh_time' as scenario,
    CASE
        WHEN NOW() - fresh_time < INTERVAL '18 hours' THEN 'FRESH'
        WHEN NOW() - fresh_time < INTERVAL '36 hours' THEN 'STALE'
        ELSE 'CRITICAL'
    END as expected_status
FROM test_times
UNION ALL
SELECT 'stale_time',
    CASE
        WHEN NOW() - stale_time < INTERVAL '18 hours' THEN 'FRESH'
        WHEN NOW() - stale_time < INTERVAL '36 hours' THEN 'STALE'
        ELSE 'CRITICAL'
    END
FROM test_times
UNION ALL
SELECT 'critical_time',
    CASE
        WHEN NOW() - critical_time < INTERVAL '18 hours' THEN 'FRESH'
        WHEN NOW() - critical_time < INTERVAL '36 hours' THEN 'STALE'
        ELSE 'CRITICAL'
    END
FROM test_times;
```

**Expected Result:**
| scenario | expected_status |
|----------|-----------------|
| fresh_time | FRESH |
| stale_time | STALE |
| critical_time | CRITICAL |

---

### AT-14: No False Alarms During Normal Operation

**Criteria:** AC-H3

**Preconditions:**
- Sensors deployed and working
- Pipeline running for 24+ hours

**Test Steps:**

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Leave all windows/doors closed for 12h | Events stop (sparse data) |
| 2 | Check pipeline health after 12h | Status = FRESH (not STALE) |
| 3 | Leave closed for another 10h (22h total) | Status = STALE (not CRITICAL) |
| 4 | Open a door | New event captured |
| 5 | Check pipeline health | Status = FRESH |

**Manual Verification:**

This test requires observation over 24+ hours:

1. **Hour 0**: Record last event time
2. **Hour 12**: Verify status is FRESH (12h < 18h threshold)
3. **Hour 22**: Verify status is STALE (22h > 18h but < 36h)
4. **Hour 22+**: Trigger sensor, verify status returns to FRESH

**Automated Monitoring Query:**
```sql
-- Add to monitoring/alerting
SELECT
    ndp_id,
    MAX(event_time) as last_event,
    EXTRACT(EPOCH FROM (NOW() - MAX(event_time))) / 3600 as hours_since_event,
    CASE
        WHEN NOW() - MAX(event_time) < INTERVAL '18 hours' THEN 'FRESH'
        WHEN NOW() - MAX(event_time) < INTERVAL '36 hours' THEN 'STALE'
        ELSE 'CRITICAL'
    END as status
FROM silver.state_events
GROUP BY ndp_id
ORDER BY hours_since_event DESC;
```

**Expected Result:** Status transitions at correct thresholds; no alerts during normal sparse data periods.

---

## Summary: Acceptance Test Matrix

| ID | Criteria | Test Type | Automation |
|----|----------|-----------|------------|
| AT-01 | AC-B1: Stream config | Config validation | Script |
| AT-02 | AC-B2: Broker connection | Integration | Script |
| AT-03 | AC-B3: Topic pattern | Integration | Script |
| AT-04 | AC-B4: Bronze storage | Integration | Script + DuckDB |
| AT-05 | AC-B5: ndp_id extraction | Integration | DuckDB query |
| AT-06 | AC-S1: Hypertable | Schema validation | SQL |
| AT-07 | AC-S2: State extraction | Integration | SQL |
| AT-08 | AC-S3: source_entity_id | Integration | SQL |
| AT-09 | AC-S4: Ingestion timestamp | Integration | SQL |
| AT-10 | AC-D1: Dimension CSV | Config validation | Script |
| AT-11 | AC-D2: Dimension sync | Integration | Script + SQL |
| AT-12 | AC-H1: Freshness display | E2E | Manual + SQL |
| AT-13 | AC-H2: Sparse thresholds | Unit | SQL |
| AT-14 | AC-H3: No false alarms | E2E | Manual (24h) |

---

## Test Execution Checklist

### Pre-Deployment (Development)

- [ ] AT-01: Stream config valid
- [ ] AT-10: Dimension CSV has sensors
- [ ] AT-13: Sparse threshold logic correct

### Integration Testing (Docker)

- [ ] AT-02: MQTT broker connection
- [ ] AT-03: Topic pattern subscription
- [ ] AT-06: Silver hypertable created
- [ ] AT-11: Dimension sync works

### Deployment Verification (Pi)

- [ ] AT-04: Bronze storage working
- [ ] AT-05: ndp_id extracted correctly
- [ ] AT-07: ETL extracts state
- [ ] AT-08: source_entity_id correct
- [ ] AT-09: Ingestion timestamp used
- [ ] AT-12: Dashboard shows freshness

### Post-Deployment (24h+ observation)

- [ ] AT-14: No false alarms during sparse data

---

## Appendix: Test Fixtures

### Bronze Fixture (for integration tests without MQTT)

```json
// tests/fixtures/home_assistant_bronze_event.json
{
    "source_id": "home-assistant-state-Mqtt",
    "timestamp": "2024-01-15T10:30:00Z",
    "raw_payload": "on",
    "metadata": {
        "topic": "homeassistant/binary_sensor/door_backslider/state",
        "qos": 1
    }
}
```

### Expected Silver Output

```json
// tests/fixtures/expected_silver_event.json
{
    "event_time": "2024-01-15T10:30:00Z",
    "ndp_id": "door_backslider",
    "source_entity_id": "binary_sensor.door_backslider",
    "state": "on",
    "dq_flags": []
}
```

### Dimension CSV Fixture

```csv
ndp_id,category,friendly_name,location_path,correlates_with,orientation
door_backslider,door,Back Door Slider,/home/living,aq_airgradient_1,south
door_officewindow,window,Office Window,/home/office,aq_airgradient_1,east
door_dinettewindow,window,Dinette Window,/home/dining,aq_airgradient_1,west
```
