# AIR-009: Source Identity and Context Configuration

## Overview

Implement the `ndp_id` and `context` configuration architecture across the NDP stack, enabling stable source identification and mutable context attributes that travel with every record.  The overall goal is to add the ability to add meaning to the data as it arrives.  

## Background

NDP needs a clear separation between:
- **Source identity** (`ndp_id`): Stable, NDP-assigned identifier that never changes
- **Source context** (`context`): Mutable attributes denormalized at write time

This enables:
- Querying all data from a source regardless of metadata changes
- Point-in-time accuracy (historical records retain their context at write time)
- Device mobility (sensors can move rooms without breaking data lineage)
- Flexible, domain-specific attributes without schema changes

## Design Decisions (from discussion)

### Naming
| Field | Purpose | Mutability |
|-------|---------|------------|
| `ndp_id` | Stable source identifier | Immutable |
| `context` | Attributes written with records | Mutable |

### Context Structure
```yaml
context:
  location:                    # Standardized schema
    coordinates: [lat, lon]    # Tuple (not flattened)
    type: indoor | outdoor     # Optional
    path: house/zone/room      # Hierarchical, flexible depth
  device_type: airgradient     # Domain-specific (dynamic keys)
  model: ONE-V9
  tags: [primary, calibrated]
```

Example config files have been prepared and stored in config/samples directory.

### Key Rules
1. `ndp_id` is outside `context:` (identity vs attributes)
2. Everything in `context:` gets written with every record
3. Config supports nesting; ingestion flattens (except `coordinates`)
4. All context fields are optional
5. Context keys are dynamic per stream (not hardcoded schema)

## Scope

### 1. Stream Configuration Updates

**Goal:** Add `ndp_id` and `context` to all active stream definitions.

**Streams to modify:**
- `air-quality` (MQTT)
- `nws-observations` (HTTP)
- `nws-forecast-hourly` (HTTP)
- `nws-gridpoints-forecast` (HTTP)
- `outdoor-air-quality` (HTTP)
- `outdoor-weather` (HTTP)

**Changes per stream:**
```yaml
sources:
  - type: mqtt
    ndp_id: <assigned-id>      # NEW: Stable identifier
    context:                    # NEW: Written with records
      location:
        coordinates: [lat, lon]
        type: indoor | outdoor
        path: <hierarchy>
      # ... domain-specific fields
```

### 2. Configuration Sync (etcd)

**Goal:** Ensure `ndp_id` and `context` are properly synced to etcd.

**Tasks:**
- Verify ConfigSyncService handles new fields
- Confirm etcd key structure for nested context
- Test round-trip: YAML → etcd → application config

**Expected etcd keys:**
```
/streams/{stream_id}/sources/0/ndp_id
/streams/{stream_id}/sources/0/context/location/coordinates
/streams/{stream_id}/sources/0/context/location/type
/streams/{stream_id}/sources/0/context/location/path
/streams/{stream_id}/sources/0/context/{dynamic_key}
```

### 3. Ingestion Pipeline Modifications

**Goal:** Write `ndp_id` and flattened `context` with every record.

**Tasks:**
- Modify `SourceConfig` struct to include `ndp_id` and `context`
- Update parsers to attach context to parsed records
- Implement context flattening logic:
  - Flatten nested keys with dot notation: `location.type`, `location.path`
  - Preserve `coordinates` as tuple/array (for geospatial queries)
  - Handle dynamic keys (not hardcoded)
- Update Bronze layer writer to include new fields

**Record structure (post-ingestion):**
```json
{
  "timestamp": "2024-01-15T10:30:00Z",
  "ndp_id": "airgradient-office-001",
  "location.coordinates": [29.958, -81.308],
  "location.type": "indoor",
  "location.path": "home/upstairs/office",
  "device_type": "airgradient",
  "model": "ONE-V9",
  "pm25": 12.5,
  "temperature": 22.3
}
```

### 4. Data Dictionary (TimescaleDB Silver Layer)

**Goal:** Reflect `ndp_id` and context fields in the data dictionary.

**Tasks:**
- Add `ndp_id` column to TimescaleDB tables
- Define context field schema in data dictionary
- Support dynamic context fields (JSONB or flattened columns) Leaning toward JSONB
- Update ETL from Bronze → Silver to map new fields
- Create indexes on `ndp_id` for efficient querying

**Schema considerations:**
```sql
-- Option A: Flattened columns
ALTER TABLE sensor_readings ADD COLUMN ndp_id TEXT NOT NULL;
ALTER TABLE sensor_readings ADD COLUMN location_type TEXT;
ALTER TABLE sensor_readings ADD COLUMN location_path TEXT;
ALTER TABLE sensor_readings ADD COLUMN location_coordinates POINT;

-- Option B: JSONB for flexible context
ALTER TABLE sensor_readings ADD COLUMN ndp_id TEXT NOT NULL;
ALTER TABLE sensor_readings ADD COLUMN context JSONB;

-- Index for source queries
CREATE INDEX idx_readings_ndp_id ON sensor_readings(ndp_id);
```

## Out of Scope

- Webhook source type (future)
- Context validation/schema enforcement
- UI for managing source context
- Migration of existing records (new records only)

## Success Criteria

1. All active streams have `ndp_id` and `context` in config
2. `ndp_id` and `context` visible in etcd after sync
3. Ingested records include `ndp_id` and flattened context
4. TimescaleDB data dictionary includes new fields
5. Can query: `SELECT * FROM readings WHERE ndp_id = 'x'`
6. Sample configs in `config/samples/` remain valid documentation

## Dependencies

- DP-003: MQTT Multi-Subscription Support (complete)
- Existing stream configurations
- ConfigSyncService
- Bronze/Silver layer infrastructure

## References

- Sample configs: `config/samples/mqtt_stream.yaml`, `config/samples/http_stream.yaml`
- Design discussion: Session notes on `ndp_id` vs `entity_id` naming
- Amazon event envelope pattern (context separation inspiration)
