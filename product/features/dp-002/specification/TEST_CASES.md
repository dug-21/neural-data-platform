# DP-002 Test Cases

## Overview

This document contains detailed test cases for DP-002: Online Data Dictionary & HomeAssistant Stream Preparation. Test cases are organized by scope item and include preconditions, steps, and expected results.

---

## Test Case Format

Each test case follows this structure:

| Field | Description |
|-------|-------------|
| **ID** | Unique identifier (TC-{scope}-{number}) |
| **Description** | Brief description of what is tested |
| **Priority** | Critical / High / Medium / Low |
| **Type** | Unit / Integration / E2E |
| **Preconditions** | Required state before test |
| **Steps** | Numbered actions to perform |
| **Expected Result** | What should happen |
| **Postconditions** | State after test (if applicable) |

---

## TC-1.x: DuckDB Container Removal Tests

### TC-1.1: Verify DuckDB Plugin Queries Work Without Container

| Field | Value |
|-------|-------|
| **ID** | TC-1.1 |
| **Description** | Grafana DuckDB plugin can query Parquet files directly without requiring DuckDB container |
| **Priority** | Critical |
| **Type** | Integration |
| **Preconditions** | - DuckDB container removed from docker-compose.yml<br>- Grafana running with DuckDB plugin installed<br>- Bronze Parquet files exist in /data/bronze/ |
| **Steps** | 1. Ensure DuckDB container is NOT running (`docker ps \| grep duckdb` returns nothing)<br>2. Open Grafana dashboard that uses DuckDB datasource<br>3. Execute query: `SELECT * FROM '/data/bronze/air-quality/*.parquet' LIMIT 10`<br>4. Verify results returned |
| **Expected Result** | Query executes successfully and returns data rows. No connection errors. |
| **Postconditions** | None |

---

### TC-1.2: Verify No Container Runtime Dependencies

| Field | Value |
|-------|-------|
| **ID** | TC-1.2 |
| **Description** | Application startup does not require DuckDB container |
| **Priority** | Critical |
| **Type** | Integration |
| **Preconditions** | - DuckDB service removed from docker-compose.yml<br>- No references to DuckDB container in app code |
| **Steps** | 1. Stop all containers: `docker-compose down`<br>2. Remove DuckDB from docker-compose.yml<br>3. Start services: `docker-compose up -d`<br>4. Wait for all services to be healthy (60s timeout)<br>5. Check air-quality-app health: `curl http://localhost:8080/health` |
| **Expected Result** | All services start without error. Health check returns healthy status. |
| **Postconditions** | Services running without DuckDB container |

---

### TC-1.3: Verify Existing Dashboards Load Without DuckDB Container

| Field | Value |
|-------|-------|
| **ID** | TC-1.3 |
| **Description** | All existing Grafana dashboards that use DuckDB datasource continue to work |
| **Priority** | High |
| **Type** | E2E |
| **Preconditions** | - DuckDB container removed<br>- Grafana running<br>- Dashboards exist: outdoor-conditions, air-quality, etc. |
| **Steps** | 1. Open each dashboard in browser<br>2. For each dashboard, verify all panels load without error<br>3. Check Grafana logs for any datasource errors |
| **Expected Result** | All panels display data. No "Datasource not found" or connection errors. |
| **Postconditions** | None |

---

## TC-2.x: TimescaleDB Instantiation Tests

### TC-2.1: TimescaleDB Container Starts Successfully

| Field | Value |
|-------|-------|
| **ID** | TC-2.1 |
| **Description** | TimescaleDB container starts and becomes healthy |
| **Priority** | Critical |
| **Type** | Integration |
| **Preconditions** | - TimescaleDB added to docker-compose.yml<br>- Docker installed and running |
| **Steps** | 1. Start services: `./deploy.sh start`<br>2. Wait for container health check (120s timeout)<br>3. Verify container status: `docker ps \| grep timescaledb`<br>4. Test connection: `docker exec timescaledb psql -U postgres -c "SELECT 1;"` |
| **Expected Result** | Container shows "healthy" status. psql command returns "1". |
| **Postconditions** | TimescaleDB running and accepting connections |

---

### TC-2.2: TimescaleDB Schema Creation Succeeds

