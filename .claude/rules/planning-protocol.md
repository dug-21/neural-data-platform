---
paths:
  - "product/features/**/*"
  - "CLAUDE.md"
---

# Planning Swarm Protocol

Triggers on: specification, pseudocode, architecture, design, research, scope, roadmap, SPARC S/P/A phases.

---

## Execution Model

Planning swarms use **coordinator delegation**: the primary agent spawns `ndp-scrum-master` as the single coordinator. The scrum-master spawns planning agents, runs vision alignment, generates the implementation brief, and creates the GH Issue.

```
Primary Agent                    ndp-scrum-master                 Planning Agents
─────────────                    ────────────────                 ───────────────
get-pattern
read SCOPE.md
spawn scrum-master ──────────►   read protocol + SCOPE.md
                                 swarm init
                                 TaskCreate (all tasks)
                                 seed shared memory
                                 spawn agents ────────────────►  produce SPARC artifacts
                                 ◄────────────────────────────── return artifact paths
                                 spawn vision guardian
                                 generate IMPLEMENTATION-BRIEF.md
                                 gh issue create
◄──────────────────────────────  return summary
present variances to user
reflexion
save-pattern
```

Do NOT use TeamCreate. Planning swarms are coordinator-driven via Task tool spawn-and-wait.

### Concurrency Rules

Each message batches ALL related operations of the same type:

- ALWAYS batch ALL TaskCreate calls in ONE message
- ALWAYS spawn ALL agents in ONE message via Task tool
- ALWAYS batch ALL file reads/writes/edits in ONE message
- ALWAYS batch ALL Bash commands in ONE message
- ALWAYS batch ALL memory store/retrieve operations in ONE message

### Planning Rules

- Output goes to `product/features/{feature-id}/{phase}/` ONLY
- NO code changes. NO file edits outside `product/features/`
- NO launching implementation agents (ndp-rust-dev, ndp-tester)
- Each planning agent gets: SCOPE.md + relevant existing SPARC artifacts + relevant AgentDB patterns
- Agents return: artifact paths + key decisions + open questions (NOT full file contents)

---

## Flow: 4 Phases

### Phase 1: Preparation (primary agent)

Pattern search and scope reading happen BEFORE spawning the coordinator.

```
/get-pattern — search AgentDB for relevant patterns
```

Note which pattern IDs were returned for reflexion later.

Read `product/features/{feature-id}/SCOPE.md` — this defines what the planning swarm must produce.

#### Scope Pre-Check (REQUIRED)

Before spawning the coordinator, perform a quick alignment scan of SCOPE.md against `product/vision/ALIGNMENT-CRITERIA.md`. Check the 7 alignment principles at a surface level:

1. Does the scope imply cloud-only dependencies? (Edge-Only violation)
2. Does the scope require hardcoded values? (Config-Driven violation)
3. Does the scope introduce banned dependencies? (Resource-Constrained violation)
4. Does the scope target the correct version? (Version discipline)

If any red flags are found, present them to the user BEFORE spawning the planning swarm. This prevents wasting a full planning cycle on a misaligned scope.

### Phase 2: Delegation (primary agent)

Spawn `ndp-scrum-master` with the full context needed to run the planning swarm. ONE Task call.

```
Task(
  subagent_type: "ndp-scrum-master",
  prompt: "You are coordinating the planning swarm for {feature-id}.

    Read the planning protocol: .claude/rules/planning-protocol.md
    Read the scope: product/features/{feature-id}/SCOPE.md

    Pattern IDs from get-pattern: {list IDs}
    Feature namespace: {feature-id}

    Execute the planning swarm: init → define tasks → spawn planning agents →
    vision alignment → generate brief → create GH Issue.
    Return: artifacts produced, key decisions, open questions, GH Issue URL,
    and any vision alignment variances requiring user approval."
)
```

After spawning: tell the user that the scrum-master is coordinating, then STOP.

### Phase 3: Swarm Execution (ndp-scrum-master)

