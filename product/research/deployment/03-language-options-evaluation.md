# Language Options Evaluation for Deployment Tooling Refactoring

**Document ID:** deployment/03-language-options-evaluation
**Author:** NDP Architect Agent
**Date:** 2026-02-05
**Status:** Research Complete

---

## Executive Summary

This document evaluates three options for refactoring the 2,868-line `deploy/pi/deploy.sh` script. After comprehensive analysis of Pi resource constraints, existing Rust tooling patterns, maintainability, and the declarative manifest philosophy, **Option C (Hybrid/Gradual Extraction)** is recommended as the optimal path forward.

---

## Current State Analysis

### deploy.sh Statistics

| Metric | Value |
|--------|-------|
| Total Lines | 2,868 |
| Functions | ~50+ |
| Command Handlers | 35+ |
| Declaration Types | 8 (stream, silver-table, tool, migration, gold-tables, dimensions, dictionary, container) |
| Deployment Phases | 11 |
| External Dependencies | jq, yq, python3, docker, git |

### Complexity Hotspots

1. **YAML/JSON Parsing (~300 lines)** - `yaml_get()`, `yaml_array_len()`, `yaml_array_get()` with 4 fallback mechanisms
2. **Data Dictionary Sync (~400 lines)** - Complex SQL generation from config files
3. **Domain Objectives Sync (~200 lines)** - Multi-table UPSERT generation
4. **Declarative Apply (~300 lines)** - 11-phase orchestration with error handling
5. **Silver ETL Management (~150 lines)** - Container lifecycle, daemon management

### Existing Rust Tooling Patterns

The project already has two Rust CLI tools that provide established patterns:

#### ndp-gold-ddl (tools/ndp-gold-ddl/)
- **Purpose:** Gold layer DDL generation
- **Stack:** clap 4, serde, tokio, tokio-postgres, tracing
- **Architecture:** Config loader -> Generator -> DB Client
- **Binary Size:** ~8-10MB (release, stripped)
- **Integration:** Called from deploy.sh with `--database-url` for idempotency

#### ndp-validate (tools/ndp-validate/)
- **Purpose:** Two-layer config validation (schema + semantic)
- **Stack:** clap 4, serde, jsonschema, schemars, sqlparser
- **Architecture:** Schema validator -> Semantic validator -> Output formatter
- **Binary Size:** ~6-8MB (release, stripped)
- **Integration:** Called from deploy.sh for domain validation

---

## Option A: Split into Multiple Bash Scripts

### Proposed Structure

```
deploy/pi/
├── deploy.sh              # Main orchestrator (< 200 lines)
├── lib/
│   ├── validation.sh      # Phase 1: Manifest validation
│   ├── migrations.sh      # Phase 3: Database migrations
│   ├── tools.sh           # Phase 2.5: Tool builds
│   ├── gold.sh            # Phase 5: Gold tables
│   ├── domains.sh         # Phase 6: Domain/aligned views
│   ├── streams.sh         # Phase 7: Stream config sync
│   ├── dictionary.sh      # Phase 9: Data dictionary sync
│   ├── containers.sh      # Phase 10: Container management
│   └── utils.sh           # Shared utilities (yaml_get, logging)
```

### Evaluation

| Criterion | Score | Notes |
|-----------|-------|-------|
| **Pi Resource Footprint** | **A** | Zero additional disk/memory beyond current |
| **Maintainability** | **C** | Still Bash - limited type safety, complex debugging |
| **Testability** | **D** | Bash testing is cumbersome (bats, shunit2) |
| **Onboarding** | **B** | Familiar technology, but complex YAML parsing |
| **Future Extensibility** | **C** | Adding features means more Bash complexity |
| **CI/CD Integration** | **B** | Simple shell execution, but error handling is brittle |
| **Error Handling** | **D** | Shell exit codes, no structured error types |
| **Debugging** | **D** | `set -x` tracing, echo-debugging |

### Code Example: utils.sh

```bash
#!/bin/bash
# deploy/pi/lib/utils.sh - Shared utilities

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log() { echo -e "${GREEN}[DEPLOY]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# YAML parsing with fallbacks (still required)
yaml_get() {
    local file="$1"
    local key="$2"
    local default="$3"
    # ... existing 60+ line implementation with jq/yq/python fallbacks
}
```

### Pros
- Zero additional dependencies
- Minimal change from current approach
- Familiar to any Linux admin
- Works on any system with bash

