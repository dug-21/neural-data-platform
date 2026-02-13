# Continuous Improvement Plan v3

Based on insights report (77 sessions, 63 analyzed, 509h compute) + user refinements.

## Problem Statement

Six friction areas to address:
1. **Swarm protocol bypass** (4+ sessions) — agents skip init, launch single Tasks instead of swarms
2. **No validation gate** — implementation sessions have no automated quality check
3. **Pattern workflow skipped** — get-pattern/reflexion/save-pattern documented but not enforced
4. **Context window exhaustion** — three distinct pressure points burn through context
5. **Tracking fragmentation** — implementation status and bugs tracked in-repo (STATUS.md, bugs/) but not visible in GitHub; planning docs and tracking are intermingled
6. **Vision drift in specifications** — deep spec work loses sight of product vision; user catches misalignments manually by reading 15-20K lines of SPARC artifacts

## Accepted Constraints
- Dev space has no production access — deploy validation stays manual/relay-based
- User retains architectural control — no autonomous architecture decisions
- No CI/CD changes — Docker builds on Pi, git as transport

---

## Workstream 1: Split Swarm Protocols + Hook Enforcement

**Goal**: Different protocols for planning vs implementation, enforced by hooks.

### The Problem

Planning and implementation have different requirements but currently share one protocol. Planning needs artifact structure rules and no-code-changes guardrails. Implementation needs test suites, integration env, validation gates, and cargo output management.

### 1a. Split into two protocol files

**`.claude/rules/planning-protocol.md`** — triggers on: spec, plan, design, research, scope, architecture, pseudocode, SPARC S/P/A phases

Rules:
- Output goes to `product/features/{feature-id}/{phase}/` only
- NO code changes, NO file edits outside product/features/
- NO launching implementation agents
- Each planning agent gets: SCOPE.md + relevant existing SPARC artifacts + relevant AgentDB patterns
- Agents return: artifact paths + key decisions + open questions
- Generate an **implementation brief** as final artifact (see WS4)

**`.claude/rules/implementation-protocol.md`** — triggers on: implement, build, code, fix, refactor, migrate, TDD, test, SPARC R/C phases

Rules:
- Read the implementation brief (NOT the full spec tree)
- Run `cargo test --workspace` before presenting results
- Run integration env validation for schema/ETL/config changes (see WS2)
- `/validate` is mandatory before completion
- Agents return: file paths + test pass/fail + issues (NOT file contents)
- Cargo output truncated to first error + summary line
- Create/update GitHub Issue for implementation tracking (see WS5)

### 1b. Hook detects planning vs implementation

Updated `.claude/hooks/prompt-check.sh`:

```bash
#!/bin/bash
PROMPT="$1"
PROMPT_LOWER=$(echo "$PROMPT" | tr '[:upper:]' '[:lower:]')

# Always inject current version (keeps versioning OUT of CLAUDE.md)
CURRENT_VERSION=$(git describe --tags --abbrev=0 2>/dev/null || echo "unknown")

# Simple tasks — no protocol needed
SKIP_KEYWORDS="typo|update comment|quick question|what is|how do|explain |read the|show me|^list |^commit|^push|^status|^review |^check |rename |single file|one line|^debug |^where is|^find |^search |^look at|^can you|^does |^tell me|^why does|^why is|insights|reflexion|save-pattern|get-pattern|learner"

if echo "$PROMPT_LOWER" | grep -qiE "$SKIP_KEYWORDS"; then
  echo "[SIMPLE_TASK] No swarm needed. Version: $CURRENT_VERSION"
  exit 0
fi

# Planning keywords
PLAN_KEYWORDS="sparc planning|specification|pseudocode|architecture phase|design|research|scope|roadmap|sparc s |sparc p |sparc a |phase-a|phase-b|plan for"

# Implementation keywords
IMPL_KEYWORDS="implement|tdd|build |code |fix |refactor|migrate|sparc r |sparc c |refinement|completion|phase-r|phase-c"

# General swarm keywords (default to implementation if ambiguous)
SWARM_KEYWORDS="feature|schema|etl|pipeline|generator|migration|new stream|new table|gold layer|silver layer|bronze layer|ops-|dp-|fe-|ml-|al-|db-|across|multiple files"

if echo "$PROMPT_LOWER" | grep -qiE "$PLAN_KEYWORDS"; then
  echo "================================================================"
  echo "PLANNING SWARM — Version: $CURRENT_VERSION"
  echo "Protocol: .claude/rules/planning-protocol.md"
  echo ""
  echo "MANDATORY steps:"
  echo "  1. /get-pattern — search AgentDB for relevant patterns"
  echo "  2. claude-flow swarm init --topology hierarchical --max-agents 8"
  echo "  3. claude-flow memory store --namespace {feature-id}"
  echo "  4. Spawn planning agents (ndp-architect, ndp-scrum-master, specification, pseudocode)"
  echo ""
  echo "PLANNING RULES:"
  echo "  - Output to product/features/{id}/{phase}/ ONLY"
  echo "  - NO code changes. NO implementation agents."
  echo "  - Generate implementation brief as final artifact"
  echo ""
  echo "AFTER: /reflexion + /save-pattern"
  echo "================================================================"
  exit 0
fi

if echo "$PROMPT_LOWER" | grep -qiE "$IMPL_KEYWORDS"; then
  echo "================================================================"
  echo "IMPLEMENTATION SWARM — Version: $CURRENT_VERSION"
  echo "Protocol: .claude/rules/implementation-protocol.md"
  echo ""
  echo "MANDATORY steps:"
  echo "  1. /get-pattern — search AgentDB for relevant patterns"
  echo "  2. claude-flow swarm init --topology hierarchical --max-agents 8"
  echo "  3. claude-flow memory store --namespace {feature-id}"
  echo "  4. Read IMPLEMENTATION BRIEF (not full spec tree)"
  echo "  5. Spawn implementation agents (ndp-rust-dev, ndp-tester)"
  echo ""
  echo "IMPLEMENTATION RULES:"
  echo "  - Agents read brief + specific source files only"
  echo "  - cargo test --workspace before reporting"
  echo "  - /validate before presenting results"
  echo "  - Integration env for schema/ETL/config changes"
  echo "  - Truncate cargo output: first error + summary"
  echo "  - Track progress via GitHub Issue (not STATUS.md)"
  echo ""
  echo "AFTER: /reflexion + /save-pattern"
  echo "================================================================"
  exit 0
fi

if echo "$PROMPT_LOWER" | grep -qiE "$SWARM_KEYWORDS"; then
  echo "================================================================"
  echo "SWARM DETECTED — Version: $CURRENT_VERSION"
  echo "Determine if this is PLANNING or IMPLEMENTATION, then follow"
  echo "the appropriate protocol in .claude/rules/"
  echo ""
  echo "MANDATORY: /get-pattern first. /reflexion + /save-pattern after."
  echo "================================================================"
  exit 0
fi

# Medium-complexity: pattern workflow still required
echo "[TASK] Version: $CURRENT_VERSION. Run /get-pattern before work. /reflexion when done."
```

### 1c. Post-Task reflexion reminder

Add to PostToolUse in settings.json alongside existing Task hook:

```json
{
  "type": "command",
  "command": "echo '[REMINDER] Task complete. You owe: /reflexion for each pattern used, /save-pattern for new discoveries.'",
  "timeout": 1000,
  "continueOnError": true
}
```

### 1d. Versioning stays OUT of CLAUDE.md

The hook injects `$CURRENT_VERSION` into every prompt context via `git describe --tags --abbrev=0`. No CLAUDE.md section needed — the version is always live, eliminating stale references.

---

## Workstream 2: Validation Skill with Integration Environment

**Goal**: Every implementation session has a structured validation gate. Integration testing uses the full deploy.sh path.

### 2a. Create `/validate` skill

`.claude/skills/validate/SKILL.md` — three tiers, always run Tier 1, conditionally escalate.

**Tier 1 — Unit (always)**
1. `cargo build --workspace 2>&1 | head -50` — compile check, truncated
2. `cargo test --workspace 2>&1 | tail -30` — test suite, summary only
3. Anti-stub scan: grep for `todo!()`, `unimplemented!()`, `TODO`, `FIXME`
4. `git diff deploy/pi/deploy.sh` — deploy.sh integrity

**Tier 2 — Lint (always for new code)**
5. `cargo clippy --workspace -- -D warnings 2>&1 | head -30`

**Tier 3 — Integration (for qualifying changes)**

Two paths, chosen by what's changing:

**Path A: Full release test via deploy.sh**
For changes that affect the deployed application (binary, config loading, startup behavior):
```bash
DEPLOY_ENV=integration ./deploy/pi/deploy.sh build
DEPLOY_ENV=integration ./deploy/pi/deploy.sh deploy
DEPLOY_ENV=integration ./deploy/pi/deploy.sh status
# deploy.sh handles: docker build, compose up, healthchecks, config sync
# Uses config/integration/ paths automatically
DEPLOY_ENV=integration ./deploy/pi/deploy.sh stop
```

**Path B: docker-compose only**
For isolated component changes (schema DDL, Grafana dashboards, MCP tools):
```bash
docker compose -f docker-compose.integration.yml up -d
# Wait for healthchecks
# Run targeted checks (DDL apply, query test, dashboard load)
docker compose -f docker-compose.integration.yml down -v
```

