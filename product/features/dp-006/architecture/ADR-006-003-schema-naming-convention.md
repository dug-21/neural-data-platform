# ADR-006-003: Schema Naming Convention

**Feature**: dp-006 (Silver Layer Implementation)
**Status**: Accepted
**Date**: 2026-01-10
**Author**: NDP Architect
**Supersedes**: None

---

## Context

The Silver layer needs a PostgreSQL schema structure to organize tables. NDP is designed as a generic data platform supporting multiple domains over time:

- **Phase 1 (Current)**: Weather and Air Quality domain
- **Phase 2 (Planned)**: Energy, Smart Home domains
- **Phase 3 (Future)**: Financial, Industrial domains

The naming convention must balance:
1. **Simplicity now** - Don't over-engineer for hypothetical future
2. **Extensibility later** - Don't paint into a corner
3. **Query ergonomics** - Easy to write and understand
4. **Multi-domain support** - Clear separation when needed

### Current Tables (Phase 1)

| Table | Source Streams |
|-------|----------------|
| air_quality_observations | air-quality |
| weather_observations | outdoor-weather, nws-observations |
| weather_forecasts | nws-forecast-hourly, nws-gridpoints-forecast |
| outdoor_air_quality | outdoor-air-quality |

---

## Decision

**Use flat `silver.*` schema for Phase 1**, with documented migration path to domain-specific schemas.

```sql
-- Phase 1: All Silver tables in single schema
CREATE SCHEMA silver;

CREATE TABLE silver.air_quality_observations (...);
CREATE TABLE silver.weather_observations (...);
CREATE TABLE silver.weather_forecasts (...);
CREATE TABLE silver.outdoor_air_quality (...);
```

### Naming Convention

| Element | Convention | Example |
|---------|------------|---------|
| Schema | `silver` | `silver.` |
| Table | `{domain}_{entity_type}` | `weather_observations` |
| Entity types | `observations`, `events`, `forecasts`, `metrics` | - |

### Full Table Names

- `silver.air_quality_observations`
- `silver.weather_observations`
- `silver.weather_forecasts`
- `silver.outdoor_air_quality`

---

## Consequences

### Positive

1. **Simple queries** - No schema switching needed: `SELECT * FROM silver.weather_observations`
2. **Single connection** - Grafana needs one schema, not multiple
3. **Easy cross-domain joins** - All tables in same schema
4. **Low cognitive load** - Developers know where to look
5. **Sufficient for Phase 1** - 4 tables don't need complex organization

### Negative

1. **Future migration** - May need to reorganize if domains grow significantly
2. **Name collisions** - Must prefix with domain to avoid collisions
3. **No per-domain permissions** - Can't grant schema-level access by domain

### Neutral

1. **Migration path defined** - Can split to `silver_aq.*`, `silver_weather.*` later
2. **Config-driven** - `target_table` in config makes migration tractable

---

## Alternatives Considered

### Alternative 1: Domain Schemas from Start

**Description**: Create domain-specific schemas immediately.

```sql
CREATE SCHEMA silver_aq;      -- Air quality domain
CREATE SCHEMA silver_weather; -- Weather domain

CREATE TABLE silver_aq.observations (...);
CREATE TABLE silver_weather.observations (...);
CREATE TABLE silver_weather.forecasts (...);
```

| Factor | Flat silver.* | Domain Schemas |
|--------|---------------|----------------|
| Query complexity | Simple | Search path management |
| Cross-domain joins | Easy | Requires fully-qualified names |
| Future domains | Add prefix | Add new schema |
| Permissions | Coarse | Fine-grained by schema |
| Grafana setup | Single schema | Multiple datasources or search_path |

**Rejected because**: Over-engineering for 4 tables. Adds complexity to Grafana dashboards. Can migrate later if needed.

### Alternative 2: Hierarchical Naming

**Description**: Use `domain_entity_type_version` pattern.

```sql
CREATE TABLE silver.aq_observations_v1 (...);
CREATE TABLE silver.weather_observations_v1 (...);
CREATE TABLE silver.weather_forecasts_v1 (...);
```

