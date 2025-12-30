# DP-002: Acceptance Criteria

**Document Type**: SPARC Specification
**Version**: 1.0.0
**Last Updated**: 2025-12-30
**Status**: Draft

---

## Overview

This document defines testable acceptance criteria for DP-002: Online Data Dictionary & HomeAssistant Stream Preparation. Each criterion is:
- Specific and measurable
- Independently verifiable
- Linked to requirements

---

## Acceptance Test Index

| ID | Scope Item | Description |
|----|------------|-------------|
| AT-1.x | DuckDB Removal | Container and volume removal verification |
| AT-2.x | TimescaleDB | Service deployment and configuration |
| AT-3.x | Entity Schemas | Schema additions to all streams |
| AT-4.x | Data Dictionary | Database tables and views |
| AT-5.x | HomeAssistant Stream | New stream configuration |
| AT-6.x | Deploy Script | sync-dictionary command |
| AT-7.x | DQ Dashboard | Grafana dashboard panels |
| AT-8.x | Documentation | Procedure updates |

---

## AT-1: DuckDB Container Removal

### AT-1.1: DuckDB Service Removed

**Requirement**: REQ-1.1

**Test Procedure**:
1. Open `deploy/pi/docker-compose.yml`
2. Search for `duckdb` service definition

**Expected Result**:
- No service with name `duckdb` exists
- No container named `duckdb` in services section

**Verification Command**:
```bash
grep -c "^\s*duckdb:" deploy/pi/docker-compose.yml
# Expected: 0
```

---

### AT-1.2: DuckDB Volume Removed

**Requirement**: REQ-1.2

**Test Procedure**:
1. Open `deploy/pi/docker-compose.yml`
2. Check volumes section for `duckdb-data`

**Expected Result**:
- No volume named `duckdb-data` in volumes section
- No references to `duckdb-data` in service volume mounts

**Verification Command**:
```bash
grep -c "duckdb-data" deploy/pi/docker-compose.yml
# Expected: 0
```

---

### AT-1.3: Grafana DuckDB Plugin Independence

**Requirement**: REQ-1.3

**Test Procedure**:
1. Start stack with `docker compose up -d`
2. Wait for Grafana to be healthy
3. Open Grafana at http://localhost:3000
4. Navigate to Explore
5. Select DuckDB datasource
6. Run query: `SELECT COUNT(*) FROM read_parquet('/data/air-quality/**/*.parquet')`

**Expected Result**:
- Query executes successfully
- Returns count > 0 (if data exists)
- No errors related to missing DuckDB container

**Preconditions**:
- Bronze layer Parquet files exist in `/data/air-quality/`

---

### AT-1.4: Grafana Volume Mounts Updated

**Requirement**: REQ-1.4

**Test Procedure**:
1. Open `deploy/pi/docker-compose.yml`
2. Inspect grafana service volumes

**Expected Result**:
- Volume `air-quality-data:/data:ro` is present
- No volume mount for `duckdb-data` (or if present, only for plugin database)

**Verification Command**:
```bash
grep -A20 "^\s*grafana:" deploy/pi/docker-compose.yml | grep "air-quality-data"
# Expected: Shows volume mount
```

---

## AT-2: TimescaleDB Instantiation

### AT-2.1: TimescaleDB Service Added

**Requirement**: REQ-2.1

**Test Procedure**:
1. Open `deploy/pi/docker-compose.yml`
2. Verify timescaledb service exists

**Expected Result**:
- Service named `timescaledb` exists
- Image is `timescale/timescaledb:latest-pg15` or compatible ARM64 image
- Container name is set (e.g., `timescaledb`)

**Verification Command**:
```bash
grep -A5 "timescaledb:" deploy/pi/docker-compose.yml | grep "image:"
# Expected: Shows timescale/timescaledb image
```

---

### AT-2.2: Resource Limits Configured

**Requirement**: REQ-2.2

**Test Procedure**:
1. Open `deploy/pi/docker-compose.yml`
2. Check timescaledb service deploy.resources.limits

