# Stepwise Migration Plan: Unified NDP CLI

> **Date**: 2026-02-05
> **Goal**: Migrate to unified `ndp` CLI while maintaining full functionality
> **Constraint**: Zero downtime, no broken capabilities at any step

---

## Executive Summary

This plan migrates from the current architecture to a unified `ndp` CLI in **8 phases** over approximately **16-20 weeks**. Each phase is independently deployable and maintains full functionality of both deployment and MCP capabilities.

### Current State

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          CURRENT ARCHITECTURE                            │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  deploy.sh (2,868 lines)          ndp-mcp-server (separate binary)      │
│  ────────────────────────         ─────────────────────────────────     │
│  • 11 deployment phases           • 14 MCP tools                        │
│  • Manifest parsing               • Bronze layer tools                  │
│  • DDL generation                 • Silver layer tools                  │
│  • etcd sync                      • ETL observability                   │
│  • Container management           • Dictionary tools                    │
│                                                                          │
│  ndp-gold-ddl (standalone)        ndp-validate (standalone)             │
│  ─────────────────────────        ──────────────────────────            │
│  • Gold DDL generation            • Config validation                   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Target State

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          TARGET ARCHITECTURE                             │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│                              ndp (unified CLI)                           │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │                                                                    │  │
│  │  CLI Mode                         MCP Mode                        │  │
│  │  ─────────                        ────────                        │  │
│  │  ndp deploy apply                 ndp mcp serve                   │  │
│  │  ndp bronze list                  → bronze_list                   │  │
│  │  ndp silver describe              → silver_describe               │  │
│  │  ndp config sync                  → config_sync                   │  │
│  │                                                                    │  │
│  │                         ndp-lib                                   │  │
│  │  ┌────────────────────────────────────────────────────────────┐  │  │
│  │  │ bronze/ silver/ gold/ stream/ deploy/ config/ dictionary/ │  │  │
│  │  └────────────────────────────────────────────────────────────┘  │  │
│  │                                                                    │  │
│  └───────────────────────────────────────────────────────────────────┘  │
│                                                                          │
│  deploy.sh (thin wrapper)                                                │
│  ────────────────────────                                                │
│  #!/bin/bash                                                             │
│  ndp deploy "$@"                                                         │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Migration Principles

### 1. Additive Changes First

Add new capabilities before removing old ones. Never remove until new path is proven.

### 2. Parallel Operation

Run old and new implementations side-by-side during transition.

### 3. Feature Flags

Use environment variables to switch between old/new implementations.

### 4. Automated Testing

Each phase includes tests that verify both old and new paths work.

### 5. Rollback Ready

Every phase can be rolled back by reverting a single commit/tag.

---

## Phase Overview

| Phase | Duration | Focus | Risk | Functionality |
|-------|----------|-------|------|---------------|
| **1** | 2 weeks | Create ndp-lib foundation | Low | deploy.sh ✅, MCP ✅ |
| **2** | 2 weeks | Migrate existing Rust tools to ndp-lib | Low | deploy.sh ✅, MCP ✅ |
| **3** | 2 weeks | Create ndp CLI skeleton | Low | deploy.sh ✅, MCP ✅, ndp CLI (partial) |
| **4** | 3 weeks | Migrate MCP tools to ndp-lib | Medium | deploy.sh ✅, MCP ✅ (both paths) |
| **5** | 2 weeks | Add ndp mcp serve | Low | deploy.sh ✅, MCP ✅ (old + new) |
| **6** | 3 weeks | Add ndp deploy commands | Medium | deploy.sh ✅, ndp deploy (parallel) |
| **7** | 2 weeks | CI/CD integration | Low | All ✅, faster builds |
| **8** | 2 weeks | Deprecate old paths | Low | ndp CLI primary, wrappers for compat |

---

## Phase 1: Create ndp-lib Foundation (Weeks 1-2)

### Goal
Create the shared library crate with core types and traits. No functionality changes.

### Changes

