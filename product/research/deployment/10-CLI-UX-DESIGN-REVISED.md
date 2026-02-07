# CLI UX Design: Revised Reference

> **Date**: 2026-02-06
> **Supersedes**: Docs 07, 08 (command structure sections)
> **Status**: Active design document — tracks implementation progress

---

## Design Principles

### 1. Library of Actions
`ndp-lib` is the product. CLI, MCP, and API are interfaces to it. Every action is a function in ndp-lib that takes parsed structs and returns structured output.

### 2. Entity/Verb Pattern
```
ndp <entity> <verb> [target] [flags]
```
Mirrors manifest declarations 1:1:
```
Manifest:  {"type": "stream", "action": "sync", "id": "air-quality"}
CLI:       ndp stream sync air-quality
Library:   ndp_lib::stream::sync(config, db, opts)
```

### 3. Layer as Qualifier, Not Entity
Data layers (Bronze, Silver, Gold) are accessed via `--layer` flag on stream commands, not as top-level commands. Exception: Gold WRITE operations keep their own entity because they map to distinct manifest types.

### 4. Validate as Cross-Cutting Concern
Every mutating action validates config first by default. Opt out with `--no-validate`. Run standalone with `ndp validate --all` or per-entity with `ndp stream validate`.

### 5. deploy.sh Remains Release Governor
deploy.sh handles git, Docker builds, phase ordering, device state. `ndp` provides the domain actions it calls. No `ndp deploy apply`.

### 6. Architect for Future, Build for Now
Commands marked [V1.2], [V1.3], [V2.0] define reserved namespaces. They won't be implemented until needed but establish the mental model.

---

## Implementation Status Legend

- ✅ Built and integrated in ndp-cli (via ndp-lib)
- 🔶 Exists as standalone binary, not yet consolidated into ndp-cli
- ⬜ Not yet built
- 🔮 Future version (V1.2+)

---

## Complete Command Reference

### `ndp stream` — Primary Entity

Streams are the core data entity. Layer-specific operations use `--layer bronze|silver|gold`.

| Command | Verb | Type | Status | Notes |
|---------|------|------|--------|-------|
| `ndp stream list` | list | READ | ⬜ | All streams with layer status summary |
| `ndp stream describe <id>` | describe | READ | ⬜ | Full details; `--layer` for layer-specific schema |
| `ndp stream sample <id>` | sample | READ | ⬜ | Sample rows; `--layer` (default: silver), `--limit` |
| `ndp stream stats <id>` | stats | READ | ⬜ | Statistics; `--layer` (default: silver) |
| `ndp stream sync <id>` | sync | WRITE | ⬜ | Sync config to etcd; validates first |
| `ndp stream sync --all` | sync | WRITE | ⬜ | Sync all stream configs |
| `ndp stream create <id>` | create | WRITE | ⬜ | Scaffold new stream config |
| `ndp stream validate <id>` | validate | READ | ⬜ | Validate stream config (schema + semantic) |
| `ndp stream validate --all` | validate | READ | ⬜ | Validate all stream configs |
| `ndp stream status <id>` | status | READ | ⬜ | Health across all layers, ETL freshness |

**`--layer` flag behavior:**

| Flag | `describe` | `sample` | `stats` |
|------|-----------|----------|---------|
| `--layer bronze` | Parquet schema, file count, size | Raw Parquet rows | File count, size, time range |
| `--layer silver` (default) | Hypertable schema, indexes | Silver table rows | Row count, compression, freshness |
| `--layer gold` | Continuous aggregate definitions | Aggregate rows | CA refresh status, lag |
| (omitted) | Summary across all layers | Silver (most useful default) | Silver (most useful default) |

**Library mapping:**
```
ndp stream list       → ndp_lib::stream::list(db, config)
ndp stream describe   → ndp_lib::stream::describe(id, db, config, layer)
ndp stream sample     → ndp_lib::stream::sample(id, db, layer, limit)
ndp stream stats      → ndp_lib::stream::stats(id, db, layer)
ndp stream sync       → ndp_lib::stream::sync(id, config, etcd)
ndp stream validate   → ndp_lib::validate::stream(id, config)
ndp stream status     → ndp_lib::stream::status(id, db, config)
```

