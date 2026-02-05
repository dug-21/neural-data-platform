# Standard Verbs & MCP Integration Analysis

> **Date**: 2026-02-05
> **Purpose**: Define standard verb set and integrate existing MCP tools with CLI
> **Approach**: Consistent verbs across CLI commands and MCP tools

---

## Part 1: Existing MCP Tools Inventory

### Current MCP Tools (14 total)

| Tool | Category | Verb | Operation |
|------|----------|------|-----------|
| `list_streams` | Bronze | list | READ |
| `describe_schema` | Bronze | describe | READ |
| `validate_config` | Bronze | validate | READ |
| `sample_data` | Bronze | sample | READ |
| `list_silver_tables` | Silver | list | READ |
| `describe_silver_table` | Silver | describe | READ |
| `sample_silver_data` | Silver | sample | READ |
| `silver_stats` | Silver | stats | READ |
| `etl_status` | ETL | status | READ |
| `etl_history` | ETL | history | READ |
| `data_freshness` | ETL | freshness | READ |
| `query_dictionary` | Dictionary | query | READ |
| `describe_column` | Dictionary | describe | READ |
| `trace_lineage` | Dictionary | trace | READ |
| `list_dq_rules` | Dictionary | list | READ |

### Key Observations

1. **All existing tools are READ-ONLY** - designed for exploration and troubleshooting
2. **No mutation operations** - no create, update, delete
3. **Category-focused** - tools organized by data layer (Bronze, Silver, ETL, Dictionary)
4. **Inconsistent naming** - some use `_` separator, different verb positions

---

## Part 2: Standard Verb Definitions

### Core CRUD Verbs

| Verb | Operation | Description | Example |
|------|-----------|-------------|---------|
| **list** | READ | List multiple resources | `ndp stream list` |
| **get** | READ | Get single resource by ID | `ndp stream get air-quality` |
| **describe** | READ | Get detailed information | `ndp stream describe air-quality` |
| **create** | CREATE | Create new resource | `ndp stream create` |
| **update** | UPDATE | Modify existing resource | `ndp stream update air-quality` |
| **delete** | DELETE | Remove resource | `ndp stream delete air-quality` |

### Extended Verbs

| Verb | Operation | Description | Example |
|------|-----------|-------------|---------|
| **validate** | READ | Check validity without mutation | `ndp stream validate air-quality` |
| **sync** | WRITE | Synchronize state (idempotent) | `ndp config sync` |
| **apply** | WRITE | Apply changes from file/manifest | `ndp deploy apply manifest.json` |
| **status** | READ | Get current operational status | `ndp etl status` |
| **history** | READ | Get historical records | `ndp etl history` |
| **sample** | READ | Get sample data | `ndp bronze sample air-quality` |
| **query** | READ | Search with filters | `ndp dictionary query "temperature"` |
| **trace** | READ | Follow relationships/lineage | `ndp dictionary trace column` |
| **stats** | READ | Get statistics | `ndp silver stats air-quality` |
| **diff** | READ | Compare states | `ndp deploy diff manifest.json` |
| **watch** | READ | Monitor continuously | `ndp deploy watch` |
| **run** | EXECUTE | Execute once (jobs, ETL) | `ndp job run scan-correlations` |
| **start** | EXECUTE | Start daemon/service | `ndp etl start` |
| **stop** | EXECUTE | Stop daemon/service | `ndp etl stop` |

### Verb Consistency Rules

1. **`list` vs `get`**: `list` returns multiple, `get` returns one by ID
2. **`get` vs `describe`**: `get` returns data, `describe` returns metadata/schema
3. **`create` vs `add`**: Use `create` (more explicit about resource creation)
4. **`delete` vs `remove`**: Use `delete` (standard CRUD terminology)
5. **`update` vs `set`**: Use `update` for resources, `set` for config values

---

## Part 3: Should MCP Tools Merge with CLI?

### Analysis

| Factor | Separate MCP Server | Merged in CLI |
|--------|---------------------|---------------|
| **Binary size** | Smaller CLI | Slightly larger |
| **Code sharing** | Duplication risk | Guaranteed same logic |
| **Deployment** | 2 binaries | 1 binary |
| **Testing** | Test both | Test once |
| **Consistency** | May diverge | Always in sync |
| **Startup time** | CLI faster | Same |

### Recommendation: **Merge**

