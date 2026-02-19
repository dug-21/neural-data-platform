# NDP Pattern Seed — Procedures, Testing, Vision, and Cross-Cutting Conventions

> **Generated**: 2026-02-19
> **Researcher**: procedures-researcher agent
> **Sources**: docs/procedures/RELEASE-POLICY.md, docs/procedures/DEPLOYMENT-DECLARATIVES.md,
>   tests/integration/README.md, product/features/gold-001/FEATURE-ROADMAPv1.3.md,
>   CLAUDE.md, .claude/rules/swarm-protocol.md, .claude/rules/testing.md,
>   .claude/rules/agent-routing.md, docs/architecture/CONSOLIDATED_ARCHITECTURE_DECISIONS.md,
>   docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md

---

## Pattern Index

1. Release artifacts — three required artifacts per release
2. Semantic versioning rules — when to bump MAJOR/MINOR/PATCH
3. Release checklist — pre/create/deploy/post steps
4. Declarative deployment — manifest-based deploy.sh apply
5. Deployment execution order — 12-phase orchestration
6. Declaration types — all 9 manifest declaration types
7. Rollback procedure — manifest-based rollback without data loss risk
8. Integration testbed framework — 4 testbed types, composable pipeline
9. Assertion library — 8 standard assertions for integration validation
10. Feature testbed process — when needed, structure, and lessons from fe-004
11. Gold-001 v1.3 tiered intelligence architecture — Online NN / SONA / LLM
12. Feature engineering separation — ndp-features crate distinct from intelligence
13. EWMA online normalization — normalization for continuous learning
14. Swarm protocol — 2-message swarm launch with coordinator delegation
15. Agent routing — NDP-specific agents over generic ones, 17-agent roster
16. SPARC planning swarm — wave-ordered planning with validation gate
17. Pattern workflow (AgentDB vs claude-flow memory) — mandatory before/after every task
18. Deprecated approaches — DuckDB, Polars streaming
19. Pi resource constraints — memory budgets per container
20. Anti-drift mechanism — Level-1 summary in every agent prompt
21. Memory key convention — swarm coordination naming standard
22. Testing conventions — London TDD, integration env mandatory phases
23. Feature tracking via GitHub Issues — no STATUS.md files

---

### Pattern: ndp-release-artifacts

- **taskType**: procedure:release
- **approach**: Every NDP release MUST produce exactly 3 artifacts:
  1. **Manifest**: `.deploy/releases/vX.Y.Z.manifest.json` — contains `$schema`, `version: "1.0"`, `release_version`, `description`, `changes[]`
  2. **Git tag**: annotated (`git tag -a vX.Y.Z -m "..."`) — must match manifest `release_version`
  3. **Changelog**: entry in `CHANGELOG.md` following Keep-a-Changelog format (Added/Changed/Fixed/Removed)
  Missing any of these three = release is incomplete. Tag message should match manifest description. `release_version` in JSON uses no `v` prefix (e.g., `"1.2.0"` not `"v1.2.0"`).
- **successRate**: 1.0
- **tags**: release, manifest, git-tag, changelog, procedure
- **source**: docs/procedures/RELEASE-POLICY.md
- **status**: current

---

### Pattern: ndp-semver-rules

- **taskType**: procedure:versioning
- **approach**: NDP follows SemVer 2.0.0.
  - **MAJOR** (breaking): schema breaking change (e.g., remove `entity_schemas`), API contract change, config format change requiring migration, DB schema break, protocol change. MAJOR releases require migration script/procedure, updated docs, changelog BREAKING section.
  - **MINOR** (backwards-compatible feature): new stream, new Silver table, new API endpoint, new MCP tool, new optional config field.
  - **PATCH** (backwards-compatible fix): bug fix, config correction, security patch, perf fix, doc fix, DQ rule adjustment.
  Examples: 1.2.3 → 1.3.0 (new stream), 1.3.0 → 1.3.1 (bug fix), 1.3.1 → 2.0.0 (schema migration).
