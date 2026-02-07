# ops-003 Analysis: Recommendations

> **Date**: 2026-02-06
> **Goal**: Eliminate agent confusion by consolidating deployment tooling
> **Constraint**: Preserve all 687 existing tests, zero functionality regression

---

## Executive Summary

ops-003 should **consolidate the three deployment binaries into one `ndp` binary** by:
1. Making ndp-gold-ddl depend on ndp-lib for shared infrastructure
2. Adding `gold` and `validate` subcommands to ndp-cli
3. Reducing deploy.sh to a single `command -v ndp` check

This is NOT a rewrite. The generators (376 tests) and validators (217 tests) stay in their crates. Only the entry points and shared infrastructure move.

---

## Recommended Approach: Facade Pattern

**Keep ndp-gold-ddl and ndp-validate as library crates. Add thin facades in ndp-cli.**

```
BEFORE (3 binaries, agent confusion):
  deploy.sh → ndp (dictionary/dimension/domain)
  deploy.sh → ndp-validate (config validation)
  deploy.sh → ndp-gold-ddl (Gold DDL generation)

AFTER (1 binary, clear ownership):
  deploy.sh → ndp dictionary sync
  deploy.sh → ndp dimension sync
  deploy.sh → ndp domain sync
  deploy.sh → ndp validate config [--all]
  deploy.sh → ndp gold generate [--stream X]
  deploy.sh → ndp gold sync [--stream X]
```

### Why Facade, Not Full Extraction

The 18-week plan calls for extracting all logic into ndp-lib. This is the right long-term goal but the wrong ops-003 scope because:

1. **376 ndp-gold-ddl tests pass today**. Moving all generators to ndp-lib risks breaking them for no immediate benefit.
2. **217 ndp-validate tests pass today**. Same concern.
3. **The agent confusion is about entry points**, not internal logic. Agents get confused about WHICH BINARY to work in, not which internal module.
4. **Facade costs ~100 lines of glue code** vs ~2000 lines of extraction/refactoring.

### Architecture After ops-003

```
ndp-cli (single binary: ndp)
├── commands/dictionary.rs   → ndp_lib::dictionary::sync_dictionary()
├── commands/dimension.rs    → ndp_lib::dimension::sync_dimension()
├── commands/domain.rs       → ndp_lib::domain::sync_domains()
├── commands/validate.rs     → ndp_validate::validate_*()        # NEW facade
├── commands/gold.rs         → ndp_gold_ddl::*()                 # NEW facade
└── commands/status.rs       → (future)

ndp-lib (shared infrastructure)
├── db.rs          DbClient trait, PostgresClient        (exists)
├── config.rs      ConfigLoader trait                    (exists)
├── types.rs       SyncReport, SyncOptions               (exists)
├── constants.rs   VALID_METRICS, GOLD_SCHEMA, etc.      # NEW: single source of truth
└── [modules]      dictionary, dimension, domain          (exist)

ndp-validate (library + optional binary)
├── [all existing modules unchanged]
├── Cargo.toml: add `ndp-lib` dependency for shared constants
└── lib.rs: public API for ndp-cli to call

ndp-gold-ddl (library + optional binary)
├── [all existing modules unchanged]
├── Cargo.toml: add `ndp-lib` dependency for DbClient, ConfigLoader
└── lib.rs: public API for ndp-cli to call
```

---

## Phased Delivery (3 phases, ~3-4 weeks total)

### Phase A: Shared Infrastructure (v1.2.0)

**Goal**: ndp-gold-ddl and ndp-validate depend on ndp-lib for shared types.

**Tasks**:
1. Extract `VALID_METRICS`, `VALID_ROLLING_STATS` to `ndp-lib/src/constants.rs`
2. Extract `NoOpDbClient` to `ndp-lib/src/db.rs` (deduplicate from 3 copies)
3. Migrate ndp-gold-ddl to use `ndp_lib::DbClient` trait (superset of its own)
   - ndp-gold-ddl's `db::client.rs` becomes a thin re-export
   - `CaChecker` stays in ndp-gold-ddl (Gold-specific)
   - 376 tests must still pass
4. Add `ndp-lib` as dependency to ndp-validate's Cargo.toml
   - ndp-validate uses ndp-lib constants for VALID_METRICS (no more hardcoded lists)
   - 217 tests must still pass

**Manifest type**: MINOR (new shared infrastructure, backwards compatible)

**Risk**: Low. Only moving shared constants and making ndp-gold-ddl consume an existing trait.

### Phase B: Subcommand Facades (v1.2.1)

**Goal**: `ndp gold` and `ndp validate` commands exist in ndp-cli.

**Tasks**:
1. Add `ndp-gold-ddl` as dependency to ndp-cli's Cargo.toml
2. Create `commands/gold.rs`:
   - `ndp gold generate --stream <id>` → calls `ndp_gold_ddl::run()` with appropriate args
   - `ndp gold generate --domain <id>` → same
   - `ndp gold validate --stream <id>` → same
   - `ndp gold sync --stream <id> --database-url <url>` → same with sync action
