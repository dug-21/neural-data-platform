# Implementation Launch Prompt: fe-007

## Proposed Prompt

> Implement fe-007: Granger Causality (Statistical Validation)
> GitHub Issue: #{N} (see SCOPE.md Tracking section)
> Brief: product/features/fe-007/IMPLEMENTATION-BRIEF.md
> Pattern IDs from planning: 34 (test-trait), 35 (stationarity), 36 (table-schema), 37 (cycle-integration), 38 (feature-flag), 39 (config-schema), 40 (bic-lag), 41 (fdr-correction)
> Constraints: Pure Rust + ndarray, no external stats libs, ARM64 Pi, <30s scan, <50MB memory, feature-flagged NDP_GRANGER_ENABLED
> Wave structure: Wave 1 (core stats library + DDL), Wave 2 (scanner integration + cycle), Wave 3 (config schema + deploy)

## Reminders for User

- Review ALIGNMENT-REPORT.md for the v1.2.x vs v1.3 WARN (accepted as v1.2.x)
- Verify acceptance criteria in SCOPE.md (10 ACs, all mapped in ACCEPTANCE-MAP.md)
- The incomplete beta function (stats.rs) is the most numerically delicate component -- validate against known F-distribution tables

## Gotchas Discovered During Planning

- **Domain schema additionalProperties: false**: Adding `granger` to the intelligence block requires updating `config/schemas/domain.schema.json`. This is a known pattern from v1.2.3 (Pattern ID 50 domain-schema-update-required).
- **Gold aligned view column naming**: Streams use `{alias}_{field}` prefix. The Granger candidate extractor must parse compound column names to identify source/target streams correctly.
- **IntelligenceConfig struct propagation**: Adding `granger: Option<GrangerConfig>` to IntelligenceConfig affects all code that constructs or destructures this struct. Check all call sites.
- **CycleSummary Display impl**: Extending CycleSummary with granger fields requires updating the Display implementation in service.rs.
- **Integration domain.json intelligence block**: The test integration config uses `embedding`/`search` format (not `embeddings`/`similarity`). Verify the exact field names before adding `granger`.
- **Deterministic PRNG for tests**: Use xorshift64 or similar simple PRNG seeded per test to avoid depending on the `rand` crate. All statistical tests must be reproducible.
- **F-distribution p-value precision**: The regularized incomplete beta function via continued fraction needs sufficient iterations (200+) and tight epsilon (1e-12) for accuracy at extreme F-statistics.