```
workspace/
├── Cargo.toml                    # Add workspace
├── ndp-lib/                      # NEW
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── error.rs              # Unified error types
│       ├── types.rs              # Core types (StreamInfo, etc.)
│       ├── traits.rs             # Storage, ConfigStore traits
│       └── db/
│           ├── mod.rs
│           ├── postgres.rs       # Connection pool
│           └── etcd.rs           # etcd client wrapper
├── core/
│   └── ndp-mcp-server/           # UNCHANGED
├── tools/
│   ├── ndp-gold-ddl/             # UNCHANGED
│   └── ndp-validate/             # UNCHANGED
└── deploy/
    └── pi/
        └── deploy.sh             # UNCHANGED
```

### Tasks

1. **Create Cargo workspace**
   ```toml
   # Cargo.toml (root)
   [workspace]
   members = [
       "ndp-lib",
       "core/ndp-mcp-server",
       "tools/ndp-gold-ddl",
       "tools/ndp-validate",
   ]
   ```

2. **Create ndp-lib with shared types**
   - Move common types from ndp-mcp-server
   - Define traits for storage/config access
   - No breaking changes to existing code

3. **Update existing crates to use ndp-lib**
   - ndp-mcp-server depends on ndp-lib for types
   - Existing functionality unchanged

### Verification

```bash
# All existing tests pass
cargo test --workspace

# deploy.sh still works
./deploy.sh status

# MCP server still works
curl http://localhost:3000/health
```

### Rollback

Remove ndp-lib from workspace, revert Cargo.toml changes.

---

## Phase 2: Migrate Existing Rust Tools to ndp-lib (Weeks 3-4)

### Goal

Move logic from `ndp-gold-ddl` and `ndp-validate` into `ndp-lib`. Keep binaries as thin wrappers.

### Changes

```
ndp-lib/
└── src/
    ├── gold/                     # NEW - from ndp-gold-ddl
    │   ├── mod.rs
    │   ├── generator.rs
    │   ├── aggregates.rs
    │   └── domains.rs
    └── validate/                 # NEW - from ndp-validate
        ├── mod.rs
        ├── config.rs
        └── schema.rs

tools/
├── ndp-gold-ddl/
│   └── src/
│       └── main.rs               # SIMPLIFIED - calls ndp_lib::gold
└── ndp-validate/
    └── src/
        └── main.rs               # SIMPLIFIED - calls ndp_lib::validate
```

### Tasks

1. **Move gold DDL logic to ndp-lib**
   ```rust
   // ndp-lib/src/gold/mod.rs
   pub mod generator;
   pub mod aggregates;

   pub use generator::GoldDdlGenerator;
   ```

2. **Move validation logic to ndp-lib**
   ```rust
   // ndp-lib/src/validate/mod.rs
   pub mod config;
   pub mod schema;

   pub use config::ConfigValidator;
   ```

3. **Simplify tool binaries**
   ```rust
   // tools/ndp-gold-ddl/src/main.rs
   use ndp_lib::gold::GoldDdlGenerator;

   fn main() {
       let generator = GoldDdlGenerator::from_args();
       generator.run();
   }
   ```

### Verification

```bash
# Existing tool behavior unchanged
ndp-gold-ddl generate --stream air-quality --dry-run
ndp-validate --all

# deploy.sh still calls tools correctly
./deploy.sh apply --dry-run .deploy/manifest.json
```

### Rollback

Revert to standalone tool implementations.

---

## Phase 3: Create ndp CLI Skeleton (Weeks 5-6)

### Goal

Create the `ndp` CLI binary with basic commands. Does not replace anything yet.

### Changes

```
workspace/
├── ndp-cli/                      # NEW
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── context.rs            # Shared CLI context
│       └── commands/
│           ├── mod.rs
│           ├── gold.rs           # ndp gold (wraps ndp-lib::gold)
│           ├── validate.rs       # ndp validate (wraps ndp-lib::validate)
│           ├── status.rs         # ndp status (new)
│           └── version.rs        # ndp version
```

### Tasks

1. **Create ndp-cli crate**
   ```toml
   # ndp-cli/Cargo.toml
   [package]
   name = "ndp"
   version = "0.1.0"

   [[bin]]
   name = "ndp"
   path = "src/main.rs"

   [dependencies]
   ndp-lib = { path = "../ndp-lib" }
   clap = { version = "4", features = ["derive"] }
   ```

