# DP-002: Functional Requirements

**Document Type**: SPARC Specification
**Version**: 1.0.0
**Last Updated**: 2025-12-30
**Status**: Draft

---

## Overview

This document defines the functional requirements for DP-002: Online Data Dictionary & HomeAssistant Stream Preparation. Requirements are organized by scope item and include rationale and acceptance test references.

---

## REQ-1: DuckDB Container Removal

### REQ-1.1: Remove DuckDB Service from Docker Compose

**Description**: Remove the `duckdb` service definition from `deploy/pi/docker-compose.yml`.

**Rationale**: The DuckDB container is unused. Grafana connects directly to Parquet files via the DuckDB plugin, making the standalone container redundant.

**Acceptance Test**: AT-1.1

### REQ-1.2: Remove DuckDB Volume

**Description**: Remove the `duckdb-data` volume from the Docker Compose volumes section.

**Rationale**: With the container removed, the volume is no longer needed.

**Acceptance Test**: AT-1.2

### REQ-1.3: Verify Grafana DuckDB Plugin Independence

**Description**: Confirm that Grafana's DuckDB plugin operates independently of the DuckDB container by querying Bronze layer Parquet files directly.

**Rationale**: The DuckDB plugin embeds its own DuckDB engine and does not require an external container.

**Acceptance Test**: AT-1.3

### REQ-1.4: Update Grafana Volume Mounts

**Description**: Update Grafana's volume mounts to remove dependency on `duckdb-data` volume while retaining access to `air-quality-data` for Parquet files.

**Rationale**: Grafana needs Parquet file access but not the DuckDB database file.

**Acceptance Test**: AT-1.4

---

## REQ-2: TimescaleDB Instantiation

### REQ-2.1: Add TimescaleDB Service

**Description**: Add a TimescaleDB service to `deploy/pi/docker-compose.yml` using the official `timescale/timescaledb:latest-pg15` image (ARM64 compatible).

**Rationale**: TimescaleDB provides time-series optimized PostgreSQL for the Silver Layer with support for continuous aggregates and compression.

**Acceptance Test**: AT-2.1

### REQ-2.2: Configure Resource Limits

**Description**: Configure TimescaleDB with memory limits appropriate for Raspberry Pi 5 (max 512MB).

**Rationale**: Pi 5 has 8GB RAM shared across all services. Conservative limits prevent memory pressure.

**Acceptance Test**: AT-2.2

### REQ-2.3: Create Data Dictionary Schema

**Description**: Create the data dictionary tables in TimescaleDB on first initialization:
- `data_dictionary` - Normalized view of all entity schemas
- `entity_schemas` - Raw entity schema definitions from config
- `streams` - Stream metadata

**Rationale**: These tables store the queryable data dictionary populated from etcd config.

**Acceptance Test**: AT-2.3

### REQ-2.4: Configure Persistence Volume

**Description**: Create a persistent volume `timescaledb-data` for database storage.

**Rationale**: Data dictionary must survive container restarts.

**Acceptance Test**: AT-2.4

### REQ-2.5: Expose PostgreSQL Port

**Description**: Expose port 5432 for local connections from Grafana and deploy scripts.

**Rationale**: Grafana and sync scripts need to query TimescaleDB.

**Acceptance Test**: AT-2.5

### REQ-2.6: Add Health Check

**Description**: Configure a health check using `pg_isready` command.

**Rationale**: Dependent services (sync script, Grafana) should wait for TimescaleDB readiness.

**Acceptance Test**: AT-2.6

---

## REQ-3: Entity Schema Addition

### REQ-3.1: Define Entity Schema YAML Format

**Description**: Establish a standardized YAML format for `entity_schemas` in stream config files with:
- `schema_name`: Unique identifier (string)
- `description`: Human-readable description
- `device_class`: Optional device classification (for HomeAssistant)
- `attributes`: Array of attribute definitions

