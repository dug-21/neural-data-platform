# FE-002 Specification Overview

> **Feature:** Domain Configuration Standardization
> **Version:** 1.0
> **Status:** Draft
> **Created:** 2026-02-05
> **Last Updated:** 2026-02-05

---

## Executive Summary

FE-002 standardizes domain configuration to follow established NDP architecture patterns, resolving two architecture gaps identified during FE-001 Phase D validation:

| Gap | Issue | Resolution |
|-----|-------|------------|
| **GAP-001** | Domain config uses YAML instead of JSON | Phase A: YAML to JSON Migration |
| **GAP-003** | No JSON Schema validation for domain configs | Phase B: Schema Validation Integration |

---

## Critical Success Factor

### The Testing Guarantee

**The #1 risk is that converting `domain.yaml` to `domain.json` could change how `ndp-gold-ddl` interprets the config, producing DIFFERENT DDL output. This MUST NOT happen.**

```
YAML Config --> ndp-gold-ddl --> DDL Output (baseline)
JSON Config --> ndp-gold-ddl --> DDL Output (must be IDENTICAL)
```

This guarantee is enforced through:
1. **Baseline capture** before any changes
2. **Automated comparison** after migration
3. **Byte-identical verification** of generated DDL

---

## Phase Dependencies

```
+-------------------+          +------------------------+
|   Phase A         |          |   Phase B              |
|   YAML to JSON    |--------->|   Schema Validation    |
|   Migration       |          |   Integration          |
|   (GAP-001)       |          |   (GAP-003)            |
+-------------------+          +------------------------+
        |                               |
        |  BLOCKING DEPENDENCY          |
        +-------------------------------+
              Phase A MUST complete
              before Phase B can begin
```

**Why Sequential:**
1. JSON Schema validates JSON format - cannot validate YAML files
2. Both phases modify `loader.rs` - parallel changes would conflict
3. Phase A enables Phase B (JSON files enable schema validation)

---

## Schema Format Decision

### Critical Discovery

During specification analysis, a format discrepancy was identified:

| Component | Current Format | Expected by Schema |
|-----------|---------------|-------------------|
| `domain.yaml` | Flat (no wrapper) | N/A |
| `domain.schema.json` | Wrapped (`"domain": { ... }`) | - |
| `DomainConfig` struct | Flat (no wrapper) | - |
| Semantic validator | Wrapped (`domain_config.get("domain")`) | - |

### Decision Required

Phase A must decide between two approaches:

**Option 1: Flat Format (Recommended)**
- Keep current flat format in JSON: `{ "id": "...", "streams": [...] }`
- Update semantic validator to handle flat format
- Update schema to not require wrapper (or create `domain-flat.schema.json`)
- Matches current Rust struct expectations

**Option 2: Wrapped Format**
- Add wrapper in JSON: `{ "domain": { "id": "...", "streams": [...] } }`
- Update `DomainConfig` struct to handle wrapper
- Schema already expects this format
- Semantic validator already expects this format

**Recommendation:** Option 1 (Flat Format) because:
1. Minimizes code changes (only schema/validator updates)
2. Maintains consistency with existing Rust struct
3. Simplifies conversion (direct YAML to JSON)

---

## Specification Documents

| Document | Phase | Purpose |
|----------|-------|---------|
| [SPEC-A01](./SPEC-A01-yaml-to-json-migration.md) | Phase A | YAML to JSON migration requirements |
| [SPEC-B01](./SPEC-B01-schema-validation-integration.md) | Phase B | Schema validation integration requirements |

---

## Acceptance Criteria Summary

### Phase A: YAML to JSON Migration

| ID | Criterion | Verification |
|----|-----------|--------------|
| AC-A1 | `domain.json` exists at expected path | Inspection |
| AC-A2 | `domain.json` is valid JSON | `jq .` passes |
| AC-A3 | `domain.yaml` deleted | Inspection |
| AC-A4 | `ndp-gold-ddl` uses `serde_json` | Code review |
| AC-A5 | All tests pass | `cargo test -p ndp-gold-ddl` |
| AC-A6 | CLI works | `ndp-gold-ddl generate --domain indoor-air-quality` |
| AC-A7 | No `serde_yaml` in ndp-gold-ddl | `grep serde_yaml` returns nothing |
| **AC-A8** | **DDL output identical** | **Baseline comparison** |

### Phase B: Schema Validation Integration

| ID | Criterion | Verification |
|----|-----------|--------------|
| AC-B1 | `--domain` flag validates single config | CLI test |
| AC-B2 | `--all --domain` validates all configs | CLI test |
| AC-B3 | Layer 1 errors show JSONPath | Error format test |
| AC-B4 | Layer 2 runs after Layer 1 passes | Integration test |
| AC-B5 | Invalid configs produce clear errors | Error message test |
| AC-B6 | All tests pass | `cargo test -p ndp-validate` |
| AC-B7 | deploy.sh validates domains | Integration test |
| AC-B8 | IDE autocomplete works | Manual verification |

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| DDL output changes after migration | Low | **Critical** | Baseline comparison, byte-identical verification |
| Schema-struct format mismatch | Medium | Medium | Document decision, update schema or struct |
| Test fixture conversion errors | Low | Low | Automated conversion with `jq`, manual review |
| Existing workflow disruption | Low | Medium | No behavioral changes, format only |

---

## Related Documents

- **SCOPE.md:** Feature scope and implementation plan
- **ADR-016-001:** Configuration Source of Truth
- **dp-019:** Two-Layer Validation Pattern
- **GitHub Issues:** [#11](https://github.com/dug-21/neural-data-platform/issues/11), [#13](https://github.com/dug-21/neural-data-platform/issues/13)

---

## Glossary

| Term | Definition |
|------|------------|
| Layer 1 Validation | JSON Schema structural validation |
| Layer 2 Validation | Rust semantic validation (business rules) |
| DDL | Data Definition Language (SQL statements) |
| Flat Format | JSON without `"domain": { }` wrapper |
| Wrapped Format | JSON with `"domain": { }` wrapper |
