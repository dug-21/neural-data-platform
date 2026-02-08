# OPS-003: Unified Action Library

> **Feature ID:** ops-003
> **Versions:** v1.1.14, v1.1.15, v1.1.16
> **Created:** 2026-02-06
> **Status:** Scoping
> **Phase:** ops (Infrastructure / Deployment)

---

## Executive Summary

OPS-003 migrates Gold DDL generation and configuration validation into `ndp-lib`, establishing it as the **single library of NDP actions**. Every operation the platform can perform — sync, generate, validate — becomes a function in ndp-lib, callable from CLI today and from MCP/API in the future. The `ndp` binary becomes the sole deployment tool; `ndp-gold-ddl` and `ndp-validate` standalone binaries are retired from deploy.sh.

Delivered as **three independent releases** that each leave deployment fully functional.

### The OPS-003 Promise

| Capability | Description |
|------------|-------------|
| **Gold actions in ndp-lib** | `ndp_lib::gold::generate()`, `sync()`, `recreate()` — migrated from ndp-gold-ddl |
| **Validate actions in ndp-lib** | `ndp_lib::validate::stream()`, `domain()`, `all()` — migrated from ndp-validate |
| **Cross-cutting validation** | Mutating commands validate config first by default; opt-out via `--no-validate` |
| **Single deployment binary** | deploy.sh calls `ndp` for all 7 dispatch sites; no more `ndp-validate` / `ndp-gold-ddl` checks |
| **Shared infrastructure** | One `DbClient`, one `ConfigLoader`, one `VALID_METRICS` list — no more duplication |
| **593 tests preserved** | ndp-gold-ddl (376) + ndp-validate (217) tests migrate with code; import paths change, logic doesn't |

### Success Test

> **Can `DEPLOY_ENV=integration ./deploy.sh apply <manifest>` complete identically using only the `ndp` binary for validation, Gold DDL generation, dictionary sync, dimension sync, and domain sync?**

If yes, deploy.sh dispatches to a single binary and agents have one codebase to investigate when deployment fails.

---

## Problem Statement

### Current State (V1.1.13)

After ops-001 and ops-002, deployment functionality is spread across **three binaries with no shared code**:

```
deploy.sh
  ├── command -v ndp          (3 sites: dictionary, dimension, domain sync)
  ├── command -v ndp-validate (2 sites: config validation)
  └── command -v ndp-gold-ddl (2 sites: Gold DDL generation)
```

This causes:

1. **Agent confusion**: When deploy.sh fails, agents must determine which of 3 codebases to investigate. During ops-002, time was lost fixing issues in the wrong tool.

2. **Code duplication**: 9 categories of duplicated code (see `analysis/DUPLICATION-AUDIT.md`):
   - `DbClient` trait defined in ndp-lib AND ndp-gold-ddl
   - `ConfigLoader` trait defined in ndp-lib AND ndp-gold-ddl
   - `PostgresClient` connection logic in ndp-lib AND ndp-gold-ddl
   - `VALID_METRICS` / `VALID_ROLLING_STATS` lists in ndp-gold-ddl AND ndp-validate
   - Gold config validation in ndp-gold-ddl AND ndp-validate (divergent logic)
   - `NoOpDbClient` copied 3 times within ndp-cli
   - Config types defined independently across 3 crates
   - Granularity validation duplicated
   - Stream filesystem discovery duplicated

3. **Validation is fragmented**: Config validation is a core competency of a config-driven platform, but it lives in a standalone binary that other tools can't call. Gold generation re-implements its own validation instead of calling the canonical validator.

4. **ndp-lib is underused**: It was designed as the shared action library, but only ndp-cli consumes it. ndp-gold-ddl and ndp-validate don't depend on it at all.

### Target State (V1.1.16)

- `ndp-lib` is the single library of NDP actions
- `ndp gold generate/sync/recreate` replaces `ndp-gold-ddl generate`
- `ndp validate --all/--stream/--domain` replaces `ndp-validate --all/--domain`
- Gold generation calls `ndp_lib::validate::gold_config()` before generating DDL
- deploy.sh has exactly one binary check pattern: `command -v ndp`
- All shared infrastructure (DbClient, ConfigLoader, constants) defined once in ndp-lib

### Why Now (Before V1.2 Pattern Detection)

V1.2 adds `ndp pattern scan`, `ndp job create`, and other actions that will live in ndp-lib. If the library extraction isn't done, V1.2 agents face the same fragmentation — and worse, they might model new features after the standalone-binary anti-pattern instead of the library-first pattern.

---

## Scope Definition

### In Scope

#### Release 1 — v1.1.14: Gold Migration