**Expected Result**:
- Memory limit set to 512M or less
- Limits section present under deploy.resources

**Verification Command**:
```bash
grep -A15 "timescaledb:" deploy/pi/docker-compose.yml | grep -A3 "limits:"
# Expected: Shows memory: 512M (or less)
```

---

### AT-2.3: Data Dictionary Schema Created

**Requirement**: REQ-2.3

**Test Procedure**:
1. Start stack with `docker compose up -d`
2. Connect to TimescaleDB
3. Query for required tables

**Expected Result**:
- Table `streams` exists
- Table `entity_schemas` exists
- Table `data_dictionary` exists

**Verification Commands**:
```bash
docker exec timescaledb psql -U postgres -d ndp -c "\dt"
# Expected: Lists streams, entity_schemas, data_dictionary

docker exec timescaledb psql -U postgres -d ndp -c "SELECT COUNT(*) FROM streams;"
# Expected: Returns 0 or more (no error)
```

---

### AT-2.4: Persistence Volume Created

**Requirement**: REQ-2.4

**Test Procedure**:
1. Open `deploy/pi/docker-compose.yml`
2. Check volumes section for timescaledb-data

**Expected Result**:
- Volume `timescaledb-data` defined in volumes section
- TimescaleDB service mounts this volume

**Verification Command**:
```bash
grep "timescaledb-data" deploy/pi/docker-compose.yml
# Expected: At least 2 matches (definition and mount)
```

---

### AT-2.5: PostgreSQL Port Exposed

**Requirement**: REQ-2.5

**Test Procedure**:
1. Open `deploy/pi/docker-compose.yml`
2. Check timescaledb service ports

**Expected Result**:
- Port 5432 exposed (5432:5432 or similar)

**Verification Command**:
```bash
grep -A10 "timescaledb:" deploy/pi/docker-compose.yml | grep "5432"
# Expected: Shows port mapping
```

---

### AT-2.6: Health Check Configured

**Requirement**: REQ-2.6

**Test Procedure**:
1. Open `deploy/pi/docker-compose.yml`
2. Check timescaledb service healthcheck

**Expected Result**:
- Healthcheck configured with pg_isready
- Interval, timeout, retries specified

**Verification Command**:
```bash
grep -A8 "healthcheck:" deploy/pi/docker-compose.yml | grep "pg_isready"
# Expected: Shows pg_isready command
```

---

## AT-3: Entity Schema Addition

### AT-3.1: Entity Schema YAML Format Defined

**Requirement**: REQ-3.1

**Test Procedure**:
1. Review ENTITY_SCHEMA_FORMAT.md specification
2. Validate against JSON Schema (if created)

**Expected Result**:
- Format specification document exists
- Defines required fields: schema_name, attributes
- Defines attribute fields: name, type, unit, description, nullable
- Includes examples for all supported types

**Verification**: Manual review of specification document

---

### AT-3.2: air-quality Entity Schema Added

**Requirement**: REQ-3.2

**Test Procedure**:
1. Open `config/base/streams/air-quality/config.yaml`
2. Verify entity_schemas section exists
3. Validate schema contains required attributes

**Expected Result**:
- entity_schemas section present
- Schema named "airgradient" defined
- Attributes include: pm25, pm10, co2, temperature, humidity, tvoc, nox
- Each attribute has type, unit, description

**Verification Command**:
```bash
grep -c "entity_schemas:" config/base/streams/air-quality/config.yaml
# Expected: 1
grep "schema_name: airgradient" config/base/streams/air-quality/config.yaml
# Expected: Match found
```

---

### AT-3.3: outdoor-weather Entity Schema Added

**Requirement**: REQ-3.3

**Test Procedure**:
1. Open `config/base/streams/outdoor-weather/config.yaml`
2. Verify entity_schemas section with nws-weather schema

**Expected Result**:
- Schema named "nws-weather" defined
- Attributes include: temperature, feels_like, pressure, humidity, wind_speed, etc.

