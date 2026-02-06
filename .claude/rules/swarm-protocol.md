---
paths:
  - "product/features/**/*"
  - "CLAUDE.md"
---

# Swarm Orchestration Protocol

## Two Memory Systems — Know the Difference

| System | Tool | Persistence | Purpose | Example |
|--------|------|-------------|---------|---------|
| **AgentDB** | `/get-pattern`, `/save-pattern`, `/reflexion` skills | **Permanent** | Architecture, conventions, procedures — project knowledge that outlives any session | "How do we add a new stream?" |
| **Claude-Flow Memory** | `claude-flow memory` CLI via Bash | **Transient** | Coordination, task status, agent handoffs — dies when the work is done | "Agent-3 finished the schema, Agent-4 can start tests" |

**Rule**: If it's useful to an agent 6 months from now → AgentDB. If it's only useful during this swarm session → claude-flow memory.

---

## Swarm Launch Sequence (ONE message, all steps)

When a task qualifies for swarm (see complexity detection below), execute these steps in a SINGLE message:

### Step 1: Initialize swarm coordination
```bash
claude-flow swarm init --topology hierarchical --max-agents 8 --strategy specialized
```

### Step 2: Seed shared memory with task context
```bash
claude-flow memory store --key "{feature-id}-context" --value "{task description, goals, constraints}" --namespace {feature-id}
```
The namespace is the coordination channel. All agents will read/write to it.

### Step 3: Spawn agents via Task tool (background)

Each agent prompt MUST include:
1. The task description
2. The namespace to coordinate through
3. Explicit instruction to read/write shared memory

Example agent prompt:
```
You are working on {feature-id}.

Read shared context: claude-flow memory retrieve --key "{feature-id}-context" --namespace {feature-id}

When you complete your work, store results:
claude-flow memory store --key "{feature-id}-{your-role}-result" --value "{summary}" --namespace {feature-id}

Your task: [specific work for this agent]
```

### Step 4: Tell the user, then STOP

Report what agents are working on. Do not add more tool calls. Wait for agents to return results.

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
# Small teams (6-8 agents) — tight control
claude-flow swarm init --topology hierarchical --max-agents 8 --strategy specialized

# Large teams (10-15 agents) — queen + peer communication
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

---

## Spawn and Wait Pattern

After spawning background agents:
1. **TELL USER** what agents are working on
2. **STOP** — no more tool calls
3. **WAIT** — let agents complete
4. **SYNTHESIZE** — review results, check shared memory for coordination notes

DO NOT: poll TaskOutput repeatedly, check swarm status continuously, or add more tool calls after spawning.
