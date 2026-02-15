# 04: Early Vision and Goal Validation

> Evaluator: research agent-4
> Date: 2026-02-15
> Scope: Shift-left validation, continuous alignment, goal-based assertions, SPARC phase gates
> Files reviewed: 22 protocol/agent/skill/feature/vision files

---

## Executive Summary

- **Validation currently happens too late.** Vision alignment occurs at Step 3d of planning (AFTER all specs are written) and Step 3d of implementation (AFTER all code is written). By the time drift is detected, the full cost of the drifted work has already been incurred. The fe-004 alignment report shows 7 PASS -- but this outcome is unknowable until the entire planning swarm completes.
- **No validation exists during the SPARC phases themselves.** Specification, pseudocode, and architecture agents produce artifacts in isolation. Each agent has the Level-1 summary (anti-drift), but no agent checks its own output against the 7 alignment principles before returning. Self-validation would catch most drift at zero additional agent cost.
- **Acceptance criteria are prose, not executable.** The SCOPE.md acceptance criteria (e.g., "Search latency <1ms p99", "Daemon runs 10+ minutes") are human-readable but never converted into assertions that run during implementation. They are checked manually during the final drift check.
- **Progressive confidence is implicit, not tracked.** The system has natural confidence checkpoints (spec complete, architecture decided, tests passing, integration validated) but never quantifies or reports confidence at each stage. The user sees nothing until the final alignment report or implementation summary.
- **The `/align` skill exists as an on-demand tool but is never called mid-phase.** It is designed for post-planning use. A lightweight variant could run during individual SPARC phase completion at minimal cost.

---

## 1. Current Validation Timeline

### When Validation Happens Today

```
PLANNING SWARM
==============

Phase 1: Preparation
  [get-pattern]                    No validation
  [read SCOPE.md]                  No validation

Phase 2: Delegation
  [spawn ndp-scrum-master]         No validation

Phase 3a: Swarm Init               No validation
Phase 3b: Task Definition          No validation
Phase 3c: Agent Spawning
  specification agent ---------> produces SPECIFICATION.md    No validation
  pseudocode agent ------------> produces PSEUDOCODE.md       No validation
  ndp-architect agent ----------> produces ARCHITECTURE.md    No validation

Phase 3d: Vision Alignment         FIRST VALIDATION POINT <--- here, after all planning
  ndp-vision-guardian ----------> produces ALIGNMENT-REPORT.md

Phase 3e: ADR Storage              No validation
Phase 3f: Brief Generation         No validation (brief quality gate missing, per 01-report)
Phase 3g: GH Issue Creation        No validation

Phase 4: Completion
  [present variances to user]      SECOND VALIDATION POINT <--- user approval


IMPLEMENTATION SWARM
====================

Phase 1: Preparation
  [get-pattern]                    No validation
  [read brief]                     No validation

Phase 2: Delegation                No validation

Phase 3a-3c: Init + Spawn
  ndp-rust-dev agents ----------> write code                  No validation
  ndp-tester agents ------------> write tests                 No validation

Phase 3d: Drift Check              THIRD VALIDATION POINT <--- after all code written
  - Files outside scope?
  - Stubs left?
  - Acceptance criteria missed?
  - Test count decreased?

Phase 3e: Validation               FOURTH VALIDATION POINT <--- cargo build/test/clippy
  - Tier 1: build + test
  - Tier 2: clippy
  - Tier 3: integration (if qualifying)

Phase 3f: GH Issue Update          No validation

Phase 4: Completion
  [reflexion]                      No validation
```

### Validation Gap Analysis

| Phase | Time Elapsed (est.) | Cost of Drift if Detected Here | Current Validation | Gap |
|-------|---------------------|-------------------------------|-------------------|-----|
| SCOPE.md written | 0 | Near zero | None | Could validate scope against vision principles before planning begins |
| Specification complete | 2-4 hours | Low (text only) | None | Could check spec against 7 principles immediately |
| Architecture complete | 4-8 hours | Medium (ADRs may conflict with constraints) | None | Could check ADRs against technical constraints |
| Pseudocode complete | 6-10 hours | Medium (design committed) | None | Could check pseudocode against integration-first mandate |
| Planning alignment (current) | 8-12 hours | High (all planning done) | Vision guardian | Too late for cheap correction |
| Wave 1 code complete | +3-4 days | Very high (code written) | None | Could validate per-wave against acceptance criteria |
| Wave 2 code complete | +6-8 days | Very high | None | Same gap |
| Implementation drift check | +12-17 days | Maximum (all code done) | Drift check | Far too late for structural drift |
| Final validation | +12-17 days | Maximum | cargo build/test | Catches compilation errors, not goal drift |