2. **Implement initial commands**
   ```rust
   // ndp-cli/src/main.rs
   use clap::{Parser, Subcommand};

   #[derive(Parser)]
   #[command(name = "ndp", about = "Neural Data Platform CLI")]
   struct Cli {
       #[command(subcommand)]
       command: Commands,
   }

   #[derive(Subcommand)]
   enum Commands {
       Gold(GoldCommand),
       Validate(ValidateCommand),
       Status,
       Version,
   }
   ```

3. **Add to CI build**
   - Build `ndp` binary in CI
   - Do NOT deploy to Pi yet

### Verification

```bash
# New CLI works
ndp --help
ndp gold generate --stream air-quality --dry-run
ndp validate --all
ndp status

# Old tools still work (unchanged)
ndp-gold-ddl generate --stream air-quality
ndp-validate --all

# deploy.sh unchanged
./deploy.sh status
```

### Rollback

Remove ndp-cli from workspace. Existing tools unaffected.

---

## Phase 4: Migrate MCP Tools to ndp-lib (Weeks 7-9)

### Goal

Move MCP tool logic into ndp-lib. MCP server calls library functions.

### Changes

```
ndp-lib/
└── src/
    ├── bronze/                   # NEW - from MCP tools
    │   ├── mod.rs
    │   ├── list.rs               # list_streams logic
    │   ├── describe.rs           # describe_schema logic
    │   ├── sample.rs             # sample_data logic
    │   └── validate.rs           # validate_config logic
    ├── silver/                   # NEW - from MCP tools
    │   ├── mod.rs
    │   ├── list.rs
    │   ├── describe.rs
    │   ├── sample.rs
    │   └── stats.rs
    ├── etl/                      # NEW - from MCP tools
    │   ├── mod.rs
    │   ├── status.rs
    │   ├── history.rs
    │   └── freshness.rs
    └── dictionary/               # NEW - from MCP tools
        ├── mod.rs
        ├── list.rs
        ├── query.rs
        ├── describe.rs
        └── trace.rs

core/ndp-mcp-server/
└── src/
    └── mcp/
        └── tools/
            ├── list_streams.rs   # SIMPLIFIED - calls ndp_lib::bronze::list
            ├── describe_schema.rs
            └── ...
```

### Tasks

1. **Extract bronze tools to ndp-lib**
   ```rust
   // ndp-lib/src/bronze/list.rs
   pub async fn list(
       storage: &impl BronzeStorage,
       config: &impl ConfigStore,
   ) -> Result<Vec<StreamInfo>> {
       // Logic moved from MCP tool
   }
   ```

2. **Update MCP tools to call library**
   ```rust
   // core/ndp-mcp-server/src/mcp/tools/list_streams.rs
   use ndp_lib::bronze;

   pub async fn execute(ctx: &Context) -> McpResult<McpToolResult> {
       let streams = bronze::list(&ctx.storage, &ctx.config).await?;
       McpToolResult::success(&streams)
   }
   ```

3. **Maintain backward compatibility**
   - Same MCP tool names
   - Same request/response formats
   - Same behavior

### Verification

```bash
# MCP tools still work identically
curl -X POST http://localhost:3000/mcp \
  -d '{"tool": "list_streams"}'

# Compare old vs new output
diff <(old_mcp list_streams) <(new_mcp list_streams)

# deploy.sh unchanged
./deploy.sh status
```

### Rollback

Revert MCP tools to inline implementations.

---

## Phase 5: Add ndp mcp serve (Weeks 10-11)

### Goal

Add MCP server capability to `ndp` CLI. Run both old and new servers in parallel for validation.

### Changes

```
ndp-cli/
└── src/
    └── commands/
        ├── mcp.rs                # NEW - ndp mcp serve
        └── ...

ndp-lib/
└── src/
    └── mcp/                      # NEW - MCP protocol handling
        ├── mod.rs
        ├── server.rs
        ├── protocol.rs
        └── tools/
            ├── mod.rs
            ├── bronze.rs         # Wraps ndp_lib::bronze::*
            ├── silver.rs         # Wraps ndp_lib::silver::*
            └── ...
```

### Tasks

1. **Add MCP server to ndp-lib**
   ```rust
   // ndp-lib/src/mcp/server.rs
   pub struct McpServer {
       tools: ToolRegistry,
   }

   impl McpServer {
       pub async fn serve_stdio(&self) -> Result<()> { ... }
       pub async fn serve_http(&self, port: u16) -> Result<()> { ... }
   }
   ```

