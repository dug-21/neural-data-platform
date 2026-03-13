# Design Protocol

Session 1 protocol. Read by the **Design Leader** (ndp-scrum-master).

Triggers on: specification, architecture, design, risk strategy, SPARC S/A phases, new feature planning.

---

## Execution Model

The primary agent spawns `ndp-scrum-master` as the Design Leader. The Design Leader spawns design agents, runs vision alignment, generates the implementation brief, and returns everything to the human. **Session 1 ends after this protocol completes.** The human reviews and approves before Session 2 begins.

```
Primary Agent                    Design Leader                    Design Agents
─────────────                    ─────────────                    ─────────────
get-pattern
read SCOPE.md
scope pre-check
spawn Design Leader ──────────►  read protocol + SCOPE.md
                                 swarm init
                                 TaskCreate (all tasks)
                                 seed shared memory
                                 Wave 1: spawn arch + spec ─────► produce arch + spec
                                 ◄──────────────────────────────  return artifact paths
                                 Wave 2: spawn risk strategist ─► risk strategy (reads arch + spec)
                                 ◄──────────────────────────────  return artifact path
                                 Wave 3: spawn vision guardian ─► alignment check (reads all 3)
                                 ◄──────────────────────────────  return report
                                 Wave 4: spawn synthesizer ─────► brief + map + GH Issue
                                 ◄──────────────────────────────  return paths + URL
◄──────────────────────────────  return summary
present to human for review
reflexion + save-pattern
★ SESSION 1 ENDS ★
```

Do NOT use TeamCreate. Design swarms are coordinator-driven via Task tool spawn-and-wait.

### Concurrency Rules

Each message batches ALL related operations of the same type:

- ALWAYS batch ALL TaskCreate calls in ONE message
- ALWAYS spawn all agents WITHIN each wave in ONE message via Task tool
- ALWAYS batch ALL file reads/writes/edits in ONE message
- ALWAYS batch ALL memory store/retrieve operations in ONE message

### Design Rules

- Output goes to `product/features/{feature-id}/` ONLY
- NO code changes. NO file edits outside `product/features/`
- NO launching implementation agents (ndp-rust-dev, sparc-coder)
- NO launching component design agents (ndp-pseudocode) — that's Session 2
- Each design agent gets: SCOPE.md + relevant AgentDB patterns
- Agents return: artifact paths + key decisions + open questions (NOT full file contents)

---

## Flow: 4 Phases

### Phase 1: Preparation (primary agent)

Pattern search and scope reading happen BEFORE spawning the Design Leader.

```
/get-pattern — search AgentDB for relevant patterns
```

Note which pattern IDs were returned for reflexion later.

Read `product/features/{feature-id}/SCOPE.md` — this defines what the design must produce.

#### Scope Pre-Check (REQUIRED)

Before spawning the Design Leader, perform a quick alignment scan of SCOPE.md against `product/vision/ALIGNMENT-CRITERIA.md`. Check the 7 alignment principles at a surface level:

1. Does the scope imply cloud-only dependencies? (Edge-Only violation)
2. Does the scope require hardcoded values? (Config-Driven violation)
3. Does the scope introduce banned dependencies? (Resource-Constrained violation)
4. Does the scope target the correct version? (Version discipline)

If any red flags are found, present them to the user BEFORE spawning the Design Leader. This prevents wasting a full design cycle on a misaligned scope.

### Phase 2: Delegation (primary agent)

Spawn `ndp-scrum-master` as the Design Leader with the full context. ONE Task call.

```
Task(
  subagent_type: "ndp-scrum-master",
  prompt: "You are the Design Leader for {feature-id}.

    Read the design protocol: .claude/protocols/design-protocol.md
    Read the scope: product/features/{feature-id}/SCOPE.md

    Pattern IDs from get-pattern: {list IDs}
    Feature namespace: {feature-id}

    Execute the design protocol: init → define tasks → Wave 1 (arch + spec) →
    Wave 2 (risk strategy) → Wave 3 (vision alignment) → Wave 4 (synthesizer).
    Return: artifacts produced, key decisions, open questions, GH Issue URL,
    and any vision alignment variances requiring user approval."
)
```

After spawning: tell the user that the Design Leader is coordinating, then STOP.

### Phase 3: Design Execution (Design Leader)

The Design Leader executes the following steps autonomously.

#### Step 3a: Initialize + Define Tasks (ONE message)

Batch all initialization and task creation in ONE message:

```
# 1. Register each planned agent
mcp__claude-flow__agent_spawn(agentId: "agent-1-arch", agentType: "ndp-architect")
mcp__claude-flow__agent_spawn(agentId: "agent-2-spec", agentType: "ndp-specification")
mcp__claude-flow__agent_spawn(agentId: "agent-3-risk", agentType: "ndp-risk-strategist")
mcp__claude-flow__agent_spawn(agentId: "agent-4-vision", agentType: "ndp-vision-guardian")
mcp__claude-flow__agent_spawn(agentId: "agent-5-synth", agentType: "ndp-synthesizer")

# 2. Seed shared context
mcp__claude-flow__memory_store(
  key: "swarm/shared/{feature-id}-context",
  value: "{scope summary, goals, constraints, pattern IDs}",
  namespace: "coordination",
  upsert: true
)

# 3. Define ALL tasks with wave dependencies

# Wave 1 — parallel (no dependencies)
TaskCreate("Architecture document", "Produce ARCHITECTURE.md with ADRs for {feature}", "Designing architecture")
TaskCreate("Specification document", "Produce SPECIFICATION.md for {feature}", "Writing specification")

# Wave 2 — BLOCKED BY Wave 1
TaskCreate("Risk-Based Test Strategy", "Produce RISK-TEST-STRATEGY.md using Architecture + Specification + product vision", "Analyzing risks")

# Wave 3 — BLOCKED BY Wave 2
TaskCreate("Vision alignment", "Produce ALIGNMENT-REPORT.md checking all 3 source docs", "Checking alignment")

# Wave 4 — BLOCKED BY Wave 3
TaskCreate("Implementation brief + GH Issue", "Produce IMPLEMENTATION-BRIEF.md, ACCEPTANCE-MAP.md, GH Issue", "Compiling brief")
```

Set task dependencies with TaskUpdate after creation:
- Wave 2 task (risk) is `addBlockedBy` Wave 1 tasks (arch, spec)
- Wave 3 task (vision) is `addBlockedBy` Wave 2 task (risk)
- Wave 4 task (synthesizer) is `addBlockedBy` Wave 3 task (vision)

#### Step 3b: Wave 1 — Architecture + Specification (parallel, ONE message)

Spawn two specialist agents in ONE message:

**ndp-architect**: Produces `architecture/ARCHITECTURE.md`
- Performs codebase consultation for integration surface analysis
- Creates ADRs, stores each in AgentDB via `/save-pattern`
- Component breakdown with interfaces and contracts
- Returns ADR pattern IDs in completion message

**ndp-specification**: Produces `specification/SPECIFICATION.md`
- Translates SCOPE.md into structured requirements
- Detailed functional + non-functional requirements
- Acceptance criteria with verification methods
- Domain models and ubiquitous language

Each agent prompt MUST include:
1. `Your agent ID: {feature}-agent-N-{role}` — activates Swarm Coordination
2. Task description (2-3 sentences)
3. The SCOPE.md path
4. Relevant AgentDB pattern IDs

Wait for BOTH Wave 1 agents to complete before proceeding to Wave 2.

#### Step 3c: Wave 2 — Risk Strategy (sequential, AFTER Wave 1)

Spawn `ndp-risk-strategist`. This agent reads the Architecture and Specification produced in Wave 1, plus the product vision criteria, to build a risk strategy that is grounded in actual design decisions.

```
"Produce RISK-TEST-STRATEGY.md for {feature-id}.
 Read these inputs:
 - product/features/{id}/SCOPE.md
 - product/features/{id}/architecture/ARCHITECTURE.md
 - product/features/{id}/specification/SPECIFICATION.md
 - product/vision/ALIGNMENT-CRITERIA.md (product vision context)
 Identify feature-level risks, map to test scenarios, prioritize by severity × likelihood."
```

Wait for the risk strategist to complete before proceeding to Wave 3.

#### Step 3d: Wave 3 — Vision Alignment (sequential, AFTER Wave 2)

Spawn `ndp-vision-guardian`. Now all three source documents exist:

```
"Read product/vision/ALIGNMENT-CRITERIA.md and the three source documents at
 product/features/{feature-id}/. Produce ALIGNMENT-REPORT.md.
 Flag any variances requiring user approval."
```

Save to `product/features/{feature-id}/ALIGNMENT-REPORT.md`.

Include variances in the return summary. The primary agent will present them to the user.

Wait for the vision guardian to complete before proceeding to Wave 4.