- **successRate**: 1.0
- **tags**: semver, versioning, release, major, minor, patch
- **source**: docs/procedures/RELEASE-POLICY.md
- **status**: current

---

### Pattern: ndp-release-checklist

- **taskType**: procedure:release-checklist
- **approach**: Mandatory checklist every release:
  **Pre-Release**: all changes tested in integration env; stream configs validated (`./tools/ndp-validate/ndp-validate.sh --all`); DDL tested (`./deploy.sh apply --dry-run <manifest>`); no uncommitted changes; on correct branch.
  **Create Release**: determine version bump; create manifest; verify with `jq .`; update CHANGELOG.md; export trust snapshot (`/trust-dashboard` → `.deploy/trust/vX.Y.Z.json`); update test baseline (`.ndp/test-baseline.txt`); commit (`git commit -m "release: vX.Y.Z"`); create annotated tag; push code and tag.
  **Deploy**: `git pull` on device; verify tag; `./deploy.sh apply .deploy/releases/vX.Y.Z.manifest.json`; verify `/var/ndp/deployed-version`; smoke test.
  **Post-Release**: monitor logs; verify Grafana; document issues.
- **successRate**: 1.0
- **tags**: release, checklist, deploy, procedure, pre-release
- **source**: docs/procedures/RELEASE-POLICY.md
- **status**: current

---

### Pattern: ndp-declarative-deployment

- **taskType**: procedure:deployment
- **approach**: NDP uses declarative deployment: declare WHAT changed in a manifest JSON, run `./deploy.sh apply <manifest>` to orchestrate. Manifest format:
  ```json
  {
    "$schema": "../schemas/manifest.schema.json",
    "version": "1.0",
    "description": "Human-readable description",
    "changes": [
      {"type": "stream", "id": "air-quality", "action": "create"},
      {"type": "silver-table", "stream_id": "air-quality", "action": "sync"},
      {"type": "dictionary", "action": "sync"}
    ]
  }
  ```
  File location: `.deploy/releases/vX.Y.Z.manifest.json`. The `deploy.sh apply` command resolves all declaration dependencies automatically. Never run manual deployment commands directly — always use a manifest.
- **successRate**: 1.0
- **tags**: deployment, manifest, declarative, deploy-sh, procedure
- **source**: docs/procedures/DEPLOYMENT-DECLARATIVES.md
- **status**: current

---

### Pattern: ndp-deploy-execution-order

- **taskType**: procedure:deployment-phases
- **approach**: `deploy.sh apply` executes declarations in a fixed 12-phase order:
  - Phase 1: Validation (manifest + infrastructure check)
  - Phase 2: Container Builds (`container` build actions — early, so new code is available)
  - Phase 2.5: Tool Builds (`tool` — Rust CLI tools like ndp-gold-ddl, ndp-validate)
  - Phase 3: Migrations (`migration` — SQL files in declaration order)
  - Phase 4: Silver Tables (`silver-table` — generate and apply DDL)
  - Phase 5: Gold Tables (`gold-tables` — continuous aggregates, requires ndp-gold-ddl from Phase 2.5)
  - Phase 6: Domains (`domain` — cross-stream aligned views)
  - Phase 7: Streams (`stream` — sync to etcd)
  - Phase 8: Dimensions (`dimensions` — sync CSVs)
  - Phase 9: Dictionary (`dictionary` — sync from stream configs)
  - Phase 10: Container Restarts (`container` restart actions — last, picks up all config changes)
  - Phase 11: Device State (update `/var/ndp/deployed-version`, `/var/ndp/deployed-manifest`, `/var/ndp/deployed-timestamp`)
  Rationale: builds before migrations, tool builds before gold tables, streams before dictionary, restarts last.
- **successRate**: 1.0
- **tags**: deployment, execution-order, phases, manifest, deploy-sh
- **source**: docs/procedures/DEPLOYMENT-DECLARATIVES.md
- **status**: current

---

### Pattern: ndp-manifest-declaration-types

