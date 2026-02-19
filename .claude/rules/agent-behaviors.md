# Agent Standard Behaviors

## Swarm Coordination

**This section activates ONLY when your spawn prompt includes `Your agent ID: <id>`.**
If no agent ID was provided, skip this section entirely.

When part of a swarm, you MUST report status through shared memory:

**ON START** — immediately after reading your task:
```
Use ToolSearch to find "claude-flow memory" tools, then:
mcp__claude-flow__memory_store(
  key: "swarm/<your-agent-id>/status",
  value: '{"status":"task-received","task":"<brief description>","feature":"<feature-id>"}',
  namespace: "coordination",
  upsert: true
)
```

**ON PROGRESS** — after each major step:
```
mcp__claude-flow__memory_store(
  key: "swarm/<your-agent-id>/progress",
  value: '{"current_step":"<what you did>","files_modified":["<paths>"],"progress_pct":<N>,"feature":"<feature-id>"}',
  namespace: "coordination",
  upsert: true
)
```

**ON COMPLETE** — before returning results:
```
mcp__claude-flow__memory_store(
  key: "swarm/<your-agent-id>/complete",
  value: '{"status":"complete","deliverables":["<paths>"],"test_results":"<summary>","feature":"<feature-id>"}',
  namespace: "coordination",
  upsert: true
)
```

**READ SHARED CONTEXT** — at start:
```
mcp__claude-flow__memory_retrieve(
  key: "swarm/shared/<feature-id>-context",
  namespace: "coordination"
)
```

## Feature Work Context

When assigned work as part of a feature implementation swarm, read these files from the feature directory (the scrum master's spawn prompt tells you which component files are yours):

1. `IMPLEMENTATION-BRIEF.md` — your assignment, constraints, wave structure, component map
2. `architecture/ARCHITECTURE.md` — ADRs, integration surface findings
3. `pseudocode/OVERVIEW.md` — how your component connects to others
4. `pseudocode/{your-component}.md` — implementation detail for your specific component
5. `test-plan/OVERVIEW.md` — overall test strategy, testbed design
6. `test-plan/{your-component}.md` — what to test, expected assertions for your component

Read these files BEFORE starting implementation.

If the feature has a testbed at `product/features/{id}/testbed/`, review `testbed/validate.sh` to understand the integration assertions your code must satisfy.

## Pattern Workflow (All Agents)

### BEFORE Work
Use `get-pattern` skill to retrieve relevant patterns for your domain. Note which pattern IDs were returned.

### DURING Work
When you discover a pattern is wrong, outdated, or conflicts with current reality — flag it immediately. Do NOT wait until after your work is done. Use reflexion with reward=0.0 and a specific critique explaining what's wrong and what supersedes it.

### AFTER Work — Return Format
Include in your return message to the coordinator:
- Files modified: [paths]
- Tests: pass/fail count
- Issues: [any blockers]
- Patterns used: {ID: helped/didn't help/not relevant} for each pattern from get-pattern
- Discoveries: [any new approaches worth saving as patterns]

### AFTER Work — Reflexion (REQUIRED)
Call reflexion for EACH pattern you retrieved from get-pattern. Reference the pattern by ID.
- Pattern helped: reward 0.7-1.0, critique explains what specifically worked
- Pattern was irrelevant: reward 0.4-0.5, critique explains why it didn't apply
- Pattern was wrong/outdated: reward 0.0-0.2, critique explains what's incorrect and what to use instead

This is the feedback signal that makes pattern search better over time. Without it, future agents get the same bad recommendations you got.

## Self-Check Additions (Append to Agent-Specific Checks)

Every agent MUST verify these before returning:
- [ ] You called `get-pattern` before starting work
- [ ] You called `reflexion` for each pattern retrieved (one entry per pattern ID)
- [ ] Your return message includes the Patterns Used section
- [ ] If you discovered a pattern was wrong/outdated, you flagged it with reward=0.0 and specific critique
