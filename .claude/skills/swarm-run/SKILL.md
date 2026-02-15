---
name: "swarm-run"
description: "Launch a real working swarm: claude-flow coordination layer + Claude Code agent runtime. Wires MCP state tracking to actual Task-tool agents."
---

# /swarm-run — Working Swarm Launcher

Bridges claude-flow MCP (coordination/memory) with Claude Code's Task tool (real agent runtime).

## Architecture

```
Claude-Flow MCP Layer          Claude Code Runtime Layer
(state, memory, tracking)      (actual agent processes)

  hive-mind/init  ──────────>  TeamCreate or parallel Tasks
  memory/store    <──────────  Agents write findings back
  task/create     <──────────  Agents update task status
  hive-mind/status ─────────>  Orchestrator checks progress
```

## How To Use This Skill

When you invoke `/swarm-run`, follow the protocol below. The args contain the objective.

### Step 1: Initialize Coordination Layer (claude-flow MCP)

```
Call mcp__claude-flow__hive-mind_init with:
  topology: "mesh" (or "hierarchical" for queen-led)
  queenId: "swarm-lead"
```

This creates `.claude-flow/hive-mind/state.json` for shared state.

### Step 2: Store the objective in shared memory

```
Call mcp__claude-flow__memory_store with:
  key: "swarm-objective"
  value: <the user's objective from args>
  namespace: "swarm"
```

### Step 3: Decompose into tasks

Break the objective into 2-4 concrete subtasks. For each:

```
Call mcp__claude-flow__task_create with:
  title: "<subtask title>"
  description: "<what the agent should do>"
  assignee: "agent-N"
  priority: "high"
```

### Step 4: Spawn REAL agents via Claude Code Task tool

For each subtask, spawn a real Claude Code agent using the Task tool. These are actual running agents, not metadata entries.

#### If a compiled spec exists (feature implementation swarm):

Check if a Level-1 summary was stored by `/spec-compile`:

```
mcp__claude-flow__memory_retrieve(key="{feature}/summary", namespace="spec-{feature}")
```

If found, EVERY agent prompt MUST include the Level-1 summary. This is the anti-drift mechanism — it gives agents the objective, ADR references, constraints, and scope exclusions.

Agent prompt template for feature work:

```
You are agent-N implementing {subtask} for {feature-id}.

{Level-1 summary — paste the full text from memory, including objective, ADR list, constraints, NOT in scope}

YOUR SPECIFIC TASK: {subtask description — 2-3 sentences}

BEFORE implementing:
1. Retrieve ADRs relevant to your task via /get-pattern conventions:
   Use ToolSearch to find "agentdb pattern" tools, then call:
   mcp__agentdb__agentdb_pattern_search(task="adr:{feature-id}", k=10)
   Read the full ADR text — it contains the architectural reasoning you must follow.

2. If you need spec details (data structures, function signatures, test expectations):
   Use ToolSearch to find "claude-flow memory" tools, then call:
   mcp__claude-flow__memory_search(query="your question", namespace="spec-{feature-id}")

AFTER completing your work:
3. Store your results in shared memory:
   mcp__claude-flow__memory_store(
     key="result-agent-N",
     value="<files changed, decisions made, issues encountered>",
     namespace="swarm-results"
   )

4. Update your task status:
   mcp__claude-flow__task_complete(taskId="<your-task-id>")
```

#### If no compiled spec exists (research/ad-hoc swarm):

Agent prompt template for non-feature work:

```
You are agent-N in a swarm. Your task: <subtask>

COORDINATION PROTOCOL:
1. Read the objective:
   Use ToolSearch to find "claude-flow memory" tools, then call:
   mcp__claude-flow__memory_retrieve(key="swarm-objective", namespace="swarm")

2. Do your work using available tools (Read, Write, Edit, Bash, Grep, etc.)

3. Store your results in shared memory:
   mcp__claude-flow__memory_store(
     key="result-agent-N",
     value="<your findings/output>",
     namespace="swarm-results"
   )

4. Update your task status:
   mcp__claude-flow__task_complete(taskId="<your-task-id>")
```

#### Spawning rules:
- Use `subagent_type: "general-purpose"` for full tool access (or NDP-specific agents per agent-routing.md)
- Use `model: "opus"` per user preference (or "sonnet" for simple tasks)
- Set `run_in_background: true` if you want parallel execution
- Set `mode: "bypassPermissions"` for autonomous agents

### Step 5: Collect results

After agents complete, retrieve their results:

```
Call mcp__claude-flow__memory_search with:
  query: "result"
  namespace: "swarm-results"
```

### Step 6: Report

Synthesize agent results and present to the user:

```
## Swarm Results

**Objective**: <objective>
**Agents**: N spawned, M completed
**Coordination**: claude-flow hive-mind (mesh topology)

### Agent Results
- agent-1: <summary>
- agent-2: <summary>
...

### Synthesized Output
<combined findings>
```

### Step 7: Cleanup

```
Call mcp__claude-flow__hive-mind_shutdown with:
  graceful: true
  force: true
```

---

## Example: Research Swarm

```
User: /swarm-run "Find all error handling patterns in the codebase"

Orchestrator:
1. hive-mind_init(topology="mesh")
2. memory_store(key="swarm-objective", value="Find error handling patterns")
3. Decompose:
   - agent-1: Search core/ for error handling
   - agent-2: Search crates/ for error handling
   - agent-3: Search apps/ for error handling
4. Spawn 3 real Task agents (general-purpose, run_in_background=true)
5. Each agent greps, reads, analyzes, stores results in memory
6. Collect results from memory
7. Synthesize and report
```

## When NOT to Use

- Single-file changes (just do it directly)
- Tasks that don't benefit from parallelism
- When you need interactive back-and-forth (use regular conversation)

## Key Insight

**claude-flow MCP = coordination backbone** (state, memory, tasks)
**Claude Code Task tool = agent runtime** (actual processing)

Never expect `claude-flow swarm start` or `agent spawn` to create running agents.
They only create metadata. The Task tool creates real agents.
