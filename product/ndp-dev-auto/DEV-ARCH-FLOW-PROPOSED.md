# NDP Development Flow: Proposed Architecture

**Compare with**: `ARCHITECTURE-FLOW.md` (current state)
**Informed by**: `research-progressive-autonomy.md`, `research-validation-confidence.md`, reports 01-06

---

## Design Principles

1. **Paired architecture, ceded execution**: Human stays deeply involved in scope and architecture (L1). Everything downstream earns autonomy through demonstrated reliability.
2. **Trust is earned, not assumed**: Each phase starts human-supervised. Shadow mode → risk-gated → auto-approved. Regression drops you back.
3. **Glass box, not black box**: Every automated decision shows its reasoning. The human reviews reports, not code.
4. **Asymmetric autonomy**: Different phases operate at different trust levels. Architecture stays L1 forever. Validation climbs from L0 to L3.
5. **Fitness functions are the foundation**: Architectural intent encoded as executable tests. This is what makes ceding validation possible.
6. **Single knowledge store**: AgentDB holds ALL permanent knowledge — patterns, ADRs, reflexions, AND trust scores. No separate trust database.

---

## 1. One-Page Overview

```
USER                  AGENTS                           SYSTEMS              TRUST
----                  ------                           -------              LEVEL
                                                                           -----
 Idea
  |
  v
 Dialog <===========> Primary Agent                                         L1
  |                     |  architecture discussion                       (paired)
  |                     |  pros/cons, tradeoffs
  |                     |  /get-pattern (existing ADRs)
  v                     v
 SCOPE.md            Scope pre-check ---------> ALIGNMENT-CRITERIA.md     GATE 1
  |                     |  "Does this scope                              (30%
  |                     |   align with vision?"                        confidence)
  v                     v
 "Plan {id}" -------> Primary Agent
                        |
                        | Task(ndp-scrum-master)
                        v
                      PLANNING SWARM
                        |  ndp-architect -------> ADRs + ARCHITECTURE.md
                        |  spec + pseudocode ---> SPECIFICATION.md, etc.
                        |  /save-pattern -------> AgentDB (permanent)
                        |  vision-guardian ------> ALIGNMENT-REPORT.md     GATE 2
                        |  acceptance map -------> ACCEPTANCE-MAP.md       (60%
                        |  brief ----------------> IMPLEMENTATION-BRIEF.md  confidence)
                        |  gh issue create ------> GitHub Issue #N
                        v
                      Primary Agent
                        |  present variances
                        v
 Review plan --------> USER                                                 L1
  |                                                                       (paired)
  v
 "Implement {id}" --> Primary Agent
                        |
                        | /spec-compile (REQUIRED, not optional)
                        | Task(ndp-scrum-master)
                        v
                      IMPLEMENTATION SWARM
                        |  Level-1 summary in every agent prompt
                        |  Agent self-checks (scope + ADR conformance)
                        |  ndp-rust-dev --------> Rust code
                        |  ndp-tester ----------> Tests + AC coverage
                        |  Drift check
                        v
                      VALIDATION PIPELINE (5 tiers)                       GATE 3
                        |  T1: cargo build/test/clippy (deterministic)
                        |  T2: fitness functions (architectural intent)
                        |  T3: AC coverage mapping (spec compliance)
                        |  T4: risk classification + anomaly detection
                        |  T5: LLM judge (intent alignment, optional)
                        |
                        |  --> VALIDATION REPORT (glass box)
                        |  --> gh issue comment
                        v
 Review report ------> USER  (or auto-approve if trust earned)       L0->L1->L2->L3
  |
  v
 "Release" ----------> Release workflow
                        |  manifest + tag + changelog
                        |  deploy.sh build/deploy/status
                        v
                      POST-RELEASE
                        |  /reflexion (per pattern)
                        |  /learner (update agent definitions)
                        |  shadow-compare (if in shadow mode)
                        |  trust score update → AgentDB (reflexion_store)
                        v
                      KNOWLEDGE CAPTURED -------> AgentDB (permanent)
                                                  Agent definitions (evolved)
```

---