### Cons
- Does not address root complexity (YAML parsing, SQL generation)
- Testing remains difficult
- No type safety for manifest parsing
- Complex error handling patterns persist
- YAML fallback chain (jq -> yq -> python -> grep/sed) remains

---

## Option B: Migrate to a Proper Language

### Sub-Option B1: Python with Click/Typer

```python
# deploy/pi/ndp-deploy/src/ndp_deploy/cli.py
import typer
from pathlib import Path
from .phases import apply_manifest
from .models import Manifest

app = typer.Typer()

@app.command()
def apply(
    manifest: Path = typer.Argument(..., help="Manifest file path"),
    dry_run: bool = typer.Option(False, "--dry-run", help="Show what would be done")
):
    """Apply a declarative manifest."""
    m = Manifest.from_file(manifest)
    apply_manifest(m, dry_run=dry_run)

@app.command()
def status():
    """Show deployment status."""
    ...
```

| Criterion | Score | Notes |
|-----------|-------|-------|
| **Pi Resource Footprint** | **C** | Python runtime ~50-100MB RAM, 200MB+ disk |
| **Maintainability** | **B+** | Type hints, Pydantic models |
| **Testability** | **A** | pytest, rich testing ecosystem |
| **Onboarding** | **A** | Python widely known |
| **Future Extensibility** | **A** | Rich library ecosystem |
| **CI/CD Integration** | **A** | Excellent tooling |
| **Error Handling** | **A** | Exceptions, structured errors |
| **Debugging** | **A** | pdb, rich tracebacks |

### Sub-Option B2: Rust CLI Tool

```rust
// tools/ndp-deploy/src/main.rs
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ndp-deploy")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Apply a declarative manifest
    Apply {
        #[arg(short, long)]
        manifest: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Show deployment status
    Status,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Apply { manifest, dry_run } => {
            let m = Manifest::from_file(&manifest)?;
            apply_manifest(&m, dry_run).await?;
        }
        Commands::Status => status().await?,
    }
    Ok(())
}
```

| Criterion | Score | Notes |
|-----------|-------|-------|
| **Pi Resource Footprint** | **A+** | Single binary ~10MB, minimal runtime memory |
| **Maintainability** | **A** | Strong types, compile-time guarantees |
| **Testability** | **A** | Cargo test, integration test crates |
| **Onboarding** | **C** | Rust learning curve |
| **Future Extensibility** | **A** | Rich ecosystem, async support |
| **CI/CD Integration** | **A** | cargo build in CI |
| **Error Handling** | **A+** | Result types, thiserror |
| **Debugging** | **B+** | Good but less convenient than Python |

### Sub-Option B3: Go Single Binary

```go
// tools/ndp-deploy/cmd/deploy/main.go
package main

import (
    "github.com/spf13/cobra"
)

func main() {
    rootCmd := &cobra.Command{
        Use:   "ndp-deploy",
        Short: "NDP declarative deployment tool",
    }

    applyCmd := &cobra.Command{
        Use:   "apply [manifest]",
        Short: "Apply a declarative manifest",
        RunE:  runApply,
    }

    rootCmd.AddCommand(applyCmd)
    rootCmd.Execute()
}
```

| Criterion | Score | Notes |
|-----------|-------|-------|
| **Pi Resource Footprint** | **A** | Single binary ~15-20MB, low memory |
| **Maintainability** | **B+** | Good but less expressive than Rust |
| **Testability** | **A** | go test, table-driven tests |
| **Onboarding** | **B+** | Easy to learn |
| **Future Extensibility** | **B+** | Good stdlib, fewer crates |
| **CI/CD Integration** | **A** | go build |
| **Error Handling** | **B** | Error values, no Result types |
| **Debugging** | **A** | delve debugger |

### Option B Comparison Matrix

| Language | Disk | RAM | Startup | Type Safety | Ecosystem Fit |
|----------|------|-----|---------|-------------|---------------|
| **Python** | 200MB+ | 50-100MB | ~300ms | Optional | Different from NDP |
| **Rust** | ~10MB | ~5-10MB | <50ms | Strong | **Matches existing tools** |
| **Go** | ~15MB | ~10-15MB | <50ms | Strong | New to project |

**Recommendation within Option B:** Rust, due to alignment with existing `ndp-gold-ddl` and `ndp-validate` patterns.

