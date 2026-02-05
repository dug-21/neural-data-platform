# CLI Command Structure: Roadmap Alignment Analysis

> **Date**: 2026-02-05
> **Purpose**: Validate CLI command organization against V1.1→V2.0 roadmap
> **Approach**: Architect for the future, build for today

---

## Executive Summary

The current CLI design covers **V1.1 requirements well** but needs **additional command categories** to support V1.2 (Pattern Detection), V1.3 (Prediction & Actions), and V2.0 (Multi-Stream Intelligence).

### Recommendation Summary

| Current Command | Status | Notes |
|-----------------|--------|-------|
| `ndp deploy` | ✅ Keep | Add `watch` for V2.0 |
| `ndp config` | ✅ Keep | Expand for streams/sources |
| `ndp gold` | ✅ Keep | Already planned |
| `ndp silver` | ✅ Keep | Already planned |
| `ndp validate` | ✅ Keep | Already planned |
| `ndp dictionary` | ✅ Keep | Already planned |
| `ndp mcp` | ✅ Keep | Already planned |
| **`ndp stream`** | 🆕 Add | Stream lifecycle (supersedes some config) |
| **`ndp objective`** | 🆕 Add | V1.1+ objectives management |
| **`ndp pattern`** | 🆕 Add | V1.2 correlation/candidate management |
| **`ndp model`** | 🆕 Add | V1.3 model lifecycle |
| **`ndp action`** | 🆕 Add | V1.3 action framework |
| **`ndp predict`** | 🆕 Add | V1.3 forecasting |
| **`ndp job`** | 🆕 Add | V1.2+ scheduled analytics |
| `ndp status` | ✅ Keep | System-wide status |

---

## Analysis by Roadmap Version

### V1.1: Gold Layer Foundation

**Roadmap Requirements:**
- Declarative Gold layer generation
- Stream classification (state, continuous, forecast)
- Continuous aggregates per stream
- Cross-stream alignment views
- State transition tracking
- Objectives schema
- Feature computation (lag, rolling, trend)

**Current CLI Coverage:**

| Requirement | Covered By | Gap? |
|-------------|------------|------|
| Gold aggregates | `ndp gold generate` | ✅ No |
| Gold apply | `ndp gold apply` | ✅ No |
| Stream config sync | `ndp config sync` | ✅ No |
| Stream classification | `ndp config` (stream metadata) | ⚠️ Partial |
| Objectives | `ndp config` | ⚠️ Implicit |
| Data dictionary | `ndp dictionary sync` | ✅ No |
| Silver ETL | `ndp silver etl` | ✅ No |
| Validation | `ndp validate` | ✅ No |
| Deploy manifests | `ndp deploy apply` | ✅ No |

**V1.1 Gaps Identified:**

1. **Objectives as first-class entity**: Objectives deserve their own command, not buried in `config`
2. **Stream lifecycle**: Adding/removing streams is more than config sync

**Recommendations for V1.1:**

```
# Add: ndp objective
ndp objective list                    # List all objectives
ndp objective get <id>                # Get objective details
ndp objective validate                # Validate objectives against streams
ndp objective status                  # Show target achievement status

# Enhance: ndp config → focus on low-level config
# Add: ndp stream → stream lifecycle
ndp stream list                       # List all streams with classification
ndp stream describe <id>              # Show stream details, type, status
ndp stream add <id> --type <type>     # Scaffold new stream config
ndp stream validate <id>              # Validate single stream config
```

---

### V1.2: Pattern Detection Engine

**Roadmap Requirements:**
- Correlation scanning (Granger causality)
- Transition event materialization
- Response window analysis
- Lag optimization
- Candidate ranking and promotion
- Candidate registry
- Pattern visualization

**New Entities Introduced:**
- **Candidates**: Correlation relationships flagged for validation
- **Patterns**: Validated correlations with metadata
- **Analytics Jobs**: Scheduled scanning tasks

**CLI Commands Needed:**

```
# New: ndp pattern - Correlation/candidate management
ndp pattern list                      # List all candidates/patterns
ndp pattern describe <id>             # Show correlation details
ndp pattern scan                      # Trigger correlation scan (manual)
ndp pattern scan --streams a,b        # Scan specific stream pairs
ndp pattern promote <id>              # Promote candidate to validated pattern
ndp pattern demote <id>               # Demote pattern back to candidate
ndp pattern history <id>              # Show correlation strength over time
ndp pattern export                    # Export patterns for analysis

# New: ndp job - Scheduled analytics
ndp job list                          # List scheduled jobs
ndp job create <type>                 # Create analytics job
ndp job status <id>                   # Job status
ndp job run <id>                      # Trigger job manually
ndp job logs <id>                     # View job logs
ndp job schedule <id> --cron "..."    # Set schedule

# Job types for V1.2:
#   - correlation-scan: Nightly Granger causality scan
#   - threshold-check: Check objectives against data
#   - candidate-promote: Auto-promote strong candidates
```