2. **Add ndp mcp command**
   ```rust
   // ndp-cli/src/commands/mcp.rs
   #[derive(Subcommand)]
   enum McpCommand {
       Serve {
           #[arg(long, default_value = "stdio")]
           transport: String,
           #[arg(long, default_value = "3000")]
           port: u16,
       },
       Tools,
   }
   ```

3. **Run parallel validation**
   ```bash
   # Old server on port 3000
   ndp-mcp-server --port 3000

   # New server on port 3001
   ndp mcp serve --transport http --port 3001

   # Compare outputs
   ./scripts/compare-mcp-outputs.sh
   ```

### Verification

```bash
# Both servers respond identically
curl http://localhost:3000/mcp -d '{"tool": "list_streams"}'
curl http://localhost:3001/mcp -d '{"tool": "list_streams"}'

# New CLI MCP works
ndp mcp serve --transport stdio

# Old server still works
ndp-mcp-server

# deploy.sh unchanged
./deploy.sh status
```

### Rollback

Remove mcp command from ndp-cli. Old server unaffected.

---

## Phase 6: Add ndp deploy Commands (Weeks 12-14)

### Goal

Implement deployment commands in `ndp` CLI. Run parallel with deploy.sh for validation.

### Changes

```
ndp-lib/
└── src/
    ├── deploy/                   # NEW
    │   ├── mod.rs
    │   ├── manifest.rs           # Manifest parsing
    │   ├── phases.rs             # 11 deployment phases
    │   ├── executor.rs           # Phase execution
    │   └── state.rs              # Device state
    ├── config/                   # NEW
    │   ├── mod.rs
    │   ├── sync.rs               # etcd sync
    │   └── stream.rs             # Stream config
    ├── stream/                   # NEW
    │   ├── mod.rs
    │   ├── list.rs
    │   └── describe.rs
    └── objective/                # NEW
        ├── mod.rs
        └── ...

ndp-cli/
└── src/
    └── commands/
        ├── deploy.rs             # ndp deploy apply|status|diff
        ├── config.rs             # ndp config sync|get|set
        ├── stream.rs             # ndp stream list|describe
        ├── bronze.rs             # ndp bronze list|describe|sample
        ├── silver.rs             # ndp silver list|describe|sample
        └── ...
```

### Tasks

1. **Implement deploy module in ndp-lib**
   ```rust
   // ndp-lib/src/deploy/mod.rs
   pub struct DeployEngine { ... }

   impl DeployEngine {
       pub async fn apply(&self, manifest: &Path) -> Result<DeployResult> {
           // Implement all 11 phases
       }
   }
   ```

2. **Port deploy.sh logic incrementally**
   - Phase 1: Validation → `ndp_lib::deploy::validate()`
   - Phase 2: Container builds → `ndp_lib::deploy::build_containers()`
   - Phase 3: Migrations → `ndp_lib::deploy::run_migrations()`
   - etc.

3. **Add parallel execution mode**
   ```bash
   # deploy.sh with comparison mode
   DEPLOY_COMPARE=1 ./deploy.sh apply manifest.json
   # Runs both old and new, compares results
   ```

4. **Feature flag for gradual rollout**
   ```bash
   # Use old path (default)
   ./deploy.sh apply manifest.json

   # Use new path (opt-in)
   DEPLOY_ENGINE=rust ./deploy.sh apply manifest.json
   # OR
   ndp deploy apply manifest.json
   ```

### Verification

```bash
# Old path still works
./deploy.sh apply manifest.json

# New path produces same result
ndp deploy apply manifest.json

# Compare outputs
diff <(./deploy.sh status) <(ndp deploy status)

# Parallel validation
DEPLOY_COMPARE=1 ./deploy.sh apply --dry-run manifest.json
```

### Rollback

Disable DEPLOY_ENGINE=rust flag. deploy.sh continues working.

---

## Phase 7: CI/CD Integration (Weeks 15-16)

### Goal

Move builds to CI/CD. Deploy pre-built `ndp` binary to Pi.

### Changes

