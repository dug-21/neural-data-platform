# ADR-001: ndp_id Design

## Status

Proposed

## Date

2025-12-31

## Context

The Neural Data Platform ingests data from multiple sources (MQTT sensors, HTTP APIs, etc.). Currently, data is identified by:

1. **stream_id**: Identifies the data stream (e.g., "air-quality")
2. **location_id**: Extracted from the payload (e.g., sensor serial number)

This creates several problems:

### Problem 1: Unstable Identity

Sensors and API endpoints can change their identifiers:
- Device replacement keeps same physical location but gets new serial number
- API provider changes their station identifiers
- Firmware updates change device IDs

**Impact**: Historical data becomes fragmented. Querying "all readings from the office sensor" requires knowing every historical ID that sensor used.

### Problem 2: No Separation of Identity vs. Attributes

Current `location_id` conflates:
- **Who is sending data** (identity)
- **Where the data comes from** (location attribute)
- **What type of device** (device attribute)

**Impact**: Changing a device's room or updating its model requires changing how we identify it, breaking data lineage.

### Problem 3: No Platform-Owned Identifier

The platform relies entirely on external identifiers (sensor serial numbers, API station codes). NDP has no control over these identifiers.

**Impact**: NDP cannot guarantee identifier stability or uniqueness across different source types.

## Decision

**Introduce `ndp_id` as a stable, platform-owned identifier for each source instance.**

### Design Principles

1. **ndp_id is OUTSIDE context**: Identity is not an attribute
2. **ndp_id is IMMUTABLE**: Once assigned, never changes
3. **ndp_id is REQUIRED**: Every source must have one (for new records)
4. **ndp_id is UNIQUE**: No two sources share the same ndp_id

### Schema Location

```yaml
sources:
  - type: mqtt
    ndp_id: airgradient-office-001    # HERE: Top-level, outside context
    context:                           # Mutable attributes
      location:
        type: indoor
        path: home/upstairs/office
      device_type: airgradient
```

### Naming Convention

Format: `{device-type}-{location-hint}-{sequence}`

Examples:
- `airgradient-office-001`
- `owm-weather-home`
- `nws-observations-ksgj`
- `purpleair-outdoor-001`

Rules:
- Lowercase alphanumeric with hyphens
- 3-64 characters
- Must start with a letter
- Human-readable for debugging

### Storage

```sql
-- Bronze Layer (Parquet)
ndp_id STRING NOT NULL

-- Silver Layer (TimescaleDB)
ndp_id TEXT NOT NULL
CREATE INDEX idx_readings_ndp_id ON readings (ndp_id, time DESC);
```

### Query Patterns

```sql
-- All data from a specific source
SELECT * FROM readings WHERE ndp_id = 'airgradient-office-001';

-- Compare sources
SELECT ndp_id, AVG(pm25) as avg_pm25
FROM readings
WHERE time > NOW() - INTERVAL '24 hours'
GROUP BY ndp_id;

-- Source history (even after device replacement)
SELECT time, pm25, temperature
FROM readings
WHERE ndp_id = 'airgradient-office-001'
ORDER BY time DESC;
```

## Consequences

### Positive

1. **Stable Data Lineage**: All data from a source is linked by one identifier, regardless of device replacement or config changes

2. **Clean Separation**: Identity (ndp_id) is separate from attributes (context). Device can move rooms without breaking queries

3. **Platform Control**: NDP owns and manages identifiers, not dependent on external systems

4. **Predictable Queries**: `WHERE ndp_id = 'x'` always works, no need to track historical location_ids

5. **Multi-Source Correlation**: Easy to JOIN data across streams using stable identifiers

### Negative

1. **Configuration Required**: Each source must have ndp_id in config (cannot be auto-generated from payload)

2. **Naming Coordination**: Operators must ensure ndp_id uniqueness manually (no automated enforcement yet)