**Library Modules Needed:**

```
ndp-lib/
└── src/
    ├── pattern/              # V1.2
    │   ├── mod.rs
    │   ├── scanner.rs        # Granger causality implementation
    │   ├── candidate.rs      # Candidate management
    │   ├── registry.rs       # Pattern storage
    │   └── lag.rs            # Lag optimization
    └── job/                  # V1.2
        ├── mod.rs
        ├── scheduler.rs      # Job scheduling
        ├── executor.rs       # Job execution
        └── types.rs          # Job type definitions
```

---

### V1.3: Prediction & Actions

**Roadmap Requirements:**
- Causal validation (PC algorithm)
- Model zoo (TCN-Lite, ARIMA, Prophet, MLP)
- Tournament selection
- Prediction service
- Action framework (define, score, execute)
- Outcome tracking
- Feedback learning (EWC++)
- Autonomy controller

**New Entities Introduced:**
- **Models**: ML models for prediction
- **Predictions**: Forecasts with confidence
- **Actions**: Defined interventions
- **Outcomes**: Action → result pairs
- **Autonomy Policies**: Per-action automation levels

**CLI Commands Needed:**

```
# New: ndp model - Model lifecycle
ndp model list                        # List all models
ndp model describe <id>               # Model details, metrics
ndp model train <type> --pattern <p>  # Train model on validated pattern
ndp model tournament --patterns a,b   # Run model tournament
ndp model deploy <id>                 # Deploy model for predictions
ndp model retire <id>                 # Retire model
ndp model compare <a> <b>             # Compare model performance
ndp model export <id>                 # Export model artifact

# New: ndp predict - Forecasting
ndp predict <stream>.<metric>         # Get current prediction
ndp predict <stream>.<metric> --horizon 1h
ndp predict status                    # Prediction service status
ndp predict accuracy                  # Show prediction accuracy metrics
ndp predict history <stream>          # Historical predictions vs actuals

# New: ndp action - Action framework
ndp action list                       # List defined actions
ndp action define <id>                # Define new action (interactive/config)
ndp action describe <id>              # Action details, preconditions, effects
ndp action suggest                    # Get action suggestions for objectives
ndp action execute <id>               # Manually execute action
ndp action history                    # Action execution history
ndp action score <id>                 # Show predicted impact

# New: ndp outcome - Outcome tracking
ndp outcome list                      # List recent outcomes
ndp outcome record <action> --result <status>
ndp outcome analyze                   # Analyze action effectiveness
ndp outcome feedback                  # Show feedback learning status

# Enhance: ndp objective
ndp objective suggest                 # Suggest objectives from data
ndp objective forecast                # Forecast objective achievement
```

**Library Modules Needed:**

```
ndp-lib/
└── src/
    ├── model/                # V1.3
    │   ├── mod.rs
    │   ├── zoo.rs            # Model implementations
    │   ├── tournament.rs     # Model selection
    │   ├── training.rs       # Training pipeline
    │   └── artifact.rs       # Model serialization
    ├── predict/              # V1.3
    │   ├── mod.rs
    │   ├── service.rs        # Prediction service
    │   ├── forecast.rs       # Forecasting logic
    │   └── accuracy.rs       # Accuracy tracking
    ├── action/               # V1.3
    │   ├── mod.rs
    │   ├── framework.rs      # Action definitions
    │   ├── scorer.rs         # Impact scoring
    │   ├── executor.rs       # Action execution
    │   └── autonomy.rs       # Autonomy policies
    └── outcome/              # V1.3
        ├── mod.rs
        ├── tracker.rs        # Outcome recording
        ├── analyzer.rs       # Effectiveness analysis
        └── feedback.rs       # EWC++ feedback learning
```

---

### V2.0: Multi-Stream Intelligence

**Roadmap Requirements:**
- Financial stream sources (FRED, Alpaca, Finnhub)
- Cross-stream pattern detection (no domain boundaries)
- Multi-stream objectives
- Seeded financial models
- Stream-specific feature templates
- Unified dashboard
- Watch mode (auto-detect and deploy)

