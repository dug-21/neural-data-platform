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

Use the tested swarm-run methodology — MCP tools for coordination, Task tool for agents. Do NOT use `claude-flow swarm init` CLI (it is cosmetic and creates no real state).

```
Use ToolSearch to find "claude-flow hive" tools, then call:

mcp__claude-flow__hive-mind_init(
  topology: "hierarchical",
  queenId: "planning-lead"
)
```

This creates `.claude-flow/hive-mind/state.json` for shared coordination state.

#### Step 3b: Definition + Coordination

Define ALL tasks via TaskCreate and seed shared memory via MCP — in ONE message.

```
TaskCreate("Specification artifact", "Produce SPECIFICATION.md for {feature}", "Writing specification")
TaskCreate("Task decomposition", "Produce TASK-DECOMPOSITION.md for {feature}", "Decomposing tasks")
TaskCreate("Architecture ADRs", "Produce ARCHITECTURE.md for {feature}", "Designing architecture")
TaskCreate("Pseudocode", "Produce PSEUDOCODE.md for {feature}", "Writing pseudocode")
TaskCreate("Vision alignment", "Produce ALIGNMENT-REPORT.md for {feature}", "Checking alignment")
TaskCreate("Implementation brief", "Produce IMPLEMENTATION-BRIEF.md for {feature}", "Generating brief")
TaskCreate("GH Issue creation", "Create GH Issue from brief", "Creating GH Issue")

mcp__claude-flow__memory_store(
  key: "{feature-id}-context",
  value: "{scope summary, goals, constraints, pattern IDs}",
  namespace: "{feature-id}"
)
```

Set task dependencies with TaskUpdate after creation.

#### Step 3c: Agent Spawning

Spawn ALL planning agents in ONE message (parallel).

**Pre-spawn checklist**:
- [ ] swarm init ran
- [ ] Tasks defined
- [ ] Memory seeded
- [ ] SCOPE.md read

Agent types for planning: `ndp-architect`, `specification`, `pseudocode`

Do NOT spawn: `ndp-rust-dev`, `ndp-tester`, `coder`, `sparc-coder`.

Each agent prompt MUST include:
1. Task description (2-3 sentences)
2. Namespace for coordination
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

#### Step 3h: Validate Planning Artifacts

Run `/validate-plan {feature-id}` to verify planning output quality:
- Required artifacts exist (IMPLEMENTATION-BRIEF.md, ACCEPTANCE-MAP.md, LAUNCH-PROMPT.md, ALIGNMENT-REPORT.md, SPECIFICATION.md, ARCHITECTURE.md)
- AC coverage: every AC-ID from SCOPE.md appears in ACCEPTANCE-MAP.md
- ADR pattern IDs resolve in AgentDB
- No stale references (deprecated pattern IDs, removed file paths)
- Internal consistency: file paths in brief are valid, AC-IDs match SCOPE.md

If validation fails, fix issues before returning to the primary agent. Report validation result in the summary.

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
  Step 3a:  MCP: hive-mind_init (coordination layer)
  Step 3b:  TaskCreate (batch ALL) + MCP: memory_store (shared context)
  Step 3c:  Task() spawn ALL planning agents (parallel)
  Step 3d:  Task(ndp-vision-guardian) — alignment check
  Step 3e:  Store each ADR in AgentDB via agentdb_pattern_store (permanent)
  Step 3f:  Generate ACCEPTANCE-MAP.md + LAUNCH-PROMPT.md + IMPLEMENTATION-BRIEF.md
  Step 3g:  gh issue create + update SCOPE.md
  Step 3h:  /validate-plan {feature-id} — verify planning artifacts
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

## Two Memory Systems

| System | Tool | Purpose |
|--------|------|---------|
| **AgentDB** | `/get-pattern`, `/save-pattern`, `/reflexion` | Permanent project knowledge |
| **Claude-Flow Memory** | `claude-flow memory` CLI via Bash | Transient swarm coordination |

If it's useful 6 months from now → AgentDB. If it's only useful during this swarm → claude-flow memory.