## 2. What Changed (Current → Proposed)

| Aspect | Current | Proposed | Why |
|--------|---------|----------|-----|
| Scope phase | Human writes SCOPE.md alone | **Dialog + scope pre-check** against vision | Catch misalignment at 30% confidence, not 60% |
| Architecture | Agent produces, human reviews | **Paired dialog** — human + agent co-design | Architecture is highest-leverage human contribution |
| spec-compile | Optional ("if run") | **Required** — no implementation without it | Agents without Level-1 summary drift |
| Acceptance criteria | Prose in brief | **ACCEPTANCE-MAP.md** with AC-to-test mapping | Makes spec compliance mechanically verifiable |
| Agent self-checks | None | **Self-validation block** in every agent prompt | Agents catch their own drift before returning |
| Validation | 3 tiers (build/lint/integration) | **5 tiers** with glass box report | Fitness functions + risk classification enable trust |
| Validation output | Pass/fail | **Structured report** showing what was checked, not checked, and why | Human reviews report, not code |
| Human review | Reviews code | **Reviews report** (spot-checks code on sampling basis) | 10x faster review cycle |
| Trust tracking | None | **Bayesian trust scores** in AgentDB (reflexion table) | Evidence-based autonomy progression, no new infrastructure |
| Shadow mode | None | **Automated + human review in parallel** with comparison | Calibrates the automation before trusting it |
| Learning | /reflexion + /save-pattern | **+ /learner updates agent definitions** | Agents get smarter between features |
| Rollback | None | **Git tag rollback** + documented procedure | Safety net for ceded validation |

---

## 3. Phase Breakdown (Proposed)

### Phase 0: Dialog (Human + Agent, L1 — Paired)

This is NEW. Before SCOPE.md exists, human and agent explore the problem space together.

```
Human: "I'm thinking about adding prediction capabilities..."
Agent: /get-pattern → retrieves relevant ADRs, prior features
Agent: "fe-003 built the embeddings foundation. Here are 3 approaches
        for prediction, with tradeoffs..."
Human: "I like approach B, but concerned about memory..."
Agent: "ADR-006 from fe-004 set a 256MB container limit. Approach B
        fits within that. Here's why..."
```

| Aspect | Detail |
|--------|--------|
| Who | Human + Primary Agent (co-equal) |
| Autonomy | L1 — Agent suggests, human decides |
| Skills | `/get-pattern`, Read (vision docs, prior features) |
| Output | Mental model alignment. Human writes SCOPE.md after. |
| Gate | None — this is exploratory |

**Key**: Agent reads ALIGNMENT-CRITERIA.md at session start. Every suggestion is implicitly aligned.

### Phase 0.5: Scope Pre-Check (Agent, automatic)

When SCOPE.md is written, agent automatically checks it against vision.

```
Agent reads SCOPE.md + ALIGNMENT-CRITERIA.md
→ Flags: "SCOPE includes MCP query interface, but that's V1.3.
   Current version is V1.2. Proceed anyway?"
→ Confidence: 30% (scope exists, not yet validated by planning)
```

| Aspect | Detail |
|--------|--------|
| Who | Primary Agent (automated) |
| Trigger | SCOPE.md created or modified |
| Skills | `/align` (lightweight — scope only, not full artifacts) |
| Output | Pre-check result: PASS / WARN with suggestions |
| Gate | **GATE 1**: Warnings presented to human before planning starts |

### Phase 1: Plan (Planning Swarm, L3 — Ceded)

Same as current, with additions:

| Addition | What | Why |
|----------|------|-----|
| **ACCEPTANCE-MAP.md** | Planning swarm produces AC-to-test mapping | Makes spec compliance mechanically verifiable |
| **Fitness function list** | Architect identifies which fitness functions apply | Tells validation pipeline what to check |
| **Agent contracts** | Each implementation task gets preconditions/postconditions | Agents know their boundaries |