**Rationale:**
1. **MCP tools ARE CLI commands** - they perform the same operations
2. **Library-first design** - both call `ndp-lib`, just different interfaces
3. **Single binary simplicity** - easier Pi deployment
4. **Guaranteed consistency** - same code path

### Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                          ndp binary                                  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   CLI Interface                    MCP Interface                    │
│   ─────────────                    ─────────────                    │
│   ndp stream list          ←→      list_streams                     │
│   ndp stream describe      ←→      describe_schema                  │
│   ndp silver list          ←→      list_silver_tables               │
│   ndp silver describe      ←→      describe_silver_table            │
│   ndp etl status           ←→      etl_status                       │
│   ndp dictionary query     ←→      query_dictionary                 │
│                                                                      │
│                          ▼                                          │
│   ┌─────────────────────────────────────────────────────────────┐  │
│   │                       ndp-lib                                │  │
│   │                                                              │  │
│   │   stream::list()      silver::describe()     etl::status()  │  │
│   │   stream::describe()  silver::sample()       etl::history() │  │
│   │   bronze::sample()    silver::stats()        dict::query()  │  │
│   │                                                              │  │
│   └─────────────────────────────────────────────────────────────┘  │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Part 4: Revised Command Structure (with MCP Integration)

### Does MCP Change the Design?

**Yes, in important ways:**

1. **Need `ndp bronze` command** - MCP has Bronze-specific tools not in original CLI design
2. **`describe` should be standard verb** - MCP uses it heavily, CLI should too
3. **`sample` is valuable** - MCP has it, CLI should expose it
4. **Category alignment** - CLI categories should mirror MCP categories for mental model consistency

### Original vs Revised

| Original | Revised | Reason |
|----------|---------|--------|
| No bronze command | `ndp bronze` | MCP has `list_streams`, `sample_data`, `describe_schema` |
| `ndp stream list` | `ndp stream list` | Keep - but move Bronze-specific to `bronze` |
| No sample | `ndp bronze sample` | MCP has `sample_data`, `sample_silver_data` |
| No describe | `ndp stream describe` | Standard verb from MCP |
| Mixed etl commands | `ndp etl` category | MCP has `etl_status`, `etl_history`, `data_freshness` |

---

## Part 4.5: Manifest Declaration ↔ CLI Mapping

**Critical Design Principle**: CLI commands mirror manifest declarations exactly.

The deployment manifest (see `docs/procedures/DEPLOYMENT-DECLARATIVES.md`) uses:
```json
{"type": "<entity>", "action": "<verb>", "id": "<identifier>"}
```

This maps 1:1 to CLI:
```
ndp <entity> <verb> [identifier]
```

### Complete Mapping Table

| Manifest Declaration | CLI Command | Phase |
|---------------------|-------------|-------|
| `{"type": "stream", "id": "X", "action": "create"}` | `ndp stream create X` | 7 |
| `{"type": "stream", "id": "X", "action": "update"}` | `ndp stream update X` | 7 |
| `{"type": "stream", "id": "X", "action": "sync"}` | `ndp stream sync X` | 7 |
| `{"type": "silver-table", "stream_id": "X", "action": "sync"}` | `ndp silver sync X` | 4 |
| `{"type": "gold-tables", "stream_id": "X", "action": "sync"}` | `ndp gold sync X` | 5 |
| `{"type": "gold-tables", "stream_id": "X", "action": "recreate"}` | `ndp gold recreate X` | 5 |
| `{"type": "domain", "domain_id": "X", "action": "sync"}` | `ndp domain sync X` | 6 |
| `{"type": "dimensions", "action": "sync"}` | `ndp dimension sync` | 8 |
| `{"type": "dictionary", "action": "sync"}` | `ndp dictionary sync` | 9 |
| `{"type": "tool", "id": "X", "action": "build"}` | *(deploy.sh / CI/CD)* | 2.5 |
| `{"type": "container", "target": "X", "action": "build"}` | *(deploy.sh / CI/CD)* | 2 |
| `{"type": "container", "target": "X", "action": "restart"}` | `ndp container restart X` | 10 |
| `{"type": "migration", "file": "X"}` | *(deploy.sh only)* | 3 |

**NOTE:** Build operations (`tool build`, `container build`) and migrations remain in
`deploy.sh` or CI/CD pipelines. The `ndp` CLI focuses on runtime operations and
troubleshooting, not build-time actions.

### How `ndp deploy apply` Works

