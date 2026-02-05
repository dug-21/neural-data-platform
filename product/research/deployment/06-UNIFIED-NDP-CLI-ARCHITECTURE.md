# Unified NDP CLI Architecture

> **Addendum to Deployment Research**
> **Date**: 2026-02-05
> **Status**: Recommended Architecture Revision

---

## Executive Summary

This document revises the original hybrid recommendation based on a critical insight:

> **Build a unified `ndp` CLI with componentized library that exposes functionality through BOTH CLI commands AND MCP tools.**

This changes the recommendation from "multiple Rust tools" to **single unified platform CLI** with shared business logic.

---

## The Insight

Instead of:
```
ndp-gold-ddl     → standalone binary
ndp-validate     → standalone binary
ndp-dictionary   → standalone binary (proposed)
ndp-config       → standalone binary (proposed)
ndp-deploy       → standalone binary (proposed)
ndp-mcp-server   → separate MCP server
```

Build:
```
ndp              → single CLI with subcommands
                   └── shared library with all logic
                       └── exposed via CLI AND MCP
```

---

## Why This Is Better

| Aspect | Multiple Tools | Unified CLI |
|--------|---------------|-------------|
| **Binaries to deploy** | 5-6 separate | 1 single |
| **Disk footprint** | ~50MB (5×10MB) | ~15MB (shared deps) |
| **Code duplication** | Config parsing in each | One config module |
| **Testing** | Test each tool | Test library once |
| **Consistency** | May diverge | Guaranteed same behavior |
| **CLI UX** | `ndp-gold-ddl`, `ndp-validate`, ... | `ndp gold`, `ndp validate`, ... |
| **MCP integration** | Separate server | Built into CLI: `ndp mcp serve` |
| **Discoverability** | Must know tool names | `ndp --help` shows all |

---

## Proposed Architecture

### Crate Structure

```
workspace/
├── Cargo.toml                    # Workspace root
│
├── ndp-lib/                      # Library crate - ALL business logic
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── manifest/             # Manifest parsing, validation, diffing
│       │   ├── mod.rs
│       │   ├── parser.rs
│       │   ├── validator.rs
│       │   └── diff.rs
│       ├── config/               # Configuration management
│       │   ├── mod.rs
│       │   ├── stream.rs
│       │   ├── etcd.rs
│       │   └── schema.rs
│       ├── deploy/               # Deployment orchestration
│       │   ├── mod.rs
│       │   ├── phases.rs
│       │   ├── executor.rs
│       │   └── state.rs
│       ├── gold/                 # Gold layer DDL (from ndp-gold-ddl)
│       │   ├── mod.rs
│       │   ├── generator.rs
│       │   └── aggregates.rs
│       ├── silver/               # Silver layer operations
│       │   ├── mod.rs
│       │   ├── ddl.rs
│       │   └── etl.rs
│       ├── dictionary/           # Data dictionary sync
│       │   ├── mod.rs
│       │   ├── sync.rs
│       │   └── schema.rs
│       ├── validate/             # Validation (from ndp-validate)
│       │   ├── mod.rs
│       │   └── config.rs
│       └── db/                   # Database utilities
│           ├── mod.rs
│           ├── postgres.rs
│           └── migrations.rs
│
├── ndp-cli/                      # CLI binary
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       └── commands/
│           ├── mod.rs
│           ├── deploy.rs         # ndp deploy [apply|status|diff|watch]
│           ├── config.rs         # ndp config [sync|get|set|list]
│           ├── gold.rs           # ndp gold [generate|apply|status]
│           ├── silver.rs         # ndp silver [migrate|etl|status]
│           ├── validate.rs       # ndp validate [config|manifest|all]
│           ├── dictionary.rs     # ndp dictionary [sync|query|describe]
│           └── mcp.rs            # ndp mcp [serve|tools]
│
└── ndp-mcp/                      # MCP server (optional separate, or in CLI)
    ├── Cargo.toml
    └── src/
        ├── main.rs               # Or: CLI spawns this via `ndp mcp serve`
        └── tools/                # MCP tools calling ndp-lib
            ├── mod.rs
            ├── deploy_tools.rs   # deploy_apply, deploy_status, deploy_diff
            ├── config_tools.rs   # config_sync, stream_list, stream_get
            ├── gold_tools.rs     # gold_generate, gold_apply
            └── query_tools.rs    # Existing MCP tools
```

### Key Design Principle: Library-First