- **taskType**: procedure:manifest-declarations
- **approach**: 9 declaration types available in manifests:
  - `stream`: sync stream config to etcd. Fields: `id`, `action` (create/update/validate-only), `reload` (sources/full/none). Config at `config/base/streams/{id}/config.json`.
  - `silver-table`: generate + apply Silver DDL. Fields: `stream_id`, `action` (sync/validate-only). Requires `silver_etl.enabled: true` in stream config.
  - `tool`: build Rust CLI. Fields: `id` (ndp-gold-ddl/ndp-validate), `action: "build"`, `profile` (release/debug).
  - `migration`: run SQL file. Fields: `file` (path relative to repo root). Common locations: `migrations/`, `deploy/pi/init-scripts/`.
  - `gold-tables`: generate Gold layer continuous aggregates. Fields: `stream_id`, `action` (sync/recreate). Requires `ndp-gold-ddl` tool built first.
  - `domain`: generate cross-stream aligned view. Fields: `domain_id`, `action: "sync"`. Config at `config/domains/{domain_id}/config.yaml`.
  - `dimensions`: sync CSVs to TimescaleDB. Fields: `action: "sync"`. Files from `config/base/dimensions/`.
  - `dictionary`: sync data dictionary from stream configs. Fields: `action: "sync"`. Syncs to `data_dictionary.*` tables.
  - `container`: build or restart Docker container. Fields: `target` (air-quality-app/ndp-mcp-server/silver-etl/grafana/ndp-intelligence), `action` (build/restart), `no_cache` (boolean).
- **successRate**: 1.0
- **tags**: manifest, declarations, deployment, stream, silver, gold, container, migration
- **source**: docs/procedures/DEPLOYMENT-DECLARATIVES.md
- **status**: current

---

### Pattern: ndp-rollback-procedure

- **taskType**: procedure:rollback
- **approach**: To roll back to a previous version:
  **Quick rollback**: On target device, run `./deploy.sh apply .deploy/releases/v{previous}.manifest.json`. Verify with `cat /var/ndp/deployed-version`.
  **Full rollback with Git**: `git checkout v{previous}` → `./deploy.sh apply .deploy/releases/v{previous}.manifest.json` → `./deploy.sh status`.
  **Important**: Database migrations are NOT automatically reversed. Schema changes may require manual reverse migration. All prior release manifests are kept in `.deploy/releases/` — never delete them. Device state tracked in `/var/ndp/` directory (deployed-version, deployed-manifest, deployed-timestamp).
- **successRate**: 0.9
- **tags**: rollback, deployment, procedure, recovery
- **source**: docs/procedures/RELEASE-POLICY.md
- **status**: current

---

### Pattern: ndp-integration-testbed-framework

- **taskType**: testing:integration-testbed
- **approach**: Integration testbed framework at `tests/integration/` validates full NDP pipeline end-to-end (MQTT → Bronze WAL → Silver ETL → Gold CAs → Domain config → Intelligence). Four testbed types:
  - **smoke** (`< 2 min`): 10 MQTT messages, validate Silver rows, etcd sync. Command: `./tests/integration/run-testbed.sh smoke`
  - **regression** (`~10 min`): all streams, all layers, intelligence. Command: `./run-testbed.sh regression --intelligence`
  - **stress** (`30 min`): sustained load (18000 messages @ 10/s), RSS monitoring, 256MB threshold. Command: `./run-testbed.sh stress --timeout 1800 --count 18000 --rate 10`
  - **feature** (variable): feature-specific assertions. Command: `./run-testbed.sh feature --path product/features/{id}/testbed`
  Each run follows 4 phases: (1) Prep — docker compose down -v + up -d; (2) Config — `DEPLOY_ENV=integration deploy.sh apply <manifest>`; (3) Inject — mosquitto_pub via docker exec; (4) Validate — assertions from validate.sh.
  Integration env config: `docker-compose.integration.yml`. Container names use `integration-` prefix.
- **successRate**: 1.0
- **tags**: testing, integration, testbed, smoke, regression, stress, feature, pipeline
- **source**: tests/integration/README.md, .claude/rules/testing.md
- **status**: current

