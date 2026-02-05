# Future Roadmap & Deployment Requirements Analysis

> **Document**: product/research/deployment/04-future-requirements-analysis.md
> **Created**: 2026-02-05
> **Author**: NDP Architect
> **Purpose**: Analyze how Gold Layer roadmap (V1.1 through V2.0) impacts deployment architecture decisions

---

## Executive Summary

This analysis maps the Feature Roadmap (V1.1 through V2.0) to deployment requirements, identifying gaps in the current Bash-based declarative deployment system and recommending architecture changes to support the platform's evolution toward autonomous, configuration-driven intelligence.

### Key Findings

1. **V1.1 (Gold Layer Foundation)** - Current system is adequate with planned declarations (`gold-tables`, `domains`, `tool`)
2. **V1.2 (Pattern Detection Engine)** - Requires new deployment primitives for scheduled jobs and analytics tasks
3. **V1.3 (Prediction & Actions)** - Introduces significant complexity: model lifecycle, action framework, graduated autonomy
4. **V2.0 (Multi-Stream Intelligence)** - Current Bash approach unlikely to scale; requires orchestration engine

### Critical Insight

The long-term goal stated in the roadmap is:

> "achieve a simple 'deploy' command, and have the system recognize the key changes and know how to deploy them"

This vision implies **intelligent deployment orchestration** that goes beyond scripted phase execution. The current Bash approach can evolve incrementally through V1.2, but V1.3+ will likely require a more sophisticated deployment engine.

---

## Version-by-Version Deployment Requirements Matrix

### V1.1: Gold Layer Foundation (Current)

| Feature | Deployment Requirement | Current Support | Gap |
|---------|----------------------|-----------------|-----|
| **v11-A01** Gold ETL JSON Schema | Schema validation | Partial (jq-based) | Need JSON Schema validator |
| **v11-A02** Gold ETL Interpreter | Rust tool build | `tool` declaration | None |
| **v11-003** Per-Stream Continuous Aggregates | DDL generation & apply | `gold-tables` declaration | None |
| **v11-005** Cross-Stream Aligned View | Materialized view creation | Via DDL tool | None |
| **v11-006** State Transition Materializer | SQL view creation | Via migration/DDL | None |
| **v11-007** Objectives Storage | etcd sync | `domain` declaration | None |
| **v11-012** Threshold Crossing Generator | SQL view from config | Via DDL tool | None |
| **v11-013** Unified Events View | Composite view | Via DDL tool | None |

**V1.1 Assessment**: Current deployment system handles V1.1 requirements. The `gold-tables`, `domain`, and `tool` declarations cover needed capabilities.

---

### V1.2: Pattern Detection Engine

| Feature | Deployment Requirement | Current Support | Gap |
|---------|----------------------|-----------------|-----|
| **v12-001** Transition Event Materializer | SQL materialized view | Via DDL tool | None |
| **v12-002** Granger Causality Scanner | **Scheduled analytics job** | None | **NEW PRIMITIVE NEEDED** |
| **v12-003** Response Window Analyzer | Part of scanner job | N/A | Coupled to v12-002 |
| **v12-004** Lag Optimizer | Part of scanner job | N/A | Coupled to v12-002 |
| **v12-005** Correlation Aggregator | Materialized view | Via DDL tool | None |
| **v12-006** Candidate Ranker | View or scheduled job | Partial | May need job |
| **v12-007** Candidate Promoter | Threshold-based promotion | None | **NEW PRIMITIVE** |
| **v12-008** Candidate Registry | New table + etcd storage | Via migration | Minor gap |
| **v12-009** Pattern Candidates Dashboard | Grafana provisioning | Dashboard declaration | None |
| **v12-010** Correlation Strength Tracker | Continuous aggregate | Via DDL tool | None |

#### New Deployment Primitives Required for V1.2

```yaml
# NEW: analytics-job declaration
- type: analytics-job
  id: granger-scanner
  schedule: "0 2 * * *"  # Nightly at 2 AM
  container: silver-etl  # Or dedicated analytics container
  command: "ndp-analytics granger-scan"
  timeout: 3600
  resource_limits:
    memory: 512Mi
    cpu: 0.5
  depends_on:
    - gold.aligned_hourly
```