```
ndp deploy apply manifest.json

1. Parse manifest declarations
2. Group by phase (1-11)
3. For each declaration:
   - Map to ndp-lib function: stream::sync(), gold::sync(), etc.
   - Execute in phase order
4. Update device state
```

**The same library functions are called whether you:**
- Use declarative: `ndp deploy apply manifest.json`
- Use imperative: `ndp stream sync air-quality`

### Boundary: deploy.sh vs ndp CLI

| Responsibility | Tool | Rationale |
|----------------|------|-----------|
| **Build operations** | `deploy.sh` / CI/CD | CPU-intensive, may move to GitHub Actions |
| - Docker image builds | `deploy.sh` | |
| - Rust tool compilation | `deploy.sh` | |
| - SQL migrations | `deploy.sh` | Deployment-time only, not imperative |
| **Runtime operations** | `ndp` CLI | Troubleshooting, exploration, MCP |
| - Entity sync (stream, gold, etc.) | `ndp` | Imperative + manifest-driven |
| - Container restart/logs | `ndp` | Troubleshooting |
| - Data layer queries | `ndp` | Exploration, MCP tools |
| - ETL control | `ndp` | Runtime operations |

**Future state:** `deploy.sh apply` becomes thin wrapper calling `ndp deploy apply`,
which orchestrates `ndp-lib` functions. Build operations may move to CI/CD.

---

## Part 5: Complete Revised Command Structure

### Command Categories

```
ndp
├── INFRASTRUCTURE ──────────────────────────────────────────────────
│   deploy      Deployment operations
│   config      Low-level configuration
│   container   Container troubleshooting (logs, restart)
│   mcp         MCP server operations
│   status      System-wide status
│
├── DATA LAYERS ─────────────────────────────────────────────────────
│   bronze      Bronze layer operations (Parquet/raw data)
│   silver      Silver layer operations (TimescaleDB)
│   gold        Gold layer operations (aggregates/features)
│
├── ENTITIES ────────────────────────────────────────────────────────
│   stream      Stream lifecycle management
│   domain      Domain/aligned views management
│   objective   Objectives management
│   dimension   Dimension table management
│   dictionary  Data dictionary operations
│
├── OPERATIONS ──────────────────────────────────────────────────────
│   etl         ETL operations and monitoring
│   validate    Validation commands
│
├── INTELLIGENCE (V1.2+) ────────────────────────────────────────────
│   pattern     Pattern/correlation management
│   job         Scheduled analytics jobs
│
├── PREDICTION (V1.3+) ──────────────────────────────────────────────
│   model       Model lifecycle
│   predict     Forecasting operations
│   action      Action framework
│   outcome     Outcome tracking
│
└── ADVANCED (V2.0+) ────────────────────────────────────────────────
    domain      Multi-domain management
    template    Feature templates
```

### Full Command Reference with Standard Verbs