```
PLANNING SWARM OUTPUT (additions in bold):
├── specification/SPECIFICATION.md
├── architecture/ARCHITECTURE.md (with ADRs)
├── pseudocode/PSEUDOCODE.md
├── ALIGNMENT-REPORT.md
├── IMPLEMENTATION-BRIEF.md
├── **ACCEPTANCE-MAP.md**          ← NEW: AC-ID → test function mapping
├── **FITNESS-FUNCTIONS.md**       ← NEW: which arch checks apply
├── **LAUNCH-PROMPT.md**           ← NEW: proposed impl kickoff prompt for user review
└── GitHub Issue #N (brief as body)
```

| Aspect | Detail |
|--------|--------|
| Autonomy | L3 — Agent plans, human reviews output |
| Gate | **GATE 2**: Vision alignment + human reviews brief + ADRs |
| Confidence | 60% after planning passes vision check |

### Phase 2: Implement (Implementation Swarm, L3 — Ceded)

Same as current, with mandatory changes:

| Change | What |
|--------|------|
| **spec-compile is REQUIRED** | No "if run" — always compile before spawning agents |
| **Agent self-checks** | Every agent prompt includes self-validation block |
| **AC-aware testing** | ndp-tester uses ACCEPTANCE-MAP.md to name tests after ACs |
| **Fitness functions run** | Part of validation, not separate |

Agent self-check block (appended to every agent prompt):
```
BEFORE RETURNING YOUR RESULTS, verify:
1. All files you modified are listed in the brief's "Files to Create/Modify"
2. No TODO, unimplemented!(), or placeholder functions remain
3. Your changes align with the ADRs you retrieved via /get-pattern
4. You have not modified files outside your assigned scope
5. Tests you wrote cover the acceptance criteria assigned to you
If any check fails, fix it. If you cannot fix it, report it.
```

### Phase 3: Validate (5-Tier Pipeline)

This is the biggest structural change. Validation becomes a formal pipeline with a glass box report.

```
TIER 1: DETERMINISTIC (always, already trusted)
  cargo build --workspace
  cargo test --workspace
  cargo clippy --workspace -- -D warnings
  → Binary: PASS/FAIL

TIER 2: FITNESS FUNCTIONS (always, earns trust)
  Banned dependency scan (no DuckDB/Polars/jemalloc)
  Layer dependency rules (tools/ → lib/ → core/, no reverse)
  Anti-stub scan (no todo!/unimplemented! in non-test code)
  File scope check (all modified files appear in brief)
  Struct propagation check (all initializers complete)
  Config schema validation
  → Per-check: PASS/FAIL with evidence

TIER 3: SPECIFICATION COMPLIANCE (always, earns trust)
  Acceptance criteria coverage (from ACCEPTANCE-MAP.md)
  Test count delta (must not decrease)
  New dependency check (must appear in brief)
  → Per-AC: COVERED/NOT COVERED with test function name

TIER 4: RISK CLASSIFICATION (always, earns trust)
  Change scope: narrow/moderate/broad
  Change depth: surface/logic/structural
  Change domain: tooling/platform/core
  → Risk score → escalation decision
  Anomaly detection (file count, diff size, test regressions)
  → Normal/Anomalous with explanation

TIER 5: LLM JUDGE (optional, for medium+ risk)
  Edge-only compliance (no cloud references)
  Config-driven compliance (no hardcoded thresholds)
  Integration-first compliance (extends existing code)
  Intent alignment (diff matches scope)
  → Per-criterion: PASS/FAIL with reasoning chain
```

Output: **Glass Box Validation Report**

```
VALIDATION REPORT: fe-004 Wave 2
=================================
SUMMARY: PASS (22 checks, 0 failures, 1 warning)
CONFIDENCE: 87/100

TIER 1: COMPILATION .............. PASS
  Build: 0 errors
  Tests: 924 (908 existing + 16 new, 0 failures)
  Clippy: 0 warnings

TIER 2: ARCHITECTURE ............. PASS
  Banned deps: PASS (0 found)
  Layer rules: PASS (no upward deps)
  Stub scan: PASS (0 stubs)
  File scope: PASS (8/8 files in brief)
  Config valid: PASS

TIER 3: SPEC COMPLIANCE .......... PASS (10/12 ACs covered)
  AC-01 (embeddings stored): test_embeddings_stored .... PASS
  AC-02 (predictions): test_predictions_after_warmup ... PASS
  ...
  AC-11 (pgvector latency): ........................... NOT COVERED
  AC-12 (full cycle): test_full_cycle_latency ......... PASS

TIER 4: RISK ..................... LOW (narrow + logic + platform = 6)
  Anomalies: none

NOT CHECKED:
  - Integration deploy (no Pi available in this environment)
  - Performance benchmarks (no baseline established)

RECOMMENDED HUMAN REVIEW:
  - AC-11 has no test coverage (pgvector latency)
  - New HybridBronzeReader pattern (first use, no prior examples)
```