| ID | Feature | Description | Acceptance Criteria |
|----|---------|-------------|---------------------|
| **ops-003-01** | Gold module in ndp-lib | Migrate generator logic from ndp-gold-ddl to `ndp_lib::gold` | All 376 ndp-gold-ddl tests pass under new module paths |
| **ops-003-02** | Shared DbClient | ndp-gold-ddl's `CaChecker` uses `ndp_lib::DbClient` | Single trait definition; ndp-gold-ddl crate depends on ndp-lib |
| **ops-003-03** | `ndp gold` subcommands | `ndp gold generate`, `ndp gold sync`, `ndp gold recreate` in ndp-cli | Commands produce identical output to `ndp-gold-ddl` standalone |
| **ops-003-04** | deploy.sh gold switchover | Replace `command -v ndp-gold-ddl` sites with `ndp gold` | deploy.sh Gold phases work via `ndp` binary |

#### Release 2 — v1.1.15: Validate Migration

| ID | Feature | Description | Acceptance Criteria |
|----|---------|-------------|---------------------|
| **ops-003-05** | Validate module in ndp-lib | Migrate validation logic from ndp-validate to `ndp_lib::validate` | All 217 ndp-validate tests pass under new module paths |
| **ops-003-06** | `ndp validate` subcommands | `ndp validate --all`, `--stream`, `--domain`, `--schema` in ndp-cli | Commands produce identical output to `ndp-validate` standalone |
| **ops-003-07** | deploy.sh validate switchover | Replace `command -v ndp-validate` sites with `ndp validate` | deploy.sh validation phases work via `ndp` binary |

#### Release 3 — v1.1.16: Shared Constants + Cross-cutting Validation

| ID | Feature | Description | Acceptance Criteria |
|----|---------|-------------|---------------------|
| **ops-003-08** | Shared constants | `VALID_METRICS`, `VALID_ROLLING_STATS`, `GOLD_SCHEMA`, `SILVER_SCHEMA` in `ndp_lib::constants` | Constants defined once; both gold and validate modules use them |
| **ops-003-09** | Cross-cutting validation | `ndp gold sync` calls `ndp_lib::validate::gold_config()` before generating DDL | Gold sync validates config by default; `--no-validate` skips |
| **ops-003-10** | Gold validation unification | Remove duplicate Gold config validation from gold module; use `validate::gold_config()` | Single validation pipeline for Gold config |
| **ops-003-11** | NoOpDbClient dedup | Single `NoOpDbClient` in ndp-lib for dry-run mode | 3 copies → 1 |
| **ops-003-12** | Standalone binary thin wrappers | ndp-gold-ddl and ndp-validate re-export from ndp-lib | Standalone binaries still buildable but just delegate to ndp-lib |
| **ops-003-13** | Retire stale YAML stream configs | Rename `config/**/config.yaml` to `config.yaml.bak` | No `.yaml` stream/domain configs remain active; `platform.yaml` unchanged |

### Out of Scope (Deferred)

| Item | Reason | Target |
|------|--------|--------|
| Unifying StreamConfig types | 3 structs serve different purposes (core, gold, sync); premature | V1.3 |
| `ndp stream` commands | No operational need yet; stream sync is etcd-facing (Bash adequate) | V1.2+ |
| `ndp config get/set` | Needs config-client integration; etcd access from CLI | V1.3 |
| `ndp status` commands | Nice to have, not blocking anything | V1.2+ |
| MCP/API serving | Security requirements not determined | V1.3+ |
| `ndp deploy apply` | deploy.sh remains release governor | Evaluate V2.0 |
| Migrating stream/etcd sync from Bash | Not DB-facing; Bash is adequate | V1.3 |
| Dictionary/dimension read commands | Already work in MCP server; library extraction later | V1.2+ |

---

## Technical Design

### Crate Architecture After OPS-003

