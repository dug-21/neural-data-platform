# BUG-001: Pre-Transform Config Schema Mismatch

## Summary

The DP-007 pre-transform feature is non-functional due to a complete mismatch between
the YAML configuration format and the Rust struct definitions. The feature was partially
implemented with two incompatible approaches that were never reconciled.

## Status

- **Severity**: Critical (feature completely broken)
- **Discovered**: 2026-01-12
- **Root Cause**: Design divergence during implementation

## Symptoms

1. `nws-gridpoints-forecast` stream silently fails to load to Silver layer
2. No error messages - deserialization fails and `pre_transform` becomes `None`
3. ETL runs but produces no data (DuckDB can't process nested arrays)

## Root Cause Analysis

### Issue 1: YAML Config Schema Mismatch

**Current YAML** (`config/base/streams/nws-gridpoints-forecast/config.yaml:542-546`):
```yaml
pre_transform:
  enabled: true
  parser_type: column_oriented
  parser_config_ref: sources[0].parser
```

**Rust Struct Expects** (`core/src/config/silver_etl.rs:696-708`):
```yaml
pre_transform:
  transform_type:
    type: array_explosion
    metrics_base_path: properties
    timestamp_field: validTime
    value_field: value
    values_path: values
    metrics:
      - metric_path: temperature
        target_column: temperature_c
        type: double_precision
```

**Result**: Serde deserialization fails, `Option<PreTransformConfig>` becomes `None`.

### Issue 2: Integration Not Wired Up

In `apps/silver-etl/src/etl.rs:451-458`:
```rust
// TODO (dp-007): When pre_transform.rs is implemented, call apply_pre_transform here
let use_pre_transform = config.pre_transform.is_some();
if use_pre_transform {
    info!("Pre-transform enabled - will use pre_transformed temp table");
    // Future: self.apply_pre_transform_if_needed(&config, stream_id, bronze_path)?;
}
```

The code:
1. Checks if `pre_transform.is_some()`
2. Logs that pre-transform is enabled
3. **NEVER actually calls `apply_pre_transform()`** - it's commented out!
4. SQL generation uses `FROM pre_transformed` but the table is never created

### Issue 3: Two Incompatible Designs

| Component | Design Approach | Status |
|-----------|-----------------|--------|
| YAML Config | References `ColumnOrientedParser` via `parser_config_ref` | Incomplete |
| Rust Structs | New `ArrayExplosionConfig` with its own mappings | Complete but unused |
| `pre_transform.rs` | Uses `ColumnOrientedParser` directly | Working but never called |
| `etl.rs` | Should wire them together | TODO comment, not implemented |

### Issue 4: Incorrect Pattern in AgentDB

Pattern `arch-pre-transform-silver-etl` documents the WRONG config format:
```
CONFIG: pre_transform.enabled, pre_transform.parser_type (column_oriented),
pre_transform.metrics_base_path, pre_transform.columns[]
```

This matches the broken YAML but NOT the Rust structs.

## Affected Files

| File | Issue |
|------|-------|
| `config/base/streams/nws-gridpoints-forecast/config.yaml` | Wrong YAML format |
| `core/src/config/silver_etl.rs` | Struct doesn't match YAML |
| `apps/silver-etl/src/etl.rs` | Integration not wired up |
| `apps/silver-etl/src/pre_transform.rs` | Working but never called |
| AgentDB pattern `arch-pre-transform-silver-etl` | Documents wrong format |

## Decision: Which Design to Use

**Selected: Enum-based `PreTransformType` (Approach B)**

Rationale:
1. **Extensible**: Tagged enum allows adding new transform types easily
2. **Type-safe**: Each transform has strongly-typed config
3. **Self-documenting**: `type: array_explosion` explicit in YAML
4. **Standard pattern**: Idiomatic Rust approach for polymorphic configs
5. **Future-proof**: Can add `ColumnOriented`, `JsonFlatten`, etc. variants

The current `ArrayExplosionConfig` in the Rust structs is the correct pattern.
The YAML config and AgentDB pattern need to be updated to match.

## Related Documents

- `product/features/dp-007/SCOPE.md` - Original requirements
- `product/features/dp-007/architecture/ADR-001-PRE-TRANSFORM-DESIGN.md` - Design doc
- `core/src/config/silver_etl.rs` - Rust struct definitions (source of truth)
