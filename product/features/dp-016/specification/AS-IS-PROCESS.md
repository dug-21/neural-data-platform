# AS-IS Process: Adding a New Stream to NDP

**Document Type:** Current State Documentation
**Feature:** dp-016 Configuration Architecture Review
**Last Updated:** 2026-02-01

---

## Overview

This document captures the **current process** for adding a new data stream to the Neural Data Platform. It is intentionally detailed to expose pain points and inform the architecture review.

**Estimated Time:** 2-4 hours (longer if issues arise)
**Manual Steps:** 8+
**Failure Points:** 12+

---

## Prerequisites

Before starting, ensure:

1. [ ] Access to the Pi (SSH) or dev environment
2. [ ] Understanding of the data source (MQTT topic, HTTP endpoint, payload format)
3. [ ] TimescaleDB credentials
4. [ ] Git access to push changes

---

## Step 1: Create Stream Configuration YAML

**Location:** `config/base/streams/{stream-id}/config.yaml`

### 1.1 Create Directory

```bash
mkdir -p config/base/streams/my-new-stream
```

### 1.2 Create config.yaml

```yaml
stream_id: "my-new-stream"
description: "Description of data source"
version: "1.0.0"
enabled: true
retention_days: 90
partitioning_strategy: daily

# Bronze layer field definitions
fields:
  - name: my_field
    type: float
    nullable: false
    unit: units
    range: [0.0, 100.0]

# Data source configuration
sources:
  - type: mqtt  # or http_poll
    enabled: true
    ndp_id: "my-source-001"
    broker_url: "mosquitto"
    port: 1883
    topic_pattern: "my/topic/+"
    parser:
      parser_type: flat_json
      location_id_field: id

# Optional: Storage overrides
storage:
  batch_size: 50
  batch_timeout_secs: 30
```

### 1.3 If Silver ETL Needed, Add silver_etl Section

```yaml
# Silver layer ETL configuration
silver_etl:
  enabled: true
  target_table: silver.my_table_name
  description: "What this Silver table contains"
  grain: "One row per..."

  timestamp:
    source_field: timestamp
    target_field: observation_time
    transform: microseconds_to_timestamp

  identity_fields:
    - source: ndp_id
      target: ndp_id

  field_mappings:
    - source_path: raw_payload.my_field
      target_column: my_field
      type: double_precision
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 100.0
          action: flag

  deduplication:
    enabled: true
    key_columns: [observation_time, ndp_id]
    strategy: upsert
```

### Known Issues at This Step

| Issue | Symptom | Workaround |
|-------|---------|------------|
| Typo in `silver_etl` (e.g., `silver_elt`) | Section silently ignored | Double-check spelling |
| Invalid `source_path` | NULL values in Silver | No validation - must test manually |
| Unknown fields | Captured in `extra` HashMap | No warning - review carefully |

---

## Step 2: Create Silver Table DDL (MANUAL)

**Location:** `deploy/timescaledb/init/0XX_my_stream_schema.sql` or `deploy/timescaledb/migrations/`

### 2.1 Write CREATE TABLE Statement

```sql
-- deploy/timescaledb/migrations/XXX_my_stream_schema.sql

CREATE TABLE IF NOT EXISTS silver.my_table_name (
    -- Audit columns
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    observation_time    TIMESTAMPTZ NOT NULL,
    source_stream       TEXT NOT NULL DEFAULT 'my-new-stream',
    ndp_id              TEXT NOT NULL,

    -- Data columns (match silver_etl.field_mappings)
    my_field            DOUBLE PRECISION,

    -- DQ transparency
    dq_flags            TEXT[],

    PRIMARY KEY (observation_time, ndp_id)
);

-- Convert to hypertable
SELECT create_hypertable('silver.my_table_name',
    'observation_time',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

-- Compression policy
ALTER TABLE silver.my_table_name SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'ndp_id',
    timescaledb.compress_orderby = 'observation_time DESC'
);
SELECT add_compression_policy('silver.my_table_name', INTERVAL '7 days');

-- Retention policy
SELECT add_retention_policy('silver.my_table_name', INTERVAL '90 days');

-- Grants
GRANT SELECT ON silver.my_table_name TO grafana_reader;
GRANT SELECT, INSERT, UPDATE, DELETE ON silver.my_table_name TO ndp_app;
```

