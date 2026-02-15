# ops-006: Validation Pipeline & Trust Infrastructure

## Vision

Today, validation is L0 — I do it manually. The rules exist — agent definitions, protocols, hooks, skills — but adherence is partial. Agents sometimes skip `/get-pattern`, omit `/reflexion`, leave stubs, drift from scope, or ignore the Level-1 summary. There's no way to measure how well the process is being followed, and no enforcement beyond honor-system reminders.

The goal of ops-006 is to **tighten the existing development process so it actually works**, then **instrument it so I can observe and build confidence**. When this is done:
- Agent definitions, rules, hooks, and skills will be updated to close adherence gaps
- Hooks will enforce (not just remind) critical process steps
- `/validate` will produce glass box reports showing process + code quality
- Trust scores will accumulate in AgentDB so I can track improvement

**Implementation approach**: Careful, targeted updates to agent definitions (`.claude/agents/ndp/`), rules (`.claude/rules/`), skills (`.claude/skills/`), hooks (`.claude/settings.json`), and CLAUDE.md. No new Rust code. No external tools. All development runs through Claude Code + claude-flow.

**ALWAYS BALANCE precision with Context Window noise.  Concise/complete direction wins**

This maps to **Phase A (Foundation) + Phase B (Shadow Mode)** from `product/ndp-dev-auto/DEV-ARCH-FLOW-PROPOSED.md`.
Current state development process is mapped in `product/ndp-dev-auto/ARCHITECTURE-FLOW.md`.  

## Tracking

- GitHub Issue: https://github.com/dug-21/neural-data-platform/issues/19
- Research: `product/ndp-dev-auto/` (reports 01-07, PROPOSED-ACTIONS.md, DEV-ARCH-FLOW-PROPOSED.md)

## Scope

### Workstream 1: Cleanup & Baselines

Remove broken artifacts and establish measurable baselines.

**WS1-01: Delete legacy build files** (ACT-006, ACT-007)
- Delete `Makefile` and `Makefile.v2` (reference non-existent Neural Trader binaries)
- Fix `.cargo/config.toml` — remove `autonomous-platform` references, gate `instrument-coverage` behind env var

**WS1-02: Test baseline and flaky test management** (ACT-008, ACT-009)
- Store test count baseline (AgentDB pattern or config file)
- Create flaky test manifest listing known flaky tests (5 wiremock + `acceptance_partition_structure`)
- `/validate` warns on test count regression; separates flaky from real failures

### Workstream 2: Adherence Audit & Definition Tightening

The core problem: rules exist but aren't consistently followed. This workstream audits what's broken and fixes it at the source.

**WS2-01: Adherence audit** (planning swarm research task)
- Audit every agent definition in `.claude/agents/ndp/` — identify where instructions are vague, optional, or contradicted
- Audit `.claude/rules/` — identify rules that are advisory vs enforceable, find gaps between what protocols say and what agents do
- Audit `.claude/settings.json` hooks — what actually fires, what's cosmetic, what could gate
- Audit `.claude/skills/` — which skills are referenced by protocols but never invoked
- Cross-reference with research report 01 (protocol evaluation, 3.5/5, 6 contradictions found)
- Deliverable: prioritized list of definition/rule changes with before/after examples

**WS2-02: Agent definition updates**
- Tighten agent definitions based on audit findings
- Remove ambiguity — replace "should" with "must", eliminate contradictory instructions
- Add mandatory steps (e.g., `/get-pattern` as step 1, `/reflexion` as final step) as numbered sequences, not suggestions
- Ensure every agent definition references the correct protocol files
- Remove references to deprecated patterns (IDs 29, 32), stale file paths, non-existent features

**WS2-03: Rules and protocol updates**
- Close gaps identified in audit — make spec-compile truly required (not "if run")
- Standardize protocol structure so all protocols follow the same enforcement pattern
- Add the self-check block to swarm-protocol.md agent prompt template
- Add LAUNCH-PROMPT.md to planning protocol deliverables
- Add ACCEPTANCE-MAP.md to planning protocol deliverables
- Remove stale/contradictory references (ACT-001, ACT-002, ACT-033, ACT-034)

**WS2-04: Scope pre-check** (ACT-010)
- When SCOPE.md is written, primary agent checks against ALIGNMENT-CRITERIA.md
- Lightweight version of `/align` — scope only, not full SPARC artifacts
- Implemented as rule or hook, not a new tool

### Workstream 3: Hook Enforcement

Upgrade hooks from advisory reminders to actual gates where it matters.

**WS3-01: Pre-commit quality gate**
- Pre-command hook on `git commit`: `cargo fmt --check` + anti-stub grep on staged `.rs` files
- Blocks the commit if checks fail (not just warns)