Each attribute must include:
- `name`: Attribute name (snake_case)
- `type`: Data type (float, int, string, bool, json)
- `unit`: Measurement unit (optional)
- `description`: Human-readable description
- `nullable`: Whether null values are allowed

**Rationale**: Standardized format enables automated data dictionary generation and validation.

**Acceptance Test**: AT-3.1

### REQ-3.2: Add Entity Schema to air-quality Stream

**Description**: Add `entity_schemas` section to `config/base/streams/air-quality/config.yaml` documenting the AirGradient sensor attributes.

**Schema Name**: `airgradient`

**Attributes to document**:
- pm25 (float, ug/m3): Particulate Matter 2.5 micrometers
- pm10 (float, ug/m3): Particulate Matter 10 micrometers
- co2 (int, ppm): Carbon Dioxide concentration
- temperature (float, celsius): Ambient temperature
- humidity (float, percent): Relative humidity
- tvoc (int, ppb): Total Volatile Organic Compounds
- nox (int, ppb): Nitrogen Oxides

**Rationale**: Documents the air-quality stream for data dictionary and DQ validation.

**Acceptance Test**: AT-3.2

### REQ-3.3: Add Entity Schema to outdoor-weather Stream

**Description**: Add `entity_schemas` section to `config/base/streams/outdoor-weather/config.yaml` documenting NWS weather attributes.

**Schema Name**: `nws-weather`

**Attributes**: temperature, feels_like, pressure, humidity, wind_speed, wind_deg, wind_gust, clouds, visibility, rain_1h, snow_1h

**Rationale**: Documents the outdoor-weather stream for data dictionary.

**Acceptance Test**: AT-3.3

### REQ-3.4: Add Entity Schema to outdoor-air-quality Stream

**Description**: Add `entity_schemas` section to `config/base/streams/outdoor-air-quality/config.yaml` documenting AirNow/OpenWeatherMap attributes.

**Schema Name**: `airnow`

**Attributes**: aqi, co, no, no2, o3, so2, pm2_5, pm10, nh3

**Rationale**: Documents the outdoor-air-quality stream for data dictionary.

**Acceptance Test**: AT-3.4

### REQ-3.5: Add Entity Schema to nws-observations Stream

**Description**: Add `entity_schemas` section to `config/base/streams/nws-observations/config.yaml` documenting NWS station observation attributes.

**Schema Name**: `nws-observations`

**Attributes**: temperature, dewpoint, wind_direction, wind_speed, wind_gust, barometric_pressure, sea_level_pressure, visibility, max_temperature_24h, min_temperature_24h, precipitation_1h, precipitation_3h, precipitation_6h, relative_humidity, wind_chill, heat_index

**Rationale**: Documents the nws-observations stream for data dictionary.

**Acceptance Test**: AT-3.5

### REQ-3.6: Add Entity Schema to nws-forecast-hourly Stream

**Description**: Add `entity_schemas` section to `config/base/streams/nws-forecast-hourly/config.yaml` documenting NWS hourly forecast attributes.

**Schema Name**: `nws-hourly`

**Attributes**: temperature, dewpoint, relative_humidity, wind_speed, wind_direction, short_forecast, probability_of_precipitation, forecast_issue_time

**Rationale**: Documents the nws-forecast-hourly stream for data dictionary.

**Acceptance Test**: AT-3.6

### REQ-3.7: Add Entity Schema to nws-gridpoints-forecast Stream

**Description**: Add `entity_schemas` section to `config/base/streams/nws-gridpoints-forecast/config.yaml` documenting NWS gridpoint forecast attributes.

**Schema Name**: `nws-gridpoints`

**Attributes**: All 40+ fields including temperature suite, wind suite, precipitation suite, sky/visibility, humidity, fire weather indices, and marine fields.

**Rationale**: Documents the comprehensive nws-gridpoints-forecast stream for data dictionary.

**Acceptance Test**: AT-3.7

### REQ-3.8: Preserve Existing Fields Section

