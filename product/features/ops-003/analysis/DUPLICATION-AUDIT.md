# ops-003 Analysis: Code Duplication Audit

> **Date**: 2026-02-06
> **Purpose**: Catalog all duplication across the three deployment tools

---

## Critical Duplications

### 1. DbClient Trait (2 definitions)

| Location | Methods | Used By |
|----------|---------|---------|
| `crates/ndp-lib/src/db.rs` | `query()`, `execute()`, `batch_execute()` | ndp-cli |
| `tools/ndp-gold-ddl/src/db/client.rs` | `query()` only | ndp-gold-ddl |

ndp-gold-ddl's version is simpler (query-only). ndp-lib's is richer. The ops-001 SCOPE explicitly planned this extraction but ndp-gold-ddl was never migrated to consume ndp-lib's trait.

### 2. ConfigLoader Trait (2 definitions)

| Location | Methods | Config Types |
|----------|---------|-------------|
| `crates/ndp-lib/src/config.rs` | `load_stream_configs()`, `load_dimension_config()`, `load_domain_configs()` | ndp-lib's own types |
| `tools/ndp-gold-ddl/src/config/loader.rs` | `load_stream_config()`, `load_domain_config()` | ndp-gold-ddl's own types |

Both load from `config/base/streams/{id}/config.json` but deserialize into completely different Rust structs.

### 3. PostgresClient (2 implementations)

| Location | Features |
|----------|----------|
| `crates/ndp-lib/src/db.rs` | URL validation, timeout, NoTls |
| `tools/ndp-gold-ddl/src/db/client.rs` | URL validation, timeout, NoTls, spawns connection handler |

Nearly identical connection setup code.

### 4. Config Types (3 independent definitions)

| Type | ndp-gold-ddl | ndp-validate | ndp-lib/ndp-types |
|------|-------------|-------------|-------------------|
| StreamConfig | Own struct in `config/types.rs` | `serde_json::Value` (untyped) | Own struct in `config.rs` |
| GoldEtlConfig | Own struct in `config/types.rs` | `serde_json::Value` fields | Not defined |
| DomainConfig | Own struct in `config/domain.rs` | `serde_json::Value` | Own struct in `config.rs` |
| SourceType | Not used | Via `ndp_types::SourceType` | Via `ndp_types::SourceType` |
| FieldType | Not used | Via `ndp_types::FieldType` | Via `ndp_types::FieldType` |

**ndp-gold-ddl defines its own config types from scratch**, not sharing with ndp-types or ndp-lib. This is the root cause of divergence risk.

### 5. Validation Error Types (2 definitions)

| Location | Types |
|----------|-------|
| `crates/ndp-types/src/validate.rs` | `ValidationError`, `ErrorCode`, `Severity`, `ValidationLayer`, `NdpValidate` trait |
| `tools/ndp-validate/src/error.rs` | `ValidationError`, `ErrorCode`, `Severity`, `ValidationLayer` (different structures) |

ndp-validate does NOT use ndp-types' validation infrastructure despite depending on ndp-types.

### 6. Gold Validation Constants (2 definitions)

| Constant | ndp-gold-ddl location | ndp-validate location |
|----------|----------------------|----------------------|
| `VALID_METRICS` | `src/config/types.rs` | `src/semantic/gold.rs` (hardcoded list) |
| `VALID_ROLLING_STATS` | `src/config/types.rs` | `src/semantic/gold.rs` (hardcoded list) |
| Granularity validation | `parse_granularity()` in `validation/` | `is_valid_granularity()` in `semantic/gold.rs` AND `semantic/domain.rs` |

These lists are identical today but maintained independently -- guaranteed to drift.

### 7. NoOpDbClient (3 definitions)

| Location | Purpose |
|----------|---------|
| `tools/ndp-cli/src/commands/dictionary.rs` | Dry-run mode |
| `tools/ndp-cli/src/commands/dimension.rs` | Dry-run mode |
| `tools/ndp-cli/src/commands/domain.rs` | Dry-run mode |

Identical implementation copy-pasted 3 times within the same crate. Noted for ops-003 in BUG-002 architecture doc.

### 8. Levenshtein / find_closest_match (5 implementations)

| Location | Implementation |
|----------|---------------|
| `ndp-validate/src/semantic/dq_rules.rs` | Hand-rolled `levenshtein_distance()` |
| `ndp-validate/src/semantic/source_path.rs` | `strsim::levenshtein` |
| `ndp-validate/src/semantic/gold.rs` | `strsim::levenshtein` |
| `ndp-validate/src/semantic/domain.rs` | `strsim::levenshtein` |
| `ndp-validate/src/semantic/sources.rs` | `strsim::levenshtein` |

Each module has its own `find_closest_match()` helper with identical logic.

### 9. Stream Discovery (2 implementations)

| Location | How |
|----------|-----|
| `ndp-validate/src/semantic/domain.rs` | `discover_streams()` reads filesystem |
| `ndp-gold-ddl/src/config/loader.rs` | `FileSystemConfigLoader` reads filesystem |

Both enumerate `config/base/streams/*/config.json` independently.

---

## Duplication Impact Summary

| Category | Count | Risk Level | Agent Confusion? |
|----------|-------|------------|-----------------|
| DbClient trait | 2 | Medium (drift) | **Yes** -- which DbClient? |
| Config types | 3 independent | **High** (already divergent) | **Yes** -- which StreamConfig? |
| ConfigLoader | 2 | Medium | **Yes** -- which loader? |
| PostgresClient | 2 | Low (stable) | Yes |
| Validation types | 2 | Medium | Yes |
| Constants | 2 | **High** (silent drift) | No |
| NoOpDbClient | 3 | Low (internal) | No |

**The top agent-confusion items are config types and DbClient** -- when an agent needs to modify "StreamConfig", there are 3 different structs across the workspace with that name.
