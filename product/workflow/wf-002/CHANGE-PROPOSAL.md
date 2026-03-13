# wf-002: Change Proposal — Spec-Driven Development with Risk-Based Testing

> All design decisions resolved. Ready for implementation.

---

## Resolved Decisions

| # | Decision | Resolution |
|---|----------|------------|
| 1 | Phase 2 agent composition | Keep specialists. Arch + Spec parallel (Wave 1), Risk Strategy sequential after both (Wave 2) |
| 2 | Risk-Based Test Strategy author | New `ndp-risk-strategist` agent (pure risk analyst, distinct from ndp-tester) |
| 3 | Human approval gate | Two separate sessions. Design session ends for human review. |
| 4 | SCOPE.md authorship | Research agent + human collaborate interactively. Agent writes SCOPE.md. Human approves. |
| 5 | Synthesizer fate | Stays. Produces IMPLEMENTATION-BRIEF.md (coordinator's operating doc) + ACCEPTANCE-MAP.md + GH Issue |
| 6 | Directory structure | Keep current layout. Add RISK-TEST-STRATEGY.md at feature root. No moves. |
| 7 | Validation cost | One validator agent, three focused spawns. Acceptable trade-off. |
| 8 | Vision guardian | Runs after Phase 2, before human review |
| 9 | Backward compatibility | None. Moving forward only. Past sessions are historic context. |
| 10 | Phase transitions | Gates 3a→3b→3c auto-proceed on pass. Any failure → stop, return to human. |
| 11 | Coordinator model | Two leaders: Design Leader (Phase 2) + Delivery Leader (Phase 3). Start simple, adjust if context pressure emerges. |

---

## Architecture: Two Sessions, Two Leaders

```
SESSION 1 — DESIGN                          SESSION 2 — DELIVERY
═══════════════════                          ════════════════════

Human provides intent                       Human approves design
        ↓                                           ↓
Phase 1: Research & Scope                   Delivery Leader reads:
  ndp-researcher + human collaborate          IMPLEMENTATION-BRIEF.md
  Agent writes SCOPE.md                       (gets component map, ACs, GH Issue #)
  Human approves                                    ↓
        ↓                                   Stage 3a: Component Design
Phase 2: Design                               ndp-pseudocode + ndp-tester
  Wave 1 (parallel):                             → per-component pseudocode
    ndp-architect → ARCHITECTURE.md              → per-component test plans
    ndp-specification → SPECIFICATION.md         ★ Gate 3a: validator ★
        ↓ (sequential)                                ↓
  Wave 2:                                      Stage 3b: Implementation
    ndp-risk-strategist → RISK-TEST-STRAT.md     ndp-rust-dev + specialists
    (reads arch + spec + product vision)         → code + component tests
        ↓                                        ★ Gate 3b: validator ★
  Wave 3:                                             ↓
    ndp-vision-guardian → ALIGNMENT-REPORT.md
        ↓
  Wave 4:
    ndp-synthesizer → IMPLEMENTATION-BRIEF.md
                   + ACCEPTANCE-MAP.md               ↓
                   + GH Issue                   Stage 3c: Testing & Risk Validation
        ↓                                       ndp-tester (execution)
  ★ RETURN TO HUMAN ★                           → integration tests
  Human reviews:                                → risk coverage verification
    - 3 source documents                        → RISK-COVERAGE-REPORT.md
    - alignment report                          ★ Gate 3c: validator ★
    - acceptance map                                 ↓
    - implementation brief                      Phase 4: Delivery
                                                GH Issue updated, results returned
                                                ★ RETURN TO HUMAN ★
```

### Handoff Between Sessions

The **Implementation Brief** is the handoff document. It contains:
- Component Map (which components, which agents)
- Acceptance Map reference (what ACs to verify at each gate)
- GH Issue number (tracking, accumulated context via comments)
- Paths to all three source documents
- Wave structure for delivery

The Delivery Leader reads the brief + three source docs. The GH Issue accumulates context across sessions via comments.

### Gate Behavior

| Gate passes | Action |
|-------------|--------|
| Gate 3a ✓ | Delivery Leader proceeds to Stage 3b automatically |
| Gate 3b ✓ | Delivery Leader proceeds to Stage 3c automatically |
| Gate 3c ✓ | Delivery Leader completes, returns results to human |

| Gate fails | Action |
|------------|--------|
| Reworkable failure | Delivery Leader loops back to previous stage agents (max 2 iterations) |
| Scope/feasibility failure | Delivery Leader stops, returns to human with recommendation |
| Any gate fails after 2 rework iterations | Stop, return to human |

---

## Changes by File

### New Files

#### 1. `.claude/protocols/design-protocol.md`

Phase 2 protocol. Read by the Design Leader (ndp-scrum-master, Session 1).

**Content**:
- Trigger: Approved SCOPE.md exists
- Flow: Read SCOPE.md → Wave 1: spawn architect + specification (parallel) → Wave 2: spawn risk-strategist (reads arch + spec + vision) → Wave 3: spawn vision-guardian → Wave 4: spawn synthesizer (brief + acceptance map + GH Issue) → return results
- Output: Three source documents + alignment report + implementation brief + acceptance map + GH Issue
- Constraint: NO pseudocode, NO component decomposition, NO code
- Session ends after returning results to human

**Agents involved**: ndp-architect, ndp-specification, ndp-risk-strategist, ndp-vision-guardian, ndp-synthesizer

**What it replaces**: Waves 1 + 3 of current `planning-protocol.md`

#### 2. `.claude/protocols/delivery-protocol.md`

Phase 3 protocol. Read by the Delivery Leader (ndp-scrum-master, Session 2).

**Content**:
- Trigger: Human approves three source documents
- Input: IMPLEMENTATION-BRIEF.md (contains component map, ACs, GH Issue #, paths to source docs)
- Flow:
  - Stage 3a: spawn ndp-pseudocode + ndp-tester (component design) → Gate 3a (ndp-validator)
  - Stage 3b: spawn ndp-rust-dev + specialists (implementation) → Gate 3b (ndp-validator)
  - Stage 3c: spawn ndp-tester (execution) → Gate 3c (ndp-validator)
- Gate logic: pass → proceed; reworkable fail → loop back (max 2); scope fail → stop
- Exit: All gates pass → update GH Issue → return results

**What it replaces**: Current `implementation-protocol.md` + Wave 2 of `planning-protocol.md`

**Subsumes**: Component design, implementation, and testing into one protocol with three stages. The Delivery Leader follows one document with clear stage boundaries.

#### 3. `.claude/protocols/research-protocol.md`

Phase 1 protocol. Lightweight — may be used by primary agent directly (no swarm needed).

**Content**:
- Trigger: Human provides high-level feature intent
- Flow: Spawn ndp-researcher → agent explores problem space, technical landscape, constraints → agent + human discuss findings → agent writes SCOPE.md → human approves
- Output: SCOPE.md (agent-authored, human-approved)
- Note: This phase is interactive. The research agent may be spawned as a conversational agent, not a fire-and-forget swarm worker.

#### 4. `.claude/agents/ndp/ndp-researcher.md`

```yaml
name: ndp-researcher
type: specialist
scope: broad
description: Research specialist for problem space exploration, technical landscape analysis, and collaborative scope definition
capabilities:
  - problem_space_exploration
  - technical_landscape_analysis
  - scope_recommendation
  - codebase_analysis
```

**Key behaviors**:
- Explores: existing codebase patterns, docs, AgentDB patterns, technical constraints
- Synthesizes findings into structured research output
- Proposes scope boundaries with rationale
- Collaborates with human — presents options, incorporates feedback
- Writes SCOPE.md when human and agent converge on scope
- Does NOT: make architecture decisions, write code, produce design documents

#### 5. `.claude/agents/ndp/ndp-risk-strategist.md`

```yaml
name: ndp-risk-strategist
type: specialist
scope: broad
description: Risk-based test strategy specialist for feature-level risk identification, test scenario mapping, and risk coverage planning
capabilities:
  - risk_identification
  - risk_prioritization
  - test_scenario_mapping
  - risk_coverage_planning
```

**Key behaviors**:
- Runs in Wave 2 (sequential), after Architecture + Specification complete in Wave 1
- Reads SCOPE.md + Architecture + Specification + product vision criteria
- Identifies: what could fail, what would impact users/business, integration risks, edge cases, failure modes
- Maps each risk to specific testing scenarios
- Prioritizes by severity (impact) x likelihood
- Produces RISK-TEST-STRATEGY.md with:
  - Risk inventory (ID, description, severity, likelihood, impact area)
  - Test scenario mapping (risk → scenarios that prove mitigation)
  - Coverage requirements (what must be tested to close each risk)
  - Priority order (test the highest-risk items first)
- This is NOT unit test strategy. This is "what could go wrong and how do we prove it didn't?"
- Does NOT: write test code, produce per-component test plans, execute tests

**Relationship to other agents**:
- Reads: ndp-architect output (integration risks come from architecture)
- Feeds: ndp-tester in Phase 3a (component test plans root in this strategy) and Phase 3c (risk coverage report maps to this strategy)
- Checked by: ndp-validator at every gate (traceability back to this document)

### Modified Files

#### 6. `.claude/protocols/swarm-protocol.md` → MODIFIED

**Changes**:
- Update execution model to describe 4-phase lifecycle with two sessions
- Replace protocol table:
  ```
  | Session | Leader | Protocol |
  |---------|--------|----------|
  | Session 1 (Design) | Design Leader | .claude/protocols/design-protocol.md |
  | Session 2 (Delivery) | Delivery Leader | .claude/protocols/delivery-protocol.md |
  ```
- Add section: "Two Leaders, Two Sessions" explaining the handoff model
- Add section: "Three Validation Gates" with gate pass/fail behavior
- Add section: "Two-Tier Escalation" (reworkable vs. scope/feasibility)
- Preserve: All MCP/memory/hive conventions, agent ID activation, spawn patterns, anti-drift config

#### 7. `.claude/protocols/agent-routing.md` → MODIFIED

**Changes**:
- Add ndp-researcher and ndp-risk-strategist to roster
- Replace planning swarm template with design swarm template
- Replace implementation swarm template with delivery swarm template (3 stages)
- Update composition rules for new phase structure
- New templates:

```
Design Swarm (Session 1):
  Leader: ndp-scrum-master (Design Leader)
  Wave 1: ndp-architect, ndp-specification (parallel)
  Wave 2: ndp-risk-strategist (reads arch + spec + product vision)
  Wave 3: ndp-vision-guardian (reads all 3 source docs)
  Wave 4: ndp-synthesizer (fresh context)

Delivery Swarm (Session 2):
  Leader: ndp-scrum-master (Delivery Leader)
  Stage 3a: ndp-pseudocode, ndp-tester (parallel) → ndp-validator (Gate 3a)
  Stage 3b: ndp-rust-dev, specialists (parallel) → ndp-validator (Gate 3b)
  Stage 3c: ndp-tester (execution) → ndp-validator (Gate 3c)
```

#### 8. `.claude/agents/ndp/ndp-scrum-master.md` → MODIFIED

**Changes**:
- Update protocol table to reference design-protocol.md and delivery-protocol.md
- Two operating modes:
  - **Design Leader**: Follows design-protocol.md. Spawns design agents, vision guardian, synthesizer. Returns three source docs + brief + GH Issue. Session ends.
  - **Delivery Leader**: Follows delivery-protocol.md. Reads IMPLEMENTATION-BRIEF.md. Runs three stages with gates. Auto-proceeds on pass, stops on failure. Returns final results.
- Add gate pass/fail behavior documentation
- Add two-tier escalation model
- Remove references to current planning-protocol.md and implementation-protocol.md
- Preserve: Agent spawning mechanics, memory conventions, GH Issue lifecycle, learning gate, component map routing, exit gate checklist

#### 9. `.claude/agents/ndp/ndp-validator.md` → MODIFIED

**Changes**: Add three gate modes alongside existing planning/implementation validation.

**Gate 3a — Component Design Validation**:
- Input: Per-component pseudocode + test plans
- Validates against: Architecture, Specification, Risk-Based Test Strategy
- Checks:
  1. Each component aligns with approved Architecture (boundaries, interfaces)
  2. Pseudocode implements what Specification requires (functional coverage)
  3. Component test plans address risks from RISK-TEST-STRATEGY.md (risk traceability)
  4. Component interfaces are consistent with architecture contracts
- Trust check names: `gate-3a:arch_alignment`, `gate-3a:spec_coverage`, `gate-3a:risk_traceability`, `gate-3a:interface_consistency`

**Gate 3b — Code Implementation Validation**:
- Input: Implemented code + test cases
- Validates against: Pseudocode (from 3a), Architecture, Specification
- Checks:
  1. Code matches validated pseudocode from Stage 3a
  2. Implementation aligns with approved Architecture
  3. Component interfaces implemented as specified
  4. Test cases match component test plans
  5. Compilation passes, no stubs (existing Tier 1 checks)
- Trust check names: `gate-3b:pseudocode_match`, `gate-3b:arch_alignment`, `gate-3b:interface_impl`, `gate-3b:test_plan_match`, `gate-3b:compilation`

**Gate 3c — Risk Coverage Validation**:
- Input: Test results + RISK-COVERAGE-REPORT.md
- Validates against: RISK-TEST-STRATEGY.md, Specification, Architecture
- Checks:
  1. Test results prove identified risks are mitigated (risk-to-test mapping)
  2. Test coverage matches Risk-Based Test Strategy (no uncovered risks)
  3. All risks from Phase 2 have test coverage (completeness)
  4. Delivered code matches approved Specification (final spec compliance)
  5. System architecture matches approved Architecture (final arch compliance)
- Trust check names: `gate-3c:risk_mitigation`, `gate-3c:strategy_coverage`, `gate-3c:risk_completeness`, `gate-3c:spec_compliance`, `gate-3c:arch_compliance`

**Spawn interface**: The Delivery Leader passes `gate: "3a"|"3b"|"3c"` in the validator's prompt. The validator uses this to select the appropriate check set.

#### 10. `.claude/agents/ndp/ndp-specification.md` → MODIFIED

**Changes**:
- Now produces Specification as one of three Phase 2 source documents
- Enhanced document scope:
  - Detailed feature requirements
  - User workflows and use cases
  - Functional AND non-functional requirements
  - Success criteria and acceptance conditions
  - Domain models and ubiquitous language
- Remove TASK-DECOMPOSITION.md from output (component decomposition moves to Phase 3a)
- TASK-DECOMPOSITION.md becomes the pseudocode agent's responsibility (as COMPONENT-MAP.md or embedded in pseudocode/OVERVIEW.md)

#### 11. `.claude/agents/ndp/ndp-architect.md` → MODIFIED

**Changes**:
- Phase 2 emphasis: component breakdown and boundaries, interfaces/contracts/data flow, technology stack with versions, integration points
- Still owns ADR lifecycle (create, store, prune, deprecate)
- Still produces Integration Surface analysis for cross-boundary features
- Runs in Wave 1 parallel with ndp-specification. Risk strategist runs after in Wave 2.

#### 12. `.claude/agents/ndp/ndp-tester.md` → MODIFIED

**Changes**:
- Phase 2: Does NOT participate (ndp-risk-strategist handles risk strategy)
- Phase 3a: Produces per-component test plans DERIVED FROM the Risk-Based Test Strategy (existing behavior, but now explicitly rooted in risk strategy rather than ad-hoc)
- Phase 3c: Executes all tests — component tests, integration tests, feature-level tests mapped to risk strategy. Produces RISK-COVERAGE-REPORT.md mapping test results to identified risks.
- New output artifact: RISK-COVERAGE-REPORT.md

#### 13. `.claude/agents/ndp/ndp-synthesizer.md` → MODIFIED

**Changes**:
- Input: Three source documents (Architecture, Specification, Risk-Based Test Strategy) + ALIGNMENT-REPORT.md + ADR pattern IDs
- Output (unchanged in kind, updated in source):
  1. IMPLEMENTATION-BRIEF.md — coordinator's operating document. Contains: component map, resolved decisions table (from ADRs), file paths, data structures, constraints, acceptance criteria references, wave structure for delivery
  2. ACCEPTANCE-MAP.md — AC-ID to verification method mapping
  3. GH Issue — created from brief
- LAUNCH-PROMPT.md: Removed. Human initiates Session 2 directly.
- The brief now synthesizes from 3 source docs instead of full SPARC artifact tree. Cleaner input, same output purpose.

### Retired Files

#### 14. `.claude/protocols/planning-protocol.md` → RETIRED

Replaced by `design-protocol.md` (Phase 2) + delivery-protocol.md Stage 3a (component design).

The current 3-wave planning structure no longer maps to the new lifecycle. Keeping it would create ambiguity about which protocol to follow.

#### 15. `.claude/protocols/implementation-protocol.md` → RETIRED

Replaced by `delivery-protocol.md` (Phases 3a + 3b + 3c).

The delivery protocol subsumes component design, implementation, and testing into one document with stage boundaries and gates.

### Updated CLAUDE.md

#### Feature Directory Structure

```
product/features/{phase}-{NNN}/
├── SCOPE.md                    # Phase 1 output (agent-authored, human-approved)
├── specification/              # Phase 2 source document
│   └── SPECIFICATION.md
├── architecture/               # Phase 2 source document
│   └── ARCHITECTURE.md
├── RISK-TEST-STRATEGY.md       # Phase 2 source document (NEW)
├── ALIGNMENT-REPORT.md         # Phase 2 vision check
├── IMPLEMENTATION-BRIEF.md     # Phase 2 synthesizer output (coordinator's doc)
├── ACCEPTANCE-MAP.md           # Phase 2 synthesizer output
├── pseudocode/                 # Phase 3a component designs
│   ├── OVERVIEW.md
│   └── {component}.md
├── test-plan/                  # Phase 3a component test plans
│   ├── OVERVIEW.md
│   └── {component}.md
├── testing/                    # Phase 3c (NEW)
│   └── RISK-COVERAGE-REPORT.md
├── reports/                    # Validation gate reports
│   ├── gate-3a-report.md
│   ├── gate-3b-report.md
│   └── gate-3c-report.md
├── refinement/                 # SPARC R (if needed)
└── completion/                 # SPARC C (if needed)
```

Changes from current:
- SCOPE.md now agent-authored (was human-only)
- RISK-TEST-STRATEGY.md added (new Phase 2 artifact)
- `testing/` directory added (Phase 3c output)
- `reports/` now contains per-gate reports (was single validation report)
- LAUNCH-PROMPT.md removed (human initiates directly)

#### Non-Negotiable Rules Update

Rule 1 becomes:
> Feature work follows a 4-phase lifecycle with two sessions. Session 1 (Design): `ndp-scrum-master` as Design Leader reads `design-protocol.md`. Returns three source documents for human approval. Session 2 (Delivery): `ndp-scrum-master` as Delivery Leader reads `delivery-protocol.md`. Runs Stages 3a→3b→3c with validation gates. No solo feature work.

---

## Agent Roster (Updated)

### Coordination (2 agents — mandatory)

| Agent | When | Role |
|-------|------|------|
| ndp-scrum-master | Both sessions | Design Leader (Session 1) or Delivery Leader (Session 2) |
| ndp-validator | Session 2 | Spawned 3x: Gate 3a, Gate 3b, Gate 3c |

### Research (1 agent — Phase 1)

| Agent | When | Role |
|-------|------|------|
| ndp-researcher (NEW) | Phase 1 | Problem space exploration, collaborative scope definition |

### Design (4 agents — Session 1)

| Agent | When | Output |
|-------|------|--------|
| ndp-architect | Phase 2 | ARCHITECTURE.md + ADRs in AgentDB |
| ndp-specification | Phase 2 | SPECIFICATION.md |
| ndp-risk-strategist (NEW) | Phase 2 | RISK-TEST-STRATEGY.md |
| ndp-vision-guardian | Phase 2 | ALIGNMENT-REPORT.md |

### Synthesis (1 agent — Session 1)

| Agent | When | Output |
|-------|------|--------|
| ndp-synthesizer | Phase 2 | IMPLEMENTATION-BRIEF.md + ACCEPTANCE-MAP.md + GH Issue |

### Component Design (2 agents — Stage 3a)

| Agent | When | Output |
|-------|------|--------|
| ndp-pseudocode | Stage 3a | pseudocode/OVERVIEW.md + per-component files |
| ndp-tester | Stage 3a | test-plan/OVERVIEW.md + per-component files |

### Implementation (variable — Stage 3b)

| Agent | When | Role |
|-------|------|------|
| ndp-rust-dev | Stage 3b | General Rust implementation |
| ndp-parquet-dev | Stage 3b | Bronze layer work |
| ndp-timescale-dev | Stage 3b | Silver layer work |
| ndp-analytics-engineer | Stage 3b | Gold layer work |
| ndp-dq-engineer | Stage 3b | Data quality work |
| ndp-feature-engineer | Stage 3b | ML feature work |
| ndp-ml-engineer | Stage 3b | Model work |
| ndp-grafana-dev | Stage 3b | Dashboard work |
| ndp-alert-engineer | Stage 3b | Alert/trigger work |
| ndp-meteorologist | Stage 3b | Weather domain |
| ndp-air-quality-specialist | Stage 3b | AQI domain |

### Testing (1 agent — Stage 3c)

| Agent | When | Output |
|-------|------|--------|
| ndp-tester | Stage 3c | Test execution + RISK-COVERAGE-REPORT.md |

**Total: 19 agents** (2 coordination + 1 research + 4 design + 1 synthesis + 2 component design + variable implementation + 1 testing)

---

## Implementation Plan

### Priority 1: Core Protocol Files
1. Write `design-protocol.md`
2. Write `delivery-protocol.md`
3. Write `research-protocol.md`

### Priority 2: New Agent Definitions
4. Write `ndp-researcher.md`
5. Write `ndp-risk-strategist.md`

### Priority 3: Modified Agent Definitions
6. Update `ndp-scrum-master.md` — two leader modes, gate logic, escalation
7. Update `ndp-validator.md` — three gate modes with specific checks
8. Update `ndp-synthesizer.md` — new input (3 source docs), drop LAUNCH-PROMPT.md
9. Update `ndp-specification.md` — remove TASK-DECOMPOSITION.md, enhance spec scope
10. Update `ndp-architect.md` — Phase 2 emphasis
11. Update `ndp-tester.md` — Phase 3a + 3c roles, RISK-COVERAGE-REPORT.md

### Priority 4: Infrastructure Updates
12. Update `swarm-protocol.md` — lifecycle overview, two-session model
13. Update `agent-routing.md` — new roster, new composition templates
14. Update `CLAUDE.md` — directory structure, Rule 1, remove LAUNCH-PROMPT.md
15. Retire `planning-protocol.md` and `implementation-protocol.md`

---

## Risk Assessment

| Risk | Severity | Mitigation |
|------|----------|------------|
| Human approval gate adds latency | Medium | Intentional — catches design flaws before code |
| 3 validation gates increase cost | Medium | Focused gates prevent expensive late rework |
| Delivery Leader context pressure | Medium | Monitor. If problematic, split into Coding + Testing leaders |
| Risk-Based Test Strategy is new concept | Medium | Start with one feature, refine agent definition |
| Protocol retirement breaks muscle memory | Low | Clean break — no backward compat, just the new way |
| 4 sequential waves add latency to Session 1 | Low | Better risk quality justifies the extra wave. Risk strategy grounded in actual arch + spec decisions prevents rework downstream |

---

## Next Steps

1. Implement Priority 1-2 (protocols + new agents)
2. Implement Priority 3 (modified agents)
3. Implement Priority 4 (infrastructure)
4. Test on one feature end-to-end
5. Refine based on results