---

### Pattern: ndp-assertion-library

- **taskType**: testing:assertions
- **approach**: Standard assertion helpers defined in `tests/integration/lib/assert.sh`. Source it in validate.sh with: `source "${SCRIPT_DIR}/../../../../tests/integration/lib/assert.sh"`. Available functions:
  - `assert_service_healthy <container>` — Docker health status = "healthy"
  - `assert_etcd_key <key>` — etcd key exists with non-empty value
  - `assert_silver_rows <table> <min>` — Silver table has >= N rows
  - `assert_bronze_wal_exists <stream>` — WAL directory exists
  - `assert_embedding_exists <domain>` — Intelligence embeddings table has rows
  - `assert_container_rss_below <container> <mb>` — Container RSS < threshold
  - `assert_gold_object_exists <name>` — Gold table/materialized view exists
  - `assert_summary` — Prints totals, returns exit 0 (all pass) or 1 (any fail) — ALWAYS call last
  For custom SQL checks: `psql -c "SELECT count(*) FROM table WHERE column IS NOT NULL"`. Always check both existence (rows exist) AND correctness (values are valid, not silently zero/NULL).
- **successRate**: 1.0
- **tags**: testing, assertions, integration, library, validate
- **source**: tests/integration/README.md
- **status**: current

---

### Pattern: ndp-feature-testbed-process

- **taskType**: testing:feature-testbed
- **approach**: Features touching integration boundaries MUST include a feature testbed at `product/features/{id}/testbed/`. A testbed is needed when the feature touches: SQL queries, new containers, Bronze/Silver/Gold data flow, or configuration (etcd/domain). NOT needed for library-only changes or documentation.
  **Structure**: `manifest.json` (same format as production releases), `compose-override.yml` (adjust thresholds — production defaults like 168 warmup observations are too slow for testbeds), `data/` (fixtures, optional seed.sql), `validate.sh` (source assert.sh, check prerequisites before feature outputs, call assert_summary last).
  **Key lesson from fe-004**: 5 production bugs (v1.2.7–v1.2.11) all involved real PostgreSQL types, real view schemas, or real container lifecycle — none reproducible in unit tests. A feature testbed would have caught all 5 before first deploy. Use `--skip-clean` for fast iteration after fixing: rebuild container, then `./run-testbed.sh feature --path ... --skip-clean`.
  **Checklist before merging**: testbed dir exists, manifest declares deployment, compose-override adjusts timing, validate.sh checks every integration point, clean-slate run passes.
- **successRate**: 1.0
- **tags**: testing, feature-testbed, integration, testbed, fe-004, lessons-learned
- **source**: tests/integration/README.md
- **status**: current

---

### Pattern: ndp-v13-tiered-intelligence-architecture

- **taskType**: vision:v1.3-architecture
- **approach**: V1.3 replaces K-NN retrieval with a 3-tier online learning intelligence architecture:
  **Tier 1 — Online NN** (continuous, ~microseconds): Per-domain lightweight MLP. Takes normalized feature vector, predicts next value every Gold refresh cycle (15 min). Updates weights via online gradient descent every cycle. Cost: cheap, always running.
  **Tier 2 — SONA Attention** (continuous, ~milliseconds): Meta-learner watching NN prediction errors. ReasoningBank stores known error patterns as embeddings. When SONA recognizes a pattern → applies micro-LoRA (fast adaptation without forgetting via EWC++). When situation is novel → escalates to Tier 3.
  **Tier 3 — External LLM via MCP** (on-demand, ~seconds/$): Reasoning engine for novel situations. LLM receives structured EscalationContext, investigates via MCP tools (query_external_api, query_gold_history, check_system_health), returns adjustment (micro-LoRA vector, config update, checkpoint rollback, false_alarm, observation_note).
  K-NN demoted to bootstrap mechanism (primary during warmup) + validation baseline (compare NN vs K-NN predictions as signal for SONA).
  Key insight: prediction error IS the anomaly signal — the NN's mistakes are what SONA analyzes.
