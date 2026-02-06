# CLAUDE.md Restructure Plan

## Goal
Reduce CLAUDE.md from 935 lines to ~120. Move procedural details to rules files. Store foundational conventions as AgentDB patterns. Delete reference docs that duplicate CAPABILITIES.md.

## Tasks

### 1. Rewrite CLAUDE.md
- [x] Add 7-line "Non-Negotiable Rules" constitution at top
- [x] Keep: project context, architecture, agent selection, feature conventions, release pointer, behavioral guards
- [x] Delete: lines 498-920 (CLI reference, hooks tables, agent lists, intelligence system, env vars, setup, etc.)
- [x] Add single-line pointer to `.claude-flow/CAPABILITIES.md` and `.claude/rules/`

### 2. Create `.claude/rules/` files
- [x] `swarm-protocol.md` — swarm init, topology, spawn-and-wait pattern (from lines 1-159)
- [x] `agent-routing.md` — routing tables, team formation, complexity detection (from lines 260-306)
- [x] `pattern-workflow.md` — get-pattern/reflexion/save-pattern detailed procedure (from lines 160-258)
- [x] `memory-commands.md` — claude-flow memory CLI syntax reference (from lines 841-899)
- [x] `testing.md` — integration env usage, SPARC validation requirements

### 3. Store missing AgentDB patterns
- [x] `conventions:deprecated-approaches` (ID 27) — no DuckDB, no Polars, why
- [x] `conventions:feature-naming` (ID 28) — {phase}-{NNN} table
- [x] `conventions:feature-directory-structure` (ID 29) — SPARC layout template
- [x] `architecture:ndp-overview` (ID 30) — Bronze/Silver/Gold, hexagonal, domain adapters
- [x] `conventions:project-structure` (ID 31) — directory layout
- [x] `procedure:sparc-swarm-workflow` (ID 32) — which agents for which SPARC phase
- [x] `procedure:integration-environment` (ID 33) — docker-compose, services, ports, DEPLOY_ENV

### 4. Post-implementation
- [x] Verify rules files load (check `.claude/rules/` glob)
- [ ] Run `/reflexion` on this restructure