**Verification Command**:
```bash
grep "schema_name: nws-weather" config/base/streams/outdoor-weather/config.yaml
# Expected: Match found
```

---

### AT-3.4: outdoor-air-quality Entity Schema Added

**Requirement**: REQ-3.4

**Test Procedure**:
1. Open `config/base/streams/outdoor-air-quality/config.yaml`
2. Verify entity_schemas section with airnow schema

**Expected Result**:
- Schema named "airnow" defined
- Attributes include: aqi, co, no, no2, o3, so2, pm2_5, pm10, nh3

**Verification Command**:
```bash
grep "schema_name: airnow" config/base/streams/outdoor-air-quality/config.yaml
# Expected: Match found
```

---

### AT-3.5: nws-observations Entity Schema Added

**Requirement**: REQ-3.5

**Test Procedure**:
1. Open `config/base/streams/nws-observations/config.yaml`
2. Verify entity_schemas section

**Expected Result**:
- Schema named "nws-observations" defined
- Attributes include all 15+ observation fields

**Verification Command**:
```bash
grep "schema_name: nws-observations" config/base/streams/nws-observations/config.yaml
# Expected: Match found
```

---

### AT-3.6: nws-forecast-hourly Entity Schema Added

**Requirement**: REQ-3.6

**Test Procedure**:
1. Open `config/base/streams/nws-forecast-hourly/config.yaml`
2. Verify entity_schemas section

**Expected Result**:
- Schema named "nws-hourly" defined
- Attributes include: temperature, dewpoint, relative_humidity, etc.

**Verification Command**:
```bash
grep "schema_name: nws-hourly" config/base/streams/nws-forecast-hourly/config.yaml
# Expected: Match found
```

---

### AT-3.7: nws-gridpoints-forecast Entity Schema Added

**Requirement**: REQ-3.7

**Test Procedure**:
1. Open `config/base/streams/nws-gridpoints-forecast/config.yaml`
2. Verify entity_schemas section

**Expected Result**:
- Schema named "nws-gridpoints" defined
- Attributes include all 40+ gridpoint fields

**Verification Command**:
```bash
grep "schema_name: nws-gridpoints" config/base/streams/nws-gridpoints-forecast/config.yaml
# Expected: Match found
```

---

### AT-3.8: Existing Fields Section Preserved

**Requirement**: REQ-3.8

**Test Procedure**:
1. For each stream config, compare `fields` section before and after changes
2. Verify no fields were modified or removed

**Expected Result**:
- `fields` section unchanged in all 6 stream configs
- Only `entity_schemas` section added

**Verification Command**:
```bash
# Git diff should show only additions, no modifications to fields
git diff --stat config/base/streams/*/config.yaml | grep "+"
# Expected: Only addition lines, no deletion lines for fields
```

---

## AT-4: Data Dictionary Tables

### AT-4.1: Streams Table Created

**Requirement**: REQ-4.1

**Test Procedure**:
1. Connect to TimescaleDB
2. Describe streams table

**Expected Result**:
- Table exists with columns: stream_id, description, version, enabled, retention_days, created_at, updated_at
- stream_id is PRIMARY KEY

**Verification Command**:
```bash
docker exec timescaledb psql -U postgres -d ndp -c "\d streams"
# Expected: Shows table structure with expected columns
```

---

### AT-4.2: Entity Schemas Table Created

**Requirement**: REQ-4.2

**Test Procedure**:
1. Connect to TimescaleDB
2. Describe entity_schemas table

**Expected Result**:
- Table exists with columns: id, stream_id, schema_name, description, device_class, created_at, updated_at
- Foreign key to streams(stream_id)
- Unique constraint on (stream_id, schema_name)

**Verification Command**:
```bash
docker exec timescaledb psql -U postgres -d ndp -c "\d entity_schemas"
# Expected: Shows table structure with FK constraint
```

---

### AT-4.3: Data Dictionary Table Created

**Requirement**: REQ-4.3

**Test Procedure**:
1. Connect to TimescaleDB
2. Describe data_dictionary table