### Key Observation

The first validation point (Step 3d of planning) occurs after 100% of planning work is done. The first implementation validation point (Step 3d of implementation) occurs after 100% of coding work is done. This means drift is maximally expensive to fix when detected.

---

## 2. Gap Analysis: What Falls Through

### Gap 1: No Scope-to-Vision Pre-Check

SCOPE.md is written by the user and assumed to be aligned with the vision. But it could contain requests that violate alignment principles. For example:
- Requesting a cloud-dependent feature (violates Edge-Only)
- Hardcoding thresholds (violates Config-Driven)
- Requesting DuckDB usage (violates Technical Constraints)

Currently, these would not be detected until Step 3d of planning -- after 3 agents have already produced specifications based on the misaligned scope.

### Gap 2: Per-Agent Output Validation

Each planning agent (specification, pseudocode, ndp-architect) produces artifacts independently. None of them validates their own output against alignment criteria before returning. The anti-drift mechanism (Level-1 summary in prompt) tells agents WHAT to avoid but never verifies they actually avoided it.

### Gap 3: Acceptance Criteria Are Never Executable

The SCOPE.md for fe-004 lists 12 acceptance criteria (AC-01 through AC-12). These are copied into the specification and implementation brief as prose tables. During implementation:
- Agents write tests that may or may not correspond to these criteria
- The drift check at Step 3d mentions "Acceptance criteria missed" but performs a text-level grep, not a structural mapping
- No system maps acceptance criteria IDs to test function names
- No system reports: "AC-03 (HNSW search <1ms p99) is NOT covered by any test"

### Gap 4: No Inter-Phase Validation Gates

SPARC phases (S, P, A, R, C) are designed as sequential refinements. Each phase should validate that the previous phase's output is correct before proceeding. Currently:
- S (Specification) starts from SCOPE.md with no validation
- P (Pseudocode) starts from Specification with no validation that the spec is complete
- A (Architecture) starts from both with no validation of internal consistency
- R (Refinement = implementation) starts from the brief with no validation that the brief covers all acceptance criteria
- C (Completion) is the deployment/release phase with no acceptance criteria gate

### Gap 5: No Continuous Alignment During Implementation

Implementation agents execute for 12-17 days (per fe-004 estimate) with zero alignment checks between waves. Each wave could introduce drift that compounds in subsequent waves. The drift check only runs after ALL waves are complete. If Wave 1 drifts, Waves 2-5 build on the drifted foundation.

### Gap 6: No Progressive Confidence Reporting

The user sees:
1. "Planning swarm started" (0% visibility)
2. "Alignment report: 7 PASS" (100% visibility after planning)
3. "Implementation swarm started" (0% visibility)
4. "Tests pass, validation PASS" (100% visibility after implementation)

There is no intermediate signal like: "Specification complete, 6/7 alignment principles verified, Config-Driven pending architecture review."

---

## 3. Proposed Validation Checkpoints (Per SPARC Phase)

### Phase S (Specification): Scope Pre-Check + Spec Self-Validation

**When**: Immediately after SCOPE.md is read (before planning agents spawn) AND after the specification agent returns.

**Pre-check (runs in the primary agent, no spawn needed)**:

```
Read SCOPE.md
Read ALIGNMENT-CRITERIA.md (7 principles + technical constraints)

For each principle:
  Check SCOPE.md text for obvious violations:
    - Edge-Only: search for "cloud", "API endpoint", "external service"
    - Config-Driven: search for specific numeric values without "configurable"
    - Domain-Portable: search for hardcoded domain names without "generic"
    - Resource-Constrained: search for banned deps (DuckDB, Polars, jemalloc)
    - Integration-First: search for "new module", "separate system"
    - Privacy: search for "telemetry", "analytics", "external"
    - Self-Learning: check if feature type warrants learning (skip for ops)

  Classify: LIKELY_PASS | REVIEW_NEEDED | LIKELY_FAIL

Report to user: "Scope pre-check: 6 LIKELY_PASS, 1 REVIEW_NEEDED (Config-Driven:
  SCOPE mentions '168 hours' as a fixed value -- should this be configurable?)"
```