---

## Option C: Hybrid/Gradual Extraction

### Philosophy

Keep Bash as the thin orchestration layer while delegating complex logic to purpose-built Rust tools. This follows the existing pattern where `deploy.sh` calls `ndp-gold-ddl` and `ndp-validate`.

### Proposed Architecture

```
deploy/pi/
├── deploy.sh                    # Thin orchestrator (~500 lines)
│                                # - Command routing
│                                # - Docker Compose wrapper
│                                # - Phase sequencing
│
tools/
├── ndp-gold-ddl/               # [EXISTING] Gold layer DDL
├── ndp-validate/               # [EXISTING] Config validation
├── ndp-manifest/               # [NEW] Manifest operations
│   ├── src/
│   │   ├── main.rs            # CLI entry point
│   │   ├── parse.rs           # JSON manifest parsing
│   │   ├── validate.rs        # Manifest validation
│   │   └── diff.rs            # Manifest diffing
│   └── Cargo.toml
├── ndp-dictionary/             # [NEW] Data dictionary sync
│   ├── src/
│   │   ├── main.rs
│   │   ├── bronze_sync.rs     # Bronze metadata sync
│   │   ├── silver_sync.rs     # Silver metadata sync
│   │   └── sql_gen.rs         # SQL generation
│   └── Cargo.toml
└── ndp-config/                 # [NEW] Configuration operations
    ├── src/
    │   ├── main.rs
    │   ├── yaml_parser.rs     # Unified YAML/JSON parsing
    │   ├── etcd_sync.rs       # etcd operations
    │   └── stream_ops.rs      # Stream config operations
    └── Cargo.toml
```

### Migration Path

| Phase | Scope | Lines Removed from Bash | New Tool |
|-------|-------|-------------------------|----------|
| **1** | Manifest parsing & validation | ~100 | `ndp-manifest parse`, `ndp-manifest validate` |
| **2** | YAML/JSON utilities | ~300 | Shared `ndp-config` library |
| **3** | Data dictionary sync | ~400 | `ndp-dictionary sync` |
| **4** | Domain objectives | ~200 | `ndp-dictionary sync-domains` |
| **5** | Remaining complex logic | ~200 | Various subcommands |

### Resulting deploy.sh (~500 lines)

```bash
#!/bin/bash
# Neural Data Platform - Deployment Orchestrator
# Delegates complex operations to Rust CLI tools

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Tool discovery
find_tool() {
    local tool="$1"
    if command -v "$tool" &>/dev/null; then echo "$tool"
    elif [ -x "/opt/ndp/bin/$tool" ]; then echo "/opt/ndp/bin/$tool"
    elif [ -x "$REPO_ROOT/target/release/$tool" ]; then echo "$REPO_ROOT/target/release/$tool"
    elif [ -x "$REPO_ROOT/target/debug/$tool" ]; then echo "$REPO_ROOT/target/debug/$tool"
    else echo ""; fi
}

NDP_MANIFEST=$(find_tool ndp-manifest)
NDP_DICTIONARY=$(find_tool ndp-dictionary)
NDP_GOLD_DDL=$(find_tool ndp-gold-ddl)
NDP_VALIDATE=$(find_tool ndp-validate)

# Docker Compose helpers (unchanged)
dc() { docker compose -f "$COMPOSE_FILE" "$@"; }
dcx() { dc exec -T "$@"; }

# Apply manifest using Rust tool
apply() {
    local manifest_file="${1:-$REPO_ROOT/.deploy/manifest.json}"

    log "Phase 1: Validation"
    if [ -n "$NDP_MANIFEST" ]; then
        "$NDP_MANIFEST" validate "$manifest_file" || error "Manifest validation failed"
    fi

    log "Phase 3: Migrations"
    local migrations=$("$NDP_MANIFEST" extract --type migration "$manifest_file")
    echo "$migrations" | while read -r file; do
        dcx timescaledb psql -U postgres -d ndp -f "$file"
    done

    log "Phase 4: Silver Tables"
    # ... similar pattern for each phase

    log "Phase 9: Dictionary"
    if [ -n "$NDP_DICTIONARY" ]; then
        "$NDP_DICTIONARY" sync --config-dir "$CONFIG_STREAMS_DIR"
    fi
}
```

### Code Example: ndp-manifest Tool

