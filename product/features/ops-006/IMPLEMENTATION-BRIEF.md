# Implementation Brief: ops-006 Validation Pipeline & Trust Infrastructure

## SPARC Artifacts

| Artifact | Path |
|----------|------|
| Scope | product/features/ops-006/SCOPE.md |
| Adherence Audit | product/features/ops-006/specification/ADHERENCE-AUDIT.md |
| Specification | product/features/ops-006/specification/SPECIFICATION.md |
| Task Decomposition | product/features/ops-006/specification/TASK-DECOMPOSITION.md |
| Architecture (ADRs) | product/features/ops-006/architecture/ARCHITECTURE.md |
| Alignment Report | product/features/ops-006/ALIGNMENT-REPORT.md |
| Acceptance Map | product/features/ops-006/ACCEPTANCE-MAP.md |
| Launch Prompt | product/features/ops-006/LAUNCH-PROMPT.md |

## Goal

Tighten the NDP development process so it actually works, then instrument it for observation and trust building. Agent definitions, rules, hooks, and skills will be updated to close adherence gaps identified by a comprehensive audit. Hooks will enforce (not just remind) critical process steps. Two new validation skills (/validate-plan and /validate-impl with 4 tiers) will produce glass box reports showing exactly what was checked. Trust scores will accumulate in AgentDB so the user can track improvement over time via /trust-dashboard.

This is entirely definition/rule/skill/hook file edits. No Rust code. No external tools. ~31 files created, modified, or deleted across 3 implementation waves.

## GitHub Issue

https://github.com/dug-21/neural-data-platform/issues/19

## Resolved Decisions

| Decision | Resolution | Source | Pattern ID |
|----------|-----------|--------|------------|
| validate-plan structure | 5 checks: artifacts exist, AC coverage, pattern IDs resolve, no stale refs, internal consistency | ADR-001 | 25 |
| validate-impl structure | 4 tiers: compilation, process adherence, spec compliance, risk classification | ADR-002 | 26 |
| Hook enforcement model | Pre-commit BLOCKS (continueOnError:false); post-task WARNS; pre-spawn is protocol step | ADR-003 | 27 |
| Trust storage | AgentDB reflexion entries with trust:validation:{tier}:{check} prefix; Beta scoring | ADR-004 | 28 |
| trust-dashboard design | Query reflexion, compute Beta per check, composite score, render dashboard | ADR-005 | 29 |
| shadow-judge design | approve/reject commands; stores reward=1.0 or 0.0; same reflexion table | ADR-006 | 30 |
| ACCEPTANCE-MAP.md format | Table with AC-ID, description, verification method (test/manual/file-check/grep/shell), status | ADR-007 | 31 |
| LAUNCH-PROMPT.md format | Proposed prompt, reminders, gotchas, deliverables table; user reviews before pasting | ADR-008 | 32 |
| Glass box report format | Per-check PASS/FAIL with evidence, NOT CHECKED section, RECOMMENDED HUMAN REVIEW, confidence score | ADR-009 | 33 |
| Test baseline and flaky management | .ndp/test-baseline.txt (908), .ndp/flaky-tests.txt (6 tests); validate compares and separates | ADR-010 | 34 |

## Files to Create/Modify

### CREATE (8 files)

| Path | Purpose |
|------|---------|
| `.claude/hooks/pre-commit-gate.sh` | Pre-commit quality gate: cargo fmt --check + anti-stub grep + test regression warning |
| `.claude/hooks/post-task-check.sh` | Post-task artifact existence check by task type |
| `.claude/skills/validate-plan/SKILL.md` | Planning validation skill (5 checks) |
| `.claude/skills/trust-dashboard/SKILL.md` | Trust score dashboard (queries AgentDB, computes Beta scores) |
| `.claude/skills/shadow-judge/SKILL.md` | Human judgment recording (approve/reject) |
| `.ndp/test-baseline.txt` | Test count baseline (908) |
| `.ndp/flaky-tests.txt` | Known flaky test manifest (6 tests) |
| `.deploy/trust/` | Directory for trust snapshots at release |

### MODIFY (~20 files)