**Description**: When adding `entity_schemas`, the existing `fields` section must remain unchanged.

**Rationale**: `fields` drives ingestion; modifications could break data flow. Accept temporary duplication for stability.

**Acceptance Test**: AT-3.8

---

## REQ-4: Data Dictionary Tables

### REQ-4.1: Create Streams Table

**Description**: Create a `streams` table in TimescaleDB:

```sql
CREATE TABLE streams (
    stream_id VARCHAR(64) PRIMARY KEY,
    description TEXT,
    version VARCHAR(20),
    enabled BOOLEAN DEFAULT true,
    retention_days INTEGER,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);
```

**Rationale**: Stores stream-level metadata for dictionary queries.

**Acceptance Test**: AT-4.1

### REQ-4.2: Create Entity Schemas Table

**Description**: Create an `entity_schemas` table in TimescaleDB:

```sql
CREATE TABLE entity_schemas (
    id SERIAL PRIMARY KEY,
    stream_id VARCHAR(64) REFERENCES streams(stream_id),
    schema_name VARCHAR(128) NOT NULL,
    description TEXT,
    device_class VARCHAR(64),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(stream_id, schema_name)
);
```

**Rationale**: Stores entity schema metadata linked to streams.

**Acceptance Test**: AT-4.2

### REQ-4.3: Create Data Dictionary Table

**Description**: Create a `data_dictionary` table in TimescaleDB:

```sql
CREATE TABLE data_dictionary (
    id SERIAL PRIMARY KEY,
    stream_id VARCHAR(64) REFERENCES streams(stream_id),
    schema_id INTEGER REFERENCES entity_schemas(id),
    schema_name VARCHAR(128) NOT NULL,
    attribute_name VARCHAR(64) NOT NULL,
    attribute_type VARCHAR(32) NOT NULL,
    unit VARCHAR(32),
    description TEXT,
    nullable BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(stream_id, schema_name, attribute_name)
);
```

**Rationale**: Normalized table for querying all attributes across all streams.

**Acceptance Test**: AT-4.3

### REQ-4.4: Create Unified Dictionary View

**Description**: Create a view that presents the data dictionary in a flat, queryable format:

```sql
CREATE VIEW v_data_dictionary AS
SELECT
    dd.stream_id,
    es.schema_name,
    es.description as schema_description,
    es.device_class,
    dd.attribute_name,
    dd.attribute_type,
    dd.unit,
    dd.description as attribute_description,
    dd.nullable
FROM data_dictionary dd
JOIN entity_schemas es ON dd.schema_id = es.id;
```

**Rationale**: Provides single-query access to complete dictionary regardless of stream structure.

**Acceptance Test**: AT-4.4

### REQ-4.5: Support Pattern Matching for HomeAssistant

**Description**: The data dictionary must support wildcard schema names like `sensor.airgradient_*` for pattern-matching HomeAssistant entities.

**Rationale**: HomeAssistant entities follow patterns (e.g., `sensor.airgradient_abc123_pm25`). The dictionary must match actual entity IDs to schema patterns.

**Acceptance Test**: AT-4.5

---

## REQ-5: HomeAssistant Stream Configuration

### REQ-5.1: Create HomeAssistant Stream Directory

**Description**: Create `config/base/streams/homeassistant/config.yaml`.

**Rationale**: New stream for HomeAssistant data via MQTT Statestream.

**Acceptance Test**: AT-5.1

### REQ-5.2: Define Generic Bronze Schema

**Description**: Define a generic Bronze layer schema for HomeAssistant events:
- entity_id (string): HomeAssistant entity identifier
- state (string): Current state value
- last_changed (timestamp): When state last changed
- last_updated (timestamp): When entity was last updated
- attributes (json): Entity-specific attributes

**Rationale**: Generic schema captures all HA entities; typing happens via entity_schemas in the dictionary.

**Acceptance Test**: AT-5.2

### REQ-5.3: Create AirGradient Entity Schema

