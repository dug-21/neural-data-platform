# Adherence Audit: ops-006

> Date: 2026-02-15
> Scope: Agent definitions, rules, hooks, skills -- gap analysis for adherence enforcement
> Cross-referenced: report-01 (protocol evaluation), report-05 (truth/verify evaluation), PROPOSED-ACTIONS.md

---

## Executive Summary

The NDP development process is architecturally sound (3.5/5 per report-01) but enforcement is honor-system. Rules say "REQUIRED" but no mechanism blocks non-compliance. Hooks fire but all have `continueOnError: true` -- none are gates. Agent definitions mix "should" and "must" inconsistently, and 3 contain stale technology references. The prompt-check hook injects contradictory guidance (CLI commands the protocols say not to use). Of 44 skills, only 7-9 are referenced by NDP protocols; the rest are cognitive noise. The pattern workflow is described as "mandatory" in 5 places but has zero enforcement mechanism.

**Top 3 systemic issues:**
1. No hook blocks anything -- every hook has `continueOnError: true`
2. Pattern workflow (get-pattern/reflexion) is "mandatory" with no gate
3. Contradictions between prompt-check.sh and protocol files on swarm init

---

## Per-File Audit Findings

### Agent Definitions (`.claude/agents/ndp/`)

| File | Finding | Severity | Before | After Fix |
|------|---------|----------|--------|-----------|
| `ndp-scrum-master.md` L33 | Says "Run `claude-flow swarm init`" -- contradicts swarm-protocol.md L47 which says "Do NOT use" | P0 | `2. Run \`claude-flow swarm init\`` | `2. Initialize via MCP \`hive-mind_init\`` |
| `ndp-architect.md` L49 | Silver Layer marked "Planned" -- Silver is implemented (908 tests, TimescaleDB active) | P1 | `Silver Layer (Planned)` | `Silver Layer (Current)` |
| `ndp-architect.md` L50 | Gold Layer marked "Future" -- Gold DDL generator exists (`ndp-gold-ddl` crate) | P1 | `Gold Layer (Future)` | `Gold Layer (Current - DDL generation)` |
| `ndp-architect.md` L99 | Silver Storage marked "Planned" in tech table | P1 | `Silver Storage \| TimescaleDB \| Planned` | `Silver Storage \| TimescaleDB \| Current` |
| `ndp-architect.md` L69-87 | ADR format uses `# ADR-NNN` + `## Status` -- contradicts planning-protocol.md which uses `## ADR-NNN` + `### Context` | P1 | `# ADR-NNN:` with `## Status/Context/Decision` | `## ADR-NNN:` with `### Context/Decision/Consequences` (match planning-protocol.md) |
| `ndp-architect.md` L120 | Memory budget says "<1GB total" -- ALIGNMENT-CRITERIA.md says "~5.5GB typical of 16GB" | P1 | `Memory budget: <1GB total` | `Memory budget: ~5.5GB typical (256MB per container)` |
| `ndp-tester.md` L36-46 | Test directory structure references `tests/components/redis_streams/`, `tests/orchestrator/` -- may be outdated | P2 | Stale directory listing | Verify and update to match current test structure |
| `ndp-tester.md` L249 | References `cargo-tarpaulin` for coverage -- not installed, not in scope | P2 | `cargo tarpaulin --out Html` | Remove or note as "if installed" |
| `ndp-rust-dev.md` | No self-check block -- agent has no mechanism to validate own output | P1 | No self-check section | Add SELF-CHECK section per report-04 design |
| `ndp-tester.md` | No self-check block | P1 | No self-check section | Add SELF-CHECK section |
| `ndp-vision-guardian.md` L151 | References `specification` and `pseudocode` as related agents -- no NDP definitions exist for these | P2 | `specification -- Produces SPARC S-phase` | Note: uses generic agent types (no NDP customization) |
| All agent defs | Pattern Integration sections say "REQUIRED" but no mechanism enforces get-pattern/reflexion calls | P1 | `(REQUIRED)` label only | Add to agent self-check: "Verify you called get-pattern" |

### Rules (`.claude/rules/`)

