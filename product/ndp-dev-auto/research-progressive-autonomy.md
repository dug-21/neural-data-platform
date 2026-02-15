# Research: Progressive Autonomy in AI-Assisted Development

**Date**: 2026-02-15
**Scope**: Frameworks, patterns, and metrics for gradually ceding developer control to AI agent swarms
**Context**: Solo developer with NDP project, current flow is scope -> planning swarm -> implementation swarm -> human validates -> release

---

## 1. Key Frameworks Discovered

### 1.1 Sheridan-Verplanck 10-Level Automation Scale (1978)

The foundational taxonomy for human-machine task allocation. Originally developed at MIT for undersea teleoperators, it defines a 10-point continuum from fully manual to fully autonomous. The critical insight is that automation is not binary -- it is a spectrum of decision authority allocation between human and machine.

| Level | Description |
|-------|-------------|
| 1 | Human does everything |
| 2 | Computer offers alternatives |
| 3 | Computer narrows to a few options |
| 4 | Computer suggests one action |
| 5 | Computer executes if human approves |
| 6 | Computer executes, human can veto within window |
| 7 | Computer executes, informs human afterward |
| 8 | Computer executes, informs human only if asked |
| 9 | Computer executes, informs human only if it decides to |
| 10 | Computer does everything autonomously |

**Relevance to NDP**: The current workflow sits at approximately Level 5-6 for planning/implementation (agent proposes, human approves/vetoes) but Level 2-3 for validation (human does it, with agent assistance). The bottleneck is asymmetric authority allocation.

