# Swarm Orchestration Protocol

Base protocol for all swarm operations. Extended by:
- `design-protocol.md` — Session 1: research, design, vision check, brief compilation
- `delivery-protocol.md` — Session 2: component design, implementation, testing with 3 validation gates

Both are in `.claude/protocols/` — NOT auto-loaded as rules. The scrum-master reads them explicitly via Read().

---

## Two Sessions, Two Leaders

The development lifecycle executes across two distinct sessions with a human approval gate between them.

| Session | Leader | Protocol | What It Does |
|---------|--------|----------|-------------|
| Session 1 (Design) | Design Leader | `design-protocol.md` | Research → 3 source docs → vision check → brief → GH Issue. Returns to human. |
| Session 2 (Delivery) | Delivery Leader | `delivery-protocol.md` | Stage 3a → Gate 3a → Stage 3b → Gate 3b → Stage 3c → Gate 3c → Delivery. |

Both leaders are the same agent (ndp-scrum-master) reading different protocols. The **Implementation Brief** is the handoff document — it contains the component map, acceptance criteria, GH Issue number, and paths to the three source documents.

```
SESSION 1                                SESSION 2
═════════                                ═════════

Phase 1: Research & Scope                Stage 3a: Component Design
Phase 2: Design (3 source docs)            ★ Gate 3a ★
  → Vision alignment                     Stage 3b: Implementation
  → Synthesizer (brief + map + GH)         ★ Gate 3b ★
  ★ HUMAN REVIEWS & APPROVES ★           Stage 3c: Testing & Risk Validation
                                           ★ Gate 3c ★
                                         Phase 4: Delivery
```

---

## Three Validation Gates

The Delivery Leader runs three validation gates sequentially. Each gate uses the same validator agent (ndp-validator) with different focused checks.

| Gate | What It Validates | Validates Against | On Pass | On Fail |
|------|-------------------|-------------------|---------|---------|
| Gate 3a | Component designs, pseudocode, test plans | Architecture, Specification, Risk Strategy | Auto-proceed to 3b | Rework (2x) or stop |
| Gate 3b | Implemented code, test cases | Pseudocode, Architecture, Specification | Auto-proceed to 3c | Rework (2x) or stop |
| Gate 3c | Test results, risk coverage | Risk Strategy, Specification, Architecture | Deliver | Rework (2x) or stop |

---

## Two-Tier Escalation

At every gate, failures fall into two categories:

**Reworkable**: Design doesn't match spec, code doesn't match pseudocode, test gaps. Loop back to previous stage agents. Max 2 iterations per gate.

**Scope/Feasibility**: Scope was wrong, technology doesn't work, architecture can't support a requirement. Stop the session immediately and return to human with a recommendation.

---

## Execution Model

Swarms use **coordinator delegation**: the primary agent spawns `ndp-scrum-master` as the single coordinator, who then spawns worker agents, monitors results, detects drift, and controls flow. Use **Task tool** (spawn-and-wait) at both levels. Do NOT use TeamCreate — Teams are for long-running collaborative work requiring inter-agent messaging.

See `design-protocol.md` and `delivery-protocol.md` for the specific delegation flows.

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

**Two required steps per agent** (+ one optional):
1. `agent_spawn(agentId)` — registers in agent store (`.claude-flow/agents/store.json`) **REQUIRED**
2. `Task()` — creates the real Claude Code process **REQUIRED**
3. `hive-mind_join(agentId, role)` — registers in hive workers (`.claude-flow/hive-mind/state.json`) **OPTIONAL** — only needed if you want `hive-mind_status` tracking

Without `agent_spawn`, `agent_update` and `agent_status` fail. `agent_spawn` MUST happen before the Task call.

### Claude-Flow Memory

Agents use `memory_store`/`memory_retrieve` with `namespace: "coordination"` for all swarm state. Key convention: `swarm/{agent-id}/{status|progress|complete}` for per-agent state, `swarm/shared/{feature}-context` for shared context. Storage is SQLite + HNSW vectors.

**Memory conventions (IMPORTANT):**
- All `memory_store` calls MUST use `upsert: true` to prevent UNIQUE constraint failures on retries
- All memory values MUST include `"feature":"<feature-id>"` in the JSON payload (not just in the key) — this improves semantic search recall for discovery
- Discovery uses `memory_list` + `memory_retrieve` (exact-key, 100% reliable), NOT `memory_search` (semantic, 20-80% recall for JSON payloads)

Do NOT use `claude-flow swarm init` or `claude-flow agent spawn` CLI commands — they are cosmetic. Use MCP tools for coordination and the Task tool for agent processes.

### Agent ID = Swarm Activation

All NDP agent definitions (except ndp-scrum-master) contain a `## Swarm Coordination` section. This section is **dormant** unless the agent's spawn prompt includes `Your agent ID: <id>`. When present, the agent MUST:
- Write `swarm/{id}/status` on start (with `upsert: true`)
- Write `swarm/{id}/progress` after each major step (with `upsert: true`)
- Write `swarm/{id}/complete` before returning (with `upsert: true`)
- Read `swarm/shared/{feature}-context` for shared context

The coordinator's only job is to **pass the agent ID**. The agent definition handles the rest. This means the coordinator prompt can be minimal — no need to repeat memory instructions.

**Note**: The hive-mind layer is optional. Agents coordinate through `memory_store`/`memory_retrieve` with `namespace: "coordination"`. The hive-mind layer is useful for topology tracking but not required for agent coordination.

---

## Swarm Launch: 2 Messages

When a task qualifies for swarm (see complexity detection below), execute in 2 messages. Batch aggressively — all initialization in one message, all agent spawns in the next.

### Message 1: Initialize + Register + Define (ALL batched)

All MCP calls and TaskCreate calls go in ONE message:

```
# STEP 1: Register each agent
mcp__claude-flow__agent_spawn(agentId: "agent-1-{role}", agentType: "{type}")
mcp__claude-flow__agent_spawn(agentId: "agent-2-{role}", agentType: "{type}")

# STEP 2: Seed shared context
mcp__claude-flow__memory_store(
  key: "swarm/shared/{feature-id}-context",
  value: "{task description, goals, constraints}",
  namespace: "coordination",
  upsert: true
)

# STEP 3: Define ALL tasks (batched)
TaskCreate("Task 1 subject", "description", "active form")
TaskCreate("Task 2 subject", "description", "active form")
```

Set task dependencies with TaskUpdate after creation.

### Message 2: Execute (ALL agents spawned in parallel)

Spawn ALL agents in ONE message via Task tool. Every Task call runs in parallel.

Each agent prompt MUST include:
1. `Your agent ID: {feature}-agent-N-{role}` — this activates the Swarm Coordination block in agent definitions
2. The task description (2-3 sentences)
3. Specific file paths

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

## Multi-Stage Features

For the Delivery Leader running Stages 3a → 3b → 3c:
- Complete each stage fully before starting the next
- Run the validation gate between stages
- If a gate passes, proceed to the next stage in a NEW message
- If a gate fails, rework (max 2 iterations) or stop
- Post progress to GH Issue after each gate

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

In the design session, agents get the SCOPE.md for alignment. In the delivery session, agents get the three source documents + implementation brief. These are the primary anti-drift mechanisms.

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
