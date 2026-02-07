# ops-003 Analysis: Migration Plan Critique

> **Date**: 2026-02-06
> **Subject**: Assessment of doc 09 (Stepwise Migration Plan) against current reality

---

## The Plan vs Reality

Doc 09 describes an **8-phase, 18-week** migration to a unified `ndp` CLI. After ops-001 and ops-002, we've completed roughly **Phase 1 (partial)** and **Phase 3 (partial)**.

### What the Plan Got Right

1. **Library-first architecture** -- ndp-lib exists and works well. The `DbClient` trait, `ConfigLoader` trait, and `SyncReport` pattern are clean and testable.

2. **Entity/verb CLI pattern** -- ndp-cli implements this correctly for the 3 commands it has.

3. **deploy.sh fallback pattern** -- `if command -v ndp` works well and is proven across 3 integration points.

4. **Additive-first principle** -- We added ndp-cli alongside existing tools without breaking anything.

### What the Plan Got Wrong

1. **Assumed sequential phases would prevent confusion**. In practice, having 3 binaries with overlapping concerns (especially Gold config validation in both ndp-validate and ndp-gold-ddl) created more confusion than if we'd consolidated earlier.

2. **Phase 2 (migrate existing tools to ndp-lib) was skipped**. ndp-gold-ddl was never migrated to depend on ndp-lib. The plan said "simplify tool binaries to call ndp_lib" but ops-001 explicitly deferred this. This left us with the worst of both worlds: a shared library nobody shares.

3. **18-week timeline is too long for a single-developer project**. The plan was designed for a team. For an agent-driven project with one human, we need faster consolidation of the agent-confusing parts.

4. **MCP migration (Phases 4-5) is premature**. The MCP server works and isn't causing agent confusion. Migrating it adds risk without solving the immediate problem.

5. **CI/CD (Phase 7) is irrelevant**. Project explicitly uses git-as-transport with Pi builds. No GitHub Actions needed.

---

## What Actually Caused Agent Confusion

### Root Cause 1: Three "StreamConfig" types

When an agent is told "fix the StreamConfig", it has to figure out which one:
- `platform_core::config::StreamConfig` (core library -- 100+ fields, used by apps)
- `ndp_gold_ddl::config::StreamConfig` (tools/ndp-gold-ddl -- Gold-specific fields)
- `ndp_lib::config::StreamConfig` (crates/ndp-lib -- sync-focused fields)

ndp-validate doesn't even have a struct -- it works on raw `serde_json::Value`.

### Root Cause 2: Two validation pipelines

- `ndp-validate` validates Gold config via `semantic/gold.rs` (regex, Value inspection)
- `ndp-gold-ddl` validates Gold config via `validation/config_validator.rs` (typed structs)

An agent fixing a Gold validation bug might fix it in the wrong validator.

### Root Cause 3: deploy.sh dispatches to 3 different binaries

```
deploy.sh
  ├── command -v ndp          (lines 386, 894, 1063)
  ├── command -v ndp-validate (lines 1535, 2035)
  └── command -v ndp-gold-ddl (lines 1938, 2071)
```

When deploy.sh fails, the agent has to determine which of 3 binaries failed and which of 3 codebases to investigate.

### Root Cause 4: No dependency between deployment tools

```
ndp-cli ──> ndp-lib ──> ndp-types
ndp-validate ──────────> ndp-types
ndp-gold-ddl ──────────> (nothing)
```

The three tools don't share code, so fixing a bug in one doesn't fix it in the others.

---

## Phases We Should Skip or Reorder

| Original Phase | Recommendation | Reason |
|----------------|---------------|--------|
| Phase 1: ndp-lib foundation | **Done** (ops-001) | Already exists |
| Phase 2: Migrate tools to ndp-lib | **DO THIS FIRST** (ops-003) | Root cause of confusion |
| Phase 3: ndp CLI skeleton | **Done** (ops-001) | Already exists |
| Phase 4: Migrate MCP to ndp-lib | **Defer** (V1.3+) | Not causing confusion |
| Phase 5: ndp mcp serve | **Defer** (V1.3+) | Not needed yet |
| Phase 6: ndp deploy commands | **Defer** (V2.0) | deploy.sh works fine |
| Phase 7: CI/CD | **Skip entirely** | Project uses git transport |
| Phase 8: Deprecate old paths | **Partial** (ops-003) | Gold/validate become subcommands |

---

## Revised Priority: Fix Agent Confusion First

The migration plan optimizes for **zero risk** with parallel operation. But the current state IS risky -- agents waste time investigating the wrong codebase. The revised priority should be:

1. **Consolidate shared types** (ndp-gold-ddl depends on ndp-lib for DbClient, config types)
2. **Make ndp-gold-ddl a subcommand** (`ndp gold generate`, `ndp gold validate`)
3. **Make ndp-validate a subcommand** (`ndp validate config`, `ndp validate schema`)
4. **Single binary in deploy.sh** (`command -v ndp` everywhere, no separate binary checks)

This is essentially Phase 2 + Phase 3 completion from the original plan, but reframed as the urgent priority rather than a leisurely migration.