```rust
// tools/ndp-manifest/src/main.rs
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ndp-manifest")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate a manifest file
    Validate { manifest: PathBuf },

    /// Extract declarations by type
    Extract {
        #[arg(long)]
        r#type: String,
        manifest: PathBuf,
    },

    /// Diff two manifests
    Diff {
        old: PathBuf,
        new: PathBuf,
    },
}

#[derive(Deserialize, Serialize)]
struct Manifest {
    version: String,
    description: Option<String>,
    changes: Vec<Declaration>,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum Declaration {
    Stream { id: String, action: Option<String> },
    SilverTable { stream_id: String, action: Option<String> },
    GoldTables { stream_id: String, action: Option<String> },
    Migration { file: String },
    Dictionary { action: Option<String> },
    Container { target: String, action: String },
    // ...
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Validate { manifest } => {
            let m: Manifest = serde_json::from_str(&std::fs::read_to_string(&manifest)?)?;
            println!("Manifest valid: {} changes", m.changes.len());
            Ok(())
        }
        Commands::Extract { r#type, manifest } => {
            let m: Manifest = serde_json::from_str(&std::fs::read_to_string(&manifest)?)?;
            for change in m.changes {
                // Output matching declarations
            }
            Ok(())
        }
        Commands::Diff { old, new } => {
            // Compute manifest diff
            Ok(())
        }
    }
}
```

### Evaluation

| Criterion | Score | Notes |
|-----------|-------|-------|
| **Pi Resource Footprint** | **A** | Each tool ~8-10MB, lazy loading |
| **Maintainability** | **A** | Type-safe Rust for complex logic, thin Bash orchestration |
| **Testability** | **A** | Rust tools fully testable, Bash minimal |
| **Onboarding** | **B+** | Bash entry point familiar, Rust for deep work |
| **Future Extensibility** | **A** | New Rust tools follow established patterns |
| **CI/CD Integration** | **A** | cargo test, shellcheck for Bash |
| **Error Handling** | **A** | Rust Result types for complex logic |
| **Debugging** | **A** | Rust tools debuggable, Bash is simple passthrough |

---

## Resource Impact Analysis

### Pi 5 Specifications (Reference)
- CPU: Quad-core ARM Cortex-A76 @ 2.4GHz
- RAM: 4GB or 8GB
- Storage: microSD or NVMe

### Disk Usage Comparison

| Component | Option A | Option B (Rust) | Option C |
|-----------|----------|-----------------|----------|
| deploy.sh | 100KB | 0 | 20KB |
| Bash lib/ | 80KB | 0 | 0 |
| ndp-manifest | 0 | 10MB | 10MB |
| ndp-dictionary | 0 | 10MB | 10MB |
| ndp-gold-ddl | 10MB | 10MB | 10MB |
| ndp-validate | 8MB | 8MB | 8MB |
| **Total Delta** | **+80KB** | **+10MB** | **+20MB** |

### Memory Usage (Runtime)

| Scenario | Option A | Option B | Option C |
|----------|----------|----------|----------|
| Idle | ~0 | ~0 | ~0 |
| apply() running | 20-50MB (bash+jq+python) | 5-10MB (single binary) | 10-20MB (bash + rust tool) |
| Peak during deploy | 100MB+ | 15-20MB | 30-40MB |

### Startup Time

| Operation | Option A | Option B | Option C |
|-----------|----------|----------|----------|
| CLI startup | ~50ms | ~30ms | ~50ms + 30ms per tool |
| Manifest parse | ~200ms (jq/python) | ~5ms | ~5ms (Rust) |
| Full apply (no changes) | ~5s | ~2s | ~3s |

---

## Declarative Manifest Integration Analysis

### How Each Option Handles Manifests

**Option A (Bash Split):**
```bash
# Still requires jq for parsing
local changes=$(jq -c '.changes[]' "$manifest_file")
echo "$changes" | while read -r decl; do
    local type=$(echo "$decl" | jq -r '.type')
    # ... dispatch based on type
done
```
- Relies on jq (must be installed)
- No compile-time validation of manifest structure
- Error messages not helpful for invalid manifests

**Option B (Full Rust):**
```rust
#[derive(Deserialize)]
struct Manifest {
    version: String,
    changes: Vec<Declaration>,
}

fn apply(manifest: &Manifest) -> Result<(), Error> {
    for change in &manifest.changes {
        match change {
            Declaration::Stream { id, action } => handle_stream(id, action)?,
            Declaration::Migration { file } => handle_migration(file)?,
            // ...
        }
    }
    Ok(())
}
```
- Type-safe deserialization
- Compile-time exhaustiveness checking
- Rich error messages with source location