```yaml
# NEW: threshold-promoter declaration
- type: threshold-promoter
  id: correlation-candidate-promoter
  source_view: gold.correlation_scores
  target_table: gold.candidate_correlations
  threshold_column: correlation_strength
  threshold_value: 0.7
  schedule: "*/15 * * * *"  # Every 15 minutes
```

**V1.2 Assessment**: Current system needs extension for scheduled analytics jobs. Options:
1. Add cron-based job scheduling to `deploy.sh`
2. Leverage TimescaleDB job scheduler
3. Add dedicated analytics container with scheduling

---

### V1.3: Prediction & Actions

| Feature | Deployment Requirement | Current Support | Gap |
|---------|----------------------|-----------------|-----|
| **v13-001** Causal Validation Engine | Analytics job | Via v12 primitives | None |
| **v13-002** Model Zoo | **Model registry & deployment** | None | **MAJOR GAP** |
| **v13-003** Tournament Selection | Model evaluation job | None | **NEW PRIMITIVE** |
| **v13-004** Prediction Service | **Real-time inference endpoint** | None | **MAJOR GAP** |
| **v13-005** Action Framework | **Action definition & routing** | None | **MAJOR GAP** |
| **v13-006** Action Scoring | Part of prediction service | N/A | Coupled |
| **v13-007** Outcome Tracker | Table + ETL | Via existing | None |
| **v13-008** Feedback Learning | **Model update pipeline** | None | **MAJOR GAP** |
| **v13-009** Autonomy Controller | Configuration + runtime | None | **NEW PRIMITIVE** |
| **v13-010** Prediction Dashboard | Grafana provisioning | Dashboard declaration | None |

#### New Deployment Primitives Required for V1.3

```yaml
# NEW: model declaration
- type: model
  id: co2-tcn-predictor
  version: "1.0.0"
  framework: ruv-fann  # or tch-rs
  artifact: models/co2-tcn-v1.0.0.bin
  action: deploy
  endpoint:
    enabled: true
    path: /predict/co2
    timeout_ms: 100
  resources:
    memory: 256Mi
    inference_threads: 2
```

```yaml
# NEW: model-tournament declaration
- type: model-tournament
  id: co2-forecast-tournament
  target_metric: co2
  horizon_hours: 1
  candidates:
    - model_id: co2-tcn-v1
    - model_id: co2-arima-v1
    - model_id: co2-prophet-v1
  evaluation:
    metric: rmse
    holdout_days: 7
  schedule: "0 3 * * 0"  # Weekly Sunday 3 AM
  auto_promote: true
```

```yaml
# NEW: action declaration
- type: action
  id: open-window
  description: "Open window to reduce CO2"
  preconditions:
    - stream: outdoor-air-quality
      metric: pm25
      condition: "<"
      threshold: 35
  effect:
    target_stream: air-quality
    target_metric: co2
    expected_change: -15%
    expected_lag_minutes: 17
  triggers:
    - webhook: "http://homeassistant.local/api/switch/window"
    - mqtt: "home/window/command"
  autonomy_level: suggest  # alert | suggest | auto
  safety_limits:
    max_executions_per_hour: 4
    cooldown_minutes: 30
```

```yaml
# NEW: feedback-loop declaration
- type: feedback-loop
  id: co2-prediction-feedback
  model_id: co2-tcn-predictor
  outcome_table: gold.prediction_outcomes
  learning:
    algorithm: ewc++
    batch_size: 100
    update_frequency: daily
  retrain_threshold:
    rmse_increase: 20%
    samples_since_last: 1000
```

**V1.3 Assessment**: Deployment complexity increases dramatically. The Bash approach becomes unwieldy:

| Challenge | Why Bash Struggles |
|-----------|-------------------|
| Model versioning | Requires artifact registry, not file sync |
| Real-time endpoints | Needs container orchestration, health checks |
| Action framework | Complex state machine, not declarative config |
| Feedback loops | Continuous processes, not one-time deployments |
| Autonomy control | Runtime state management across restarts |

---

### V2.0: Multi-Stream Intelligence