```
crates/ndp-lib/src/                          # The action library
├── lib.rs
├── db.rs              DbClient, PostgresClient, NoOpDbClient
├── config.rs          ConfigLoader, FileSystemConfigLoader
├── constants.rs       VALID_METRICS, VALID_ROLLING_STATS, GOLD_SCHEMA, SILVER_SCHEMA   # NEW
├── convert.rs         Config → sync-type bridges
├── error.rs           NdpLibError
├── types.rs           SyncReport, SyncOptions
├── dictionary/        exists (ops-001)
├── dimension/         exists (ops-001)
├── domain/            exists (ops-002)
├── gold/              NEW — migrated from tools/ndp-gold-ddl/src/
│   ├── mod.rs         Public API: generate(), sync(), recreate()
│   ├── config.rs      Gold-specific config types (GoldEtlConfig, etc.)
│   ├── generators/    ContinuousAggregate, AlignedView, StateTransition, Events
│   ├── planner/       SyncPlanner (create/skip/recreate decisions)
│   ├── registry/      FeatureRegistry (lag, rolling, trend)
│   └── validation/    Gold-specific config validation
└── validate/          NEW — migrated from tools/ndp-validate/src/
    ├── mod.rs         Public API: all(), stream(), domain(), gold_config()
    ├── schema.rs      JSON Schema validation (Layer 1)
    ├── schema_gen.rs  Schema generation from ndp-types
    └── semantic/      Cross-field validation (Layer 2)
        ├── mod.rs     SemanticValidator coordinator
        ├── sources.rs
        ├── source_path.rs
        ├── dq_rules.rs
        ├── gold.rs    → calls constants from ndp_lib::constants
        └── domain.rs

tools/ndp-gold-ddl/                          # Becomes thin wrapper (v1.1.16)
├── Cargo.toml         depends on ndp-lib
└── src/
    ├── lib.rs         re-exports from ndp_lib::gold
    └── main.rs        CLI entry point → calls ndp_lib::gold::*

tools/ndp-validate/                          # Becomes thin wrapper (v1.1.16)
├── Cargo.toml         depends on ndp-lib
└── src/
    ├── lib.rs         re-exports from ndp_lib::validate
    └── main.rs        CLI entry point → calls ndp_lib::validate::*

tools/ndp-cli/src/                           # Single binary
├── main.rs            Clap routing + global flags
└── commands/
    ├── mod.rs
    ├── dictionary.rs  exists
    ├── dimension.rs   exists
    ├── domain.rs      exists
    ├── gold.rs        NEW — routes to ndp_lib::gold::*
    └── validate.rs    NEW — routes to ndp_lib::validate::*
```

### Dependency Graph After OPS-003

```
ndp-types (foundation — no workspace deps)
    │
    └──> ndp-lib (action library — depends on ndp-types)
            │
            ├──> ndp-cli        (binary: ndp)         — depends on ndp-lib
            ├──> ndp-gold-ddl   (binary: ndp-gold-ddl) — depends on ndp-lib (thin wrapper)
            └──> ndp-validate   (binary: ndp-validate) — depends on ndp-lib (thin wrapper)
```

No cycles. Clean layering. ndp-lib is the sole authority for all NDP actions.

### Key Design Decisions

#### D1: Library extraction, not facade

The gold generators and validators MOVE into ndp-lib. They don't stay in their crates with thin wrappers. This enables cross-module calls:

```rust
// ndp_lib::gold::sync() calls ndp_lib::validate::gold_config() directly
pub async fn sync(
    config: &StreamConfig,
    db: &(impl DbClient + Send + Sync),
    opts: &SyncOptions,
) -> Result<SyncReport> {
    if opts.validate {
        crate::validate::gold_config(config)?;  // sibling module call
    }
    let ddl = generate(config, &GenerateOptions::from(opts))?;
    // ... apply DDL
}
```

This is impossible with separate crates unless you add ndp-validate as a dependency of ndp-gold-ddl, creating a tangle.

#### D2: Cross-cutting validation via SyncOptions

```rust
// ndp_lib::types.rs
pub struct SyncOptions {
    pub dry_run: bool,
    pub validate: bool,    // default: true
    pub verbose: bool,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self { dry_run: false, validate: true, verbose: false }
    }
}
```

Every mutating function in ndp-lib checks `opts.validate` before proceeding. The CLI exposes this as `--no-validate`. Default is always validate.

#### D3: Gold config types stay Gold-specific

ndp-gold-ddl defines `StreamConfig`, `GoldEtlConfig`, `DomainConfig` with Gold-specific fields. These move to `ndp_lib::gold::config` — NOT merged with `ndp_lib::config::StreamConfig` (which is sync-focused). The types serve different purposes:

| Type | Purpose | Fields |
|------|---------|--------|
| `ndp_lib::config::StreamConfig` | Dictionary/dimension sync | stream_id, description, fields, silver_etl |
| `ndp_lib::gold::config::StreamConfig` | Gold DDL generation | stream_id, fields, gold_etl, timestamp |
| `platform_core::config::StreamConfig` | Runtime ingestion | Everything (100+ fields) |

Premature unification would force every consumer to carry every field. Defer to V1.3 when we can evaluate whether a single canonical type with optional sections works.

#### D4: Standalone binaries remain buildable

ndp-gold-ddl and ndp-validate keep their `main.rs` and remain workspace members. They just re-export from ndp-lib now:

```rust
// tools/ndp-gold-ddl/src/main.rs (after migration)
use ndp_lib::gold;

fn main() {
    // Same CLI arg parsing, calls ndp_lib::gold::* instead of local modules
}
```

deploy.sh stops calling them, but they still build for anyone who wants them.

#### D5: No fallback in deploy.sh — fail loudly