**Effort**: Zero additional agents. 30 seconds of primary agent time. Catches obvious misalignment before any planning work begins.

**Post-spec validation (added to specification agent prompt)**:

Add to the specification agent's prompt:
```
BEFORE returning your result, self-check against ALIGNMENT-CRITERIA.md:
1. Does the specification introduce any cloud dependencies? (Edge-Only)
2. Are all numeric values in the spec configurable? (Config-Driven)
3. Does the spec use existing traits/interfaces or create new ones? (Integration-First)
4. Are there any banned dependencies mentioned? (Technical Constraints)

If any check fails, note it in your return summary as a "spec-level variance".
```

**Effort**: Adds ~200 tokens to the specification agent's prompt. No additional agent spawn. Catches spec-level drift before the architecture and pseudocode agents start building on it.

### Phase P (Pseudocode): Behavioral Pre-Validation

**When**: After pseudocode agent returns.

**Self-check (added to pseudocode agent prompt)**:

```
BEFORE returning, verify:
1. Every acceptance criterion from SCOPE.md has a corresponding pseudocode section
2. No pseudocode section implements behavior NOT in SCOPE.md
3. Error handling follows existing patterns (exponential backoff, graceful degradation)
4. Data flow matches Bronze->Silver->Gold architecture

Return a coverage matrix:
| Acceptance Criterion | Pseudocode Section | Status |
|---------------------|-------------------|--------|
| AC-01 | Section 3.1 | COVERED |
| AC-02 | Section 3.4 | COVERED |
| AC-03 | NOT FOUND | GAP |
```

**Effort**: Adds ~150 tokens to pseudocode agent prompt. Produces a machine-readable coverage matrix that the vision guardian can later verify.

### Phase A (Architecture): Constraint Validation

**When**: After architecture agent returns.

**Self-check (added to ndp-architect agent prompt)**:

```
BEFORE returning, verify each ADR against technical constraints:
- ARM64 compatible? (check every dependency mentioned)
- Within resource budget? (256 MB container, 5.5 GB total)
- No banned deps? (DuckDB, Polars, jemalloc)
- Config-driven? (no hardcoded values in ADR decisions)
- Uses existing interfaces? (extend traits, don't create parallel)

For each ADR, add a "## Constraint Compliance" section:
  ARM64: PASS/FAIL (evidence)
  Resource: PASS/FAIL (estimate)
  Banned deps: PASS/FAIL
  Config: PASS/FAIL
  Integration: PASS/FAIL
```

**Effort**: Adds ~200 tokens to ndp-architect prompt. Each ADR gets a built-in compliance check. The vision guardian's later review is simplified -- it only needs to verify the self-assessment, not discover violations from scratch.

### Phase R (Refinement/Implementation): Per-Wave Validation

**When**: After each implementation wave completes (not just after all waves).

**Per-wave alignment check (added to implementation-protocol.md)**:

Between Step 3c (agent spawning) and Step 3d (drift check), add a new sub-step:

```
Step 3c.5: Per-Wave Acceptance Check

After wave agents return, before drift check:
1. Map completed tasks to acceptance criteria:
   | Task | Acceptance Criterion | Test Exists? | Test Passes? |
   | P2-02 | AC-03 (HNSW <1ms) | Yes: test_hnsw_search_latency | PASS |
   | P2-03 | AC-04 (pgvector <10ms) | No | N/A |

2. If a task maps to an AC and no test exists: flag as WARN
3. If a task maps to an AC and the test fails: flag as FAIL
4. Report to coordinator before next wave spawn

This prevents Wave 2 from building on Wave 1 if Wave 1 has unverified ACs.
```

**Effort**: Adds 1-2 minutes per wave. Catches drift between waves instead of after all waves.

### Phase C (Completion): Acceptance Criteria Gate

