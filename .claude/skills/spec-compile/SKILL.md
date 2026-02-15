---
name: "spec-compile"
description: "Compile an IMPLEMENTATION-BRIEF.md into claude-flow memory chunks and store ADRs in AgentDB. Agents get a Level-1 summary with objective, ADR pattern IDs, and scope — pull details on-demand."
---

# /spec-compile — Spec-to-Memory Compiler

Two jobs:
1. Store **brief sections** in claude-flow memory (transient, for this swarm)
2. Store **each ADR** in AgentDB via save-pattern (permanent, for all future agents)

Agents receive a Level-1 summary containing the full objective, ADR pattern IDs, resolved decisions, constraints, and scope exclusions. They pull brief sections via `memory_search` and full ADRs via `get-pattern`.

## Usage

```
/spec-compile <feature-id>
```

Example: `/spec-compile fe-004`

## Protocol

When invoked, follow these steps exactly:

### Step 1: Read the brief and ADRs

```
Read product/features/{feature-id}/IMPLEMENTATION-BRIEF.md
Read product/features/{feature-id}/architecture/ARCHITECTURE.md (if exists)
```

Count total lines across both files. This is the "before" measurement.

### Step 2: Store ADRs in AgentDB (permanent) via /save-pattern

Parse the ARCHITECTURE.md by `## ADR-NNN:` headers. For EACH ADR, store it using the `/save-pattern` skill conventions:

```
/save-pattern
  taskType: "adr:{feature-id}-{nnn}"
  approach: "{full ADR text: Context + Decision + Consequences — verbatim, no summarizing}"
  successRate: 1.0
  tags: ["adr", "{feature-id}", "architecture", "{title-slug}"]
```

See `/save-pattern` skill for the underlying `agentdb_pattern_store` call and best practices (check for duplicates first, include file references, etc.).

Record the returned pattern IDs. These go into the Level-1 summary so agents can retrieve them via `/get-pattern`.

ADRs are the primary anti-drift mechanism. They MUST be stored in full — agents need the complete reasoning (Context: why, Decision: what, Consequences: tradeoffs), not a 1-line summary.

### Step 3: Split brief into sections

Parse the brief by `## N.` headers. Each header becomes a section. Map headers to semantic keys:

| Header Pattern | Memory Key | Tags |
|---------------|------------|------|
| Goal | `{feature}/objective` | objective, goal |
| Resolved Decisions | `{feature}/decisions` | decisions, adr |
| Files to Create | `{feature}/files-create` | files, create |
| Files to Modify | `{feature}/files-modify` | files, modify |
| Data Structures | `{feature}/data-structures` | structs, types, rust |
| Key Function Signatures | `{feature}/function-signatures` | api, functions, signatures |
| Implementation Waves | `{feature}/implementation-waves` | waves, order, tasks |
| Test Expectations | `{feature}/testing` | tests, testing, assertions |
| Constraints | `{feature}/constraints` | constraints, rules, limits |
| Dependencies | `{feature}/dependencies` | deps, cargo, toml |
| NOT in Scope | `{feature}/not-in-scope` | exclusions, scope |
| Alignment | `{feature}/alignment` | alignment, vision |

For any header that doesn't match these patterns, use the header text slugified as the key.

### Step 4: Create Level-1 summary

This is what goes into every agent prompt. It must give agents enough context to understand WHY they're building something, WHAT constraints bind them, and HOW to get deeper details.