When deploy.sh switches from `ndp-gold-ddl` to `ndp gold`, it does **not** fall back to the old binary. If `ndp` is missing or the subcommand fails, deploy.sh fails immediately. Fallbacks cover up problems; we need to know they exist to fix them.

---

## Implementation Phases

### Release 1: v1.1.14 — Gold Migration

**Moves 29 source files and 376 tests. Switches 2 deploy.sh dispatch sites.**

| Task | Description |
|------|-------------|
| Create `ndp_lib::gold` module structure | `mod.rs`, `config.rs`, `generators/`, `planner/`, `registry/`, `validation/` |
| Move generator source files | `git mv` + update `use` paths. ContinuousAggregate, AlignedView, StateTransition, Events, RefreshPolicy |
| Move planner source files | SyncPlanner, CaChecker, CaInfo |
| Move registry source files | FeatureRegistry, LagFeature, RollingFeature, TrendFeature |
| Move config types | GoldEtlConfig, StreamConfig (Gold-specific), DomainConfig, Action enum |
| Move validation | ConfigValidator, parse_granularity, parse_window |
| Wire `CaChecker` to use `ndp_lib::DbClient` | Replace ndp-gold-ddl's local DbClient with ndp-lib's trait |
| Add `NoOpDbClient` to `ndp_lib::db` | Move one of the 3 copies; others continue to work (dedup in v1.1.16) |
| Verify all 376 tests pass | `cargo test -p ndp-lib` includes gold tests |
| Update ndp-gold-ddl to thin-wrap ndp-lib | Cargo.toml change; main.rs calls `ndp_lib::gold::*` |
| Verify ndp-gold-ddl standalone still works | `ndp-gold-ddl generate --stream air-quality` produces same output |
| Add `commands/gold.rs` to ndp-cli | `ndp gold generate`, `ndp gold sync`, `ndp gold recreate` |
| Verify `ndp gold` matches `ndp-gold-ddl` output | Same DDL for all stream and domain configs |
| **deploy.sh: switch gold dispatch sites** | See deploy.sh changes below |
| Integration test | `DEPLOY_ENV=integration ./deploy.sh apply` — Gold phases work via `ndp` |

#### v1.1.14 deploy.sh Changes

**Site 1: `handle_gold_tables()` (line ~1936)**

BEFORE:
```bash
# Check if ndp-gold-ddl tool is available
local gold_ddl_tool=""
if command -v ndp-gold-ddl &> /dev/null; then
    gold_ddl_tool="ndp-gold-ddl"
elif [ -x "/opt/ndp/bin/ndp-gold-ddl" ]; then
    gold_ddl_tool="/opt/ndp/bin/ndp-gold-ddl"
elif [ -x "$REPO_ROOT/target/release/ndp-gold-ddl" ]; then
    gold_ddl_tool="$REPO_ROOT/target/release/ndp-gold-ddl"
elif [ -x "$REPO_ROOT/target/debug/ndp-gold-ddl" ]; then
    gold_ddl_tool="$REPO_ROOT/target/debug/ndp-gold-ddl"
else
    warn "  ndp-gold-ddl tool not found, skipping Gold DDL generation"
    warn "  Build the tool with: cargo build --release -p ndp-gold-ddl"
    return 0
fi
# ...
ddl=$("$gold_ddl_tool" --config-dir "$REPO_ROOT/config" \
    --database-url "$db_url" \
    --db-timeout 10 \
    generate --stream "$stream_id" --action "$action" 2>&1)
```

AFTER:
```bash
# Resolve ndp tool (required — no fallback)
local ndp_tool=""
if command -v ndp &> /dev/null; then
    ndp_tool="ndp"
elif [ -x "/opt/ndp/bin/ndp" ]; then
    ndp_tool="/opt/ndp/bin/ndp"
elif [ -x "$REPO_ROOT/target/release/ndp" ]; then
    ndp_tool="$REPO_ROOT/target/release/ndp"
elif [ -x "$REPO_ROOT/target/debug/ndp" ]; then
    ndp_tool="$REPO_ROOT/target/debug/ndp"
else
    error "ndp tool not found. Build with: cargo build --release -p ndp-cli"
    return 1
fi
# ...
ddl=$("$ndp_tool" gold "$action" --stream "$stream_id" \
    --config-dir "$REPO_ROOT/config" \
    --db-url "$db_url" \
    --db-timeout 10 2>&1)
```

**Site 2: `handle_domain_declaration()` gold dispatch (line ~2069)**