**When**: After all implementation waves, before final GH Issue update.

**Gate (added to implementation-protocol.md Step 3e)**:

```
Step 3e.5: Acceptance Criteria Gate

After validation passes, verify ALL acceptance criteria from SCOPE.md:
1. Parse SCOPE.md ## Acceptance Criteria table
2. For each criterion:
   a. Is there a test that explicitly validates it?
   b. Does the test pass?
   c. Is the criterion measurable by the test? (e.g., "10+ minutes" needs a soak test, not a unit test)
3. Report:
   | Criterion | Test | Status | Measurable? |
   | AC-01 | test_embeddings_stored | PASS | Yes |
   | AC-08 | test_daemon_runs_60s | PASS | Partial (60s not 10min) |
   | AC-11 | NONE | MISSING | N/A |

4. If any AC is MISSING or FAIL, flag before closing the issue
```

**Effort**: Adds 1-2 minutes. Prevents features from being marked "complete" when acceptance criteria are unverified.

---

## 4. Agent Self-Validation Protocol

### The Problem

Currently, agents rely entirely on the coordinator (ndp-scrum-master) for quality control. The coordinator checks AFTER all agents return. This is like code review after the entire PR is written -- effective but late.

### Proposed: Agent Self-Check Pattern

Add a standardized self-check block to every agent prompt. The block varies by agent type but follows a common structure:

```
SELF-CHECK (run before returning):

1. SCOPE ALIGNMENT:
   - Read the acceptance criteria from SCOPE.md (or Level-1 summary)
   - Verify your output addresses the criteria assigned to your task
   - Flag any criteria you cannot fully address

2. CONSTRAINT COMPLIANCE:
   - ARM64 compatible: [yes/no/unknown]
   - No banned deps: [yes/no, list any new deps]
   - Config-driven: [yes/no, list any hardcoded values]
   - Integration-first: [yes/no, list new abstractions if any]

3. ANTI-STUB:
   - grep your output files for todo!(), unimplemented!(), TODO, FIXME
   - If found, resolve them or explain why they are needed

4. OUTPUT SUMMARY (structured):
   Files: [list]
   Tests added: [count]
   Acceptance criteria covered: [list AC-IDs]
   Self-check result: PASS / WARN(reason) / FAIL(reason)
```

### Implementation: Per-Agent-Type Variants

**Planning agents (specification, pseudocode, ndp-architect)**:
```
SELF-CHECK additions:
- Does your artifact cover all SCOPE.md deliverables?
- Does your artifact introduce anything not in SCOPE.md?
- Are all dependencies ARM64-safe?
```

**Implementation agents (ndp-rust-dev)**:
```
SELF-CHECK additions:
- Does the code compile? (cargo build)
- Do the tests pass? (cargo test for modified crate)
- Any clippy warnings?
- Any hardcoded values that should be configurable?
```

**Test agents (ndp-tester)**:
```
SELF-CHECK additions:
- Do tests cover the acceptance criteria assigned to you?
- Are integration tests properly marked #[ignore]?
- Do unit tests use mockall (London TDD style)?
```

### Cost-Benefit

| Metric | Without Self-Check | With Self-Check |
|--------|-------------------|-----------------|
| Prompt size increase | 0 | +200-300 tokens per agent |
| Drift detection time | After all agents complete | At individual agent completion |
| Corrective iterations | 0-2 per wave (expensive, full re-spawn) | 0-1 per agent (cheap, same context) |
| Coordinator load | High (must analyze all outputs for drift) | Reduced (agents pre-flag issues) |
| False positive risk | N/A | Low (checks are objective, not subjective) |

The key insight: an agent self-correcting within its own context window is dramatically cheaper than a coordinator spawning a new agent to fix drift.

---

## 5. Executable Acceptance Criteria Design

### The Problem

Acceptance criteria today are prose in SCOPE.md:

```markdown
| Criterion | Target |
|-----------|--------|
| Search latency (HNSW) | <1ms p99 |
| Daemon runs without crash | 10+ minutes continuous operation |
| Docker container builds | On both x86_64 and aarch64 |
```

These are human-readable but:
- Not linked to specific test functions
- Not automatically verified during implementation
- Not tracked for coverage

