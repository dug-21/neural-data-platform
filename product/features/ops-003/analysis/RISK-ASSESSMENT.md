# ops-003 Analysis: Risk Assessment

> **Date**: 2026-02-06

---

## Risk Matrix

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| ndp-gold-ddl tests break when adding ndp-lib dependency | Medium | High (376 tests) | Add dependency, don't change internal code yet. Only wire DbClient trait via re-export. |
| ndp-validate tests break when adding ndp-lib dependency | Low | High (217 tests) | Only use ndp-lib for constants. Don't restructure validation logic. |
| deploy.sh regression when switching from standalone to subcommand | Medium | High (production) | Test in integration env. Keep standalone binaries buildable as fallback. |
| Flag incompatibility between standalone and subcommand forms | Low | Medium | Map every flag explicitly (see DEPLOY-SH-AUDIT.md). Forward unknown flags. |
| ndp binary size grows significantly | Low | Low | Libraries linked statically already. Incremental growth ~1-2MB. |
| Circular dependency between crates | Low | High | ndp-lib is leaf (depends only on ndp-types). ndp-gold-ddl and ndp-validate depend on ndp-lib. ndp-cli depends on all three. No cycles. |

---

## Dependency Chain After ops-003

```
ndp-types (foundation)
    │
    ├──> ndp-lib (shared infra: DbClient, ConfigLoader, constants)
    │       │
    │       ├──> ndp-gold-ddl (Gold DDL generators, uses ndp-lib::DbClient)
    │       │
    │       └──> ndp-validate (config validation, uses ndp-lib::constants)
    │
    └──> ndp-cli (binary, depends on all three above)
```

No cycles. Clean layering. Each crate has a clear single purpose.

---

## Rollback Strategy

Each phase is independently revertable:

| Phase | Rollback |
|-------|----------|
| A (shared infra) | Remove ndp-lib dependency from ndp-gold-ddl/ndp-validate Cargo.toml. Restore inline constants. |
| B (subcommand facades) | Remove commands/gold.rs and commands/validate.rs from ndp-cli. Standalone binaries still work. |
| C (deploy.sh consolidation) | Restore `command -v ndp-validate` and `command -v ndp-gold-ddl` checks. Standalone binaries still exist. |

---

## What Could Go Wrong

### 1. ndp-gold-ddl's DbClient has different semantics

ndp-gold-ddl's `DbClient::query()` returns `Vec<Row>`. ndp-lib's `DbClient::query()` also returns `Vec<Row>`. The signatures should be compatible, but ndp-gold-ddl also has `CaChecker` which depends on `DbClient` -- this trait bound needs to work with ndp-lib's version.

**Mitigation**: Make ndp-gold-ddl's internal `DbClient` a re-export of `ndp_lib::DbClient`. Add `execute()` and `batch_execute()` to the trait bound where `CaChecker` is used (it only uses `query()`, so this is additive).

### 2. Config type mismatch

ndp-gold-ddl defines `StreamConfig` with Gold-specific fields (gold_etl, transitions, etc.) that ndp-lib's `StreamConfig` doesn't have. These are NOT the same type and should NOT be unified yet.

**Mitigation**: Leave config types as-is in ops-003. ndp-gold-ddl keeps its own `StreamConfig`. Future ops feature can unify when all fields are catalogued.

### 3. Binary size on Pi

Adding ndp-gold-ddl and ndp-validate as dependencies to ndp-cli will grow the binary. Current ndp binary is ~5MB. With generators and validators linked in, expect ~8-10MB.

**Mitigation**: Still well under the 15MB target from the migration plan. ARM64 binary size is not a concern.

### 4. Compilation time

Adding two large crates to ndp-cli's dependency tree will increase build time on Pi.

**Mitigation**: Build tools on Pi is already ~20-35 minutes. Single binary means ONE build instead of three, potentially faster overall. Long-term: CI cross-compilation (Phase 7 of migration plan, deferred).