Source: Sheridan & Verplank, "Human and Computer Control of Undersea Teleoperators" (MIT, 1978), via [ResearchGate](https://www.researchgate.net/figure/Sheridan-and-Verplanks-original-levels-of-automation-2_tbl1_337253476)

### 1.2 ASDLC Levels of Autonomy (2025)

The Autonomous Software Development Lifecycle project adapted the SAE J3016 self-driving taxonomy to software development. It standardizes on L3 (Conditional Autonomy) as the practical frontier.

| Level | Name | Human Role | AI Role | Failure Mode |
|-------|------|-----------|---------|--------------|
| L1 | Assistive | Driver, hands on wheel 100% | Suggestions, zero state retention | Distraction, minor syntax errors |
| L2 | Task-Based | Reviewer checking before commit | Operates on individual files | Logic bugs within single file |
| L3 | Conditional | Instructor defining constraints | Multi-file orchestration | Regression toward mediocrity |
| L4 | High | Auditor, post-hoc analysis | Self-directed planning | Silent failure, strategic drift |
| L5 | Full | Consumer, passive beneficiary | Fully autonomous | Existential alignment drift |

**Key finding**: Research shows engineers "fully delegate only 0-20% of work" with an average of "4.1 human turns per Claude Code session." This means even developers who believe they are at L3 are actually performing L2-style micro-reviews.

**Relevance to NDP**: The current workflow is solidly L3 for planning and implementation. The goal is to reach L3-L4 for validation (agent validates with human auditing), which requires the trust infrastructure described below.

Source: [ASDLC.io Levels of Autonomy](https://asdlc.io/concepts/levels-of-autonomy/)

### 1.3 SAE J3016 / Self-Driving Analogy

The SAE levels provide the most culturally understood autonomy framework. The critical insight for software development is the concept of the "Operational Design Domain" (ODD) -- the specific conditions under which the automation is designed to function.

- L3 self-driving works on highways but not city streets.
- L3 agent autonomy works for well-tested CRUD features but not novel architecture.

This maps directly to NDP: the "ODD" for agent autonomy is the combination of (task complexity) x (test coverage) x (pattern familiarity). Agents can be granted more autonomy within a well-characterized ODD and less in uncharted territory.

Source: [SAE J3016 Levels of Driving Automation](https://www.sae.org/news/blog/sae-levels-driving-automation-clarity-refinements), [Tessl.io Application to AI Agents](https://tessl.io/blog/the-5-levels-of-ai-agent-autonomy-learning-from-self-driving-cars/)

### 1.4 NASA Human-Autonomy Teaming (HAT)

NASA's approach to human-machine teaming introduces several concepts directly applicable to AI development workflows:

**Transparency as trust enabler**: "It falls to the designer to ensure that the system is trustworthy, but future systems also require a sufficiently high level of user trust in the automated actions being performed. The approach to encouraging trust is for the machine partner to provide the user the bases for its decisions."

**Graduated authority**: The level of automation can range from fully manual (Level 1) to fully autonomous (Level 10). Increasingly autonomous systems are characterized by "the ability to independently assume functions with less human intervention overall and for longer periods of time."

**Teaming vs. tooling**: "The concept of a human teaming with a technology fundamentally alters the assumptions of traditional human-automation interaction paradigms, as the technology is required to assume many of the responsibilities (and authorities) traditionally held by humans."

**Relevance to NDP**: The distinction between "teaming" and "tooling" is exactly the distinction between paired architecture design (teaming) and ceded validation (tooling). Different phases of the workflow demand different interaction paradigms.

Source: [NASA Human-Autonomy Teaming](https://ntrs.nasa.gov/api/citations/20190002683/downloads/20190002683.pdf), [NASA CSAOB](https://csaob.larc.nasa.gov/nasahrpiws/humansystemsintegration_cyberteaming/)

### 1.5 Aviation "Trust But Verify"

Aviation's approach to autopilot trust provides the most battle-tested model for progressive autonomy in safety-critical systems.

**Key principles from the Flight Safety Foundation**:

1. **Measured skepticism builds better trust than blind faith**: "I try never to totally trust the automation, and I make every attempt to verify that the automation is doing what I expect."
2. **Deliberate degradation maintains capability**: Many pilots intentionally use simpler automation modes to maintain engagement and situational awareness.
3. **Verbal confirmation makes automation state observable**: Crews verbalize every automation mode change ("Audible callouts, point and say").
4. **Override capability must always exist**: "When I become task saturated with programming automation, I click off the autopilot and fly the airplane."
5. **Trust is proportional to demonstrated reliability, not sophistication**.

**Relevance to NDP**: The "deliberate degradation" concept is profound. Occasionally running a task at a lower autonomy level (human validates instead of agent validates) keeps the human calibrated and able to catch drift. This should be a built-in practice, not an emergency fallback.

Source: [Flight Safety Foundation - Trust but Verify](https://flightsafety.org/asw-article/trust-but-verify/)

---

## 2. Applicable Patterns for the NDP Context

### 2.1 The Three Developer Loops Framework

IT Revolution's framework divides AI-assisted development into three temporal loops:

**Inner Loop (seconds-minutes)**: Immediate collaboration with AI. The compile-test-debug cycle becomes request-output-verify. Success requires mastering task decomposition, frequent checkpointing, and verification practices.

**Middle Loop (hours-days)**: Context management between sessions. Key strategies include:
- External Memory Systems: AI documents progress, plans, and insights in persistent files before ending sessions
- Golden Rules Documentation: Project-specific guidelines injected into every conversation (CLAUDE.md, AGENTS.md)

**Outer Loop (weeks-months)**: Architectural governance, CI/CD integration, recovery procedures. Establishing outer loop governance includes implementing architectural safeguards, enhanced CI/CD, and recovery procedures.

**NDP mapping**:
- Inner loop: Already strong (claude-flow, MCP tools, agent swarms)
- Middle loop: Strong (AgentDB patterns, MEMORY.md, reflexion workflow)
- Outer loop: THIS IS THE GAP. Validation is manual. Architectural governance is human-dependent. This is where progressive autonomy should focus.

Source: [IT Revolution - The Three Developer Loops](https://itrevolution.com/articles/the-three-developer-loops-a-new-framework-for-ai-assisted-coding/)

### 2.2 Spotify's Verification Loop Pattern

Spotify's background coding agent system (1,500+ merged PRs) provides the most mature production model for trusted agent output. Their key insight: **trust comes from constraints, not capabilities**.

**Three failure modes** (in order of severity):
1. Agent fails to produce a PR -- minor inconvenience
2. PR fails CI -- frustrating but caught
3. PR passes CI but is functionally incorrect -- **erodes trust**, most dangerous

**Verification architecture**:
- **Deterministic verifiers**: Auto-activated based on codebase content (Maven verifier triggers on pom.xml). Build, test, format checks. Agent does not know how verification works -- it only knows it can call the verifier.
- **LLM judge layer**: Reviews proposed diffs against original prompts. Catches agents acting "too ambitious." Vetoes ~25% of sessions; agents self-correct ~50% of the time.
- **Bounded access**: Agents can view code, edit files, and run verifiers. They CANNOT push code, interact with Slack, or author prompts.

**Key metric**: "Reduced flexibility makes it more predictable."

**NDP application**: This is directly applicable. Build a verification layer where:
1. `ndp validate` runs deterministic checks (already exists)
2. An LLM judge compares the diff against the SCOPE.md/IMPLEMENTATION-BRIEF.md
3. Agent output is sandboxed until verification passes
4. Human reviews only the judge's report, not every line of code

Source: [Spotify Engineering - Feedback Loops for Background Coding Agents](https://engineering.atspotify.com/2025/12/feedback-loops-background-coding-agents-part-3)

### 2.3 Conductor-to-Orchestrator Progression

Addy Osmani (Google) and Nicholas Zakas describe a three-stage evolution of the developer role:

1. **Autocomplete (2024)**: AI as enhanced code completion. Human drives.
2. **Conductor (2025)**: Human gives prompts, reviews each step in real-time tight feedback loop. Human navigates.
3. **Orchestrator (2025+)**: Human defines tasks, delegates to parallel agents, reviews completed PRs asynchronously. Human audits.

**Key practices for orchestration**:
- Workspace isolation: each agent works on its own Git branch
- Artifact preservation: branches, commits, and PRs document agent work
- Hybrid workflows: seamlessly switch between real-time collaboration and async delegation based on task complexity
- Drop-down capability: when agents underperform, orchestrator can "drop down to conductor mode"

**NDP mapping**: The current workflow is already Conductor/Orchestrator for implementation. The missing piece is orchestration of validation -- having agents run validation pipelines and present results for human audit rather than requiring human execution.

Source: [O'Reilly - Conductors to Orchestrators](https://www.oreilly.com/radar/conductors-to-orchestrators-the-future-of-agentic-coding/), [Human Who Codes](https://humanwhocodes.com/blog/2026/01/coder-orchestrator-future-software-engineering/)

### 2.4 Architecture Fitness Functions

From Neal Ford's "Building Evolutionary Architectures," fitness functions are automated tests that validate architectural characteristics -- not just functional correctness but structural properties of the system.

**Types**:
- **Atomic**: Test one characteristic (e.g., no module depends on a deprecated crate)
- **Holistic**: Test emergent properties (e.g., end-to-end latency under load)
- **Triggered**: Run on change (CI)
- **Continuous**: Run always (monitoring)

**Examples relevant to NDP**:
- "Bronze layer writes only via WAL" (structural constraint)
- "No DuckDB or Polars streaming imports exist" (deprecated approach guard)
- "All StreamConfig initializers include all required fields" (struct propagation)
- "Silver ETL reads from HybridBronzeReader, not raw Parquet" (architectural intent)
- "Docker image size < threshold" (operational constraint)

**Key insight**: Fitness functions encode architectural INTENT in executable form. When an agent system can run fitness functions, it can validate architecture without understanding the reasoning behind it. This is the bridge from "human validates architecture" to "system validates architecture."

Source: [Continuous Architecture - Fitness Functions](https://continuous-architecture.org/practices/fitness-functions/), [AWS Architecture Blog](https://aws.amazon.com/blogs/architecture/using-cloud-fitness-functions-to-drive-evolutionary-architecture/)

### 2.5 Agent Contracts (Relari)

The Agent Contracts framework provides a formal structure for defining, verifying, and certifying AI agent behavior. It addresses the fundamental challenge that AI systems exhibit probabilistic behavior that is hard to validate.

**Three-part contract structure**:
- **Preconditions**: What must be true before the agent executes (e.g., SCOPE.md exists, tests pass on main)
- **Pathconditions**: Constraints on the agent's process (e.g., must not modify files outside feature directory, must run tests before committing)
- **Postconditions**: What must be true after execution (e.g., all tests pass, no new warnings, diff matches scope)

**Verification modes**:
- **Offline**: Run agents through predefined test scenarios after development
- **Runtime**: Monitor agents during execution, continuously validating behavior

**NDP application**: Define contracts for each swarm phase:
- Planning contract: output must be an IMPLEMENTATION-BRIEF.md that covers all SCOPE.md requirements
- Implementation contract: all code changes must be within declared scope, tests must pass, no TODOs
- Validation contract: all fitness functions pass, diff risk assessment below threshold, no regressions

Source: [Relari Agent Contracts on GitHub](https://github.com/relari-ai/agent-contracts)

### 2.6 Contract-First / Spec-Driven Development

GitHub's Spec Kit (2025) formalizes what NDP already does informally with SCOPE.md and IMPLEMENTATION-BRIEF.md. The pattern places specifications at the center of the engineering process.

Key properties of spec-driven development:
- Specs drive implementation, checklists, and task breakdowns
- A "constitution" encodes organizational standards and tech-stack preferences
- Specs are testable -- you can verify whether implementation satisfies the spec
- Specs create a natural validation boundary: does the output conform to the input?

**NDP application**: The existing SCOPE.md -> IMPLEMENTATION-BRIEF.md -> implementation flow is already spec-driven. The gap is automated conformance checking: no system currently verifies that the implementation matches the brief.

Source: [ASDLC.io](https://asdlc.io/concepts/levels-of-autonomy/), [Microsoft AI-led SDLC](https://techcommunity.microsoft.com/blog/appsonazureblog/an-ai-led-sdlc-building-an-end-to-end-agentic-software-development-lifecycle-wit/4491896)

---

## 3. Proposed Autonomy Levels for AI Dev Workflow

Based on the research above, here is a 6-level taxonomy specific to an AI-assisted development workflow where a solo developer manages agent swarms. Each level describes who does what, what trust evidence is required to advance, and what regression signals should trigger a step back.

### Level 0: Manual with AI Assistance

**Who does what**: Human writes code, AI suggests completions and answers questions.
**Validation**: Entirely human.
**Trust evidence to advance**: N/A -- this is the starting point.

### Level 1: Paired Design, Ceded Drafting

**Who does what**: Human and AI co-design architecture through dialog. AI drafts implementation. Human reviews every file.
**Validation**: Human runs tests, reads diffs, approves each change.
**Trust evidence to advance**: 3+ features delivered where AI drafts required < 20% rework.

### Level 2: Ceded Planning, Supervised Implementation

**Who does what**: Human writes scope. AI swarm produces planning artifacts (IMPLEMENTATION-BRIEF). AI swarm implements. Human reviews implementation against brief.
**Validation**: Human validates. Deterministic checks (compile, test, lint) run automatically.
**Trust evidence to advance**: 5+ planning cycles where brief accurately predicted implementation shape. Test suite catches > 90% of issues before human review.

**NDP is approximately here.**

### Level 3: Ceded Implementation, Structured Validation

**Who does what**: Human writes scope and reviews planning output. AI swarm implements without per-file human review. AI runs validation pipeline and presents structured report.
**Validation**: Multi-layer automated validation:
1. Deterministic: compile, test, lint, format (PASS/FAIL)
2. Fitness functions: architectural constraints (PASS/FAIL with explanation)
3. LLM judge: diff-vs-scope conformance (PASS/WARN/FAIL with reasoning)
4. Risk assessment: change classification (routine/notable/significant/risky)

Human reviews the validation REPORT, not the code. Human spot-checks code only when report flags issues or on a random sampling basis.

**Trust evidence to advance**: 10+ features where validation report accurately predicted quality. Zero "passed validation but broke in production" incidents. LLM judge catches issues human would have caught.
**Regression signal**: Validation report misses an issue human later finds. Reset to more frequent spot-checking.

### Level 4: Ceded Routine Validation, Human on Exceptions

**Who does what**: Human writes scope. AI handles planning, implementation, and validation end-to-end for "routine" task types (bug fixes, dependency updates, config changes, well-patterned features). Human reviews only exceptions flagged by the validation pipeline.
**Validation**: Same multi-layer pipeline as Level 3, but with auto-approval for routine tasks that pass all checks. Human reviews:
- Any task with risk > "routine"
- Any validation with WARN or FAIL
- Random 10-20% sample of auto-approved tasks (aviation-style spot checks)
**Trust evidence to advance**: 20+ auto-approved tasks with zero quality issues found in spot checks. Validation pipeline demonstrates consistent sensitivity (catches real issues) and specificity (low false positive rate).
**Regression signal**: Spot check reveals missed issue. Increase spot-check rate. If repeated, regress to Level 3.

### Level 5: Full Autonomy with Audit

**Who does what**: Human defines product direction and architectural principles. AI manages backlog, plans, implements, validates, and releases for all task types. Human performs periodic audits (weekly/monthly) of cumulative output.
**Validation**: Fully automated pipeline with self-monitoring. System flags its own confidence levels. Human reviews:
- Aggregated metrics dashboards
- Architectural drift reports
- Confidence trend lines
- Selected deep-dives on flagged items
**Trust evidence**: This level may not be desirable for a solo developer. The value of the human-in-the-loop for architecture may exceed the cost of the time spent. This level is included for completeness.
**Regression signal**: Architectural drift detected. Confidence metrics declining. Metrics show divergence from product intent.

### Level Transitions Summary

```
L0 -> L1: Start using AI. No prerequisites.
L1 -> L2: AI drafts are reliable (< 20% rework over 3+ features).
L2 -> L3: Planning output is accurate. Test suite is trustworthy. [NEXT STEP FOR NDP]
L3 -> L4: Validation report is accurate over 10+ features. Zero misses.
L4 -> L5: Validation pipeline has proven sensitivity/specificity over 20+ auto-approved tasks.
```

### Regression Protocol

Inspired by aviation's "deliberate degradation" principle:

1. **Scheduled regression**: Every N features, intentionally validate at one level lower. This keeps the human calibrated and able to detect drift.
2. **Triggered regression**: Any validation miss triggers temporary regression with increased spot-checking.
3. **Domain-specific levels**: Different parts of the codebase can be at different autonomy levels. Core architecture: L2. Well-tested utilities: L4. New experimental features: L1.

---

## 4. Confidence Metrics That Could Be Tracked

### 4.1 Trust Calibration (from Bayesian/SDT Research)

Research on adaptive trust calibration found that:
- A **three-checkpoint moving average** detects inappropriate trust patterns
- Trust calibration cues are most effective when **adaptive** (only shown when over-trust or under-trust is detected)
- **Verbal cues** (explaining WHY something may be wrong) outperform visual indicators by 3x
- Approximately **40% improvement in trust** and **5% improvement in team performance** when machines provide self-assessment of their confidence

**Applicable metrics**:

| Metric | What It Measures | How to Compute |
|--------|-----------------|----------------|
| **Validation Accuracy Rate** | Does the automated validation pipeline agree with human judgment? | (Auto-pass that human would pass + Auto-fail that human would fail) / Total |
| **Miss Rate** | How often does auto-validation miss a real issue? | Issues found by human after auto-pass / Total auto-passes |
| **False Alarm Rate** | How often does auto-validation flag non-issues? | False flags / Total auto-flags |
| **Rework Rate** | How much human modification is needed post-agent-implementation? | Lines changed by human / Lines produced by agent |
| **Scope Conformance** | Does implementation match the spec? | Checklist items from SCOPE.md satisfied / Total items |
| **Architectural Drift** | Are fitness functions degrading over time? | Fitness function pass rate over rolling window |
| **Agent Confidence Calibration** | When the agent says "high confidence," is it right? | Accuracy within self-reported confidence buckets |
| **Time-to-Issue** | How quickly are agent-introduced issues discovered? | Time between agent commit and issue detection |

### 4.2 DORA Metrics for AI-Assisted Development

The 2025 DORA report (Google) found critical insights about AI and quality:

- **90% of engineering teams** have integrated AI tools
- **Only 3%** report "high trust" in AI-generated output
- AI adoption has a **positive relationship with throughput** but a **negative relationship with stability**
- The central finding: "AI doesn't fix a team; it amplifies what's already there"

**Implication for NDP**: Before increasing autonomy, ensure the existing feedback loops (tests, fitness functions, validation pipeline) are robust. AI acceleration without strong control systems leads to instability. The test suite is the prerequisite, not the autonomy level.

**Recommended DORA-style metrics**:
- Deployment frequency (can increase with autonomy)
- Lead time for changes (should decrease)
- Change failure rate (MUST NOT increase -- this is the critical constraint)
- Time to restore service (should remain stable)

Source: [DORA 2025 Report](https://dora.dev/research/2025/dora-report/), [InfoQ Analysis](https://www.infoq.com/news/2025/09/dora-state-of-ai-in-dev-2025/)

### 4.3 Spotify-Inspired Operational Metrics

| Metric | Target | Regression Signal |
|--------|--------|-------------------|
| Judge veto rate | 15-30% (too low means judge is not scrutinizing, too high means agents are sloppy) | Sustained > 40% or < 10% |
| Agent self-correction rate after veto | > 50% | Declining over time |
| PR merge rate without human modification | > 80% for routine tasks | Dropping below 70% |
| Deterministic verifier pass rate | > 90% on first attempt | Dropping below 80% |
| Time from task assignment to merged PR | Stable or decreasing | Increasing without corresponding complexity increase |

### 4.4 Proposed Composite Trust Score

A single metric combining the above into a trust score that can be tracked over time:

```
Trust Score = w1 * Validation_Accuracy + w2 * (1 - Miss_Rate) + w3 * (1 - Rework_Rate)
            + w4 * Scope_Conformance + w5 * Fitness_Pass_Rate

where w1=0.30, w2=0.30, w3=0.15, w4=0.15, w5=0.10
```

The Miss Rate gets the heaviest penalty because undetected issues are the most dangerous failure mode (Spotify's third failure mode: "PR passes CI but is functionally incorrect").

**Threshold for level advancement**: Trust Score > 0.85 sustained over 5+ consecutive features.
**Threshold for regression**: Trust Score < 0.70 for any single feature, or < 0.80 for rolling 3-feature average.

---

## 5. Novel Ideas Worth Considering

### 5.1 The "Operational Design Domain" for Agent Autonomy

Borrowed from self-driving, define an explicit ODD for each autonomy level. An agent is not "generally trustworthy" -- it is trustworthy within specific conditions:

```yaml
odd_level_3:
  task_types: [bug_fix, config_change, dependency_update, well_patterned_feature]
  codebase_areas: [tools/*, config/*, crates/ndp-lib/*]
  test_coverage_minimum: 80%
  pattern_match_required: true  # AgentDB must have a relevant pattern
  max_files_changed: 20

odd_level_4:
  task_types: [bug_fix, config_change, dependency_update]
  codebase_areas: [tools/*, config/*]
  test_coverage_minimum: 90%
  pattern_match_required: true
  max_files_changed: 10
```

Tasks outside the ODD automatically revert to a lower autonomy level. This is exactly how self-driving cars handle the highway-vs-city distinction.

### 5.2 "Deliberate Degradation" Drills

From aviation: periodically run validation at a LOWER autonomy level even when the system has proven reliable at a higher level. This serves two purposes:
1. Keeps the human's validation skills sharp
2. Creates a ground-truth dataset for measuring whether automated validation is drifting

Proposed cadence: Every 5th feature, human performs full manual validation alongside automated validation. Compare results. Any discrepancy updates the trust score.

### 5.3 The Validation Ladder

Instead of a binary "human validates / agent validates," create a graduated validation ladder that can be climbed per-feature based on risk:

```
Tier 1 (Automated):     Compile, test, lint, format
Tier 2 (Fitness):       Architecture fitness functions
Tier 3 (Judge):         LLM diff-vs-scope analysis
Tier 4 (Risk):          Automated risk classification
Tier 5 (Spot Check):    Random human sampling (10-20%)
Tier 6 (Full Review):   Human reviews all changes
```

Routine tasks: Tiers 1-4 only, with Tier 5 sampling.
Notable tasks: Tiers 1-5.
Significant/risky tasks: Tiers 1-6.

The classification of "routine vs notable vs significant vs risky" can itself be automated based on the ODD definition.

### 5.4 Agent Self-Assessment and Confidence Reporting

Research from Frontiers in Robotics and AI shows that "self-assessment in machines boosts human trust" by ~40%. If agents report their own confidence level, and that confidence is well-calibrated (high confidence = actually correct), trust builds faster.

**Implementation for NDP swarms**:
- After each task, agent reports: confidence level (0-1), areas of uncertainty, what it could not verify, what it would want a human to check
- Track calibration: when agent says 0.9 confidence, is it right 90% of the time?
- Over-confident agents (say 0.9, correct 60%) get autonomy reduced
- Well-calibrated agents (say 0.7, correct 70%) earn autonomy increases

This is the agent equivalent of a developer knowing what they do not know.

### 5.5 The "Glass Box" Validation Report

Inspired by NASA's transparency requirements and the aviation "point and say" protocol. Instead of a validation result being PASS/FAIL, require agents to produce a structured report:

```markdown
## Validation Report: fe-004
### What I checked
- [PASS] 908 existing tests still pass
- [PASS] 12 new tests added and passing
- [PASS] No DuckDB/Polars imports (fitness function)
- [PASS] All StreamConfig initializers complete (fitness function)
- [WARN] Coverage for new code is 72% (threshold: 80%)

### What I could NOT check
- Integration with Pi deployment (no integration env available)
- Performance under load (no benchmark baseline)

### My confidence: 0.82
- High confidence in functional correctness (well-tested)
- Lower confidence in operational behavior (untested integration path)

### Recommended human review areas
- New HybridBronzeReader logic (novel pattern, no prior examples)
- Docker compose changes (operational impact)
```

This transforms validation from a black box ("it passed") to a glass box ("here is what I checked, what I could not check, and where I am uncertain"). The human can make informed decisions about where to focus attention.

### 5.6 Asymmetric Autonomy Across Workflow Phases

Not all phases need the same autonomy level. The current NDP workflow could adopt:

| Phase | Current Level | Target Level | Rationale |
|-------|--------------|--------------|-----------|
| Architecture/Scope | L1 (paired) | L1 (keep paired) | Highest leverage, lowest frequency. Human insight is irreplaceable here. |
| Planning | L3 (ceded) | L3-L4 | Planning is already well-automated. Agent contracts could enable L4. |
| Implementation | L3 (ceded) | L3-L4 | Well-characterized ODD with good test coverage enables higher autonomy. |
| Validation | L1-L2 (human) | L3 (structured) | THE KEY TRANSITION. Build the validation ladder, track trust score. |
| Release | L2 (human approves) | L3 (agent proposes, human approves) | Release manifests, changelogs, tags can be agent-generated. |

The insight: keep architecture at L1 forever. It is the highest-value human contribution and the lowest-frequency activity. Focus autonomy gains on the high-frequency, lower-risk activities (validation, release mechanics).

### 5.7 The "N Repetitions" Question

The adaptive trust calibration research suggests there is no universal number of repetitions. Instead, trust should be updated using a Bayesian-style approach:

- **Prior**: Initial skepticism (assume agent will fail until proven otherwise)
- **Evidence**: Each successful task updates the prior. Each failure updates it more strongly (negativity bias is appropriate for safety).
- **Posterior**: Current trust level, which determines autonomy level.

A practical approximation:
- 3 successes: Enough to try the next level with heavy monitoring
- 5 successes without regression: Enough to operate at the level with normal monitoring
- 10 successes without regression: Enough to consider the next level
- 1 failure: Resets the counter, increases monitoring, may trigger regression

This matches the aviation approach: trust is earned slowly and lost quickly.

### 5.8 Architecture Fitness Functions as the Trust Foundation

Fitness functions are the single most important infrastructure investment for progressive autonomy. They are:
1. **Machine-verifiable**: No human judgment needed
2. **Intent-preserving**: They encode WHY, not just WHAT
3. **Regression-detecting**: They catch drift automatically
4. **Trust-building**: Each passing fitness function is evidence that the system works

**Proposed NDP fitness functions** (in priority order):
1. No deprecated approaches (DuckDB, Polars streaming) in any code
2. All StreamConfig initializers include all required fields
3. Bronze layer writes only via WAL (no direct Parquet writes in hot path)
4. Silver ETL uses HybridBronzeReader
5. No TODO/unimplemented!/todo!() markers
6. Docker image size within threshold
7. All crate dependencies resolve without conflicts
8. No circular dependencies between crates
9. Config schema validation passes for all stream configs
10. Test coverage above threshold for each crate

These 10 functions, automated in CI, would eliminate a large fraction of the validation work currently done by humans.

### 5.9 The "Inner Loop / Outer Loop" Agent Deployment

From OpenHands research, deploy agents at different trust levels based on where they operate:

**Inner loop agents** (real-time, on developer machine):
- Full access, human monitoring
- Used for architecture discussion, complex implementation
- Trust is managed by real-time attention

**Outer loop agents** (async, in CI/cloud):
- Sandboxed access, no human monitoring during execution
- Used for validation, testing, routine maintenance
- Trust is managed by constraints and verification

The insight: outer loop agents need MORE trust infrastructure (sandboxing, verification loops, audit trails) precisely because they have LESS human attention. This is the Spotify model.

**NDP application**: Move validation to an outer-loop agent that:
1. Runs in a sandboxed environment (Docker container)
2. Executes the validation ladder (Tiers 1-4)
3. Produces the glass box report
4. Human reviews the report (not the code)
5. System logs everything for audit

Source: [OpenHands - Agents in the Outer Loop](https://openhands.dev/blog/20251202-agents-in-the-outer-loop)

### 5.10 Property-Based Testing as Autonomy Enabler

Standard unit tests verify specific examples. Property-based tests verify invariants across random inputs. For progressive autonomy, property-based tests are more valuable because:
- They catch edge cases that example-based tests miss
- They define WHAT should be true, not HOW to achieve it
- They are harder for agents to "game" (writing tests that pass trivially)
- They provide higher confidence per test

**NDP application for Rust**: The `proptest` crate can define properties like:
- "For any valid StreamConfig, serialization roundtrips perfectly"
- "For any sequence of WAL writes, replay produces the same state"
- "For any Bronze data, HybridBronzeReader deduplicates correctly"

A codebase with strong property-based tests can support higher agent autonomy because the safety net is wider.

---

## 6. Recommended Next Steps for NDP

Based on this research, the path from current state (L2-L3) to target state (L3-L4) requires:

### Phase 1: Build the Trust Infrastructure (Prerequisites)

1. **Implement 5-10 architecture fitness functions** as automated tests
2. **Define the ODD** for agent autonomy (which tasks, which codebase areas, what coverage thresholds)
3. **Create the glass box validation report template** and require agents to produce it
4. **Establish baseline metrics** (current rework rate, current miss rate, current scope conformance)

### Phase 2: Operate at Level 3 with Training Wheels

5. **Run automated validation IN PARALLEL with human validation** for 5-10 features
6. **Compare results**: Did automated validation catch everything the human caught? Did it flag false positives?
7. **Track the trust score** and the individual component metrics
8. **Perform deliberate degradation drills** every 5th feature

### Phase 3: Transition to Level 3

9. **When trust score > 0.85 for 5 consecutive features**, switch to human-reviews-report mode
10. **Maintain 20% spot-check rate** for the first 10 features at this level
11. **Reduce spot-check to 10%** after 10 features without regression
12. **Document the transition** as an Architecture Decision Record

### Phase 4: Selective Level 4

13. **For routine task types within the ODD**, enable auto-approval
14. **Maintain spot-checking** at 10-20%
15. **Monitor composite trust score** continuously
16. **Regress immediately** on any miss

---

## Sources

### Autonomy Frameworks
- [ASDLC Levels of Autonomy](https://asdlc.io/concepts/levels-of-autonomy/)
- [SAE J3016 Levels of Driving Automation](https://www.sae.org/news/blog/sae-levels-driving-automation-clarity-refinements)
- [Tessl - 5 Levels of AI Agent Autonomy](https://tessl.io/blog/the-5-levels-of-ai-agent-autonomy-learning-from-self-driving-cars/)
- [Sheridan-Verplanck Levels of Automation](https://www.researchgate.net/figure/Sheridan-and-Verplanks-original-levels-of-automation-2_tbl1_337253476)

### Trust and Calibration
- [Adaptive Trust Calibration for Human-AI Collaboration (PMC)](https://pmc.ncbi.nlm.nih.gov/articles/PMC7034851/)
- [Rapid Trust Calibration through Interpretable and Uncertainty-Aware AI (PMC)](https://pmc.ncbi.nlm.nih.gov/articles/PMC7660448/)
- [Self-Assessment in Machines Boosts Human Trust (Frontiers)](https://www.frontiersin.org/journals/robotics-and-ai/articles/10.3389/frobt.2025.1557075/full)
- [Measuring and Understanding Trust Calibrations (CHI 2023)](https://dl.acm.org/doi/full/10.1145/3544548.3581197)

### NASA and Aviation
- [NASA Concepts for Design of Human-Autonomy Systems](https://ntrs.nasa.gov/api/citations/20190002683/downloads/20190002683.pdf)
- [NASA Human-Automation Teaming: Lessons Learned](https://ntrs.nasa.gov/api/citations/20190001937/downloads/20190001937.pdf)
- [Flight Safety Foundation - Trust but Verify](https://flightsafety.org/asw-article/trust-but-verify/)
- [NASA CSAOB - Cyber-Physical-Human Teaming](https://csaob.larc.nasa.gov/nasahrpiws/humansystemsintegration_cyberteaming/)

### AI-Assisted Development
- [DORA 2025 State of AI-Assisted Software Development](https://dora.dev/research/2025/dora-report/)
- [IT Revolution - Three Developer Loops Framework](https://itrevolution.com/articles/the-three-developer-loops-a-new-framework-for-ai-assisted-coding/)
- [Spotify - Background Coding Agents Part 3: Feedback Loops](https://engineering.atspotify.com/2025/12/feedback-loops-background-coding-agents-part-3)
- [Spotify - Background Coding Agents Part 1](https://engineering.atspotify.com/2025/11/spotifys-background-coding-agent-part-1)
- [O'Reilly - Conductors to Orchestrators](https://www.oreilly.com/radar/conductors-to-orchestrators-the-future-of-agentic-coding/)
- [Human Who Codes - Coder to Orchestrator](https://humanwhocodes.com/blog/2026/01/coder-orchestrator-future-software-engineering/)
- [OpenHands - Agents in the Outer Loop](https://openhands.dev/blog/20251202-agents-in-the-outer-loop)

### Architecture and Contracts
- [Continuous Architecture - Fitness Functions](https://continuous-architecture.org/practices/fitness-functions/)
- [AWS Architecture Blog - Cloud Fitness Functions](https://aws.amazon.com/blogs/architecture/using-cloud-fitness-functions-to-drive-evolutionary-architecture/)
- [Relari - Agent Contracts](https://github.com/relari-ai/agent-contracts)
- [GitHub ADR](https://adr.github.io/)

### Human-AI Collaboration
- [Stack Overflow - Pair Programming Model for AI](https://stackoverflow.blog/2024/04/03/developers-with-ai-assistants-need-to-follow-the-pair-programming-model/)
- [Friedrich Kurz - Pair Programming in the Age of AI](https://www.friedrichkurz.me/posts/2025-03-25-pair-programming-in-the-age-of-ai/)
- [LinkedIn Learning - AI Pair Programming Driver/Navigator](https://www.linkedin.com/learning/structured-vibe-coding-with-ai-coding-agents/ai-pair-programming-human-as-navigator-ai-as-driver)

### Industry Reports
- [DORA Report via InfoQ](https://www.infoq.com/news/2025/09/dora-state-of-ai-in-dev-2025/)
- [Tao of Mac - AI-Assisted Development in 2026](https://taoofmac.com/space/notes/2026/02/01/2130)
- [Cogent - Self-Evolving Software 2026](https://www.cogentinfo.com/resources/ai-driven-self-evolving-software-the-rise-of-autonomous-codebases-by-2026)
- [BayTech - Future of AI-Driven Development 2026](https://www.baytechconsulting.com/blog/unlocking-ai-software-development-2026)