### Proposed: AC-to-Test Mapping File

Add a new file to each feature directory: `product/features/{feature-id}/ACCEPTANCE-MAP.md`

This file is generated during planning (by the specification agent) and updated during implementation (by the scrum-master):

```markdown
# fe-004 Acceptance Criteria Map

| AC-ID | Criterion | Target | Test Function | Test Type | Status |
|-------|-----------|--------|--------------|-----------|--------|
| AC-01 | Embeddings generated | All hours embedded | `test_embeddings_stored_for_all_hours` | integration | PENDING |
| AC-02 | Predictions after warmup | >= 1/hour after 168h | `test_predictions_after_warmup` | integration | PENDING |
| AC-03 | HNSW search latency | <1ms p99 | `test_hnsw_search_latency_p99` | bench | PENDING |
| AC-04 | pgvector search latency | <10ms p99 | `test_pgvector_search_latency_p99` | bench | PENDING |
| AC-05 | Full cycle latency | <500ms | `test_full_cycle_latency` | integration | PENDING |
| AC-06 | Daemon memory | <100 MB actual | `test_daemon_memory_rss` | integration | PENDING |
| AC-07 | Prediction accuracy | Logged | `test_prediction_accuracy_logged` | integration | PENDING |
| AC-08 | Daemon stability | 10+ minutes | `test_daemon_soak_10min` | soak | PENDING |
| AC-09 | One-shot mode | Exit 0 | `test_one_shot_mode_exit_0` | integration | PENDING |
| AC-10 | Backfill mode | Process N hours | `test_backfill_n_hours` | integration | PENDING |
| AC-11 | Docker builds | x86_64 + aarch64 | `test_docker_build_multi_arch` | ci | PENDING |
| AC-12 | deploy.sh works | Intelligence starts | `test_deploy_sh_intelligence` | manual | PENDING |
```

### Lifecycle

1. **Planning (specification agent)**: Creates the ACCEPTANCE-MAP.md with AC-IDs, criteria, and target values. Test function names are proposed (may be refined during implementation).
2. **Implementation (ndp-rust-dev)**: When writing a test for an AC, names the test function to match the map. Updates the "Status" column to IN_PROGRESS.
3. **Per-wave check (scrum-master)**: After each wave, scans the map for PENDING entries that should be covered by the completed wave. Flags gaps.
4. **Completion gate (scrum-master)**: Before marking the feature complete, verifies all entries are PASS, MANUAL_VERIFIED, or DEFERRED(with reason).

### Verification Script

A lightweight script (or `just` recipe) that verifies the map against actual test output:

```bash
# Parse ACCEPTANCE-MAP.md, extract test function names
# Run cargo test --list to get all test names
# Report: which AC test functions exist? which are missing?

just ac-check fe-004
# Output:
# AC-01: test_embeddings_stored_for_all_hours - FOUND (unit test)
# AC-02: test_predictions_after_warmup - FOUND (ignored, needs integration env)
# AC-03: test_hnsw_search_latency_p99 - NOT FOUND
# AC-04: test_pgvector_search_latency_p99 - NOT FOUND
# ...
# Coverage: 8/12 (67%)
# Missing: AC-03, AC-04, AC-11, AC-12
```

### Cost

- Planning overhead: ~10 minutes for specification agent to produce the map (trivial, since the ACs already exist in SCOPE.md)
- Implementation overhead: test naming convention (zero runtime cost)
- Verification: automated script, 5 seconds per run
- Value: eliminates the "did we test everything the user asked for?" uncertainty

---

## 6. Progressive Confidence Model

### Concept

Instead of binary "aligned/not aligned" at the end, track confidence progressively:

```
                                     Confidence
Scope written            ............|... 30% (vision pre-check passed)
Spec complete            ............|.......... 50% (spec self-check passed)
Architecture complete    ............|............... 65% (ADR constraint checks passed)
Pseudocode complete      ............|.................. 70% (AC coverage matrix complete)
Alignment report         ............|...................... 80% (7/7 PASS)
Brief generated          ............|........................ 85% (brief quality gate passed)
Wave 1 code + tests      ............|.......................... 88% (per-wave AC check)
Wave 2 code + tests      ............|............................ 90%
Wave 3 code + tests      ............|.............................. 92%
Wave 4 code + tests      ............|................................ 94%
Cargo build+test+clippy  ............|.................................. 96%
Integration tests pass   ............|.................................... 98%
AC gate all PASS         ............|...................................... 100%
```