```
ndp deploy
    apply       Apply deployment manifest               [WRITE]
    status      Show current deployment state           [READ]
    diff        Show changes between manifest/state     [READ]
    rollback    Rollback to previous deployment         [WRITE]
    watch       Watch for changes, auto-deploy (V2.0)   [READ/WRITE]
    history     Show deployment history                 [READ]

ndp config
    get <key>   Get configuration value from etcd       [READ]
    set <key>   Set configuration value in etcd         [WRITE]
    list        List all configuration keys             [READ]
    validate    Validate all configurations             [READ]
    # NOTE: sync is on entities (stream, domain, etc.) not here

ndp bronze                                              # NEW CATEGORY
    list        List Bronze streams with storage info   [READ] ← list_streams
    describe    Get Bronze stream schema                [READ] ← describe_schema
    sample      Get sample rows from Bronze             [READ] ← sample_data
    validate    Validate Bronze data against config     [READ] ← validate_config
    status      Show Bronze layer health                [READ]

ndp silver
    list        List Silver hypertables                 [READ] ← list_silver_tables
    describe    Get Silver table schema                 [READ] ← describe_silver_table
    sample      Get sample rows from Silver             [READ] ← sample_silver_data
    stats       Get Silver table statistics             [READ] ← silver_stats
    sync <id>   Sync Silver table DDL from stream config[WRITE] ← manifest: {"type":"silver-table","action":"sync"}
    status      Show Silver layer health                [READ]

ndp gold
    list        List Gold aggregates/views              [READ]
    describe    Get Gold aggregate details              [READ]
    generate    Generate Gold DDL from config           [READ]
    sync <id>   Sync Gold tables (create if not exists) [WRITE] ← manifest: {"type":"gold-tables","action":"sync"}
    recreate    Drop and recreate Gold tables           [WRITE] ← manifest: {"type":"gold-tables","action":"recreate"}
    refresh     Force refresh aggregates                [WRITE]
    status      Show Gold layer health                  [READ]

ndp stream
    list        List all streams with classification    [READ]
    get <id>    Get stream configuration                [READ]
    describe    Get stream details (schema, status)     [READ]
    create      Create new stream (scaffold config)     [WRITE]
    update      Update stream configuration             [WRITE]
    sync [id]   Sync stream config(s) to etcd           [WRITE]  ← manifest: {"type":"stream","action":"sync"}
    delete      Delete stream                           [WRITE]
    validate    Validate stream configuration           [READ]
    status      Show stream health (data flow)          [READ]

ndp objective
    list        List all objectives                     [READ]
    get <id>    Get objective configuration             [READ]
    describe    Get objective details                   [READ]
    create      Create new objective                    [WRITE]
    update      Update objective                        [WRITE]
    delete      Delete objective                        [WRITE]
    validate    Validate objectives against streams     [READ]
    status      Show objective achievement status       [READ]
    forecast    Forecast objective achievement (V1.3)   [READ]

ndp dictionary
    list        List dictionary entries                 [READ] ← list_dq_rules (partial)
    query       Search dictionary                       [READ] ← query_dictionary
    describe    Get column details                      [READ] ← describe_column
    trace       Trace column lineage                    [READ] ← trace_lineage
    sync        Sync dictionary to database             [WRITE] ← manifest: {"type":"dictionary","action":"sync"}

ndp etl
    status      Get ETL status for streams              [READ] ← etl_status
    history     Get ETL run history                     [READ] ← etl_history
    freshness   Check data freshness                    [READ] ← data_freshness
    run         Run ETL once                            [EXECUTE]
    start       Start ETL daemon                        [EXECUTE]
    stop        Stop ETL daemon                         [EXECUTE]
    logs        View ETL logs                           [READ]

ndp dimension
    list        List dimension tables                   [READ]
    describe    Get dimension schema and stats          [READ]
    sync        Sync dimension CSVs to database         [WRITE] ← manifest: {"type":"dimensions","action":"sync"}
    validate    Validate dimension data                 [READ]
    status      Show dimension health                   [READ]

ndp validate
    config      Validate stream/objective configs       [READ]
    manifest    Validate deployment manifest            [READ]
    schema      Validate against JSON schema            [READ]
    all         Validate everything                     [READ]

ndp pattern (V1.2)
    list        List correlation candidates/patterns    [READ]
    get <id>    Get pattern details                     [READ]
    describe    Get pattern analysis                    [READ]
    scan        Trigger correlation scan                [EXECUTE]
    promote     Promote candidate to pattern            [WRITE]
    demote      Demote pattern to candidate             [WRITE]
    delete      Delete pattern                          [WRITE]
    history     Show correlation strength over time     [READ]

ndp job (V1.2)
    list        List scheduled jobs                     [READ]
    get <id>    Get job configuration                   [READ]
    describe    Get job details and history             [READ]
    create      Create new job                          [WRITE]
    update      Update job configuration                [WRITE]
    delete      Delete job                              [WRITE]
    run         Trigger job manually                    [EXECUTE]
    status      Get job status                          [READ]
    logs        View job logs                           [READ]
    schedule    Set job schedule                        [WRITE]

ndp model (V1.3)
    list        List all models                         [READ]
    get <id>    Get model details                       [READ]
    describe    Get model metrics and info              [READ]
    train       Train model on pattern                  [EXECUTE]
    tournament  Run model tournament                    [EXECUTE]
    deploy      Deploy model for predictions            [WRITE]
    retire      Retire model                            [WRITE]
    delete      Delete model                            [WRITE]
    compare     Compare model performance               [READ]
    export      Export model artifact                   [READ]

ndp predict (V1.3)
    get         Get prediction for metric               [READ]
    list        List active predictions                 [READ]
    status      Prediction service status               [READ]
    accuracy    Show prediction accuracy                [READ]
    history     Predictions vs actuals                  [READ]

ndp action (V1.3)
    list        List defined actions                    [READ]
    get <id>    Get action configuration                [READ]
    describe    Get action details                      [READ]
    create      Define new action                       [WRITE]
    update      Update action                           [WRITE]
    delete      Delete action                           [WRITE]
    suggest     Get action suggestions                  [READ]
    execute     Execute action                          [EXECUTE]
    history     Action execution history                [READ]
    score       Get predicted impact                    [READ]

ndp outcome (V1.3)
    list        List recent outcomes                    [READ]
    get <id>    Get outcome details                     [READ]
    record      Record action outcome                   [WRITE]
    analyze     Analyze effectiveness                   [READ]
    feedback    Show learning status                    [READ]

ndp domain (V1.1+ aligned views, V2.0 multi-domain)
    list        List domains                            [READ]
    get <id>    Get domain configuration                [READ]
    describe    Get domain details                      [READ]
    create      Create domain                           [WRITE]
    update      Update domain                           [WRITE]
    sync <id>   Sync domain to etcd + generate views    [WRITE] ← manifest: {"type":"domain","action":"sync"}
    delete      Delete domain                           [WRITE]
    streams     List streams in domain                  [READ]
    objectives  List objectives in domain               [READ]
    patterns    List patterns in domain (V2.0)          [READ]

ndp template (V2.0)
    list        List feature templates                  [READ]
    get <id>    Get template configuration              [READ]
    describe    Get template details                    [READ]
    create      Create template                         [WRITE]
    apply       Apply template to stream                [WRITE]
    delete      Delete template                         [WRITE]

ndp container (troubleshooting only - builds stay in deploy.sh/CI)
    list        List running containers                 [READ]
    status      Show container health                   [READ]
    logs <tgt>  View container logs                     [READ]
    restart     Restart container                       [EXECUTE] ← manifest: {"type":"container","action":"restart"}

ndp mcp
    serve       Start MCP server                        [EXECUTE]
    tools       List available MCP tools                [READ]
    status      MCP server status                       [READ]

ndp status
    system      Overall system status                   [READ]
    services    Service health                          [READ]
    data        Data freshness                          [READ]
```