| Field | Value |
|-------|-------|
| **ID** | TC-2.2 |
| **Description** | Data dictionary tables are created in TimescaleDB |
| **Priority** | Critical |
| **Type** | Integration |
| **Preconditions** | - TimescaleDB container running<br>- Schema migration scripts available |
| **Steps** | 1. Run schema creation: `./deploy.sh sync-dictionary`<br>2. Connect to database: `docker exec timescaledb psql -U postgres -d ndp`<br>3. List tables: `\dt`<br>4. Describe data_dictionary table: `\d data_dictionary` |
| **Expected Result** | Table `data_dictionary` exists with columns: stream_id, schema_name, attribute_name, attribute_type, unit, description, created_at, updated_at |
| **Postconditions** | Schema created in database |

---

### TC-2.3: TimescaleDB Memory Usage Within Pi Limits

| Field | Value |
|-------|-------|
| **ID** | TC-2.3 |
| **Description** | TimescaleDB memory consumption is acceptable for Raspberry Pi |
| **Priority** | High |
| **Type** | Performance |
| **Preconditions** | - TimescaleDB running<br>- Data dictionary populated with all stream schemas |
| **Steps** | 1. Start all services: `./deploy.sh start`<br>2. Wait 5 minutes for stabilization<br>3. Check memory: `docker stats timescaledb --no-stream`<br>4. Record memory usage |
| **Expected Result** | Memory usage < 512MB (constraint for Pi 5 with 8GB RAM, other services need memory) |
| **Postconditions** | None |

---

### TC-2.4: TimescaleDB Persists Data Across Restarts

| Field | Value |
|-------|-------|
| **ID** | TC-2.4 |
| **Description** | Data dictionary entries survive container restart |
| **Priority** | High |
| **Type** | Integration |
| **Preconditions** | - TimescaleDB running<br>- Data dictionary contains entries |
| **Steps** | 1. Insert test entry: `INSERT INTO data_dictionary (...) VALUES (...)`<br>2. Verify entry exists: `SELECT * FROM data_dictionary WHERE ...`<br>3. Restart container: `docker restart timescaledb`<br>4. Wait for healthy status<br>5. Query for entry again |
| **Expected Result** | Entry exists after restart. No data loss. |
| **Postconditions** | None |

---

## TC-3.x: Entity Schema Parsing Tests

### TC-3.1: Parser Handles Valid Entity Schema YAML

| Field | Value |
|-------|-------|
| **ID** | TC-3.1 |
| **Description** | Entity schema parser correctly parses valid YAML configuration |
| **Priority** | Critical |
| **Type** | Unit |
| **Preconditions** | None |
| **Steps** | 1. Create valid entity_schema YAML string with schema_name, description, attributes<br>2. Call parse_entity_schema(yaml)<br>3. Verify returned struct fields match input |
| **Expected Result** | Parser returns Ok(EntitySchema) with correct values |
| **Postconditions** | None |

**Test Data**:
```yaml
schema_name: airgradient
description: AirGradient indoor sensors
device_class: air_quality
attributes:
  - name: pm25
    type: f64
    unit: "ug/m3"
    description: PM2.5 concentration
```

---

### TC-3.2: Parser Rejects Invalid Schema - Missing Required Fields

| Field | Value |
|-------|-------|
| **ID** | TC-3.2 |
| **Description** | Parser returns error for schema missing required fields |
| **Priority** | Critical |
| **Type** | Unit |
| **Preconditions** | None |
| **Steps** | 1. Create YAML missing `schema_name` field<br>2. Call parse_entity_schema(yaml)<br>3. Verify error type |
| **Expected Result** | Parser returns Err(ParseError::MissingField("schema_name")) |
| **Postconditions** | None |

---

### TC-3.3: Parser Rejects Invalid Schema - Invalid Attribute Type

| Field | Value |
|-------|-------|
| **ID** | TC-3.3 |
| **Description** | Parser returns error for invalid attribute type |
| **Priority** | High |
| **Type** | Unit |
| **Preconditions** | None |
| **Steps** | 1. Create YAML with attribute type "invalid_type"<br>2. Call parse_entity_schema(yaml)<br>3. Verify error type |
| **Expected Result** | Parser returns Err(ParseError::InvalidType("invalid_type")) |
| **Postconditions** | None |

**Valid types**: f64, i64, string, bool, json