| Feature | Deployment Requirement | Current Support | Gap |
|---------|----------------------|-----------------|-----|
| **v2-001** Financial Stream Sources | New source adapters | `stream` declaration | Minor |
| **v2-002** Stream Source Registry | Runtime adapter registration | None | **ARCHITECTURAL** |
| **v2-003** Full Correlation Scanner | Scaled analytics jobs | Via v12 primitives | Scale concern |
| **v2-004** Multi-Stream Objectives | Config expansion | `domain` declaration | Minor |
| **v2-005** Seeded Financial Models | Model deployment | Via v13 primitives | None |
| **v2-006** Stream-Specific Feature Templates | Template registry | None | **NEW PRIMITIVE** |
| **v2-007** Unified Dashboard | Grafana provisioning | Dashboard declaration | None |

#### The V2.0 Validation Test

The roadmap defines the definitive architecture test:

> A new domain (e.g., "Energy Efficiency") is declared via JSON config. Without code changes, the system materializes Gold infrastructure, runs pattern detection, surfaces candidates, and enables predictions.

**Deployment implications of passing this test:**

```
User adds config/domains/energy-efficiency/domain.json
        ↓
System detects new domain config (how?)
        ↓
Deployment engine:
  1. Validates domain config against schema
  2. Identifies required streams (some may need creation)
  3. Generates Gold layer DDL for new streams
  4. Creates cross-stream alignment views
  5. Configures correlation scanner for new objectives
  6. Sets up prediction endpoints (if models exist)
  7. Provisions dashboard panels
  8. Updates data dictionary
        ↓
Within 15 minutes: Domain operational
```

This is **not** a `./deploy.sh apply manifest.json` workflow. It requires:

1. **Change detection** - Watching config files for changes
2. **Dependency resolution** - Understanding what a new domain requires
3. **Incremental generation** - Only creating what's new
4. **Health verification** - Confirming each step succeeded
5. **Rollback capability** - Reverting partial failures

---

## Gap Analysis: Current vs Needed Capabilities

### Current Deployment System (V1.0/V1.1)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      CURRENT: deploy.sh + Manifest                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  SUPPORTED DECLARATIONS                                                     │
│  ──────────────────────                                                     │
│  ✅ stream         - Sync config to etcd                                    │
│  ✅ silver-table   - Generate DDL, create hypertable                        │
│  ✅ gold-tables    - Generate continuous aggregate DDL                      │
│  ✅ domain         - Sync objectives to data dictionary                     │
│  ✅ tool           - Build Rust CLI tools                                   │
│  ✅ migration      - Run SQL migrations                                     │
│  ✅ container      - Build/restart Docker containers                        │
│  ✅ dimensions     - Sync dimension CSVs                                    │
│  ✅ dictionary     - Sync data dictionary metadata                          │
│                                                                             │
│  EXECUTION MODEL                                                            │
│  ───────────────                                                            │
│  • Manual trigger: ./deploy.sh apply manifest.json                          │
│  • Fixed 11-phase execution order                                           │
│  • Serial processing within phases                                          │
│  • No dependency resolution (order by convention)                           │
│  • No rollback (except manual git checkout + reapply)                       │
│  • No change detection (full manifest reprocessing)                         │
│                                                                             │
│  STRENGTHS                                                                  │
│  ─────────                                                                  │
│  • Simple mental model                                                      │
│  • Debuggable (bash -x)                                                     │
│  • No external dependencies beyond Docker/jq                                │
│  • Idempotent for most declarations                                         │
│  • Works well for current scope (5 streams, 10 Silver tables)               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Required Capabilities by Version

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    CAPABILITY REQUIREMENTS BY VERSION                        │
├───────────────────┬──────────┬──────────┬──────────┬──────────┬────────────┤
│ Capability        │ V1.0/1.1 │   V1.2   │   V1.3   │   V2.0   │ Complexity │
├───────────────────┼──────────┼──────────┼──────────┼──────────┼────────────┤
│ Config sync       │    ✅    │    ✅    │    ✅    │    ✅    │    Low     │
│ DDL generation    │    ✅    │    ✅    │    ✅    │    ✅    │    Low     │
│ Container mgmt    │    ✅    │    ✅    │    ✅    │    ✅    │    Low     │
├───────────────────┼──────────┼──────────┼──────────┼──────────┼────────────┤
│ Scheduled jobs    │    ❌    │    ⚠️    │    ✅    │    ✅    │   Medium   │
│ Job monitoring    │    ❌    │    ⚠️    │    ✅    │    ✅    │   Medium   │
├───────────────────┼──────────┼──────────┼──────────┼──────────┼────────────┤
│ Model registry    │    ❌    │    ❌    │    ⚠️    │    ✅    │    High    │
│ Model deployment  │    ❌    │    ❌    │    ⚠️    │    ✅    │    High    │
│ Inference service │    ❌    │    ❌    │    ⚠️    │    ✅    │    High    │
├───────────────────┼──────────┼──────────┼──────────┼──────────┼────────────┤
│ Action framework  │    ❌    │    ❌    │    ⚠️    │    ✅    │    High    │
│ Autonomy control  │    ❌    │    ❌    │    ⚠️    │    ✅    │    High    │
│ Feedback loops    │    ❌    │    ❌    │    ⚠️    │    ✅    │   V.High   │
├───────────────────┼──────────┼──────────┼──────────┼──────────┼────────────┤
│ Change detection  │    ❌    │    ❌    │    ❌    │    ⚠️    │   Medium   │
│ Dep. resolution   │    ❌    │    ❌    │    ❌    │    ⚠️    │    High    │
│ Incremental apply │    ❌    │    ❌    │    ❌    │    ⚠️    │    High    │
│ Auto-rollback     │    ❌    │    ❌    │    ❌    │    ⚠️    │   V.High   │
├───────────────────┼──────────┼──────────┼──────────┼──────────┼────────────┤
│                   │          │          │          │          │            │
│ Legend:  ✅ Required & Supported                                            │
│          ⚠️ Required but Partial/Planned                                    │
│          ❌ Not Required                                                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Scalability Assessment: Bash Approach vs Alternatives

