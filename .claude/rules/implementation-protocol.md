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

## Rules

- Read the IMPLEMENTATION BRIEF — not the full spec tree
- Run `cargo test --workspace` before presenting results
- Run integration env validation for qualifying changes (see Tier 3 below)
- `/validate` is mandatory before completion
- Agents return: file paths + test pass/fail + issues (NOT file contents)
- Cargo output truncated to first error + summary line
- Track progress via GitHub Issue (not STATUS.md)
- Max 2 validation fix iterations to protect context window

---

## Implementation Swarm Steps

### Step 1: Pattern search
```
/get-pattern — search AgentDB for relevant patterns
```

### Step 2: Initialize swarm coordination
```bash
claude-flow swarm init --topology hierarchical --max-agents 8 --strategy specialized
```

### Step 3: Seed shared memory
```bash
claude-flow memory store --key "{feature-id}-context" --value "{task description, goals, constraints}" --namespace {feature-id}
```

### Step 4: Read Implementation Brief

Read `product/features/{feature-id}/IMPLEMENTATION-BRIEF.md` — this is the ONLY planning artifact implementation agents need. Do NOT read the full specification, pseudocode, or architecture directories.

If no implementation brief exists, ask the user: "No implementation brief found. Should I read the full SPARC specs, or generate a brief first?"

### Step 5: Spawn implementation agents via Task tool

Each agent prompt MUST include:
1. Task description (2-3 sentences)
2. Namespace for claude-flow memory coordination
3. Specific file paths from the brief's "Files to Create/Modify" section
4. Relevant AgentDB pattern IDs

Agent types for implementation: `ndp-rust-dev`, `ndp-tester`, `ndp-timescale-dev`, `ndp-parquet-dev`

### Step 6: Validate before reporting

Run `/validate`. Three tiers:

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
- Iteration 1: Fix the FIRST error only. Re-run `/validate`.
- Iteration 2: If still failing, STOP iterating.
  Report to user: "Validation failed after 2 attempts. Remaining errors: [summary]"
- Do NOT iterate beyond 2. This protects context for the user to intervene.

**Context-saving option:** If the coordinator has already consumed significant context, spawn a Task agent (ndp-tester) for validation instead:
```
"Run /validate on the workspace. Fix up to 2 blocking errors.
 Report: PASS/WARN/FAIL with summary."
```

### Step 7: Synthesize and report

Report to user:
- Files created/modified (paths only)
- Test results (pass/fail count)
- Validation result (PASS/WARN/FAIL)
- Issues encountered
- GitHub Issue to update (if applicable)

### Step 8: After completion
```
/reflexion — record pattern effectiveness (per pattern used)
/save-pattern — store new discoveries (if any)
```
Update the GitHub Issue with completion status.

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
