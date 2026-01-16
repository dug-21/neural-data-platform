# ADR-009-002: Silver ETL Config Schema Extension

**Feature**: dp-009 (Config-Driven Silver Layer Data Dictionary)
**Status**: Proposed
**Date**: 2026-01-16
**Author**: NDP Architect
**Depends On**: dp-006 (Silver Layer), ADR-009-001 (Silver Dictionary Tables)

---

## Context

The Silver data dictionary (ADR-009-001) requires metadata for Silver columns:
- **Unit**: Measurement unit (celsius, ug/m3, percent)
- **Description**: Human-readable column purpose

Currently, `silver_etl.field_mappings` in stream configs contains:
- `source_path`: Bronze JSON path
- `target_column`: Silver column name
- `type`: PostgreSQL data type
- `nullable`: NULL allowed
- `dq_rules[]`: Data quality rules

### Current Config Example (air-quality)

```yaml
silver_etl:
  enabled: true
  target_table: silver.air_quality_observations

  field_mappings:
    - source_path: raw_payload.pm02Compensated
      target_column: pm25
      type: double_precision
      nullable: false
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 1000.0
          action: flag
```

### Gap

To populate `silver_columns.unit` and `silver_columns.description`, we need these fields in the config.

### Options

1. **Extend field_mappings** with `unit` and `description` (proposed)
2. **Derive from Bronze entity_schemas** (lookup by mapping)
3. **Separate documentation file** (out-of-band metadata)

---

## Decision

**Add optional `unit` and `description` fields to `silver_etl.field_mappings` entries, and table-level metadata to the `silver_etl` block.**

### Extended Config Schema

```yaml
silver_etl:
  enabled: true
  target_table: silver.air_quality_observations

  # NEW: Table-level documentation
  description: "Indoor air quality measurements from AirGradient sensors"
  grain: "One row per sensor reading (~1 minute intervals)"

  field_mappings:
    - source_path: raw_payload.pm02Compensated
      target_column: pm25
      type: double_precision
      nullable: false
      # NEW: Column-level documentation
      unit: "ug/m3"
      description: "PM2.5 particulate matter concentration (compensated)"
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 1000.0
          action: flag
```

### New Fields

#### Table-Level (in `silver_etl` block)

| Field | Type | Required | Default | Purpose |
|-------|------|----------|---------|---------|
| `description` | string | No | NULL | Table description for data dictionary |
| `grain` | string | No | NULL | What one row represents |

#### Column-Level (in `field_mappings[]` entries)

| Field | Type | Required | Default | Purpose |
|-------|------|----------|---------|---------|
| `unit` | string | No | NULL | Measurement unit |
| `description` | string | No | NULL | Column description |

### Backward Compatibility

All new fields are **optional**:
- Existing configs without these fields will continue to work
- Sync will use NULL for missing values
- No breaking changes to silver-etl binary or config parsing

### Validation Rules

1. `unit` should follow established conventions:
   - Temperature: `celsius`, `fahrenheit`, `kelvin`
   - Pressure: `pa`, `hpa`, `mbar`
   - Concentration: `ug/m3`, `ppm`, `ppb`
   - Percentage: `percent`, `pct`
   - Length: `m`, `km`, `mm`
   - Speed: `m/s`, `km/h`, `mph`

2. `description` should be concise (< 200 chars)

3. `grain` should describe the row granularity

---

## Rationale

### Why Extend field_mappings

**Considered Alternative 1**: Derive unit from Bronze entity_schemas.

```yaml
# Bronze has unit in entity_schemas
entity_schemas:
  - schema_name: airgradient
    attributes:
      - name: pm25
        unit: ug/m3  # Could lookup from here
```

**Rejected because**:
1. Bronze and Silver column names may differ (pm02Compensated → pm25)
2. Transforms change units (kelvin_to_celsius)
3. Lookup adds complexity to sync mechanism
4. Silver should be self-documenting

**Considered Alternative 2**: Separate documentation file.

```yaml
# silver-dictionary.yaml
silver.air_quality_observations:
  columns:
    pm25:
      unit: ug/m3
      description: PM2.5 concentration
```

**Rejected because**:
1. Splits documentation from implementation
2. Risk of drift between files
3. Additional file to maintain
4. Violates single-source-of-truth principle

### Why Optional Fields

Making fields required would:
1. Break all existing stream configs
2. Block deployment until all configs updated
3. Force documentation before feature is useful

Optional fields enable:
1. Gradual adoption
2. Partial documentation (some columns documented, others not)
3. Non-breaking upgrade path

### Why Not Version the Config Schema

**Considered**: Add `silver_etl.version: "2.0"` to indicate new schema.

**Rejected because**:
1. New fields are purely additive
2. No structural changes requiring version bump
3. Existing code ignores unknown fields (forward compatible)
4. Adds complexity without benefit

---

## Consequences

### Positive

