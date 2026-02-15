# Research: Building Validation Confidence for Automated Agent Code Review

**Date**: 2026-02-15
**Agent**: research specialist
**Scope**: How automated validation systems build confidence and earn trust from human operators

---

## Table of Contents

1. [Validation Maturity Model](#1-validation-maturity-model)
2. [Architecture Conformance Techniques](#2-architecture-conformance-techniques)
3. [Observable Validation Design](#3-observable-validation-design)
4. [Trust Accumulation Framework](#4-trust-accumulation-framework)
5. [Recommended Escalation Rules](#5-recommended-escalation-rules)
6. [Shadow Mode Implementation Sketch](#6-shadow-mode-implementation-sketch)
7. [NDP-Specific Application](#7-ndp-specific-application)
8. [Sources](#8-sources)

---

## 1. Validation Maturity Model

The central problem: a solo developer currently validates all agent-produced code manually. They trust `cargo build` and `cargo test` (908 tests), somewhat trust `cargo clippy`, and do not trust automation for architectural review, drift detection, or "does this match what I asked for?" judgment. The goal is a path from manual to automated that never outruns the operator's actual confidence.

### The Five Levels

```
Level 0: MANUAL
  Human reviews every line of every change.
  Automation: cargo build + cargo test + cargo clippy.
  Human: architecture review, drift detection, intent alignment.
  Trust basis: personal inspection.
  Status: This is where NDP is today.

Level 1: OBSERVABLE AUTOMATION
  Automated checks run AND show their reasoning.
  The human reads the validation report, not the code.
  Automation adds: architecture conformance tests, file-scope checks,
    dependency drift detection, acceptance criteria mapping.
  Human: reads validation report, spot-checks suspicious items.
  Trust basis: transparency. "I can see what it checked and why it passed."

Level 2: SHADOW MODE
  Automated validation runs alongside human review.
  Both produce independent judgments. Results are compared.
  Human still reviews everything. Automation records what it would
    have flagged. Disagreements are logged and analyzed.
  Trust basis: agreement rate. "It catches everything I catch,
    plus some things I miss."

Level 3: RISK-GATED AUTOMATION
  Low-risk changes are auto-approved by the validator.
  High-risk changes require human review.
  The boundary is defined by change classification rules.
  Human: reviews high-risk changes only. Spot-checks random subset
    of auto-approved changes (canary validation).
  Trust basis: track record. "N auto-approved changes with zero
    post-approval issues."

Level 4: FULL AUTOMATION WITH OVERSIGHT
  All changes go through automated validation.
  Human reviews periodic summaries and exception reports.
  Anomaly detection flags unusual patterns for human attention.
  Human: reviews exceptions, adjusts rules, audits samples.
  Trust basis: statistical confidence. "99.7% agreement rate over
    200 validations, with zero false negatives on high-risk changes."
```

### Level Transition Criteria

Each level transition requires evidence, not faith. The operator should not move to the next level until they can answer "yes" to all criteria.

| Transition | Required Evidence |
|------------|-------------------|
| 0 -> 1 | Automated conformance tests exist for every architectural rule the human currently checks mentally. Validation reports show reasoning, not just pass/fail. |
| 1 -> 2 | Shadow mode infrastructure exists. Automated and manual reviews run independently on the same changes for at least 20 changes. |
| 2 -> 3 | Agreement rate >= 95% over 50+ shadow comparisons. Zero false negatives (automation missed something human caught) on high-risk changes. Disagreements are explainable. |
| 3 -> 4 | 100+ auto-approved changes with zero post-approval issues. Canary validation (random spot-checks) confirms ongoing accuracy. Anomaly detection is calibrated (no more than 1 false positive per 20 changes). |

### Key Insight: Asymmetric Error Costs

False positives (automation flags something that is actually fine) are annoying but safe. False negatives (automation misses something the human would catch) destroy trust. The entire framework is biased toward minimizing false negatives at the cost of more false positives. A validator that flags too much is annoying; a validator that misses real problems is dangerous.

---

## 2. Architecture Conformance Techniques

The hard question: "Is this implementation faithful to the architecture?" This is the check that humans do mentally and that is hardest to automate. The answer is to decompose "architectural faithfulness" into many small, testable assertions.

### 2.1 Fitness Functions (Neal Ford / Evolutionary Architecture)

An architectural fitness function is any mechanism that provides an objective integrity assessment of some architectural characteristic. The key word is "objective" -- if you can state the rule precisely enough to test, you can automate the check.

**Categories of fitness functions:**

- **Atomic**: Run against a single context. Example: "No crate in `tools/` depends on `core/`." This is a single dependency check.
- **Holistic**: Run against the full system. Example: "The full build completes in under 5 minutes on the CI server." This requires building everything.
- **Triggered**: Run on specific events. Example: "When Cargo.toml changes, verify no banned dependencies were added."

**Applicability to NDP:**

NDP's architectural rules are already partially documented in CLAUDE.md and ALIGNMENT-CRITERIA.md. Each rule can become a fitness function:

| Architectural Rule (from NDP docs) | Fitness Function | Implementation |
|-------------------------------------|-----------------|----------------|
| No DuckDB, no Polars, no jemalloc | Banned dependency check | `grep -rn 'duckdb\|polars\|jemalloc' **/Cargo.toml` |
| Hexagonal architecture: domain adapters use Source/Sink traits | Trait conformance test | Compile-time: every adapter crate must implement `Source` or `Sink` trait. Test: `cargo test` fails if trait not satisfied. |
| Bronze -> Silver -> Gold data flow | Layer dependency check | Cargo workspace: Silver crate may depend on Bronze types but not Gold. Gold may depend on Silver but not Bronze directly. |
| Config drives behavior, not data lifecycle | No hardcoded thresholds in non-test code | `grep -rn '[0-9]\{3,\}' --include='*.rs' \| grep -v test \| grep -v const` (heuristic) |
| ARM64 compatible | Cross-compilation check | `cargo check --target aarch64-unknown-linux-gnu` (if cross toolchain available) |
| Resource-constrained (256 MB per container) | Memory budget assertion | Integration test: check RSS of running container stays under limit |

### 2.2 Rust-Specific Architecture Enforcement

Unlike Java (where ArchUnit reads bytecode), Rust enforces architecture primarily through its type system and crate boundaries at compile time.

**Cargo workspace as architecture enforcement:**

Rust's crate system is the most powerful architecture enforcement mechanism available. Each architectural layer can be a separate crate. Dependencies in `Cargo.toml` are the layer dependency rules. The compiler rejects violations.

NDP already uses this pattern:
- `ndp-types` (shared types, no business logic)
- `core/` (domain logic)
- `crates/ndp-lib` (shared library functions)
- `tools/` (CLI tools, depend on lib)
- `apps/` (binaries, depend on everything)

**cargo-deny for dependency governance:**

`cargo-deny` lints the dependency graph against configurable rules: license policies, banned crates, duplicate versions, security advisories. It fills the role of ArchUnit's dependency checks but at the package level, not the class level.

**Custom architecture tests in Rust:**

```rust
// tests/architecture.rs -- Architecture conformance tests

#[test]
fn no_banned_dependencies() {
    let cargo_lock = std::fs::read_to_string("Cargo.lock").unwrap();
    let banned = ["duckdb", "polars", "jemalloc"];
    for dep in &banned {
        assert!(
            !cargo_lock.contains(dep),
            "Banned dependency found: {dep}"
        );
    }
}

#[test]
fn tools_do_not_depend_on_apps() {
    // Parse Cargo.toml files in tools/ and verify none reference apps/
    let tools_dir = std::path::Path::new("tools");
    for entry in std::fs::read_dir(tools_dir).unwrap() {
        let cargo_toml = entry.unwrap().path().join("Cargo.toml");
        if cargo_toml.exists() {
            let content = std::fs::read_to_string(&cargo_toml).unwrap();
            assert!(
                !content.contains("air-quality-app"),
                "Tool {:?} depends on app binary",
                cargo_toml
            );
        }
    }
}

#[test]
fn all_stream_configs_are_valid_json() {
    let streams_dir = std::path::Path::new("config/base/streams");
    for entry in std::fs::read_dir(streams_dir).unwrap().flatten() {
        let config = entry.path().join("config.json");
        if config.exists() {
            let content = std::fs::read_to_string(&config).unwrap();
            let _: serde_json::Value = serde_json::from_str(&content)
                .unwrap_or_else(|e| panic!("Invalid JSON in {:?}: {e}", config));
        }
    }
}

#[test]
fn no_stubs_in_non_test_code() {
    let output = std::process::Command::new("grep")
        .args(["-rn", r"todo!\|unimplemented!", "--include=*.rs"])
        .args(["core/", "apps/", "crates/", "tools/"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let non_test_stubs: Vec<&str> = stdout
        .lines()
        .filter(|line| !line.contains("#[test]") && !line.contains("_test") && !line.contains("tests/"))
        .collect();
    assert!(
        non_test_stubs.is_empty(),
        "Found stubs in non-test code:\n{}",
        non_test_stubs.join("\n")
    );
}
```

These tests run in `cargo test --workspace` alongside all other tests. They enforce architecture as code. When an agent violates a rule, the test fails with a clear message explaining what went wrong.

### 2.3 Specification Compliance Checking

The gap between "the spec says X" and "the code does X" is where most subjective review happens. Approaches to narrow this gap:

**Acceptance Criteria as Test Names:**

The existing research in `04-early-validation.md` proposes an ACCEPTANCE-MAP.md that links acceptance criterion IDs to test function names. This is the simplest and most effective specification compliance mechanism. If every AC has a named test, and all tests pass, specification compliance is mechanically verified.

**LLM-as-Judge for Spec Compliance:**

For the aspects that cannot be reduced to test assertions (code clarity, naming conventions, documentation quality), an LLM judge model can evaluate compliance. This is detailed in Section 2.4.

### 2.4 LLM-as-Judge for Code Review

Research shows LLMs can serve as automated code reviewers, but with important caveats.

**What works:**
- Binary classification (correct/incorrect, compliant/non-compliant) is more reliable than nuanced scoring
- Chain-of-thought reasoning before the final judgment significantly improves quality
- Separate evaluation criteria (completeness, accuracy, adherence to style) should be split into separate judge calls
- Models specifically fine-tuned to be judges perform worse than general-purpose capable models on code evaluation

**What does not work:**
- LLMs would be unreliable in a fully automated code review environment (research shows up to 24.8% of correct code may receive incorrect suggestions)
- A single judge call evaluating multiple aspects simultaneously produces lower quality results
- The same model that generated the code should not judge the code (self-enhancement bias)

**Practical implementation for NDP:**

The judge does not replace `cargo test`. It handles the subjective questions that tests cannot answer.

```
JUDGE PROMPT (structured evaluation template):

You are reviewing code changes for the Neural Data Platform.
The implementation brief specified: {brief_excerpt}
The code changes are: {diff}

Evaluate ONLY the following criterion (one at a time):

CRITERION: Does this change introduce any dependency on external cloud services?
CONTEXT: NDP runs on a Raspberry Pi at the edge. All processing must be local.
EXPECTED: No cloud service URLs, no external API calls, no cloud SDK imports.

Think through the code changes step by step, then respond with EXACTLY:
{"reasoning": "...", "label": "PASS" or "FAIL", "evidence": "..."}
```

Key design decisions:
- Use a different model than the one that generated the code (prevent self-enhancement bias)
- Binary labels only (PASS/FAIL), never 1-10 scores
- One criterion per judge call
- Always require reasoning before the label
- Set temperature to 0 for reproducibility

**Validating the judge:**

Before trusting the judge, run it in shadow mode (Section 6) on 50+ changes where the human also reviews. Measure:
- Precision: Of changes the judge flagged, how many were actual problems?
- Recall: Of problems the human found, how many did the judge also find?
- Target: Recall >= 95% (miss nothing), Precision >= 70% (some false positives acceptable)

---

## 3. Observable Validation Design

The single most important trust-building property of automated validation is transparency. A validator that outputs "PASS" is a black box. A validator that outputs "PASS: checked 7 architectural rules, verified 12 acceptance criteria against 14 tests, scanned 23 modified files against scope, found 0 violations -- here is the evidence for each check" is observable.

### 3.1 Validation Report Structure

A validation report should use progressive disclosure: the summary is one line, the details are available for anyone who wants to drill down.

```
VALIDATION REPORT: fe-004 Wave 2
=================================

SUMMARY: PASS (18 checks, 0 failures, 2 warnings)

TIER 1: COMPILATION
  cargo build --workspace ............ PASS (0 errors)
  cargo test --workspace ............. PASS (924 tests, 16 new, 0 failures)
  Test count delta ................... +16 (was 908)

TIER 2: LINT
  cargo clippy ....................... PASS (0 warnings with -D warnings)

TIER 3: ARCHITECTURE CONFORMANCE
  Banned dependencies ................ PASS (0 found in Cargo.lock)
  Layer dependency rules ............. PASS (no upward deps detected)
  Stub scan .......................... PASS (0 todo!/unimplemented! in non-test code)
  File scope check ................... PASS (all 8 modified files appear in brief)
  New dependency check ............... WARN: added `hnsw` to ndp-intelligence Cargo.toml
    -> hnsw IS listed in IMPLEMENTATION-BRIEF.md ADR-03. Acceptable.
  Hardcoded value scan ............... PASS (0 non-const numeric literals in non-test code)

TIER 4: SPECIFICATION COMPLIANCE
  Acceptance criteria coverage:
    AC-01 (embeddings stored) ........ PASS (test_embeddings_stored_for_all_hours)
    AC-02 (predictions after warmup) . PASS (test_predictions_after_warmup)
    AC-03 (HNSW <1ms p99) ........... WARN: test exists but uses assert_eq, not latency check
    AC-04 (pgvector <10ms p99) ....... NOT COVERED (no matching test function)
    AC-05 (full cycle <500ms) ........ PASS (test_full_cycle_latency)
    ...
  Coverage: 10/12 (83%), 1 warning, 1 gap

TIER 5: INTENT ALIGNMENT (LLM-judge, optional)
  Edge-only compliance ............... PASS (no cloud service references)
  Config-driven compliance ........... PASS (no hardcoded thresholds)
  Integration-first compliance ....... PASS (extends existing traits)
  Resource-constrained compliance .... PASS (no banned deps, mem_limit set)

EVIDENCE LOG (for drill-down):
  [expand] Full cargo test output (924 lines)
  [expand] Modified file list with diff stats
  [expand] Dependency diff (Cargo.toml changes)
  [expand] LLM judge reasoning traces
```

### 3.2 Confidence Scores

Each tier contributes to an overall confidence score. The score is NOT a probability -- it is a coverage metric indicating how much of the validation space has been checked.

```
Confidence calculation:
  Tier 1 (compilation):     20 points (binary: 0 or 20)
  Tier 2 (lint):            10 points (binary: 0 or 10)
  Tier 3 (architecture):    30 points (proportional to checks passed)
  Tier 4 (spec compliance): 30 points (proportional to ACs covered)
  Tier 5 (intent alignment): 10 points (proportional to principles checked)
  Total: 0-100

Thresholds:
  >= 90: AUTO-APPROVE eligible (if change is classified low-risk)
  70-89: REVIEW RECOMMENDED (human should look at warnings)
  < 70:  HUMAN REQUIRED (significant gaps in validation coverage)
```

This is explicitly a heuristic, not a statistical measure. The value is in making the validation coverage visible, not in pretending it represents true correctness probability.

### 3.3 What a Trust-Building Report Looks Like

Properties that build human trust in validation output:

1. **Shows what was checked**: Not just pass/fail but the list of checks that ran.
2. **Shows what was NOT checked**: "Tier 5 skipped: no LLM judge configured" is more trustworthy than silence.
3. **Shows evidence**: For each check, what was the actual input and observation?
4. **Explains warnings**: A warning with context ("hnsw dependency added, but it IS in the brief") is worth more than a bare warning.
5. **Quantifies coverage**: "10/12 acceptance criteria have tests" tells the human exactly where the gaps are.
6. **Compares to baseline**: "Test count: 924 (was 908, +16)" tells the human things are improving.
7. **Timestamps and reproduces**: Report includes the exact commands run, so the human can re-run any check.

---

## 4. Trust Accumulation Framework

### 4.1 Bayesian Trust Model

Research on trust in automated systems shows that trust follows a Beta distribution: it starts neutral, grows with positive experiences, and drops sharply with negative experiences. Negative experiences have disproportionate impact (roughly 5x the weight of positive experiences).

**Applied to validation automation:**

```
Model: Beta(alpha, beta) distribution
  alpha = count of correct validations + 1 (prior)
  beta  = count of incorrect validations + 1 (prior)

  P(correct) = alpha / (alpha + beta)

Initial state (no evidence):
  alpha = 1, beta = 1
  P(correct) = 0.50 (maximum uncertainty)

After 20 correct validations, 0 incorrect:
  alpha = 21, beta = 1
  P(correct) = 0.955

After 20 correct, 1 incorrect:
  alpha = 21, beta = 2
  P(correct) = 0.913

After 20 correct, 2 incorrect:
  alpha = 21, beta = 3
  P(correct) = 0.875

After 50 correct, 0 incorrect:
  alpha = 51, beta = 1
  P(correct) = 0.981

After 50 correct, 1 incorrect:
  alpha = 51, beta = 2
  P(correct) = 0.962
```

**Interpretation:**
- 20 clean validations with zero misses gets you to 95.5% confidence
- A single miss drops you from 95.5% back to 91.3% (requires ~10 more correct validations to recover)
- 50 clean validations with zero misses gets you to 98.1%
- This asymmetry is intentional and matches how humans actually build trust

**Per-check trust tracking:**

Each check type (banned deps, file scope, stub scan, etc.) should have its own trust score. A check that has never been wrong in 50 runs has earned different trust than a check that has been wrong twice in 20 runs.

```
trust_scores:
  banned_dependency_check:
    correct: 47
    incorrect: 0
    confidence: 0.980
    auto_approve: true  (earned after 30 correct, 0 incorrect)

  file_scope_check:
    correct: 23
    incorrect: 2
    confidence: 0.880
    auto_approve: false  (below 0.95 threshold)

  llm_intent_judge:
    correct: 8
    incorrect: 1
    confidence: 0.818
    auto_approve: false  (requires 50+ observations)
```

### 4.2 Trust Categories

Different types of checks earn trust at different rates because their failure modes differ.

| Check Type | Trust Category | Auto-Approve Threshold | Minimum Observations |
|------------|----------------|----------------------|---------------------|
| Compilation (cargo build) | Deterministic | Already trusted | N/A (deterministic) |
| Tests (cargo test) | Deterministic | Already trusted | N/A (deterministic) |
| Lint (cargo clippy) | Deterministic | Already trusted | N/A (deterministic) |
| Banned dependency scan | Deterministic | 0.95 | 20 |
| Stub scan | Deterministic | 0.95 | 20 |
| File scope check | Heuristic | 0.95 | 30 |
| New dependency check | Heuristic | 0.95 | 30 |
| Hardcoded value scan | Heuristic | 0.90 | 40 |
| AC coverage mapping | Heuristic | 0.90 | 40 |
| LLM intent judge | Probabilistic | 0.95 | 50 |

Deterministic checks (exact pattern matching, compiler output) earn trust faster because their failure modes are fully predictable. Heuristic checks (regex patterns that might have edge cases) require more observations. Probabilistic checks (LLM judges) require the most observations because their failure modes are unpredictable.

### 4.3 How Many Clean Validations Before Automation?

The answer depends on the risk tolerance and the check type. Using the Bayesian model with a 95% confidence threshold:

| Desired Confidence | With 0 Failures | With 1 Failure | With 2 Failures |
|--------------------|-----------------|----------------|-----------------|
| 90% | 9 observations | 18 observations | 27 observations |
| 95% | 19 observations | 38 observations | 57 observations |
| 99% | 99 observations | 198 observations | 297 observations |

For a solo developer producing ~2-3 changes per week, reaching 95% confidence on a check with zero failures takes approximately 7-10 weeks of shadow mode. This is a realistic timeline -- long enough to catch edge cases, short enough that the developer sees the finish line.

### 4.4 Trust Decay

Trust should decay slowly when a check has not been exercised recently. A check that was last triggered 6 months ago may no longer be valid (the codebase has evolved, the check's assumptions may not hold).

```
Decay model:
  effective_alpha = alpha * decay_factor^(days_since_last_check / 30)
  decay_factor = 0.95

After 30 days inactive:
  effective_alpha = alpha * 0.95 (5% decay)

After 90 days inactive:
  effective_alpha = alpha * 0.857 (14% decay)

After 180 days inactive:
  effective_alpha = alpha * 0.735 (27% decay)
```

This prevents "stale trust" -- a check that earned high confidence a year ago but has not been validated recently should not retain full trust.

---

## 5. Recommended Escalation Rules

### 5.1 Change Classification

Every change should be classified before validation rules are applied. The classification determines which checks run and whether human review is required.

```
CHANGE CLASSIFICATION MATRIX

Dimension 1: Scope
  NARROW   = 1-3 files, single crate
  MODERATE = 4-10 files, 2-3 crates
  BROAD    = 11+ files, 4+ crates, or touches shared types

Dimension 2: Depth
  SURFACE  = config changes, documentation, test additions
  LOGIC    = business logic changes, new functions, bug fixes
  STRUCTURAL = new crates, new traits, dependency changes, schema changes

Dimension 3: Domain
  TOOLING  = CLI tools, scripts, validation
  PLATFORM = Bronze/Silver/Gold pipeline, ETL, data flow
  CORE     = shared types, traits, fundamental abstractions

Risk Score = Scope * Depth * Domain

  NARROW + SURFACE + TOOLING   = LOW (1)     -> auto-approve eligible
  NARROW + LOGIC + TOOLING     = LOW (2)     -> auto-approve eligible
  MODERATE + LOGIC + PLATFORM  = MEDIUM (12) -> human-review recommended
  BROAD + STRUCTURAL + CORE    = HIGH (27)   -> human review required
```

### 5.2 Escalation Decision Tree

```
START
  |
  v
[Run Tier 1-3 checks]
  |
  +-- Any FAIL? -----> HUMAN REQUIRED (regardless of risk)
  |
  v
[Classify change risk]
  |
  +-- HIGH risk? -----> HUMAN REQUIRED
  |                     (structural changes, core domain, broad scope)
  |
  +-- MEDIUM risk? ---> Check trust scores for all Tier 3+ checks
  |                     |
  |                     +-- All checks above auto-approve threshold? -> HUMAN RECOMMENDED
  |                     |   (show report, human can skip if satisfied)
  |                     |
  |                     +-- Any check below threshold? -> HUMAN REQUIRED
  |
  +-- LOW risk? ------> Check trust scores for all checks
                        |
                        +-- All above threshold AND confidence >= 90? -> AUTO-APPROVE
                        |   (log for canary audit)
                        |
                        +-- Otherwise -> HUMAN RECOMMENDED
```

### 5.3 What Gets Auto-Approved

With sufficient trust accumulation, these change types can be auto-approved:

| Change Type | Example | Required Trust Score |
|-------------|---------|---------------------|
| Test-only additions | New tests, no source changes | 0.90 across all checks |
| Config value changes | Threshold updates in JSON configs | 0.90 + config validator passes |
| Documentation updates | Markdown files, comments only | 0.85 (lowest bar, lowest risk) |
| Single-file bug fixes | Fix in one .rs file, tests pass | 0.95 + all architecture checks pass |
| Dependency version bumps | Cargo.toml version updates only | 0.95 + cargo-deny passes |

### 5.4 What Always Requires Human Eyes

These should never be auto-approved regardless of trust scores:

- New crate creation (structural change to workspace)
- New trait or interface definition (architectural decision)
- Changes to shared types in `ndp-types` (cross-cutting impact)
- Changes to deployment scripts or Docker configuration
- Any change that touches 10+ files
- Any change that removes tests
- Any change that adds a new external dependency
- Schema changes (Silver DDL, Gold DDL)
- Changes to the validation system itself (marking your own homework)

### 5.5 Anomaly Detection

Beyond risk classification, anomaly detection catches unusual agent behavior that does not fit established patterns.

```
ANOMALY SIGNALS:

File count anomaly:
  Baseline: agent typically modifies 3-5 files for this task type
  Anomaly:  agent modified 15 files
  Action:   flag for human review regardless of risk score

Test count anomaly:
  Baseline: test count should increase or stay the same
  Anomaly:  test count decreased by 3
  Action:   BLOCK. Never auto-approve test count regressions.

New dependency anomaly:
  Baseline: brief lists 2 new dependencies
  Anomaly:  agent added 5 dependencies not in brief
  Action:   flag for human review

Diff size anomaly:
  Baseline: similar tasks produce 100-300 line diffs
  Anomaly:  this diff is 2,000 lines
  Action:   flag for human review (possible hallucination or scope creep)

Crate boundary violation:
  Baseline: task is scoped to crates/ndp-intelligence
  Anomaly:  agent also modified core/src/bronze.rs
  Action:   flag for human review (cross-boundary modification)
```

---

## 6. Shadow Mode Implementation Sketch

Shadow mode is the critical bridge between Level 1 (observable automation) and Level 3 (risk-gated automation). During shadow mode, the automated validator and the human both review every change. Their judgments are compared to calibrate the automation.

### 6.1 Architecture

```
                  +-------------------+
                  |  Agent produces   |
                  |  code change      |
                  +--------+----------+
                           |
              +------------+------------+
              |                         |
              v                         v
    +------------------+     +-------------------+
    | AUTOMATED        |     | HUMAN REVIEW      |
    | VALIDATOR        |     | (existing process)|
    | Runs all checks  |     | Manual inspection |
    | Produces report  |     | Produces judgment  |
    +--------+---------+     +--------+----------+
              |                         |
              v                         v
    +------------------+     +-------------------+
    | Automated        |     | Human             |
    | judgment:        |     | judgment:          |
    | PASS/FAIL per    |     | APPROVE/REJECT    |
    | check, with      |     | with notes        |
    | reasoning        |     |                   |
    +--------+---------+     +--------+----------+
              |                         |
              +------------+------------+
                           |
                           v
              +---------------------------+
              | COMPARISON ENGINE         |
              | Agreement? Disagreement?  |
              | Log results.              |
              | Update trust scores.      |
              +---------------------------+
```

### 6.2 Comparison Categories

| Automated | Human | Category | Action |
|-----------|-------|----------|--------|
| PASS | APPROVE | Agreement (true positive) | Increment trust for all passing checks |
| FAIL | REJECT | Agreement (true negative) | Increment trust for the failing check |
| PASS | REJECT | False negative (DANGEROUS) | Decrement trust sharply. Analyze what the human caught that automation missed. Add new check. |
| FAIL | APPROVE | False positive (ANNOYING) | Decrement trust mildly. Refine the check that false-alarmed. |

### 6.3 Shadow Mode Workflow for NDP

The workflow fits into the existing NDP implementation swarm protocol without modifying the protocol itself.

**Step 1**: After the implementation swarm completes and the scrum-master runs validation (Step 3e of implementation-protocol.md), the automated validator additionally runs the Tier 3-5 checks described in Section 3.1. It writes the report to the GH Issue as a comment.

**Step 2**: The human reviews the change as they normally would. They record their judgment: APPROVE or REJECT, with notes on what they looked at and any concerns.

**Step 3**: A comparison script (or the human themselves) checks whether the automated validator and the human agree. Disagreements are logged.

**Step 4**: Trust scores are updated per-check based on the comparison.

```bash
# shadow-compare.sh -- compare automated and human judgments
# Usage: ./shadow-compare.sh <feature-id> <human-judgment>
#   human-judgment: "approve" or "reject:reason"

FEATURE=$1
HUMAN_JUDGMENT=$2

# Read automated report from the GH Issue
AUTOMATED=$(gh issue view $ISSUE_NUMBER --json comments \
  | jq -r '.comments[-1].body' \
  | grep -c "FAIL")

if [ "$AUTOMATED" -eq 0 ] && [ "$HUMAN_JUDGMENT" = "approve" ]; then
  echo "AGREEMENT: Both approve"
  # Update trust scores: increment all checks
elif [ "$AUTOMATED" -gt 0 ] && [[ "$HUMAN_JUDGMENT" == reject* ]]; then
  echo "AGREEMENT: Both found issues"
  # Update trust scores: increment failing check
elif [ "$AUTOMATED" -eq 0 ] && [[ "$HUMAN_JUDGMENT" == reject* ]]; then
  echo "FALSE NEGATIVE: Automation missed what human caught"
  echo "REASON: ${HUMAN_JUDGMENT#reject:}"
  # CRITICAL: Log this. Decrement trust. Analyze gap.
elif [ "$AUTOMATED" -gt 0 ] && [ "$HUMAN_JUDGMENT" = "approve" ]; then
  echo "FALSE POSITIVE: Automation flagged, human disagrees"
  # Mild: refine the triggering check
fi
```

### 6.4 Duration of Shadow Mode

Shadow mode should run for at least:
- 20 changes at minimum (statistical floor for meaningful comparison)
- 50 changes ideally (reaches 95%+ confidence intervals)
- All change types represented (not just trivial changes)
- At least 3 high-risk changes observed (to validate escalation rules)

At 2-3 changes per week, shadow mode runs for 7-25 weeks. This is a significant investment, but it is the investment that makes Level 3 (risk-gated automation) trustworthy rather than aspirational.

### 6.5 Canary Validation Post-Shadow

After exiting shadow mode and enabling auto-approval for low-risk changes, the human should continue spot-checking a random subset of auto-approved changes. This is canary validation.

```
Canary rate:
  First month after shadow mode exit:  50% of auto-approved changes spot-checked
  Second month:                        25%
  Third month and beyond:              10%
  If any canary finds a false negative: return to 50% and investigate
```

The canary rate decreases over time as confidence accumulates, but never reaches zero. Permanent 10% spot-checking provides ongoing calibration and catches drift in the validator itself.

---

## 7. NDP-Specific Application

### 7.1 Current State Assessment

NDP is at Level 0 (Manual) for architectural validation and Level 1 (Observable) for compilation and testing. The gap is between "tests pass" and "the code is architecturally correct."

**What NDP already has:**

| Capability | Status | Trust Level |
|------------|--------|-------------|
| `cargo build --workspace` | Automated, trusted | Deterministic, fully trusted |
| `cargo test --workspace` (908 tests) | Automated, trusted | Deterministic, fully trusted |
| `cargo clippy` | Automated, trusted | Deterministic, fully trusted |
| Config validation (`ndp validate`) | Automated, trusted | Schema + semantic checks, trusted |
| Anti-stub scan | Automated, somewhat trusted | Grep-based, high confidence |
| File scope check (drift check) | Semi-automated, partially trusted | In implementation protocol Step 3d |
| Integration env (`deploy.sh`) | Semi-automated, somewhat trusted | Requires manual trigger |
| Architectural review | Manual | Entirely human |
| Intent alignment ("does this match what I asked for?") | Manual | Entirely human |

**What NDP needs to reach Level 1 (Observable Automation):**

1. Architecture conformance tests (fitness functions) as described in Section 2.2
2. Acceptance criteria mapping (ACCEPTANCE-MAP.md) as described in `04-early-validation.md`
3. Validation report format (Section 3.1) that shows reasoning, not just pass/fail
4. Dependency governance via `cargo-deny`

**What NDP needs to reach Level 2 (Shadow Mode):**

5. Shadow mode comparison infrastructure (Section 6.3)
6. Trust score tracking per check type (Section 4.1)
7. Human judgment recording workflow

**What NDP needs to reach Level 3 (Risk-Gated Automation):**

8. Change classification engine (Section 5.1)
9. Escalation decision tree implementation (Section 5.2)
10. Anomaly detection rules (Section 5.5)
11. LLM-as-judge for intent alignment (Section 2.4)

### 7.2 Recommended Implementation Order

**Phase A: Foundation (Level 0 -> Level 1)**

These are all deterministic checks that can be added to the existing `/validate` skill or as architecture tests in the Cargo workspace.

| Step | What | Effort | Value |
|------|------|--------|-------|
| A1 | Write architecture conformance tests (banned deps, layer rules, stub scan) as Rust tests in a `tests/architecture.rs` file | 2-3 hours | High: makes implicit rules explicit and testable |
| A2 | Add `cargo-deny` configuration for license, advisory, and banned crate checks | 1 hour | Medium: catches supply chain issues |
| A3 | Implement ACCEPTANCE-MAP.md generation in planning protocol (from `04-early-validation.md` P1-1) | 2 hours | High: makes spec compliance trackable |
| A4 | Create validation report template that shows check-by-check results with evidence | 2 hours | High: transforms opaque pass/fail into observable reasoning |
| A5 | Add agent self-check blocks to planning and implementation agent prompts (from `04-early-validation.md` P0-2) | 1 hour | High: catches drift within agent context |

Total: ~8-10 hours of work. After this, every validation run produces a detailed, evidence-backed report that the human can quickly scan.

**Phase B: Shadow Mode (Level 1 -> Level 2)**

| Step | What | Effort | Value |
|------|------|--------|-------|
| B1 | Create a simple trust-tracking JSON file that records per-check success/failure history | 1 hour | Foundation for all trust metrics |
| B2 | After each implementation swarm, post the full validation report to the GH Issue | 30 min | Enables shadow comparison |
| B3 | Create a lightweight human judgment recording command (`just shadow-judge approve/reject:reason`) | 1 hour | Captures the human side of the comparison |
| B4 | Create a comparison script that updates trust scores based on agreement/disagreement | 2 hours | The actual shadow mode engine |
| B5 | Run shadow mode for 20+ changes. Analyze disagreements. Refine checks. | 7-10 weeks | The trust accumulation period |

Total: ~4-5 hours of tooling, then 7-10 weeks of practice.

**Phase C: Risk-Gated (Level 2 -> Level 3)**

Only begin this after shadow mode demonstrates >= 95% agreement rate with zero false negatives on high-risk changes.

| Step | What | Effort | Value |
|------|------|--------|-------|
| C1 | Implement change classification (scope + depth + domain -> risk score) | 3 hours | Enables differential review |
| C2 | Implement escalation decision tree as a script or validation tier | 3 hours | Auto-routes changes to appropriate review level |
| C3 | Add anomaly detection (file count, test count, diff size baselines) | 2 hours | Catches unusual agent behavior |
| C4 | Optionally: add LLM-as-judge for intent alignment on medium-risk changes | 4 hours | Addresses the subjective judgment gap |
| C5 | Enable auto-approval for low-risk changes with canary spot-checking | 1 hour | The payoff: reduced manual review load |

Total: ~13 hours of work, but only after shadow mode evidence justifies it.

### 7.3 Applicable Industry Patterns

**From FDA validation (IQ/OQ/PQ):**

The FDA's approach to software validation provides a useful mental model even outside regulated industries:
- **Installation Qualification (IQ)**: "Is the system installed correctly?" For NDP: does `cargo build` succeed, are all dependencies resolved, does the Docker container start?
- **Operational Qualification (OQ)**: "Does the system function as specified?" For NDP: do all 908 tests pass, does config validation pass, do architecture conformance tests pass?
- **Performance Qualification (PQ)**: "Does the system perform acceptably under real conditions?" For NDP: does the integration environment (`deploy.sh`) work end-to-end, does memory stay under budget, does throughput meet baselines?

The modern FDA approach (Computer Software Assurance) emphasizes risk-proportionality: more scrutiny for higher-risk components. This maps directly to the escalation rules in Section 5.

**From mutation testing:**

Mutation testing introduces deliberate bugs to test whether the test suite catches them. Property-based tests are 52x more likely to catch mutations than unit tests (2025 OOPSLA research). For NDP, this means:
- The existing 908 tests may have gaps that only mutation testing would reveal
- Before trusting the test suite as a validation gate, run mutation testing (cargo-mutants) to measure test effectiveness
- If mutation survival rate is low (< 10%), the test suite can be trusted as a strong validation signal
- If mutation survival rate is high (> 30%), the test suite has significant gaps that weaken validation confidence

**From chaos engineering (applied to validation):**

Instead of only testing that the validator catches known bad patterns, deliberately introduce violations to verify the validator works:
- Add a banned dependency to a Cargo.toml. Does the validator catch it?
- Add a `todo!()` to non-test code. Does the stub scan find it?
- Modify a file not listed in the implementation brief. Does the file scope check flag it?
- Submit a change that violates an acceptance criterion. Does the AC mapping report it?

This is "chaos engineering for the validator" -- testing the safety net itself. Run these adversarial tests during shadow mode to build confidence that the validator catches what it claims to catch.

**From canary deployment:**

Start with a 1-5% traffic split (1 in 20 changes auto-approved, rest human-reviewed). Expand only when metrics confirm safety. If any canary fails, revert to full human review and investigate.

For a solo developer producing 2-3 changes per week, "1 in 20" means auto-approving roughly one change per month initially. This is deliberately slow. Trust is built through conservative progression, not aggressive automation.

---

## 8. Sources

### Web Sources

- [Addy Osmani - My LLM Coding Workflow Going Into 2026](https://addyosmani.com/blog/ai-coding-workflow/)
- [Addy Osmani - Code Review in the Age of AI](https://addyo.substack.com/p/code-review-in-the-age-of-ai)
- [Evaluating Large Language Models for Code Review (arXiv)](https://arxiv.org/html/2505.20206v1)
- [AI-powered Code Review with LLMs: Early Results (arXiv)](https://arxiv.org/html/2404.18496v2)
- [Fitness Functions for Your Architecture - InfoQ](https://www.infoq.com/articles/fitness-functions-architecture/)
- [Fitness Functions - Safeguard Architecture with Automated Checks](https://continuous-architecture.org/practices/fitness-functions/)
- [Fitness Function-Driven Development - Thoughtworks](https://www.thoughtworks.com/en-us/insights/articles/fitness-function-driven-development)
- [Building Evolutionary Architectures, 2nd Edition (O'Reilly)](https://www.oreilly.com/library/view/building-evolutionary-architectures/9781492097532/ch04.html)
- [LLM-as-a-Judge: Complete Guide - Evidently AI](https://www.evidentlyai.com/llm-guide/llm-as-a-judge)
- [LLM-as-a-Judge Explored - Medium](https://medium.com/online-inference/llm-as-a-judge-explored-2c6cd0d169fe)
- [Utilizing LLM-as-a-Judge to Evaluate LLM-Generated Code - Softtech](https://medium.com/softtechas/utilising-llm-as-a-judge-to-evaluate-llm-generated-code-451e9631c713)
- [LLM-As-Judge: Best Practices & Evaluation Templates - Monte Carlo](https://www.montecarlodata.com/blog-llm-as-judge/)
- [LLM as a Judge: 2026 Guide - Label Your Data](https://labelyourdata.com/articles/llm-as-a-judge)
- [Evidence Accumulation and Trust in AI (arXiv)](https://arxiv.org/html/2511.22617v1)
- [Trust in Automation: Simulation Model (MASS)](https://www.tandfonline.com/doi/full/10.1080/10447318.2024.2399439)
- [Toward Quantifying Trust Dynamics (PMC)](https://pmc.ncbi.nlm.nih.gov/articles/PMC10374998/)
- [Mutation Testing: Ultimate Guide 2025](https://mastersoftwaretesting.com/testing-fundamentals/types-of-testing/mutation-testing)
- [Property-Based Testing Empirical Evaluation (OOPSLA 2025)](https://cseweb.ucsd.edu/~mcoblenz/assets/pdf/OOPSLA_2025_PBT.pdf)
- [LLM-Guided Formal Verification with Mutation Testing](https://ieeexplore.ieee.org/document/10546729/)
- [FDA IQ/OQ/PQ Guide - The FDA Group](https://www.thefdagroup.com/blog/a-basic-guide-to-iq-oq-pq-in-fda-regulated-industries)
- [Computer System Validation in Pharma (PMC)](https://pmc.ncbi.nlm.nih.gov/articles/PMC11416705/)
- [Shadow Testing - Microsoft Engineering Playbook](https://microsoft.github.io/code-with-engineering-playbook/automated-testing/shadow-testing/)
- [Post-Launch Reviews: Shadow Mode in AI Rollouts - Cobbai](https://cobbai.com/blog/ai-rollout-post-launch-review)
- [Shadow Deployment Guide - DhiWise](https://www.dhiwise.com/post/risk-free-production-testing-shadow-deployment)
- [Google SRE: Canary Release Deployment Safety](https://sre.google/workbook/canarying-releases/)
- [Canary Deployments Best Practices - Octopus](https://octopus.com/devops/software-deployments/canary-deployment/)
- [Google Cloud Chaos Engineering Framework](https://www.infoq.com/news/2025/11/google-chaos-engineering/)
- [Chaos Engineering and Fault Injection - Microsoft Azure](https://azure.microsoft.com/en-us/blog/advancing-resilience-through-chaos-engineering-and-fault-injection/)
- [cargo-deny - Rust Package Registry](https://crates.io/crates/cargo-deny)
- [ArchUnit in Practice: Keep Your Architecture Clean](https://www.codecentric.de/en/knowledge-hub/blog/archunit-in-practice-keep-your-architecture-clean)
- [Risk-Based Code Scoring in DevOps 2025 - Qodo](https://www.qodo.ai/blog/best-automated-code-review-tools-2026/)

### NDP Codebase Sources

- `/workspaces/neural-data-platform/CLAUDE.md` -- Project rules and architectural constraints
- `/workspaces/neural-data-platform/.claude/rules/testing.md` -- Testing conventions and integration environment
- `/workspaces/neural-data-platform/.claude/rules/implementation-protocol.md` -- Implementation swarm protocol with validation tiers
- `/workspaces/neural-data-platform/.claude/rules/swarm-protocol.md` -- Swarm coordination and anti-drift config
- `/workspaces/neural-data-platform/scripts/validate.sh` -- Local validation script (quick/standard/full modes)
- `/workspaces/neural-data-platform/tools/ndp-cli/src/commands/validate.rs` -- CLI validation command (schema + semantic)
- `/workspaces/neural-data-platform/crates/ndp-types/src/validate.rs` -- Validation trait and error types
- `/workspaces/neural-data-platform/product/ndp-dev-auto/04-early-validation.md` -- Early validation research (acceptance criteria, progressive confidence)
- `/workspaces/neural-data-platform/product/ndp-dev-auto/05-truth-verify-evaluation.md` -- Claude-flow truth scoring evaluation
- `/workspaces/neural-data-platform/product/ndp-dev-auto/03-local-cicd-e2e.md` -- CI/CD and testing workflow analysis
- `/workspaces/neural-data-platform/product/ndp-dev-auto/01-protocol-agent-evaluation.md` -- Protocol and agent evaluation
- `/workspaces/neural-data-platform/product/features/ops-005/SCOPE.md` -- Edge case lifecycle scope (data consistency, recovery)
