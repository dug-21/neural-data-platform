# fe-009: SONA Learning

## Vision

Close the learning loop. The intelligence layer currently finds similar past situations and predicts what happened next — but it doesn't learn from whether those predictions were right or wrong. SONA (Self-Organizing Neural Architecture) adds per-relationship micro-LoRA adaptation: when a prediction about CO2 breach is confirmed correct, the embedding transformation for CO2-related features gets a small positive update. Over time, the system gets better at the specific predictions that matter for this domain.

This is NOT a model training exercise. SONA adapts the embedding space — making relevant dimensions more prominent and irrelevant ones less so — based on prediction outcome feedback. The K-NN search algorithm is unchanged.

## Tracking

- Feature: fe-009
- GitHub Issue: TBD
- Parent roadmap: `product/features/gold-001/FEATURE-ROADMAPv1.2.md` (v13-001 through v13-004)
- Predecessors: fe-004 (predictions + outcome tracking)
- Version target: v1.3.0

## What fe-004 Delivers (Prerequisites)

- Predictions with confidence scores in `gold.predictions`
- Outcome tracking (actual_value, actual_breach, correct columns)
- Enough prediction history to measure adaptation effectiveness

## Deliverables

| ID | Task | Description |
|----|------|-------------|
| S-01 | SONA engine integration | Wire ruvector-sona into intelligence binary, behind feature flag |
| S-02 | Trajectory recording | Each prediction cycle becomes a SONA trajectory: state (embedding) -> action (prediction) -> reward (correct/incorrect) |
| S-03 | Micro-LoRA adaptation | Per-relationship embedding transformation (~50KB each). Adapts which dimensions matter for each prediction target |
| S-04 | ReasoningBank patterns | Cluster successful prediction patterns. "When these embedding dimensions are high, CO2 breach predictions tend to be correct" |
| S-05 | EWC++ integration | Elastic Weight Consolidation prevents new adaptations from degrading old ones (built into SONA) |
| S-06 | Benchmark harness | SONA vs ARIMA vs raw K-NN comparison framework. Run when sufficient prediction history exists |
| S-07 | Adaptation metrics | Track adaptation effectiveness: prediction accuracy before/after SONA, per-relationship improvement |

## Constraints

- SONA adaptation runs AFTER the prediction cycle, not during (non-blocking)
- Micro-LoRA adapters are tiny (~50KB each) — negligible memory impact
- EWC++ is built into ruvector-sona, not a separate implementation
- ruvector-sona must compile on aarch64 (Pi 5). If compilation fails, feature is gated off and K-NN continues without adaptation
- Benchmark is a capability, not a gate — build it, run it when data permits, report results
- Must NOT degrade prediction quality — if SONA-adapted predictions are worse than raw K-NN, disable per-relationship

## Acceptance Criteria

| Criterion | Target |
|-----------|--------|
| Trajectories recorded | Every prediction cycle produces a SONA trajectory |
| Micro-LoRA adapts | Embedding transformation weights change based on outcomes |
| EWC++ preserves old knowledge | Adapting new relationships doesn't degrade existing ones |
| ReasoningBank populated | Successful patterns clustered and queryable |
| Benchmark runnable | SONA vs ARIMA vs K-NN comparison produces results |
| Adaptation metrics tracked | Per-relationship accuracy delta visible |
| Pi resource budget | <50MB additional memory for SONA engine + adapters |
| Fallback works | If ruvector-sona unavailable, K-NN continues unmodified |

## Out of Scope

- Action framework (fe-010)
- MCP query interface (future)
- Cross-domain transfer learning (future)
- Sysops domain (future major release)

## Release

v1.3.0 — Adaptive intelligence. System learns from outcomes.
