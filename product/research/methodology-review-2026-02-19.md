# NDP Control Plane Methodology Review

**Date**: 2026-02-19
**Scope**: CLAUDE.md, .claude/rules, .claude/agents/ndp, .claude/skills, .claude/commands
**Lens**: Minimum context window for maximum agent effectiveness

---

## Executive Summary

The methodology is architecturally sound: planning teams produce structured artifacts, delivery teams consume them via AgentDB, and reflexion closes the feedback loop. The control plane successfully separates **permanent knowledge** (AgentDB) from **transient coordination** (claude-flow memory) from **structural guidance** (rules/agents/skills).

**However**, the current implementation has significant token waste from three root causes:

1. **Always-on rules that should be conditional** (~1,738 words loaded into EVERY conversation)
2. **Cross-file redundancy** (~500+ words duplicated across protocol files)
3. **Skill/command bloat** (35 non-NDP skills pollute the system-prompt listing, 168 commands listed as skills)

Estimated waste: **2,500-4,500 tokens per conversation** on baseline context, plus **3,000-8,000 tokens per skill invocation** from oversized skill files.

---

## How Claude Code Loads Context (Critical Foundation)

Understanding this mechanism is the basis for every recommendation:

| Source | When Loaded | Token Cost |
|--------|-------------|------------|
| `CLAUDE.md` | **EVERY** conversation, always in system prompt | Always paid |
| `.claude/rules/*.md` WITHOUT `paths:` frontmatter | **EVERY** conversation | Always paid |
| `.claude/rules/*.md` WITH `paths:` frontmatter | Only when files matching those paths are read/edited | Conditional |
| `.claude/agents/*.md` | Only when that agent type is spawned via Task tool | Per-spawn |
| `.claude/skills/*/SKILL.md` | Only when the skill is invoked | Per-invocation |
| `.claude/commands/*.md` | Only when the command is invoked (listed as skills) | Per-invocation |
| Skill/command **names** in system-reminder listing | **EVERY** conversation | Always paid |

**The cheapest token is the one never loaded.** Rules without `paths:` are the most expensive — they tax every conversation regardless of task type.

---

## Finding 1: THREE Rules Without `paths:` Are Always-On [HIGH]

**Impact: ~1,738 words (~2,300 tokens) loaded into EVERY conversation**

| File | Words | Relevant When | Actually Loaded |
|------|-------|---------------|-----------------|
| `pattern-workflow.md` | 982 | Feature work, implementation | Always |
| `agent-behaviors.md` | 498 | Swarm coordination only | Always |
| `memory-commands.md` | 258 | Swarm coordination only | Always |

**Why this matters**: A user asking "what does this config field do?" gets 1,738 words of swarm coordination and pattern workflow instructions injected into their context. Claude processes all of it, wasting compute and potentially getting confused about what's being asked.

**Fix**: Add `paths:` frontmatter to all three:

```yaml
# agent-behaviors.md
---
paths:
  - "product/features/**/*"
  - ".claude/agents/**/*"
---

# memory-commands.md
---
paths:
  - "product/features/**/*"
  - ".claude/agents/**/*"
---

# pattern-workflow.md
---
paths:
  - "product/features/**/*"
---
```

**Savings**: ~1,738 words (~2,300 tokens) for every non-feature conversation.

---

## Finding 2: `CLAUDE.md` Path Trigger Cascades [HIGH]

**Impact: 3,772 words loaded whenever `CLAUDE.md` is read/edited**

Both `planning-protocol.md` (2,393 words) and `swarm-protocol.md` (1,379 words) list `CLAUDE.md` in their `paths:` trigger. This means ANY edit to CLAUDE.md cascades into loading both full protocol files.

But editing CLAUDE.md is a methodology task, not a planning or swarm task. Loading 3,772 words of agent orchestration protocol is pure waste.

**Fix**: Remove `CLAUDE.md` from `paths:` in both files:

```yaml
# planning-protocol.md — BEFORE
paths:
  - "product/features/**/*"
  - "CLAUDE.md"                    # DELETE THIS LINE

# swarm-protocol.md — BEFORE
paths:
  - "product/features/**/*"
  - "CLAUDE.md"                    # DELETE THIS LINE
```

**Savings**: ~3,772 words (~5,000 tokens) per CLAUDE.md edit session.

---

## Finding 3: Cross-Protocol Redundancy [MEDIUM]

**Impact: ~500 words duplicated across protocol files**

Three sections are copy-pasted across `planning-protocol.md`, `implementation-protocol.md`, and `swarm-protocol.md`:

| Section | Appears In | Words Each |
|---------|-----------|------------|
| "Three Memory Systems" table | planning + implementation | ~100 |
| "Concurrency Rules" | planning + implementation | ~60 |
| "Agent Context Budget" | planning + implementation | ~80 |
| Message batching rules | all three files | ~80 |

Both planning-protocol and implementation-protocol already reference swarm-protocol as their "base protocol." But they duplicate its content rather than relying on the base.

**Fix**: Move shared content to `swarm-protocol.md` (the declared base). In the child protocols, add a single line: `See swarm-protocol.md for concurrency rules, memory systems, and context budget.`

**Savings**: ~500 words when both a child protocol and swarm-protocol load simultaneously.

---

## Finding 4: Agent Preference Table Duplicated [LOW]

**Impact: ~100 words**

The "NDP Agent Preference" table appears in both:
- `CLAUDE.md` (always loaded)
- `agent-routing.md` (loaded for feature/agent paths)

When both are in context, it's redundant.

**Fix**: Remove from CLAUDE.md (keep the brief reference), keep the full table in agent-routing.md where it belongs alongside the complete roster.

---

## Finding 5: Skill Files Are Too Verbose [HIGH]

**Impact: ~2,800 extra words per get-pattern + reflexion combo**

### get-pattern SKILL.md: 1,647 words