**New Entities Introduced:**
- **Sources**: Stream source types (HTTP, MQTT, Financial APIs)
- **Domains**: Declared groupings of streams + objectives
- **Templates**: Feature computation templates per stream type

**CLI Commands Needed:**

```
# Enhance: ndp stream
ndp stream sources                    # List available source types
ndp stream add-source <type>          # Add new source type (plugin)

# New: ndp domain - Multi-domain management
ndp domain list                       # List declared domains
ndp domain create <id>                # Create new domain
ndp domain describe <id>              # Domain details
ndp domain streams <id>               # Streams in domain
ndp domain objectives <id>            # Objectives in domain
ndp domain patterns <id>              # Patterns affecting domain

# Enhance: ndp pattern
ndp pattern cross-domain              # Find patterns spanning domains
ndp pattern discover                  # Auto-discover new patterns

# Enhance: ndp predict
ndp predict domain <id>               # Predictions for entire domain

# Enhance: ndp deploy
ndp deploy watch                      # Watch mode - auto-detect changes
ndp deploy diff                       # Show what would change
ndp deploy plan                       # Generate deployment plan

# New: ndp template - Feature templates
ndp template list                     # List feature templates
ndp template apply <stream> <template>
ndp template create                   # Define new template
```

**Library Modules Needed:**

```
ndp-lib/
└── src/
    ├── source/               # V2.0
    │   ├── mod.rs
    │   ├── registry.rs       # Source type registry
    │   ├── financial.rs      # Financial API sources
    │   └── plugin.rs         # Source plugins
    ├── domain/               # V2.0
    │   ├── mod.rs
    │   ├── manager.rs        # Domain lifecycle
    │   └── resolver.rs       # Cross-domain resolution
    └── template/             # V2.0
        ├── mod.rs
        └── registry.rs       # Feature templates
```

---

## Revised Command Structure

### Complete CLI Design (V1.1 → V2.0)

```
ndp - Neural Data Platform CLI

CORE COMMANDS (Build Now - V1.1):
    deploy      Deployment operations
    config      Low-level configuration
    stream      Stream lifecycle management
    gold        Gold layer operations
    silver      Silver layer operations
    validate    Validation commands
    dictionary  Data dictionary operations
    objective   Objectives management
    mcp         MCP server operations
    status      System status

INTELLIGENCE COMMANDS (Build V1.2+):
    pattern     Pattern/correlation management        [V1.2]
    job         Scheduled analytics jobs              [V1.2]
    model       Model lifecycle                       [V1.3]
    predict     Forecasting operations                [V1.3]
    action      Action framework                      [V1.3]
    outcome     Outcome tracking                      [V1.3]

ADVANCED COMMANDS (Build V2.0):
    domain      Multi-domain management               [V2.0]
    template    Feature templates                     [V2.0]
```

### Command Hierarchy

