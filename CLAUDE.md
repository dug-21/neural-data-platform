# NDP — Non-Negotiable Rules

1. **BEFORE any work**: `/get-pattern` to search existing knowledge. No exceptions.
2. **AFTER any work**: `/reflexion` to record what helped and what didn't. `/save-pattern` for new discoveries.
3. **Features use SPARC phases with SWARM** — no solo feature work. See `.claude/rules/swarm-protocol.md`.
4. **Integration env exists** (`deploy/pi/deploy.sh`, `docker-compose.integration.yml`) — use it for all validation. See `.claude/rules/testing.md`.
5. **You are training future agents.** Knowledge capture is as important as code delivery.
6. **Anti-stub**: Never leave TODO, `unimplemented!()`, `todo!()`, or placeholder functions. Ask the user if blocked.
7. **Never save files to root.** Use project directory structure.
8. **Never use claude plan mode** Write to scope.md.  Leverage full SPARC planning swarm.

---

## Project Context

**Neural Data Platform** — Configuration-driven time-series data platform for Raspberry Pi. Declarative deployment, neural capabilities for causal relationship learning, predictive actions on the edge.

### Architecture

- **Data Lake**: Bronze (Parquet + WAL) → Silver (TimescaleDB hypertables + continuous aggregates) → Gold (materialized views, generated DDL)
- **Hexagonal Architecture**: Domain adapters with Source/Sink traits
- **Deployment**: Docker containers on Pi, git as transport, declarative manifests in `.deploy/releases/`

### Deprecated Approaches (DO NOT USE)

- **DuckDB** (as ETL engine or Gold layer) — eliminated entirely, use TimescaleDB
- **Polars with streaming** — use TimescaleDB continuous aggregates

### Project Structure

```
/core                    - Rust library (neural-core)
/apps                    - Application binaries (air-quality-app)
/crates                  - Shared crates (ndp-types, ndp-lib)
/config                  - Stream configurations (base/streams/)
/config-client           - etcd configuration client
/deploy                  - Docker and Pi deployment
/docs                    - Architecture docs and procedures
/product/features/       - SPARC documentation per feature
/tools                   - CLI tools (ndp-cli, ndp-validate, ndp-gold-ddl)
/.claude/agents/ndp      - NDP agent definitions
/.claude/rules           - Contextual rules (swarm, routing, testing, patterns)
```

### Cargo Workspace Members

ndp-types, config-client, core, ndp-mcp-server, air-quality domain, air-quality-app, silver-etl, ndp-validate, ndp-gold-ddl. ops-001 adds: `crates/ndp-lib`, `tools/ndp-cli`.

---

## Agent Selection

Always use NDP-specific agents over generic ones:

| Instead of | Use | Why |
|------------|-----|-----|
| `coder` | `ndp-rust-dev` | Knows Rust patterns, project structure |
| `system-architect` | `ndp-architect` | Knows Domain Adapter pattern, ADRs |
| `tester` | `ndp-tester` | Knows test patterns, mocking approach |
| `planner` | `ndp-scrum-master` | Knows feature lifecycle, SPARC phases |

Full agent roster and routing tables: `.claude/rules/agent-routing.md`
Agent definitions: `.claude/agents/ndp/README.md`

---

## Feature Conventions

Features follow `{phase}-{NNN}` pattern in `product/features/`:

| Phase | Prefix | Focus |
|-------|--------|-------|
| Air Quality | `air` | Foundation, sensors, external data (COMPLETE) |
| Data Platform | `dp` | Silver layer, TimescaleDB, ETL |
| Feature Engineering | `fe` | ML features, aggregations |
| Dashboards | `db` | Grafana, visualization |
| Predictions | `ml` | ruv-FANN, forecasting |
| Alerts | `al` | Triggers, notifications |
| Operations | `ops` | Tooling, CLI, deployment automation |

### Feature Directory Structure (SPARC)

```
product/features/{phase}-{NNN}/
├── SCOPE.md                    # Human writes, agents never modify
├── IMPLEMENTATION-BRIEF.md     # Planning swarm output, implementation input
├── ALIGNMENT-REPORT.md         # Vision guardian output
├── specification/              # SPARC S
├── pseudocode/                 # SPARC P
├── architecture/               # SPARC A
├── refinement/                 # SPARC R
├── completion/                 # SPARC C
└── reports/
```

### Implementation Tracking

New features and bugs are tracked via **GitHub Issues**, not in-repo STATUS.md files.

- Implementation: `gh issue create --label "implementation,{phase}"`
- Bugs: `gh issue create --label "bug,{phase}"`
- Cross-reference: SCOPE.md `## Tracking` links to GH Issue; commits reference `(#NNN)`

---

## Release Methodology

Follow `docs/procedures/RELEASE-POLICY.md` exactly. Semver: MAJOR (breaking), MINOR (features), PATCH (fixes).

Every release requires 3 artifacts:
1. **Manifest**: `.deploy/releases/vX.Y.Z.manifest.json`
2. **Git Tag**: `vX.Y.Z` (annotated)
3. **Changelog**: `CHANGELOG.md` entry

See also: `docs/procedures/DEPLOYMENT-DECLARATIVES.md`, `.deploy/releases/TEMPLATE.manifest.json`

---

## Behavioral Rules

- Be concise. Prefer short answers. Skip preamble, summaries, and repetition unless asked.
- **Pattern workflow is mandatory**: `/get-pattern` before work. `/reflexion` (per pattern used) after work. A session without reflexion is incomplete.
- Do what has been asked; nothing more, nothing less.
- NEVER create files unless absolutely necessary. Prefer editing existing files.
- NEVER proactively create documentation files unless explicitly requested.

---

## Reference

- **Swarm protocol, model routing, spawn patterns**: `.claude/rules/swarm-protocol.md`
- **Agent routing, team formation**: `.claude/rules/agent-routing.md`
- **Pattern workflow (get-pattern/reflexion/save-pattern)**: `.claude/rules/pattern-workflow.md`
- **Memory commands (claude-flow CLI)**: `.claude/rules/memory-commands.md`
- **Testing, integration environment**: `.claude/rules/testing.md`
- **Validation (implementation)**: `.claude/skills/validate/SKILL.md` -- 4-tier validation with glass box reports
- **Validation (planning)**: `.claude/skills/validate-plan/SKILL.md` -- 5-check planning artifact validation
- **Trust dashboard**: `.claude/skills/trust-dashboard/SKILL.md` -- Bayesian trust scores from AgentDB
- **Shadow judge**: `.claude/skills/shadow-judge/SKILL.md` -- Human judgment recording
- **Full CLI, hooks, agents, intelligence reference**: `.claude-flow/CAPABILITIES.md`
- **Agent definitions**: `.claude/agents/ndp/README.md`