| Path | Change Summary |
|------|---------------|
| `.claude/agents/ndp/ndp-scrum-master.md` | Fix swarm-init contradiction (L33: CLI -> MCP) |
| `.claude/agents/ndp/ndp-architect.md` | Fix stale status (Silver/Gold), ADR format, memory budget; add SELF-CHECK |
| `.claude/agents/ndp/ndp-rust-dev.md` | Add SELF-CHECK section |
| `.claude/agents/ndp/ndp-tester.md` | Add SELF-CHECK section; update test directory structure |
| `.claude/agents/ndp/ndp-vision-guardian.md` | Add SELF-CHECK section |
| All 16 agent .md files | Replace "should" with "must" where enforcement intended; remove stale refs |
| `.claude/rules/implementation-protocol.md` | Make spec-compile required; add Step 3c.5; add pre-spawn build check |
| `.claude/rules/planning-protocol.md` | Add scope pre-check; add ACCEPTANCE-MAP.md + LAUNCH-PROMPT.md + validate-plan steps |
| `.claude/rules/swarm-protocol.md` | Add agent self-check block to prompt template |
| `.claude/rules/testing.md` | Add baseline/flaky references |
| `.claude/settings.json` | Add pre-commit gate (continueOnError:false); add post-task check |
| `.claude/hooks/prompt-check.sh` | Fix swarm-init CLI contradiction (L35, L59) |
| `.claude/skills/validate/SKILL.md` | Upgrade 3-tier to 4-tier; add glass box report; add trust recording |
| `CLAUDE.md` | Reference validate-plan, validate-impl skills |

### DELETE (3 items)

| Path | Reason |
|------|--------|
| `Makefile` | References non-existent Neural Trader binaries |
| `Makefile.v2` | Same -- stale build artifact |
| `.claude/skills/verification-quality/` | Commands don't exist (report-05 confirmed) |

## Wave Sequencing

### Wave 1: Cleanup + Audit + Definition Tightening (WS1 + WS2)
- 13 tasks, mostly parallelizable
- Delete stale files, create baselines, fix agent definitions, update protocols
- ACs covered: AC-01, AC-02, AC-09, AC-10, AC-11, AC-16, AC-17, AC-18

### Wave 2: Hook Enforcement + Validation Skills (WS3 + WS4 + WS5)
- 6 tasks, partially parallelizable
- Create hook scripts, update settings.json, create validate-plan skill, upgrade validate-impl
- ACs covered: AC-03, AC-04, AC-05, AC-06, AC-06a, AC-07, AC-08

### Wave 3: Trust Infrastructure (WS6)
- 5 tasks, partially parallelizable
- Create trust-dashboard, shadow-judge skills; add trust recording to validate
- ACs covered: AC-12, AC-13, AC-14, AC-15

## Constraints

- No new Rust code -- all changes are .md, .sh, .json, SKILL.md files
- No external tools (just, cargo-deny, cargo-nextest) -- all checks via Claude Code hooks and skills
- No new agent definitions -- tighten existing ones only
- No CI/CD -- all validation is local through Claude Code
- No Tier 5 LLM judge -- deferred to Phase C
- No auto-approve or risk-gated mode -- ops-006 is observation only
- All validation checks in skills are shell commands, not compiled Rust tests
- Trust scores use existing AgentDB reflexion_store (no new database/table)

## Dependencies

- fe-003 (complete): AgentDB + reflexion infrastructure exists
- ops-004 (complete): WAL-only Bronze, 908 tests passing baseline
- AgentDB MCP: reflexion_store/reflexion_retrieve must support prefix queries on task field

## NOT in Scope

- New Rust code or compiled tests
- External tools (just, cargo-deny, cargo-nextest)
- Tier 5 LLM judge
- Auto-approve / risk-gated autonomy mode
- Concurrent swarm support
- Agent evolution / /learner updates
- GitHub Actions CI
- New agent definitions
- Branch-per-feature workflow

## Alignment Status

All 7 alignment principles: PASS or N/A. No variances requiring approval. See product/features/ops-006/ALIGNMENT-REPORT.md.

## Research Base

All design decisions informed by prior research in product/ndp-dev-auto/:
- 01-protocol-agent-evaluation.md (protocol gaps, contradictions, agent roster)
- 04-early-validation.md (shift-left validation, self-checks, acceptance mapping)
- 05-truth-verify-evaluation.md (hook effectiveness, claims system, vaporware skill)
- 06-strategic-recommendations.md (test baseline, stale refs, pattern maintenance)
- research-validation-confidence.md (Bayesian trust, Beta distribution)
- research-progressive-autonomy.md (autonomy levels, shadow mode)
- DEV-ARCH-FLOW-PROPOSED.md (5-tier validation, trust model, glass box reports)
- PROPOSED-ACTIONS.md (ACT-NNN items mapped to specific improvements)