**MCP tools** (when implemented):
```
stream_list           → ndp_lib::stream::list()
stream_describe       → ndp_lib::stream::describe()    # layer param
stream_sample         → ndp_lib::stream::sample()      # layer param
stream_stats          → ndp_lib::stream::stats()       # layer param
stream_status         → ndp_lib::stream::status()
```

---

### `ndp gold` — Gold Layer Write Operations

Gold keeps its own entity because it maps to the `gold-tables` manifest type and generates DDL (a fundamentally different action from stream CRUD).

| Command | Verb | Type | Status | Notes |
|---------|------|------|--------|-------|
| `ndp gold generate --stream <id>` | generate | READ | 🔶 | Generate CA DDL; validates first |
| `ndp gold generate --domain <id>` | generate | READ | 🔶 | Generate aligned view DDL |
| `ndp gold generate --stream <id> --transitions` | generate | READ | 🔶 | State transition view DDL |
| `ndp gold generate --domain <id> --events` | generate | READ | 🔶 | Events infrastructure DDL |
| `ndp gold sync --stream <id>` | sync | WRITE | 🔶 | Idempotent apply (create if not exists) |
| `ndp gold sync --domain <id>` | sync | WRITE | 🔶 | Sync domain aligned views |
| `ndp gold recreate --stream <id>` | recreate | WRITE | 🔶 | Drop and recreate CAs |

All 🔶 items currently work via `ndp-gold-ddl` standalone binary. Not yet consolidated into `ndp` CLI.

**Manifest mapping:**
```
{"type": "gold-tables", "stream_id": "X", "action": "sync"}      → ndp gold sync --stream X
{"type": "gold-tables", "stream_id": "X", "action": "recreate"}  → ndp gold recreate --stream X
{"type": "domain", "domain_id": "X", "action": "sync"}           → ndp gold generate --domain X (partial)
```

**Library mapping:**
```
ndp gold generate     → ndp_lib::gold::generate(config, opts)
ndp gold sync         → ndp_lib::gold::sync(config, db, opts)       # validates first
ndp gold recreate     → ndp_lib::gold::recreate(config, db, opts)   # validates first
```

**Flags:**

| Flag | Description | Default |
|------|-------------|---------|
| `--stream <id>` | Target stream | (required, or --domain) |
| `--domain <id>` | Target domain | (required, or --stream) |
| `--transitions` | Include state transition views | false |
| `--events` | Include events infrastructure | false |
| `--no-validate` | Skip config validation | false (validate by default) |
| `--dry-run` | Generate DDL without applying | false |

---

### `ndp domain` — Cross-Stream Entity

| Command | Verb | Type | Status | Notes |
|---------|------|------|--------|-------|
| `ndp domain list` | list | READ | ⬜ | List all domains |
| `ndp domain describe <id>` | describe | READ | ⬜ | Domain details, member streams, objectives |
| `ndp domain sync <id>` | sync | WRITE | ✅ | Sync domain config to data_dictionary |
| `ndp domain sync --all` | sync | WRITE | ⬜ | Sync all domains |
| `ndp domain validate <id>` | validate | READ | 🔶 | Validate domain config (via ndp-validate) |
| `ndp domain validate --all` | validate | READ | 🔶 | Validate all domain configs |
| `ndp domain create <id>` | create | WRITE | ⬜ | Scaffold new domain config |

**Manifest mapping:**
```
{"type": "domain", "domain_id": "X", "action": "sync"}  → ndp domain sync X
```

---

### `ndp dictionary` — Data Dictionary Sync

| Command | Verb | Type | Status | Notes |
|---------|------|------|--------|-------|
| `ndp dictionary sync` | sync | WRITE | ✅ | Sync stream configs to data_dictionary tables |
| `ndp dictionary query <term>` | query | READ | ⬜ | Search dictionary |
| `ndp dictionary describe <column>` | describe | READ | ⬜ | Column details |
| `ndp dictionary trace <column>` | trace | READ | ⬜ | Column lineage |
| `ndp dictionary list` | list | READ | ⬜ | List dictionary entries / DQ rules |

