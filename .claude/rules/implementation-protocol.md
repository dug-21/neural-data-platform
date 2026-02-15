---
paths:
  - "core/**/*.rs"
  - "apps/**/*.rs"
  - "crates/**/*.rs"
  - "tools/**/*.rs"
  - "config/**/*"
  - "deploy/**/*"
  - "product/features/**/refinement/**/*"
  - "product/features/**/completion/**/*"
---

# Implementation Swarm Protocol

Triggers on: implement, TDD, build, code, fix, refactor, migrate, SPARC R/C phases.

---

## Execution Model

Implementation swarms use **coordinator delegation**: the primary agent spawns `ndp-scrum-master` as the single swarm coordinator. The scrum-master then spawns implementation agents, monitors results, detects drift, runs validation, and updates the GH Issue.

```
Primary Agent                    ndp-scrum-master                 Implementation Agents
─────────────                    ────────────────                 ─────────────────────
get-pattern
read brief
spawn scrum-master ──────────►   read protocol + brief
                                 swarm init
                                 TaskCreate (all tasks)
                                 seed shared memory
                                 spawn agents (wave) ──────────► execute tasks
                                 ◄──────────────────────────────  return results
                                 drift check
                                 validate
                                 gh issue comment
◄──────────────────────────────  return summary
reflexion
save-pattern
```

Do NOT use TeamCreate — swarms are coordinator-driven via Task tool spawn-and-wait.

### Concurrency Rules

Each message batches ALL related operations of the same type:

- ALWAYS batch ALL TaskCreate calls in ONE message
- ALWAYS spawn ALL agents in ONE message via Task tool
- ALWAYS batch ALL file reads/writes/edits in ONE message
- ALWAYS batch ALL Bash commands in ONE message
- ALWAYS batch ALL memory store/retrieve operations in ONE message

### Agent Rules

- Agents return: file paths + test pass/fail + issues (NOT file contents)
- Read the IMPLEMENTATION BRIEF from the GH Issue body — not the full spec tree
- GH Issue is the single source of truth — all progress updates go to the issue via `gh issue comment`
- Do NOT write progress to markdown files (STATUS.md, completion reports, etc.)
- Max 2 validation fix iterations to protect context window
- Cargo output truncated to first error + summary line

---

## Flow: 4 Phases

### Phase 1: Preparation (primary agent)

Pattern search and brief reading happen BEFORE spawning the coordinator.

```
/get-pattern — search AgentDB for relevant patterns
```

Note which pattern IDs were returned for reflexion later.

**Read the implementation brief from the GitHub Issue body** (`gh issue view <N> --json body`). The GH Issue is the single source of truth. Do NOT read full SPARC specification/pseudocode/architecture directories — the brief contains everything agents need.

If the GH Issue body does not contain a brief, check `product/features/{feature-id}/IMPLEMENTATION-BRIEF.md` as fallback. If neither exists, ask the user: "No implementation brief found. Should I read the full SPARC specs, or generate a brief first?"

### Phase 2: Delegation (primary agent)

Spawn `ndp-scrum-master` with the full context needed to run the swarm. ONE Task call.

```
Task(
  subagent_type: "ndp-scrum-master",
  prompt: "You are coordinating the implementation swarm for {feature-id}.

    Read the implementation protocol: .claude/rules/implementation-protocol.md
    Read the brief: {GH Issue number or IMPLEMENTATION-BRIEF.md path}

    Pattern IDs from get-pattern: {list IDs}
    Feature namespace: {feature-id}

    Execute the swarm: init → define tasks → spawn agents → drift check →
    validate → update GH Issue.
    Return: files changed, test results, validation result, issues encountered."
)
```

After spawning: tell the user that the scrum-master is coordinating, then STOP. Wait for the scrum-master to return.

### Phase 3: Swarm Execution (ndp-scrum-master)

The scrum-master executes the following steps autonomously. These details are here so the scrum-master can read this file and follow them.