```rust
// ndp-lib/src/deploy/mod.rs
pub struct DeployEngine {
    manifest: Manifest,
    state: DeviceState,
    db: PostgresPool,
    etcd: EtcdClient,
}

impl DeployEngine {
    /// Apply a deployment manifest
    /// Called by BOTH CLI and MCP
    pub async fn apply(&self, manifest_path: &Path, opts: ApplyOptions) -> Result<DeployResult> {
        // Shared implementation
    }

    /// Get deployment diff
    pub async fn diff(&self, manifest_path: &Path) -> Result<Vec<Change>> {
        // Shared implementation
    }

    /// Watch for changes and auto-deploy
    pub async fn watch(&self, config_dir: &Path) -> Result<()> {
        // V2.0 feature
    }
}
```

```rust
// ndp-cli/src/commands/deploy.rs
use ndp_lib::deploy::DeployEngine;

#[derive(Parser)]
pub struct DeployCommand {
    #[command(subcommand)]
    action: DeployAction,
}

#[derive(Subcommand)]
enum DeployAction {
    Apply { manifest: PathBuf, #[arg(long)] dry_run: bool },
    Status,
    Diff { manifest: PathBuf },
    Watch { config_dir: PathBuf },
}

impl DeployCommand {
    pub async fn run(&self, engine: &DeployEngine) -> Result<()> {
        match &self.action {
            DeployAction::Apply { manifest, dry_run } => {
                let opts = ApplyOptions { dry_run: *dry_run };
                let result = engine.apply(manifest, opts).await?;
                // CLI-specific output formatting
            }
            // ...
        }
    }
}
```

```rust
// ndp-mcp/src/tools/deploy_tools.rs
use ndp_lib::deploy::DeployEngine;

pub async fn deploy_apply(engine: &DeployEngine, params: DeployApplyParams) -> McpResult {
    let opts = ApplyOptions { dry_run: params.dry_run };
    let result = engine.apply(&params.manifest_path, opts).await?;
    // MCP-specific response formatting
    McpResult::success(serde_json::to_value(result)?)
}
```

---

## CLI Command Structure

```
ndp - Neural Data Platform CLI

USAGE:
    ndp <COMMAND>

COMMANDS:
    deploy      Deployment operations
    config      Configuration management
    gold        Gold layer operations
    silver      Silver layer operations
    validate    Validation commands
    dictionary  Data dictionary operations
    mcp         MCP server operations
    status      System status
    help        Print help

─────────────────────────────────────────────────────────

ndp deploy

USAGE:
    ndp deploy <COMMAND>

COMMANDS:
    apply       Apply a deployment manifest
    status      Show current deployment state
    diff        Show changes between manifest and current state
    watch       Watch for config changes and auto-deploy (V2.0)
    rollback    Rollback to previous deployment

─────────────────────────────────────────────────────────

ndp config

USAGE:
    ndp config <COMMAND>

COMMANDS:
    sync        Sync all configurations to etcd
    get         Get a configuration value
    set         Set a configuration value
    list        List all streams/configs

─────────────────────────────────────────────────────────

ndp mcp

USAGE:
    ndp mcp <COMMAND>

COMMANDS:
    serve       Start MCP server (stdio or HTTP)
    tools       List available MCP tools
```

---

## MCP Integration

### Option A: MCP Server in CLI Binary

```bash
# CLI commands
ndp deploy apply manifest.json
ndp config sync

# Start MCP server (same binary!)
ndp mcp serve --transport stdio
ndp mcp serve --transport http --port 3000
```

**Pros**: Single binary, shared dependencies
**Cons**: Slightly larger binary for CLI-only users

### Option B: Separate MCP Binary, Shared Library

```bash
# CLI binary
ndp deploy apply manifest.json

# MCP binary (separate)
ndp-mcp-server --transport stdio
```

**Pros**: Smaller CLI binary
**Cons**: Two binaries to manage

### Recommendation: Option A

For edge deployment simplicity, a single `ndp` binary that can operate in both CLI and MCP server modes is cleaner:

```bash
# On Pi, Claude Code calls:
ndp deploy apply .deploy/releases/v1.2.0.manifest.json

# For MCP integration (stdio):
ndp mcp serve

# For MCP integration (HTTP, e.g., Grafana integration):
ndp mcp serve --transport http --port 8080
```

---

## Migration Path

### Phase 1: Create ndp-lib (Weeks 1-3)

1. Create workspace with `ndp-lib` crate
2. Move `ndp-gold-ddl` logic to `ndp-lib/src/gold/`
3. Move `ndp-validate` logic to `ndp-lib/src/validate/`
4. Both existing binaries become thin wrappers calling lib

```rust
// tools/ndp-gold-ddl/src/main.rs (temporary wrapper)
use ndp_lib::gold::GoldDdlGenerator;

fn main() {
    // Delegate to library
    let generator = GoldDdlGenerator::new();
    generator.run(args);
}
```