**Option C (Hybrid):**
```bash
# Bash orchestrator
apply() {
    "$NDP_MANIFEST" validate "$manifest_file" || exit 1

    for phase in "migration" "silver-table" "gold-tables" "stream" "dictionary"; do
        "$NDP_MANIFEST" extract --type "$phase" "$manifest_file" | while read -r item; do
            handle_$phase "$item"
        done
    done
}
```
- Rust tool validates and extracts
- Bash handles orchestration and Docker operations
- Best of both worlds

### Manifest Schema Enforcement

All options can use the existing `schemas/manifest.schema.json` for validation:

| Option | Schema Validation | Runtime Type Safety |
|--------|-------------------|---------------------|
| A | jq + ajv (external) | None |
| B | serde + jsonschema | Full |
| C | ndp-manifest (Rust) | Full for parsed items |

---

## Recommendation: Option C (Hybrid/Gradual Extraction)

### Rationale

1. **Aligns with Existing Patterns**
   - NDP already uses `ndp-gold-ddl` and `ndp-validate` called from Bash
   - Pattern is proven and understood by the team
   - No architectural shift required

2. **Gradual Migration Path**
   - Can migrate one function at a time
   - No "big bang" rewrite risk
   - Each migration is independently testable

3. **Optimal Resource Balance**
   - Bash remains for what it's good at (glue, Docker orchestration)
   - Rust handles what Bash struggles with (YAML parsing, SQL generation, validation)

4. **Testability Improvement**
   - Rust tools get comprehensive unit tests
   - Bash reduced to simple, auditable orchestration
   - Integration tests can use `--dry-run` flags

5. **Extensibility**
   - New features = new Rust subcommands
   - Follows established `ndp-*` CLI pattern
   - Shared crate for common types (`ndp-types` already exists)

### Implementation Priority

| Priority | Tool | Complexity Removed | LOC Saved |
|----------|------|-------------------|-----------|
| **P1** | `ndp-manifest` | JSON parsing, validation | ~100 |
| **P2** | `ndp-dictionary` | SQL generation, YAML parsing | ~600 |
| **P3** | `ndp-config` | etcd operations, stream sync | ~200 |
| **P4** | Bash cleanup | Remaining simplification | ~200 |

### Migration Timeline (Suggested)

| Week | Deliverable |
|------|-------------|
| 1 | Create `ndp-manifest` with `validate` and `extract` subcommands |
| 2 | Create `ndp-dictionary` with `sync` command |
| 3 | Refactor `deploy.sh` to use new tools |
| 4 | Create `ndp-config` for remaining YAML operations |
| 5 | Integration testing and documentation |

---

## Appendix: Existing Rust Tooling Dependencies

### Shared Dependencies (potential workspace unification)

```toml
# Cargo.toml workspace
[workspace]
members = [
    "core",
    "tools/ndp-gold-ddl",
    "tools/ndp-validate",
    "tools/ndp-manifest",      # NEW
    "tools/ndp-dictionary",    # NEW
    "tools/ndp-config",        # NEW
]

[workspace.dependencies]
# CLI
clap = { version = "4", features = ["derive", "env"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"

# Database
tokio-postgres = { version = "0.7", features = ["with-chrono-0_4"] }

# Async
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }

# Error handling
thiserror = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

### Build Commands

```bash
# Build all tools
cargo build --release --workspace

# Build specific tool
cargo build --release -p ndp-manifest

# Run tests
cargo test --workspace

# Install to /opt/ndp/bin (deployment)
for tool in ndp-gold-ddl ndp-validate ndp-manifest ndp-dictionary ndp-config; do
    cp "target/release/$tool" /opt/ndp/bin/
done
```

---

## References

- `/workspaces/neural-data-platform/deploy/pi/deploy.sh` - Current implementation (2,868 lines)
- `/workspaces/neural-data-platform/tools/ndp-gold-ddl/` - Existing Rust CLI pattern
- `/workspaces/neural-data-platform/tools/ndp-validate/` - Existing validation tool
- `/workspaces/neural-data-platform/docs/procedures/DEPLOYMENT-DECLARATIVES.md` - Manifest specification