**Expected Result**:
- Table exists with columns: id, stream_id, schema_id, schema_name, attribute_name, attribute_type, unit, description, nullable, created_at, updated_at
- Foreign keys to streams and entity_schemas
- Unique constraint on (stream_id, schema_name, attribute_name)

**Verification Command**:
```bash
docker exec timescaledb psql -U postgres -d ndp -c "\d data_dictionary"
# Expected: Shows table structure with constraints
```

---

### AT-4.4: Unified Dictionary View Created

**Requirement**: REQ-4.4

**Test Procedure**:
1. Connect to TimescaleDB
2. Query v_data_dictionary view

**Expected Result**:
- View exists
- Returns columns: stream_id, schema_name, schema_description, device_class, attribute_name, attribute_type, unit, attribute_description, nullable

**Verification Command**:
```bash
docker exec timescaledb psql -U postgres -d ndp -c "SELECT * FROM v_data_dictionary LIMIT 1;"
# Expected: Returns row (after sync) or empty result (before sync), no error
```

---

### AT-4.5: Pattern Matching Support

**Requirement**: REQ-4.5

**Test Procedure**:
1. Insert a schema with pattern name (e.g., `sensor.airgradient_*`)
2. Query to match actual entity ID

**Expected Result**:
- Pattern stored in schema_name
- Query can match `sensor.airgradient_abc123` to pattern

**Verification Query**:
```sql
SELECT * FROM entity_schemas
WHERE 'sensor.airgradient_abc123' LIKE REPLACE(schema_name, '*', '%');
-- Expected: Returns the matching schema
```

---

## AT-5: HomeAssistant Stream Configuration

### AT-5.1: HomeAssistant Stream Directory Created

**Requirement**: REQ-5.1

**Test Procedure**:
1. Check for config file existence

**Expected Result**:
- File exists: `config/base/streams/homeassistant/config.yaml`

**Verification Command**:
```bash
test -f config/base/streams/homeassistant/config.yaml && echo "EXISTS" || echo "MISSING"
# Expected: EXISTS
```

---

### AT-5.2: Generic Bronze Schema Defined

**Requirement**: REQ-5.2

**Test Procedure**:
1. Open HomeAssistant config.yaml
2. Verify fields section

**Expected Result**:
- Fields include: entity_id, state, last_changed, last_updated, attributes

**Verification Command**:
```bash
grep -A20 "^fields:" config/base/streams/homeassistant/config.yaml
# Expected: Shows entity_id, state, last_changed, last_updated, attributes
```

---

### AT-5.3: AirGradient Entity Schema Created

**Requirement**: REQ-5.3

**Test Procedure**:
1. Open HomeAssistant config.yaml
2. Verify entity_schemas section

**Expected Result**:
- Schema named `sensor.airgradient_*` exists
- device_class is `air_quality`
- Attributes include: pm02, pm10, atmp, rhum, rco2, tvoc

**Verification Command**:
```bash
grep "sensor.airgradient_" config/base/streams/homeassistant/config.yaml
# Expected: Match found
```

---

### AT-5.4: MQTT Source Configured

**Requirement**: REQ-5.4

**Test Procedure**:
1. Open HomeAssistant config.yaml
2. Verify sources section

**Expected Result**:
- Source type is `mqtt`
- Topic pattern matches HomeAssistant Statestream format

**Verification Command**:
```bash
grep "homeassistant/+/+/state" config/base/streams/homeassistant/config.yaml
# Expected: Match found (or similar pattern)
```

---

### AT-5.5: Stream Metadata Complete

**Requirement**: REQ-5.5

**Test Procedure**:
1. Open HomeAssistant config.yaml
2. Verify stream metadata

**Expected Result**:
- stream_id: homeassistant
- description present
- retention_days: 365
- enabled: false

**Verification Command**:
```bash
grep "enabled: false" config/base/streams/homeassistant/config.yaml
# Expected: Match found
```

---

## AT-6: Deploy Script Extension

### AT-6.1: sync-dictionary Command Available

**Requirement**: REQ-6.1