The scrum-master executes the following steps autonomously.

#### Step 3a: Initialize Coordination Layer (MCP)

Use MCP tools for coordination, Task tool for agents. Do NOT use `claude-flow swarm init` CLI (it is cosmetic).

```
Use ToolSearch to find "claude-flow hive" tools, then call:

# 1. Create the hive
mcp__claude-flow__hive-mind_init(
  topology: "hierarchical",
  queenId: "planning-lead"
)

# 2. Register each planned agent in BOTH stores (repeat per agent)
mcp__claude-flow__agent_spawn(agentId: "agent-1-spec", agentType: "specification")
mcp__claude-flow__hive-mind_join(agentId: "agent-1-spec", role: "worker")

mcp__claude-flow__agent_spawn(agentId: "agent-2-arch", agentType: "ndp-architect")
mcp__claude-flow__hive-mind_join(agentId: "agent-2-arch", role: "specialist")

# ... register all agents before spawning any Task

# 3. Seed shared context (agents read this via memory_retrieve)
mcp__claude-flow__memory_store(
  key: "swarm/shared/{feature-id}-context",
  value: "{scope summary, goals, constraints, pattern IDs}",
  namespace: "coordination"
)
```

This creates `.claude-flow/hive-mind/state.json` with workers list and shared memory. Agents registered via `agent_spawn` + `hive-mind_join` are visible in `hive-mind_status` AND trackable via `agent_update`.

#### Step 3b: Definition (batch with Step 3a)

Define ALL tasks in the SAME message as Step 3a. Batch all TaskCreate calls with the MCP calls above:

```
TaskCreate("Specification artifact", "Produce SPECIFICATION.md for {feature}", "Writing specification")
TaskCreate("Task decomposition", "Produce TASK-DECOMPOSITION.md for {feature}", "Decomposing tasks")
TaskCreate("Architecture ADRs", "Produce ARCHITECTURE.md for {feature}", "Designing architecture")
TaskCreate("Pseudocode", "Produce PSEUDOCODE.md for {feature}", "Writing pseudocode")
TaskCreate("Vision alignment", "Produce ALIGNMENT-REPORT.md for {feature}", "Checking alignment")
TaskCreate("Implementation brief", "Produce IMPLEMENTATION-BRIEF.md for {feature}", "Generating brief")
TaskCreate("GH Issue creation", "Create GH Issue from brief", "Creating GH Issue")
```

Set task dependencies with TaskUpdate after creation.

#### Step 3c: Agent Spawning

Spawn ALL planning agents in ONE message (parallel).

**Pre-spawn checklist**:
- [ ] hive-mind_init ran
- [ ] Agents registered (agent_spawn + hive-mind_join for each)
- [ ] Tasks defined
- [ ] Shared context seeded (memory_store key="swarm/shared/{feature}-context")
- [ ] SCOPE.md read
- [ ] hive-mind_status shows all agents in workers array

Agent types for planning: `ndp-architect`, `specification`, `pseudocode`

Do NOT spawn: `ndp-rust-dev`, `ndp-tester`, `coder`, `sparc-coder`.

Each agent prompt MUST include:
1. `Your agent ID: {feature}-agent-N-{role}` — activates the Swarm Coordination block in agent definitions
2. Task description (2-3 sentences)
3. Specific SPARC phase to produce
4. The SCOPE.md path

**Architecture agent (ndp-architect) MUST produce individual ADRs** in `product/features/{feature-id}/architecture/ARCHITECTURE.md` using this format:

```markdown
## ADR-NNN: {Title}

### Context
{Why this decision is needed — the forces at play}

### Decision
{What was decided — concrete implementation approach with code examples}

### Consequences
{Tradeoffs — what this enables, what it costs, what it rules out}
```

Each ADR must cover a distinct architectural choice (not a grab-bag). Good ADR scoping: one decision per ADR, with cross-references between related ADRs.

#### Step 3d: Vision Alignment