---

### TC-3.4: Parser Handles Optional Fields Gracefully

| Field | Value |
|-------|-------|
| **ID** | TC-3.4 |
| **Description** | Parser accepts schema with optional fields omitted |
| **Priority** | Medium |
| **Type** | Unit |
| **Preconditions** | None |
| **Steps** | 1. Create minimal YAML with only required fields (schema_name, attributes)<br>2. Call parse_entity_schema(yaml)<br>3. Verify optional fields have default values |
| **Expected Result** | Parser returns Ok(). description is empty string, device_class is None. |
| **Postconditions** | None |

---

### TC-3.5: All 6 Existing Streams Have Valid Entity Schemas

| Field | Value |
|-------|-------|
| **ID** | TC-3.5 |
| **Description** | Entity schemas for all 6 existing streams parse successfully |
| **Priority** | Critical |
| **Type** | Integration |
| **Preconditions** | - Entity schemas added to all 6 stream config files |
| **Steps** | For each stream (air-quality, outdoor-weather, outdoor-air-quality, nws-observations, nws-forecast-hourly, nws-gridpoints-forecast):<br>1. Load config.yaml<br>2. Extract entity_schemas section<br>3. Parse each schema<br>4. Verify no errors |
| **Expected Result** | All 6 streams have valid, parseable entity_schemas |
| **Postconditions** | None |

---

### TC-3.6: Parser Handles Multiple Attributes Per Schema

| Field | Value |
|-------|-------|
| **ID** | TC-3.6 |
| **Description** | Parser correctly handles schemas with many attributes |
| **Priority** | Medium |
| **Type** | Unit |
| **Preconditions** | None |
| **Steps** | 1. Create YAML with 10+ attributes<br>2. Call parse_entity_schema(yaml)<br>3. Verify all attributes are parsed |
| **Expected Result** | All attributes accessible in returned schema |
| **Postconditions** | None |

---

## TC-4.x: Data Dictionary Query Tests

### TC-4.1: Query All Streams From Dictionary

| Field | Value |
|-------|-------|
| **ID** | TC-4.1 |
| **Description** | Data dictionary returns list of all configured streams |
| **Priority** | Critical |
| **Type** | Integration |
| **Preconditions** | - Data dictionary populated with all stream schemas<br>- TimescaleDB running |
| **Steps** | 1. Execute: `SELECT DISTINCT stream_id FROM data_dictionary ORDER BY stream_id`<br>2. Verify all 7 streams returned (6 existing + homeassistant) |
| **Expected Result** | Returns: air-quality, homeassistant, nws-forecast-hourly, nws-gridpoints-forecast, nws-observations, outdoor-air-quality, outdoor-weather |
| **Postconditions** | None |

---

### TC-4.2: Query Attributes for Specific Stream

| Field | Value |
|-------|-------|
| **ID** | TC-4.2 |
| **Description** | Data dictionary returns all attributes for a given stream |
| **Priority** | Critical |
| **Type** | Integration |
| **Preconditions** | - air-quality stream schema synced to dictionary |
| **Steps** | 1. Execute: `SELECT attribute_name, attribute_type, unit FROM data_dictionary WHERE stream_id = 'air-quality'`<br>2. Verify expected attributes returned |
| **Expected Result** | Returns attributes: pm25, pm10, rco2, atmp, rhum, tvoc, nox (with correct types and units) |
| **Postconditions** | None |

---

### TC-4.3: Query by Schema Pattern (HomeAssistant)

| Field | Value |
|-------|-------|
| **ID** | TC-4.3 |
| **Description** | Data dictionary supports pattern-based queries for HomeAssistant entities |
| **Priority** | High |
| **Type** | Integration |
| **Preconditions** | - HomeAssistant stream schema synced |
| **Steps** | 1. Execute: `SELECT * FROM data_dictionary WHERE stream_id = 'homeassistant' AND schema_name LIKE 'sensor.airgradient%'`<br>2. Verify matching schemas returned |
| **Expected Result** | Returns AirGradient sensor schema with expected attributes |
| **Postconditions** | None |

---

### TC-4.4: Query Returns Empty for Nonexistent Stream