3. Add `ndp-validate` as dependency to ndp-cli's Cargo.toml
4. Create `commands/validate.rs`:
   - `ndp validate config <path>` → calls `ndp_validate::validate_stream()`
   - `ndp validate config --all` → calls `ndp_validate::validate_all()`
   - `ndp validate domain <path>` → calls `ndp_validate::validate_domain()`
   - `ndp validate domain --all` → calls `ndp_validate::validate_all_domains()`
   - `ndp validate schema --generate` → calls `ndp_validate::generate_schema()`
5. Both ndp-validate and ndp-gold-ddl keep their standalone binaries (backwards compat)

**Manifest type**: PATCH (no deploy changes, additive CLI commands)

**Risk**: Low. Standalone binaries still work. New commands are just routing.

### Phase C: deploy.sh Consolidation (v1.2.2)

**Goal**: deploy.sh uses only `command -v ndp`, not individual tool binaries.

**Tasks**:
1. Replace `command -v ndp-validate` sites (lines 1535, 2035) with `ndp validate`
2. Replace `command -v ndp-gold-ddl` sites (lines 1938, 2071) with `ndp gold`
3. Remove standalone binary builds from deploy.sh (ndp-validate, ndp-gold-ddl built as libs only)
4. Update `Cargo.toml` workspace to optionally build standalone binaries (feature flag or separate profile)
5. Test full deployment pipeline

**Manifest type**: PATCH (deploy.sh change, same functionality)

**Risk**: Medium. deploy.sh changes need integration testing. Fallback: revert to individual binaries.

---

## What NOT to Do in ops-003

| Temptation | Why Not |
|------------|---------|
| Move generators to ndp-lib | 376 tests at risk, no agent-confusion benefit |
| Move validators to ndp-lib | 217 tests at risk, no agent-confusion benefit |
| Unify StreamConfig types | 3 different structs serve 3 different purposes; premature unification |
| Add MCP commands | Not causing confusion, defer to V1.3 |
| Add stream/etl/bronze commands | Defer to V1.2 (Pattern Detection) when needed |
| Implement config from etcd | Defer to V1.3 as planned |
| Rewrite ndp-validate as typed | Works fine with serde_json::Value, 217 tests pass |

---

## Version Numbering

ops-003 introduces `ndp gold` and `ndp validate` as new CLI commands. Per RELEASE-POLICY.md, new backwards-compatible functionality = MINOR bump.

| Release | Content | Type |
|---------|---------|------|
| v1.2.0 | Phase A: shared infrastructure, ndp-gold-ddl depends on ndp-lib | MINOR |
| v1.2.1 | Phase B: `ndp gold` and `ndp validate` subcommands | PATCH |
| v1.2.2 | Phase C: deploy.sh consolidation | PATCH |

---

## Success Criteria

- [ ] Single binary (`ndp`) handles all deployment operations
- [ ] `command -v ndp` is the only binary check in deploy.sh
- [ ] `VALID_METRICS` and `VALID_ROLLING_STATS` defined in exactly one place
- [ ] `DbClient` trait defined in exactly one place (ndp-lib)
- [ ] `NoOpDbClient` defined in exactly one place
- [ ] All 687 existing tests pass
- [ ] deploy.sh full pipeline works in integration environment
- [ ] Standalone `ndp-validate` and `ndp-gold-ddl` binaries still buildable (compat)

---

## Impact on Agent Routing

After ops-003, agent instructions simplify from:

> "If the issue is in Gold DDL generation, work in tools/ndp-gold-ddl. If it's in config validation, work in tools/ndp-validate. If it's in dictionary/dimension/domain sync, work in tools/ndp-cli. If it's in shared types, work in crates/ndp-lib."

To:

> "All deployment tooling enters through tools/ndp-cli. Business logic lives in the library each command delegates to. Shared infrastructure is in crates/ndp-lib."

The agent's first question changes from "which binary?" to "which command?" -- a much simpler decision.

---

## Relationship to Migration Plan (Doc 09)

ops-003 corresponds to an accelerated version of **Phases 2-3** from the Stepwise Migration Plan, adapted for actual project constraints:

| Migration Plan Phase | ops-003 Mapping |
|---------------------|-----------------|
| Phase 1: ndp-lib foundation | **Already done** (ops-001) |
| Phase 2: Migrate tools to ndp-lib | **Phase A** (shared infra only, not full extraction) |
| Phase 3: ndp CLI skeleton | **Already done** (ops-001) + **Phase B** (add gold/validate) |
| Phase 4-8 | **Deferred** (not needed to solve agent confusion) |

The key insight: we don't need the full 18-week plan to solve the immediate problem. Facade pattern + shared constants + single binary gets us 80% of the benefit with 20% of the effort.
