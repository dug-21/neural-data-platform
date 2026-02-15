---
paths:
  - "product/features/**/*"
  - "CLAUDE.md"
---

# Swarm Orchestration Protocol

Base protocol for all swarm operations. Extended by `implementation-protocol.md` (for coding) and `planning-protocol.md` (for SPARC planning).

---

## Execution Model

Swarms use **coordinator delegation**: the primary agent spawns `ndp-scrum-master` as the single coordinator, who then spawns worker agents, monitors results, detects drift, and controls flow. Use **Task tool** (spawn-and-wait) at both levels. Do NOT use TeamCreate — Teams are for long-running collaborative work requiring inter-agent messaging.

See `implementation-protocol.md` and `planning-protocol.md` for the specific delegation flows.

---

## Two Memory Systems — Know the Difference

| System | Tool | Persistence | Purpose | Example |
|--------|------|-------------|---------|---------|
| **AgentDB** | `/get-pattern`, `/save-pattern`, `/reflexion` skills | **Permanent** | Architecture, conventions, procedures — project knowledge that outlives any session | "How do we add a new stream?" |
| **Claude-Flow Memory** | MCP `memory_store`/`memory_search`/`memory_retrieve` | **Transient** | Coordination, task status, agent handoffs — dies when the work is done | "Agent-3 finished the schema, Agent-4 can start tests" |

**Rule**: If it's useful to an agent 6 months from now → AgentDB. If it's only useful during this swarm session → claude-flow memory.

---

## Swarm Architecture: Two Layers

```
Claude-Flow MCP Layer          Claude Code Runtime Layer
(state, memory, tracking)      (actual agent processes)

  hive-mind/init  ──────────>  TaskCreate + Task tool spawns
  memory/store    <──────────  Agents write findings back
  task/create     <──────────  Agents update task status
  hive-mind/status ─────────>  Orchestrator checks progress
```

**claude-flow MCP = coordination backbone** (state, memory, tasks)
**Claude Code Task tool = agent runtime** (actual processing)

Do NOT use `claude-flow swarm init` or `claude-flow agent spawn` CLI commands — they are cosmetic and create no real state. Use MCP tools for coordination and the Task tool for agent processes. See `/swarm-run` skill for the tested, working implementation.

---

## Swarm Launch: 3 Messages

When a task qualifies for swarm (see complexity detection below), execute in 3 messages:

### Message 1: Infrastructure

Initialize the coordination layer via MCP tools. Use ToolSearch to find "claude-flow hive" tools first.

```
mcp__claude-flow__hive-mind_init(
  topology: "hierarchical",
  queenId: "swarm-lead"
)
```

### Message 2: Definition + Coordination

Define ALL tasks and seed shared memory in ONE message. Batch all TaskCreate calls and MCP memory_store calls together.

```
TaskCreate("Task 1 subject", "description", "active form")
TaskCreate("Task 2 subject", "description", "active form")
...

mcp__claude-flow__memory_store(
  key: "{feature-id}-context",
  value: "{task description, goals, constraints}",
  namespace: "{feature-id}"
)
```

The namespace is the coordination channel. All agents will read/write to it.

### Message 3: Execution

Spawn ALL agents in ONE message via Task tool. Every Task call runs in parallel.

Each agent prompt MUST include:
1. The Level-1 summary (if feature work — from `/spec-compile`)
2. The task description (2-3 sentences)
3. The namespace to coordinate through
4. Instructions to retrieve ADRs via `/get-pattern` (if feature work)
5. Instructions to read/write shared memory via MCP tools
6. Specific file paths

Example agent prompt:
```
You are agent-N implementing {subtask} for {feature-id}.

{Level-1 summary — objective, ADR list with pattern IDs, constraints, NOT in scope}

YOUR SPECIFIC TASK: {subtask description}

BEFORE implementing:
  Use ToolSearch to find "agentdb pattern" tools, then call:
  mcp__agentdb__agentdb_pattern_search(task="adr:{feature-id}", k=10)

For spec details:
  Use ToolSearch to find "claude-flow memory" tools, then call:
  mcp__claude-flow__memory_search(query="your question", namespace="spec-{feature-id}")

SELF-CHECK (before returning results):
  - [ ] All modified files are within the scope defined in the brief
  - [ ] No todo!(), unimplemented!(), TODO, FIXME, or HACK in non-test code
  - [ ] Tests pass (cargo test --workspace if Rust code modified)
  - [ ] You called get-pattern before implementing
  If any check fails, fix it before returning.

AFTER completing:
  mcp__claude-flow__memory_store(key="result-agent-N", value="<summary>", namespace="swarm-results")
```

After spawning: tell the user what agents are working on, then STOP.

---

## Spawn and Wait Pattern

After spawning agents:
1. **TELL USER** what agents are working on
2. **STOP** — no more tool calls
3. **WAIT** — let agents complete
4. **SYNTHESIZE** — review results, check shared memory for coordination notes

DO NOT: poll TaskOutput repeatedly, check swarm status continuously, or add more tool calls after spawning.

---

## Multi-Wave Features

For features with sequential waves (Wave 1 → Wave 2 → Wave 3):
- Spawn ALL agents within a wave in ONE message (parallel)
- Wait for the wave to complete
- Mark completed tasks, update TaskList
- Spawn the next wave's agents in a NEW message (parallel)
- Repeat until all waves complete

Do NOT spawn agents from different waves in the same message if Wave N+1 depends on Wave N outputs.

---

## Hooks That Fire Automatically (DO NOT duplicate)

These run via `.claude/settings.json` without agent action:

| Event | Hook | What it does |
|-------|------|-------------|
| Every user message | `route --task "$PROMPT"` | Routes to recommended agent |
| Every Task spawn | `pre-task --task-id ... --description ...` | Registers task |
| Every Task completion | `post-task --task-id ... --success ...` | Records outcome |
| Every Bash command | `pre-command` / `post-command` | Risk assessment + tracking |
| Every file edit | `pre-edit` / `post-edit` | Context + learning |
| Session start | `daemon start` + `session-restore` | Restores state |

**Do NOT manually run** `pre-task`, `route`, `pre-command`, `pre-edit`, or `session-start`. They already fire.

---

## Anti-Drift Config

```
# Small swarms (6-8 agents) — tight control
mcp__claude-flow__hive-mind_init(topology: "hierarchical", queenId: "swarm-lead")

# Large swarms (10-15 agents) — peer communication
mcp__claude-flow__hive-mind_init(topology: "mesh", queenId: "swarm-lead")
```

Plus: every agent gets the Level-1 summary (objective + ADR pattern IDs + constraints + NOT-in-scope) in its prompt. This is the primary anti-drift mechanism.

---

## 3-Tier Model Routing

The `model-route` hook fires automatically, but if you need to check manually:
```bash
claude-flow hooks model-route --task "[task description]"
```

| Tier | Model | Use Cases |
|------|-------|-----------|
| 1 | Agent Booster (<1ms, $0) | Simple transforms: var-to-const, add-types, remove-console |
| 2 | Haiku (~500ms) | Simple tasks, bug fixes, low complexity |
| 3 | Sonnet/Opus (2-5s) | Architecture, security, complex reasoning |

---

## Task Complexity Detection

**USE SWARM when**: 3+ files, new feature, cross-module refactor, API changes, security changes, performance work, schema changes.

**SKIP SWARM for**: single file edits, 1-2 line fixes, documentation updates, config changes, questions/exploration.