- **successRate**: 1.0
- **tags**: vision, v1.3, gold-001, intelligence, online-learning, sona, mcp-escalation, tiered-architecture
- **source**: product/features/gold-001/FEATURE-ROADMAPv1.3.md
- **status**: current

---

### Pattern: ndp-feature-engineering-separation

- **taskType**: vision:architecture-boundary
- **approach**: V1.3 establishes a critical architectural boundary — three separate concerns with two library crates and one binary:
  1. **ndp-lib::gold** (`crates/ndp-lib/src/gold/`) — deploy-time DDL generation only. Creates tables, CAs, refresh jobs. Runs at deploy time, not runtime.
  2. **ndp-features** (`crates/ndp-features/`) — runtime feature engineering. EWMA normalization, encoding (numeric/binary/text), feature vector assembly, embedding storage. Runs every Gold CA refresh cycle.
  3. **ndp-intelligence** (`crates/ndp-intelligence/`) — learning engine. Online MLP, EWC, prediction loop, SONA integration, MCP escalation bridge.
  4. **apps/ndp-intelligence-app** — single binary orchestrating both crates via PG NOTIFY listener.
  Anti-pattern from fe-004: z-score normalization, embedding construction, pgvector storage, K-NN search all in one binary — these are feature engineering steps, not intelligence steps.
  Rule: deploy-time DDL generation ≠ runtime feature engineering ≠ intelligence/learning.
- **successRate**: 1.0
- **tags**: vision, architecture, feature-engineering, ndp-features, ndp-intelligence, crate-boundaries, v1.3
- **source**: product/features/gold-001/FEATURE-ROADMAPv1.3.md
- **status**: current

---

### Pattern: ndp-ewma-online-normalization

- **taskType**: vision:online-learning
- **approach**: Online learning requires normalization that adapts without seeing all data upfront. NDP uses EWMA (Exponentially Weighted Moving Average) z-score:
  - `u(t) = a * x(t) + (1-a) * u(t-1)` — running mean
  - `s2(t) = a * (x(t) - u(t))^2 + (1-a) * s2(t-1)` — running variance
  - `z(t) = (x(t) - u(t)) / sqrt(s2(t))` — normalized value
  Alpha (decay factor): 0.01 (slow/stable features), 0.05 (default), 0.1 (fast/volatile features).
  **Normalization state MUST persist in Gold table** (`gold.normalization_state`): domain_id, feature_name, running_mean, running_var, observation_count, last_updated, alpha. Without persistence, system re-normalizes from scratch after every restart → garbage predictions until warmup completes.
  This supersedes batch z-score normalization (which requires all data upfront).
- **successRate**: 1.0
- **tags**: vision, online-learning, ewma, normalization, feature-engineering, persistence, v1.3
- **source**: product/features/gold-001/FEATURE-ROADMAPv1.3.md
- **status**: current

---

### Pattern: ndp-swarm-protocol

- **taskType**: convention:swarm-coordination
- **approach**: Swarms use coordinator delegation — primary agent spawns `ndp-scrum-master` as single coordinator, who spawns workers. Use Task tool (spawn-and-wait), NOT TeamCreate.
  **2-message swarm launch**:
  Message 1 (batch all initialization): `agent_spawn(agentId)` for each agent, `memory_store` shared context at `swarm/shared/{feature}-context` (namespace: "coordination", upsert: true), `TaskCreate` all tasks.
  Message 2 (all agents in parallel): Task tool for each agent. Agent prompt MUST include `Your agent ID: {feature}-agent-N-{role}` (this activates Swarm Coordination block in agent definitions) + Level-1 summary + task description + file paths.
  **After spawning**: tell user what agents are working on → STOP → wait.
  **Memory rules**: all `memory_store` calls MUST use `upsert: true`; all values MUST include `"feature":"<feature-id>"` in JSON; use `memory_list` + `memory_retrieve` (exact-key, reliable) NOT `memory_search` (semantic, 20-80% recall).
  **Swarm when**: 3+ files, new feature, cross-module refactor, API changes, schema changes. **Skip swarm for**: single file edits, 1-2 line fixes, config changes, docs.
