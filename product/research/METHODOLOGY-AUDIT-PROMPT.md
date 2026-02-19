# Methodology Audit Prompt (Continuous Improvement)

Run this prompt periodically (after each feature, or monthly) to identify waste, gaps, and improvements in the NDP control plane methodology.

---

## How to Use

1. Paste this prompt into a new Claude Code conversation
2. Add your own observations in the `## User Learnings` section
3. Review the output and apply fixes
4. Update the `## Previous Findings` section with what was fixed

---

## The Prompt

```
Perform a methodology audit of the NDP control plane under this lens:
"Get planning and delivery teams the BEST information with the LEAST context window usage."

## Audit Scope

Read and analyze:
1. CLAUDE.md — always-on rules, token cost
2. .claude/rules/*.md — check paths: frontmatter, identify always-on vs conditional
3. .claude/agents/ndp/*.md — check agent definition sizes, redundancy
4. .claude/skills/*/SKILL.md — check sizes, identify bloat
5. .claude/commands/ — count, check for dead weight

## Analysis Framework

For each file in scope, answer:
- WHEN is this loaded? (always / conditional paths / on-demand)
- WHO needs this? (primary agent / scrum-master / worker agents / all)
- WHAT is the token cost? (word count × 1.3 ≈ tokens)
- IS there redundancy? (same content in multiple files)
- CAN it be narrowed? (more specific paths trigger, split into basic/advanced)

## Specific Checks

### Token Budget Audit
- Count words in all rules WITHOUT paths: frontmatter (these are always-on)
- Count words in CLAUDE.md
- Total = baseline context cost EVERY conversation
- Target: <1,500 words always-on (currently ~2,480)

### Redundancy Audit
- Search for identical sections across rule files (Three Memory Systems, Concurrency Rules, etc.)
- Search for content in CLAUDE.md that's duplicated in rules files
- Identify skills that duplicate content from rules (pattern-workflow vs get-pattern)

### Path Trigger Audit
- List every paths: trigger and what it loads
- Flag overly broad triggers (e.g., all .rs files loading swarm protocol)
- Flag cascade triggers (CLAUDE.md triggering protocol loads)

### Skill Size Audit
- List all SKILL.md files by word count
- Flag any >800 words (should be split into basic + advanced)
- Identify "Optional" or "Enhanced" sections that could be separate skills

### Dead Weight Audit
- List all skills/commands in .claude/skills/ and .claude/commands/
- Flag any not referenced by NDP agents, protocols, or workflows
- Count system-prompt listing overhead from non-NDP skills

### Reflexion ROI Audit
- Check agentdb_pattern_stats() — how many patterns exist?
- Check reflexion episode count — how many episodes recorded?
- Check causal_query() — how many causal edges exist?
- Check if learning session exists — is RL training active?
- Assess: is the reflexion overhead producing measurable retrieval improvement?

## User Learnings (INJECT YOUR OBSERVATIONS HERE)

<!-- Add your learnings between the markers below -->
<!-- Format: - [DATE] Observation: ... -->
<!-- Example: - [2026-02-20] Agent ndp-tester reads test-plan twice (once from brief, once from get-pattern) -->

LEARNINGS_START
- [2026-02-19] Initial audit completed. See product/research/methodology-review-2026-02-19.md for baseline.
- [2026-02-19] Three rules files (agent-behaviors, memory-commands, pattern-workflow) lack paths: frontmatter, costing ~1,738 words always-on.
- [2026-02-19] planning-protocol and swarm-protocol trigger on CLAUDE.md edits unnecessarily.
- [2026-02-19] get-pattern (1,647w) and reflexion (1,765w) skills carry ~65% optional/advanced content.
- [2026-02-19] 35 non-NDP skills and 168 commands pollute system-prompt listing in every conversation.
- [2026-02-19] Hive-mind ceremony required in planning but optional in implementation — inconsistent.
- [2026-02-19] 0 causal edges, no RL learning session — enhanced reflexion methods produce no value yet.
LEARNINGS_END

## Previous Findings (Track What's Fixed)

| Date | Finding | Status | Notes |
|------|---------|--------|-------|
| 2026-02-19 | 3 rules without paths: always loaded | OPEN | Add paths: to agent-behaviors, memory-commands, pattern-workflow |
| 2026-02-19 | CLAUDE.md in protocol path triggers | OPEN | Remove CLAUDE.md from planning-protocol and swarm-protocol paths |
| 2026-02-19 | Oversized get-pattern and reflexion skills | OPEN | Split into basic + advanced |
| 2026-02-19 | 35 non-NDP skills in listing | OPEN | Audit and archive |
| 2026-02-19 | Cross-protocol redundancy | OPEN | Consolidate to swarm-protocol base |
| 2026-02-19 | Broad implementation-protocol triggers | OPEN | Narrow to SPARC R/C phase paths |
| 2026-02-19 | Disproportionate reflexion mandate | OPEN | Proportional reflexion by task complexity |
| 2026-02-19 | Hive-mind inconsistency | OPEN | Make optional everywhere |

## Output Format

Produce a structured report:
1. **Baseline Metrics**: Total always-on words, conditional words by trigger, skill count
2. **Changes Since Last Audit**: What was fixed, what's new
3. **New Findings**: Issues not in Previous Findings
4. **User Learning Validation**: Which user observations are confirmed, which need investigation
5. **Recommended Actions**: Prioritized by token savings, with specific file edits
6. **Updated Previous Findings Table**: With current status
```

---

## Cadence

- **After each feature delivery**: Quick audit (focus on Reflexion ROI + any new rules/skills added)
- **Monthly**: Full audit (all checks)
- **After methodology changes**: Targeted audit (verify the change didn't introduce new waste)