### Option 1: Extend Current Bash Approach

**Viability by version:**

| Version | Viability | Approach | Risk |
|---------|-----------|----------|------|
| V1.1 | Excellent | Current design | Low |
| V1.2 | Good | Add cron/TimescaleDB jobs | Medium |
| V1.3 | Marginal | Would require ~2000+ lines of bash | High |
| V2.0 | Poor | Bash doesn't scale to requirements | Very High |

**What V1.3 in Bash would look like:**

```bash
# deploy.sh would need:
# - Model artifact management (~300 lines)
# - Model versioning logic (~200 lines)
# - Endpoint health checks (~150 lines)
# - Action state machine (~400 lines)
# - Autonomy level enforcement (~200 lines)
# - Feedback loop orchestration (~300 lines)
# - Tournament evaluation (~200 lines)
# - Rollback for all above (~400 lines)
# Total: ~2200 new lines of Bash

# Example complexity (model deployment in bash):
handle_model_deploy() {
    local model_id=$(echo "$1" | jq -r '.id')
    local version=$(echo "$1" | jq -r '.version')
    local artifact=$(echo "$1" | jq -r '.artifact')
    local endpoint_enabled=$(echo "$1" | jq -r '.endpoint.enabled')

    # Validate artifact exists
    if [ ! -f "$REPO_ROOT/$artifact" ]; then
        error "Model artifact not found: $artifact"
    fi

    # Check for existing deployment
    local current_version=$(get_deployed_model_version "$model_id")
    if [ "$current_version" = "$version" ]; then
        log "Model $model_id v$version already deployed"
        return 0
    fi

    # Backup current model (for rollback)
    if [ -n "$current_version" ]; then
        backup_model "$model_id" "$current_version"
    fi

    # Deploy new model
    # ... 100+ more lines for deployment, health check, rollback
}
```

**Verdict**: Bash can handle V1.1-V1.2. Beyond that, complexity becomes unmanageable.

---

### Option 2: Rust Deployment Engine

**Concept**: Replace `deploy.sh` with a Rust binary that:
- Parses manifests with strong typing
- Manages state via SQLite or etcd
- Handles complex workflows with proper error handling
- Integrates with existing Rust ecosystem (tokio, sqlx)

**Viability by version:**

| Version | Viability | Effort | Risk |
|---------|-----------|--------|------|
| V1.1 | Overkill | High (rebuild) | Medium |
| V1.2 | Good | Medium | Low |
| V1.3 | Excellent | Medium | Low |
| V2.0 | Excellent | Low (incremental) | Low |

**Architecture sketch:**