- **successRate**: 0.9
- **tags**: swarm, coordination, protocol, agent-spawn, task-tool, memory
- **source**: .claude/rules/swarm-protocol.md
- **status**: current

---

### Pattern: ndp-agent-routing

- **taskType**: convention:agent-selection
- **approach**: Always use NDP-specific agents over generic ones:
  - `ndp-rust-dev` instead of `coder`
  - `ndp-architect` instead of `system-architect`
  - `ndp-tester` instead of `tester`
  - `ndp-scrum-master` instead of `planner`
  - `ndp-validator` instead of `reviewer`
  Full 17-agent roster: 2 coordination (ndp-scrum-master, ndp-validator — mandatory on every swarm), 4 planning (ndp-architect, specification, ndp-pseudocode, ndp-tester), 4 data layer (ndp-parquet-dev, ndp-timescale-dev, ndp-analytics-engineer, ndp-dq-engineer), 2 domain scientists (ndp-meteorologist, ndp-air-quality-specialist), 2 ML (ndp-feature-engineer, ndp-ml-engineer), 2 viz/alerts (ndp-grafana-dev, ndp-alert-engineer), 1 alignment (ndp-vision-guardian — planning swarms only).
  ndp-scrum-master and ndp-validator are non-negotiable on every swarm. Max wave size: 5 workers.
- **successRate**: 1.0
- **tags**: agents, routing, ndp-specific, swarm, coordination
- **source**: .claude/rules/agent-routing.md, CLAUDE.md
- **status**: current

---

### Pattern: ndp-sparc-planning-swarm

- **taskType**: convention:planning
- **approach**: Planning swarms use wave-ordered execution:
  **Wave 1** (parallel): `ndp-architect` → ARCHITECTURE.md with ADRs + Integration Surface table; `specification` → SPECIFICATION.md, TASK-DECOMPOSITION.md.
  **Wave 2** (parallel, after Wave 1): `ndp-pseudocode` → pseudocode/OVERVIEW.md + per-component pseudocode; `ndp-tester` → test-plan/OVERVIEW.md + per-component test plans.
  **Wave 3** (sequential): `ndp-vision-guardian` → ALIGNMENT-REPORT.md; `ndp-validator` → 5-check plan validation.
  Final outputs: IMPLEMENTATION-BRIEF.md (with Component Map), ACCEPTANCE-MAP.md, LAUNCH-PROMPT.md, GH Issue.
  Feature directory structure: `product/features/{phase}-{NNN}/` with SCOPE.md (human-written, never modified by agents), specification/, pseudocode/, architecture/, refinement/, completion/, reports/.
  SCOPE.md is written by humans. Agents never modify it. Tracking via GitHub Issues, not STATUS.md files.
- **successRate**: 0.9
- **tags**: planning, sparc, swarm, waves, specification, architecture, pseudocode
- **source**: .claude/rules/agent-routing.md, CLAUDE.md
- **status**: current

---

### Pattern: ndp-pattern-workflow-mandatory

- **taskType**: convention:pattern-workflow
- **approach**: Mandatory before/after every task — no exceptions:
  **BEFORE work**: `/get-pattern` — searches AgentDB patterns + RL predictions + certified recall. Use the `get-pattern` skill.
  **AFTER work**: `/reflexion` for EACH pattern retrieved (one entry per pattern ID, NOT a project status update). Rate patterns:
  - 1.0: Pattern exactly right, followed directly
  - 0.7-0.9: Helped but needed minor adaptation
  - 0.4-0.6: Partially relevant, significant gaps
  - 0.1-0.3: Misleading/outdated, caused rework
  - 0.0: Wrong, actively harmful
  If a pattern is wrong/deprecated, record reflexion IMMEDIATELY (mid-task if needed, before any other work).
  `/save-pattern` for NEW reusable knowledge discovered.
  **Critical distinction**: AgentDB (permanent knowledge) vs claude-flow memory (transient coordination). NEVER use claude-flow memory for architectural knowledge. NEVER use AgentDB for swarm coordination status.