3. **Migration Effort**: Existing records lack ndp_id (backfill not in scope)

4. **Storage Overhead**: Additional column in every record (~30 bytes per record)

### Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Duplicate ndp_id assignment | Future: Add validation in ConfigSyncService |
| Poor naming choices | Documentation + naming convention enforcement |
| Forgetting to set ndp_id | Make field required in SourceConfig validation |
| Changing ndp_id accidentally | Log warnings, consider immutability check |

## Alternatives Considered

### Alternative 1: Auto-Generate from Payload

```yaml
# REJECTED: Auto-derive from sensor data
sources:
  - type: mqtt
    # ndp_id generated from first location_id seen
```

**Rejected because**:
- Violates immutability (payload can change)
- No platform control over identifier format
- Race condition on first message

### Alternative 2: UUID Generation

```yaml
# REJECTED: System-generated UUID
sources:
  - type: mqtt
    ndp_id: 550e8400-e29b-41d4-a716-446655440000
```

**Rejected because**:
- Not human-readable for debugging
- Hard to correlate with physical devices
- Operators cannot predict IDs for queries

### Alternative 3: ndp_id Inside Context

```yaml
# REJECTED: ndp_id as context attribute
sources:
  - type: mqtt
    context:
      ndp_id: airgradient-office-001   # Inside context
      location: ...
```

**Rejected because**:
- Conflates identity with attributes
- Context is mutable, ndp_id should not be
- Semantic confusion

### Alternative 4: Composite Key

```yaml
# REJECTED: Use stream_id + index as identifier
# Identity would be: air-quality/source/0
```

**Rejected because**:
- Breaks if source order changes in config
- Not stable across config refactoring
- Confusing semantics

## Implementation Impact

### Files Modified

| File | Change |
|------|--------|
| `core/src/types/stream_config.rs` | Add `ndp_id: Option<String>` to SourceConfig |
| `core/src/types/stream_record.rs` | Add ndp_id to RecordMetadata |
| `core/src/sources/mqtt.rs` | Attach ndp_id to created points |
| `core/src/sources/http_poll.rs` | Attach ndp_id to created points |
| `core/src/storage/parquet.rs` | Add ndp_id column to schema |
| `config/base/streams/*/config.yaml` | Add ndp_id to all sources |

### API Changes

```rust
// Before
pub struct SourceConfig {
    pub source_type: SourceType,
    pub enabled: bool,
    pub params: HashMap<String, Value>,
}

// After
pub struct SourceConfig {
    pub source_type: SourceType,
    pub enabled: bool,
    pub ndp_id: Option<String>,        // NEW
    pub context: Option<Value>,         // NEW (see ADR-002)
    pub params: HashMap<String, Value>,
}
```

### Validation Rules

```rust
impl SourceConfig {
    pub fn validate(&self) -> Result<(), StreamConfigError> {
        // ndp_id validation
        if let Some(ref ndp_id) = self.ndp_id {
            if !is_valid_ndp_id(ndp_id) {
                return Err(StreamConfigError::InvalidNdpId(ndp_id.clone()));
            }
        }
        Ok(())
    }
}

fn is_valid_ndp_id(id: &str) -> bool {
    let len = id.len();
    if len < 3 || len > 64 { return false; }
    if !id.chars().next().map_or(false, |c| c.is_ascii_lowercase()) { return false; }
    id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}
```

## Related Decisions

- [ADR-002: Context Flattening Approach](./ADR-002-context-flattening.md)
- [ADR-003: Silver Layer Schema Choice](./ADR-003-silver-layer-schema.md)

## References

- [SCOPE.md](../SCOPE.md) - Feature requirements
- [Amazon Event Envelope Pattern](https://docs.aws.amazon.com/eventbridge/latest/userguide/aws-events.html) - Inspiration for context separation
- [Data Lineage Best Practices](https://www.dataversity.net/data-lineage/) - Background on stable identifiers