```rust
// core/src/deploy/mod.rs
pub struct DeploymentEngine {
    manifest: Manifest,
    state: DeploymentState,
    handlers: HashMap<DeclarationType, Box<dyn DeclarationHandler>>,
}

impl DeploymentEngine {
    pub async fn apply(&mut self) -> Result<DeploymentResult> {
        // 1. Validate manifest
        self.validate()?;

        // 2. Compute execution plan (dependency-aware)
        let plan = self.compute_plan()?;

        // 3. Execute with rollback capability
        for step in plan.steps {
            match self.execute_step(&step).await {
                Ok(_) => self.state.mark_complete(step.id),
                Err(e) => {
                    self.rollback(&step).await?;
                    return Err(e);
                }
            }
        }

        Ok(DeploymentResult::Success)
    }
}

// Type-safe declaration handling
#[derive(Deserialize)]
#[serde(tag = "type")]
enum Declaration {
    Stream(StreamDecl),
    SilverTable(SilverTableDecl),
    GoldTables(GoldTablesDecl),
    Model(ModelDecl),           // V1.3
    Action(ActionDecl),         // V1.3
    AnalyticsJob(JobDecl),      // V1.2
    FeedbackLoop(FeedbackDecl), // V1.3
}
```

**Pros:**
- Type safety catches manifest errors early
- Proper error handling and rollback
- Integrates with existing Rust codebase
- Can reuse DDL generation, config parsing
- Testable with unit tests

**Cons:**
- Significant upfront investment
- Another binary to maintain
- Need to port ~2500 lines of working bash

---

### Option 3: Hybrid Approach (Recommended)

**Concept**: Keep Bash for V1.1-V1.2, build Rust engine incrementally for V1.3+

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       HYBRID DEPLOYMENT ARCHITECTURE                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  V1.1-V1.2: deploy.sh (Current)                                            │
│  ────────────────────────────────                                           │
│  ┌────────────────────────────────────────────────────────────────────────┐│
│  │ deploy.sh apply manifest.json                                          ││
│  │                                                                         ││
│  │ Handles: stream, silver-table, gold-tables, domain, tool, migration,   ││
│  │          container, dimensions, dictionary                              ││
│  │                                                                         ││
│  │ + NEW (V1.2): analytics-job → delegates to TimescaleDB job scheduler   ││
│  └────────────────────────────────────────────────────────────────────────┘│
│                                                                             │
│  V1.3+: ndp-deploy (Rust)                                                  │
│  ───────────────────────                                                    │
│  ┌────────────────────────────────────────────────────────────────────────┐│
│  │ ndp-deploy apply manifest.json                                         ││
│  │                                                                         ││
│  │ Handles: All V1.1-V1.2 declarations (migrated)                         ││
│  │          + model, model-tournament, action, feedback-loop              ││
│  │          + autonomy-policy, prediction-endpoint                        ││
│  │                                                                         ││
│  │ Features:                                                               ││
│  │   • Dependency-aware execution planning                                ││
│  │   • Transactional rollback                                             ││
│  │   • State persistence (SQLite)                                         ││
│  │   • Health check integration                                           ││
│  │   • Dry-run mode                                                       ││
│  └────────────────────────────────────────────────────────────────────────┘│
│                                                                             │
│  V2.0: ndp-deploy + Watch Mode                                             │
│  ─────────────────────────────                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐│
│  │ ndp-deploy watch                                                       ││
│  │                                                                         ││
│  │ Monitors: config/base/streams/, config/domains/                        ││
│  │                                                                         ││
│  │ On change:                                                              ││
│  │   1. Detect what changed (new stream, modified domain, etc.)           ││
│  │   2. Compute minimal deployment plan                                   ││
│  │   3. Auto-apply with notification                                      ││
│  │                                                                         ││
│  │ Enables: "Add domain via config → infrastructure in 15 minutes"        ││
│  └────────────────────────────────────────────────────────────────────────┘│
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Timeline:**

| Phase | Timeframe | Deliverable |
|-------|-----------|-------------|
| Now | V1.1 | Continue with deploy.sh |
| V1.2 | +2 months | Add analytics-job to deploy.sh |
| Pre-V1.3 | +4 months | Build ndp-deploy MVP (migrate existing declarations) |
| V1.3 | +6 months | Add model, action, feedback declarations to ndp-deploy |
| V2.0 | +9 months | Add watch mode, dependency resolution |

