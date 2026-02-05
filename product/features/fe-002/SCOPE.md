# FE-002: Domain Configuration Standardization

> **Status:** Scoped
> **Priority:** High (Architecture Consistency)
> **Estimated Effort:** 2-3 days (19-26 hours)
> **Risk Level:** Low
> **Blocks:** V1.2 Pattern Detection Engine planning

---

## Summary

Standardize domain configuration to follow established NDP architecture patterns:
1. **Migrate domain configs from YAML to JSON** (ADR-016-001 compliance)
2. **Add Layer 1 JSON Schema validation** (dp-019 two-layer validation compliance)

This feature resolves two architecture gaps identified during FE-001 Phase D validation:
- [GitHub Issue #11](https://github.com/dug-21/neural-data-platform/issues/11) - GAP-001: Domain config uses YAML instead of JSON
- [GitHub Issue #13](https://github.com/dug-21/neural-data-platform/issues/13) - GAP-003: No JSON Schema validation for domain configs

---

## Context

### Problem Statement

During FE-001 Phase D Fast-Follower testing, two architecture violations were identified:

| Gap | Current State | Expected State | Impact |
|-----|---------------|----------------|--------|
| **GAP-001** | `domain.yaml` + `serde_yaml` | `domain.json` + `serde_json` | Inconsistent tooling, no schema validation |
| **GAP-003** | Rust deserialization only | Layer 1 (Schema) + Layer 2 (Semantic) | Poor error messages, no IDE support |

### Evidence

**GAP-001 (ADR-016-001 Violation):**
```rust
// tools/ndp-gold-ddl/src/config/loader.rs:46-47
fn domain_config_path(&self, domain_id: &str) -> PathBuf {
    self.config_dir.join("domains").join(domain_id).join("domain.yaml")  // ❌ Should be .json
}

// tools/ndp-gold-ddl/src/config/loader.rs:80
let config: DomainConfig = serde_yaml::from_str(&content)  // ❌ Should be serde_json
```

**GAP-003 (dp-019 Violation):**
```
Stream Validation:    Layer 1 (Schema) ✅ + Layer 2 (Semantic) ✅
Domain Validation:    Layer 1 (Schema) ❌ + Layer 2 (Semantic) ✅

Note: domain.schema.json EXISTS but is not integrated into validation pipeline
```

**GAP-004 (Schema Format Inconsistency - Discovered during FE-002 planning):**

| Component | Format | Evidence |
|-----------|--------|----------|
| Stream configs | FLAT | `{"stream_id": "...", ...}` |
| Stream schema | FLAT | `"required": ["stream_id", ...]` |
| Domain YAML | FLAT | `id: indoor-air-quality` (no wrapper) |
| Domain Rust struct | FLAT | `pub struct DomainConfig { pub id, ... }` |
| Domain schema | **WRAPPED** ❌ | `"required": ["domain"]` |
| Semantic validator | **WRAPPED** ❌ | `domain_config.get("domain")` |

**Design Principle:** All NDP configs should use consistent FLAT format. The domain schema was designed with an unnecessary wrapper that doesn't match stream configs or the Rust struct.

### Architecture References

- **ADR-016-001**: "JSON files are the primary source of truth. JSON is the platform-wide configuration standard."
- **dp-019**: "Two-layer validation: Layer 1 (JSON Schema) for structural, Layer 2 (Rust) for semantic"

### Design Principle: FLAT Format Consistency

All NDP configuration files use **FLAT format** (no wrapper objects):

```json
// ✅ CORRECT: Stream config (flat)
{ "stream_id": "air-quality", "fields": [...] }

// ✅ CORRECT: Domain config (flat)
{ "id": "indoor-air-quality", "streams": [...] }

// ❌ WRONG: Wrapped format
{ "domain": { "id": "indoor-air-quality", ... } }
```

This ensures:
- Consistent parsing patterns across all config types
- Simpler Rust struct definitions (no wrapper types)
- Uniform JSONPath expressions in error messages (`$.streams[0]` not `$.domain.streams[0]`)

---

## Scope

### In Scope

#### Phase A: YAML to JSON Migration (GAP-001)
- Convert `config/domains/indoor-air-quality/domain.yaml` to `domain.json`
- Update `ndp-gold-ddl` loader to use JSON path and parser
- Update inline test fixtures from YAML to JSON
- Remove `serde_yaml` dependency from ndp-gold-ddl

#### Phase B0: Schema Format Standardization (GAP-004 - Foundational)
- Fix `domain.schema.json` to use FLAT format (match stream configs)
- Fix `semantic/domain.rs` to expect FLAT format (remove wrapper expectation)
- **Must complete before validation wiring** - defines target format

#### Phase B: Schema Validation Integration (GAP-003)
- Add `--domain` flag to `ndp-validate` CLI
- Wire `domain.schema.json` into Layer 1 validation
- Connect existing semantic validation to CLI flow
- Add domain validation to `deploy.sh` workflow

### Out of Scope

- New domain configurations (future features will add domains)
- Stream configuration changes (already compliant)

---

## Dependencies

### Sequential Execution Required

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Phase A        │     │  Phase B0       │     │  Phase B        │
│  YAML → JSON    │────►│  Schema Format  │────►│  Validation     │
│  (GAP-001)      │     │  (GAP-004)      │     │  (GAP-003)      │
└─────────────────┘     └─────────────────┘     └─────────────────┘
        │                       │                       │
        └───────────────────────┴───────────────────────┘
                    Principle: FLAT format consistency
```

**Why sequential:**
1. **Phase A first:** JSON files must exist before schema validation
2. **Phase B0 before B:** Schema format defines what validation expects
3. **Consistency principle:** All NDP configs use FLAT format (no wrappers)

### External Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| ADR-016-001 | ✅ Accepted | JSON source of truth |
| dp-019 Two-Layer Validation | ✅ Implemented | Pattern exists for streams |
| `domain.schema.json` | ⚠️ Format Fix Needed | Needs FLAT format (Phase B0) |
| `semantic/domain.rs` | ⚠️ Format Fix Needed | Needs FLAT format (Phase B0) |

---

## Implementation Plan

### Phase A: YAML to JSON Migration (4-6 hours)

| Step | Task | Files | Effort |
|------|------|-------|--------|
| A1 | Convert domain.yaml to domain.json | `config/domains/indoor-air-quality/` | 30 min |
| A2 | Update loader path extension | `loader.rs:46-47` | 5 min |
| A3 | Update parser to serde_json | `loader.rs:80` | 5 min |
| A4 | Convert test fixtures to JSON | `domain.rs` (3 tests) | 45 min |
| A5 | Remove serde_yaml dependency | `Cargo.toml` | 10 min |
| A6 | Run test suite, fix any issues | All | 60 min |
| A7 | Manual CLI validation | - | 20 min |

**Phase A Checkpoint:**
```bash
# All must pass before proceeding to Phase B
cargo test -p ndp-gold-ddl
ndp-gold-ddl generate --domain indoor-air-quality
jq . config/domains/indoor-air-quality/domain.json
```

### Phase B0: Schema Format Standardization (1-2 hours)

| Step | Task | Files | Effort |
|------|------|-------|--------|
| B0-1 | Fix domain.schema.json to FLAT format | `config/schemas/domain.schema.json` | 15 min |
| B0-2 | Fix semantic validator to expect FLAT | `semantic/domain.rs` | 30 min |
| B0-3 | Update semantic validator tests | `semantic/domain.rs` (tests) | 30 min |
| B0-4 | Verify schema validates domain.json | Manual test | 15 min |

**Phase B0 Checkpoint:**
```bash
# Schema should validate flat-format domain.json
cat config/domains/indoor-air-quality/domain.json | \
  npx ajv validate -s config/schemas/domain.schema.json -d -
```

### Phase B: Schema Validation Integration (13-18 hours)

| Step | Task | Files | Effort |
|------|------|-------|--------|
| B1 | Add `--domain` CLI flag | `cli.rs` | 2 hours |
| B2 | Add domain config type enum | `cli.rs` | 30 min |
| B3 | Load domain schema for Layer 1 | `schema.rs`, `main.rs` | 2 hours |
| B4 | Wire semantic validation to CLI | `semantic/mod.rs` | 1 hour |
| B5 | Add domain validation flow | `main.rs` | 2 hours |
| B6 | Write unit tests (30-40 tests) | `tests/` | 4 hours |
| B7 | Integration testing | - | 2 hours |
| B8 | Add to deploy.sh workflow | `deploy.sh` | 1 hour |

**Phase B Checkpoint:**
```bash
# All must pass
cargo test -p ndp-validate
ndp-validate config/domains/indoor-air-quality/domain.json
ndp-validate --all --domain
ndp-validate --schema-only config/domains/indoor-air-quality/domain.json
```

---

## Files Affected

### Phase A Files

| File | Change Type | Lines Changed |
|------|-------------|---------------|
| `config/domains/indoor-air-quality/domain.yaml` | Delete | -107 |
| `config/domains/indoor-air-quality/domain.json` | Create | +120 |
| `tools/ndp-gold-ddl/src/config/loader.rs` | Modify | 2 |
| `tools/ndp-gold-ddl/src/config/domain.rs` | Modify | ~30 (test strings) |
| `tools/ndp-gold-ddl/Cargo.toml` | Modify | -1 (remove serde_yaml) |

### Phase B0 Files (Schema Format Fix)

| File | Change Type | Lines Changed |
|------|-------------|---------------|
| `config/schemas/domain.schema.json` | Modify | ~10 (remove wrapper) |
| `tools/ndp-validate/src/semantic/domain.rs` | Modify | ~20 (flat format) |

### Phase B Files

| File | Change Type | Lines Changed |
|------|-------------|---------------|
| `tools/ndp-validate/src/cli.rs` | Modify | +80-120 |
| `tools/ndp-validate/src/main.rs` | Modify | +50-80 |
| `tools/ndp-validate/src/schema.rs` | Modify | +30-50 |
| `tools/ndp-validate/src/semantic/mod.rs` | Modify | +15-25 |
| `tools/ndp-validate/src/lib.rs` | Modify | +2-5 |
| `deploy/pi/deploy.sh` | Modify | +10-20 |

---

## Acceptance Criteria

### Phase A: YAML to JSON Migration

- [ ] **AC-A1**: `domain.json` exists at `config/domains/indoor-air-quality/domain.json`
- [ ] **AC-A2**: `domain.json` is valid JSON (verified by `jq .`)
- [ ] **AC-A3**: `domain.yaml` has been deleted
- [ ] **AC-A4**: `ndp-gold-ddl` loads domain config via `serde_json`
- [ ] **AC-A5**: `cargo test -p ndp-gold-ddl` passes (all tests)
- [ ] **AC-A6**: `ndp-gold-ddl generate --domain indoor-air-quality` works
- [ ] **AC-A7**: No `serde_yaml` references remain in ndp-gold-ddl

### Phase B0: Schema Format Standardization

- [ ] **AC-B0-1**: `domain.schema.json` uses FLAT format (no `"domain"` wrapper)
- [ ] **AC-B0-2**: Schema validates flat-format `domain.json` successfully
- [ ] **AC-B0-3**: `semantic/domain.rs` expects FLAT format (no `.get("domain")` call)
- [ ] **AC-B0-4**: All existing semantic validation tests pass with FLAT format

### Phase B: Schema Validation Integration

- [ ] **AC-B1**: `ndp-validate --domain <path>` validates a single domain config
- [ ] **AC-B2**: `ndp-validate --all --domain` validates all domain configs
- [ ] **AC-B3**: Layer 1 errors show JSONPath locations (e.g., `$.streams[0].stream_id`)
- [ ] **AC-B4**: Layer 2 semantic validation runs after Layer 1 passes
- [ ] **AC-B5**: Invalid domain configs produce clear, actionable error messages
- [ ] **AC-B6**: `cargo test -p ndp-validate` passes (including 30-40 new tests)
- [ ] **AC-B7**: `deploy.sh` validates domain configs before deployment
- [ ] **AC-B8**: IDE autocomplete works for domain.json files (JSON Schema integration)

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Schema-struct mismatch | Low | Medium | Phase A testing validates JSON structure before Phase B |
| Test fixture conversion errors | Low | Low | Use jq for automated conversion, manual review |
| Existing workflow disruption | Low | Medium | No behavioral changes, format only |
| Parallel development conflict | N/A | N/A | Sequential execution eliminates this risk |
| Schema format change breaks existing code | Low | Low | Phase B0 runs before validation wiring; no production code uses domain schema yet |

---

## Success Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Domain config format | 100% JSON | No YAML files in `config/domains/` |
| Validation coverage | Two-layer | Both Layer 1 and Layer 2 for domains |
| Test coverage | >90% | New validation code paths covered |
| Developer experience | Clear errors | Schema errors show field paths and suggestions |

---

## Related Documents

- ADR-016-001: [Configuration Source of Truth](/product/features/dp-016/architecture/ADR-016-001-config-source-of-truth.md)
- dp-019: [Two-Layer Validation](/product/features/dp-019/architecture/ADR-019-001-two-layer-validation.md)
- Domain Schema: [domain.schema.json](/config/schemas/domain.schema.json)
- Semantic Validation: [domain.rs](/tools/ndp-validate/src/semantic/domain.rs)
- GitHub Issues: [#11](https://github.com/dug-21/neural-data-platform/issues/11), [#13](https://github.com/dug-21/neural-data-platform/issues/13)

---

## SPARC Phases

| Phase | Focus | Key Deliverables |
|-------|-------|------------------|
| **Specification** | This document | SCOPE.md, acceptance criteria |
| **Pseudocode** | Algorithm design | Config loading flow, validation pipeline |
| **Architecture** | Design decisions | None needed (reuses existing patterns) |
| **Refinement** | TDD implementation | Phase A + Phase B code changes |
| **Completion** | Deployment & verification | Release manifest, GitHub issue closure |

---

*Created: 2026-02-05*
*Feature Owner: Architecture Team*
*GitHub Issues: #11, #13*