**Description**: Create initial `entity_schemas` entry for AirGradient devices in HomeAssistant:

**Schema Name**: `sensor.airgradient_*`
**Device Class**: `air_quality`
**Attributes**: pm02, pm10, atmp, rhum, rco2, tvoc (matching HomeAssistant entity attributes)

**Rationale**: AirGradient devices are already publishing via MQTT and can be used for validation.

**Acceptance Test**: AT-5.3

### REQ-5.4: Configure MQTT Source for Statestream

**Description**: Configure an MQTT source for the HomeAssistant stream:
- Topic pattern: `homeassistant/+/+/state`
- Parser type: Custom HomeAssistant MQTT parser

**Rationale**: HA MQTT Statestream publishes entity states to this topic structure.

**Acceptance Test**: AT-5.4

### REQ-5.5: Document Stream Configuration

**Description**: Include standard stream metadata:
- stream_id: homeassistant
- description: Home Assistant entity states via MQTT Statestream
- retention_days: 365
- enabled: false (initially disabled until HA integration is active)

**Rationale**: Stream should be ready but disabled until Phase 2 HomeAssistant integration.

**Acceptance Test**: AT-5.5

---

## REQ-6: Deploy Script Extension

### REQ-6.1: Add sync-dictionary Command

**Description**: Extend `deploy/pi/deploy.sh` with a new `sync-dictionary` command that:
1. Reads all stream configs from etcd
2. Extracts entity_schemas from each stream
3. Upserts to TimescaleDB data dictionary tables

**Rationale**: Single command to sync config to dictionary, consistent with existing deploy.sh patterns.

**Acceptance Test**: AT-6.1

### REQ-6.2: Implement Upsert Logic

**Description**: Sync script must support:
- **Add**: New stream/schema appears in config -> insert to dictionary
- **Update**: Schema definition changes -> update dictionary
- **Delete**: Schema removed from config -> remove from dictionary

**Rationale**: Full CRUD support ensures dictionary matches config.

**Acceptance Test**: AT-6.2

### REQ-6.3: Read from etcd

**Description**: Sync script reads stream configurations from etcd at `/streams/{stream-id}/` prefix.

**Rationale**: etcd is the operational config source; sync from etcd not YAML files.

**Acceptance Test**: AT-6.3

### REQ-6.4: Validate Before Sync

**Description**: Sync script must validate entity_schema format before writing to database:
- Required fields present (schema_name, attributes)
- Attribute types are valid
- No duplicate attribute names within schema

**Rationale**: Prevents invalid data from corrupting the dictionary.

**Acceptance Test**: AT-6.4

### REQ-6.5: Idempotent Operation

**Description**: Running `sync-dictionary` multiple times with unchanged config produces no database changes.

**Rationale**: Safe to run as part of deployment automation.

**Acceptance Test**: AT-6.5

### REQ-6.6: Logging and Reporting

**Description**: Sync script outputs:
- Number of streams processed
- Number of schemas added/updated/deleted
- Any validation errors encountered

**Rationale**: Operators need feedback on sync results.

**Acceptance Test**: AT-6.6

---

## REQ-7: Data Quality Dashboard

### REQ-7.1: Create Grafana Dashboard

**Description**: Create a Grafana dashboard JSON file at `config/grafana/dashboards/homeassistant-data-quality.json`.

**Rationale**: Centralized DQ monitoring for HomeAssistant stream readiness.

**Acceptance Test**: AT-7.1

### REQ-7.2: Schema Coverage Panel

**Description**: Panel showing:
- Total defined schemas
- Total streams
- Percentage of streams with entity_schemas

**Rationale**: Shows overall data dictionary coverage.

**Acceptance Test**: AT-7.2

### REQ-7.3: Unknown Entities Panel

**Description**: Panel listing entities that:
- Appear in Bronze data
- Do not match any entity_schema pattern

**Rationale**: Identifies entities needing schema definitions.