**Rejected because**: Version suffix in table names is awkward. Schema versioning should be metadata, not table names.

### Alternative 3: Entity-Centric Naming

**Description**: Single table per entity type across domains.

```sql
CREATE TABLE silver.observations (...);  -- All domains
CREATE TABLE silver.forecasts (...);     -- All domains
```

**Rejected because**: Different domains have different columns. Would require sparse columns or JSONB overflow. Reduces query performance.

---

## Migration Path

When Phase 2 domains are added, evaluate whether to:

### Option A: Keep Flat (Preferred)

If total tables < 20, continue with prefixed naming:

```sql
silver.aq_observations
silver.weather_observations
silver.energy_readings
silver.home_events
```

### Option B: Split to Domain Schemas

If domains grow complex or need isolation:

```sql
-- Migration script
CREATE SCHEMA silver_aq;
CREATE SCHEMA silver_weather;
CREATE SCHEMA silver_energy;

-- Move tables (with zero-downtime views)
ALTER TABLE silver.air_quality_observations
  SET SCHEMA silver_aq;

ALTER TABLE silver_aq.air_quality_observations
  RENAME TO observations;

-- Backward-compatible view
CREATE VIEW silver.air_quality_observations AS
  SELECT * FROM silver_aq.observations;
```

### Migration Triggers

Consider migration when:
- Total Silver tables > 15
- Need domain-specific permissions
- Domain teams want isolation
- Query performance needs optimization by domain

---

## Table Naming Details

### Observations Pattern

Time-series measurements from sensors/APIs:

```sql
CREATE TABLE silver.{domain}_observations (
    observation_time  TIMESTAMPTZ NOT NULL,
    ingestion_time    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ndp_id            TEXT NOT NULL,
    -- domain-specific measurement columns
    -- ...
    dq_flags          TEXT[],
    PRIMARY KEY (observation_time, ndp_id)
);
```

### Forecasts Pattern

Future predictions with valid time windows:

```sql
CREATE TABLE silver.{domain}_forecasts (
    issue_time        TIMESTAMPTZ NOT NULL,
    valid_time        TIMESTAMPTZ NOT NULL,
    ingestion_time    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ndp_id            TEXT NOT NULL,
    -- domain-specific forecast columns
    -- ...
    dq_flags          TEXT[],
    PRIMARY KEY (issue_time, valid_time, ndp_id)
);
```

### Events Pattern (Future)

Discrete state changes:

```sql
CREATE TABLE silver.{domain}_events (
    event_time        TIMESTAMPTZ NOT NULL,
    ingestion_time    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ndp_id            TEXT NOT NULL,
    event_type        TEXT NOT NULL,
    -- event-specific columns
    -- ...
    dq_flags          TEXT[],
    PRIMARY KEY (event_time, ndp_id, event_type)
);
```

---

## Column Naming Convention

### Standard Columns

| Column | Type | Purpose |
|--------|------|---------|
| `observation_time` | TIMESTAMPTZ | When measurement taken |
| `ingestion_time` | TIMESTAMPTZ | When inserted to Silver |
| `ndp_id` | TEXT | Platform source identifier |
| `source_provider` | TEXT | Data provider (nws, owm) |
| `dq_flags` | TEXT[] | Data quality violation flags |

### Measurement Columns

Unit suffix pattern from `analytics-silver-data-types`:

| Suffix | Unit | Example |
|--------|------|---------|
| `_c` | Celsius | `temperature_c` |
| `_pct` | Percent | `humidity_pct` |
| `_pa` | Pascals | `pressure_pa` |
| `_kmh` | km/h | `wind_speed_kmh` |
| `_deg` | Degrees | `wind_direction_deg` |
| `_m` | Meters | `visibility_m` |
| `_mm` | Millimeters | `precipitation_mm` |

---

## References

1. Pattern: `analytics-silver-data-types` - Column naming conventions
2. Pattern: `arch-silver-schema` - Hypertable design
3. Research: `research/agenticdataplatform/silver/03-data-dictionary.md`
4. Research: `research/agenticdataplatform/silver/08-platform-architecture-assessment.md`

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-10 | NDP Architect | Initial decision |