**Manifest mapping:**
```
{"type": "dictionary", "action": "sync"}  → ndp dictionary sync
```

---

### `ndp dimension` — Dimension Table Sync

| Command | Verb | Type | Status | Notes |
|---------|------|------|--------|-------|
| `ndp dimension sync <id>` | sync | WRITE | ✅ | Sync CSV to Silver table |
| `ndp dimension list` | list | READ | ⬜ | List dimension tables |
| `ndp dimension describe <id>` | describe | READ | ⬜ | Dimension schema and row count |
| `ndp dimension validate <id>` | validate | READ | ⬜ | Validate dimension CSV against schema |

**Manifest mapping:**
```
{"type": "dimensions", "action": "sync"}  → ndp dimension sync
```

---

### `ndp validate` — Platform-Wide Validation

Convenience entry point. Calls per-entity validators. Also usable per-entity via `ndp <entity> validate`.

| Command | Verb | Type | Status | Notes |
|---------|------|------|--------|-------|
| `ndp validate --all` | (all) | READ | 🔶 | Full platform validation (schema + semantic) |
| `ndp validate --stream <id>` | stream | READ | 🔶 | Single stream validation |
| `ndp validate --domain <id>` | domain | READ | 🔶 | Single domain validation |
| `ndp validate --schema --generate` | schema | READ | 🔶 | Generate JSON Schema from ndp-types |
| `ndp validate --schema --verify <path>` | schema | READ | 🔶 | Verify committed schema matches generated |

All 🔶 items currently work via `ndp-validate` standalone binary. Not yet consolidated.

**Cross-cutting validation behavior:**

| Mutating command | Validates | Opt-out |
|-----------------|-----------|---------|
| `ndp gold sync` | Gold config + stream config | `--no-validate` |
| `ndp gold generate` | Gold config + stream config | `--no-validate` |
| `ndp stream sync` | Stream config | `--no-validate` |
| `ndp domain sync` | Domain config | `--no-validate` |

**Library mapping:**
```
ndp validate --all            → ndp_lib::validate::all(config_dir)
ndp validate --stream X       → ndp_lib::validate::stream(id, config_dir)
ndp validate --domain X       → ndp_lib::validate::domain(id, config_dir)
ndp validate --schema         → ndp_lib::validate::schema(opts)
ndp gold sync (internal)      → ndp_lib::validate::gold_config(config)  # called before sync
```

---

### `ndp config` — Low-Level etcd Operations

| Command | Verb | Type | Status | Notes |
|---------|------|------|--------|-------|
| `ndp config get <key>` | get | READ | ⬜ | Get value from etcd |
| `ndp config set <key> <value>` | set | WRITE | ⬜ | Set value in etcd |
| `ndp config list` | list | READ | ⬜ | List all config keys |

---

### `ndp status` — System Health

| Command | Verb | Type | Status | Notes |
|---------|------|------|--------|-------|
| `ndp status` | (default) | READ | ⬜ | Overall system health |
| `ndp status services` | services | READ | ⬜ | Service health (containers, etcd, TimescaleDB) |
| `ndp status data` | data | READ | ⬜ | Data freshness across streams |

---

### `ndp mcp` — Machine Interface [Future]

| Command | Verb | Type | Status | Notes |
|---------|------|------|--------|-------|
| `ndp mcp serve` | serve | EXEC | ⬜ | Start MCP server (stdio or HTTP) |
| `ndp mcp tools` | tools | READ | ⬜ | List available MCP tools |

Security requirements not yet determined. Deferred until needed. Could alternatively be an API server (`ndp api serve`). Same ndp-lib functions either way.

---

### V1.2: Pattern Detection [Future]

