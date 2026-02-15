# Specification: ops-006 Validation Pipeline & Trust Infrastructure

> Date: 2026-02-15
> Source: SCOPE.md (6 workstreams, 18 ACs)
> Approach: All changes are to .md, .sh, .json, SKILL.md files. No Rust code.

---

## Workstream 1: Cleanup & Baselines

### WS1-01: Delete legacy build files (AC: baseline)

| File | Change | Why |
|------|--------|-----|
| `Makefile` | DELETE | References non-existent Neural Trader binaries |
| `Makefile.v2` | DELETE | Same -- stale build artifacts |
| `.cargo/config.toml` | EDIT: remove `autonomous-platform` references, gate `instrument-coverage` behind env var | Prevents build confusion on clean checkout |

### WS1-02: Test baseline and flaky test management (AC-16, AC-18)

| File | Change | Why |
|------|--------|-----|
| `.ndp/test-baseline.txt` | CREATE: contains `908` | Stores current passing test count |
| `.ndp/flaky-tests.txt` | CREATE: lists 6 known flaky tests | Separates flaky from real failures |
| `.claude/rules/testing.md` | EDIT: add flaky test manifest reference and baseline concept | Documents the baseline/flaky system |

---

## Workstream 2: Adherence Audit & Definition Tightening

### WS2-01: Adherence audit (AC-01)

| File | Change | Why |
|------|--------|-----|
| `product/features/ops-006/specification/ADHERENCE-AUDIT.md` | CREATE (planning deliverable) | Prioritized gap list with before/after |

### WS2-02: Agent definition updates (AC-02)

| File | Change | Why |
|------|--------|-----|
| `.claude/agents/ndp/ndp-scrum-master.md` L33 | EDIT: `claude-flow swarm init` -> `MCP hive-mind_init` | Fix P0 contradiction (ACT-001) |
| `.claude/agents/ndp/ndp-architect.md` L49-50 | EDIT: Silver "Planned"->"Current", Gold "Future"->"Current" | Fix stale status (ACT-034) |
| `.claude/agents/ndp/ndp-architect.md` L69-87 | EDIT: ADR format to match planning-protocol.md | Fix inconsistency (ACT-033) |
| `.claude/agents/ndp/ndp-architect.md` L99 | EDIT: Silver Storage "Planned"->"Current" | Fix stale tech table |
| `.claude/agents/ndp/ndp-architect.md` L120 | EDIT: memory budget "<1GB" -> "~5.5GB typical" | Fix incorrect constraint |
| `.claude/agents/ndp/ndp-rust-dev.md` | EDIT: add SELF-CHECK section before Pattern Integration | Agent self-validates output (ACT-011) |
| `.claude/agents/ndp/ndp-tester.md` | EDIT: add SELF-CHECK section, update test directory structure | Self-validates + fix stale refs (ACT-103) |
| `.claude/agents/ndp/ndp-architect.md` | EDIT: add SELF-CHECK section | Self-validates against constraints |
| `.claude/agents/ndp/ndp-vision-guardian.md` | EDIT: add SELF-CHECK section | Self-validates alignment report |
| All agent .md files (16) | EDIT: replace "should" with "must" where enforcement is intended | Remove ambiguity across all definitions |

### WS2-03: Rules and protocol updates (AC-02, AC-10)

| File | Change | Why |
|------|--------|-----|
| `.claude/rules/implementation-protocol.md` L129 | EDIT: make spec-compile required, not conditional | Fix P1 gap (close "if run" loophole) |
| `.claude/rules/implementation-protocol.md` | EDIT: add Step 3c.5 per-wave acceptance check | Per-wave AC mapping (ACT-012) |
| `.claude/rules/implementation-protocol.md` | EDIT: add `cargo build --workspace` to pre-spawn checklist | Pre-spawn compile check (AC-09) |
| `.claude/rules/planning-protocol.md` | EDIT: add ACCEPTANCE-MAP.md to Step 3f deliverables | Planning produces AC map (ACT-042) |
| `.claude/rules/planning-protocol.md` | EDIT: add LAUNCH-PROMPT.md to Step 3f deliverables | Planning produces launch prompt |
| `.claude/rules/planning-protocol.md` | EDIT: add validate-plan step after Step 3f | Run plan validation before GH Issue |
| `.claude/rules/swarm-protocol.md` | EDIT: add agent self-check block to agent prompt template | Standardize self-check (ACT-011) |
| `.claude/rules/testing.md` | EDIT: add baseline/flaky references | Document test infrastructure |