### Implementation

The scrum-master reports confidence to the user (and stores in memory) at each checkpoint:

```
Memory: confidence/{feature-id}
Value: {
  "phase": "architecture_complete",
  "confidence": 0.65,
  "checks_passed": ["scope_precheck", "spec_selfcheck", "adr_constraints"],
  "checks_pending": ["alignment_report", "brief_quality", "wave_1", ...],
  "variances": [],
  "timestamp": "2026-02-14T12:00:00Z"
}
```

The user sees in the GH Issue:

```
## Planning Progress

| Checkpoint | Confidence | Status | Notes |
|-----------|------------|--------|-------|
| Scope pre-check | 30% | PASS | 7/7 principles, no red flags |
| Specification | 50% | PASS | All 17 deliverables covered, 12 ACs mapped |
| Architecture | 65% | PASS | 8 ADRs, all constraint-compliant |
| Pseudocode | 70% | PASS | AC coverage: 12/12 (100%) |
| Alignment | 80% | PASS | 7 PASS, 0 WARN |
| Brief quality | 85% | PASS | All ADR pattern IDs resolve, all file paths valid |
```

### Value

- The user gets visibility into planning/implementation progress without waiting for the final report.
- Variances surface early: if spec self-check produces a WARN at 50%, the user can redirect before architecture work begins.
- Confidence regression is a signal: if confidence drops from 65% to 60% after pseudocode (e.g., pseudocode introduces an out-of-scope dependency), the coordinator can pause before implementation.

---

## 7. Drift Detection Signals

### Measurable Signals During Implementation

| Signal | How to Detect | Severity | When to Check |
|--------|--------------|----------|---------------|
| File scope violation | Agent modifies a file not in the brief's "Files to Create/Modify" list | High | After each agent returns |
| New dependency addition | Agent adds a dep to Cargo.toml not in the brief's "Dependencies" section | Medium | After each agent returns |
| Test count decrease | `cargo test --workspace` produces fewer tests than baseline | High | After each wave |
| Acceptance criteria gap | AC-ID has no corresponding test function in the codebase | Medium | After each wave |
| Stub left in code | `grep -rn 'todo!\|unimplemented!' --include='*.rs'` in non-test files | High | After each agent returns |
| Hardcoded value | New numeric literal in non-test `.rs` file that should be configurable | Medium | After each wave |
| Out-of-scope feature | Agent implements something not in the deliverables table | Medium | After each agent returns |
| New abstraction | New trait or struct not in the brief's "Data Structures" section | Low-Medium | After each wave |
| Banned dependency | DuckDB, Polars, jemalloc, ndarray (for fe-004) added to any Cargo.toml | Critical | After each agent returns |
| Resource budget violation | New container with no mem_limit, or mem_limit exceeding architecture decision | High | After deployment-related agents return |

### Automated Detection Script

These signals can be checked mechanically after each agent or wave:

```bash
# 1. File scope: compare modified files against brief's file list
git diff --name-only HEAD~1 | while read f; do
  grep -q "$f" product/features/$FEATURE/IMPLEMENTATION-BRIEF.md || \
    echo "WARN: $f modified but not in brief"
done

# 2. New deps: check Cargo.toml diff for added dependencies
git diff HEAD~1 -- '**/Cargo.toml' | grep '^+' | grep -v '^+++' | \
  grep -E '^\+[a-z]' | while read dep; do
    grep -q "$(echo $dep | cut -d= -f1 | tr -d '+ ')" \
      product/features/$FEATURE/IMPLEMENTATION-BRIEF.md || \
      echo "WARN: New dependency not in brief: $dep"
  done

# 3. Banned deps
git diff HEAD~1 -- '**/Cargo.toml' | grep '^+' | \
  grep -iE 'duckdb|polars|jemalloc' && echo "FAIL: Banned dependency added"

# 4. Stubs
grep -rn 'todo!()\|unimplemented!()' --include='*.rs' core/ apps/ crates/ tools/ | \
  grep -v '#\[test\]\|_test\|test_' | head -10
```