### Known Issues at This Step

| Issue | Symptom | Workaround |
|-------|---------|------------|
| Column type mismatch with config | Runtime INSERT errors | Manually verify types match |
| Forgot to run migration | Silent failure - no data in Silver | Check if table exists |
| Missing grants | Permission denied at runtime | Add GRANT statements |

---

## Step 3: Add Dimension Entries (If Applicable)

**Location:** `data/dimensions/entity_context.csv`

### 3.1 Edit CSV File

Add rows for each entity (device, sensor, etc.):

```csv
ndp_id,category,friendly_name,location_path,correlates_with,orientation
my-source-001,sensor,My Sensor,/home/room,aq_airgradient_1,
```

### 3.2 Create/Update Dimension Config

**Location:** `config/base/dimensions/entity_context.yaml`

Ensure config exists with correct schema fields.

### Known Issues at This Step

| Issue | Symptom | Workaround |
|-------|---------|------------|
| CSV column mismatch | COPY command fails | Match CSV headers to YAML schema |
| Missing ndp_id reference | No JOIN enrichment | Manually verify ndp_id values |

---

## Step 4: Update Data Dictionary (Entity Schemas)

### 4.1 Add entity_schemas to config.yaml

```yaml
entity_schemas:
  - schema_name: my_reading
    description: "Schema for my readings"
    device_class: sensor
    attributes:
      - name: my_field
        type: Float
        unit: units
        description: "My field description"
```

### Known Issues at This Step

| Issue | Symptom | Workaround |
|-------|---------|------------|
| Schema doesn't match Silver table | MCP tools show wrong metadata | Manually keep in sync |

---

## Step 5: Commit and Push Changes

```bash
git add config/base/streams/my-new-stream/
git add deploy/timescaledb/migrations/
git add data/dimensions/entity_context.csv
git commit -m "Add my-new-stream configuration"
git push origin main
```

---

## Step 6: Deploy to Pi

### 6.1 SSH to Pi and Pull Changes

```bash
ssh pi@raspberry-pi
cd /opt/neural-data-platform
git pull origin main
```

### 6.2 Run Silver Migration

```bash
./deploy/pi/deploy.sh silver-migrate
```

**Expected Output:**
```
[DEPLOY] Running Silver Layer migrations...
Applying migration: XXX_my_stream_schema.sql
[DEPLOY] Silver migrations complete
```

### 6.3 Sync Configuration to etcd

```bash
./deploy/pi/deploy.sh sync
```

**Expected Output:**
```
[DEPLOY] Syncing configuration to etcd...
Syncing stream: my-new-stream to /streams/my-new-stream
Config sync complete!
```

### 6.4 Sync Data Dictionary

```bash
./deploy/pi/deploy.sh sync-dictionary
```

### 6.5 Sync Dimensions

```bash
./deploy/pi/deploy.sh sync-dimensions
```

### 6.6 Restart Application

```bash
docker restart air-quality-app
```

### Known Issues at This Step

| Issue | Symptom | Workaround |
|-------|---------|------------|
| etcd sync validation error | Stream not in etcd, no Silver ETL | Check logs for validation errors |
| Migration already applied | Safe - uses IF NOT EXISTS | None needed |
| App doesn't pick up config | Stale cache | Restart app |

---

## Step 7: Verify Bronze Layer

### 7.1 Check Logs

```bash
docker logs air-quality-app 2>&1 | grep my-new-stream
```

**Look for:**
- `Starting source: my-new-stream-mqtt`
- No ERROR or WARN messages

### 7.2 Check etcd

```bash
docker exec etcd etcdctl get /streams/my-new-stream/config
```

### 7.3 Check Parquet Files