The file includes four retrieval methods:
1. `agentdb_pattern_search` (PRIMARY — used 95% of the time)
2. `recall_with_certificate` (ENHANCED — rarely needed)
3. `learning_predict` (OPTIONAL — requires learning session that doesn't exist yet)
4. `learning_explain` (OPTIONAL — for high-stakes decisions only)

Every `/get-pattern` call loads all 1,647 words. But methods 2-4 are used <5% of the time.

### reflexion SKILL.md: 1,765 words

The file includes:
1. `reflexion_store` (PRIMARY — the actual reflexion)
2. `learning_feedback` (ENHANCED — feeds RL engine)
3. `causal_add_edge` (ENHANCED — builds causal graph)
4. `learning_train` (PERIODIC — retrains policy)

Every `/reflexion` call loads all 1,765 words. But items 2-4 are used <10% of the time.

**Fix**: Split each into a lean primary skill and an advanced skill:

```
get-pattern     → 600 words  (pattern_search + basic workflow)
get-pattern-adv → 1,000 words (recall_with_certificate + learning_predict + learning_explain)

reflexion       → 600 words  (reflexion_store + reward scale + examples)
reflexion-adv   → 1,100 words (learning_feedback + causal_add_edge + learning_train)
```

**Savings**: ~2,200 words per standard get-pattern + reflexion cycle.

---

## Finding 6: 35 Non-NDP Skills Pollute System Prompt [MEDIUM]

**Impact: ~3,500 characters (~900 tokens) in every system-prompt skill listing**

These skills exist in `.claude/skills/` but are not NDP-related:

```
v3-*                (9 skills) — claude-flow v3 development
flow-nexus-*        (3 skills) — Flow Nexus platform
agentdb-*           (5 skills) — generic AgentDB features
github-*            (5 skills) — generic GitHub automation
reasoningbank-*     (2 skills) — ReasoningBank integration
hive-mind-advanced  (1 skill)  — generic hive-mind
pair-programming    (1 skill)  — pair programming mode
stream-chain        (1 skill)  — JSON streaming
swarm-*             (3 skills) — generic swarm orchestration
hooks-automation    (1 skill)  — hook setup
sparc-methodology   (1 skill)  — generic SPARC
skill-builder       (1 skill)  — meta skill creation
agentic-jujutsu     (1 skill)  — version control
```

Each skill name + description in the system-reminder listing costs ~50-100 characters. 35 skills = ~2,500+ characters just for the NDP-irrelevant listings.

**Fix**: Move non-NDP skills to a separate directory or prefix them so they don't load in NDP project context. Or create a `.claude/skills/.skillignore` or use project-scoped skill filtering if Claude Code supports it.

---

## Finding 7: 168 Commands Listed as Skills [MEDIUM]

**Impact: ~10,000+ characters in system-prompt skill listing**

The `.claude/commands/` directory has 168 markdown files, many organized into subdirectories (sparc: 32, github: 19, swarm: 17, hooks: 14, hive-mind: 12...). Every one appears in the system-prompt's skill listing.

Most are generic claude-flow operational commands (sparc modes, hive-mind operations, monitoring, training, optimization) that NDP agents never invoke directly.

**Fix**: Audit which commands are actually invoked in NDP workflows. Archive the rest to prevent listing. Key NDP commands to keep:
- `get-pattern`, `save-pattern`, `reflexion`, `learner`, `pattern-manage`
- `validate`, `validate-plan`, `trust-dashboard`, `shadow-judge`
- `align`, `spec-compile`
- `ndp-github-workflow`

Everything else is either generic tooling or never directly invoked.

---

## Finding 8: Implementation Protocol Too Broadly Triggered [MEDIUM]

**Impact: 2,033 words loaded for ANY .rs file touch**

`implementation-protocol.md` triggers on `core/**/*.rs`, `apps/**/*.rs`, `crates/**/*.rs`, `tools/**/*.rs`. Reading a single Rust file to answer a question loads the entire 2,033-word swarm orchestration protocol.

**Fix**: Narrow the trigger. The protocol is only relevant during multi-file implementation work, not single-file reads. Options:
- Trigger only on `product/features/**/refinement/**/*` and `product/features/**/completion/**/*` (SPARC R/C phases where implementation actually happens)
- Keep the Rust triggers but dramatically compress the protocol (see Finding 3)

---

## Finding 9: Hive-Mind Ceremony Is Inconsistent and Wasteful [LOW-MEDIUM]

**Impact: ~15 MCP calls per planning swarm, uncertain value**

Planning protocol REQUIRES `hive-mind_init` + `hive-mind_join` for each agent. Implementation protocol says hive-mind is OPTIONAL. Neither protocol's agents actually read hive-mind state for coordination — they use `memory_store`/`memory_retrieve`.

The hive-mind layer creates files on disk (`.claude-flow/hive-mind/state.json`) but no downstream consumer reads them for decision-making.

**Fix**: Make hive-mind optional in BOTH protocols. Document when it's actually useful (topology visualization, agent count tracking) vs. when it's ceremony. Remove it from the planning protocol's "pre-spawn checklist."

**Savings**: ~15 MCP tool calls per planning swarm × ToolSearch + call overhead.

---

## Finding 10: Reflexion Mandate Is Disproportionate for Simple Tasks [MEDIUM]

**Impact: 5-20 MCP calls per swarm for learning data of declining marginal value**

The rules require reflexion for EACH pattern retrieved, per agent, per task. A 4-agent swarm using 3-4 patterns each = 12-16 reflexion calls. Each requires ToolSearch + MCP call.

With 77 patterns already seeded and 0 causal edges, the reflexion data is accumulating but not demonstrably improving retrieval quality. The RL learning session doesn't even exist yet.

**Fix**:
- **Simple tasks**: One aggregate reflexion entry per task (not per-pattern)
- **Feature work**: Per-pattern reflexion (current behavior)
- **Make causal edges and RL feedback truly optional** — remove them from the default workflow; invoke only via `reflexion-adv` skill when the user requests deeper learning
- Add a "reflexion budget" concept: max 3 reflexion calls for simple tasks, max 10 for features

---

## Finding 11: CLAUDE.md Bans Plan Mode Unnecessarily [LOW]

**Impact: Forces expensive swarm spawning for simple planning**

Rule: "Never use claude plan mode — Write to scope.md. Leverage full SPARC planning swarm."

Claude's built-in plan mode is zero-cost (no extra spawns, no MCP calls). For small tasks (single-component features, bug investigations, config changes), a SPARC planning swarm (scrum-master + 4-6 agents) is 6-10x more expensive than built-in plan mode.

**Fix**: Allow plan mode for investigation/exploration. Reserve SPARC planning swarms for features that produce IMPLEMENTATION-BRIEF.md artifacts. The rule should be: "For features following SPARC phases, use planning swarms. For investigation, debugging, or simple planning, use built-in plan mode."

---

## Finding 12: `agent-behaviors.md` Swarm Section Is Dead Weight for Primary Agent [LOW]

The file contains: "This section activates ONLY when your spawn prompt includes `Your agent ID: <id>`." But because it lacks `paths:`, it's loaded into the primary agent's context where it will NEVER activate. It's 498 words of instructions the primary agent should ignore.

**Already covered by Finding 1** — adding `paths:` fixes this.

---

## Prioritized Action Plan

| Priority | Finding | Action | Token Savings |
|----------|---------|--------|---------------|
| P0 | #1 | Add `paths:` to 3 always-on rules | ~2,300 tok/conv |
| P0 | #2 | Remove `CLAUDE.md` from protocol path triggers | ~5,000 tok/edit |
| P1 | #5 | Split get-pattern and reflexion into basic+advanced | ~2,900 tok/invocation |
| P1 | #6,#7 | Audit and archive non-NDP skills/commands | ~900 tok/conv |
| P2 | #3 | Consolidate redundant protocol sections | ~700 tok/protocol load |
| P2 | #8 | Narrow implementation-protocol triggers | ~2,700 tok/rs-read |
| P2 | #10 | Proportional reflexion (aggregate for simple tasks) | ~10-15 MCP calls/swarm |
| P3 | #9 | Make hive-mind optional everywhere | ~15 MCP calls/planning |
| P3 | #11 | Allow plan mode for non-feature work | Variable |

**Total estimated savings**: 3,000-8,000 tokens per typical conversation, 10,000-20,000 tokens per feature swarm lifecycle.

---

## Methodology Strengths (Keep These)

1. **Clean separation of planning vs. delivery** — planning produces artifacts, delivery consumes them
2. **AgentDB as permanent knowledge store** — survives sessions, enables learning across agent lifetimes
3. **Component Map routing** — scrum-master gives each agent ONLY their relevant pseudocode/test-plan files
4. **Agent ID activation pattern** — `Your agent ID:` in spawn prompt activates coordination without bloating coordinator prompts
5. **Skills as on-demand context** — only loaded when invoked, not always in context
6. **Cargo output truncation rules** — explicit guidance prevents context bloat from build output
7. **Anti-stub rule** — prevents the #1 failure mode of code generation agents
8. **Three-tier model routing** — right-sizes compute to task complexity

---

## Open Questions for User Input

1. **Is the RL learning layer producing value?** 0 causal edges, no learning session active. Should the RL/causal enhanced methods be deferred until there's enough reflexion data to train on? (Recommendation: yes, defer to `reflexion-adv` skill)

2. **Are the 168 commands actually invoked?** Running a usage audit (which commands were invoked in the last 10 sessions) would reveal dead weight.

3. **Should SPARC planning swarms be required for ops-* and dp-* patches?** Currently every feature goes through the full swarm regardless of complexity. A lightweight path for small changes could save significant overhead.

4. **Is the `ndp-validator` producing actionable trust data?** If glass-box reports are generated but never reviewed, the validation step is ceremony rather than quality gate.