BEFORE:
```bash
# Check if ndp-gold-ddl tool is available for aligned view generation
local gold_ddl_tool=""
if command -v ndp-gold-ddl &> /dev/null; then
    gold_ddl_tool="ndp-gold-ddl"
# ... same 4-way lookup ...
fi

if [ -z "$gold_ddl_tool" ]; then
    warn "  ndp-gold-ddl tool not found, skipping aligned view DDL generation"
    return 0
fi

ddl=$("$gold_ddl_tool" --config-dir "$REPO_ROOT/config" generate --domain "$domain_id" --action "$action" 2>&1)
```

AFTER:
```bash
# Resolve ndp tool (required — no fallback)
local ndp_tool=""
if command -v ndp &> /dev/null; then
    ndp_tool="ndp"
elif [ -x "/opt/ndp/bin/ndp" ]; then
    ndp_tool="/opt/ndp/bin/ndp"
elif [ -x "$REPO_ROOT/target/release/ndp" ]; then
    ndp_tool="$REPO_ROOT/target/release/ndp"
elif [ -x "$REPO_ROOT/target/debug/ndp" ]; then
    ndp_tool="$REPO_ROOT/target/debug/ndp"
else
    error "ndp tool not found. Build with: cargo build --release -p ndp-cli"
    return 1
fi

ddl=$("$ndp_tool" gold generate --domain "$domain_id" \
    --config-dir "$REPO_ROOT/config" 2>&1)
```

**Key differences**:
- `error` + `return 1` instead of `warn` + `return 0`. No fallback. Deployment fails if `ndp` is not available.
- Domain dispatch uses `gold generate` explicitly (not `gold "$action"`). Domain DDL generation never uses DB checks — it always generates DDL that deploy.sh pipes to psql. The `$action` variable is irrelevant here.