These checks take <5 seconds and can run after every agent completes, not just after all waves.

---

## 8. Implementation Priority

### P0: Immediate (Before Next Feature)

| # | Change | Effort | Impact | Files to Modify |
|---|--------|--------|--------|----------------|
| P0-1 | **Scope pre-check**: Add a 7-principle scan of SCOPE.md to the primary agent's Phase 1 (before spawning the planning swarm). No new agents needed -- it is a prompt addition to the primary agent's preparation step. | Small (30 min) | High -- catches misaligned scope before ANY planning work | `.claude/rules/planning-protocol.md` |
| P0-2 | **Agent self-check block**: Add the self-check protocol to planning agent prompts (specification, pseudocode, ndp-architect). Each agent validates its own output against alignment criteria before returning. | Small (1 hour) | High -- catches per-agent drift within the agent's context (cheapest possible correction) | `.claude/agents/ndp/ndp-architect.md`, `.claude/rules/planning-protocol.md` (prompt templates) |
| P0-3 | **Per-wave acceptance check**: Add Step 3c.5 to implementation-protocol.md. After each wave, the scrum-master maps completed tasks to acceptance criteria and flags gaps before spawning the next wave. | Small (30 min) | High -- prevents compound drift across waves | `.claude/rules/implementation-protocol.md` |

### P1: Within Next 2 Features

| # | Change | Effort | Impact | Files to Modify |
|---|--------|--------|--------|----------------|
| P1-1 | **ACCEPTANCE-MAP.md generation**: Add to specification agent output. Machine-readable mapping of AC-IDs to proposed test function names. | Medium (2 hours) | High -- makes acceptance criteria tractable and trackable | `.claude/rules/planning-protocol.md`, specification agent prompt |
| P1-2 | **Acceptance criteria gate at completion**: Add Step 3e.5 to implementation-protocol.md. Before the feature is marked complete, verify all ACs are covered by tests. | Small (30 min) | High -- prevents "feature complete but AC-03 never tested" | `.claude/rules/implementation-protocol.md` |
| P1-3 | **Progressive confidence reporting**: Scrum-master reports confidence percentage and checkpoint status to GH Issue after each phase transition. | Medium (1 hour) | Medium -- user visibility, no functional change | `.claude/rules/planning-protocol.md`, `.claude/rules/implementation-protocol.md` |
| P1-4 | **Brief quality gate**: After brief generation (Step 3f), verify: all ADR pattern IDs resolve in AgentDB, all file paths in "Files to Create/Modify" are valid workspace paths, all acceptance criteria from SCOPE.md appear in the brief. (Already recommended in 01-report as P1-3.) | Medium (1 hour) | High -- prevents implementation swarm from building on an incomplete brief | `.claude/rules/planning-protocol.md` |

### P2: Strategic

| # | Change | Effort | Impact | Files to Modify |
|---|--------|--------|--------|----------------|
| P2-1 | **Lightweight mid-phase /align**: Create a `/align-lite` skill that runs a subset of the vision guardian checks (technical constraints + scope alignment only, skip full principle analysis). Can be invoked after each SPARC phase at ~30 seconds per check. | Medium (2 hours) | Medium -- catches constraint violations between phases | New skill in `.claude/skills/align-lite/` |
| P2-2 | **Implementation agent self-check**: Extend the self-check protocol to ndp-rust-dev and ndp-tester agents. Each implementation agent runs `cargo build` and `cargo test` on its own modified crate before returning, plus checks for stubs and hardcoded values. | Medium (1 hour per agent) | High -- catches compilation errors within the agent's context instead of waiting for the coordinator's Tier 1 validation | Agent definitions in `.claude/agents/ndp/` |
| P2-3 | **AC verification script**: Implement `just ac-check {feature-id}` that parses ACCEPTANCE-MAP.md, cross-references with `cargo test --list`, and reports coverage. | Medium (2 hours) | Medium -- automated acceptance criteria tracking | New `Justfile` recipe + script |
| P2-4 | **Drift detection hook**: Post-edit hook that checks every file modification against the implementation brief's file list. If a file is modified that is not in the brief, emit a warning in the agent's output. | Medium (2 hours) | Medium -- real-time drift signal | `.claude/hooks/` |
| P2-5 | **Confidence regression alert**: If progressive confidence decreases between phases (e.g., architecture phase finds a constraint violation that spec phase missed), the scrum-master pauses and reports to the user before proceeding. | Small (30 min) | Medium -- prevents blind forward progress when problems are detected | `.claude/rules/planning-protocol.md` |