After planning agents complete, spawn `ndp-vision-guardian`:

```
"Read product/vision/ALIGNMENT-CRITERIA.md and the SPARC artifacts at
 product/features/{feature-id}/. Produce ALIGNMENT-REPORT.md.
 Flag any variances requiring user approval."
```

Save to `product/features/{feature-id}/ALIGNMENT-REPORT.md`.

Include variances in the return summary. The primary agent will present them to the user.

#### Step 3e: Store ADRs in AgentDB via /save-pattern (permanent knowledge)

After the architecture agent completes, store each ADR as a permanent AgentDB pattern using `/save-pattern`. This is how implementation agents later access architectural decisions via `/get-pattern`.

For EACH `## ADR-NNN:` in the ARCHITECTURE.md, use `/save-pattern`:

```
taskType: "adr:{feature-id}-{nnn}"
approach: "{full ADR text verbatim — Context + Decision + Consequences}"
successRate: 1.0
tags: ["adr", "{feature-id}", "architecture", "{title-slug}"]
```

The `/save-pattern` skill handles duplicate checking, embedding generation, and storage. See that skill for best practices.

Record the returned pattern IDs — they go into the IMPLEMENTATION-BRIEF.md's Resolved Decisions table so `/spec-compile` can reference them in the Level-1 summary.

#### Step 3f: Generate Planning Deliverables

Produce the following deliverables:

**1. ACCEPTANCE-MAP.md** at `product/features/{feature-id}/ACCEPTANCE-MAP.md`:

```markdown
# {feature-id} Acceptance Criteria Map

| AC-ID | Description | Verification Method | Verification Detail | Status |
|-------|-------------|--------------------|--------------------|--------|
| AC-01 | Description from SCOPE.md | test/manual/file-check/grep/shell | Specific verification command or procedure | PENDING |
```

Verification method types: `test` (cargo test function), `manual` (human check), `file-check` (file exists), `grep` (content match), `shell` (run command, check exit code). Every AC from SCOPE.md must appear.

**2. LAUNCH-PROMPT.md** at `product/features/{feature-id}/LAUNCH-PROMPT.md`:

```markdown
# Implementation Launch Prompt: {feature-id}

## Proposed Prompt
> Implement {feature-id}: {title}
> GitHub Issue: #{N}
> Brief: product/features/{id}/IMPLEMENTATION-BRIEF.md
> Pattern IDs from planning: {list}
> Constraints: {key constraints}
> Wave structure: {summary}

## Reminders for User
- Review ALIGNMENT-REPORT.md for any variances
- Verify acceptance criteria in SCOPE.md

## Gotchas Discovered During Planning
- {gotcha 1}
```

**3. IMPLEMENTATION-BRIEF.md** at `product/features/{feature-id}/IMPLEMENTATION-BRIEF.md` (200-400 lines):

- **SPARC artifact links table** (MUST include):
  ```
  | Artifact | Path |
  |----------|------|
  | Scope | product/features/{feature-id}/SCOPE.md |
  | Specification | product/features/{feature-id}/specification/SPECIFICATION.md |
  | Task Decomposition | product/features/{feature-id}/specification/TASK-DECOMPOSITION.md |
  | Architecture (ADRs) | product/features/{feature-id}/architecture/ARCHITECTURE.md |
  | Pseudocode | product/features/{feature-id}/pseudocode/PSEUDOCODE.md |
  | Alignment Report | product/features/{feature-id}/ALIGNMENT-REPORT.md |
  ```
- Goal (2-3 sentences — the full objective, not a 1-liner)
- Resolved Decisions table: `| Decision | Resolution | Source | Pattern ID |` — include the AgentDB pattern ID from Step 3e so spec-compile can reference it
- GitHub Issue link (added in Step 3g)
- Files to create/modify (paths + 1-line summaries)
- Data structures (actual Rust code)
- Function signatures (actual Rust code)
- Test expectations (unit + integration)
- Constraints (version, banned deps, ARM64, config-driven, no hardcoded DDL)
- Dependencies (crates, features)
- NOT in scope
- Alignment status (from ALIGNMENT-REPORT.md)