**WS3-02: Post-task adherence check**
- Post-task hook verifies agent produced expected artifacts for the task type
- For planning tasks: did IMPLEMENTATION-BRIEF.md, ACCEPTANCE-MAP.md, LAUNCH-PROMPT.md get created?
- For implementation tasks: did `/reflexion` get called? Were stubs introduced?
- Reports violations — doesn't silently pass

**WS3-03: Pre-spawn workspace check** (ACT-055)
- Pre-task hook on implementation swarm: `cargo build --workspace` must pass
- Fail fast if workspace doesn't compile — don't waste agent time

**WS3-04: Pre-command hook on git commit — test count regression**
- Compare test count against stored baseline before allowing commit
- Warn (not block) if test count decreased

### Workstream 4: Validation Skill Split + Upgrade

Split current `/validate` into two phase-specific skills. They run at different times, check different artifacts, and map to different post-task hooks.

**WS4-01: `/validate-plan` skill** (NEW — runs after planning swarm)
- Required artifacts exist? (IMPLEMENTATION-BRIEF.md, ACCEPTANCE-MAP.md, LAUNCH-PROMPT.md, ALIGNMENT-REPORT.md)
- All SCOPE.md acceptance criteria addressed in the brief?
- ADR pattern IDs resolve in AgentDB?
- No stale file paths or deprecated pattern references in deliverables?
- Internal consistency (brief references files that exist, AC-IDs match between map and brief)
- Report: PASS/WARN/FAIL with specific gaps

**WS4-02: `/validate-impl` skill** (upgraded current `/validate` — runs after implementation)
- Tier 1: cargo build/test/clippy (existing, already trusted)
- Tier 2 — process adherence checks:
  - Banned dependency scan (grep Cargo.toml for DuckDB/Polars/jemalloc)
  - Anti-stub scan (grep non-test `.rs` files for `todo!()`/`unimplemented!()`)
  - File scope check (modified files appear in the brief)
  - Stale reference scan (deprecated pattern IDs, removed file paths)
  - Config schema validation (streams YAML parses without error)
  - All shell checks in the skill, not Rust tests
- Tier 3 — spec compliance:
  - AC coverage: cross-reference ACCEPTANCE-MAP.md against `cargo test --list`
  - Test count delta against pre-implementation baseline
  - New dependency check: new crate deps must appear in the brief
- Tier 4 — risk classification:
  - Scope (narrow < 5 files, moderate 5-15, broad > 15)
  - Depth (surface/logic/structural)
  - Domain (tooling/platform/core)
  - Composite → LOW/MEDIUM/HIGH
  - Anomaly flags (large diffs, test count decrease, new external deps)

**WS4-03: Glass box report output** (both skills produce structured reports)
- Per-check PASS/FAIL/NOT CHECKED with evidence
- "NOT CHECKED" section (with reasons)
- "RECOMMENDED HUMAN REVIEW" section
- Confidence score
- Written to `product/features/{id}/reports/`

### Workstream 5: Planning Swarm Deliverables

New artifacts the planning swarm must produce to enable spec compliance checking and better handoffs.

**WS5-01: ACCEPTANCE-MAP.md format and protocol** (ACT-042)
- Define format: AC-ID, description, proposed test function name, status
- Update planning protocol: specification agent produces ACCEPTANCE-MAP.md
- Machine-parseable (markdown table)

**WS5-02: Implementation launch prompt** (planning swarm deliverable)
- Planning swarm produces `LAUNCH-PROMPT.md` alongside the brief
- Contains: proposed implementation kickoff prompt, reminders, constraints, gotchas discovered during planning, references to key deliverables
- User reviews/edits before launching — confidence checkpoint
- Lives in `product/features/{id}/LAUNCH-PROMPT.md`

**WS5-03: Per-wave acceptance check** (ACT-012)
- Add step to implementation protocol: scrum-master maps completed tasks to ACs after each wave
- Flags uncovered ACs before proceeding to next wave

### Workstream 6: Trust Infrastructure (AgentDB-based)

The trust tracking that enables eventual autonomy progression.

**WS6-01: `/trust-dashboard` skill**
- Queries AgentDB reflexion entries with `trust:validation:*` prefix
- Computes `Beta(correct+1, incorrect+1)` per check
- Renders human-readable summary: per-check scores, composite score, trend
- Shows last N features with pass/fail per check

**WS6-02: Trust score recording in `/validate`**
- After validation runs, store results as reflexion entries
- Task format: `trust:validation:{tier}:{check_name}`
- Reward: 1.0 (correct), 0.0 (missed/false negative)
- Initially: all entries are self-reported (no human comparison yet)

**WS6-03: `/shadow-judge` skill**
- After human reviews code, record judgment: approve/reject + notes
- Compare against automated validation report
- Store comparison result as trust reflexion entry (agreed/disagreed)
- `/shadow-judge approve` or `/shadow-judge reject "missed null check in foo.rs"`