| Command | Verb | Type | Status | Notes |
|---------|------|------|--------|-------|
| `ndp pattern list` | list | READ | 🔮 | List correlation candidates/patterns |
| `ndp pattern describe <id>` | describe | READ | 🔮 | Correlation details |
| `ndp pattern scan` | scan | EXEC | 🔮 | Trigger Granger causality scan |
| `ndp pattern promote <id>` | promote | WRITE | 🔮 | Candidate → validated pattern |
| `ndp pattern history <id>` | history | READ | 🔮 | Correlation strength over time |
| `ndp job list` | list | READ | 🔮 | List scheduled analytics jobs |
| `ndp job create <type>` | create | WRITE | 🔮 | Create analytics job |
| `ndp job run <id>` | run | EXEC | 🔮 | Trigger job manually |
| `ndp job status <id>` | status | READ | 🔮 | Job status |

---

### V1.3: Prediction & Actions [Future]

| Command | Verb | Type | Status | Notes |
|---------|------|------|--------|-------|
| `ndp model list` | list | READ | 🔮 | List models |
| `ndp model train <type>` | train | EXEC | 🔮 | Train model on pattern |
| `ndp model tournament` | tournament | EXEC | 🔮 | Run model tournament |
| `ndp model deploy <id>` | deploy | WRITE | 🔮 | Deploy for predictions |
| `ndp action list` | list | READ | 🔮 | List defined actions |
| `ndp action suggest` | suggest | READ | 🔮 | Get suggestions for objectives |
| `ndp action execute <id>` | execute | EXEC | 🔮 | Execute action |

---

## Global Flags

| Flag | Env Var | Default | Description |
|------|---------|---------|-------------|
| `--db-url <url>` | `TIMESCALE_URL` | (required for DB ops) | TimescaleDB connection |
| `--config-dir <path>` | `NDP_CONFIG_DIR` | auto-detected | Config directory root |
| `--env <name>` | `DEPLOY_ENV` | `pi` | Environment: `integration` or `pi` |
| `--dry-run` | — | false | Show what would happen without doing it |
| `--no-validate` | — | false | Skip pre-mutation validation |
| `--format <fmt>` | — | `human` | Output format: `human` or `json` |
| `--verbose` | — | false | Detailed output |

---

## Standard Verb Definitions

| Verb | Type | Description |
|------|------|-------------|
| `list` | READ | List multiple resources (table format) |
| `describe` | READ | Detailed view of single resource |
| `sample` | READ | Sample rows from data layer |
| `stats` | READ | Statistics and metrics |
| `status` | READ | Operational health |
| `validate` | READ | Check config validity without mutation |
| `query` | READ | Search with filters |
| `trace` | READ | Follow relationships/lineage |
| `history` | READ | Historical records |
| `sync` | WRITE | Synchronize state (idempotent) |
| `create` | WRITE | Create new resource |
| `recreate` | WRITE | Drop and recreate |
| `generate` | READ | Produce output (DDL) without applying |
| `run` | EXEC | Execute once |

---

## MCP Tool Naming Convention

When MCP/API is implemented, tools follow `{entity}_{verb}` with parameters:

```
stream_list()                                   # no params
stream_describe(id, layer?)                     # optional layer
stream_sample(id, layer?, limit?)               # optional layer, limit
stream_stats(id, layer?)                        # optional layer
gold_generate(stream_id?, domain_id?, opts?)    # one of stream/domain required
dictionary_query(term)                          # search term
validate_all()                                  # full platform validation
```

Estimated tool count by version:

| Version | Tools | Notes |
|---------|-------|-------|
| V1.1 | ~15 | stream (5), gold (2), domain (2), dictionary (3), validate (1), status (2) |
| V1.2 | ~22 | + pattern (4), job (3) |
| V1.3 | ~30 | + model (4), action (4) |

---

## Consolidation Progress

### Summary