**When to use which:**

| Change Type | Path | Why |
|-------------|------|-----|
| Binary changes (Rust code) | A (deploy.sh) | Tests the full build + deploy cycle |
| Config changes (stream YAML) | A (deploy.sh) | deploy.sh handles config sync to etcd |
| Schema changes (DDL, SQL) | B (compose) | Only need TimescaleDB running |
| Dashboard changes (Grafana) | B (compose) | Only need Grafana + TimescaleDB |
| MCP server changes | B (compose) | Only need MCP + dependencies |
| ETL pipeline changes | A (deploy.sh) | Tests MQTT→Bronze→Silver flow |

**Reporting format:**
```
VALIDATION RESULT: PASS | WARN | FAIL
  Tier 1 (Unit):        PASS  [904 tests, 0 failures]
  Tier 2 (Lint):        WARN  [2 clippy warnings]
  Tier 3 (Integration): PASS  [deploy.sh: build OK, deploy OK, status healthy]
  Anti-stub:            PASS  [0 stubs found]
  Deploy.sh:            PASS  [unchanged]
```

### 2b. Wire validation into implementation-protocol.md

Validation is Step 5 (mandatory):

```
### Step 5: Validate before reporting
1. Run /validate
2. If FAIL: fix blocking issues, re-run (max 2 iterations to avoid context burn)
3. If still FAIL after 2 iterations: report failure clearly with error summary
4. If PASS/WARN: proceed to synthesis
```

### 2c. Integration environment triggers

| Change Type | Detection (git diff paths) | Tier 3 Path |
|-------------|---------------------------|-------------|
| Binary (Rust code) | `core/`, `apps/`, `crates/` | A (deploy.sh) |
| Config (stream YAML) | `config/base/streams/`, `config/integration/` | A (deploy.sh) |
| Schema (SQL, DDL) | `tools/ndp-gold-ddl/`, `deploy/pi/init-scripts/` | B (compose) |
| Dashboard (Grafana) | `config/grafana/` | B (compose) |
| MCP server | `core/ndp-mcp-server/` | B (compose) |
| ETL pipeline | `apps/silver-etl/`, `crates/ndp-lib/src/silver/` | A (deploy.sh) |

If no qualifying paths touched, Tier 3 is skipped.

### 2d. Known gap: test datasets

Integration env currently has no pre-loaded test data. For now:
- MQTT injection: `mosquitto_pub` with sample payloads (already documented in docker-compose.integration.yml)
- Bronze: verified by write+read round-trip
- Silver: verified by MQTT→ETL→query path
- Gold: verified by DDL apply + empty view creation

Future enhancement: `config/integration/test-data/` with fixture files.

---

## Workstream 3: Strengthen Pattern Workflow Enforcement

**Goal**: Make get-pattern/reflexion/save-pattern harder to skip than to follow.

### 3a. CLAUDE.md behavioral rule (concise)

Add to existing Behavioral Rules section:

```markdown
- **Pattern workflow is mandatory**: `/get-pattern` before work. `/reflexion` (per pattern used) after work. A session without reflexion is incomplete.
```

One line. Not a new section. Reinforces the hook injection.

### 3b. Stop hook reflexion check

Update the existing Stop hook:

```bash
echo '[SESSION END] /reflexion recorded? /save-pattern for new discoveries?' && echo '{"ok": true}'
```

### 3c. Pattern maintenance cadence

| Trigger | Action | Frequency |
|---------|--------|-----------|
| After every feature completion | `/learner` run | Per feature |
| Every 10 sessions (~weekly) | `agentdb reflexion prune 90 0.5` | Weekly |
| Every 20 sessions (~biweekly) | `agentdb db stats` + review, deprecate stale patterns | Biweekly |

---

## Workstream 4: Context Window Protection (3 Pressure Points)

### Pressure Point 1: Spec Volume → Implementation Brief

**The problem**: SPARC planning generates 15-20K+ lines per feature (ops-003: 19,018 lines across 24 files). Implementation agents try to read the full spec tree PLUS source code, exhausting context before writing a single line.

**The solution: Implementation Brief**

Each planning swarm's FINAL artifact is a condensed `IMPLEMENTATION-BRIEF.md` (target: 200-400 lines) stored at `product/features/{id}/IMPLEMENTATION-BRIEF.md`.

The brief contains ONLY what an implementation agent needs:

```markdown
# {Feature} Implementation Brief

## Goal (2-3 sentences)
What this feature does and why.

## GitHub Issue
Link to the implementation tracking issue.

## Files to Create/Modify
- `path/to/file.rs` — what to add/change (1-line summary)
- `path/to/other.rs` — what to add/change

## Data Structures
Key structs, enums, trait signatures (actual Rust code, not prose).

## Function Signatures
Exact signatures to implement, with doc comments.

## Test Expectations
- Unit: what to test, expected assertions
- Integration: what to verify against live stack

## Constraints
- Version: v1.1.x
- No DuckDB, no Polars
- ARM64 compatible
- Config-driven, no hardcoding

## Dependencies
Crates to add to Cargo.toml, features to enable.

## NOT in Scope
What this feature explicitly does NOT include.
```

**Enforcement**: Planning protocol requires generating the brief. Implementation protocol requires reading the brief (NOT the full spec tree). The hook injects this rule.

**Implementation agents receive**: IMPLEMENTATION-BRIEF.md + the specific source files listed in "Files to Create/Modify" + relevant AgentDB patterns. Nothing else.

### Pressure Point 2: Swarm Memory Distribution

**Already addressed by better protocol compliance** (WS1). Key reinforcement in both protocol files:

```markdown
## Agent Context Budget
Each spawned agent should receive:
- Task description (2-3 sentences)
- Namespace for claude-flow memory coordination
- Specific file paths to read (not "explore the codebase")
- Relevant AgentDB pattern IDs (not full pattern text)

Do NOT paste full spec documents, full source files, or full cargo output into agent prompts.
```

### Pressure Point 3: Validation Iteration Cargo Output

**The problem**: When `/validate` finds failures, the coordinator iterates — reading full cargo output each time. 3 iterations x 200 lines = 600 lines burned on diagnostics.

**Solutions (layered)**:

**A. Truncate aggressively in the validate skill**:
```bash
# Only first error + summary
cargo build --workspace 2>&1 | grep -A5 "^error" | head -20
cargo build --workspace 2>&1 | tail -3
```

**B. Cap validation iterations at 2**:
```
If /validate fails:
  - Iteration 1: Fix the FIRST error only. Re-run.
  - Iteration 2: If still failing, STOP.
    Report to user: "Validation failed after 2 attempts. Remaining errors: [summary]"
  - Do NOT iterate beyond 2.
```

**C. Delegate validation to a sub-agent when context is tight**:
Spawn a Task agent (ndp-tester) for validation. Keeps error output in the sub-agent's context, not the coordinator's.

---

## Workstream 5: GitHub Issues for Implementation Tracking

**Goal**: Separate concerns — SPARC planning stays in `product/features/`, implementation tracking and bugs move to GitHub Issues.

### The Current State

| System | Files | Status |
|--------|-------|--------|
| STATUS.md (38 files) | In `product/features/{id}/STATUS.md` | Maintained by ndp-scrum-master agent |
| bugs/ dirs (29 bug files) | In `product/features/{id}/bugs/BUG-{NNN}-*.md` | Per-feature bug tracking |
| GitHub Issues (6 total) | #11-#16 | Used ad-hoc for cross-feature concerns only |

BUG-005/ops-004 already demonstrates the hybrid pattern: GitHub Issue #16 exists alongside in-repo STATUS.md and SPARC docs. This workstream formalizes that emerging pattern.

### The Split

| Activity | Where | Why |
|----------|-------|-----|
| SPARC planning (S/P/A) | `product/features/` | Long-form design artifacts, agent-generated, reviewed by user |
| Implementation brief | `product/features/{id}/IMPLEMENTATION-BRIEF.md` | Bridge from planning to implementation |
| SCOPE.md | `product/features/{id}/SCOPE.md` | Human-written, stays in-repo |
| Implementation tracking | **GitHub Issue** | Visible progress, comments, labels, cross-linking |
| Bug tracking | **GitHub Issue** | Searchable, assignable, closeable |
| SPARC refinement/completion docs | `product/features/` | Implementation details stay with planning docs |

### What changes

**5a. GitHub Issue templates** — create `.github/ISSUE_TEMPLATE/`

`ndp-implementation.yml`:
```yaml
name: Implementation Tracking
description: Track implementation of a planned feature
labels: ["implementation"]
body:
  - type: input
    id: feature-id
    attributes:
      label: Feature ID
      description: e.g., dp-021, ops-005
      placeholder: "dp-XXX"
    validations:
      required: true
  - type: input
    id: sparc-path
    attributes:
      label: SPARC Docs Path
      description: Path to planning artifacts
      placeholder: "product/features/dp-XXX/"
  - type: textarea
    id: goal
    attributes:
      label: Goal
      description: 2-3 sentence summary
  - type: textarea
    id: acceptance
    attributes:
      label: Acceptance Criteria
      description: What must be true for this to be done
  - type: textarea
    id: tasks
    attributes:
      label: Implementation Tasks
      description: Checklist of work items
      value: |
        - [ ] Read implementation brief
        - [ ] Implement changes
        - [ ] Unit tests passing
        - [ ] Integration validation
        - [ ] /validate PASS
        - [ ] /reflexion recorded
```