#### Step 3a: Initialize Coordination Layer (MCP)

Use the tested swarm-run methodology — MCP tools for coordination, Task tool for agents. Do NOT use `claude-flow swarm init` CLI (it is cosmetic and creates no real state).

```
Use ToolSearch to find "claude-flow hive" tools, then call:

mcp__claude-flow__hive-mind_init(
  topology: "hierarchical",
  queenId: "impl-lead"
)
```

This creates `.claude-flow/hive-mind/state.json` for shared coordination state.

For large features (10+ tasks): use `topology: "mesh"` for peer communication.

#### Step 3b: Definition + Coordination

Define ALL tasks via TaskCreate and seed shared memory via MCP — in ONE message.

If `/spec-compile` was run, retrieve the Level-1 summary first:
```
mcp__claude-flow__memory_retrieve(key="{feature}/summary", namespace="spec-{feature}")
```

Then define tasks and store context:
```
TaskCreate("Task 1 subject", "Task 1 description", "Active form 1")
TaskCreate("Task 2 subject", "Task 2 description", "Active form 2")
... (5-10+ tasks, with dependencies set via TaskUpdate)

mcp__claude-flow__memory_store(
  key: "{feature-id}-context",
  value: "{Level-1 summary + task descriptions + pattern IDs}",
  namespace: "{feature-id}"
)
```

Set task dependencies with TaskUpdate after creation.

#### Step 3c: Agent Spawning

Spawn ALL agents for the current wave in ONE message (parallel).

**Pre-spawn checklist** (verify before ANY Task call):
- [ ] swarm init ran (Bash output confirmed)
- [ ] Tasks defined (TaskCreate completed)
- [ ] Memory seeded (namespace confirmed)
- [ ] Brief read

If ANY item is unchecked, STOP. Complete the missing step first.

Agent types for implementation: `ndp-rust-dev`, `ndp-tester`, `ndp-timescale-dev`, `ndp-parquet-dev`

Each agent prompt MUST include:
1. **Level-1 summary** from compiled spec (if `/spec-compile` was run — retrieve from `memory_retrieve(key="{feature}/summary", namespace="spec-{feature}")`)
2. Task description (2-3 sentences)
3. Namespace for claude-flow memory coordination
4. Specific file paths from the brief's "Files to Create/Modify" section
5. Instructions to retrieve relevant ADRs before implementing — use `/get-pattern` (which calls `agentdb_pattern_search` internally)

The Level-1 summary gives agents the objective (WHY), ADR list with pattern IDs (WHAT CONSTRAINS THEM), constraints, and scope exclusions (WHAT TO AVOID). Without it, agents have tunnel vision on their narrow subtask and drift from architectural decisions.

#### Step 3d: Drift Check

After agents return, check results against the brief:

| Check | Action |
|-------|--------|
| Files modified outside scope | Flag in summary |
| TODOs, stubs, `unimplemented!()` left | Spawn fix agent |
| Acceptance criteria missed | Spawn gap-fill agent |
| Test count decreased | Investigate before next wave |

Max 2 corrective iterations per wave. If drift persists, return to primary agent.

#### Step 3e: Validation

Run validation. Three tiers:

**Tier 1 — Unit (always)**
```bash
cargo build --workspace 2>&1 | head -50
cargo test --workspace 2>&1 | tail -30
```
Plus anti-stub scan and deploy.sh integrity check.

**Tier 2 — Lint (always for new code)**
```bash
cargo clippy --workspace -- -D warnings 2>&1 | head -30
```

**Tier 3 — Integration (for qualifying changes)**

Path A — Full release test via deploy.sh (binary, config, ETL changes):
```bash
DEPLOY_ENV=integration ./deploy/pi/deploy.sh build
DEPLOY_ENV=integration ./deploy/pi/deploy.sh deploy
DEPLOY_ENV=integration ./deploy/pi/deploy.sh status
DEPLOY_ENV=integration ./deploy/pi/deploy.sh stop
```