- **successRate**: 1.0
- **tags**: pattern-workflow, agentdb, reflexion, get-pattern, save-pattern, mandatory
- **source**: .claude/rules/pattern-workflow.md, CLAUDE.md
- **status**: current

---

### Pattern: ndp-deprecated-approaches

- **taskType**: convention:deprecated
- **approach**: These approaches are ELIMINATED and must NOT be used:
  1. **DuckDB as ETL engine or Gold layer** — eliminated entirely. Use TimescaleDB continuous aggregates and materialized views for all Gold layer work. DuckDB was used in DP-001 as a virtual Silver layer but is superseded by TimescaleDB Silver ETL.
  2. **Polars with streaming** — eliminated. Use TimescaleDB continuous aggregates instead.
  3. **Tall Bronze schema** (one row per metric) — superseded by wide raw JSON schema (one row per message, `raw_payload` as JSONB). See DP-004.
  4. **ResponseParser struct system** — superseded by unified `Parser` trait in AIR-006.
  5. **Dot-notation context flattening** — superseded by simple blob storage (single JSON column). See AIR-009 ADR-002-AMENDMENT-002.
  6. **K-NN as primary intelligence** — superseded by Online NN in V1.3. K-NN continues as bootstrap + validation baseline only.
  7. **claude plan mode** — never use. Write to SCOPE.md instead and use full SPARC planning swarm.
- **successRate**: 1.0
- **tags**: deprecated, duckdb, polars, eliminated, anti-pattern
- **source**: CLAUDE.md, docs/architecture/CONSOLIDATED_ARCHITECTURE_DECISIONS.md, product/features/gold-001/FEATURE-ROADMAPv1.3.md
- **status**: current

---

### Pattern: ndp-pi-resource-constraints

- **taskType**: convention:resource-constraints
- **approach**: NDP runs on Raspberry Pi 5. Hard resource constraints must be respected:
  **General limits**:
  - Total memory: < 2GB target
  - Single application container: < 512MB
  - Config propagation: < 100ms
  - Cross-stream query: < 100ms p99
  **Container memory limits** (production):
  - mosquitto: 128MB
  - etcd: 256MB
  - air-quality-app: 512MB
  - grafana: 256MB
  - timescaledb: 256MB
  - ndp-intelligence (V1.3): 512MB limit, ~275MB actual with SONA
  **V1.3 intelligence memory budget**: existing NDP 646MB + ndp-features +15MB + MiniLM ONNX +200MB (loaded on demand) + MLP weights +5MB + EWC Fisher +2MB + K-NN HNSW +2MB + SONA ReasoningBank +50MB = ~921MB total, intelligence container ~275MB.
  **Startup time**: < 60s including weight restoration on Pi.
  When designing new features, always include memory budget analysis.
- **successRate**: 1.0
- **tags**: pi, resource-constraints, memory-budget, container-limits, performance
- **source**: docs/architecture/CONSOLIDATED_ARCHITECTURE_DECISIONS.md, product/features/gold-001/FEATURE-ROADMAPv1.3.md
- **status**: current

---

### Pattern: ndp-anti-drift-level1-summary

- **taskType**: convention:swarm-anti-drift
- **approach**: The primary mechanism to prevent agent drift in swarms is including the **Level-1 summary** in every agent prompt. Level-1 summary includes:
  - Objective (what this feature achieves)
  - ADR pattern IDs (specific AgentDB pattern IDs to follow)
  - Constraints (what must not change, what not to add)
  - NOT-in-scope list (explicit exclusions)
  The Level-1 summary is generated by `/spec-compile` from the planning swarm output. Every worker agent in a swarm receives this summary. This prevents agents from adding features, refactoring unrelated code, or departing from architectural decisions.
  For large swarms (10-15 agents): `hive-mind_init(topology: "mesh")`. For small swarms (6-8 agents): `hive-mind_init(topology: "hierarchical")`. Hive-mind topology is optional but useful for tracking.