**WS6-04: Trust snapshot at release**
- Export current trust scores to `.deploy/trust/vX.Y.Z.json`
- Git-tracked for audit trail
- Human can diff between releases to see trust progression

## NOT in Scope

- **New Rust code** — ops-006 is tooling/process changes, not product code
- **External tools** (`just`, `cargo-deny`, `cargo-nextest`) — all checks run through Claude Code hooks and skills
- **Tier 5 (LLM judge)** — deferred to Phase C (risk-gated), requires shadow mode evidence first
- **Auto-approve / risk-gated mode** — this is Phase C; ops-006 is observation only
- **Concurrent swarm support** — separate effort (ACT-035, ACT-045, ACT-090)
- **Agent evolution / /learner updates** — Phase D
- **GitHub Actions CI** (ACT-039) — all validation is local through Claude Code
- **New agent definitions** (ACT-031, ACT-050, ACT-062, ACT-063) — we tighten existing agents, not create new ones
- **Branch-per-feature workflow** (ACT-017) — convention change, separate effort

## Acceptance Criteria

- [ ] **AC-01**: Adherence audit completed — prioritized list of gaps with before/after fixes
- [ ] **AC-02**: Agent definitions updated — no "should" where "must" is intended, no stale references
- [ ] **AC-03**: `/validate-plan` checks planning artifacts exist, are internally consistent, and reference valid patterns
- [ ] **AC-04**: `/validate-impl` produces a structured glass box report covering Tiers 1-4
- [ ] **AC-05**: Both validation reports include "NOT CHECKED" and "RECOMMENDED HUMAN REVIEW" sections
- [ ] **AC-06**: Risk classification assigns LOW/MEDIUM/HIGH based on diff characteristics
- [ ] **AC-06a**: Process adherence checks (banned deps, stub scan, file scope, stale refs) run in `/validate-impl`
- [ ] **AC-07**: Pre-commit hook blocks commits with format violations or stubs
- [ ] **AC-08**: Post-task hook verifies expected artifacts exist for task type
- [ ] **AC-09**: Pre-spawn hook prevents implementation swarm on broken workspace
- [ ] **AC-10**: ACCEPTANCE-MAP.md format defined and documented in planning protocol
- [ ] **AC-11**: Planning swarm produces LAUNCH-PROMPT.md with proposed implementation kickoff prompt
- [ ] **AC-12**: `/trust-dashboard` renders per-check Bayesian trust scores from AgentDB
- [ ] **AC-13**: `/validate` writes trust reflexion entries to AgentDB after each run
- [ ] **AC-14**: `/shadow-judge` skill records human judgment and stores comparison in AgentDB
- [ ] **AC-15**: Trust snapshot exports to `.deploy/trust/vX.Y.Z.json` at release time
- [ ] **AC-16**: Flaky tests are separated from real failures in validation output
- [ ] **AC-17**: Scope pre-check flags vision misalignment before planning starts
- [ ] **AC-18**: Test count baseline stored; `/validate` warns on regression

## Planning Guidance

**Research questions** the planning swarm should investigate:
1. **Current adherence gaps** — What do agents actually skip? Where do definitions contradict protocols? Which hooks fire but don't enforce?
2. **Enforcement patterns** — How do Claude Code hooks work as gates (block vs warn)? What can post-task hooks reliably check?
3. **Minimal effective changes** — What is the smallest set of definition/rule/hook edits that closes the biggest adherence gaps?
4. **Measurement** — How do we know adherence improved? What does the trust dashboard need to show?

**Wave sequencing** — workstreams have natural dependencies. Suggested order:
- **Wave 1**: WS1 (cleanup) + WS2 (audit + tighten definitions) — foundation, everything else depends on knowing the gaps
- **Wave 2**: WS3 (hook enforcement) + WS4 (validate skills) + WS5 (planning deliverables) — enforce and measure
- **Wave 3**: WS6 (trust infrastructure) — track improvement over time

## Dependencies

- **fe-003** (complete): AgentDB + reflexion infrastructure exists
- **ops-004** (complete): WAL-only Bronze, 908 tests passing baseline
- **AgentDB MCP**: reflexion_store / reflexion_retrieve must support prefix queries on `task` field

## Estimated Effort

- Implementation is primarily definition/rule/skill/hook file edits (~30 files)
- No new Rust code, no new external dependencies
- Phase A (WS1-WS5): targeted file edits
- Phase B (WS6): skill creation + 7-10 weeks shadow mode practice

## Success Metric

After ops-006:
1. The development process is tighter — agents follow protocols because definitions are unambiguous and hooks enforce critical steps
2. Every feature produces a glass box validation report showing process adherence + code quality
3. Trust scores accumulate in AgentDB, giving me evidence for when (and whether) to cede more control
4. I have a `/trust-dashboard` I can check anytime to see how the system is performing

The human still reviews everything. But now there's measurable evidence of whether the process works.