```
Feature: {feature-id}
Version target: {from brief header}

Objective: {FULL objective paragraph from Goal section — 2-4 sentences, preserve the "why" and the user-visible outcome}

ADRs (use /get-pattern for full text):
- ADR-001: {title} — {key consequence in 1 line} [Pattern ID {N}]
- ADR-002: {title} — {key consequence in 1 line} [Pattern ID {N}]
...

Resolved Decisions:
- {decision}: {resolution} (ADR-NNN)
- {decision}: {resolution} (ADR-NNN)
...

Files: {count} new, {count} modified
Waves: {count} implementation waves

Constraints:
- {constraint 1}
- {constraint 2}
- {constraint 3}
...

NOT in scope: {comma-separated list of exclusions}

Spec namespace: spec-{feature-id}
Sections: {comma-separated list of section keys}

BEFORE implementing, retrieve relevant ADRs:
  mcp__agentdb__agentdb_pattern_search(task="adr:{feature-id}", k=10)

For spec details, search memory:
  mcp__claude-flow__memory_search(query="your question", namespace="spec-{feature-id}")
Or retrieve directly:
  mcp__claude-flow__memory_retrieve(key="{feature}/{section-key}", namespace="spec-{feature-id}")
```

### Step 5: Store brief sections in claude-flow memory (transient)

For each brief section, call:

```
mcp__claude-flow__memory_store(
  key: "{feature}/{section-key}",
  value: "{section content}",
  namespace: "spec-{feature-id}",
  tags: [{appropriate tags}],
  upsert: true
)
```

Then store the Level-1 summary:

```
mcp__claude-flow__memory_store(
  key: "{feature}/summary",
  value: "{Level-1 summary text}",
  namespace: "spec-{feature-id}",
  tags: ["summary", "level-1", "index"],
  upsert: true
)
```

Then store the section index:

```
mcp__claude-flow__memory_store(
  key: "{feature}/index",
  value: "{JSON list of all section keys with line counts}",
  namespace: "spec-{feature-id}",
  tags: ["index", "sections"],
  upsert: true
)
```

### Step 6: Report

```
## Spec Compiled: {feature-id}

**Sources**:
- product/features/{feature-id}/IMPLEMENTATION-BRIEF.md ({N} lines)
- product/features/{feature-id}/architecture/ARCHITECTURE.md ({M} lines)
**Total lines compiled**: {N+M}
**Memory namespace**: spec-{feature-id}

### ADRs Stored in AgentDB (permanent)
| ADR | Title | Pattern ID |
|-----|-------|-----------|
| ADR-001 | {title} | {ID} |
| ADR-002 | {title} | {ID} |
| ... | ... | ... |

### Brief Sections in claude-flow memory (transient)
| Key | Lines | Tags |
|-----|-------|------|
| {feature}/objective | {n} | objective, goal |
| ... | ... | ... |

### Level-1 Summary ({L} lines vs {N+M} original)
{summary text}

### Context Savings
- Full spec + ADRs: ~{(N+M)*3} tokens
- Level-1 summary: ~{L*3} tokens
- Per-agent savings: ~{percent}%
- Agent pulls ~2-3 sections on-demand: ~{estimate} tokens typical
```

## How Agents Use Compiled Specs

The Level-1 summary goes directly into every agent's prompt. Every agent knows:
- **WHY** (full objective paragraph)
- **WHAT CONSTRAINS THEM** (ADR list with pattern IDs, constraints, scope exclusions)
- **HOW TO GET DETAILS** (get-pattern for ADRs, memory_search for spec sections)

Agent prompt template:

```
You are implementing {task} for {feature-id}.

{Level-1 summary from memory — includes objective, ADR pattern IDs, constraints, scope exclusions}

BEFORE implementing, retrieve the ADR(s) relevant to your task:
  mcp__agentdb__agentdb_pattern_search(task="adr:{feature-id}", k=10)

When you need spec details, search memory:
  mcp__claude-flow__memory_search(query="your question", namespace="spec-{feature-id}")

Available sections: {section list}
```

## Two Storage Systems

| What | Where | Why | How agents access |
|------|-------|-----|------------------|
| ADRs | AgentDB (permanent) | Architectural decisions outlive any single swarm | `agentdb_pattern_search(task="adr:{feature}")` |
| Brief sections | claude-flow memory (transient) | Spec details only needed during implementation | `memory_search(query="...", namespace="spec-{feature}")` |
| Level-1 summary | claude-flow memory + agent prompt | Navigation context for every agent | Injected into prompt at spawn time |