**Test Procedure**:
1. Run `./deploy.sh` without arguments
2. Check for sync-dictionary in help output

**Expected Result**:
- sync-dictionary listed as available command

**Verification Command**:
```bash
./deploy/pi/deploy.sh 2>&1 | grep "sync-dictionary"
# Expected: Match found
```

---

### AT-6.2: Upsert Logic Works

**Requirement**: REQ-6.2

**Test Procedure**:
1. Run sync-dictionary
2. Verify data inserted
3. Modify a schema in etcd
4. Run sync-dictionary again
5. Verify update applied
6. Remove a schema from etcd
7. Run sync-dictionary again
8. Verify deletion applied

**Expected Result**:
- Initial run: X schemas added
- Update run: X schemas updated
- Deletion run: X schemas deleted

**Verification**: Manual test with logging output verification

---

### AT-6.3: Reads from etcd

**Requirement**: REQ-6.3

**Test Procedure**:
1. Check sync script source code
2. Verify it reads from etcd, not YAML files

**Expected Result**:
- Script uses etcdctl or etcd client library
- Reads from /streams/ prefix

**Verification**: Code review

---

### AT-6.4: Validation Before Sync

**Requirement**: REQ-6.4

**Test Procedure**:
1. Insert invalid entity_schema into etcd (missing required field)
2. Run sync-dictionary

**Expected Result**:
- Invalid schema logged as warning
- Valid schemas still synced
- No database corruption

**Verification Command**:
```bash
./deploy/pi/deploy.sh sync-dictionary 2>&1 | grep -i "warning\|invalid"
# Expected: Shows validation warning for invalid schema
```

---

### AT-6.5: Idempotent Operation

**Requirement**: REQ-6.5

**Test Procedure**:
1. Run sync-dictionary
2. Record database state
3. Run sync-dictionary again
4. Compare database state

**Expected Result**:
- Second run reports 0 changes
- Database timestamps unchanged

**Verification Command**:
```bash
./deploy/pi/deploy.sh sync-dictionary 2>&1 | tail -1
# First run: "Synced: X added, Y updated, 0 deleted"
# Second run: "Synced: 0 added, 0 updated, 0 deleted"
```

---

### AT-6.6: Logging and Reporting

**Requirement**: REQ-6.6

**Test Procedure**:
1. Run sync-dictionary
2. Review output

**Expected Result**:
- Shows number of streams processed
- Shows schemas added/updated/deleted
- Shows validation errors (if any)

**Verification Command**:
```bash
./deploy/pi/deploy.sh sync-dictionary 2>&1
# Expected: Structured output with counts
```

---

## AT-7: Data Quality Dashboard

### AT-7.1: Dashboard JSON Created

**Requirement**: REQ-7.1

**Test Procedure**:
1. Check for dashboard file

**Expected Result**:
- File exists: `config/grafana/dashboards/homeassistant-data-quality.json`
- Valid JSON format

**Verification Command**:
```bash
test -f config/grafana/dashboards/homeassistant-data-quality.json && echo "EXISTS" || echo "MISSING"
jq . config/grafana/dashboards/homeassistant-data-quality.json > /dev/null && echo "VALID JSON"
# Expected: EXISTS, VALID JSON
```

---

### AT-7.2: Schema Coverage Panel

**Requirement**: REQ-7.2

**Test Procedure**:
1. Open dashboard in Grafana
2. Locate schema coverage panel

**Expected Result**:
- Panel shows total defined schemas
- Panel shows total streams
- Panel shows coverage percentage

**Verification**: Manual visual inspection

---

### AT-7.3: Unknown Entities Panel

**Requirement**: REQ-7.3

**Test Procedure**:
1. Open dashboard in Grafana
2. Locate unknown entities panel

**Expected Result**:
- Panel lists entities not matching schemas
- Empty when all entities have schemas

**Verification**: Manual visual inspection

---

### AT-7.4: Incomplete Schemas Panel

**Requirement**: REQ-7.4