### WS2-04: Scope pre-check (AC-17)

| File | Change | Why |
|------|--------|-----|
| `.claude/rules/planning-protocol.md` Phase 1 | EDIT: add scope pre-check step (7-principle scan of SCOPE.md) | Catch misalignment before planning starts (ACT-010) |

---

## Workstream 3: Hook Enforcement

### WS3-01: Pre-commit quality gate (AC-07)

| File | Change | Why |
|------|--------|-----|
| `.claude/hooks/pre-commit-gate.sh` | CREATE: cargo fmt --check + anti-stub grep on staged .rs files | Blocks commit on violations |
| `.claude/settings.json` | EDIT: add PreToolUse Bash hook for git commit with `continueOnError: false` | Gating hook for commits |

### WS3-02: Post-task adherence check (AC-08)

| File | Change | Why |
|------|--------|-----|
| `.claude/hooks/post-task-check.sh` | CREATE: checks expected artifacts exist by task type | Reports missing deliverables |
| `.claude/settings.json` | EDIT: add PostToolUse Task hook calling post-task-check.sh | Fires after agent completion |

### WS3-03: Pre-spawn workspace check (AC-09)

| File | Change | Why |
|------|--------|-----|
| `.claude/rules/implementation-protocol.md` | EDIT: add cargo build to pre-spawn checklist (protocol step, not hook) | Fail fast on broken workspace |

### WS3-04: Pre-commit test count regression (AC-18)

| File | Change | Why |
|------|--------|-----|
| `.claude/hooks/pre-commit-gate.sh` | EDIT: add test count comparison against baseline | Warns on test regression |

---

## Workstream 4: Validation Skill Split + Upgrade

### WS4-01: validate-plan skill (AC-03)

| File | Change | Why |
|------|--------|-----|
| `.claude/skills/validate-plan/SKILL.md` | CREATE: 5-check planning validation skill | Validates planning artifacts |

### WS4-02: validate-impl skill upgrade (AC-04, AC-06, AC-06a)

| File | Change | Why |
|------|--------|-----|
| `.claude/skills/validate/SKILL.md` | EDIT: upgrade from 3-tier to 4-tier, add process adherence + spec compliance + risk classification | Glass box validation |

### WS4-03: Glass box report output (AC-05)

| File | Change | Why |
|------|--------|-----|
| `.claude/skills/validate/SKILL.md` | EDIT: add glass box report template to output section | Structured per-check reporting |
| `.claude/skills/validate-plan/SKILL.md` | EDIT: add glass box report template | Same format for plan validation |

---

## Workstream 5: Planning Swarm Deliverables

### WS5-01: ACCEPTANCE-MAP.md format and protocol (AC-10)

| File | Change | Why |
|------|--------|-----|
| `.claude/rules/planning-protocol.md` | EDIT: add ACCEPTANCE-MAP.md to specification agent output | Defines format, adds to protocol |

### WS5-02: Implementation launch prompt (AC-11)

| File | Change | Why |
|------|--------|-----|
| `.claude/rules/planning-protocol.md` | EDIT: add LAUNCH-PROMPT.md generation step | Planning produces launch prompt |

### WS5-03: Per-wave acceptance check (AC covered by WS2-03)

Already addressed in WS2-03 (implementation-protocol.md Step 3c.5).

---

## Workstream 6: Trust Infrastructure

### WS6-01: trust-dashboard skill (AC-12)

| File | Change | Why |
|------|--------|-----|
| `.claude/skills/trust-dashboard/SKILL.md` | CREATE: queries AgentDB reflexion, computes Beta scores, renders dashboard | Trust visibility |

