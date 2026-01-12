# Platform Architecture Assessment: Multi-Domain Extensibility

**Document**: 08-platform-architecture-assessment.md
**Author**: NDP Architect
**Date**: 2026-01-05
**Status**: Complete

---

## Executive Summary

This assessment evaluates the Silver layer design from a **multi-domain extensibility perspective**. While the current design is well-suited for the weather/air-quality domain, it contains domain-specific assumptions that would require refactoring for a truly generic N-domain platform.

### Key Findings

| Area | Current State | Extensibility Rating | Priority |
|------|---------------|---------------------|----------|
| Stream Configuration | Generic, well-designed | High | - |
| ETL Config Pattern | Generic with domain examples | High | - |
| Silver Schema | Domain-specific table names | Medium | Phase 2 |
| Unit System | Hardcoded SI for weather/AQ | Low | Phase 3 |
| Namespace/Tenancy | Single installation assumed | Low | Phase 3 |
| Metadata Catalog | Inline in config, no registry | Medium | Phase 2 |

### Recommendation

**Proceed with current design for weather/air-quality (Phase 1), then generalize incrementally (Phases 2-3) as new domains are added.** The config-driven patterns are already extensible; schema and unit systems can be refactored when concrete requirements emerge.

---

## Table of Contents

1. [What's Already Generic](#1-whats-already-generic)
2. [Domain-Specific Concerns](#2-domain-specific-concerns)
3. [Architectural Recommendations](#3-architectural-recommendations)
4. [Proposed Generic Schema Pattern](#4-proposed-generic-schema-pattern)
5. [Priority Matrix](#5-priority-matrix)

---

## 1. What's Already Generic

These patterns are well-designed and scale to N domains with minimal changes.

### 1.1 Stream Configuration Model

**Strength**: The `StreamConfig` structure is domain-agnostic.

```yaml
# Current pattern - fully generic
stream_id: "{domain}-{entity}"  # e.g., air-quality, energy-solar, health-heartrate
description: "..."
version: "1.0.0"
enabled: true
retention_days: N
partitioning_strategy: daily
fields:
  - name: metric_name
    type: float|int|string|bool|json
    unit: "..."
    nullable: true|false
    range: [min, max]
sources:
  - type: mqtt|http_poll|webhook|file_watch
    # ... source-specific config
```

**Why it works for N domains**:
- `stream_id` is a string identifier, not enum-constrained
- `fields` schema is declarative and type-agnostic
- `sources` array supports any source type via trait pattern
- etcd path `/streams/{id}/config` naturally namespaces by domain

**No changes needed** for domains like:
- `energy-solar`, `energy-consumption`, `energy-battery`
- `health-heartrate`, `health-sleep`, `health-activity`
- `home-motion`, `home-temperature`, `home-door-state`

### 1.2 Config-Driven ETL Pattern

**Strength**: The `silver_etl` config section from 06-refined-synthesis.md is generic.

```yaml
silver_etl:
  enabled: true
  target_table: silver.{domain}_{entity}  # Parameterized

  field_mappings:
    - source_path: raw_payload.{field}
      target_column: {column_name}
      type: {pg_type}
      transform:
        type: unit_conversion | formula | lookup | ...
      dq_rules:
        - rule: range_check | not_null | pattern | ...
```

**Why it works for N domains**:
- `field_mappings` are declarative, not hardcoded
- `transform` section supports pluggable transformation types
- `dq_rules` are configurable per-field
- SQL generation is template-based, not domain-specific

**Extensibility for new domains**:
```yaml
# Energy domain example
stream_id: energy-solar
silver_etl:
  target_table: silver.energy_generation
  field_mappings:
    - source_path: raw_payload.power_w
      target_column: power_watts
      type: double_precision
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 15000.0  # 15kW system max
```

### 1.3 Source Trait Abstraction

**Strength**: The Domain Adapter pattern with `Source` trait is fully generic.

```rust
#[async_trait]
pub trait Source: Send + Sync {
    async fn fetch(&self) -> Result<Vec<TimeSeriesPoint>, CoreError>;
    async fn health_check(&self) -> Result<HealthStatus, CoreError>;
}
```

**Why it works for N domains**:
- No domain-specific methods in trait
- `TimeSeriesPoint` is a generic container
- `ResponseParser` trait allows pluggable data transformations
- `ParserRegistry` maps parser names to implementations

**New domains only require**:
1. Implement `ResponseParser` for new data format
2. Create stream config YAML
3. Register parser in `ParserRegistry`

### 1.4 Parquet Bronze Storage

**Strength**: Bronze layer is completely domain-agnostic.

```
/data/{stream_id}/YYYY-MM-DD_readings.parquet
```

**Structure**:
- `timestamp`: i64 (Unix microseconds)
- `ndp_id`: string (unique point identifier)
- `stream_id`: string
- `raw_payload`: JSON (domain-specific)
- `context`: JSON (metadata)

**Why it works for N domains**:
- `raw_payload` is untyped JSON - any domain data fits
- Partitioning by date is universal
- No domain-specific columns at Bronze level

---

## 2. Domain-Specific Concerns

These elements would require refactoring for true multi-domain support.

### 2.1 Hardcoded Table Names in Silver Schema

**Issue**: Table names embed domain assumptions.

```sql
-- Current (domain-specific)
silver.air_quality_observations
silver.weather_observations
silver.weather_forecasts
silver.outdoor_air_quality
```

**Problem for N domains**:
- Adding energy domain would require `silver.energy_generation`, `silver.energy_consumption`
- No consistent naming pattern
- Analytics views reference specific table names

**Impact**: Medium - Requires schema design changes when adding domains.

### 2.2 Unit Standardization Table

**Issue**: SI unit mappings are hardcoded for weather/AQ.

From 03-data-dictionary.md:
```
| Measurement     | Silver Unit | Column Suffix |
|-----------------|-------------|---------------|
| Temperature     | Celsius     | _c            |
| Pressure        | Pascals     | _pa           |
| Wind Speed      | km/h        | _kmh          |
| PM Concentration| ug/m3       | (no suffix)   |
```

**Problem for N domains**:
- Energy domain needs: watts (W), kilowatt-hours (kWh), amperes (A), volts (V)
- Health domain needs: beats per minute (bpm), steps, calories (kcal)
- Financial domain needs: currency codes, decimal precision
- No generic unit registry

**Impact**: Low-Medium - Can be extended incrementally with lookup table.

### 2.3 Computed Fields with Domain Logic

**Issue**: Computed fields contain domain-specific formulas.

From 03-data-dictionary.md:
```sql
-- EPA AQI calculation (air quality specific)
CREATE FUNCTION calculate_aqi_pm25(pm25_value DOUBLE PRECISION)
-- Heat index calculation (weather specific)
CREATE FUNCTION calculate_heat_index(temp_c, humidity_pct)
-- Mold risk index (indoor air specific)
CREATE FUNCTION calculate_mold_risk(temp_c, humidity_pct)
```

**Problem for N domains**:
- Each domain will need its own computed fields
- No pattern for registering domain-specific functions
- Functions are created in migration scripts, not config-driven

**Impact**: Low - Standard database extension pattern, add as needed.

### 2.4 Analytics Views with Hardcoded Joins

**Issue**: Cross-stream views assume weather/AQ domain model.

From 03-data-dictionary.md:
```sql
CREATE VIEW analytics.indoor_outdoor_comparison AS
-- Hardcoded join between indoor, outdoor_aq, and weather tables
```

**Problem for N domains**:
- View assumes exactly these three data sources
- No pattern for domain-specific analytics views
- Cross-domain analysis not considered (e.g., energy + weather correlation)

**Impact**: Medium - Requires generic analytics pattern.

### 2.5 DQ Rules with Domain Thresholds

**Issue**: Data quality thresholds are weather/AQ specific.

From config examples:
```yaml
range: [-40, 85]    # Temperature in Celsius
range: [0, 100]     # Humidity percentage
range: [400, 10000] # CO2 in ppm
```

**Not a problem** - These are already in config, not code. New domains define their own ranges.

### 2.6 Grafana Dashboard Templates

**Issue**: Dashboard provisioning is domain-specific.

From 04-dashboard-integration.md:
- Indoor Air Quality Dashboard
- Outdoor Weather Conditions Dashboard
- Outdoor AQI Dashboard
- Indoor vs Outdoor Comparison

**Problem for N domains**:
- No generic dashboard template pattern
- Each domain needs custom dashboard design
- Alert thresholds hardcoded per domain

**Impact**: Low - Expected per-domain customization.

---

## 3. Architectural Recommendations

### 3.1 Schema Naming Convention (Phase 2)

**Recommendation**: Adopt hierarchical schema pattern.

```sql
-- Current (flat, domain-mixed)
silver.air_quality_observations
silver.weather_forecasts

-- Proposed (domain-namespaced)
silver_{domain}.{entity}

-- Examples:
silver_aq.observations           -- Indoor air quality
silver_aq.outdoor_observations   -- Outdoor air quality
silver_weather.observations      -- Weather actuals
silver_weather.forecasts         -- Weather predictions
silver_energy.generation         -- Solar/wind production
silver_energy.consumption        -- Grid/battery usage
silver_health.activities         -- Steps, exercise
silver_health.vitals             -- Heart rate, SpO2
```

**Benefits**:
- Clear domain separation
- PostgreSQL schema-level permissions
- Easier to reason about cross-domain queries
- Supports domain-specific retention policies

**Implementation**:
- Use PostgreSQL schemas (not just table prefixes)
- Each domain gets its own schema: `CREATE SCHEMA silver_energy;`
- Grafana datasource can filter by schema

### 3.2 Domain Configuration Layer (Phase 2)

**Recommendation**: Add domain-level configuration above streams.

```yaml
# config/domains/energy.yaml
domain_id: energy
description: "Energy monitoring domain"
version: "1.0.0"

# Domain-level defaults
defaults:
  retention_days: 365
  partitioning_strategy: daily

# Unit registry for this domain
unit_registry:
  power:
    base_unit: watts
    symbol: W
    conversions:
      kilowatts: { scale: 0.001 }
      horsepower: { scale: 0.00134 }
  energy:
    base_unit: watt_hours
    symbol: Wh
    conversions:
      kilowatt_hours: { scale: 0.001 }
      megawatt_hours: { scale: 0.000001 }

# Domain-specific computed fields
computed_fields:
  - name: daily_yield_kwh
    type: continuous_aggregate
    definition: "SUM(power_watts * interval_hours) / 1000"

# Streams in this domain
streams:
  - solar-production
  - grid-consumption
  - battery-state
```

**Benefits**:
- Centralized domain configuration
- Unit registry per domain
- Computed field templates
- Stream grouping

### 3.3 Generic Unit Registry (Phase 3)

**Recommendation**: Create extensible unit conversion system.

```yaml
# config/units/registry.yaml
unit_types:
  temperature:
    base: kelvin
    units:
      celsius: { offset: -273.15 }
      fahrenheit: { scale: 1.8, offset: -459.67 }

  power:
    base: watts
    units:
      kilowatts: { scale: 0.001 }
      milliwatts: { scale: 1000 }

  concentration:
    base: parts_per_million
    units:
      ppm: { scale: 1 }
      ppb: { scale: 1000 }
      ug_per_m3: { scale: varies }  # Depends on molecule

  rate:
    base: per_second
    units:
      per_minute: { scale: 60 }
      per_hour: { scale: 3600 }
      bpm: { scale: 60 }  # beats per minute
```

**Implementation**:
```rust
pub trait UnitConverter {
    fn to_base(&self, value: f64, from_unit: &str) -> Result<f64>;
    fn from_base(&self, value: f64, to_unit: &str) -> Result<f64>;
    fn convert(&self, value: f64, from: &str, to: &str) -> Result<f64>;
}
```

### 3.4 Metadata Catalog (Phase 2)

**Recommendation**: Create queryable metadata registry.

**Current State**: Metadata is embedded in stream config files.
- Field definitions in `config/base/streams/*/config.yaml`
- Entity schemas in `entity_schemas` section
- No central queryable catalog

**Proposed**: TimescaleDB metadata tables.

```sql
-- Domain registry
CREATE TABLE catalog.domains (
    domain_id TEXT PRIMARY KEY,
    description TEXT,
    version TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    config JSONB
);

-- Stream registry (mirrors etcd, queryable)
CREATE TABLE catalog.streams (
    stream_id TEXT PRIMARY KEY,
    domain_id TEXT REFERENCES catalog.domains,
    description TEXT,
    version TEXT,
    enabled BOOLEAN,
    config JSONB,
    CONSTRAINT fk_domain FOREIGN KEY (domain_id) REFERENCES catalog.domains(domain_id)
);

-- Field catalog (denormalized for query performance)
CREATE TABLE catalog.fields (
    stream_id TEXT REFERENCES catalog.streams,
    field_name TEXT,
    field_type TEXT,
    unit TEXT,
    description TEXT,
    nullable BOOLEAN,
    range_min DOUBLE PRECISION,
    range_max DOUBLE PRECISION,
    PRIMARY KEY (stream_id, field_name)
);

-- View for Grafana variable dropdowns
CREATE VIEW catalog.field_selector AS
SELECT
    s.domain_id,
    s.stream_id,
    f.field_name,
    f.unit,
    f.description
FROM catalog.streams s
JOIN catalog.fields f ON s.stream_id = f.stream_id
WHERE s.enabled = true;
```

**Sync pattern**: ConfigSyncService populates catalog tables from etcd/YAML.

### 3.5 Namespace/Tenancy Model (Phase 3)

**Current State**: Single `ndp_id` identifies data points, no multi-tenant support.

**Recommendation**: Add optional namespace prefix.

```yaml
# config/platform.yaml
namespace: "home"  # or "office", "rental-unit-1", etc.

# ndp_id format becomes:
# {namespace}:{stream_id}:{device_id}
# home:air-quality:aq_airgradient_1
```

**Implementation options**:

| Approach | Complexity | Use Case |
|----------|------------|----------|
| Namespace prefix in ndp_id | Low | Single installation, multiple contexts |
| Separate etcd prefixes | Medium | Shared infrastructure, tenant isolation |
| Separate databases | High | True multi-tenancy, full isolation |

**Recommendation for NDP**: Start with namespace prefix (low complexity).

### 3.6 Cross-Domain Analytics Pattern (Phase 3)

**Recommendation**: Generic correlation views.

```sql
-- Generic time-aligned cross-domain view
CREATE OR REPLACE FUNCTION analytics.cross_domain_aligned(
    p_domains TEXT[],           -- ['weather', 'energy']
    p_bucket_interval INTERVAL, -- '1 hour'
    p_start_time TIMESTAMPTZ,
    p_end_time TIMESTAMPTZ
) RETURNS TABLE (
    bucket TIMESTAMPTZ,
    domain TEXT,
    metrics JSONB
) AS $$
-- Dynamic query construction based on domain registry
$$;

-- Example usage
SELECT * FROM analytics.cross_domain_aligned(
    ARRAY['weather', 'energy'],
    '1 hour',
    NOW() - INTERVAL '7 days',
    NOW()
);
```

---

## 4. Proposed Generic Schema Pattern

### 4.1 Domain Organization

```
silver_{domain}/
  ├── {entity}_observations     -- Point-in-time measurements
  ├── {entity}_forecasts        -- Predictions (optional)
  ├── {entity}_aggregates       -- Continuous aggregates
  └── {entity}_metadata         -- Dimension tables
```

### 4.2 Example: Energy Domain

```sql
-- Schema creation
CREATE SCHEMA IF NOT EXISTS silver_energy;

-- Solar generation observations
CREATE TABLE silver_energy.solar_observations (
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    observation_time    TIMESTAMPTZ NOT NULL,
    ndp_id              TEXT NOT NULL,

    -- Device context
    device_serial       TEXT,
    inverter_model      TEXT,
    array_name          TEXT,           -- e.g., 'rooftop-south'

    -- Power metrics
    power_watts         DOUBLE PRECISION,
    voltage_v           DOUBLE PRECISION,
    current_a           DOUBLE PRECISION,

    -- Energy counters
    daily_yield_kwh     DOUBLE PRECISION,
    total_yield_kwh     DOUBLE PRECISION,

    -- Environmental
    panel_temp_c        DOUBLE PRECISION,
    ambient_temp_c      DOUBLE PRECISION,

    -- DQ transparency
    dq_flags            TEXT[],

    PRIMARY KEY (observation_time, ndp_id)
);

SELECT create_hypertable('silver_energy.solar_observations',
    'observation_time',
    chunk_time_interval => INTERVAL '1 day'
);
```

### 4.3 Example: Health Domain

```sql
CREATE SCHEMA IF NOT EXISTS silver_health;

-- Activity observations
CREATE TABLE silver_health.activity_observations (
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    observation_time    TIMESTAMPTZ NOT NULL,
    ndp_id              TEXT NOT NULL,

    -- Device context
    device_type         TEXT,           -- 'fitbit', 'apple_watch', 'garmin'
    user_id             TEXT,

    -- Activity metrics
    steps               INTEGER,
    distance_m          DOUBLE PRECISION,
    floors_climbed      INTEGER,
    active_minutes      INTEGER,
    calories_burned     DOUBLE PRECISION,

    -- Heart rate (if available)
    heart_rate_bpm      SMALLINT,
    heart_rate_zone     TEXT,           -- 'resting', 'fat_burn', 'cardio', 'peak'

    -- DQ transparency
    dq_flags            TEXT[],

    PRIMARY KEY (observation_time, ndp_id)
);
```

### 4.4 Config-Driven Schema Generation

**Future enhancement**: Generate DDL from stream config.

```yaml
# In stream config
silver_etl:
  target_schema: silver_energy
  target_table: solar_observations

  # Schema hints for DDL generation
  table_type: observations  # observations | forecasts | aggregates
  chunk_interval: 1 day

  # Generates: CREATE TABLE silver_energy.solar_observations ...
```

---

## 5. Priority Matrix

### Phase 1: Current Implementation (Now)

**Goal**: Ship Silver layer for weather/air-quality domain.

| Item | Action | Rationale |
|------|--------|-----------|
| Silver schema | Use proposed `silver.{entity}` names | Works for first domain |
| ETL config | Implement as designed | Already generic |
| Unit conversions | Hardcode weather/AQ units | Sufficient for first domain |
| Dashboards | Weather/AQ specific | Expected customization |

**Deliverables**:
- `silver.air_quality_observations`
- `silver.weather_observations`
- `silver.weather_forecasts`
- `silver.outdoor_air_quality`

### Phase 2: Domain Abstraction (When Adding 2nd Domain)

**Trigger**: Energy, health, or other domain is added.

| Item | Action | Effort |
|------|--------|--------|
| Schema naming | Migrate to `silver_{domain}.{entity}` | 4h |
| Domain config | Create `config/domains/` structure | 4h |
| Metadata catalog | Create `catalog.*` tables | 4h |
| Unit registry | Create config-driven unit system | 8h |

**Total Phase 2 Effort**: ~20 hours

### Phase 3: Full Genericity (When Scaling)

**Trigger**: Multiple installations, multi-tenant requirements, or 5+ domains.

| Item | Action | Effort |
|------|--------|--------|
| Namespace/tenancy | Implement namespace prefix | 8h |
| Cross-domain analytics | Generic correlation functions | 8h |
| Dashboard templates | Domain-agnostic panel generators | 12h |
| Schema generation | DDL from config | 16h |

**Total Phase 3 Effort**: ~44 hours

---

## Appendix A: Domain Examples

### A.1 Potential Future Domains

| Domain | Example Streams | Key Metrics |
|--------|----------------|-------------|
| **Energy** | solar-production, grid-consumption, battery-state | power_watts, energy_kwh, soc_pct |
| **Health** | heartrate, sleep, activity | bpm, sleep_stages, steps |
| **Home IoT** | motion, door-state, lighting | motion_detected, is_open, brightness_pct |
| **Financial** | portfolio, transactions | price, quantity, total_value |
| **Industrial** | vibration, pressure, flow | frequency_hz, pressure_psi, flow_lpm |
| **Vehicle** | location, fuel, diagnostics | lat, lon, fuel_level_pct, dtc_codes |

### A.2 Cross-Domain Analysis Examples

| Analysis | Domains | Purpose |
|----------|---------|---------|
| Solar vs Weather | energy + weather | Predict generation from cloud cover |
| AQ vs Energy | air_quality + energy | Optimize HVAC for air quality |
| Activity vs Sleep | health | Correlate exercise and sleep quality |
| Home Occupancy vs Energy | home + energy | Detect waste when unoccupied |

---

## Appendix B: Decision Log

| Decision | Chosen Option | Alternatives Considered | Date |
|----------|---------------|------------------------|------|
| Schema naming | `silver.{entity}` (Phase 1) | `silver_{domain}.{entity}` | 2026-01-05 |
| Unit system | Hardcode for weather/AQ | Generic registry | 2026-01-05 |
| Tenancy | Not implemented | Namespace prefix, separate DBs | 2026-01-05 |
| Catalog | Inline in config | Metadata tables | 2026-01-05 |

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-05 | NDP Architect | Initial assessment |

---

## References

1. 06-refined-synthesis.md - Config-driven ETL design
2. 03-data-dictionary.md - Schema definitions
3. 04-dashboard-integration.md - Grafana patterns
4. PLATFORM_ARCHITECTURE_OVERVIEW.md - Current architecture
5. Stream config examples - `/config/base/streams/*/config.yaml`