Path B — docker-compose only (schema DDL, Grafana, MCP changes):
```bash
docker compose -f docker-compose.integration.yml up -d
# Run targeted checks
docker compose -f docker-compose.integration.yml down -v
```

| Changed Paths | Tier 3 Path |
|---------------|-------------|
| `core/`, `apps/`, `crates/` (Rust binary) | A (deploy.sh) |
| `config/base/streams/`, `config/integration/` | A (deploy.sh) |
| `apps/silver-etl/`, `crates/ndp-lib/src/silver/` | A (deploy.sh) |
| `tools/ndp-gold-ddl/`, `deploy/pi/init-scripts/` | B (compose) |
| `config/grafana/` | B (compose) |
| `core/ndp-mcp-server/` | B (compose) |

If no qualifying paths touched, skip Tier 3.

**Validation iteration cap:**
- Iteration 1: Fix the FIRST error only. Re-validate.
- Iteration 2: If still failing, STOP iterating.
  Report: "Validation failed after 2 attempts. Remaining errors: [summary]"

#### Step 3f: GH Issue Update

Post results as an issue comment:
```bash
gh issue comment <N> --body "## Wave X Complete
- Files: [list paths]
- Tests: X passed, Y new
- Validation: PASS/WARN/FAIL
- Issues: [if any]"
```

#### Multi-Wave Features

For features with sequential waves:
- Spawn ALL agents within a wave in ONE message (parallel)
- Wait for the wave to complete
- Run drift check (Step 3d)
- Store wave results in shared memory
- Spawn the next wave's agents in a NEW message
- Repeat until complete
- Post `gh issue comment` after each wave

Do NOT spawn agents from different waves in the same message if Wave N+1 depends on Wave N.

### Phase 4: Completion (primary agent)

After ndp-scrum-master returns:

1. Review the summary (files changed, tests, validation result)
2. Record learning:
   ```
   /reflexion — record pattern effectiveness (per pattern used, referencing IDs)
   /save-pattern — store new discoveries (if any)
   ```

---

## Quick Reference: Message Map

```
PRIMARY AGENT:
  Message 1:  /get-pattern + Read GH Issue body (brief)
  Message 2:  Task(ndp-scrum-master) — delegate swarm execution
  ...wait...
  Message 3:  Review results + /reflexion + /save-pattern

NDP-SCRUM-MASTER (internal):
  Step 3a:  MCP: hive-mind_init (coordination layer)
  Step 3b:  TaskCreate (batch ALL) + MCP: memory_store (Level-1 summary + context)
  Step 3c:  Task() spawn ALL wave agents (parallel)
  Step 3d:  Drift check
  Step 3e:  Validate (Tier 1 + 2 + 3 as applicable)
  Step 3f:  gh issue comment
  (repeat 3c-3f for multi-wave)
```

---

## Agent Context Budget

Each spawned implementation agent should receive:
- Task description (2-3 sentences)
- Namespace for claude-flow memory coordination
- Specific file paths to read and modify
- Relevant AgentDB pattern IDs (not full pattern text)

Do NOT paste: full spec documents, full source files, full cargo output, or implementation brief contents into agent prompts. Agents should read files themselves.

---

## Cargo Output Truncation

Always truncate cargo output to prevent context bloat:
```bash
# Build: first error + summary
cargo build --workspace 2>&1 | grep -A5 "^error" | head -20
cargo build --workspace 2>&1 | tail -3

# Test: summary only
cargo test --workspace 2>&1 | tail -30

# Clippy: first warnings only
cargo clippy --workspace -- -D warnings 2>&1 | head -30
```

---

## Two Memory Systems

| System | Tool | Purpose |
|--------|------|---------|
| **AgentDB** | `/get-pattern`, `/save-pattern`, `/reflexion` | Permanent project knowledge |
| **Claude-Flow Memory** | `claude-flow memory` CLI via Bash | Transient swarm coordination |
