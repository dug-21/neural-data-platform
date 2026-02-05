# Deployment Architecture Research Synthesis

> **Research Initiative**: GitHub Issue #15 Analysis + Future Roadmap Planning
> **Date**: 2026-02-05
> **Status**: Complete
> **Swarm ID**: swarm-1770311361971

---

## Executive Summary

This document synthesizes findings from 5 concurrent research agents analyzing NDP's deployment architecture. The research addresses GitHub Issue #15 (refactoring the 2,868-line `deploy.sh`) while considering the platform's trajectory from V1.1 through V2.0.

### The Core Question

> How should NDP evolve its deployment capability to support both immediate maintainability needs and the long-term vision of "a simple 'deploy' command that recognizes key changes and knows how to deploy them"?

### The Answer: Unified NDP CLI with Phased Evolution

**Key Insight**: Build a single `ndp` CLI with componentized library that exposes functionality through BOTH CLI commands AND MCP tools.

| Phase | Timeline | Approach | Outcome |
|-------|----------|----------|---------|
| **Phase 1** | Immediate | Modularize Bash + justfile | Bridge while Rust CLI built |
| **Phase 2** | Short-term | CI/CD for builds | 90% faster deployments, ~1GB Pi savings |
| **Phase 3** | Medium-term | Unified `ndp` CLI + ndp-lib | Single binary, CLI + MCP exposure |
| **Phase 4** | Long-term | `ndp deploy --watch` | Intelligent auto-deployment |

**See**: `06-UNIFIED-NDP-CLI-ARCHITECTURE.md` for detailed architecture.

---

## Research Findings Summary

### 1. Current State Analysis (Agent 1)

