# Task Decomposition: ops-006

> Grouped by implementation wave. Tasks within a wave can run in parallel unless noted.

---

## Wave 1: Cleanup + Audit + Definition Tightening (WS1 + WS2)

| Task | Description | Input | Output | AC | Size |
|------|-------------|-------|--------|-----|------|
| W1-01 | Delete Makefile, Makefile.v2 | git ls-files | Files deleted | baseline | S |
| W1-02 | Fix .cargo/config.toml | .cargo/config.toml | Cleaned config | baseline | S |
| W1-03 | Create .ndp/test-baseline.txt and .ndp/flaky-tests.txt | MEMORY.md (test counts, known flaky) | Two files | AC-16, AC-18 | S |
| W1-04 | Fix ndp-scrum-master.md swarm-init contradiction | ndp-scrum-master.md, audit findings | Updated agent def | AC-02 | S |
| W1-05 | Fix ndp-architect.md (stale status, ADR format, memory budget) | ndp-architect.md, audit findings | Updated agent def | AC-02 | M |
| W1-06 | Add SELF-CHECK sections to ndp-rust-dev, ndp-tester, ndp-architect, ndp-vision-guardian | Agent defs, report-04 self-check design | 4 updated agent defs | AC-02 | M |
| W1-07 | Update remaining agent defs: should->must, remove stale references | All 16 agent .md files, audit | Updated defs | AC-02 | M |
| W1-08 | Fix prompt-check.sh (remove CLI contradiction) | prompt-check.sh | Updated hook | AC-02 | S |
| W1-09 | Update implementation-protocol.md (spec-compile required, Step 3c.5, pre-spawn build) | implementation-protocol.md | Updated protocol | AC-09 | M |
| W1-10 | Update planning-protocol.md (scope pre-check, ACCEPTANCE-MAP.md, LAUNCH-PROMPT.md, validate-plan step) | planning-protocol.md | Updated protocol | AC-10, AC-11, AC-17 | M |
| W1-11 | Update swarm-protocol.md (agent self-check block in prompt template) | swarm-protocol.md | Updated protocol | AC-02 | S |
| W1-12 | Update testing.md (baseline/flaky references) | testing.md | Updated rules | AC-16, AC-18 | S |
| W1-13 | Delete verification-quality skill directory | .claude/skills/verification-quality/ | Directory removed | cleanup | S |

**Parallelism:** W1-01 through W1-03 are independent (file cleanup). W1-04 through W1-08 are independent (agent/hook fixes). W1-09 through W1-12 are independent (protocol updates). W1-13 is independent.

**Wave 1 verification:**
- All agent defs pass grep for "should" -> only intentional uses remain
- No deprecated pattern IDs (29, 32) in .claude/ files
- prompt-check.sh contains no `claude-flow swarm init` references
- .ndp/ directory exists with baseline and flaky manifest
- Makefile, Makefile.v2 deleted
- implementation-protocol.md contains Step 3c.5 and required spec-compile

---

## Wave 2: Hook Enforcement + Validation Skills + Planning Deliverables (WS3 + WS4 + WS5)

| Task | Description | Input | Output | AC | Size |
|------|-------------|-------|--------|-----|------|
| W2-01 | Create pre-commit-gate.sh (fmt check + stub scan + test regression warning) | ADR-003 | New hook script | AC-07, AC-18 | M |
| W2-02 | Create post-task-check.sh (artifact existence check by task type) | ADR-003 | New hook script | AC-08 | M |
| W2-03 | Update settings.json (add pre-commit gate with continueOnError:false, add post-task check) | ADR-003, settings.json | Updated settings | AC-07, AC-08 | M |
| W2-04 | Create validate-plan/SKILL.md | ADR-001 | New skill | AC-03 | L |
| W2-05 | Upgrade validate/SKILL.md to 4-tier with glass box report | ADR-002, ADR-009 | Updated skill | AC-04, AC-05, AC-06, AC-06a | L |
| W2-06 | Update CLAUDE.md (reference validate-plan, validate-impl) | CLAUDE.md | Updated project instructions | doc | S |

**Parallelism:** W2-01 through W2-03 can run in parallel (hooks). W2-04 and W2-05 can run in parallel (skills). W2-06 depends on W2-04 and W2-05 (needs skill names).

**Wave 2 verification:**
- `.claude/hooks/pre-commit-gate.sh` exists and is executable
- `.claude/hooks/post-task-check.sh` exists and is executable
- settings.json contains pre-commit hook with `continueOnError: false`
- `.claude/skills/validate-plan/SKILL.md` exists
- validate/SKILL.md contains Tier 2 (process adherence), Tier 3 (spec compliance), Tier 4 (risk classification)
- Both skills produce glass box report format with NOT CHECKED and RECOMMENDED HUMAN REVIEW sections

---

## Wave 3: Trust Infrastructure (WS6)

| Task | Description | Input | Output | AC | Size |
|------|-------------|-------|--------|-----|------|
| W3-01 | Create trust-dashboard/SKILL.md | ADR-005 | New skill | AC-12 | L |
| W3-02 | Create shadow-judge/SKILL.md | ADR-006 | New skill | AC-14 | M |
| W3-03 | Add trust recording step to validate/SKILL.md | ADR-004 | Updated skill | AC-13 | M |
| W3-04 | Add trust snapshot export to release workflow | ADR-004 | Updated protocol/docs | AC-15 | S |
| W3-05 | Create .deploy/trust/ directory | ADR-004 | New directory | AC-15 | S |

**Parallelism:** W3-01 and W3-02 can run in parallel (independent skills). W3-03 depends on Wave 2 completion (validate/SKILL.md must be in final 4-tier state). W3-04 and W3-05 are independent.

**Wave 3 verification:**
- `.claude/skills/trust-dashboard/SKILL.md` exists with Beta score computation
- `.claude/skills/shadow-judge/SKILL.md` exists with approve/reject commands
- validate/SKILL.md contains trust recording step using reflexion_store
- `.deploy/trust/` directory exists
- Release workflow documentation references trust snapshot export

---

## Summary

| Wave | Tasks | Parallelizable | Key Deliverables |
|------|-------|----------------|-----------------|
| Wave 1 | 13 | Yes (3 groups) | Fixed agent defs, cleaned files, updated protocols |
| Wave 2 | 6 | Partial (2 groups) | Hook scripts, validation skills, settings.json |
| Wave 3 | 5 | Partial | Trust skills, trust recording, snapshot infrastructure |
| **Total** | **24** | | **~31 files created/modified/deleted** |