```
.github/
└── workflows/
    ├── build-ndp.yml             # NEW - Build ndp binary
    └── release.yml               # UPDATED - Include ndp in artifacts

deploy/
└── pi/
    ├── deploy.sh                 # UPDATED - Use pre-built ndp
    └── deploy-from-ci.sh         # NEW - Minimal deployment script
```

### Tasks

1. **Create CI workflow for ndp binary**
   ```yaml
   # .github/workflows/build-ndp.yml
   name: Build NDP CLI
   on:
     push:
       tags: ['v*']
   jobs:
     build:
       runs-on: ubuntu-latest
       steps:
         - uses: actions/checkout@v4
         - name: Build for ARM64
           run: |
             cargo build --release --target aarch64-unknown-linux-gnu
         - uses: actions/upload-artifact@v4
           with:
             name: ndp-arm64
             path: target/aarch64-unknown-linux-gnu/release/ndp
   ```

2. **Update release workflow**
   - Include `ndp` binary in release artifacts
   - Create deployment bundle

3. **Create minimal deployment script**
   ```bash
   # deploy/pi/deploy-from-ci.sh
   #!/bin/bash
   # Deploys from CI-built artifacts

   BUNDLE="$1"
   tar -xzf "$BUNDLE"

   # Install ndp binary
   cp ./bin/ndp /usr/local/bin/ndp
   chmod +x /usr/local/bin/ndp

   # Run deployment
   ndp deploy apply ./manifest.json
   ```

### Verification

```bash
# CI builds succeed
gh run view --log

# Pre-built binary works on Pi
scp ndp-arm64 pi:/usr/local/bin/ndp
ssh pi 'ndp --version'

# Deployment works with pre-built binary
ssh pi 'ndp deploy apply manifest.json'

# Old path still works (fallback)
ssh pi './deploy.sh apply manifest.json'
```

### Rollback

Use deploy.sh without pre-built binary (builds on Pi).

---

## Phase 8: Deprecate Old Paths (Weeks 17-18)

### Goal

Make `ndp` CLI the primary interface. Keep backwards-compatible wrappers.

### Changes

```
deploy/
└── pi/
    ├── deploy.sh                 # Wrapper → ndp deploy
    └── lib/                      # REMOVED (logic in ndp-lib)

tools/
├── ndp-gold-ddl                  # Wrapper → ndp gold
└── ndp-validate                  # Wrapper → ndp validate

core/
└── ndp-mcp-server/               # DEPRECATED → ndp mcp serve
```

### Tasks

1. **Convert deploy.sh to wrapper**
   ```bash
   #!/bin/bash
   # deploy.sh - Backwards compatibility wrapper
   # All logic now in: ndp deploy

   echo "NOTE: deploy.sh is deprecated. Use 'ndp deploy' directly."

   case "$1" in
       apply)   ndp deploy apply "${@:2}" ;;
       status)  ndp deploy status ;;
       start)   ndp deploy start ;;
       stop)    ndp deploy stop ;;
       *)       ndp deploy "$@" ;;
   esac
   ```

2. **Convert tool binaries to wrappers**
   ```bash
   # tools/ndp-gold-ddl/ndp-gold-ddl (shell script)
   #!/bin/bash
   echo "NOTE: ndp-gold-ddl is deprecated. Use 'ndp gold' directly."
   ndp gold "$@"
   ```

3. **Deprecate standalone MCP server**
   ```bash
   # core/ndp-mcp-server/run.sh
   #!/bin/bash
   echo "NOTE: ndp-mcp-server is deprecated. Use 'ndp mcp serve' directly."
   ndp mcp serve "$@"
   ```

4. **Update documentation**
   - Mark old commands as deprecated
   - Document new `ndp` CLI
   - Update README, procedures

### Verification

```bash
# Old commands still work (with deprecation notice)
./deploy.sh status
# Output: "NOTE: deploy.sh is deprecated..."
# Then: actual status output

# New commands are primary
ndp deploy status
ndp gold generate --stream air-quality
ndp mcp serve

# All functionality preserved
./test-suite.sh
```

### Rollback

Restore full deploy.sh implementation (kept in git history).

---

## Post-Migration State