- **successRate**: 0.9
- **tags**: swarm, anti-drift, level-1-summary, agent-prompt, coordination
- **source**: .claude/rules/swarm-protocol.md
- **status**: current

---

### Pattern: ndp-swarm-memory-key-convention

- **taskType**: convention:memory-keys
- **approach**: All swarm coordination uses `namespace: "coordination"` with this key structure:
  - `swarm/{agent-id}/status` — agent writes on start (task received)
  - `swarm/{agent-id}/progress` — agent writes after each major step
  - `swarm/{agent-id}/complete` — agent writes before returning
  - `swarm/shared/{feature-id}-context` — coordinator seeds, agents read
  All values MUST include `"feature":"<feature-id>"` in the JSON payload (not just in the key) for semantic search recall.
  All `memory_store` calls MUST use `upsert: true` to prevent UNIQUE constraint failures on retries.
  For discovery: use `memory_list` + `memory_retrieve` (exact-key, 100% reliable) — NOT `memory_search` (semantic search has 20-80% recall on JSON payloads).
  This is claude-flow memory (transient, dies after session), NOT AgentDB (permanent knowledge).
- **successRate**: 1.0
- **tags**: swarm, memory, coordination, key-convention, namespace
- **source**: .claude/rules/swarm-protocol.md, .claude/rules/memory-commands.md
- **status**: current

---

### Pattern: ndp-testing-conventions

- **taskType**: testing:conventions
- **approach**: NDP testing conventions:
  - **Style**: London TDD (mock-driven, outside-in). Tests tell you WHAT behavior is expected.
  - **Location**: Tests live alongside source in standard Rust locations.
  - **Unit tests**: `cargo test --workspace` runs all.
  - **Integration tests**: MUST use the integration environment (`docker-compose.integration.yml`). All SPARC Refinement and Completion phases MUST validate against it.
  - **When to use integration env**: all schema changes, all ETL changes, any config affecting runtime behavior, after any deploy.sh change (run smoke testbed).
  - **Test baseline**: stored in `.ndp/test-baseline.txt` (single integer). `/validate` skill compares against baseline and warns on regression. Update manually after each confirmed successful release.
  - **Flaky tests**: listed in `.ndp/flaky-tests.txt`. Currently 6 known flaky tests (5 weather_polling_integration wiremock timing, 1 acceptance_partition_structure). Add new flaky tests with root cause comment.
  - **Testers**: use `ndp-tester` (not generic `tester`). AgentDB pattern ID 16 has London TDD details.
- **successRate**: 1.0
- **tags**: testing, tdd, london-tdd, integration, unit-tests, baseline, flaky-tests
- **source**: .claude/rules/testing.md, CLAUDE.md
- **status**: current

---

### Pattern: ndp-feature-tracking-gh-issues

- **taskType**: convention:tracking
- **approach**: New features and bugs are tracked via GitHub Issues, NOT in-repo STATUS.md files.
  - Implementation: `gh issue create --label "implementation,{phase}"`
  - Bugs: `gh issue create --label "bug,{phase}"`
  - SCOPE.md `## Tracking` section links to the GH Issue
  - Commits reference the issue number: e.g., `feat(dp-021): add manifest validation (#42)`
  Phase labels: `air` (Air Quality), `dp` (Data Platform), `fe` (Feature Engineering), `db` (Dashboards), `ml` (Predictions), `al` (Alerts), `ops` (Operations).
  The scrum-master agent manages GH Issue lifecycle as part of the swarm (creates on planning start, updates on wave completion, closes on acceptance).
  Features follow `{phase}-{NNN}` naming convention in `product/features/{phase}-{NNN}/`.
- **successRate**: 1.0
- **tags**: tracking, github-issues, feature, convention, labels, scrum-master
- **source**: CLAUDE.md, .claude/rules/agent-routing.md
- **status**: current

---

*End of pattern seed — procedures, testing, vision, and cross-cutting conventions.*
*Total patterns: 23*
*Ready for AgentDB ingestion via /save-pattern.*