1. **Single Source of Truth**: Column metadata lives with column mapping
2. **Self-Documenting Config**: Complete information in one place
3. **Backward Compatible**: Existing configs work unchanged
4. **Gradual Adoption**: Teams can add documentation incrementally
5. **Transform-Aware**: Silver units can differ from Bronze

### Negative

1. **Config Verbosity**: Each mapping grows by 2-3 lines
2. **Manual Entry**: Units/descriptions must be typed (no auto-derive)
3. **Potential Inconsistency**: Optional fields may be missing

### Neutral

1. **No Code Changes Required**: Parser ignores unknown fields
2. **Sync Must Handle NULL**: Missing fields become NULL in database

---

## Example: Complete Stream Config

```yaml
stream_id: "air-quality"
description: "AirGradient sensor readings from MQTT"
version: "1.0.0"
enabled: true

# ... fields, sources, entity_schemas ...

silver_etl:
  enabled: true
  target_table: silver.air_quality_observations
  description: "Indoor air quality measurements from AirGradient sensors"
  grain: "One row per sensor reading (~1 minute intervals)"

  timestamp:
    source_field: timestamp
    target_field: observation_time
    transform: microseconds_to_timestamp

  field_mappings:
    - source_path: raw_payload.pm02Compensated
      target_column: pm25
      type: double_precision
      nullable: false
      unit: "ug/m3"
      description: "PM2.5 particulate matter concentration (compensated)"
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 1000.0
          action: flag

    - source_path: raw_payload.atmpCompensated
      target_column: temperature_c
      type: double_precision
      nullable: true
      unit: "celsius"
      description: "Ambient temperature (compensated for sensor heat)"
      dq_rules:
        - rule: range_check
          min: -40.0
          max: 85.0
          action: flag

    - source_path: raw_payload.rhumCompensated
      target_column: humidity_pct
      type: double_precision
      nullable: true
      unit: "percent"
      description: "Relative humidity (compensated)"
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 100.0
          action: clamp
          clamp_to_bounds: true
```

---

## Migration Plan

### Phase 1: Schema Ready (dp-009)

1. Update sync mechanism to parse new fields
2. Populate NULL for missing values
3. No config changes required

### Phase 2: Documentation Sprint

Update stream configs with metadata:

| Stream | Priority | Reason |
|--------|----------|--------|
| air-quality | High | Most queried, health-relevant |
| outdoor-weather | High | Weather dashboards |
| nws-observations | Medium | Supplements OWM |
| outdoor-air-quality | Medium | AQI calculations |
| nws-gridpoints-forecast | Low | Forecast-specific |

### Phase 3: Validation (Future)

Consider adding CI validation:
- Warn if unit missing for numeric columns
- Suggest unit based on column name pattern (_c → celsius, _pct → percent)

---

## Alternatives Considered

### Alternative 1: Centralized Unit Registry

```yaml
# units.yaml
units:
  temperature:
    - name: celsius
      symbol: C
      si: true
  concentration:
    - name: ug_m3
      symbol: ug/m3
```

**Pros**:
- Standardized unit names
- Could validate against registry

**Cons**:
- Additional file to maintain
- Over-engineering for current needs
- Still need unit reference in field_mapping

**Verdict**: Deferred - can add registry later if inconsistency becomes problem

### Alternative 2: Infer from Column Name

```python
# Pseudocode
if column.endswith('_c'):
    unit = 'celsius'
elif column.endswith('_pct'):
    unit = 'percent'
```

**Pros**:
- No config changes needed
- Enforces naming convention

**Cons**:
- Column names don't capture all unit info
- Not all columns follow pattern
- Loses precision (celsius vs kelvin)

**Verdict**: Rejected - too fragile

### Alternative 3: JSON Schema Validation

Define formal JSON Schema for silver_etl:

```json
{
  "type": "object",
  "properties": {
    "field_mappings": {
      "items": {
        "properties": {
          "unit": {"type": "string"},
          "description": {"type": "string"}
        }
      }
    }
  }
}
```

**Pros**:
- Formal validation
- IDE autocomplete

**Cons**:
- Additional tooling required
- Complexity for current scale

**Verdict**: Deferred - consider when config complexity grows

---

## Related Decisions

- **ADR-009-001**: Silver Dictionary Tables - Target schema for metadata
- **ADR-009-003**: Sync Mechanism Extension - How to parse new fields
- **ADR-006-003 (dp-006)**: Schema Naming - Column naming conventions with units

---

## References

1. [air-quality config.yaml](../../../../config/base/streams/air-quality/config.yaml) - Current config
2. [outdoor-weather config.yaml](../../../../config/base/streams/outdoor-weather/config.yaml) - Weather example
3. [ADR-006-003](../../dp-006/architecture/ADR-006-003-schema-naming-convention.md) - Unit suffix conventions

---

**Last Updated**: 2026-01-16
**Next Review**: After first stream config updated with metadata