| Field | Value |
|-------|-------|
| **ID** | TC-4.4 |
| **Description** | Query for nonexistent stream returns empty result, not error |
| **Priority** | Medium |
| **Type** | Integration |
| **Preconditions** | - Data dictionary populated |
| **Steps** | 1. Execute: `SELECT * FROM data_dictionary WHERE stream_id = 'nonexistent-stream'`<br>2. Verify result |
| **Expected Result** | Returns 0 rows, no error |
| **Postconditions** | None |

---

### TC-4.5: Query Performance Under Load

| Field | Value |
|-------|-------|
| **ID** | TC-4.5 |
| **Description** | Dictionary queries complete within acceptable time |
| **Priority** | Medium |
| **Type** | Performance |
| **Preconditions** | - Data dictionary populated with all schemas |
| **Steps** | 1. Execute 100 concurrent queries for different streams<br>2. Measure average response time |
| **Expected Result** | Average query time < 50ms |
| **Postconditions** | None |

---

## TC-5.x: HomeAssistant Stream Configuration Tests

### TC-5.1: HomeAssistant Config File Parses Correctly

| Field | Value |
|-------|-------|
| **ID** | TC-5.1 |
| **Description** | HomeAssistant stream config.yaml is valid and parseable |
| **Priority** | Critical |
| **Type** | Unit |
| **Preconditions** | - config/base/streams/homeassistant/config.yaml exists |
| **Steps** | 1. Load config.yaml<br>2. Parse as StreamConfig<br>3. Verify required fields present |
| **Expected Result** | Config parses successfully. stream_id = "homeassistant", source type is valid. |
| **Postconditions** | None |

---

### TC-5.2: AirGradient Entity Schema Validates Correctly

| Field | Value |
|-------|-------|
| **ID** | TC-5.2 |
| **Description** | AirGradient entity_schema has correct attributes for HomeAssistant integration |
| **Priority** | Critical |
| **Type** | Unit |
| **Preconditions** | - HomeAssistant config with entity_schemas defined |
| **Steps** | 1. Load homeassistant config.yaml<br>2. Extract entity_schema for "sensor.airgradient_*"<br>3. Verify attributes: pm02, pm10, atmp, rhum, rco2, tvoc |
| **Expected Result** | All 6 expected attributes present with correct types |
| **Postconditions** | None |

---

### TC-5.3: Pattern Matching Works for sensor.* Entities

| Field | Value |
|-------|-------|
| **ID** | TC-5.3 |
| **Description** | Entity pattern "sensor.airgradient_*" matches expected entities |
| **Priority** | High |
| **Type** | Unit |
| **Preconditions** | - Pattern matching function implemented |
| **Steps** | 1. Define pattern: "sensor.airgradient_*"<br>2. Test entities:<br>  - "sensor.airgradient_co2" (should match)<br>  - "sensor.airgradient_pm25" (should match)<br>  - "sensor.other_device" (should NOT match)<br>  - "binary_sensor.airgradient" (should NOT match) |
| **Expected Result** | Only sensor.airgradient_* entities match |
| **Postconditions** | None |

---

### TC-5.4: HomeAssistant Bronze Schema Is Generic

| Field | Value |
|-------|-------|
| **ID** | TC-5.4 |
| **Description** | Bronze layer schema captures all HA entities generically |
| **Priority** | High |
| **Type** | Unit |
| **Preconditions** | - HomeAssistant config defines Bronze fields |
| **Steps** | 1. Load homeassistant config.yaml<br>2. Verify fields include: entity_id (string), state (string), attributes (json), last_changed (timestamp) |
| **Expected Result** | Generic fields present, NOT device-specific fields in Bronze layer |
| **Postconditions** | None |

---

### TC-5.5: Pattern Supports Multiple Entity Schemas

| Field | Value |
|-------|-------|
| **ID** | TC-5.5 |
| **Description** | HomeAssistant stream can have multiple entity_schemas |
| **Priority** | Medium |
| **Type** | Unit |
| **Preconditions** | - HomeAssistant config with multiple entity_schemas |
| **Steps** | 1. Add second entity_schema: "binary_sensor.*_window*"<br>2. Parse config<br>3. Verify both schemas accessible |
| **Expected Result** | Both schemas parse successfully and are distinct |
| **Postconditions** | None |

---

## TC-6.x: Deploy Script Tests

### TC-6.1: sync-dictionary Creates Tables