#### Step 3e: Wave 4 — Synthesizer (sequential, AFTER Wave 3)

Spawn `ndp-synthesizer` with a fresh context window. Pass all artifact paths and the architect's ADR pattern IDs:

```
Task(
  subagent_type: "ndp-synthesizer",
  prompt: "You are compiling the implementation brief for {feature-id}.
    Your agent ID: {feature-id}-synthesizer

    Read these source documents:
    - product/features/{id}/SCOPE.md
    - product/features/{id}/specification/SPECIFICATION.md
    - product/features/{id}/architecture/ARCHITECTURE.md
    - product/features/{id}/RISK-TEST-STRATEGY.md
    - product/features/{id}/ALIGNMENT-REPORT.md

    ADR pattern IDs from architect: {list from architect's return}
    Vision variances: {from vision guardian's return}

    Produce: IMPLEMENTATION-BRIEF.md, ACCEPTANCE-MAP.md, GH Issue.
    Return: file paths + GH Issue URL."
)
```

The synthesizer gets a fresh context window — it reads artifacts directly for higher quality synthesis.

#### Step 3f: Learning Gate

After the synthesizer completes:

1. Review agent return messages for pattern assessment data
2. Record reflexion entries for each pattern used across the swarm
3. Save any new patterns from agent discoveries
4. Include learning summary in return to primary agent

#### Step 3g: Return to Primary Agent

Return the following to the primary agent:

- Artifact paths: ARCHITECTURE.md, SPECIFICATION.md, RISK-TEST-STRATEGY.md, ALIGNMENT-REPORT.md, IMPLEMENTATION-BRIEF.md, ACCEPTANCE-MAP.md
- GH Issue URL
- Vision alignment variances (if any require approval)
- Key decisions made (ADR summaries)
- Open questions for human review
- Learning summary

**Session 1 ends.** The human reviews everything and decides whether to approve, revise, or reject.

### Phase 4: Completion (primary agent)

After the Design Leader returns:

1. Review: artifacts produced, key decisions, open questions, GH Issue URL
2. Present vision alignment variances to user (if any require approval)
3. Present the three source documents for human review
4. Record learning:
   ```
   /reflexion — record pattern effectiveness (per pattern used, referencing IDs)
   /save-pattern — store new discoveries (if any)
   ```

**The human reviews the three source documents, alignment report, acceptance map, and implementation brief.** When the human approves, they initiate Session 2 using the delivery protocol.

---

## Quick Reference: Message Map

```
PRIMARY AGENT:
  Message 1:  /get-pattern + Read SCOPE.md + scope pre-check
  Message 2:  Task(ndp-scrum-master as Design Leader)
  ...wait...
  Message 3:  Review results + present to human + /reflexion + /save-pattern

DESIGN LEADER (internal):
  Step 3a:  MCP: agent_spawn (all agents) + memory_store seed + TaskCreate (all tasks)
  Step 3b:  Wave 1: Task(ndp-architect) + Task(ndp-specification) — parallel
            ...wait for Wave 1...
  Step 3c:  Wave 2: Task(ndp-risk-strategist) — reads arch + spec + vision
            ...wait for Wave 2...
  Step 3d:  Wave 3: Task(ndp-vision-guardian) — reads all 3 source docs
            ...wait for Wave 3...
  Step 3e:  Wave 4: Task(ndp-synthesizer) — fresh context, reads all artifacts
  Step 3f:  Learning gate
  Step 3g:  Return summary to primary agent
```

---

## Agent Context Budget

Each spawned design agent should receive:
- Task description (2-3 sentences)
- Namespace for claude-flow memory coordination
- SCOPE.md path (agents read it themselves)
- Relevant AgentDB pattern IDs

Do NOT paste full spec documents, source files, or cargo output into design agent prompts.

---

## Three Memory Systems

| System | Tool | Persistence | Purpose |
|--------|------|-------------|---------|
| **AgentDB** | `/get-pattern`, `/save-pattern`, `/reflexion` | Permanent | Architecture, conventions, procedures |
| **Swarm Memory** | `memory_store`/`memory_retrieve` with `namespace: "coordination"` | Session | Agent status, progress, results, shared context |
| **Hive Metadata** | `hive-mind_init`/`hive-mind_join`/`hive-mind_status` | Session | Agent registration, swarm topology tracking (optional) |

Rule: Useful 6 months from now → AgentDB. Swarm coordination → `memory_store` with `namespace: "coordination"`. Hive metadata → registration only.