**Key Metrics:**
- `deploy.sh`: 2,868 lines (57x Google's recommended 50 lines for Bash)
- `ddl-generator.sh`: 850 lines
- **Total deployment infrastructure**: 4,051 lines across 5 files
- **Functions**: 43 total, handling 11 deployment phases

**Critical Issues Identified:**

| Issue | Severity | Lines Affected | Impact |
|-------|----------|----------------|--------|
| YAML helper duplication | Medium | 170+ | Maintenance burden |
| God functions | High | 458 (sync_to_data_dictionary) | Untestable |
| Inline SQL generation | High | 600+ | Error-prone, security risk |
| Error suppression | Medium | 50+ instances | Hidden failures |

**Emerging Pattern (Positive)**: Complex logic is already migrating to Rust tools:
- `ndp-gold-ddl` - Gold layer DDL generation
- `ndp-validate` - Configuration validation

### 2. Edge Deployment Patterns (Agent 2)

**Platform Evaluation:**

| Platform | Verdict | Reason |
|----------|---------|--------|
| Balena | ❌ Overkill | Fleet management for 10+ devices, vendor lock-in |
| Mender | ❌ Overkill | OTA updates focus, 7MB client |
| k3s | ❌ Not needed | Multi-node orchestration, 512MB overhead |
| Nomad | ❌ Not needed | Same - unnecessary for single device |
| Podman | ⏸️ Deferred | 65% less memory, but Docker works fine |
| **just** | ✅ Recommended | 5MB binary, cleaner than Make |

**Recommendation**: Docker Compose remains appropriate. Add `justfile` as user-friendly task runner.

### 3. Language Options Evaluation (Agent 3)

| Option | Maintainability | Pi Footprint | Testability | Verdict |
|--------|-----------------|--------------|-------------|---------|
| A: Bash split | Poor | Excellent | Poor | ❌ Not recommended |
| B: Full Rust | Excellent | Excellent | Excellent | ⚠️ High effort, big bang risk |
| **C: Hybrid** | Excellent | Excellent | Excellent | ✅ **Recommended** |

**Why Hybrid Wins:**
1. **Validates existing pattern** - ndp-gold-ddl and ndp-validate already prove the model
2. **Gradual migration** - Extract one function at a time, no "big bang" risk
3. **Optimal responsibility split** - Bash for Docker orchestration, Rust for logic
4. **Pi-friendly** - Rust tools are ~10MB each, negligible on Pi 5

**Proposed New Rust Tools:**

| Tool | Purpose | Bash Lines Removed | Priority |
|------|---------|-------------------|----------|
| `ndp-manifest` | Manifest parsing, validation, diffing | ~100 | High |
| `ndp-dictionary` | Data dictionary sync, SQL generation | ~600 | High |
| `ndp-config` | YAML parsing, etcd operations | ~200 | Medium |

### 4. Future Roadmap Requirements (Agent 4)

**Deployment Complexity by Version:**

```
V1.0 ────────────────────────────────────────────────────────────────►
     Bronze→Silver ETL, basic stream configs
     Bash: ████████████████████████████████████████  Excellent

V1.1 ────────────────────────────────────────────────────────────────►
     Gold layer, continuous aggregates, alignment views
     Bash: ████████████████████████████████████████  Adequate
     (Current deploy.sh handles this)

V1.2 ────────────────────────────────────────────────────────────────►
     Pattern detection, correlation scanning, scheduled jobs
     Bash: ████████████████████████████░░░░░░░░░░░░  Marginal
     Needs: analytics-job, threshold-promoter declarations

V1.3 ────────────────────────────────────────────────────────────────►
     Predictions, ML models, action framework
     Bash: ████████████████░░░░░░░░░░░░░░░░░░░░░░░░  Poor
     Needs: model registry, action definitions, feedback loops

V2.0 ────────────────────────────────────────────────────────────────►
     Multi-stream intelligence, auto-detection, self-healing
     Bash: ████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  Inadequate
     Needs: Watch mode, dependency resolution, intelligent orchestration
```

**New Declaration Types Required:**

| Version | Declaration | Purpose |
|---------|-------------|---------|
| V1.2 | `analytics-job` | Scheduled correlation scanning |
| V1.2 | `threshold-promoter` | Automatic candidate promotion |
| V1.3 | `model` | ML model artifact deployment |
| V1.3 | `model-tournament` | Model evaluation/selection |
| V1.3 | `action` | Action definition and routing |
| V1.3 | `feedback-loop` | Continuous learning config |
| V2.0 | `watch-mode` | Automatic change detection |
| V2.0 | `dependency-graph` | Cross-declaration resolution |

### 5. CI/CD vs Local Split (Agent 5)

**Deployment Time Impact:**

| Operation | Current (On-Pi) | With CI/CD | Improvement |
|-----------|-----------------|------------|-------------|
| Docker builds | 15-25 min | Pre-built | 100% removed |
| Rust builds | 5-10 min | Pre-built | 100% removed |
| Manifest validation | 1 min | Pre-validated | 100% removed |
| **Total deployment** | **20-35 min** | **2-3 min** | **90% faster** |

**Function Categorization:**

| Category | Functions | Rationale |
|----------|-----------|-----------|
| **CI/CD** | Container builds, Rust tools, DDL generation, validation | Slow, CPU-intensive, deterministic |
| **Local** | SQL execution, etcd sync, container restart, health checks | Requires runtime state |
| **Hybrid** | Gold DDL, Domain config | Generate in CI, apply locally |

**Pi Footprint Savings:**
- Remove Rust toolchain: **~1GB** disk savings
- Remove build dependencies: **~200MB** savings
- Total: **~1.2GB recovered**

---

## Recommendations

### Short-Term (Issue #15 Resolution) - Weeks 1-4

**Objective**: Reduce deploy.sh to <500 lines while maintaining all functionality.

#### Step 1: Extract to Modular Bash (Week 1-2)

```
deploy/pi/
├── deploy.sh              # Thin orchestrator (~150 lines)
├── justfile               # User-friendly task runner
└── lib/
    ├── common.sh          # Logging, colors, utilities
    ├── validation.sh      # Phase 1: Manifest validation
    ├── containers.sh      # Phase 2, 10: Build/restart
    ├── migrations.sh      # Phase 3: SQL migrations
    ├── silver.sh          # Phase 4: Silver DDL
    ├── gold.sh            # Phase 5: Gold tables
    ├── domains.sh         # Phase 6: Domain views
    ├── streams.sh         # Phase 7: etcd sync
    ├── dimensions.sh      # Phase 8: Dimension import
    ├── dictionary.sh      # Phase 9: Data dictionary
    └── state.sh           # Phase 11: Device state
```

#### Step 2: Add justfile (Week 2)

```just
# User-friendly deployment commands
default:
    @just --list

deploy:
    ./deploy.sh apply .deploy/releases/$(git describe --tags --abbrev=0).manifest.json

status:
    ./deploy.sh status

logs service='all':
    ./deploy.sh logs {{service}}

sync:
    ./deploy.sh sync

# Development helpers
build target='all':
    ./deploy.sh build {{target}}

test:
    ./deploy.sh apply --dry-run .deploy/manifest.json
```

#### Step 3: Deduplicate YAML Helpers (Week 3)

Move 170+ duplicated lines to `lib/yaml-helpers.sh`, source from both `deploy.sh` and `ddl-generator.sh`.

#### Step 4: Extract SQL Generation (Week 4)

The 600+ lines of inline SQL in `sync_to_data_dictionary()` should become the first Rust tool candidate (`ndp-dictionary`).

### Medium-Term (CI/CD Enhancement) - Weeks 5-10

**Objective**: 90% faster deployments via pre-built artifacts.

#### Phase 1: CI Infrastructure (Weeks 5-6)

```yaml
# .github/workflows/build-artifacts.yml
name: Build Deployment Artifacts
on:
  push:
    tags: ['v*']

jobs:
  build-containers:
    runs-on: ubuntu-latest
    steps:
      - uses: docker/setup-qemu-action@v3  # ARM64 emulation
      - uses: docker/build-push-action@v5
        with:
          platforms: linux/arm64
          push: true
          tags: ghcr.io/${{ github.repository }}/air-quality-app:${{ github.ref_name }}

  build-tools:
    runs-on: ubuntu-latest
    steps:
      - uses: actions-rs/toolchain@v1
        with:
          target: aarch64-unknown-linux-gnu
      - run: cargo build --release --target aarch64-unknown-linux-gnu
      - uses: actions/upload-artifact@v4
```

#### Phase 2: Deployment Artifact Bundle (Weeks 7-8)

Create versioned deployment bundles:
```
ndp-deploy-v1.2.0.tar.gz
├── images/                # OCI tar archives
│   ├── air-quality-app.tar
│   ├── mcp-server.tar
│   └── silver-etl.tar
├── tools/                 # Pre-compiled ARM64 binaries
│   ├── ndp-gold-ddl
│   ├── ndp-validate
│   └── ndp-dictionary
├── ddl/                   # Pre-generated SQL
│   └── gold-tables.sql
├── manifest.json          # Deployment manifest
└── deploy-from-ci.sh      # Minimal local script
```

#### Phase 3: Minimal Local Deploy Script (Weeks 9-10)

```bash
#!/bin/bash
# deploy-from-ci.sh - Minimal Pi deployment (~100 lines)

set -e
BUNDLE="$1"

# 1. Load pre-built images
for img in images/*.tar; do docker load -i "$img"; done

# 2. Apply pre-generated DDL
psql -h localhost -U ndp_app -f ddl/gold-tables.sql

# 3. Sync configs to etcd
./tools/ndp-config sync

# 4. Restart containers
docker compose -f docker-compose.yml up -d

# 5. Update device state
echo "$VERSION" > /var/ndp/deployed-version
```

### Long-Term (V1.3+ Support) - Months 3-6

**Objective**: Build `ndp-deploy` Rust engine supporting full roadmap.

#### Architecture: Unified Deployment Engine

```rust
// ndp-deploy - Declarative deployment engine
pub struct DeployEngine {
    manifest: Manifest,
    state: DeviceState,
    executors: HashMap<DeclarationType, Box<dyn Executor>>,
}

impl DeployEngine {
    pub async fn apply(&self, manifest_path: &Path) -> Result<DeployResult> {
        let manifest = Manifest::load(manifest_path)?;
        let plan = self.plan(&manifest)?;

        for phase in plan.phases() {
            for declaration in phase.declarations() {
                let executor = self.executors.get(&declaration.type_())?;
                executor.execute(&declaration, &self.state).await?;
            }
        }

        self.state.update(&manifest)?;
        Ok(DeployResult::success())
    }

    pub async fn watch(&self, config_dir: &Path) -> Result<()> {
        // V2.0: Watch for changes, auto-deploy
    }

    pub fn diff(&self, manifest: &Manifest) -> Vec<Change> {
        // V2.0: Detect what changed since last deploy
    }
}
```

#### New Declaration Support

```rust
// Support for V1.3+ declarations
enum DeclarationType {
    // V1.0
    Stream, Migration, Container,
    // V1.1
    SilverTable, GoldTable, Domain, Dictionary,
    // V1.2
    AnalyticsJob, ThresholdPromoter,
    // V1.3
    Model, ModelTournament, Action, FeedbackLoop, AutonomyPolicy,
    // V2.0
    WatchMode, DependencyGraph,
}
```

---

## Implementation Roadmap

```
┌─────────────────────────────────────────────────────────────────────────┐
│ 2026                                                                     │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  Feb        Mar        Apr        May        Jun        Jul             │
│   │          │          │          │          │          │              │
│   ▼          ▼          ▼          ▼          ▼          ▼              │
│                                                                          │
│  ┌─────────┐                                                             │
│  │ PHASE 1 │ Modularize Bash + justfile                                 │
│  │ 4 weeks │ Issue #15 resolution                                       │
│  └────┬────┘                                                             │
│       │                                                                  │
│       ▼                                                                  │
│       ┌─────────────┐                                                    │
│       │   PHASE 2   │ CI/CD for builds                                  │
│       │   6 weeks   │ 90% faster deployments                            │
│       └──────┬──────┘                                                    │
│              │                                                           │
│              ▼                                                           │
│              ┌─────────────────┐                                         │
│              │     PHASE 3     │ ndp-dictionary, ndp-config tools       │
│              │     6 weeks     │ SQL generation in Rust                 │
│              └────────┬────────┘                                         │
│                       │                                                  │
│                       ▼                                                  │
│                       ┌─────────────────────┐                            │
│                       │       PHASE 4       │ ndp-deploy engine         │
│                       │       8 weeks       │ V1.3 declaration support  │
│                       └──────────┬──────────┘                            │
│                                  │                                       │
│                                  ▼                                       │
│                                  ┌───────────────────────────┐           │
│                                  │         PHASE 5           │           │
│                                  │        Ongoing            │           │
│                                  │ Watch mode, auto-detection│           │
│                                  │ V2.0 intelligent deploy   │           │
│                                  └───────────────────────────┘           │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Decision Matrix

For GitHub Issue #15 resolution:

| Criterion | Weight | Option A (Bash Split) | Option B (Full Rust) | Option C (Hybrid) |
|-----------|--------|----------------------|---------------------|-------------------|
| Immediate maintainability | 25% | 7/10 | 3/10 | 9/10 |
| Pi resource impact | 20% | 10/10 | 9/10 | 10/10 |
| Future extensibility | 25% | 4/10 | 10/10 | 9/10 |
| Implementation risk | 15% | 8/10 | 4/10 | 8/10 |
| CI/CD integration | 15% | 5/10 | 9/10 | 8/10 |
| **Weighted Score** | | **6.6** | **6.8** | **8.8** |

**Winner: Option C (Hybrid)** with phased evolution.

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Bash modularization introduces regressions | Medium | High | Comprehensive integration tests, gradual rollout |
| CI/CD ARM64 emulation too slow | Low | Medium | GitHub larger runners, or self-hosted ARM64 runner |
| Rust tools add complexity | Low | Medium | Follow existing ndp-gold-ddl patterns exactly |
| V1.3 declarations underspecified | Medium | Medium | Define schema before implementation |
| Developer resistance to change | Low | Low | justfile provides familiar interface |

---

## Success Criteria

### Phase 1 (Bash Modularization)
- [ ] No file exceeds 300 lines
- [ ] All functionality preserved (regression tests pass)
- [ ] YAML helpers deduplicated
- [ ] justfile provides user-friendly commands

### Phase 2 (CI/CD)
- [ ] Deployment time < 5 minutes
- [ ] Rust toolchain removed from Pi
- [ ] Artifacts versioned and reproducible

### Phase 3 (Rust Tools)
- [ ] `ndp-dictionary` replaces 600+ lines of Bash
- [ ] `ndp-config` handles all YAML parsing
- [ ] Tools integrated into CI/CD pipeline

### Phase 4 (ndp-deploy)
- [ ] Supports all V1.3 declarations
- [ ] Watch mode functional
- [ ] Single `ndp-deploy apply` command works

---

## Appendix: Research Documents

### Initial Research (5-Agent Swarm)

| Document | Agent | Focus |
|----------|-------|-------|
| `01-current-state-analysis.md` | ndp-architect | Deploy.sh structure, complexity metrics |
| `02-edge-deployment-patterns.md` | researcher | Pi constraints, lightweight solutions |
| `03-language-options-evaluation.md` | ndp-architect | Bash vs Python vs Rust comparison |
| `04-future-requirements-analysis.md` | ndp-architect | V1.1→V2.0 deployment implications |
| `05-cicd-local-split-analysis.md` | cicd-engineer | Build location optimization |

### Architecture & Planning (Follow-up Analysis)

| Document | Focus |
|----------|-------|
| `06-UNIFIED-NDP-CLI-ARCHITECTURE.md` | Single `ndp` CLI + ndp-lib design, crate structure |
| `07-CLI-ROADMAP-ALIGNMENT-ANALYSIS.md` | 20 command categories, 115 subcommands for V1.1→V2.0 |
| `08-STANDARD-VERBS-AND-MCP-INTEGRATION.md` | Standard CRUD verbs, 14 MCP tool migration mapping |
| `09-STEPWISE-MIGRATION-PLAN.md` | 8-phase, 18-week migration with zero downtime |

---

*Research conducted by 5-agent hierarchical swarm + follow-up analysis*
*Total: 10 research documents covering all aspects of deployment refactoring*
