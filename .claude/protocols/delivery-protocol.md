# Delivery Protocol

Session 2 protocol. Read by the **Delivery Leader** (ndp-scrum-master).

Triggers on: implement, TDD, build, code, fix, refactor, migrate, test, deliver, SPARC P/R/C phases.

---

## Execution Model

The primary agent spawns `ndp-scrum-master` as the Delivery Leader. The Delivery Leader reads the IMPLEMENTATION-BRIEF.md (the handoff from Session 1), then runs three stages sequentially — component design (3a), implementation (3b), testing (3c) — each with a validation gate. Gates that pass proceed automatically. Gates that fail stop and return to the human.

```
Primary Agent                    Delivery Leader                  Worker Agents
─────────────                    ───────────────                  ─────────────
get-pattern
read brief + GH Issue
spawn Delivery Leader ────────►  read protocol + brief
                                 swarm init
                                 TaskCreate (all tasks)
                                 seed shared memory
                                 ┌─ Stage 3a ──────────────────────────────────┐
                                 │  spawn pseudocode + tester ──► component designs
                                 │  spawn validator (Gate 3a) ──► PASS/FAIL    │
                                 │  PASS → continue   FAIL → rework or stop   │
                                 └─────────────────────────────────────────────┘
                                 ┌─ Stage 3b ──────────────────────────────────┐
                                 │  spawn coding agents ────────► implemented code
                                 │  spawn validator (Gate 3b) ──► PASS/FAIL    │
                                 │  PASS → continue   FAIL → rework or stop   │
                                 └─────────────────────────────────────────────┘
                                 ┌─ Stage 3c ──────────────────────────────────┐
                                 │  spawn tester (execution) ───► test results
                                 │  spawn validator (Gate 3c) ──► PASS/FAIL    │
                                 │  PASS → deliver   FAIL → rework or stop    │
                                 └─────────────────────────────────────────────┘
                                 Phase 4: GH Issue update
                                 learning gate
◄──────────────────────────────  return summary
reflexion + save-pattern
```

Do NOT use TeamCreate — delivery swarms are coordinator-driven via Task tool spawn-and-wait.

### Concurrency Rules

Each message batches ALL related operations of the same type:

- ALWAYS batch ALL TaskCreate calls in ONE message
- ALWAYS spawn ALL agents within a stage in ONE message via Task tool
- ALWAYS batch ALL file reads/writes/edits in ONE message
- ALWAYS batch ALL Bash commands in ONE message
- ALWAYS batch ALL memory store/retrieve operations in ONE message

### Agent Rules

- Agents return: file paths + test pass/fail + issues (NOT file contents)
- Max 2 rework iterations per gate to protect context window
- Cargo output truncated to first error + summary line
- GH Issue is the tracking artifact — all progress updates go via `gh issue comment`

---

## Flow: 4 Phases

### Phase 1: Preparation (primary agent)

Pattern search and brief reading happen BEFORE spawning the Delivery Leader.

```
/get-pattern — search AgentDB for relevant patterns
```

Note which pattern IDs were returned for reflexion later.

**Read the Implementation Brief.** The brief is the handoff from Session 1. It contains:
- Component Map (which components, mapped to pseudocode + test-plan file slots)
- Acceptance Map reference
- GH Issue number
- Paths to three source documents
- Resolved Decisions table with ADR pattern IDs
- Files to create/modify
- Constraints and scope exclusions

Read from `product/features/{feature-id}/IMPLEMENTATION-BRIEF.md` or from the GH Issue body (`gh issue view <N> --json body`).

If no brief exists, ask the user: "No implementation brief found. Has Session 1 (design) been completed and approved?"

### Phase 2: Delegation (primary agent)

Spawn `ndp-scrum-master` as the Delivery Leader. ONE Task call.

```
Task(
  subagent_type: "ndp-scrum-master",
  prompt: "You are the Delivery Leader for {feature-id}.

    Read the delivery protocol: .claude/protocols/delivery-protocol.md
    Read the brief: product/features/{feature-id}/IMPLEMENTATION-BRIEF.md
    GH Issue: #{N}

    Pattern IDs from get-pattern: {list IDs}
    Feature namespace: {feature-id}

    Three source documents (paths in the brief):
    - architecture/ARCHITECTURE.md
    - specification/SPECIFICATION.md
    - RISK-TEST-STRATEGY.md

    Execute: Stage 3a (component design + Gate 3a) → Stage 3b (implementation +
    Gate 3b) → Stage 3c (testing + Gate 3c) → Phase 4 (delivery).
    Auto-proceed through gates on pass. Stop on failure.
    Return: files changed, test results, gate results, issues encountered."
)
```