**Flag harmonization**: `--database-url` becomes `--db-url` (ndp-cli's existing convention). Only applies to Site 1 (stream sync). Site 2 (domain generate) has no DB flags.

#### v1.1.14 Exit Criteria

- `ndp gold generate --stream air-quality` produces identical DDL to `ndp-gold-ddl generate --stream air-quality`
- All 376 gold tests pass in ndp-lib
- deploy.sh Gold phases work via `ndp` binary in integration env
- Zero calls to `ndp-gold-ddl` remain in deploy.sh

---

### Release 2: v1.1.15 — Validate Migration

**Moves 13 source files and 217 tests. Switches 2 deploy.sh dispatch sites.**

| Task | Description |
|------|-------------|
| Create `ndp_lib::validate` module structure | `mod.rs`, `schema.rs`, `schema_gen.rs`, `semantic/` |
| Move semantic validator source files | sources, source_path, dq_rules, gold, domain |
| Move schema validator | SchemaValidator, DomainSchemaValidator, embedded schemas |
| Move schema generation | schema_gen.rs (schemars integration) |
| Move error types | Reconcile ndp-validate's ValidationError with ndp-types; prefer ndp-validate's richer version |
| Update Levenshtein usage | Use `strsim` everywhere; remove hand-rolled implementation in dq_rules |
| Deduplicate `is_valid_granularity` | Single implementation; gold.rs and domain.rs both call it |
| Verify all 217 tests pass | `cargo test -p ndp-lib` includes validate tests |
| Update ndp-validate to thin-wrap ndp-lib | Cargo.toml change; main.rs calls `ndp_lib::validate::*` |
| Verify ndp-validate standalone still works | `ndp-validate --all` produces same output |
| Add `commands/validate.rs` to ndp-cli | `ndp validate --all`, `--stream`, `--domain`, `--schema` |
| Verify `ndp validate` matches `ndp-validate` output | Same errors, same exit codes |
| **deploy.sh: switch validate dispatch sites** | See deploy.sh changes below |
| Integration test | `DEPLOY_ENV=integration ./deploy.sh apply` — validation phases work via `ndp` |

#### v1.1.15 deploy.sh Changes

**Site 3: `validate_domain_configs()` (line ~1535)**

BEFORE:
```bash
local validate_tool=""
if command -v ndp-validate &> /dev/null; then
    validate_tool="ndp-validate"
elif [ -x "/opt/ndp/bin/ndp-validate" ]; then
    validate_tool="/opt/ndp/bin/ndp-validate"
elif [ -x "$REPO_ROOT/target/release/ndp-validate" ]; then
    validate_tool="$REPO_ROOT/target/release/ndp-validate"
elif [ -x "$REPO_ROOT/target/debug/ndp-validate" ]; then
    validate_tool="$REPO_ROOT/target/debug/ndp-validate"
fi

if [ -z "$validate_tool" ]; then
    warn "ndp-validate not available, skipping domain validation"
    warn "Build with: cargo build -p ndp-validate --release"
    return 0
fi

# ...
"$validate_tool" --domain "$config_file" --format human
```

AFTER:
```bash
# Resolve ndp tool (required — no fallback)
local ndp_tool=""
if command -v ndp &> /dev/null; then
    ndp_tool="ndp"
elif [ -x "/opt/ndp/bin/ndp" ]; then
    ndp_tool="/opt/ndp/bin/ndp"
elif [ -x "$REPO_ROOT/target/release/ndp" ]; then
    ndp_tool="$REPO_ROOT/target/release/ndp"
elif [ -x "$REPO_ROOT/target/debug/ndp" ]; then
    ndp_tool="$REPO_ROOT/target/debug/ndp"
else
    error "ndp tool not found. Build with: cargo build --release -p ndp-cli"
    return 1
fi

# ...
"$ndp_tool" validate --domain "$config_file" --format human
```

**Site 4: `handle_domain_declaration()` validate dispatch (line ~2033)**

BEFORE:
```bash
local validate_tool=""
if command -v ndp-validate &> /dev/null; then
    validate_tool="ndp-validate"
# ... same 4-way lookup ...
fi

if [ -n "$validate_tool" ]; then
    "$validate_tool" --domain "$config_file" --config-dir "$CONFIG_STREAMS_DIR" --format human
else
    warn "  ndp-validate not available, skipping domain validation"
fi
```

AFTER:
```bash
# ndp tool already resolved earlier in this function (from v1.1.14 gold switchover)
# If not yet resolved, resolve it:
if [ -z "$ndp_tool" ]; then
    # ... same ndp resolution pattern ...
    # error + return 1 if not found
fi

"$ndp_tool" validate --domain "$config_file" --config-dir "$CONFIG_STREAMS_DIR" --format human
```

**Key difference**: `validate_tool` variable eliminated. Uses same `ndp_tool` already resolved by the gold dispatch in the same function. No `warn` + skip — if ndp is missing, function already failed at gold dispatch.

#### v1.1.15 Exit Criteria

- `ndp validate --all` produces identical output to `ndp-validate --all`
- All 217 validate tests pass in ndp-lib
- deploy.sh validation phases work via `ndp` binary in integration env
- Zero calls to `ndp-validate` remain in deploy.sh
- All 7 deploy.sh dispatch sites now use `ndp`

---

### Release 3: v1.1.16 — Shared Constants + Cross-cutting Validation

**No deploy.sh changes. Internal consolidation and cross-module wiring.**

| Task | Description |
|------|-------------|
| Extract shared constants to `ndp_lib::constants` | `VALID_METRICS`, `VALID_ROLLING_STATS`, `GOLD_SCHEMA`, `SILVER_SCHEMA`, `NDP_ENTITY_COLUMN` |
| Wire `validate::semantic::gold` to use shared constants | Replace hardcoded VALID_METRICS with `ndp_lib::constants` |
| Wire `gold::validation` to use shared constants | Same |
| Wire cross-cutting validation | `ndp_lib::gold::sync()` calls `ndp_lib::validate::gold_config()` |
| Unify Gold config validation | Remove `ndp_lib::gold::validation::ConfigValidator`; gold generation calls `validate::gold_config()` |
| Deduplicate NoOpDbClient | Single definition in `ndp_lib::db`; ndp-cli commands use it |
| Update ndp-gold-ddl standalone to thin wrapper | `main.rs` calls `ndp_lib::gold::*`, `lib.rs` re-exports |
| Update ndp-validate standalone to thin wrapper | `main.rs` calls `ndp_lib::validate::*`, `lib.rs` re-exports |
| All tests pass | `cargo test --workspace` — all 593+ tests green |

#### v1.1.16 Exit Criteria

- `VALID_METRICS` defined in exactly one place
- `DbClient` trait defined in exactly one place
- `NoOpDbClient` defined in exactly one place
- `ndp gold sync` validates config by default (cross-cutting)
- `ndp gold sync --no-validate` skips validation
- Standalone `ndp-gold-ddl` and `ndp-validate` still build and work

---

## deploy.sh Safety Principles

These apply to all three releases:

1. **No fallback**: If `ndp` is not found, `error` + `return 1`. Never `warn` + `return 0`. Fallbacks cover up problems.

2. **Same resolution pattern**: Use the established 4-way lookup (`command -v`, `/opt/ndp/bin/`, `target/release/`, `target/debug/`). Same pattern already used for `ndp` at lines 386, 894, 1063.

3. **One switchover per dispatch site per release**: Don't partially switch. Each release converts all its dispatch sites atomically.

4. **Integration test before every release**: `DEPLOY_ENV=integration ./deploy.sh apply <manifest>` must complete successfully with the new dispatch.

5. **Standalone binaries remain buildable**: They still work if someone runs them directly. deploy.sh just doesn't call them anymore.

---

## Flag Mapping: Standalone → Subcommand

### ndp-gold-ddl → ndp gold

| Standalone | Subcommand |
|-----------|------------|
| `ndp-gold-ddl generate --stream X` | `ndp gold generate --stream X` |
| `ndp-gold-ddl generate --domain X` | `ndp gold generate --domain X` |
| `ndp-gold-ddl generate --stream X --transitions` | `ndp gold generate --stream X --transitions` |
| `ndp-gold-ddl generate --domain X --events` | `ndp gold generate --domain X --events` |
| `ndp-gold-ddl generate --stream X --action sync --database-url U` | `ndp gold sync --stream X --db-url U` |
| `ndp-gold-ddl generate --stream X --action recreate --database-url U` | `ndp gold recreate --stream X --db-url U` |
| `ndp-gold-ddl validate --stream X` | `ndp gold generate --stream X --validate-only` |
| `--config-dir` | `--config-dir` (global) |
| `--database-url` | `--db-url` (global, harmonized) |
| `--db-timeout` | `--db-timeout` (global) |
| `--verbose` | `--verbose` (global) |

### ndp-validate → ndp validate

| Standalone | Subcommand |
|-----------|------------|
| `ndp-validate <path>` | `ndp validate --stream <path>` |
| `ndp-validate --all` | `ndp validate --all` |
| `ndp-validate --domain <path>` | `ndp validate --domain <path>` |
| `ndp-validate --domain-all` | `ndp validate --domain --all` |
| `ndp-validate --generate-schema` | `ndp validate --schema --generate` |
| `ndp-validate --verify-schema <path>` | `ndp validate --schema --verify <path>` |
| `--config-dir` | `--config-dir` (global) |
| `--format json` | `--format json` (global) |
| `--strict` | `--strict` |
| `--schema-only` | inherent in `--schema` subcommand |

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Gold test breakage during move | Low | High (376 tests) | Tests move with code; only `use` paths change. Run continuously during migration. |
| Validate test breakage during move | Low | High (217 tests) | Same approach. Schema validator has embedded JSON — move as-is. |
| deploy.sh regression (v1.1.14) | Medium | High | Integration test before release. Gold switchover is 2 sites only. |
| deploy.sh regression (v1.1.15) | Medium | High | Integration test before release. Validate switchover is 2 sites only. |
| Binary size growth | Low | Low | ~8-10MB combined, well under 15MB target |
| Compile time increase for ndp-cli | Low | Medium | Single build replaces 3 builds. Net neutral or faster. |
| Gold validation logic divergence during v1.1.14-v1.1.15 gap | Medium | Medium | Gold keeps its own validation during v1.1.14. Unification happens in v1.1.16. Acceptable temporary state. |
| Circular dependency | None | — | ndp-lib is leaf. Gold and validate are sibling modules. No cycles. |

---

## Version and Release

Per RELEASE-POLICY.md: internal restructuring without new user-facing features = PATCH bumps.

| Release | Content | Scope | deploy.sh Sites Changed |
|---------|---------|-------|------------------------|
| **v1.1.14** | Gold module → ndp-lib + `ndp gold` commands + gold deploy.sh switchover | 29 files, 376 tests, 2 dispatch sites | `handle_gold_tables()`, `handle_domain_declaration()` (gold part) |
| **v1.1.15** | Validate module → ndp-lib + `ndp validate` commands + validate deploy.sh switchover | 13 files, 217 tests, 2 dispatch sites | `validate_domain_configs()`, `handle_domain_declaration()` (validate part) |
| **v1.1.16** | Shared constants, cross-cutting validation, NoOpDbClient dedup, standalone thin wrappers | Internal only, 0 dispatch sites | None |

Each release is independently deployable. If v1.1.14 ships but v1.1.15 is delayed, deploy.sh works: gold via `ndp`, validate via `ndp-validate`.

### Manifests

```json
// v1.1.14
{
  "version": "1.0",
  "release_version": "1.1.14",
  "description": "Release v1.1.14: Gold DDL generation consolidated into ndp-lib and ndp CLI",
  "changes": [
    {"type": "tool", "id": "ndp-cli", "action": "build", "profile": "release"}
  ]
}
```

```json
// v1.1.15
{
  "version": "1.0",
  "release_version": "1.1.15",
  "description": "Release v1.1.15: Config validation consolidated into ndp-lib and ndp CLI",
  "changes": [
    {"type": "tool", "id": "ndp-cli", "action": "build", "profile": "release"}
  ]
}
```

```json
// v1.1.16
{
  "version": "1.0",
  "release_version": "1.1.16",
  "description": "Release v1.1.16: Shared constants, cross-cutting validation, deduplication",
  "changes": [
    {"type": "tool", "id": "ndp-cli", "action": "build", "profile": "release"}
  ]
}
```

---

## Test Strategy

### Test Migration

| Source | Tests | Destination | Release |
|--------|-------|-------------|---------|
| tools/ndp-gold-ddl/src/ (unit) | ~340 | crates/ndp-lib/src/gold/ | v1.1.14 |
| tools/ndp-gold-ddl/tests/ (integration) | ~36 | crates/ndp-lib/tests/gold/ | v1.1.14 |
| tools/ndp-validate/src/ (unit) | 217 | crates/ndp-lib/src/validate/ | v1.1.15 |
| tools/ndp-cli (NoOpDbClient) | 0 | crates/ndp-lib/src/db.rs | v1.1.16 |

### New Tests

| Test | Description | Release |
|------|-------------|---------|
| `ndp gold` CLI parity | Output matches standalone `ndp-gold-ddl` for all test configs | v1.1.14 |
| `ndp validate` CLI parity | Output matches standalone `ndp-validate` for all test configs | v1.1.15 |
| Cross-cutting validation | `gold::sync()` with invalid config returns validation error | v1.1.16 |
| Cross-cutting bypass | `gold::sync()` with `validate: false` skips validation | v1.1.16 |
| deploy.sh integration | Full `deploy.sh apply` with only `ndp` binary available | v1.1.15 |

### Verification Commands

```bash
# v1.1.14 — Gold parity
diff <(ndp-gold-ddl generate --stream air-quality --config-dir config/base) \
     <(ndp gold generate --stream air-quality --config-dir config/base)

# v1.1.15 — Validate parity
diff <(ndp-validate --all --config-dir config/base/streams) \
     <(ndp validate --all --config-dir config/base/streams)

# Both — Full integration
DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/v1.1.15.manifest.json
```

---

## Integration Environment

Same as ops-001/ops-002: `docker-compose.integration.yml` stack.

```bash
# 1. Start stack
docker compose -f docker-compose.integration.yml up -d

# 2. Build ndp with new modules
cargo build -p ndp-cli

# 3. Run full deploy
DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/v1.1.14.manifest.json

# 4. Verify all phases complete
# Phase 1: ndp validate --all (v1.1.15+; ndp-validate until then)
# Phase 5: ndp gold sync --stream air-quality --db-url ...
# Phase 8: ndp dimension sync ...
# Phase 9: ndp dictionary sync ... && ndp domain sync ...
```

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| ndp-lib crate | Exists (ops-001) | Add gold/ and validate/ modules |
| ndp-cli crate | Exists (ops-001) | Add gold.rs and validate.rs commands |
| ndp-gold-ddl crate | Exists | Source of gold module migration |
| ndp-validate crate | Exists | Source of validate module migration |
| ndp-types crate | Exists | Shared types, schema generation |
| jsonschema | In ndp-validate | Moves with validate module (v1.1.15) |
| sqlparser | In ndp-validate | Moves with validate module (v1.1.15) |
| schemars | In ndp-validate | Moves with validate module (v1.1.15) |
| strsim | In ndp-validate | Moves with validate module (v1.1.15) |
| mockall | In ndp-gold-ddl dev-deps | Moves with gold tests (v1.1.14) |

### Cargo.toml Changes (cumulative)

```toml
# crates/ndp-lib/Cargo.toml — v1.1.14 adds:
mockall = "0.12"          # dev-dependency (gold tests)
pretty_assertions = "1"   # dev-dependency (gold tests)

# crates/ndp-lib/Cargo.toml — v1.1.15 adds:
jsonschema = "0.17"
sqlparser = { version = "0.50", features = ["visitor"] }
schemars = "0.8"
strsim = "0.11"
serde_yaml = "0.9"
regex = "1"
sha2 = "0.10"             # dev-dependency (golden master tests)
```

---

## References

- [10-CLI-UX-DESIGN-REVISED.md](../../research/deployment/10-CLI-UX-DESIGN-REVISED.md) — Command structure and UX design
- [09-STEPWISE-MIGRATION-PLAN.md](../../research/deployment/09-STEPWISE-MIGRATION-PLAN.md) — Original migration plan (ops-003 is accelerated Phases 2-3)
- [analysis/](analysis/) — Current state analysis, duplication audit, risk assessment
- [ops-001/SCOPE.md](../ops-001/SCOPE.md) — Foundation: ndp-lib, ndp-cli, dictionary/dimension sync
- [ops-002/SCOPE.md](../ops-002/SCOPE.md) — Config-driven generators, domain sync
- [RELEASE-POLICY.md](../../../docs/procedures/RELEASE-POLICY.md) — Versioning standard
- [DEPLOYMENT-DECLARATIVES.md](../../../docs/procedures/DEPLOYMENT-DECLARATIVES.md) — Manifest format