| Field | Value |
|-------|-------|
| **ID** | TC-6.1 |
| **Description** | Running sync-dictionary command creates data dictionary entries |
| **Priority** | Critical |
| **Type** | Integration |
| **Preconditions** | - etcd running with stream configs<br>- TimescaleDB running with empty data_dictionary |
| **Steps** | 1. Verify table empty: `SELECT COUNT(*) FROM data_dictionary` returns 0<br>2. Run: `./deploy.sh sync-dictionary`<br>3. Verify entries: `SELECT COUNT(*) FROM data_dictionary` |
| **Expected Result** | Entry count > 0 (should have entries for all streams with entity_schemas) |
| **Postconditions** | Data dictionary populated |

---

### TC-6.2: sync-dictionary Handles Updates

| Field | Value |
|-------|-------|
| **ID** | TC-6.2 |
| **Description** | Running sync-dictionary updates existing entries when schema changes |
| **Priority** | High |
| **Type** | Integration |
| **Preconditions** | - Initial sync completed<br>- Entry exists in dictionary |
| **Steps** | 1. Get current entry: `SELECT description FROM data_dictionary WHERE ...`<br>2. Update entity_schema description in etcd<br>3. Run: `./deploy.sh sync-dictionary`<br>4. Get updated entry<br>5. Verify description changed |
| **Expected Result** | Entry description matches updated config |
| **Postconditions** | None |

---

### TC-6.3: sync-dictionary Handles Deletes

| Field | Value |
|-------|-------|
| **ID** | TC-6.3 |
| **Description** | Removing entity_schema from config removes it from dictionary |
| **Priority** | High |
| **Type** | Integration |
| **Preconditions** | - Initial sync completed<br>- Entry exists in dictionary |
| **Steps** | 1. Verify entry exists<br>2. Remove entity_schema from etcd config<br>3. Run: `./deploy.sh sync-dictionary`<br>4. Query for removed entry |
| **Expected Result** | Entry no longer exists in dictionary |
| **Postconditions** | None |

---

### TC-6.4: sync-dictionary Is Idempotent

| Field | Value |
|-------|-------|
| **ID** | TC-6.4 |
| **Description** | Running sync-dictionary multiple times produces same result |
| **Priority** | Medium |
| **Type** | Integration |
| **Preconditions** | - etcd and TimescaleDB running |
| **Steps** | 1. Run: `./deploy.sh sync-dictionary`<br>2. Get row count and checksums<br>3. Run: `./deploy.sh sync-dictionary` again<br>4. Get row count and checksums again |
| **Expected Result** | Row count and checksums identical after both runs |
| **Postconditions** | None |

---

### TC-6.5: sync-dictionary Reports Errors Clearly

| Field | Value |
|-------|-------|
| **ID** | TC-6.5 |
| **Description** | sync-dictionary provides clear error messages on failure |
| **Priority** | Medium |
| **Type** | Integration |
| **Preconditions** | - TimescaleDB NOT running |
| **Steps** | 1. Stop TimescaleDB: `docker stop timescaledb`<br>2. Run: `./deploy.sh sync-dictionary`<br>3. Check exit code and stderr |
| **Expected Result** | Non-zero exit code. Error message mentions database connection failure. |
| **Postconditions** | Restart TimescaleDB for other tests |

---

### TC-6.6: sync Command Still Works

| Field | Value |
|-------|-------|
| **ID** | TC-6.6 |
| **Description** | Existing sync command functionality unchanged |
| **Priority** | Critical |
| **Type** | Regression |
| **Preconditions** | - etcd running |
| **Steps** | 1. Run: `./deploy.sh sync`<br>2. Verify stream configs synced to etcd<br>3. List streams: `./deploy.sh list-streams` |
| **Expected Result** | All 6 (or 7 with HA) streams listed correctly |
| **Postconditions** | None |

---

## TC-7.x: Dashboard Tests

### TC-7.1: Schema Coverage Panel Loads

| Field | Value |
|-------|-------|
| **ID** | TC-7.1 |
| **Description** | Data Quality dashboard Schema Coverage panel loads without error |
| **Priority** | High |
| **Type** | E2E |
| **Preconditions** | - Dashboard deployed to Grafana<br>- Data dictionary populated |
| **Steps** | 1. Open Data Quality dashboard in browser<br>2. Locate "Schema Coverage Summary" panel<br>3. Verify panel displays data |
| **Expected Result** | Panel shows known vs unknown entity counts. No query errors. |
| **Postconditions** | None |