| Category | Total commands | ✅ Built | 🔶 Standalone | ⬜ Not built | 🔮 Future |
|----------|---------------|---------|--------------|-------------|----------|
| stream | 10 | 0 | 0 | 10 | 0 |
| gold | 7 | 0 | 7 | 0 | 0 |
| domain | 7 | 1 | 2 | 4 | 0 |
| dictionary | 5 | 1 | 0 | 4 | 0 |
| dimension | 4 | 1 | 0 | 3 | 0 |
| validate | 5 | 0 | 5 | 0 | 0 |
| config | 3 | 0 | 0 | 3 | 0 |
| status | 3 | 0 | 0 | 3 | 0 |
| mcp | 2 | 0 | 0 | 2 | 0 |
| pattern | 5 | 0 | 0 | 0 | 5 |
| job | 4 | 0 | 0 | 0 | 4 |
| model | 4 | 0 | 0 | 0 | 4 |
| action | 3 | 0 | 0 | 0 | 3 |
| **Total** | **62** | **3** | **14** | **29** | **16** |

### What ✅ Means (built and integrated)

These commands exist in `tools/ndp-cli`, call `ndp-lib` functions, are integrated in `deploy.sh` via `command -v ndp`, and have London TDD tests in ndp-lib.

1. `ndp dictionary sync` — ops-001 (v1.1.9)
2. `ndp dimension sync <id>` — ops-001 (v1.1.9)
3. `ndp domain sync` — ops-002/BUG-002 (v1.1.12)

### What 🔶 Means (exists but not consolidated)

These capabilities exist in standalone binaries. The logic works and is tested. ops-003 consolidates them into `ndp` CLI by migrating the logic to ndp-lib and adding command facades.

**From ndp-gold-ddl (376 tests):**
1. `ndp gold generate --stream` — currently `ndp-gold-ddl generate --stream`
2. `ndp gold generate --domain` — currently `ndp-gold-ddl generate --domain`
3. `ndp gold generate --transitions` — currently `ndp-gold-ddl generate --transitions`
4. `ndp gold generate --events` — currently `ndp-gold-ddl generate --events`
5. `ndp gold sync --stream` — currently `ndp-gold-ddl generate --action sync --database-url`
6. `ndp gold sync --domain` — currently `ndp-gold-ddl generate --domain --action sync --database-url`
7. `ndp gold recreate --stream` — currently `ndp-gold-ddl generate --action recreate --database-url`

**From ndp-validate (217 tests):**
8. `ndp validate --all` — currently `ndp-validate --all`
9. `ndp validate --stream <id>` — currently `ndp-validate <config-path>`
10. `ndp validate --domain <id>` — currently `ndp-validate --domain <path>`
11. `ndp validate --domain --all` — currently `ndp-validate --domain-all`
12. `ndp validate --schema --generate` — currently `ndp-validate --generate-schema`
13. `ndp validate --schema --verify` — currently `ndp-validate --verify-schema`
14. `ndp domain validate` — currently `ndp-validate --domain <path>`

### What ⬜ Means (not yet built)

No implementation exists. Will be built when needed for a feature or operational requirement. Key near-term candidates:

- `ndp stream list` / `describe` / `status` — High value for troubleshooting
- `ndp stream validate` — Alias into validate infrastructure
- `ndp dictionary query` / `describe` / `trace` — Already implemented in ndp-mcp-server, needs library extraction
- `ndp config get` / `set` / `list` — Needs config-client integration

---

## deploy.sh Integration

### Current State (v1.1.13)

```bash
# deploy.sh dispatches to 3 binaries across 7 call sites:
command -v ndp          # lines ~386, ~894, ~1063  (3 sites)
command -v ndp-validate # lines ~1535, ~2035       (2 sites)
command -v ndp-gold-ddl # lines ~1938, ~2071       (2 sites)
```

### Target State (post ops-003)

```bash
# deploy.sh dispatches to 1 binary:
command -v ndp          # all 7 sites consolidated

# Phase 1 (Validation):
ndp validate --all --config-dir "$CONFIG_DIR"

# Phase 5 (Gold Tables):
ndp gold sync --stream "$STREAM_ID" --db-url "$DB_URL" --config-dir "$CONFIG_DIR"
ndp gold sync --domain "$DOMAIN_ID" --db-url "$DB_URL" --config-dir "$CONFIG_DIR"

# Phase 7 (Stream sync — future):
# ndp stream sync "$STREAM_ID"    # when etcd sync moves to Rust

# Phase 8 (Dimensions):
ndp dimension sync "$DIM_ID" --config "$CONFIG" --source "$CSV" --db-url "$DB_URL"

# Phase 9 (Dictionary):
ndp dictionary sync --config-dir "$CONFIG_DIR" --db-url "$DB_URL"

# Phase 9 (Domain sync):
ndp domain sync --domains-dir "$DOMAINS_DIR" --db-url "$DB_URL"
```