`ndp-bug.yml`:
```yaml
name: Bug Report
description: Report a bug in NDP
labels: ["bug"]
body:
  - type: input
    id: feature-id
    attributes:
      label: Related Feature
      placeholder: "dp-XXX or ops-XXX"
  - type: input
    id: version
    attributes:
      label: Version
      placeholder: "v1.1.XX"
  - type: textarea
    id: description
    attributes:
      label: Description
      description: What happened vs what was expected
  - type: textarea
    id: reproduction
    attributes:
      label: Reproduction Steps
  - type: textarea
    id: root-cause
    attributes:
      label: Root Cause Analysis
      description: Fill in during investigation
```

**5b. Label scheme**

| Label Category | Labels | Purpose |
|----------------|--------|---------|
| Type | `bug`, `implementation`, `enhancement` | Issue classification |
| Feature phase | `air`, `dp`, `fe`, `ops`, `ml`, `al`, `db` | Feature domain |
| SPARC phase | `sparc:refinement`, `sparc:completion` | Which SPARC phase |
| Priority | `P0-critical`, `P1-high`, `P2-normal` | Triage |
| Status | `in-progress`, `blocked`, `needs-review` | Workflow state |

**5c. Cross-referencing convention**

- SCOPE.md gets a `## Tracking` section with GitHub Issue link
- IMPLEMENTATION-BRIEF.md includes `## GitHub Issue` field
- GitHub Issue body links to `product/features/{id}/` for SPARC docs
- Commits reference issue: `fix: description (#NNN)`

**5d. ndp-scrum-master agent rewrite**

The heaviest impact. Current agent revolves around STATUS.md maintenance. Rewrite scope:

| Current Responsibility | New Responsibility |
|----------------------|-------------------|
| Create/update STATUS.md | Create/update GitHub Issue |
| BUG-{NNN} file creation | Create GitHub Issue with `bug` label (no BUG- prefix) |
| Status template (phases, progress %) | Issue checklist + labels |
| Bug numbering (BUG-{NNN} per feature) | GitHub Issue number only (#NNN) |
| Feature completion checklist → STATUS.md | Close GitHub Issue with completion comment |

Key commands the rewritten agent uses:
```bash
# Create implementation issue
gh issue create --title "{feature-id}: {description}" \
  --label "implementation,{phase}" \
  --body "SPARC docs: product/features/{id}/"

# Create bug issue (no BUG- prefix, just descriptive title)
gh issue create --title "{description}" \
  --label "bug,{phase}" \
  --body "Related feature: {feature-id}\nVersion: {version}"

# Update progress
gh issue comment {number} --body "Phase update: {status}"

# Close on completion
gh issue close {number} --comment "Completed in {version}. /reflexion recorded."

# List active work
gh issue list --label "implementation" --state open
gh issue list --label "bug" --state open
```

**5e. Files that change**

| File | Change | Effort |
|------|--------|--------|
| `.claude/agents/ndp/ndp-scrum-master.md` | Rewrite STATUS.md → GH Issue, bug files → GH Issue | High |
| `CLAUDE.md` | Remove STATUS.md and bugs/ from feature tree, add GH Issue convention | Low |
| `.claude/skills/ndp-github-workflow/SKILL.md` | Replace STATUS.md reference with GH Issue commands | Low |
| `.claude/agents/ndp/README.md` | Update scrum-master description | Low |
| `docs/TEAMBUILDER.md` | Update coordinator template, remove STATUS.md template | Medium |
| `.github/ISSUE_TEMPLATE/` | Create ndp-implementation.yml, ndp-bug.yml | Medium |

**5f. What does NOT change (go-forward only)**

All historical data stays as-is. No retroactive migration of completed features or closed bugs.

| Item | Decision | Count |
|------|----------|-------|
| Existing STATUS.md files | Leave untouched — historical records | 38 files |
| Existing bugs/ files | Leave untouched — historical records | 29 files |
| BUG-{NNN} in source code comments | Leave — code documentation | ~20 files |
| BUG-{NNN} in CHANGELOG | Leave — historical record | ~15 entries |
| BUG-{NNN} in SPARC docs | Leave — historical SPARC artifacts | ~60 files |
| Completed features | No STATUS.md changes, no GH Issue backfill | All closed features |
| SCOPE.md | Stays in-repo (human-written) | All features |
| SPARC directories | Stay in-repo (spec/pseudo/arch/refine/complete) | All features |

**5g. Migration approach**

**Go-forward only.** New protocol applies to the NEXT feature/bug. No backfilling.

1. **New features**: GH Issue created at feature kickoff. No STATUS.md. SPARC planning artifacts stay in `product/features/`.
2. **New bugs**: GH Issue with `bug` label. No `bugs/` directory. Complex bugs that need design work get SPARC subdirs in `product/features/`, linked from the GH Issue.
3. **Active in-flight work (ops-004, air-017, #16)**: User manages closures manually. These span the old and new conventions and will never fully align with the new protocol. Agents should not attempt to retroactively restructure them.

### Design Decisions (DECIDED)

1. **Bug numbering**: **GitHub Issue numbers only.** Drop BUG-{NNN} notation for all new bugs. Use `#17`, `#18`, etc. BUG-{NNN} was a workaround for lack of issue tracking — no longer needed.

2. **SPARC bug docs**: **Yes, keep for complex bugs.** When a bug needs design work (root cause analysis, architecture changes), create SPARC subdirs in `product/features/` and link from the GH Issue. The issue is the tracker; the in-repo docs are the design artifacts. Same split as features.

3. **Active feature migration**: **No.** ops-004/air-017/#16 are manually managed by the user. They cross the old/new boundary and will close on their own timeline. Agents treat them as legacy — read but don't restructure.

---

## Workstream 6: Specification Alignment Check

**Goal**: Every planning swarm produces a vision alignment report before the user approves specs. Surface where corners are cut, where scope drifts from the roadmap, and where variances need explicit approval.

### The Problem

Deep specification work makes it easy to lose sight of the product vision. Planning agents optimize for the feature at hand — they don't step back to ask "does this move us toward v1.2 Discovery?" or "does this add cloud dependency that contradicts our edge-only architecture?" The user catches these misalignments manually, but that requires reading 15-20K lines of SPARC artifacts and holding the full vision in their head simultaneously.

### Vision Reference Document (user-owned, always current)

All alignment checks reference a single document: **`product/vision/ALIGNMENT-CRITERIA.md`**

This document is **owned by the user** and can be edited/updated at any time. It consolidates the checkable alignment criteria from the broader vision documents into one place. When the vision evolves, update this file — all agents and skills immediately align to the new criteria.

The alignment criteria document references (but does not duplicate) the full vision documents:

| Document | Role |
|----------|------|
| **`product/vision/ALIGNMENT-CRITERIA.md`** | **Primary reference for all alignment checks** — user-owned, editable |
| `product/vision/EDGE-INTELLIGENCE-PLATFORM.md` | Master product vision (background reading) |
| `product/vision/ROADMAP-TO-V2.md` | Version scoping (background reading) |
| `product/INTEGRATION_FIRST_MANDATE.md` | Integration rules (background reading) |
| Feature's own `SCOPE.md` | What the user actually asked for |

The alignment criteria doc is ~200 lines — small enough for any agent to hold alongside SPARC artifacts.

### Approach: Agent + Skill + Protocol Integration

Three pieces, each serving a different use case:

**6a. `ndp-vision-guardian` agent definition**

New agent at `.claude/agents/ndp/ndp-vision-guardian.md`:

```yaml
---
name: ndp-vision-guardian
type: reviewer
scope: broad
description: Evaluates SPARC specifications against product vision, roadmap, and architectural constraints. Produces alignment reports surfacing scope drift, corner-cutting, and variances requiring user approval.
capabilities:
  - vision_alignment
  - scope_drift_detection
  - roadmap_fit_analysis
  - constraint_verification
---
```

**What the agent does:**
1. Reads the vision corpus (5 documents, ~730 lines total)
2. Reads the SPARC artifacts being evaluated (specification, pseudocode, architecture)
3. Reads the feature's SCOPE.md (what was asked for)
4. Produces a structured alignment report

**What the agent does NOT do:**
- Make architectural decisions (that's the user's role)
- Modify specs (read-only reviewer)
- Block the planning swarm (produces a report, doesn't gate)

**6b. Alignment Report format**

```markdown
# Vision Alignment Report: {feature-id}

## Summary
{1-2 sentence assessment}

## Roadmap Fit
- **Target version**: v1.X (based on roadmap analysis)
- **In scope for this version**: YES/NO/PARTIAL
- **Dependencies on future versions**: {list or "none"}
- **Advancement toward proof point**: {which v1.X proof point this enables}

## Vision Principles Check

| Principle | Status | Notes |
|-----------|--------|-------|
| Edge-only (no cloud dependency) | PASS/FAIL/WARN | {detail} |
| Config-driven (declarative, not imperative) | PASS/FAIL/WARN | {detail} |
| Domain-portable (adapter pattern) | PASS/FAIL/WARN | {detail} |
| Resource-constrained (Pi 5 16GB) | PASS/FAIL/WARN | {detail} |
| Integration-first (extend, don't replace) | PASS/FAIL/WARN | {detail} |
| Privacy by architecture (local only) | PASS/FAIL/WARN | {detail} |
| Self-learning capable (improves over time) | PASS/FAIL/WARN | N/A for infra features |

## Scope vs SCOPE.md
- **Matches user intent**: YES/PARTIAL/NO
- **Additions beyond scope**: {list of things spec added that weren't asked for}
- **Omissions from scope**: {list of things user asked for that spec missed}

## Corners Cut
{List of simplifications, shortcuts, or deferred items in the spec. For each:}
- **What**: {description}
- **Why it matters**: {impact on vision/roadmap}
- **Recommendation**: Accept / Revisit / Flag for future version

## Variances Requiring Approval
{Items where the spec deviates from vision/roadmap that need explicit user sign-off:}
- **Variance**: {description}
- **Vision conflict**: {which principle or roadmap item it conflicts with}
- **Justification**: {why the spec chose this approach}
- **Decision needed**: {specific question for the user}

## Technical Constraint Verification
- ARM64 compatible: YES/NO
- No DuckDB: YES/NO
- No Polars: YES/NO
- Config-driven (no hardcoded values): YES/NO
- Extends existing code (no parallel systems): YES/NO

## Overall Assessment
**ALIGNED / MOSTLY ALIGNED / NEEDS REVIEW / MISALIGNED**
```

**6c. `/align` skill**

User-invokable skill at `.claude/skills/align/SKILL.md`:

```markdown
---
name: "align"
description: "Check SPARC specification alignment against product vision, roadmap, and constraints. Run after planning or anytime to evaluate a feature's direction."
---

# Align — Vision Alignment Check

## Usage
Run after SPARC planning to evaluate specification alignment, or anytime to check a feature's direction.

## Steps

1. Read the vision corpus:
   - product/vision/EDGE-INTELLIGENCE-PLATFORM.md
   - product/vision/ROADMAP-TO-V2.md
   - product/INTEGRATION_FIRST_MANDATE.md
   - CLAUDE.md (constraints section)

2. Read the feature being evaluated:
   - product/features/{feature-id}/SCOPE.md
   - product/features/{feature-id}/specification/ (all files)
   - product/features/{feature-id}/architecture/ (if exists)
   - product/features/{feature-id}/pseudocode/ (if exists)

3. Produce the alignment report (see ndp-vision-guardian agent for format).

4. Save the report to:
   product/features/{feature-id}/ALIGNMENT-REPORT.md

5. Present the Variances Requiring Approval section to the user for decision.
```

**6d. Integration into planning protocol**

In `.claude/rules/planning-protocol.md`, the vision guardian runs as the **final step before the implementation brief**:

```
Planning Swarm Steps:
  1. /get-pattern
  2. claude-flow swarm init
  3. Spawn planning agents (spec, pseudocode, architecture)
  4. Agents produce SPARC artifacts
  5. ** Spawn ndp-vision-guardian agent **
     - Reads vision corpus + SPARC artifacts
     - Produces ALIGNMENT-REPORT.md
     - Presents variances to user for approval
  6. Generate IMPLEMENTATION-BRIEF.md (includes alignment status)
```

The vision guardian runs AFTER the planning agents complete but BEFORE the brief is generated. This means:
- Specs are complete enough to evaluate
- User sees the alignment report before approving implementation
- The brief can note "alignment: MOSTLY ALIGNED — variance X approved by user"

### Why Agent + Skill (not just one)

| Mechanism | When | Why |
|-----------|------|-----|
| **Agent** (ndp-vision-guardian) | Spawned automatically in planning swarm | Runs as part of standard flow — can't be skipped |
| **Skill** (/align) | User invokes on demand | Ad-hoc checks: "did this bug fix drift from vision?" or re-check after spec revisions |

The agent ensures every planning swarm includes alignment. The skill gives the user a manual lever for anytime checks.

### Context Window Impact

The vision guardian agent reads ~730 lines of vision docs + the SPARC artifacts for one feature. For a typical feature:
- Vision corpus: ~730 lines
- Specification: ~500-1200 lines
- Architecture: ~500-1000 lines
- Pseudocode: ~500-2000 lines
- SCOPE.md: ~50-100 lines

Total: ~2300-5000 lines. Well within a single agent's context window. Runs as a separate Task agent, so it doesn't consume the coordinator's context at all.

### What this catches (examples from insights friction data)

| Past Friction | What Alignment Check Would Flag |
|---------------|--------------------------------------|
| DuckDB used for Gold layer (deprecated) | FAIL on "No DuckDB" constraint |
| Hardcoded retention periods | WARN on "Config-driven" principle |
| v1.2.0 version references | WARN on "Roadmap Fit" — v1.2 not in scope yet |
| Building infrastructure that already exists | FAIL on "Integration-first" principle |
| Polars dependency (pre-air-018) | FAIL on "Resource-constrained" — OOM on Pi |
| Cloud-dependent feature proposals | FAIL on "Edge-only" principle |

---

## Implementation Priority

| Priority | Item | Effort | Impact |
|----------|------|--------|--------|
| **P0** | 1a: Split planning-protocol.md / implementation-protocol.md | 45 min | Different rules for different work |
| **P0** | 1b: Rewrite prompt-check.sh with plan/impl detection | 30 min | Enforces correct protocol |
| **P0** | 2a: Create /validate skill (3-tier with deploy.sh integration) | 45 min | Catches failures before user sees them |
| **P0** | 4a: Implementation Brief template + planning protocol rule | 20 min | Solves spec volume context killer |
| **P1** | 5a: GitHub Issue templates | 20 min | Enables new tracking convention |
| **P1** | 5b: Label scheme | 10 min | Organizes issues |
| **P1** | 5d: ndp-scrum-master agent rewrite | 60 min | Heaviest single change |
| **P1** | 5e: CLAUDE.md + supporting file updates | 20 min | Consistency |
| **P1** | 2b: Wire validation into implementation-protocol | 10 min | Makes validation structural |
| **P1** | 2c: Integration env trigger detection | 15 min | Right-sizes validation effort |
| **P1** | 1c: Post-Task reflexion reminder hook | 5 min | Prevents forgetting reflexion |
| **P1** | 4b: Agent context budget rules | 10 min | Reduces context waste per agent |
| **P1** | 4c: Validation iteration cap + cargo truncation | 10 min | Stops error output context burn |
| **P0** | 6a: ndp-vision-guardian agent definition | 30 min | Catches vision drift before user reviews specs |
| **P0** | 6b: /align skill | 15 min | On-demand alignment checks |
| **P1** | 6c: Wire vision guardian into planning-protocol.md | 10 min | Automatic alignment in every planning swarm |
| **P1** | 6d: Alignment report template + IMPLEMENTATION-BRIEF link | 10 min | Structured output format |
| **P2** | 5f: AgentDB pattern for GH Issue workflow convention | 10 min | Teaches future agents the new convention |
| **P2** | 3a: CLAUDE.md pattern workflow line | 5 min | Reinforces hook injection |
| **P2** | 3b: Stop hook reflexion check | 5 min | Last-chance reminder |
| **P2** | 3c: Pattern maintenance cadence | 5 min | Long-term knowledge hygiene |
| **P2** | 5g: AgentDB patterns update (deprecate STATUS.md patterns, store GH Issue convention) | 10 min | Knowledge hygiene |

**Total estimated effort**: ~7 hours to implement all items.
- WS1 (protocols + hooks): ~1.5h
- WS2 (validation): ~1.5h
- WS3 (patterns): ~15min
- WS4 (context): ~30min
- WS5 (GH Issues): ~2h
- WS6 (vision alignment): ~1h

---

## Success Metrics

Track across next 20 sessions:

| Metric | Current Baseline | Target |
|--------|-----------------|--------|
| Wrong version references | ~3 sessions with errors | 0 |
| Swarm protocol bypass | ~4 sessions | 0 |
| Planning agent making code changes | ~2 sessions | 0 |
| Sessions without reflexion | High (unknown) | 0 |
| Test failures in presented results | Multiple | 0 |
| Context window exhaustion | 1+ sessions | 0 |
| Validation uses integration env (when qualifying) | 0 | 100% |
| Patterns stored per feature | ~2 | 3-5 |
| Implementation agents reading full spec tree | Common | 0 (brief only) |
| Cargo error iterations > 2 | Multiple | 0 |
| Implementation tracked via GH Issue | 0% | 100% (new features) |
| Bugs tracked via GH Issue | ~30% (only cross-feature) | 100% |
| Planning swarms with alignment report | 0% | 100% |
| Vision variances caught before implementation | 0 (caught by user mid-session) | Caught in alignment report |

---

## What This Does NOT Change

- **Architectural control**: User reviews all architecture decisions
- **Production access**: Dev space cannot reach Pi. Deploy validation stays relay-based
- **CI/CD**: No pipeline changes. Docker builds on Pi, git as transport
- **CLAUDE.md size**: Stays lean. New rules go in `.claude/rules/` files, not CLAUDE.md
- **SPARC planning in-repo**: Specification/pseudocode/architecture docs stay in product/features/
- **Historical data**: Existing STATUS.md and bugs/ files frozen, not deleted