---

### TC-7.2: Unknown Entities Panel Loads

| Field | Value |
|-------|-------|
| **ID** | TC-7.2 |
| **Description** | Unknown Entities panel displays entities without matching schema |
| **Priority** | High |
| **Type** | E2E |
| **Preconditions** | - Dashboard deployed<br>- Some Bronze data has entities not in dictionary |
| **Steps** | 1. Open Data Quality dashboard<br>2. Locate "Unknown Entities" panel<br>3. Verify list displays |
| **Expected Result** | Panel shows list of entity IDs not matching any schema pattern |
| **Postconditions** | None |

---

### TC-7.3: Dashboard Updates Without JSON Edits

| Field | Value |
|-------|-------|
| **ID** | TC-7.3 |
| **Description** | Adding new entity_schema requires no dashboard JSON changes |
| **Priority** | High |
| **Type** | Integration |
| **Preconditions** | - Dashboard deployed<br>- Dashboard queries data dictionary dynamically |
| **Steps** | 1. Note current Schema Coverage panel count<br>2. Add new entity_schema to config<br>3. Run: `./deploy.sh sync-dictionary`<br>4. Refresh dashboard<br>5. Check Schema Coverage count |
| **Expected Result** | Count increased by number of new attributes. No dashboard JSON changes needed. |
| **Postconditions** | None |

---

### TC-7.4: Dashboard Queries Use TimescaleDB Not DuckDB for Dictionary

| Field | Value |
|-------|-------|
| **ID** | TC-7.4 |
| **Description** | Data Quality dashboard queries TimescaleDB for dictionary data |
| **Priority** | Medium |
| **Type** | Integration |
| **Preconditions** | - Dashboard deployed |
| **Steps** | 1. Inspect dashboard JSON<br>2. Verify datasource for dictionary panels is TimescaleDB (not DuckDB)<br>3. Verify Bronze data panels still use DuckDB |
| **Expected Result** | Dictionary panels: TimescaleDB datasource. Bronze panels: DuckDB datasource. |
| **Postconditions** | None |

---

## TC-8.x: Documentation Tests

### TC-8.1: HOW_TO_ADD_NEW_STREAM Is Accurate

| Field | Value |
|-------|-------|
| **ID** | TC-8.1 |
| **Description** | Updated procedure document accurately describes entity_schema addition |
| **Priority** | High |
| **Type** | Review |
| **Preconditions** | - Documentation updated |
| **Steps** | 1. Follow HOW_TO_ADD_NEW_STREAM.md to add a test stream<br>2. Include entity_schemas as documented<br>3. Run sync-dictionary<br>4. Verify stream appears in data dictionary |
| **Expected Result** | Following documentation results in working stream with entity_schema |
| **Postconditions** | Remove test stream |

---

### TC-8.2: Examples in Documentation Work

| Field | Value |
|-------|-------|
| **ID** | TC-8.2 |
| **Description** | YAML examples in documentation are valid and parseable |
| **Priority** | Medium |
| **Type** | Review |
| **Preconditions** | - Documentation updated |
| **Steps** | 1. Extract all YAML code blocks from HOW_TO_ADD_NEW_STREAM.md<br>2. Parse each as entity_schema or config |
| **Expected Result** | All examples parse without error |
| **Postconditions** | None |

---

### TC-8.3: HOW_TO_ADD_NEW_SOURCE Updated

| Field | Value |
|-------|-------|
| **ID** | TC-8.3 |
| **Description** | Source procedure document clarifies source vs entity_schema relationship |
| **Priority** | Medium |
| **Type** | Review |
| **Preconditions** | - Documentation updated |
| **Steps** | 1. Read HOW_TO_ADD_NEW_SOURCE.md<br>2. Verify it explains that sources don't define entity_schemas<br>3. Verify cross-reference to HOW_TO_ADD_NEW_STREAM for entity_schemas |
| **Expected Result** | Clear distinction between source and entity_schema documented |
| **Postconditions** | None |

---

### TC-8.4: Entity Schema Format Documented