| Aspect | Detail |
|--------|--------|
| Autonomy | L0 (today) → L1 (observable) → L2 (shadow) → L3 (risk-gated) |
| Gate | **GATE 3**: Report determines if human review needed |
| Trust tracking | Per-check Bayesian scores in AgentDB, updated after each feature |

### Phase 4: Review (Human, trust-dependent)

**Current** (L0): Human reviews all code.
**Target** (L3): Human reviews the validation report. Spot-checks code on sampling basis.

The transition:

```
SHADOW MODE (L2, 7-25 weeks):
  Agent runs full validation pipeline → produces report
  Human reviews code as usual → records judgment
  Compare: did the report catch everything the human caught?
  Update trust scores per check → reflexion_store(task="trust:validation:*")
  Continue until 95% agreement over 20+ changes

RISK-GATED MODE (L3, after shadow mode):
  LOW risk + all checks pass + trust > 0.90 → auto-approve
  MEDIUM risk + all checks pass → human reviews REPORT only
  HIGH risk or any FAIL → human reviews CODE
  Always: 10% random spot-check of auto-approved changes

REGRESSION:
  Any false negative (report missed what human found) → increase spot-check rate
  Repeated misses → revert to shadow mode
  Every 5th feature → deliberate full human review (keep skills sharp)
```

### Phase 5: Release (Agent + Human, L2-L3)

Same as current, plus:
- Agent generates manifest, tag, changelog
- Agent proposes release (human approves with one command)
- Rollback procedure documented: `git checkout vX.Y.(Z-1) && deploy.sh build && deploy.sh deploy`
- Trust snapshot exported: `.deploy/trust/vX.Y.Z.json` (git-tracked audit trail)

### Phase 6: Learn (Agent, automatic)

Expanded from current:

| Step | What | System |
|------|------|--------|
| /reflexion | Rate each pattern used (per ID) | AgentDB (reflexion table) |
| /save-pattern | Store new discoveries | AgentDB (pattern table) |
| /learner | Analyze reflexion data → update agent definitions | AgentDB + Agent .md files |
| shadow-compare | Compare automated vs human validation (if in shadow mode) | AgentDB (reflexion table, `trust:validation:*` prefix) |
| trust-update | Update per-check Bayesian scores | AgentDB (reflexion table, same prefix) |
| /trust-dashboard | Render human-readable trust summary | Reads AgentDB → displays composite + per-check scores |

**/learner evolution** (new):
```
After feature completion:
  1. Analyze reflexion entries for this feature
  2. Identify top learnings relevant to each agent type
  3. Append to agent definition's "Accumulated Knowledge" section
  4. Cap at 30 lines per agent — prune oldest/lowest-rated
  5. Each learning references source: "(from fe-004 ADR-008, 2026-02-15)"
```

---

## 4. Trust Progression Model

```
                CURRENT STATE                    TARGET STATE
                (Feb 2026)                       (6-12 months)

Scope:          L1 (paired dialog)        →      L1 (keep paired forever)
Planning:       L3 (ceded, review output) →      L3 (add acceptance maps)
Implementation: L3 (ceded, review code)   →      L3 (add agent self-checks)
Validation:     L0 (human does it)        →      L3 (risk-gated, review report)
Release:        L2 (human approves)       →      L3 (agent proposes, one-click)
Learning:       L2 (human triggers)       →      L3 (automatic after feature)
```

The BIG move is validation: L0 → L3. This unlocks the speed gain.