```
ndp
├── deploy
│   ├── apply           # Apply manifest
│   ├── status          # Current state
│   ├── diff            # Show changes
│   ├── rollback        # Rollback
│   └── watch           # [V2.0] Auto-deploy on changes
│
├── config
│   ├── sync            # Sync all to etcd
│   ├── get             # Get value
│   ├── set             # Set value
│   └── list            # List configs
│
├── stream
│   ├── list            # List streams with classification
│   ├── describe        # Stream details
│   ├── add             # Scaffold new stream
│   ├── validate        # Validate stream config
│   ├── status          # Stream health/data status
│   └── sources         # [V2.0] List source types
│
├── gold
│   ├── generate        # Generate DDL
│   ├── apply           # Apply DDL
│   ├── status          # Aggregate status
│   └── refresh         # Force refresh aggregates
│
├── silver
│   ├── migrate         # Run migrations
│   ├── etl             # Run ETL once
│   ├── status          # ETL status
│   └── daemon          # Start ETL daemon
│
├── validate
│   ├── config          # Validate configs
│   ├── manifest        # Validate manifest
│   ├── schema          # Validate against schema
│   └── all             # Validate everything
│
├── dictionary
│   ├── sync            # Sync to database
│   ├── query           # Search dictionary
│   ├── describe        # Column details
│   └── lineage         # Trace lineage
│
├── objective                              # [V1.1+]
│   ├── list            # List objectives
│   ├── get             # Get details
│   ├── validate        # Validate against streams
│   ├── status          # Achievement status
│   ├── suggest         # [V1.3] Suggest from data
│   └── forecast        # [V1.3] Forecast achievement
│
├── pattern                                # [V1.2]
│   ├── list            # List candidates/patterns
│   ├── describe        # Correlation details
│   ├── scan            # Trigger scan
│   ├── promote         # Promote to validated
│   ├── demote          # Demote to candidate
│   ├── history         # Correlation over time
│   ├── discover        # [V2.0] Auto-discover
│   └── cross-domain    # [V2.0] Cross-domain patterns
│
├── job                                    # [V1.2]
│   ├── list            # List jobs
│   ├── create          # Create job
│   ├── status          # Job status
│   ├── run             # Manual trigger
│   ├── logs            # View logs
│   └── schedule        # Set schedule
│
├── model                                  # [V1.3]
│   ├── list            # List models
│   ├── describe        # Model details
│   ├── train           # Train model
│   ├── tournament      # Run tournament
│   ├── deploy          # Deploy model
│   ├── retire          # Retire model
│   ├── compare         # Compare models
│   └── export          # Export artifact
│
├── predict                                # [V1.3]
│   ├── <stream.metric> # Get prediction
│   ├── status          # Service status
│   ├── accuracy        # Accuracy metrics
│   └── history         # Predictions vs actuals
│
├── action                                 # [V1.3]
│   ├── list            # List actions
│   ├── define          # Define action
│   ├── describe        # Action details
│   ├── suggest         # Get suggestions
│   ├── execute         # Execute action
│   ├── history         # Execution history
│   └── score           # Predicted impact
│
├── outcome                                # [V1.3]
│   ├── list            # Recent outcomes
│   ├── record          # Record outcome
│   ├── analyze         # Effectiveness analysis
│   └── feedback        # Learning status
│
├── domain                                 # [V2.0]
│   ├── list            # List domains
│   ├── create          # Create domain
│   ├── describe        # Domain details
│   ├── streams         # Domain streams
│   ├── objectives      # Domain objectives
│   └── patterns        # Domain patterns
│
├── template                               # [V2.0]
│   ├── list            # List templates
│   ├── apply           # Apply template
│   └── create          # Create template
│
├── mcp
│   ├── serve           # Start MCP server
│   └── tools           # List MCP tools
│
└── status
    ├── system          # Overall status
    ├── services        # Service health
    └── data            # Data freshness
```

---

## Library Module Evolution

### Phase 1: V1.1 (Foundation)

```
ndp-lib/src/
├── manifest/           ✓ Already planned
├── config/             ✓ Already planned
├── deploy/             ✓ Already planned
├── gold/               ✓ Already planned
├── silver/             ✓ Already planned
├── dictionary/         ✓ Already planned
├── validate/           ✓ Already planned
├── db/                 ✓ Already planned
├── stream/             🆕 Add - stream lifecycle
└── objective/          🆕 Add - objectives management
```

### Phase 2: V1.2 (Pattern Detection)

```
ndp-lib/src/
├── [all V1.1 modules]
├── pattern/            🆕 Add
│   ├── scanner.rs
│   ├── candidate.rs
│   ├── registry.rs
│   └── lag.rs
└── job/                🆕 Add
    ├── scheduler.rs
    ├── executor.rs
    └── types.rs
```

### Phase 3: V1.3 (Prediction & Actions)

```
ndp-lib/src/
├── [all V1.2 modules]
├── model/              🆕 Add
│   ├── zoo.rs
│   ├── tournament.rs
│   ├── training.rs
│   └── artifact.rs
├── predict/            🆕 Add
│   ├── service.rs
│   ├── forecast.rs
│   └── accuracy.rs
├── action/             🆕 Add
│   ├── framework.rs
│   ├── scorer.rs
│   ├── executor.rs
│   └── autonomy.rs
└── outcome/            🆕 Add
    ├── tracker.rs
    ├── analyzer.rs
    └── feedback.rs
```

### Phase 4: V2.0 (Multi-Stream Intelligence)

```
ndp-lib/src/
├── [all V1.3 modules]
├── source/             🆕 Add
│   ├── registry.rs
│   ├── financial.rs
│   └── plugin.rs
├── domain/             🆕 Add
│   ├── manager.rs
│   └── resolver.rs
└── template/           🆕 Add
    └── registry.rs
```

---

## Recommendations

### 1. Add `ndp stream` and `ndp objective` Now (V1.1)

These are first-class entities that deserve their own commands:

```bash
# Today: buried in config
ndp config sync  # syncs streams AND objectives together

# Better: explicit commands
ndp stream list
ndp stream add indoor-air-quality --type observation
ndp objective list
ndp objective status
```