```bash
ls -la /opt/neural-data-platform/data/raw/my-new-stream/
```

---

## Step 8: Verify Silver Layer

### 8.1 Check Silver ETL Logs

```bash
docker logs air-quality-app 2>&1 | grep "SilverSubscriber"
```

**Look for:**
- `Created SilverSubscriber for stream: my-new-stream`
- No "Failed to create" messages

### 8.2 Check Table Has Data

```bash
docker exec timescaledb psql -U postgres -d ndp -c \
  "SELECT COUNT(*), MAX(observation_time) FROM silver.my_table_name;"
```

### 8.3 Check DQ Flags

```bash
docker exec timescaledb psql -U postgres -d ndp -c \
  "SELECT dq_flags, COUNT(*) FROM silver.my_table_name GROUP BY dq_flags;"
```

---

## Troubleshooting Guide

### Bronze Not Working

| Symptom | Likely Cause | Fix |
|---------|--------------|-----|
| No source in logs | Config not synced to etcd | Run `./deploy.sh sync`, restart app |
| Source created but no data | Wrong topic/endpoint | Check source config |
| Parquet files empty | Parser error | Check parser_type matches payload |

### Silver Not Working

| Symptom | Likely Cause | Fix |
|---------|--------------|-----|
| No SilverSubscriber created | Stream not in etcd list | Check `etcdctl get --prefix /streams` |
| SilverSubscriber created, no data | Table doesn't exist | Run `./deploy.sh silver-migrate` |
| NULL values in columns | Wrong `source_path` | Check field_mappings match payload |
| INSERT errors | Column type mismatch | Compare DDL to config types |

### Data Dictionary Issues

| Symptom | Likely Cause | Fix |
|---------|--------------|-----|
| MCP tools show wrong metadata | Dictionary not synced | Run `./deploy.sh sync-dictionary` |
| Missing columns in lineage | field_mappings incomplete | Add missing mappings |

---

## Complete Command Sequence (Summary)

```bash
# 1. Create config files (on dev machine)
mkdir -p config/base/streams/my-new-stream
# Edit config.yaml, create DDL, update dimensions

# 2. Commit and push
git add . && git commit -m "Add my-new-stream" && git push

# 3. On Pi
ssh pi@raspberry-pi
cd /opt/neural-data-platform
git pull origin main

# 4. Deploy sequence (ORDER MATTERS)
./deploy/pi/deploy.sh silver-migrate      # Create tables first
./deploy/pi/deploy.sh sync                # Sync config to etcd
./deploy/pi/deploy.sh sync-dictionary     # Update data dictionary
./deploy/pi/deploy.sh sync-dimensions     # Load dimensions
docker restart air-quality-app            # Pick up new config

# 5. Verify
docker logs air-quality-app | tail -50
docker exec timescaledb psql -U postgres -d ndp -c "SELECT COUNT(*) FROM silver.my_table_name;"
```

---

## Time Breakdown

| Step | Estimated Time | Failure Risk |
|------|----------------|--------------|
| Create YAML config | 15-30 min | Medium (no validation) |
| Create Silver DDL | 15-30 min | Medium (manual sync) |
| Add dimension entries | 5-10 min | Low |
| Update data dictionary | 5-10 min | Low |
| Commit and push | 2 min | Low |
| SSH and deploy | 10-15 min | Medium |
| Verify and debug | 15-60 min | High |
| **Total** | **1-3 hours** | |

---

## What This Process Exposes

1. **8+ Manual Steps** - High friction for common operation
2. **No Validation Until Runtime** - Errors discovered late
3. **Multiple SSH Sessions** - No single deployment command
4. **Silent Failures** - etcd sync can fail without stopping app
5. **Manual DDL** - Config describes schema but doesn't create it
6. **Order Dependency** - Steps must be done in specific order
7. **No Rollback** - If something fails, manual cleanup required

---

*This document serves as input to the dp-016 architecture review. The goal is to reduce this to 1-2 steps with automated validation.*