| File | Finding | Severity | Before | After Fix |
|------|---------|----------|--------|-----------|
| `swarm-protocol.md` L47 | Says "Do NOT use `claude-flow swarm init`" -- correct, but contradicted by ndp-scrum-master.md and prompt-check.sh | P0 | Correct instruction, contradicted elsewhere | Fix the contradicting files (scrum-master.md, prompt-check.sh) |
| `implementation-protocol.md` L129 | spec-compile is conditional: "If `/spec-compile` was run" -- should be required | P1 | `If /spec-compile was run, retrieve the Level-1 summary` | `Retrieve the Level-1 summary (spec-compile is REQUIRED)` |
| `implementation-protocol.md` | No per-wave acceptance check (Step 3c.5 missing) | P1 | No step between agent return and drift check | Add Step 3c.5: map completed tasks to ACs |
| `planning-protocol.md` L148 | References `specification` and `pseudocode` agent types -- not NDP-specific | P2 | `Agent types: ndp-architect, specification, pseudocode` | Acknowledge these are generic types (or create NDP versions, out of scope for ops-006) |
| `planning-protocol.md` | No ACCEPTANCE-MAP.md in deliverables list | P1 | Missing from Step 3f artifacts | Add ACCEPTANCE-MAP.md to planning output |
| `planning-protocol.md` | No LAUNCH-PROMPT.md in deliverables list | P1 | Missing from Step 3f artifacts | Add LAUNCH-PROMPT.md to planning output |
| `agent-routing.md` | References `security-architect` and `auditor` -- no NDP definitions exist | P2 | Generic agent references | Note as known gap (new agents out of scope for ops-006) |
| `testing.md` | No flaky test manifest or baseline concept | P1 | No mention of flaky tests | Add flaky test reference and baseline concept |

### Hooks (`.claude/settings.json`)

| File | Finding | Severity | Before | After Fix |
|------|---------|----------|--------|-----------|
| All hooks | Every hook has `continueOnError: true` -- nothing blocks | P0 | All hooks are advisory | Identify which hooks should gate (pre-commit at minimum) |
| L72 PostToolUse Task | Reflexion reminder is just an echo -- agent can ignore | P1 | `echo '[REMINDER] Record /reflexion...'` | Cannot enforce in current hook model; document as known limitation |
| L84 UserPromptSubmit | Calls prompt-check.sh which injects contradictory CLI commands | P0 | `echo "claude-flow swarm init..."` at L35, L59 | Fix prompt-check.sh to say MCP hive-mind_init |
| L96-104 SessionStart | Starts daemon + session-restore -- useful but cosmetic | P2 | Works correctly | No change needed |
| L119-125 Stop | Reflexion reminder echo only | P1 | `echo 'Did you record /reflexion?'` | Cannot enforce; document as limitation |
| No pre-commit hook | No `git commit` intercept exists | P1 | Missing entirely | Add PreToolUse hook matching `^Bash$` with commit detection |
| No pre-spawn check | No workspace compile check before agent spawn | P1 | Missing entirely | Add pre-spawn workspace build check |

### Hook Effectiveness Analysis

| Hook | Fires? | Gates? | Cosmetic? | Notes |
|------|--------|--------|-----------|-------|
| PreToolUse Write/Edit | Yes | No | Partial | Calls pre-edit but continueOnError:true |
| PreToolUse Bash | Yes | No | Partial | Calls pre-command but continueOnError:true |
| PreToolUse Task | Yes | No | Partial | Registers task but no validation |
| PostToolUse Write/Edit | Yes | No | Cosmetic | Records outcome, no action |
| PostToolUse Bash | Yes | No | Cosmetic | Records outcome, no action |
| PostToolUse Task | Yes | No | Mostly cosmetic | Reflexion reminder is ignorable echo |
| UserPromptSubmit | Yes | No | Injector | Injects protocol hints, some contradictory |
| SessionStart | Yes | No | Useful | Daemon start, session restore, memory import |
| Stop | Yes | No | Cosmetic | Reflexion reminder echo |
| Notification | Yes | No | Cosmetic | Stores notification in memory |
| TeammateIdle | Yes | No | Cosmetic | Auto-assign attempt |
| TaskCompleted | Yes | No | Cosmetic | Train patterns, notify lead |

**Summary: Zero hooks are gates. All are advisory or cosmetic.**

### Skills Gap Analysis

| Skill | Referenced By | Actually Invoked? | Status |
|-------|--------------|-------------------|--------|
| `get-pattern` | All agent defs, CLAUDE.md, pattern-workflow.md | Yes, regularly | Active |
| `save-pattern` | All agent defs, CLAUDE.md, pattern-workflow.md | Yes, regularly | Active |
| `reflexion` | All agent defs, CLAUDE.md, pattern-workflow.md | Inconsistently | Gap: no enforcement |
| `validate` | implementation-protocol.md, testing.md | Yes, manually | Active |
| `align` | ndp-vision-guardian.md, planning-protocol.md | Rarely | Underused |
| `spec-compile` | implementation-protocol.md (conditional) | Sometimes | Gap: should be required |
| `swarm-run` | swarm-protocol.md | Yes | Active |
| `ndp-github-workflow` | All agent defs | Yes | Active |
| `learner` | pattern-workflow.md | Rarely | Underused |
| `verification-quality` | None (self-referencing only) | Never | Vaporware -- commands don't exist (report-05) |
| Remaining 34 skills | No NDP references | Never | Cognitive noise |

### Prompt-Check Hook (`prompt-check.sh`)