deploy.sh remains the release governor: git pull, Docker builds, phase ordering, container restarts, device state tracking. `ndp` provides the domain actions.

---

## ndp-lib Module Map

### Current (v1.1.13)

```
crates/ndp-lib/src/
├── lib.rs            # Public API: DbClient, SyncReport, SyncOptions
├── db.rs             # DbClient trait, PostgresClient
├── config.rs         # ConfigLoader trait, FileSystemConfigLoader
├── convert.rs        # Config → sync-type bridges
├── error.rs          # NdpLibError
├── types.rs          # SyncReport, SyncOptions, SyncError
├── dictionary/       # ✅ sync_dictionary()
├── dimension/        # ✅ sync_dimension()
└── domain/           # ✅ sync_domains()
```

### Target (post ops-003)

```
crates/ndp-lib/src/
├── lib.rs
├── db.rs             # DbClient trait, PostgresClient, NoOpDbClient
├── config.rs         # ConfigLoader trait, FileSystemConfigLoader
├── constants.rs      # VALID_METRICS, VALID_ROLLING_STATS, schema names
├── convert.rs
├── error.rs
├── types.rs
├── dictionary/       # ✅ (exists)
├── dimension/        # ✅ (exists)
├── domain/           # ✅ (exists)
├── gold/             # 🔶→✅ migrated from ndp-gold-ddl
│   ├── mod.rs
│   ├── generators/   #   continuous_aggregate, aligned_view, state_transitions, events
│   ├── planner/      #   sync planner (create/skip/recreate decisions)
│   ├── registry/     #   feature generators (lag, rolling, trend)
│   └── validation/   #   gold-specific config validation
└── validate/         # 🔶→✅ migrated from ndp-validate
    ├── mod.rs        #   validate::all(), validate::stream(), validate::domain()
    ├── schema.rs     #   JSON Schema validation (Layer 1)
    └── semantic/     #   Cross-field validation (Layer 2)
```

### Future (V1.2+)

```
crates/ndp-lib/src/
├── [all above]
├── stream/           # ⬜ stream CRUD, layer-aware describe/sample/stats
├── config/           # ⬜ etcd get/set (wraps config-client)
├── pattern/          # 🔮 V1.2 correlation scanning
├── job/              # 🔮 V1.2 scheduled analytics
├── model/            # 🔮 V1.3 ML model lifecycle
└── action/           # 🔮 V1.3 action framework
```

---

## Design Decisions Log

| Decision | Rationale | Date |
|----------|-----------|------|
| `--layer` flag instead of bronze/silver/gold commands | Fewer top-level entities, streams are the mental model, layers are implementation | 2026-02-06 |
| Gold keeps its own entity for writes | Maps to `gold-tables` manifest type, DDL generation is distinct from stream CRUD | 2026-02-06 |
| Validate is cross-cutting + standalone | Core competency in config-driven platform; called before mutations by default | 2026-02-06 |
| No `ndp deploy apply` | deploy.sh stays as release governor; ndp provides actions, deploy.sh orchestrates | 2026-02-06 |
| MCP/API deferred | Security requirements not determined; same ndp-lib functions regardless of transport | 2026-02-06 |
| `describe` not `get` for detail view | One verb for detailed single-entity view; `list` for multi-entity | 2026-02-06 |
| Library extraction over facade | Actions must be callable from CLI, MCP, API, and internally (e.g., gold calls validate) | 2026-02-06 |
| ndp-lib functions take parsed structs | Source-agnostic: works with files today, etcd later | 2026-02-05 (ops-001) |
| DbClient trait for all DB access | Mockable for London TDD; single implementation shared across all operations | 2026-02-05 (ops-001) |