After spawning: tell the user that the Delivery Leader is running, then STOP.

### Phase 3: Delivery Execution (Delivery Leader)

The Delivery Leader executes three stages sequentially. Each stage follows the same pattern: initialize → spawn workers → validate → proceed or stop.

#### Step 3.0: Initialize + Define All Tasks (ONE message)

Batch all initialization and task creation in ONE message:

```
# 1. Register all planned agents
mcp__claude-flow__agent_spawn(agentId: "{feature}-agent-1-pseudo", agentType: "ndp-pseudocode")
mcp__claude-flow__agent_spawn(agentId: "{feature}-agent-2-testplan", agentType: "ndp-tester")
mcp__claude-flow__agent_spawn(agentId: "{feature}-agent-3-rustdev", agentType: "ndp-rust-dev")
# ... register all agents across all stages

# 2. Seed shared context
mcp__claude-flow__memory_store(
  key: "swarm/shared/{feature-id}-context",
  value: "{brief summary, goals, constraints, source doc paths}",
  namespace: "coordination",
  upsert: true
)

# 3. Define ALL tasks across all stages

# Stage 3a tasks
TaskCreate("Component pseudocode", "Per-component pseudocode from 3 source docs", "Writing pseudocode")
TaskCreate("Component test plans", "Per-component test plans from risk strategy", "Writing test plans")
TaskCreate("Gate 3a validation", "Validate component designs against source docs", "Validating designs")

# Stage 3b tasks (blocked by 3a)
TaskCreate("Code implementation", "Implement code from validated pseudocode", "Implementing code")
TaskCreate("Gate 3b validation", "Validate code against pseudocode + source docs", "Validating code")

# Stage 3c tasks (blocked by 3b)
TaskCreate("Test execution", "Execute all tests, produce risk coverage report", "Running tests")
TaskCreate("Gate 3c validation", "Final risk-based validation", "Validating coverage")

# Phase 4 task (blocked by 3c)
TaskCreate("Delivery", "Update GH Issue, learning gate, return results", "Delivering")
```

Set task dependencies with TaskUpdate after creation.

**Pre-spawn checklist** (verify before ANY Task call):
- [ ] Brief read (IMPLEMENTATION-BRIEF.md)
- [ ] Three source docs accessible (paths from brief)
- [ ] Agents registered (agent_spawn for each)
- [ ] Tasks defined with stage dependencies
- [ ] Shared context seeded
- [ ] `cargo build --workspace` passes (abort if broken workspace)

---

### Stage 3a: Component Design & Pseudocode

**Objective**: Decompose the approved design into per-component pseudocode and test plans.

#### 3a.1: Spawn Component Design Agents (parallel, ONE message)

Spawn in ONE message:

- **ndp-pseudocode**: Reads three source docs, produces per-component pseudocode:
  ```
  pseudocode/
    OVERVIEW.md           -- component interaction, data flow
    {component-1}.md      -- per-component pseudocode
    {component-2}.md
  ```

- **ndp-tester** (test plan mode): Reads three source docs (especially RISK-TEST-STRATEGY.md), produces per-component test plans:
  ```
  test-plan/
    OVERVIEW.md           -- overall test strategy rooted in risk strategy
    {component-1}.md      -- per-component test expectations
    {component-2}.md
  ```

Each agent prompt MUST include:
1. `Your agent ID: {feature}-agent-N-{role}`
2. Task description (2-3 sentences)
3. Paths to all three source documents
4. The Component Map from the brief (which components to produce)
5. Instruction to read source docs before producing output

Wait for BOTH agents to complete.

#### 3a.2: Gate 3a — Component Design Validation

Spawn `ndp-validator` with Gate 3a focus:

```
Task(
  subagent_type: "ndp-validator",
  prompt: "You are validating Gate 3a for {feature-id}.
    Your agent ID: {feature}-validator-3a
    Gate: 3a

    Source documents:
    - product/features/{id}/architecture/ARCHITECTURE.md
    - product/features/{id}/specification/SPECIFICATION.md
    - product/features/{id}/RISK-TEST-STRATEGY.md

    Artifacts to validate:
    - product/features/{id}/pseudocode/ (all files)
    - product/features/{id}/test-plan/ (all files)

    Gate 3a checks:
    1. Each component aligns with approved Architecture
    2. Pseudocode implements what Specification requires
    3. Component test plans address risks from RISK-TEST-STRATEGY.md
    4. Component interfaces consistent with architecture contracts

    Write report to: product/features/{id}/reports/gate-3a-report.md
    Return: PASS/FAIL, report path, issues."
)
```

#### 3a.3: Gate Decision

| Result | Action |
|--------|--------|
| PASS | Mark Stage 3a tasks complete. Proceed to Stage 3b. |
| Reworkable FAIL (iteration ≤ 2) | Spawn fix agents targeting specific issues. Re-run Gate 3a. |
| Reworkable FAIL (iteration > 2) | Stop. Return to human: "Gate 3a failed after 2 rework iterations." |
| Scope/Feasibility FAIL | Stop immediately. Return to human with recommendation. |

Post gate result to GH Issue:
```bash
gh issue comment <N> --body "## Gate 3a: {PASS|FAIL}
- Pseudocode components: {list}
- Test plan components: {list}
- Issues: {list or none}"
```

---

### Stage 3b: Code Implementation

**Objective**: Implement code from validated pseudocode and build test cases from test plans.

#### 3b.1: Spawn Coding Agents (parallel, ONE message)

Use the Component Map from the brief to determine which agents to spawn. Route each agent to its specific component files:

Each agent prompt MUST include:
1. `Your agent ID: {feature}-agent-N-{role}`
2. Task description (2-3 sentences)
3. Paths to component-specific SPARC artifacts:
   - `product/features/{id}/architecture/ARCHITECTURE.md`
   - `product/features/{id}/pseudocode/OVERVIEW.md`
   - `product/features/{id}/pseudocode/{component}.md`
   - `product/features/{id}/test-plan/OVERVIEW.md`
   - `product/features/{id}/test-plan/{component}.md`
4. Files to create/modify (from brief)
5. Instruction to retrieve relevant ADRs before implementing

**Agent spawn prompt template:**
```
Task(
  subagent_type: "{ndp-agent-type}",
  prompt: "You are implementing {subtask} for {feature-id}.
    Your agent ID: {feature-id}-agent-N-{role}

    Read these files before starting:
    - product/features/{id}/IMPLEMENTATION-BRIEF.md
    - product/features/{id}/architecture/ARCHITECTURE.md
    - product/features/{id}/pseudocode/OVERVIEW.md
    - product/features/{id}/pseudocode/{component}.md
    - product/features/{id}/test-plan/OVERVIEW.md
    - product/features/{id}/test-plan/{component}.md

    YOUR TASK: {description}
    Files to create/modify: {paths}

    RETURN FORMAT (required):
    1. Files modified: [paths]
    2. Tests: pass/fail
    3. Issues: [blockers]
    4. Pattern assessment: {ID: helped/didn't/irrelevant}
    5. Discoveries: [new approaches worth saving]"
)
```

Route ONLY the component-specific pseudocode and test-plan files each agent needs — not every file in the feature.

Agent types for implementation: `ndp-rust-dev`, `ndp-tester`, `ndp-timescale-dev`, `ndp-parquet-dev`, and other domain specialists as needed.

Wait for ALL coding agents to complete.

#### 3b.2: Drift Check

After agents return, check results against the brief:

| Check | Action |
|-------|--------|
| Files modified outside scope | Flag in summary |
| TODOs, stubs, `unimplemented!()` left | Spawn fix agent |
| Acceptance criteria missed | Spawn gap-fill agent |
| Test count decreased | Investigate before proceeding |

#### 3b.3: Gate 3b — Code Implementation Validation

Spawn `ndp-validator` with Gate 3b focus:

```
Task(
  subagent_type: "ndp-validator",
  prompt: "You are validating Gate 3b for {feature-id}.
    Your agent ID: {feature}-validator-3b
    Gate: 3b

    Source documents:
    - product/features/{id}/architecture/ARCHITECTURE.md
    - product/features/{id}/specification/SPECIFICATION.md

    Pseudocode (baseline for code match):
    - product/features/{id}/pseudocode/ (all files)

    Component test plans:
    - product/features/{id}/test-plan/ (all files)

    Gate 3b checks:
    1. Code matches validated pseudocode from Stage 3a
    2. Implementation aligns with approved Architecture
    3. Component interfaces implemented as specified
    4. Test cases match component test plans
    5. Compilation passes, no stubs or placeholders

    Write report to: product/features/{id}/reports/gate-3b-report.md
    Return: PASS/FAIL, report path, issues."
)
```

#### 3b.4: Gate Decision

Same logic as Gate 3a (pass → proceed, reworkable fail → iterate ≤ 2, scope fail → stop).

Post gate result to GH Issue.

---

### Stage 3c: Testing & Risk Validation

**Objective**: Execute all tests and prove that every risk identified in Phase 2 is mitigated by test coverage.

#### 3c.1: Spawn Testing Agents (ONE message)

Spawn `ndp-tester` in execution mode:

```
Task(
  subagent_type: "ndp-tester",
  prompt: "You are executing tests and validating risk coverage for {feature-id}.
    Your agent ID: {feature}-agent-N-tester-exec

    Read before starting:
    - product/features/{id}/RISK-TEST-STRATEGY.md
    - product/features/{id}/test-plan/OVERVIEW.md
    - product/features/{id}/test-plan/{component-1}.md
    - product/features/{id}/test-plan/{component-2}.md

    YOUR TASK:
    1. Execute all component-level tests (cargo test)
    2. Execute integration tests across components
    3. Execute feature-level tests mapped to RISK-TEST-STRATEGY.md
    4. For every risk in RISK-TEST-STRATEGY.md, verify corresponding test coverage exists
    5. Produce RISK-COVERAGE-REPORT.md:
       - Risk ID → test function(s) → pass/fail → coverage status
       - Any risks without test coverage flagged

    Write to: product/features/{id}/testing/RISK-COVERAGE-REPORT.md
    Return: test results summary, uncovered risks, report path."
)
```

If the feature qualifies for a testbed (touches SQL, containers, cross-layer data flow), also execute:
```bash
./tests/integration/run-testbed.sh feature --path product/features/{id}/testbed
```

Wait for testing to complete.

#### 3c.2: Gate 3c — Final Risk-Based Validation

Spawn `ndp-validator` with Gate 3c focus:

```
Task(
  subagent_type: "ndp-validator",
  prompt: "You are validating Gate 3c for {feature-id}.
    Your agent ID: {feature}-validator-3c
    Gate: 3c

    Source documents:
    - product/features/{id}/RISK-TEST-STRATEGY.md
    - product/features/{id}/specification/SPECIFICATION.md
    - product/features/{id}/architecture/ARCHITECTURE.md

    Test output:
    - product/features/{id}/testing/RISK-COVERAGE-REPORT.md

    Gate 3c checks:
    1. Test results prove identified risks are mitigated
    2. Test coverage matches Risk-Based Test Strategy
    3. No risks from Phase 2 lack test coverage
    4. Delivered code matches approved Specification
    5. System architecture matches approved Architecture

    Write report to: product/features/{id}/reports/gate-3c-report.md
    Return: PASS/FAIL, report path, issues."
)
```

#### 3c.3: Gate Decision

Same logic as Gates 3a/3b. If Gate 3c passes, proceed to Phase 4.

Post gate result to GH Issue.

---

### Phase 4: Delivery

**Prerequisite**: All three gates (3a, 3b, 3c) passed.

#### 4a: Per-AC Acceptance Check

Read `product/features/{id}/ACCEPTANCE-MAP.md`. For each AC:
1. Run the verification method (test, file-check, grep, shell)
2. Update status: PENDING → PASS or FAIL
3. Report: "X/Y ACs verified"

If any AC fails, spawn a fix agent (within the 2-iteration budget) or flag for human.

#### 4b: GH Issue Final Update

```bash
gh issue comment <N> --body "## Delivery Complete
- Gate 3a: PASS
- Gate 3b: PASS
- Gate 3c: PASS
- ACs verified: X/Y
- Files changed: [list]
- Tests: X passed, Y new
- Risk coverage: all risks mitigated
- Issues: [if any]"
```