| Line | Finding | Severity |
|------|---------|----------|
| L35 | Injects `claude-flow swarm init --topology hierarchical` -- contradicts swarm-protocol.md | P0 |
| L59 | Same contradictory CLI injection for implementation swarms | P0 |
| L36 | Injects `claude-flow memory store --namespace {feature-id}` -- correct but mixes CLI and MCP conventions | P2 |
| L12 | Skip keywords include "reflexion" and "save-pattern" -- means pattern workflow prompts get no protocol hint | P2 |

---

## Cross-Cutting Issues

### 1. The swarm-init contradiction (P0)

Three files say different things about swarm initialization:

| Source | Instruction |
|--------|-------------|
| `swarm-protocol.md` L47 | "Do NOT use `claude-flow swarm init` CLI" |
| `ndp-scrum-master.md` L33 | "Run `claude-flow swarm init`" |
| `prompt-check.sh` L35, L59 | Outputs `claude-flow swarm init --topology hierarchical` |

**Fix**: Update ndp-scrum-master.md and prompt-check.sh to match swarm-protocol.md (MCP hive-mind_init only).

### 2. ADR format inconsistency (P1)

| Source | Format |
|--------|--------|
| `ndp-architect.md` | `# ADR-NNN` + `## Status/Context/Decision/Consequences/Alternatives` |
| `planning-protocol.md` | `## ADR-NNN` + `### Context/Decision/Consequences` |

**Fix**: Standardize on planning-protocol.md format. Update ndp-architect.md.

### 3. spec-compile is optional (P1)

`implementation-protocol.md` L129 treats spec-compile as conditional. DEV-ARCH-FLOW-PROPOSED.md explicitly says spec-compile should be REQUIRED. Without Level-1 summary, agents drift.

**Fix**: Make spec-compile required in implementation-protocol.md.

### 4. Pattern workflow is unenforced (P1)

get-pattern/reflexion are described as "REQUIRED" or "mandatory" in:
- CLAUDE.md (rule 1 and 2)
- pattern-workflow.md (entire file)
- All 16 agent definitions
- settings.json Stop hook

But zero mechanisms verify compliance. The Stop hook echo is ignorable. No post-task check verifies reflexion was called.

**Fix**: Post-task hook could check if reflexion skill was invoked during the session. Not fully enforceable in current hook model, but post-task artifact check can verify expected outputs.

### 5. Stale technology references (P1)

ndp-architect.md references Silver as "Planned" and Gold as "Future". Both are implemented (Silver: TimescaleDB hypertables with 908 tests; Gold: ndp-gold-ddl crate generates DDL).

**Fix**: Update the technology status table and data layer diagram in ndp-architect.md.

---

## Prioritized Fix List

### P0 (Causes agent failure or silent process skip)

| # | Fix | ACT-NNN | Files |
|---|-----|---------|-------|
| 1 | Fix swarm-init contradiction (scrum-master.md L33, prompt-check.sh L35/L59) | ACT-001 | `ndp-scrum-master.md`, `prompt-check.sh` |
| 2 | Identify hooks that should gate (pre-commit at minimum) | ACT-005 | `.claude/settings.json` |
| 3 | Remove contradictory CLI injection from prompt-check.sh | ACT-001 | `.claude/hooks/prompt-check.sh` |

### P1 (Degrades quality)

| # | Fix | ACT-NNN | Files |
|---|-----|---------|-------|
| 4 | Make spec-compile required (not conditional) | ACT-032 | `implementation-protocol.md` |
| 5 | Standardize ADR format (match planning-protocol.md) | ACT-033 | `ndp-architect.md` |
| 6 | Update stale technology status (Silver, Gold) | ACT-034 | `ndp-architect.md` |
| 7 | Add agent self-check blocks to all agent definitions | ACT-011 | All agent .md files |
| 8 | Add ACCEPTANCE-MAP.md + LAUNCH-PROMPT.md to planning deliverables | ACT-042 | `planning-protocol.md` |
| 9 | Add per-wave acceptance check (Step 3c.5) | ACT-012 | `implementation-protocol.md` |
| 10 | Fix memory budget in ndp-architect.md (<1GB vs ~5.5GB) | -- | `ndp-architect.md` |
| 11 | Add flaky test manifest reference to testing.md | ACT-009 | `.claude/rules/testing.md` |
| 12 | Add post-task artifact check (did planning produce expected files?) | ACT-032 | New hook script or skill check |

### P2 (Cleanup)

| # | Fix | ACT-NNN | Files |
|---|-----|---------|-------|
| 13 | Delete verification-quality SKILL.md (commands don't exist) | ACT-073 | `.claude/skills/verification-quality/` |
| 14 | Update ndp-tester.md test directory structure | ACT-103 | `ndp-tester.md` |
| 15 | Remove cargo-tarpaulin reference from ndp-tester.md | -- | `ndp-tester.md` |
| 16 | Note generic specification/pseudocode agents as known gap | -- | Documentation only |
| 17 | Clean up prompt-check.sh memory store CLI reference | -- | `prompt-check.sh` |
