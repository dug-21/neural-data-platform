# FE-002 Pseudocode Documentation

## Overview

This directory contains the algorithmic specifications for FE-002: Domain Configuration Standardization. These pseudocode documents bridge the SCOPE specification and the actual implementation.

**Feature:** FE-002 Domain Configuration Standardization
**Phase:** Pseudocode (SPARC P)
**Status:** Complete

---

## Algorithm Index

| Document | Purpose | Critical Path |
|----------|---------|---------------|
| [ALGO-001](./ALGO-yaml-json-conversion.md) | YAML to JSON conversion | Phase A |
| [ALGO-002](./ALGO-golden-master-validation.md) | Golden master testing | Phase A (Safety) |
| [ALGO-003](./ALGO-schema-validation-pipeline.md) | ndp-validate domain support | Phase B |
| [ALGO-004](./ALGO-deploy-integration.md) | deploy.sh integration | Phase B |

---

## Algorithm Summaries

### ALGO-001: YAML to JSON Conversion

**Purpose:** Convert `domain.yaml` to `domain.json` with semantic equivalence.

**Key Points:**
- Handles flat-to-wrapped format transformation
- Preserves YAML comments as description fields where appropriate
- Validates output against JSON Schema before writing
- Complexity: O(n * s) where n = config size, s = schema size

**Input:** `config/domains/indoor-air-quality/domain.yaml`
**Output:** `config/domains/indoor-air-quality/domain.json`

---

### ALGO-002: Golden Master Validation

**Purpose:** Guarantee ZERO behavioral change during migration.

**Key Points:**
- Captures baseline DDL from YAML config
- Performs conversion to JSON
- Captures new DDL from JSON config
- Compares outputs (hash + diff)
- Automatic rollback on mismatch

**Critical Invariant:**
```
BEFORE migration DDL == AFTER migration DDL
```

**Complexity:** O(g + n) typical, O(g + n + d^2) worst case

---

### ALGO-003: Schema Validation Pipeline

**Purpose:** Add domain validation to `ndp-validate` CLI.

**Key Points:**
- Three-layer validation (Syntax, Schema, Semantic)
- New CLI flags: `--domain`, `--all-domains`
- Reuses existing `semantic/domain.rs` implementation
- JSONPath error locations for all errors

**New CLI Usage:**
```bash
ndp-validate --domain config/domains/indoor-air-quality/domain.json
ndp-validate --all-domains
```

**Complexity:** O(n * s + m * k) where m = streams, k = available streams

---

### ALGO-004: Deploy Integration

**Purpose:** Block deployment of invalid domain configs.

**Key Points:**
- New `validate_domain_configs()` function in deploy.sh
- Cross-validation of domain stream references
- Fail-fast before any deployment changes
- `--no-validate` escape hatch for emergencies

**Integration Point:**
```bash
./deploy.sh  # Now validates domains before deploying
```

**Complexity:** O(d * (v + s)) where d = domains, v = validation time

---

## Dependency Graph

```
                    ┌────────────────────┐
                    │  ALGO-001          │
                    │  YAML→JSON Conv.   │
                    └─────────┬──────────┘
                              │
                              ▼
                    ┌────────────────────┐
                    │  ALGO-002          │
                    │  Golden Master     │
                    │  Validation        │
                    └─────────┬──────────┘
                              │
              ┌───────────────┴───────────────┐
              │                               │
              ▼                               ▼
    ┌────────────────────┐        ┌────────────────────┐
    │  ALGO-003          │        │  ALGO-004          │
    │  Schema Validation │        │  Deploy Integration│
    │  Pipeline          │        │                    │
    └────────────────────┘        └────────────────────┘
```

---

## Implementation Order

### Phase A (GAP-001 Resolution)

1. **ALGO-002** - Run golden master to capture baseline
2. **ALGO-001** - Convert domain.yaml to domain.json
3. **ALGO-002** - Validate conversion preserves DDL output
4. Update `loader.rs` to use JSON
5. **ALGO-002** - Final validation

### Phase B (GAP-003 Resolution)

1. **ALGO-003** - Implement CLI extension
2. **ALGO-003** - Wire domain schema loading
3. **ALGO-004** - Add validation to deploy.sh
4. Integration testing

---

## Test Coverage Requirements

Each algorithm specifies test scenarios:

| Algorithm | Unit Tests | Integration Tests | Manual Verification |
|-----------|------------|-------------------|---------------------|
| ALGO-001 | Field mapping | Full conversion | jq validation |
| ALGO-002 | Diff comparison | Golden master script | DDL inspection |
| ALGO-003 | Validation rules | CLI end-to-end | Error messages |
| ALGO-004 | Function calls | Deploy workflow | Pi deployment |

---

## Complexity Summary

| Algorithm | Time | Space | Dominant Factor |
|-----------|------|-------|-----------------|
| ALGO-001 | O(n * s) | O(n) | Schema validation |
| ALGO-002 | O(g + n) | O(d) | DDL generation |
| ALGO-003 | O(n * s + m * k) | O(n + e) | Stream discovery |
| ALGO-004 | O(d * v) | O(d + s) | Per-domain validation |

---

## Error Handling Summary

All algorithms follow consistent error handling:

1. **Fail Fast:** Stop on first critical error
2. **Rollback:** Automatic restoration on failure (ALGO-002)
3. **Actionable Messages:** Include JSONPath, suggestions
4. **Exit Codes:** 0 (success), 1 (validation error), 2 (system error)

---

## References

- [FE-002 SCOPE.md](../SCOPE.md) - Feature specification
- [dp-019 Two-Layer Validation](../../../../docs/architecture/dp-019/) - Validation pattern
- [ADR-016-001](../../../../docs/architecture/ADR-016-001/) - JSON as source of truth
- [domain.schema.json](/config/schemas/domain.schema.json) - JSON Schema
- [semantic/domain.rs](/tools/ndp-validate/src/semantic/domain.rs) - Existing validation