| Field | Value |
|-------|-------|
| **ID** | TC-8.4 |
| **Description** | Entity schema YAML format is fully documented |
| **Priority** | Medium |
| **Type** | Review |
| **Preconditions** | - Documentation updated |
| **Steps** | 1. Review documentation for entity_schema format<br>2. Verify all fields documented: schema_name, description, device_class, attributes<br>3. Verify attribute fields documented: name, type, unit, description<br>4. Verify valid types listed |
| **Expected Result** | Complete schema reference available in documentation |
| **Postconditions** | None |

---

## TC-9.x: Regression Tests

### TC-9.1: Bronze Ingestion Continues for All Streams

| Field | Value |
|-------|-------|
| **ID** | TC-9.1 |
| **Description** | All 6 existing streams continue ingesting data to Bronze layer |
| **Priority** | Critical |
| **Type** | Regression |
| **Preconditions** | - All DP-002 changes deployed<br>- Streams running |
| **Steps** | For each stream:<br>1. Get current file count in /data/bronze/{stream}/<br>2. Wait 60 seconds<br>3. Get new file count<br>4. Verify count increased or recent file exists |
| **Expected Result** | All 6 streams show active ingestion |
| **Postconditions** | None |

---

### TC-9.2: Existing etcd Sync Unchanged

| Field | Value |
|-------|-------|
| **ID** | TC-9.2 |
| **Description** | Standard config sync to etcd works as before |
| **Priority** | Critical |
| **Type** | Regression |
| **Preconditions** | - etcd running |
| **Steps** | 1. Modify a stream config field<br>2. Run: `./deploy.sh sync`<br>3. Query etcd: `etcdctl get --prefix /air-quality/streams/`<br>4. Verify change reflected |
| **Expected Result** | Config changes sync to etcd as before |
| **Postconditions** | Revert config change |

---

### TC-9.3: Air Quality Dashboard Unchanged

| Field | Value |
|-------|-------|
| **ID** | TC-9.3 |
| **Description** | Existing Air Quality dashboard functions correctly |
| **Priority** | High |
| **Type** | Regression |
| **Preconditions** | - All changes deployed<br>- Dashboard accessible |
| **Steps** | 1. Open Air Quality dashboard<br>2. Verify all panels load<br>3. Verify data is recent (within last hour) |
| **Expected Result** | Dashboard functions identically to before DP-002 |
| **Postconditions** | None |

---

### TC-9.4: Outdoor Conditions Dashboard Unchanged

| Field | Value |
|-------|-------|
| **ID** | TC-9.4 |
| **Description** | Existing Outdoor Conditions dashboard functions correctly |
| **Priority** | High |
| **Type** | Regression |
| **Preconditions** | - All changes deployed |
| **Steps** | 1. Open Outdoor Conditions dashboard<br>2. Verify NWS data panels load<br>3. Verify forecast panels load |
| **Expected Result** | Dashboard functions identically to before DP-002 |
| **Postconditions** | None |

---

### TC-9.5: Application Health Check Passes

| Field | Value |
|-------|-------|
| **ID** | TC-9.5 |
| **Description** | Air Quality App health endpoint returns healthy |
| **Priority** | Critical |
| **Type** | Regression |
| **Preconditions** | - Application running |
| **Steps** | 1. Call: `curl http://localhost:8080/health`<br>2. Parse response |
| **Expected Result** | Returns 200 with healthy status |
| **Postconditions** | None |

---

## Test Execution Summary

### By Priority

| Priority | Count | Required for Deploy |
|----------|-------|---------------------|
| Critical | 18 | Yes |
| High | 15 | Yes (with exceptions) |
| Medium | 10 | No |
| Low | 0 | No |

### By Type

| Type | Count |
|------|-------|
| Unit | 14 |
| Integration | 20 |
| E2E | 4 |
| Performance | 3 |
| Regression | 5 |
| Review | 4 |
| **Total** | **50** |

---

## Related Documents

- [TEST_STRATEGY.md](./TEST_STRATEGY.md) - Overall test strategy
- [VALIDATION_CHECKLIST.md](./VALIDATION_CHECKLIST.md) - Manual validation steps
- [SCOPE.md](../SCOPE.md) - Feature scope definition

---

*This document contains all test cases for DP-002. Execute according to TEST_STRATEGY.md guidelines.*