---

## Part 6: MCP Tool Naming Convention

### Mapping: CLI → MCP Tool Name

| CLI Command | MCP Tool Name | Notes |
|-------------|---------------|-------|
| `ndp bronze list` | `bronze_list` | Changed from `list_streams` |
| `ndp bronze describe <id>` | `bronze_describe` | Changed from `describe_schema` |
| `ndp bronze sample <id>` | `bronze_sample` | Changed from `sample_data` |
| `ndp bronze validate <id>` | `bronze_validate` | Changed from `validate_config` |
| `ndp silver list` | `silver_list` | Changed from `list_silver_tables` |
| `ndp silver describe <id>` | `silver_describe` | Changed from `describe_silver_table` |
| `ndp silver sample <id>` | `silver_sample` | Changed from `sample_silver_data` |
| `ndp silver stats <id>` | `silver_stats` | Same |
| `ndp etl status` | `etl_status` | Same |
| `ndp etl history` | `etl_history` | Same |
| `ndp etl freshness` | `etl_freshness` | Changed from `data_freshness` |
| `ndp dictionary query` | `dictionary_query` | Changed from `query_dictionary` |
| `ndp dictionary describe` | `dictionary_describe` | Changed from `describe_column` |
| `ndp dictionary trace` | `dictionary_trace` | Changed from `trace_lineage` |
| `ndp dictionary list` | `dictionary_list` | Changed from `list_dq_rules` |

### Naming Pattern

```
{category}_{verb}

Examples:
  stream_list
  stream_describe
  bronze_sample
  silver_stats
  etl_status
  pattern_scan
  model_train
  action_suggest
```

---

## Part 7: Library Module Alignment

### Module ↔ Command ↔ MCP Tool ↔ Manifest Alignment

