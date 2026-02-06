---
paths:
  - "product/features/**/*"
  - "CLAUDE.md"
---

# Swarm Orchestration Protocol

## Swarm Launch Sequence (ONE message, no exceptions)

1. `/get-pattern` — search for relevant existing patterns
2. `claude-flow hooks pre-task --description "[task]"` — get routing recommendation
3. `claude-flow swarm init --topology hierarchical --max-agents 8 --strategy specialized` — initialize coordination
4. `claude-flow memory store --key "[task-key]" --value "[intent]" --namespace swarm` — record intent
5. Task tool spawns — launch agents with full context

All 5 steps in a SINGLE message. No exceptions.

## Anti-Drift Config

```bash
# Small teams (6-8 agents)
claude-flow swarm init --topology hierarchical --max-agents 8 --strategy specialized

# Large teams (10-15 agents)
claude-flow swarm init --topology hierarchical-mesh --max-agents 15 --strategy specialized
```

Valid topologies: hierarchical, hierarchical-mesh, mesh, ring, star, hybrid.

## 3-Tier Model Routing

Before spawning agents, run:
```bash
claude-flow hooks pre-task --description "[task description]"
```

| Tier | Handler | Use Cases |
|------|---------|-----------|
| 1 | Agent Booster (<1ms, $0) | Simple transforms: var-to-const, add-types, remove-console |
| 2 | Haiku (~500ms) | Simple tasks, bug fixes, low complexity |
| 3 | Sonnet/Opus (2-5s) | Architecture, security, complex reasoning |

When you see `[AGENT_BOOSTER_AVAILABLE]` — skip LLM, use Edit tool directly.
When you see `[TASK_MODEL_RECOMMENDATION] Use model="X"` — use that model in Task tool.

## Spawn and Wait Pattern

After spawning background agents:
1. **TELL USER** what agents are working on
2. **STOP** — no more tool calls
3. **WAIT** — let agents complete
4. **SYNTHESIZE** — review and combine results when they return

DO NOT: poll TaskOutput repeatedly, check swarm status continuously, or add more tool calls after spawning.

## Task Complexity Detection

**USE SWARM when**: 3+ files, new feature, cross-module refactor, API changes, security changes, performance work, schema changes.

**SKIP SWARM for**: single file edits, 1-2 line fixes, documentation updates, config changes, questions/exploration.