---

## Architecture Recommendation

### Short-Term (V1.1-V1.2): Enhance Bash

1. **Add `analytics-job` declaration** to deploy.sh
   - Leverage TimescaleDB job scheduler for correlation scanning
   - Keep scheduling simple (cron expressions)
   - Add `./deploy.sh job-status` command for monitoring

2. **Add `threshold-promoter` declaration**
   - Simple SQL-based promotion logic
   - Configured via manifest, executed via psql

3. **Improve validation**
   - Add JSON Schema validation for manifests
   - Validate cross-declaration dependencies

### Medium-Term (V1.3): Build Rust Engine

1. **Create `tools/ndp-deploy/` Rust project**
   - Start by wrapping existing bash functions
   - Add strong typing for declarations
   - Implement rollback capability

2. **Add V1.3-specific declarations**
   - Model lifecycle management
   - Action framework configuration
   - Autonomy level persistence

3. **Migrate from deploy.sh**
   - Run both in parallel initially
   - Deprecate bash after validation

### Long-Term (V2.0): Intelligent Orchestration

1. **Add watch mode**
   - Use `notify` crate for filesystem watching
   - Implement change detection (git diff based)
   - Auto-generate minimal manifests

2. **Add dependency resolution**
   - Build DAG of declarations
   - Topological sort for execution order
   - Parallel execution where possible

3. **Implement the V2.0 validation test**
   - Add new domain config
   - System automatically deploys
   - Within 15 minutes: operational

---

## Summary: Deployment Primitives Roadmap

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    DEPLOYMENT PRIMITIVES BY VERSION                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  V1.0-V1.1 (CURRENT)                                                        │
│  ───────────────────                                                        │
│  stream │ silver-table │ gold-tables │ domain │ tool │ migration           │
│  container │ dimensions │ dictionary                                        │
│                                                                             │
│  V1.2 (PLANNED)                                                             │
│  ──────────────                                                             │
│  + analytics-job      - Scheduled correlation scanning                      │
│  + threshold-promoter - Automatic candidate promotion                       │
│  + job-monitor        - Job status tracking                                 │
│                                                                             │
│  V1.3 (REQUIRES RUST ENGINE)                                                │
│  ──────────────────────────                                                 │
│  + model              - Model artifact deployment                           │
│  + model-tournament   - Model evaluation and selection                      │
│  + action             - Action definition and routing                       │
│  + feedback-loop      - Continuous learning configuration                   │
│  + autonomy-policy    - Per-action automation levels                        │
│  + prediction-endpoint - Real-time inference service                        │
│                                                                             │
│  V2.0 (REQUIRES INTELLIGENT ORCHESTRATION)                                  │
│  ─────────────────────────────────────────                                  │
│  + feature-template   - Stream-specific feature generation                  │
│  + source-adapter     - Runtime stream source registration                  │
│  + watch-mode         - Automatic change detection and deployment           │
│  + dependency-graph   - Cross-declaration dependency resolution             │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Conclusion

The current Bash-based deployment system (`deploy.sh`) is appropriate for V1.1 and can be extended for V1.2. However, the complexity introduced in V1.3 (model lifecycle, action framework, feedback loops) and the automation requirements of V2.0 (change detection, dependency resolution, auto-deployment) exceed what Bash can reasonably manage.

**Recommendation**: Plan for a **hybrid approach**:
1. Enhance deploy.sh for V1.2 (add analytics-job, threshold-promoter)
2. Build ndp-deploy Rust engine in parallel, targeting V1.3 readiness
3. Migrate from Bash to Rust by V1.3 release
4. Add intelligent orchestration features for V2.0

This approach preserves working code, allows incremental development, and positions the platform for the V2.0 vision of configuration-driven autonomous intelligence.

---

## References

- `/workspaces/neural-data-platform/product/features/gold-001/FEATURE-ROADMAP.md` - Full roadmap
- `/workspaces/neural-data-platform/docs/procedures/DEPLOYMENT-DECLARATIVES.md` - Current declarations
- `/workspaces/neural-data-platform/docs/procedures/RELEASE-POLICY.md` - Release methodology
- `/workspaces/neural-data-platform/deploy/pi/deploy.sh` - Current implementation