---

## 9. Relationship to Other Research Reports

| Report | Intersection with Early Validation |
|--------|------------------------------------|
| 01: Protocol and Agent Evaluation | P1-3 (brief quality gate) is the same recommendation. Agent self-check addresses the "per-agent reflexion" gap (P2-5 in 01-report). Partial failure recovery (P1-1 in 01-report) is complementary -- early validation reduces the frequency of failures that need recovery. |
| 02: Concurrent Multi-Swarm | Early validation becomes even more critical with concurrent swarms. If two swarms drift simultaneously, the cost compounds. Per-wave acceptance checks ensure each swarm stays aligned independently. |
| 03: Local CI/CD and E2E | The per-wave acceptance check (P0-3) and AC verification script (P2-3) directly integrate with the testing pyramid from 03-report. The pre-commit hook (QW-2 in 03-report) catches stubs, while the agent self-check catches them earlier (within the agent's context). |

---

## Files Referenced in This Research

| File | Path | Purpose |
|------|------|---------|
| Alignment Criteria | `/workspaces/neural-data-platform/product/vision/ALIGNMENT-CRITERIA.md` | 7 principles + technical constraints |
| Vision Guardian Agent | `/workspaces/neural-data-platform/.claude/agents/ndp/ndp-vision-guardian.md` | Agent that produces alignment reports |
| Planning Protocol | `/workspaces/neural-data-platform/.claude/rules/planning-protocol.md` | Vision check at Step 3d (AFTER planning) |
| Implementation Protocol | `/workspaces/neural-data-platform/.claude/rules/implementation-protocol.md` | Drift check at Step 3d (AFTER implementation) |
| Swarm Protocol | `/workspaces/neural-data-platform/.claude/rules/swarm-protocol.md` | Anti-drift config (Level-1 summary) |
| Spec-Compile Skill | `/workspaces/neural-data-platform/.claude/skills/spec-compile/SKILL.md` | Level-1 summary as anti-drift mechanism |
| Align Skill | `/workspaces/neural-data-platform/.claude/skills/align/SKILL.md` | On-demand alignment check |
| fe-004 SCOPE | `/workspaces/neural-data-platform/product/features/fe-004/SCOPE.md` | Example scope with 12 acceptance criteria |
| fe-004 Alignment Report | `/workspaces/neural-data-platform/product/features/fe-004/ALIGNMENT-REPORT.md` | Example alignment report (7 PASS) |
| fe-004 Specification | `/workspaces/neural-data-platform/product/features/fe-004/specification/SPECIFICATION.md` | Example spec with detailed ACs |
| fe-004 Architecture | `/workspaces/neural-data-platform/product/features/fe-004/architecture/ARCHITECTURE.md` | Example ADRs (8 decisions) |
| fe-004 Implementation Brief | `/workspaces/neural-data-platform/product/features/fe-004/IMPLEMENTATION-BRIEF.md` | Example brief (345 lines) |
| fe-004 Task Decomposition | `/workspaces/neural-data-platform/product/features/fe-004/specification/TASK-DECOMPOSITION.md` | Example wave structure |
| Integration-First Mandate | `/workspaces/neural-data-platform/product/INTEGRATION_FIRST_MANDATE.md` | Extend-don't-replace rules |
| 01 Research Report | `/workspaces/neural-data-platform/product/ndp-dev-auto/01-protocol-agent-evaluation.md` | Protocol evaluation findings |
| 02 Research Report | `/workspaces/neural-data-platform/product/ndp-dev-auto/02-concurrent-swarms.md` | Concurrent swarm architecture |
| 03 Research Report | `/workspaces/neural-data-platform/product/ndp-dev-auto/03-local-cicd-e2e.md` | CI/CD and testing workflows |