### Autonomy Levels (NDP-specific)

| Level | Description | Human Role | Who's There |
|-------|-------------|------------|-------------|
| L0 | Manual | Human does the work | Validation today |
| L1 | Paired | Human + Agent co-create | Scope/architecture (stay here) |
| L2 | Supervised | Agent does, human reviews all output | Planning, implementation |
| L3 | Risk-gated | Agent does, human reviews report + spot-checks | Validation target |
| L4 | Autonomous | Agent does, human reviews exceptions only | Future (selective) |

### Trust Score Formula

```
Trust(check) = Beta(correct + 1, incorrect + 1)
             = (correct + 1) / (correct + incorrect + 2)

Composite = 0.30 * Validation_Accuracy
          + 0.30 * (1 - Miss_Rate)
          + 0.15 * (1 - Rework_Rate)
          + 0.15 * Scope_Conformance
          + 0.10 * Fitness_Pass_Rate

Advance when: Composite > 0.85 for 5 consecutive features
Regress when: Composite < 0.70 for any feature
Drill:        Every 5th feature, full human review regardless
```

---

## 5. System Connections (Proposed)

Two systems, not three. Trust scores are permanent knowledge — they belong in AgentDB alongside patterns and reflexions.

```
+-------------------------------+       +--------------------+
|           AgentDB             |       | Claude-Flow Memory |
|       (permanent)             |       | (transient)        |
|                               |       |                    |
| Patterns  (architecture)      |       | Spec sections      |
| ADRs      (decisions)         |       | Agent results      |
| Reflexions (pattern feedback) |       | Coordination       |
| Trust scores (validation      |       | (dies w/ swarm)    |
|   feedback via reflexion_store|       |                    |
|   with trust:validation:*     |       |                    |
|   prefix)                     |       |                    |
| Learned skills                |       |                    |
+---------------+---------------+       +---------+----------+
                |                                  |
                v                                  v
+---------------+---------------+       +----------+----------+
| /get-pattern                  |       | /spec-compile       |
| /save-pattern                 |       | /swarm-run          |
| /reflexion                    |       | memory_store/       |
| /learner                      |       |   search/retrieve   |
| /trust-dashboard (NEW)        |       +---------------------+
| /validate (writes trust too)  |                 |
+-------------------------------+       +---------+-----------+
                                        | Fitness Functions    |
                                        | (Rust tests)         |
                                        |                      |
                                        | tests/architecture   |
                                        |   .rs                |
                                        | cargo-deny config    |
                                        +----------------------+
```

### Trust Storage Model

Trust scores reuse AgentDB's `reflexion_store` — because validation feedback IS reflexion. Each check outcome rates the automated pipeline's reliability, exactly like pattern reflexion rates a pattern's usefulness.

```
WRITE (after each validation run):
  reflexion_store(
    task = "trust:validation:tier2:banned_deps",    # namespaced by tier + check
    reward = 1.0,                                    # 1.0 = correct, 0.0 = false negative
    success = true,                                  # automated agreed with human
    critique = "Check caught 0 banned deps. Human confirmed. Feature: fe-004"
  )

READ (dashboard / escalation):
  reflexion_retrieve(task = "trust:validation", limit = 100)

COMPUTE:
  Trust(check) = Beta(correct + 1, incorrect + 1)
  Per-check scores rendered by /trust-dashboard skill

SNAPSHOT (at release):
  .deploy/trust/vX.Y.Z.json  — git-tracked for audit trail
  Human can diff trust progression between releases
```

**Why not a separate trust store?**
- Trust scores ARE reflexion scores — same data model, same table
- No new infrastructure — works today with existing AgentDB
- `/learner` already analyzes reflexion data — trust analysis comes free
- Semantic search works ("which tier 2 checks have been regressing?")
- Single-store rule stays clean: AgentDB = ALL permanent knowledge

### New Artifacts

