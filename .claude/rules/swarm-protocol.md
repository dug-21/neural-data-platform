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

## Memory Systems — Know the Difference

| System | Tool | Persistence | Purpose | Example |
|--------|------|-------------|---------|---------|
| **AgentDB** | `/get-pattern`, `/save-pattern`, `/reflexion` skills | **Permanent** | Architecture, conventions, procedures — project knowledge that outlives any session | "How do we add a new stream?" |
| **Swarm Memory** | MCP `memory_store`/`memory_retrieve` with `namespace: "coordination"` | **Session** | Agent status/progress/results, shared context — all swarm coordination | "Agent-3 finished the schema" |
| **Hive Metadata** | MCP `hive-mind_init`/`hive-mind_join`/`hive-mind_status` | **Session** | Agent registration and topology tracking — NOT for data exchange | "5 workers registered, topology: hierarchical" |

**Rule**: Useful 6 months from now → AgentDB. Swarm coordination → `memory_store` with `namespace: "coordination"` and `swarm/*` key convention. Hive metadata → agent registration only.

---

## Swarm Architecture: Three Layers

```
Hive State                     Agent Store                    Claude Code Runtime
(.claude-flow/hive-mind/)      (.claude-flow/agents/)         (actual agent processes)

  hive-mind_init ──────────>   agent_spawn ──────────────>   Task() spawns real agent
  hive-mind_join ──────────>   agent_update  <───────────    Agent reports completion
  hive-mind_memory  <───────   agent_status ──────────────>  Orchestrator checks status
  hive-mind_status ─────────>
```

**Three integration steps per agent** (all required):
1. `agent_spawn(agentId)` — registers in agent store (`.claude-flow/agents/store.json`)
2. `hive-mind_join(agentId, role)` — registers in hive workers (`.claude-flow/hive-mind/state.json`)
3. `Task()` — creates the real Claude Code process

Without `hive-mind_join`, agents are invisible to `hive-mind_status`. Without `agent_spawn`, `agent_update` and `agent_status` fail. Both registration steps MUST happen before the Task call.

### Claude-Flow Memory

Agents use `memory_store`/`memory_retrieve` with `namespace: "coordination"` for all swarm state. Key convention: `swarm/{agent-id}/{status|progress|complete}` for per-agent state, `swarm/shared/{feature}-context` for shared context. Storage is SQLite + HNSW vectors.

Do NOT use `claude-flow swarm init` or `claude-flow agent spawn` CLI commands — they are cosmetic. Use MCP tools for coordination and the Task tool for agent processes.

### Agent ID = Swarm Activation

All NDP agent definitions (except ndp-scrum-master) contain a `## Swarm Coordination` section. This section is **dormant** unless the agent's spawn prompt includes `Your agent ID: <id>`. When present, the agent MUST:
- Write `swarm/{id}/status` on start
- Write `swarm/{id}/progress` after each major step
- Write `swarm/{id}/complete` before returning
- Read `swarm/shared/{feature}-context` for shared context

The coordinator's only job is to **pass the agent ID**. The agent definition handles the rest. This means the coordinator prompt can be minimal — no need to repeat memory instructions.

---

## Swarm Launch: 2 Messages

When a task qualifies for swarm (see complexity detection below), execute in 2 messages. Batch aggressively — all initialization in one message, all agent spawns in the next.

### Message 1: Initialize + Register + Define (ALL batched)

All MCP calls and TaskCreate calls go in ONE message. Use ToolSearch to find "claude-flow hive" tools first, then batch everything:

```
# STEP 1: Create the hive
mcp__claude-flow__hive-mind_init(
  topology: "hierarchical",
  queenId: "swarm-lead"
)

# STEP 2: Register each agent in BOTH stores (all in same message)
mcp__claude-flow__agent_spawn(agentId: "agent-1-{role}", agentType: "{type}")
mcp__claude-flow__hive-mind_join(agentId: "agent-1-{role}", role: "worker")
mcp__claude-flow__agent_spawn(agentId: "agent-2-{role}", agentType: "{type}")
mcp__claude-flow__hive-mind_join(agentId: "agent-2-{role}", role: "worker")
# ... all agents

# STEP 3: Seed shared context (agents read this via memory_retrieve)
mcp__claude-flow__memory_store(
  key: "swarm/shared/{feature-id}-context",
  value: "{task description, goals, constraints}",
  namespace: "coordination"
)

# STEP 4: Define ALL tasks (batched)
TaskCreate("Task 1 subject", "description", "active form")
TaskCreate("Task 2 subject", "description", "active form")
# ... all tasks
```

Set task dependencies with TaskUpdate after creation. Verify with `hive-mind_status(verbose=true)`.

### Message 2: Execute (ALL agents spawned in parallel)

Spawn ALL agents in ONE message via Task tool. Every Task call runs in parallel.

Each agent prompt MUST include:
1. `Your agent ID: {feature}-agent-N-{role}` — this activates the Swarm Coordination block in agent definitions
2. The Level-1 summary (if feature work — from `/spec-compile`)
3. The task description (2-3 sentences)
4. Specific file paths

The agent ID is the critical line. All NDP agent definitions (except ndp-scrum-master) contain a `## Swarm Coordination` section that activates when `Your agent ID:` is present. This section instructs agents to:
- Write `swarm/{id}/status` on start
- Write `swarm/{id}/progress` after each major step
- Write `swarm/{id}/complete` before returning
- Read `swarm/shared/{feature}-context` for shared state

The coordinator does NOT need to repeat memory instructions in the prompt — the agent definition handles it.

Example agent prompt:
```
You are agent-N implementing {subtask} for {feature-id}.
Your agent ID: {feature-id}-agent-N-{role}

{Level-1 summary — objective, ADR list with pattern IDs, constraints, NOT in scope}

YOUR SPECIFIC TASK: {subtask description}

Files to read/modify: {paths from brief}

BEFORE implementing:
  Use ToolSearch to find "agentdb pattern" tools, then call:
  mcp__agentdb__agentdb_pattern_search(task="adr:{feature-id}", k=10)
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

## Verifying Swarm State

After registering agents and before spawning Task agents, verify the hive is healthy:

```
mcp__claude-flow__hive-mind_status(verbose: true)
```

Expected: `workers` array contains all registered agents, `sharedMemory` has the context key.

If workers array is empty, the `hive-mind_join` calls failed — re-register before spawning.

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