### Final Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          FINAL ARCHITECTURE                              │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ndp (primary CLI - ~15MB ARM64 binary)                                 │
│  ────────────────────────────────────────                               │
│  • ndp deploy apply|status|diff|watch                                   │
│  • ndp bronze list|describe|sample                                      │
│  • ndp silver list|describe|sample|stats|migrate                        │
│  • ndp gold list|describe|generate|apply                                │
│  • ndp stream list|get|describe|create|update|delete                    │
│  • ndp config sync|get|set|list                                         │
│  • ndp dictionary list|query|describe|trace|sync                        │
│  • ndp etl status|history|freshness|run|start|stop                      │
│  • ndp validate config|manifest|schema|all                              │
│  • ndp mcp serve|tools                                                  │
│  • ndp status system|services|data                                      │
│                                                                          │
│  Backwards Compatibility Wrappers                                        │
│  ─────────────────────────────────                                       │
│  deploy.sh     → ndp deploy "$@"                                        │
│  ndp-gold-ddl  → ndp gold "$@"                                          │
│  ndp-validate  → ndp validate "$@"                                      │
│                                                                          │
│  ndp-lib (shared library)                                               │
│  ────────────────────────                                                │
│  • All business logic                                                   │
│  • Called by CLI and MCP                                                │
│  • Unit tested independently                                            │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Pi Deployment Footprint

| Before | After | Savings |
|--------|-------|---------|
| Rust toolchain (~1GB) | Not needed | ~1GB |
| Multiple binaries (~50MB) | Single binary (~15MB) | ~35MB |
| Build time (20-35 min) | Pre-built (2-3 min) | ~90% |

---

## Risk Mitigation

### Phase-Specific Risks

| Phase | Risk | Mitigation |
|-------|------|------------|
| 1-2 | Type mismatches | Extensive unit tests |
| 4 | MCP behavior change | Output comparison tests |
| 6 | Deploy logic bugs | Parallel execution, dry-run first |
| 7 | CI/CD failures | Keep local build fallback |
| 8 | User confusion | Deprecation notices, docs |

### Rollback Triggers

Each phase has a "stop" trigger:

| Phase | Stop If... | Rollback Action |
|-------|------------|-----------------|
| 1 | Workspace breaks existing builds | Remove workspace config |
| 2 | Tool behavior changes | Revert to standalone tools |
| 3 | N/A (additive only) | Remove ndp-cli crate |
| 4 | MCP outputs differ | Revert MCP tool changes |
| 5 | New MCP server issues | Use old server |
| 6 | Deploy failures | Disable DEPLOY_ENGINE flag |
| 7 | CI builds fail | Deploy from local builds |
| 8 | Users confused | Keep full old implementations |

---

## Timeline Summary

```
Week  1-2:   Phase 1 - ndp-lib foundation
Week  3-4:   Phase 2 - Migrate existing tools
Week  5-6:   Phase 3 - Create ndp CLI skeleton
Week  7-9:   Phase 4 - Migrate MCP tools
Week 10-11:  Phase 5 - Add ndp mcp serve
Week 12-14:  Phase 6 - Add ndp deploy commands
Week 15-16:  Phase 7 - CI/CD integration
Week 17-18:  Phase 8 - Deprecate old paths

Total: ~18 weeks (4-5 months)
```

### Acceleration Options

| Option | Saves | Trade-off |
|--------|-------|-----------|
| Skip parallel validation in Phase 6 | 1-2 weeks | Higher risk |
| Delay CI/CD to post-migration | 2 weeks | Longer builds on Pi |
| Combine Phases 4-5 | 1 week | Higher complexity |

---

## Success Criteria

### Per-Phase Gates

| Phase | Gate Criteria |
|-------|---------------|
| 1 | All existing tests pass |
| 2 | Tools produce identical output |
| 3 | `ndp --help` works |
| 4 | MCP tools produce identical output |
| 5 | `ndp mcp serve` passes all MCP tests |
| 6 | `ndp deploy apply` matches `deploy.sh` output |
| 7 | CI-built binary deploys successfully |
| 8 | All deprecated commands show notices |

### Final Acceptance

- [ ] Single `ndp` binary handles all operations
- [ ] deploy.sh is thin wrapper (< 50 lines)
- [ ] MCP server via `ndp mcp serve`
- [ ] All 14 existing MCP tools work
- [ ] All deployment phases work
- [ ] CI/CD builds and deploys
- [ ] Documentation updated
- [ ] Pi deployment time < 5 minutes