**Rationale**: Objectives are central to the entire roadmap. They drive:
- V1.1: Which Gold features to compute
- V1.2: Which correlations are "relevant"
- V1.3: What models optimize toward
- V2.0: Multi-domain goal specification

### 2. Reserve `ndp pattern`, `ndp model`, `ndp action` Namespaces

Don't implement yet, but reserve the command structure:

```bash
ndp pattern --help
# "Pattern commands will be available in V1.2. See roadmap."

ndp model --help
# "Model commands will be available in V1.3. See roadmap."
```

**Rationale**: Establishes mental model early. Users/contributors know where features will live.

### 3. Design `ndp job` as Generic Scheduler

Not just for V1.2 pattern scanning, but extensible for:
- V1.2: Correlation scans, threshold checks
- V1.3: Model training, tournament runs
- V2.0: Cross-domain discovery

```rust
// ndp-lib/src/job/types.rs
pub enum JobType {
    // V1.2
    CorrelationScan,
    ThresholdCheck,
    CandidatePromote,

    // V1.3
    ModelTrain,
    ModelTournament,
    PredictionRefresh,

    // V2.0
    CrossDomainDiscovery,
    PatternStabilityCheck,
}
```

### 4. MCP Tool Naming Convention

Ensure MCP tools mirror CLI commands for discoverability:

| CLI Command | MCP Tool |
|-------------|----------|
| `ndp stream list` | `ndp_stream_list` |
| `ndp objective status` | `ndp_objective_status` |
| `ndp pattern scan` | `ndp_pattern_scan` |
| `ndp predict <stream>` | `ndp_predict` |
| `ndp action suggest` | `ndp_action_suggest` |

### 5. Consider `ndp intelligence` Meta-Command (V2.0)

For V2.0's "intelligent deployment" vision:

```bash
ndp intelligence status    # Overall intelligence health
ndp intelligence explain   # Why did system suggest X?
ndp intelligence audit     # Review autonomous decisions
ndp intelligence tune      # Adjust learning parameters
```

---

## Updated Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              ndp CLI                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  CORE (V1.1)              INTELLIGENCE (V1.2-1.3)      ADVANCED (V2.0)      │
│  ───────────              ───────────────────────      ────────────────      │
│  deploy                   pattern                      domain                │
│  config                   job                          template              │
│  stream ← NEW             model                        intelligence          │
│  gold                     predict                                            │
│  silver                   action                                             │
│  validate                 outcome                                            │
│  dictionary                                                                  │
│  objective ← NEW                                                             │
│  mcp                                                                         │
│  status                                                                      │
│                                                                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                              ndp-lib                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  V1.1 Modules          V1.2 Modules       V1.3 Modules       V2.0 Modules   │
│  ─────────────         ────────────       ────────────       ────────────   │
│  manifest              pattern            model              source         │
│  config                job                predict            domain         │
│  deploy                                   action             template       │
│  gold                                     outcome                           │
│  silver                                                                     │
│  dictionary                                                                 │
│  validate                                                                   │
│  db                                                                         │
│  stream ← NEW                                                               │
│  objective ← NEW                                                            │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Summary: Changes from Original Design

| Original | Revised | Rationale |
|----------|---------|-----------|
| 7 commands | 16 commands | Full roadmap coverage |
| No `stream` | Add `ndp stream` | Stream is first-class entity |
| No `objective` | Add `ndp objective` | Central to entire platform |
| No `pattern` | Add `ndp pattern` | V1.2 core capability |
| No `job` | Add `ndp job` | Generic scheduler for all versions |
| No `model` | Add `ndp model` | V1.3 model lifecycle |
| No `predict` | Add `ndp predict` | V1.3 forecasting |
| No `action` | Add `ndp action` | V1.3 action framework |
| No `outcome` | Add `ndp outcome` | V1.3 feedback loop |
| No `domain` | Add `ndp domain` | V2.0 multi-domain |
| No `template` | Add `ndp template` | V2.0 feature templates |

---

## Build Plan

| Version | New Commands | Library Modules |
|---------|--------------|-----------------|
| **V1.1** | `stream`, `objective` | `stream/`, `objective/` |
| **V1.2** | `pattern`, `job` | `pattern/`, `job/` |
| **V1.3** | `model`, `predict`, `action`, `outcome` | `model/`, `predict/`, `action/`, `outcome/` |
| **V2.0** | `domain`, `template`, `intelligence` | `source/`, `domain/`, `template/` |

Each version builds on the previous, and the library-first architecture ensures CLI and MCP stay in sync automatically.