### WS6-02: Trust score recording in validate (AC-13)

| File | Change | Why |
|------|--------|-----|
| `.claude/skills/validate/SKILL.md` | EDIT: add trust recording step (reflexion_store with trust:validation:* prefix) | Writes trust data after validation |

### WS6-03: shadow-judge skill (AC-14)

| File | Change | Why |
|------|--------|-----|
| `.claude/skills/shadow-judge/SKILL.md` | CREATE: approve/reject commands, comparison logic, AgentDB storage | Human judgment recording |

### WS6-04: Trust snapshot at release (AC-15)

| File | Change | Why |
|------|--------|-----|
| `.claude/rules/implementation-protocol.md` or release docs | EDIT: add trust snapshot export step to release workflow | Audit trail |
| `.deploy/trust/` | CREATE directory for trust snapshots | Git-tracked trust progression |

---

## Cleanup Items (from audit, no specific WS)

| File | Change | Why |
|------|--------|-----|
| `.claude/hooks/prompt-check.sh` L35, L59 | EDIT: replace `claude-flow swarm init` with MCP reference | Fix P0 contradiction |
| `.claude/skills/verification-quality/` | DELETE entire directory | Commands don't exist (report-05, ACT-073) |
| `CLAUDE.md` | EDIT: add reference to validate-plan and validate-impl skills | Reflect new skill split |

---

## File Change Summary

| Action | Count | Files |
|--------|-------|-------|
| CREATE | 8 | pre-commit-gate.sh, post-task-check.sh, validate-plan/SKILL.md, trust-dashboard/SKILL.md, shadow-judge/SKILL.md, test-baseline.txt, flaky-tests.txt, .deploy/trust/ |
| EDIT | ~20 | All agent defs (16), settings.json, planning-protocol.md, implementation-protocol.md, swarm-protocol.md, testing.md, validate/SKILL.md, prompt-check.sh, CLAUDE.md |
| DELETE | 3 | Makefile, Makefile.v2, verification-quality/ |
| TOTAL | ~31 | |

---

## AC Coverage Matrix

| AC | WS | Task | Verification |
|----|-----|------|-------------|
| AC-01 | WS2-01 | ADHERENCE-AUDIT.md | File exists with prioritized gaps |
| AC-02 | WS2-02 | Agent def updates | No "should" where "must" intended, no stale refs |
| AC-03 | WS4-01 | validate-plan skill | Skill runs, produces PASS/FAIL report |
| AC-04 | WS4-02 | validate-impl upgrade | 4-tier glass box report produced |
| AC-05 | WS4-03 | Glass box format | Reports include NOT CHECKED + RECOMMENDED REVIEW |
| AC-06 | WS4-02 | Risk classification | LOW/MEDIUM/HIGH assigned |
| AC-06a | WS4-02 | Process adherence | Banned deps, stub scan, file scope, stale refs run |
| AC-07 | WS3-01 | Pre-commit hook | Blocks on fmt violation or stub |
| AC-08 | WS3-02 | Post-task hook | Verifies expected artifacts |
| AC-09 | WS3-03 | Pre-spawn check | cargo build before agent spawn |
| AC-10 | WS5-01 | ACCEPTANCE-MAP.md | Format defined, in planning protocol |
| AC-11 | WS5-02 | LAUNCH-PROMPT.md | Planning produces launch prompt |
| AC-12 | WS6-01 | trust-dashboard | Renders Bayesian trust scores |
| AC-13 | WS6-02 | Trust recording | validate writes trust reflexion entries |
| AC-14 | WS6-03 | shadow-judge | Human judgment recorded, comparison stored |
| AC-15 | WS6-04 | Trust snapshot | Exports to .deploy/trust/vX.Y.Z.json |
| AC-16 | WS1-02 | Flaky tests | Separated from real failures |
| AC-17 | WS2-04 | Scope pre-check | Flags vision misalignment |
| AC-18 | WS1-02 + WS3-04 | Test baseline | Stored; /validate warns on regression |