#### Step 3g: Create GitHub Issue

```bash
gh issue create \
  --title "[{feature-id}] {description}" \
  --label "implementation,{phase}" \
  --body "$(cat product/features/{feature-id}/IMPLEMENTATION-BRIEF.md)"
```

Then update SCOPE.md with the issue link:
```
Add `## Tracking\n\n{issue-url}` to SCOPE.md (if not already present)
```

#### Step 3h: Validate Planning Artifacts (spawn ndp-validator)

Spawn `ndp-validator` as a dedicated agent. Do NOT run validation inline.

```
Task(
  subagent_type: "ndp-validator",
  prompt: "You are validating the planning swarm for {feature-id}.

    Swarm type: planning
    Feature: {feature-id}

    Read your agent definition: .claude/agents/ndp/ndp-validator.md
    Run the full /validate-plan skill (5 checks).
    Write glass box report.
    Record ALL trust entries in AgentDB.
    Return: PASS/WARN/FAIL, report path, confidence score, issues."
)
```

The validator checks artifact existence, AC coverage, ADR pattern IDs, stale references, and internal consistency. See `.claude/agents/ndp/ndp-validator.md` for the full procedure.

**Do NOT proceed to Phase 4 until the validator returns.** If the validator returns FAIL, fix issues before returning to the primary agent.

### Phase 4: Completion (primary agent)

After ndp-scrum-master returns:

1. Review: artifacts produced, key decisions, open questions, GH Issue URL
2. Present vision alignment variances to user (if any require approval)
3. Record learning:
   ```
   /reflexion — record pattern effectiveness (per pattern used, referencing IDs)
   /save-pattern — store new discoveries (if any)
   ```

---

## Quick Reference: Message Map

```
PRIMARY AGENT:
  Message 1:  /get-pattern + Read SCOPE.md
  Message 2:  Task(ndp-scrum-master) — delegate planning swarm
  ...wait...
  Message 3:  Review results + present variances + /reflexion + /save-pattern

NDP-SCRUM-MASTER (internal):
  Step 3a:  MCP: hive-mind_init + agent_spawn + hive-mind_join (all agents) + memory_store seed
  Step 3b:  TaskCreate (batch ALL) — in SAME message as 3a
  Step 3c:  Task() spawn ALL planning agents (parallel, ONE message)
  Step 3d:  Task(ndp-vision-guardian) — alignment check
  Step 3e:  Store each ADR in AgentDB via agentdb_pattern_store (permanent)
  Step 3f:  Generate ACCEPTANCE-MAP.md + LAUNCH-PROMPT.md + IMPLEMENTATION-BRIEF.md
  Step 3g:  gh issue create + update SCOPE.md
  Step 3h:  Task(ndp-validator) — 5-check planning validation + trust recording
```

---

## Agent Context Budget

Each spawned planning agent should receive:
- Task description (2-3 sentences)
- Namespace for claude-flow memory coordination
- SCOPE.md path (agents read it themselves)
- Specific file paths to read
- Relevant AgentDB pattern IDs

Do NOT paste full spec documents, source files, or cargo output into planning agent prompts.

---

## Three Memory Systems

| System | Tool | Persistence | Purpose |
|--------|------|-------------|---------|
| **AgentDB** | `/get-pattern`, `/save-pattern`, `/reflexion` | Permanent | Architecture, conventions, procedures |
| **Swarm Memory** | `memory_store`/`memory_retrieve` with `namespace: "coordination"` | Session | Agent status, progress, results, shared context |
| **Hive Metadata** | `hive-mind_init`/`hive-mind_join`/`hive-mind_status` | Session | Agent registration, swarm topology tracking |

Rule: Useful 6 months from now → AgentDB. Swarm coordination → `memory_store` with `namespace: "coordination"`. Hive metadata → registration only.
