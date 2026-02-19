# NDP — Non-Negotiable Rules

1. **Feature work uses swarms** — spawn `ndp-scrum-master` who reads `.claude/protocols/`. No solo feature work. See `.claude/protocols/swarm-protocol.md`.
2. **BEFORE any work**: `/get-pattern` to search existing knowledge. No exceptions.
3. **AFTER any work**: `/reflexion` to record per-pattern feedback. `/save-pattern` for new discoveries. Negative feedback (wrong/outdated, reward 0.0) is URGENT — record immediately.
4. **Anti-stub**: Never leave TODO, `unimplemented!()`, `todo!()`, or placeholder functions. Ask the user if blocked.
5. **Never save files to root.** Use project directory structure.

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

---

## Project Structure

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
/.claude/protocols       - Swarm protocols (planning, implementation, routing)
/.claude/rules           - Contextual rules (testing, rust workspace)
```

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
├── IMPLEMENTATION-BRIEF.md     # Synthesizer output, implementation input
├── ALIGNMENT-REPORT.md         # Vision guardian output
├── ACCEPTANCE-MAP.md           # AC verification map
├── LAUNCH-PROMPT.md            # Implementation launch prompt
├── specification/              # SPARC S
├── pseudocode/                 # SPARC P
├── architecture/               # SPARC A
├── test-plan/                  # Test strategy + per-component plans
├── refinement/                 # SPARC R
├── completion/                 # SPARC C
└── reports/
```

### Implementation Tracking

Features and bugs tracked via **GitHub Issues**, not in-repo STATUS.md files.

- Implementation: `gh issue create --label "implementation,{phase}"`
- Bugs: `gh issue create --label "bug,{phase}"`
- Cross-reference: SCOPE.md `## Tracking` links to GH Issue; commits reference `(#NNN)`

---

## Behavioral Rules

- Be concise. Prefer short answers. Skip preamble, summaries, and repetition unless asked.
- **Pattern workflow is mandatory**: `/get-pattern` before work. `/reflexion` after work. A session without reflexion is incomplete.
- Do what has been asked; nothing more, nothing less.
- NEVER create files unless absolutely necessary. Prefer editing existing files.
- NEVER proactively create documentation files unless explicitly requested.
