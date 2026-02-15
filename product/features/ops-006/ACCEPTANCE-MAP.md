# ops-006 Acceptance Criteria Map

| AC-ID | Description | Verification Method | Verification Detail | Status |
|-------|-------------|--------------------|--------------------|--------|
| AC-01 | Adherence audit completed -- prioritized gap list with before/after fixes | file-check | `product/features/ops-006/specification/ADHERENCE-AUDIT.md` exists with P0/P1/P2 findings | PENDING |
| AC-02 | Agent definitions updated -- no "should" where "must" intended, no stale references | grep | `grep -c 'should' .claude/agents/ndp/*.md` shows only intentional uses; no deprecated pattern IDs 29/32 in .claude/ | PENDING |
| AC-03 | /validate-plan checks planning artifacts exist, internally consistent, valid patterns | file-check | `.claude/skills/validate-plan/SKILL.md` exists with 5 checks documented | PENDING |
| AC-04 | /validate-impl produces structured glass box report covering Tiers 1-4 | grep | `.claude/skills/validate/SKILL.md` contains "Tier 2", "Tier 3", "Tier 4" sections | PENDING |
| AC-05 | Both validation reports include NOT CHECKED and RECOMMENDED HUMAN REVIEW sections | grep | Both validate SKILL.md files contain "NOT CHECKED" and "RECOMMENDED HUMAN REVIEW" in report template | PENDING |
| AC-06 | Risk classification assigns LOW/MEDIUM/HIGH based on diff characteristics | grep | validate/SKILL.md Tier 4 section contains scope/depth/domain matrix with LOW/MEDIUM/HIGH | PENDING |
| AC-06a | Process adherence checks run in /validate-impl | grep | validate/SKILL.md Tier 2 contains: banned deps, stub scan, file scope, stale refs, config valid | PENDING |
| AC-07 | Pre-commit hook blocks commits with format violations or stubs | shell | `grep 'continueOnError.*false' .claude/settings.json` returns match for pre-commit hook | PENDING |
| AC-08 | Post-task hook verifies expected artifacts exist for task type | file-check | `.claude/hooks/post-task-check.sh` exists and is referenced in settings.json | PENDING |
| AC-09 | Pre-spawn hook prevents implementation swarm on broken workspace | grep | `implementation-protocol.md` pre-spawn checklist contains "cargo build --workspace" | PENDING |
| AC-10 | ACCEPTANCE-MAP.md format defined and documented in planning protocol | grep | `planning-protocol.md` contains "ACCEPTANCE-MAP.md" in deliverables section | PENDING |
| AC-11 | Planning swarm produces LAUNCH-PROMPT.md with proposed implementation kickoff prompt | grep | `planning-protocol.md` contains "LAUNCH-PROMPT.md" in deliverables section | PENDING |
| AC-12 | /trust-dashboard renders per-check Bayesian trust scores from AgentDB | file-check | `.claude/skills/trust-dashboard/SKILL.md` exists with Beta score computation | PENDING |
| AC-13 | /validate writes trust reflexion entries to AgentDB after each run | grep | `validate/SKILL.md` contains "reflexion_store" and "trust:validation" | PENDING |
| AC-14 | /shadow-judge records human judgment and stores comparison in AgentDB | file-check | `.claude/skills/shadow-judge/SKILL.md` exists with approve/reject commands | PENDING |
| AC-15 | Trust snapshot exports to .deploy/trust/vX.Y.Z.json at release time | file-check | `.deploy/trust/` directory exists; release docs reference trust snapshot | PENDING |
| AC-16 | Flaky tests separated from real failures in validation output | file-check | `.ndp/flaky-tests.txt` exists; validate/SKILL.md references flaky manifest | PENDING |
| AC-17 | Scope pre-check flags vision misalignment before planning starts | grep | `planning-protocol.md` Phase 1 contains scope pre-check step | PENDING |
| AC-18 | Test count baseline stored; /validate warns on regression | file-check | `.ndp/test-baseline.txt` exists with value 908; validate/SKILL.md compares against baseline | PENDING |