```
ndp-lib/src/
├── bronze/              # ndp bronze *    → bronze_* MCP tools
│   ├── list.rs
│   ├── describe.rs
│   ├── sample.rs
│   └── validate.rs
├── silver/              # ndp silver *    → silver_* MCP tools
│   ├── list.rs          #                 → manifest: {"type":"silver-table"}
│   ├── describe.rs
│   ├── sample.rs
│   ├── stats.rs
│   └── sync.rs          # ndp silver sync → manifest action: "sync"
├── gold/                # ndp gold *      → gold_* MCP tools
│   ├── list.rs          #                 → manifest: {"type":"gold-tables"}
│   ├── describe.rs
│   ├── generate.rs
│   ├── sync.rs          # ndp gold sync   → manifest action: "sync"
│   └── recreate.rs      # ndp gold recreate → manifest action: "recreate"
├── stream/              # ndp stream *    → stream_* MCP tools
│   ├── list.rs          #                 → manifest: {"type":"stream"}
│   ├── get.rs
│   ├── describe.rs
│   ├── create.rs        # ndp stream create → manifest action: "create"
│   ├── update.rs        # ndp stream update → manifest action: "update"
│   ├── sync.rs          # ndp stream sync   → manifest action: "sync"
│   └── delete.rs
├── domain/              # ndp domain *    → domain_* MCP tools
│   ├── list.rs          #                 → manifest: {"type":"domain"}
│   ├── get.rs
│   ├── describe.rs
│   ├── sync.rs          # ndp domain sync → manifest action: "sync"
│   └── ...
├── dimension/           # ndp dimension * → dimension_* MCP tools
│   ├── list.rs          #                 → manifest: {"type":"dimensions"}
│   ├── describe.rs
│   ├── sync.rs          # ndp dimension sync → manifest action: "sync"
│   └── validate.rs
├── objective/           # ndp objective * → objective_* MCP tools
│   ├── list.rs
│   ├── get.rs
│   ├── status.rs
│   └── ...
├── dictionary/          # ndp dictionary *→ dictionary_* MCP tools
│   ├── list.rs          #                 → manifest: {"type":"dictionary"}
│   ├── query.rs
│   ├── describe.rs
│   ├── trace.rs
│   └── sync.rs          # ndp dictionary sync → manifest action: "sync"
├── etl/                 # ndp etl *       → etl_* MCP tools
│   ├── status.rs
│   ├── history.rs
│   ├── freshness.rs
│   └── run.rs
├── deploy/              # ndp deploy *    → deploy_* MCP tools
│   ├── apply.rs         # Orchestrates manifest declarations
│   ├── status.rs
│   └── diff.rs
├── config/              # ndp config *    → config_* MCP tools
│   ├── get.rs           # Get from etcd
│   └── set.rs           # Set in etcd (NOTE: sync is on entities, not here)
└── validate/            # ndp validate *  → validate_* MCP tools
    ├── config.rs
    ├── manifest.rs
    └── schema.rs
```

---

## Part 8: Summary of Changes

### From Original CLI Design

| Change | Reason |
|--------|--------|
| Added `ndp bronze` | MCP has Bronze-specific tools |
| Standard verbs defined | Consistency across commands |
| `describe` added everywhere | MCP pattern, valuable for details |
| `sample` added to layers | MCP pattern, useful for debugging |
| MCP tool names restructured | Match `{category}_{verb}` pattern |
| `get` vs `describe` clarified | `get` = data, `describe` = metadata |

### Final Command Count

| Category | Commands | Verbs Used | Manifest Type |
|----------|----------|------------|---------------|
| deploy | 6 | apply, status, diff, rollback, watch, history | - |
| config | 4 | get, set, list, validate | - |
| container | 4 | list, status, logs, restart | `container` (restart only) |
| bronze | 5 | list, describe, sample, validate, status | - |
| silver | 6 | list, describe, sample, stats, sync, status | `silver-table` |
| gold | 7 | list, describe, generate, sync, recreate, refresh, status | `gold-tables` |
| stream | 9 | list, get, describe, create, update, sync, delete, validate, status | `stream` |
| domain | 10 | list, get, describe, create, update, sync, delete, streams, objectives, patterns | `domain` |
| dimension | 5 | list, describe, sync, validate, status | `dimensions` |
| objective | 9 | list, get, describe, create, update, delete, validate, status, forecast | - |
| dictionary | 5 | list, query, describe, trace, sync | `dictionary` |
| etl | 7 | status, history, freshness, run, start, stop, logs | - |
| validate | 4 | config, manifest, schema, all | - |
| pattern (V1.2) | 8 | list, get, describe, scan, promote, demote, delete, history | - |
| job (V1.2) | 10 | list, get, describe, create, update, delete, run, status, logs, schedule | - |
| model (V1.3) | 10 | list, get, describe, train, tournament, deploy, retire, delete, compare, export | `model` |
| predict (V1.3) | 5 | get, list, status, accuracy, history | - |
| action (V1.3) | 10 | list, get, describe, create, update, delete, suggest, execute, history, score | `action` |
| outcome (V1.3) | 5 | list, get, record, analyze, feedback | - |
| template (V2.0) | 6 | list, get, describe, create, apply, delete | - |
| mcp | 3 | serve, tools, status | - |
| status | 3 | system, services, data | - |

