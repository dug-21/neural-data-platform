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
| **Claude-Flow Memory** | `claude-flow memory` CLI via Bash | **Transient** | Coordination, task status, agent handoffs — dies when the work is done | "Agent-3 finished the schema, Agent-4 can start tests" |

**Rule**: If it's useful to an agent 6 months from now → AgentDB. If it's only useful during this swarm session → claude-flow memory.

---

## Swarm Launch: 3 Messages

When a task qualifies for swarm (see complexity detection below), execute in 3 messages:

### Message 1: Infrastructure

Initialize the swarm coordination layer. ONE Bash call.

```bash
claude-flow swarm init --topology hierarchical --max-agents 8 --strategy specialized
```

### Message 2: Definition + Coordination

Define ALL tasks and seed shared memory in ONE message. Batch all TaskCreate calls and all Bash memory commands together.

```
TaskCreate("Task 1 subject", "description", "active form")
TaskCreate("Task 2 subject", "description", "active form")
...

Bash: claude-flow memory store --key "{feature-id}-context" \
  --value "{task description, goals, constraints}" \
  --namespace {feature-id}
```

The namespace is the coordination channel. All agents will read/write to it.

### Message 3: Execution

Spawn ALL agents in ONE message via Task tool. Every Task call runs in parallel.

Each agent prompt MUST include:
1. The task description (2-3 sentences)
2. The namespace to coordinate through
3. Explicit instruction to read/write shared memory
4. Specific file paths

Example agent prompt:
```
You are working on {feature-id}. Namespace: {feature-id}

Read shared context: claude-flow memory retrieve --key "{feature-id}-context" --namespace {feature-id}

When you complete your work, store results:
claude-flow memory store --key "{feature-id}-{your-role}-result" --value "{summary}" --namespace {feature-id}

Your task: [specific work for this agent]
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

```bash
# Small swarms (6-8 agents) — tight control
claude-flow swarm init --topology hierarchical --max-agents 8 --strategy specialized

# Large swarms (10-15 agents) — queen + peer communication
claude-flow swarm init --topology hierarchical-mesh --max-agents 15 --strategy specialized
```

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