| Artifact | Produced By | Consumed By | Purpose |
|----------|-------------|-------------|---------|
| ACCEPTANCE-MAP.md | Planning swarm | Implementation swarm + validation | AC → test function mapping |
| FITNESS-FUNCTIONS.md | Planning swarm (architect) | Validation pipeline | Which arch checks apply to this feature |
| Validation Report | Validation pipeline | Human (or auto-approve) | Glass box: what was checked, not checked, uncertain |
| Trust Scores (AgentDB) | /validate + shadow-compare | Escalation decision tree | `reflexion_store` entries with `trust:validation:*` prefix; queried by `/trust-dashboard` |
| Trust Snapshots | Release workflow | Audit trail | `.deploy/trust/vX.Y.Z.json` — git-tracked export at each release |
| Agent Learned Knowledge | /learner | Agent definitions (.md files) | Accumulated conventions per agent type |

---

## 6. Implementation Roadmap

### Phase A: Foundation (weeks 1-2)

Build the trust infrastructure. No autonomy change yet — human still reviews everything.

| Item | Effort | Enables |
|------|--------|---------|
| `tests/architecture.rs` — fitness functions (5-10 rules) | 3h | Tier 2 validation |
| `cargo-deny` configuration | 1h | Dependency governance |
| ACCEPTANCE-MAP.md format + planning protocol update | 2h | Tier 3 validation |
| Validation report template + `/validate` skill update | 3h | Glass box output |
| Agent self-check block in protocols | 1h | Agent-side drift prevention |
| Scope pre-check (lightweight /align on SCOPE.md) | 1h | Gate 1 |
| **Total** | **~11h** | |

### Phase B: Shadow Mode (weeks 3-12)

Run new validation pipeline alongside human review. Build trust evidence.

| Item | Effort | Enables |
|------|--------|---------|
| `/trust-dashboard` skill (queries AgentDB reflexion data) | 2h | Per-check trust tracking + human visibility |
| shadow-compare command (compare automated vs human) | 2h | Trust calibration |
| Human judgment recording (`just shadow-judge approve/reject`) | 1h | Comparison input |
| Trust snapshot export at release (`.deploy/trust/vX.Y.Z.json`) | 1h | Audit trail + human diffing |
| Run shadow mode for 20+ changes (7-10 weeks at 2-3/week) | Time | Trust accumulation |
| **Total** | **~6h setup + 7-10 weeks practice** | |

### Phase C: Risk-Gated (weeks 13+)

Only after shadow mode demonstrates >= 95% agreement, zero false negatives.

| Item | Effort | Enables |
|------|--------|---------|
| Change classification engine (scope × depth × domain) | 3h | Risk-based routing |
| Escalation decision tree | 2h | Auto-approve / human-review / block |
| Anomaly detection rules | 2h | Catches unusual agent behavior |
| LLM judge for intent alignment (optional) | 4h | Tier 5 validation |
| Enable auto-approve for low-risk + canary spot-checking | 1h | The payoff |
| **Total** | **~12h** | |

### Phase D: Agent Evolution (ongoing)

| Item | Effort | Enables |
|------|--------|---------|
| /learner skill update — patch agent definitions | 4h | Agents get smarter |
| Static @file references in agent definitions | 2h | Always-loaded context |
| Knowledge decay detection in AgentDB | 3h | Stale patterns pruned |
| **Total** | **~9h** | |

---

## 7. Key Differences Summary

```
CURRENT:                           PROPOSED:
Scope → Plan → Implement           Dialog → Scope Pre-Check → Plan → Implement
      → Human Validates                  → Validation Pipeline → Human Reviews Report
      → Release → Reflexion              → Release → Learn + Trust Update

Human reviews: CODE                Human reviews: REPORT (+ spot-check code)
Trust: implicit                    Trust: tracked, Bayesian, per-check (in AgentDB)
Fitness functions: none            Fitness functions: 10+ architectural rules
Agent context: on-demand           Agent context: always-loaded + self-checks
Validation: 3 tiers               Validation: 5 tiers with glass box
spec-compile: optional             spec-compile: required
Learning: reflexion only           Learning: reflexion + learner → agent evolution
Trust store: none                  Trust store: AgentDB reflexion table (no new infra)
Concurrent swarms: none            Concurrent swarms: namespace-isolated (Phase D)
```