#### 4c: Learning Gate

1. Review agent return messages for pattern assessment data
2. Record reflexion entries for each pattern used across all stages
3. Save any new patterns from agent discoveries
4. Include learning summary in return to primary agent

#### 4d: Return to Primary Agent

Return:
- Files created/modified (paths only)
- Test results (pass/fail count)
- Gate results (3a: PASS, 3b: PASS, 3c: PASS)
- AC verification count
- GH Issue URL + update confirmation
- Issues or drift encountered
- Learning summary

### Phase 5: Completion (primary agent)

After the Delivery Leader returns:

1. Review the summary (files changed, tests, gate results)
2. Record learning:
   ```
   /reflexion — record pattern effectiveness (per pattern used, referencing IDs)
   /save-pattern — store new discoveries (if any)
   ```

---

## Quick Reference: Message Map

```
PRIMARY AGENT:
  Message 1:  /get-pattern + Read IMPLEMENTATION-BRIEF.md
  Message 2:  Task(ndp-scrum-master as Delivery Leader)
  ...wait...
  Message 3:  Review results + /reflexion + /save-pattern

DELIVERY LEADER (internal):
  Step 3.0:   MCP: agent_spawn (all) + memory_store + TaskCreate (all tasks)

  Stage 3a:
    Step 3a.1:  Task(ndp-pseudocode) + Task(ndp-tester) — parallel
                ...wait...
    Step 3a.2:  Task(ndp-validator, gate=3a)
    Step 3a.3:  Gate decision → proceed or stop

  Stage 3b:
    Step 3b.1:  Task(ndp-rust-dev) + Task(specialists) — parallel
                ...wait...
    Step 3b.2:  Drift check
    Step 3b.3:  Task(ndp-validator, gate=3b)
    Step 3b.4:  Gate decision → proceed or stop

  Stage 3c:
    Step 3c.1:  Task(ndp-tester, execution mode)
                ...wait...
    Step 3c.2:  Task(ndp-validator, gate=3c)
    Step 3c.3:  Gate decision → proceed or stop

  Phase 4:
    Step 4a:    AC acceptance check
    Step 4b:    gh issue comment (final)
    Step 4c:    Learning gate
    Step 4d:    Return summary
```

---

## Two-Tier Escalation Model

At every gate, failures are classified into two tiers:

### Tier 1: Reworkable Failures

Component design doesn't match spec, code doesn't match pseudocode, test gaps exist, compilation fails, stubs found.

**Action**: Loop back to previous stage's agents for correction. Maximum 2 iterations per gate. After 2 failed iterations, escalate to Tier 2.

### Tier 2: Scope/Feasibility Failures

Original scope was wrong, technology doesn't work as assumed, architecture can't support a requirement, risk was missed entirely.

**Action**: Stop the session immediately. Return to the human with:
1. What failed and why
2. Which source document is affected
3. Recommendation: adjust scope (Phase 1), revise design (Phase 2), or approve modified approach
4. GH Issue updated with the failure

---

## Agent Context Budget

Each spawned agent should receive:
- Task description (2-3 sentences)
- Namespace for claude-flow memory coordination
- Specific file paths to read and modify
- Component-specific SPARC artifact paths (routed from Component Map)
- Relevant AgentDB pattern IDs (not full pattern text)

Do NOT paste: full spec documents, full source files, full cargo output, or implementation brief contents into agent prompts. Agents read files themselves. Route ONLY the component-specific paths each agent needs.

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

## Three Memory Systems

| System | Tool | Persistence | Purpose |
|--------|------|-------------|---------|
| **AgentDB** | `/get-pattern`, `/save-pattern`, `/reflexion` | Permanent | Architecture, conventions, procedures |
| **Swarm Memory** | `memory_store`/`memory_retrieve` with `namespace: "coordination"` | Session | Agent status, progress, results, shared context |
| **Hive Metadata** | `hive-mind_init`/`hive-mind_join`/`hive-mind_status` | Session | Agent registration, swarm topology tracking (optional) |

Rule: Useful 6 months from now → AgentDB. Swarm coordination → `memory_store` with `namespace: "coordination"`. Hive metadata → registration only.