**Test Procedure**:
1. Open dashboard in Grafana
2. Locate incomplete schemas panel

**Expected Result**:
- Panel shows missing/extra attributes
- Grouped by schema

**Verification**: Manual visual inspection

---

### AT-7.5: Attribute Heatmap Panel

**Requirement**: REQ-7.5

**Test Procedure**:
1. Open dashboard in Grafana
2. Locate attribute heatmap panel

**Expected Result**:
- Shows attribute presence by device_class
- Visual representation of completeness

**Verification**: Manual visual inspection

---

### AT-7.6: Dynamic Dashboard Queries

**Requirement**: REQ-7.6

**Test Procedure**:
1. Add new schema via sync-dictionary
2. Refresh dashboard
3. Verify new schema appears

**Expected Result**:
- Dashboard updates automatically
- No JSON edits required

**Verification**: Manual test after schema change

---

## AT-8: Documentation Updates

### AT-8.1: HOW_TO_ADD_NEW_STREAM.md Updated

**Requirement**: REQ-8.1

**Test Procedure**:
1. Open `docs/procedures/HOW_TO_ADD_NEW_STREAM.md`
2. Search for entity_schemas section

**Expected Result**:
- Section on entity_schemas exists
- Explains purpose and relationship to fields
- Includes YAML examples

**Verification Command**:
```bash
grep -c "entity_schemas" docs/procedures/HOW_TO_ADD_NEW_STREAM.md
# Expected: Multiple matches (> 3)
```

---

### AT-8.2: HOW_TO_ADD_NEW_SOURCE.md Updated

**Requirement**: REQ-8.2

**Test Procedure**:
1. Open `docs/procedures/HOW_TO_ADD_NEW_SOURCE.md`
2. Search for entity_schema references

**Expected Result**:
- Clarifies source vs entity_schema distinction
- Cross-references entity_schemas documentation

**Verification Command**:
```bash
grep "entity_schema" docs/procedures/HOW_TO_ADD_NEW_SOURCE.md
# Expected: At least one match
```

---

### AT-8.3: Entity Schema Reference Created

**Requirement**: REQ-8.3

**Test Procedure**:
1. Check for specification document

**Expected Result**:
- ENTITY_SCHEMA_FORMAT.md exists in specification folder
- Complete YAML specification
- Examples for all stream types

**Verification Command**:
```bash
test -f product/features/dp-002/specification/ENTITY_SCHEMA_FORMAT.md && echo "EXISTS"
# Expected: EXISTS
```

---

### AT-8.4: Deploy Documentation Updated

**Requirement**: REQ-8.4

**Test Procedure**:
1. Check deploy script or related documentation
2. Search for sync-dictionary documentation

**Expected Result**:
- sync-dictionary command documented
- Usage examples provided

**Verification**: Manual review

---

## Test Execution Checklist

### Pre-Deployment Tests (Development)
- [ ] AT-1.1: DuckDB service removed
- [ ] AT-1.2: DuckDB volume removed
- [ ] AT-2.1: TimescaleDB service added
- [ ] AT-2.2: Resource limits configured
- [ ] AT-2.4: Persistence volume created
- [ ] AT-2.5: PostgreSQL port exposed
- [ ] AT-2.6: Health check configured
- [ ] AT-3.1 through AT-3.8: All entity schemas added
- [ ] AT-5.1 through AT-5.5: HomeAssistant stream configured
- [ ] AT-8.1 through AT-8.4: Documentation updated

### Deployment Tests (Staging)
- [ ] AT-1.3: Grafana DuckDB plugin works
- [ ] AT-1.4: Grafana volume mounts correct
- [ ] AT-2.3: Data dictionary schema created
- [ ] AT-4.1 through AT-4.5: Database tables and views
- [ ] AT-6.1 through AT-6.6: sync-dictionary command

### Post-Deployment Tests (Production)
- [ ] AT-7.1 through AT-7.6: Dashboard panels functional
- [ ] End-to-end: Add schema, sync, verify in dashboard

---

*This document is part of the SPARC Specification phase for DP-002.*