### Phase 2: Create ndp CLI (Weeks 4-6)

1. Create `ndp-cli` crate with clap
2. Implement subcommands calling `ndp-lib`
3. Deprecate separate `ndp-gold-ddl`, `ndp-validate` binaries

```bash
# Old (deprecated)
ndp-gold-ddl generate --stream air-quality

# New
ndp gold generate --stream air-quality
```

### Phase 3: Add Deploy Functionality (Weeks 7-10)

1. Implement `ndp-lib/src/deploy/` module
2. Implement `ndp-lib/src/dictionary/` module
3. Implement `ndp-lib/src/config/` module
4. Add CLI commands for each

### Phase 4: MCP Integration (Weeks 11-14)

1. Add `ndp mcp serve` command
2. Create MCP tools that call `ndp-lib` functions
3. Migrate existing `ndp-mcp-server` tools to new architecture
4. Deprecate separate `ndp-mcp-server` binary

### Phase 5: deploy.sh Replacement (Weeks 15-18)

1. `deploy.sh` becomes thin wrapper calling `ndp deploy`
2. Gradually move all phases to Rust
3. Eventually: `deploy.sh` is just `#!/bin/bash\nndp deploy apply "$@"`

---

## Impact on Original Recommendations

| Original Recommendation | Revised Recommendation |
|------------------------|----------------------|
| Multiple Rust tools (ndp-manifest, ndp-dictionary, ndp-config) | Single `ndp` CLI with subcommands |
| Keep ndp-mcp-server separate | Integrate MCP into `ndp mcp serve` |
| Bash orchestrator + Rust tools | `ndp deploy` replaces Bash orchestrator |
| 4-phase evolution | 5-phase evolution (cleaner end state) |

---

## Final Architecture Vision

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        USER INTERFACES                                   │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐    │
│   │   CLI Terminal   │    │   Claude Code    │    │   Grafana/Web   │    │
│   │                  │    │   (MCP Client)   │    │   (HTTP API)    │    │
│   └────────┬─────────┘    └────────┬─────────┘    └────────┬─────────┘    │
│            │                       │                       │              │
│            │ ndp deploy ...        │ MCP tools/call        │ HTTP API     │
│            │                       │                       │              │
│            ▼                       ▼                       ▼              │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │                        ndp (single binary)                       │   │
│   │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │   │
│   │  │  CLI Mode   │  │  MCP Mode   │  │  HTTP Mode  │              │   │
│   │  │  (default)  │  │  (serve)    │  │  (serve)    │              │   │
│   │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘              │   │
│   │         │                │                │                      │   │
│   │         └────────────────┼────────────────┘                      │   │
│   │                          │                                       │   │
│   │                          ▼                                       │   │
│   │  ┌───────────────────────────────────────────────────────────┐  │   │
│   │  │                    ndp-lib (library)                       │  │   │
│   │  │                                                            │  │   │
│   │  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐     │  │   │
│   │  │  │ deploy   │ │ config   │ │ gold     │ │ silver   │     │  │   │
│   │  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘     │  │   │
│   │  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐     │  │   │
│   │  │  │dictionary│ │ validate │ │ manifest │ │ db       │     │  │   │
│   │  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘     │  │   │
│   │  │                                                            │  │   │
│   │  └───────────────────────────────────────────────────────────┘  │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
├─────────────────────────────────────────────────────────────────────────┤
│                        INFRASTRUCTURE                                    │
│                                                                          │
│   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐   │
│   │    etcd     │  │ TimescaleDB │  │   Docker    │  │  Filesystem │   │
│   └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Revised Success Criteria

### End State
- [ ] Single `ndp` binary handles all operations
- [ ] `ndp deploy apply` replaces `deploy.sh apply`
- [ ] `ndp mcp serve` replaces `ndp-mcp-server`
- [ ] All functionality accessible via CLI or MCP
- [ ] Pi deployment: single ~15MB binary + Docker

### V2.0 Goal Achieved
```bash
# The dream command
ndp deploy --watch

# Detects config changes, computes diff, applies intelligently
# Accessible via MCP for Claude Code orchestration
```

---

## Conclusion

The unified CLI architecture is **the right long-term approach**. It:

1. **Simplifies deployment** - One binary instead of many
2. **Ensures consistency** - Same logic for CLI and MCP
3. **Enables the V2.0 vision** - Intelligent, watchable deployments
4. **Reduces maintenance** - One codebase, one test suite
5. **Improves UX** - `ndp <command>` is intuitive

**This should become the primary architecture target**, with the Bash modularization (Phase 1 of original plan) as a short-term bridge while the Rust CLI is built.