**Acceptance Test**: AT-7.3

### REQ-7.4: Incomplete Schemas Panel

**Description**: Panel showing schemas where:
- Defined attributes differ from actual data
- Missing expected attributes
- Extra undocumented attributes

**Rationale**: Helps maintain accurate schema definitions.

**Acceptance Test**: AT-7.4

### REQ-7.5: Attribute Heatmap Panel

**Description**: Panel showing presence/absence of attributes by device_class or entity pattern.

**Rationale**: Visual overview of data completeness across devices.

**Acceptance Test**: AT-7.5

### REQ-7.6: Dynamic Dashboard Queries

**Description**: Dashboard queries the TimescaleDB data dictionary tables directly. Schema changes automatically reflected without dashboard edits.

**Rationale**: Schema-driven dashboards reduce maintenance.

**Acceptance Test**: AT-7.6

---

## REQ-8: Documentation Updates

### REQ-8.1: Update HOW_TO_ADD_NEW_STREAM.md

**Description**: Add section on entity_schemas covering:
- Purpose (data dictionary vs ingestion)
- YAML format specification
- Complete examples for different stream types
- Relationship between `fields` and `entity_schemas`

**Rationale**: Enables future contributors to add streams correctly.

**Acceptance Test**: AT-8.1

### REQ-8.2: Update HOW_TO_ADD_NEW_SOURCE.md

**Description**: Add clarification on:
- Source vs entity_schema distinction
- Cross-reference to entity_schemas documentation
- When to update data dictionary after source changes

**Rationale**: Clarifies terminology and relationships.

**Acceptance Test**: AT-8.2

### REQ-8.3: Create Entity Schema Reference

**Description**: Create or update documentation with:
- Complete entity_schema YAML specification
- All supported attribute types
- Examples for each stream type
- Pattern matching syntax for HomeAssistant

**Rationale**: Single authoritative reference for entity_schema format.

**Acceptance Test**: AT-8.3

### REQ-8.4: Update Deploy Documentation

**Description**: Document the new `sync-dictionary` command in deployment documentation.

**Rationale**: Operators need to know about new deploy script capabilities.

**Acceptance Test**: AT-8.4

---

## Requirement Traceability Matrix

| Requirement | Scope Item | Priority | Acceptance Tests |
|------------|------------|----------|------------------|
| REQ-1.1-1.4 | DuckDB Removal | High | AT-1.1 - AT-1.4 |
| REQ-2.1-2.6 | TimescaleDB | High | AT-2.1 - AT-2.6 |
| REQ-3.1-3.8 | Entity Schemas | High | AT-3.1 - AT-3.8 |
| REQ-4.1-4.5 | Data Dictionary | High | AT-4.1 - AT-4.5 |
| REQ-5.1-5.5 | HomeAssistant Stream | Medium | AT-5.1 - AT-5.5 |
| REQ-6.1-6.6 | Deploy Script | High | AT-6.1 - AT-6.6 |
| REQ-7.1-7.6 | DQ Dashboard | Medium | AT-7.1 - AT-7.6 |
| REQ-8.1-8.4 | Documentation | Medium | AT-8.1 - AT-8.4 |

---

## Non-Functional Requirements

### NFR-1: Performance

- Data dictionary queries must complete in < 100ms for typical lookups
- Sync script must complete in < 30 seconds for all streams
- Dashboard refresh must complete in < 5 seconds

### NFR-2: Resource Constraints

- TimescaleDB memory usage must not exceed 512MB
- Total stack memory must remain under 4GB (Pi 5 headroom)

### NFR-3: Reliability

- Dictionary sync must be idempotent
- Failed sync must not corrupt existing dictionary
- Dashboard must gracefully handle missing data

### NFR-4: Maintainability

- Entity schema format must be documented
- All tables must have clear naming conventions
- Dashboard queries must be readable and maintainable

---

*This document is part of the SPARC Specification phase for DP-002.*
