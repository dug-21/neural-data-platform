---
paths:
  - "product/features/**/*"
  - "CLAUDE.md"
---

# Planning Swarm Protocol

Triggers on: specification, pseudocode, architecture, design, research, scope, roadmap, SPARC S/P/A phases.

## Rules

- Output goes to `product/features/{feature-id}/{phase}/` ONLY
- NO code changes. NO file edits outside `product/features/`
- NO launching implementation agents (ndp-rust-dev, ndp-tester)
- Each planning agent gets: SCOPE.md + relevant existing SPARC artifacts + relevant AgentDB patterns
- Agents return: artifact paths + key decisions + open questions (NOT full file contents)

---

## Planning Swarm Steps

### Step 1: Pattern search
```
/get-pattern — search AgentDB for relevant patterns
```

### Step 2: Initialize swarm coordination
```bash
claude-flow swarm init --topology hierarchical --max-agents 8 --strategy specialized
```

### Step 3: Seed shared memory
```bash
claude-flow memory store --key "{feature-id}-context" --value "{task description, goals, constraints}" --namespace {feature-id}
```

### Step 4: Spawn planning agents via Task tool

Each agent prompt MUST include:
1. The task description (2-3 sentences)
2. The namespace to coordinate through
3. Specific SPARC phase to produce (specification, pseudocode, or architecture)
4. The SCOPE.md content or path

Agent types for planning: `ndp-architect`, `ndp-scrum-master`, `specification`, `pseudocode`

Do NOT spawn: `ndp-rust-dev`, `ndp-tester`, `coder`, `sparc-coder` — those are implementation agents.

### Step 5: Vision alignment check

After all planning agents complete, spawn the `ndp-vision-guardian` agent:

```
Spawn a Task agent (ndp-vision-guardian) with prompt:
  "Read product/vision/ALIGNMENT-CRITERIA.md and the SPARC artifacts at
   product/features/{feature-id}/. Produce an ALIGNMENT-REPORT.md.
   Flag any variances requiring user approval."
```

Save the report to: `product/features/{feature-id}/ALIGNMENT-REPORT.md`

**Present the Variances Requiring Approval section to the user.** Wait for approval before proceeding to Step 6.

### Step 6: Generate Implementation Brief

Produce `product/features/{feature-id}/IMPLEMENTATION-BRIEF.md` (target: 200-400 lines).

The brief contains ONLY what an implementation agent needs:
- Goal (2-3 sentences)
- GitHub Issue link
- Files to create/modify (paths + 1-line summaries)
- Data structures (actual Rust code)
- Function signatures (actual Rust code)
- Test expectations (unit + integration)
- Constraints (version, banned deps, ARM64, config-driven)
- Dependencies (crates, features)
- NOT in scope
- Alignment status (from ALIGNMENT-REPORT.md)

### Step 7: Tell the user, then STOP

Report what was produced. Do not add more tool calls. Wait for user review.

### Step 8: After completion
```
/reflexion — record pattern effectiveness (per pattern used)
/save-pattern — store new discoveries (if any)
```

---

## Agent Context Budget

Each spawned planning agent should receive:
- Task description (2-3 sentences)
- Namespace for claude-flow memory coordination
- SCOPE.md content or path
- Specific file paths to read (not "explore the codebase")
- Relevant AgentDB pattern IDs (not full pattern text)

Do NOT paste full spec documents from other features, full source files, or cargo output into planning agent prompts.

---

## Two Memory Systems

| System | Tool | Purpose |
|--------|------|---------|
| **AgentDB** | `/get-pattern`, `/save-pattern`, `/reflexion` | Permanent project knowledge |
| **Claude-Flow Memory** | `claude-flow memory` CLI via Bash | Transient swarm coordination |

If it's useful 6 months from now → AgentDB. If it's only useful during this swarm → claude-flow memory.