**Total: 22 command categories, 126 subcommands**

### Manifest-Enabled Commands

Commands with manifest type can be invoked declaratively via `ndp deploy apply manifest.json` or imperatively via direct CLI:

| Manifest `type` | CLI `sync` Command | Phase |
|-----------------|-------------------|-------|
| `stream` | `ndp stream sync [id]` | 7 |
| `silver-table` | `ndp silver sync <stream_id>` | 4 |
| `gold-tables` | `ndp gold sync <stream_id>` | 5 |
| `domain` | `ndp domain sync <domain_id>` | 6 |
| `dimensions` | `ndp dimension sync` | 8 |
| `dictionary` | `ndp dictionary sync` | 9 |

---

## Part 9: Migration Path for Existing MCP Tools

### Phase 1: Create Shared Library Functions

```rust
// Move existing MCP logic to ndp-lib

// Old: core/ndp-mcp-server/src/mcp/tools/list_streams.rs
// New: ndp-lib/src/bronze/list.rs

pub async fn list(storage: &impl BronzeStorage, config: &impl ConfigStore) -> Result<Vec<StreamInfo>> {
    // Same implementation, now callable from CLI or MCP
}
```

### Phase 2: Create CLI Commands

```rust
// ndp-cli/src/commands/bronze.rs
use ndp_lib::bronze;

#[derive(Subcommand)]
enum BronzeCommand {
    List,
    Describe { stream_id: String },
    Sample { stream_id: String, limit: Option<usize> },
    Validate { stream_id: String },
    Status,
}

impl BronzeCommand {
    pub async fn run(&self, ctx: &Context) -> Result<()> {
        match self {
            Self::List => {
                let streams = bronze::list(&ctx.storage, &ctx.config).await?;
                // CLI-specific output formatting
            }
            // ...
        }
    }
}
```

### Phase 3: Create MCP Tool Wrappers

```rust
// ndp-mcp/src/tools/bronze.rs
use ndp_lib::bronze;

pub async fn bronze_list(ctx: &McpContext) -> McpResult {
    let streams = bronze::list(&ctx.storage, &ctx.config).await?;
    McpResult::success(streams)
}
```

### Phase 4: Deprecate Old MCP Tools

```
Old Tool Name         → New Tool Name      → Status
─────────────────────────────────────────────────────
list_streams          → bronze_list        → Alias (deprecated)
describe_schema       → bronze_describe    → Alias (deprecated)
sample_data           → bronze_sample      → Alias (deprecated)
validate_config       → bronze_validate    → Alias (deprecated)
list_silver_tables    → silver_list        → Alias (deprecated)
describe_silver_table → silver_describe    → Alias (deprecated)
sample_silver_data    → silver_sample      → Alias (deprecated)
silver_stats          → silver_stats       → Keep (same)
etl_status            → etl_status         → Keep (same)
etl_history           → etl_history        → Keep (same)
data_freshness        → etl_freshness      → Alias (deprecated)
query_dictionary      → dictionary_query   → Alias (deprecated)
describe_column       → dictionary_describe→ Alias (deprecated)
trace_lineage         → dictionary_trace   → Alias (deprecated)
list_dq_rules         → dictionary_list    → Alias (deprecated)
```

---

## Conclusion

### Key Changes from MCP Integration

1. **Add `ndp bronze`** - Needed to map existing MCP Bronze tools
2. **Standard verbs** - `list`, `get`, `describe`, `create`, `update`, `delete`, `validate`, `status`
3. **Consistent naming** - `{category}_{verb}` for MCP tools
4. **Library-first** - All logic in `ndp-lib`, CLI and MCP are just interfaces

### Does MCP Change the CLI Design?

**Yes:**
- Added `bronze` command category
- Made `describe` and `sample` standard verbs
- Aligned category names with data layers

**No:**
- Core command structure remains valid
- Intelligence/Prediction categories still needed
- Standard CRUD verbs still apply

The MCP integration **validates** the library-first approach and **improves** the CLI design by ensuring consistency with already-proven tool patterns.
