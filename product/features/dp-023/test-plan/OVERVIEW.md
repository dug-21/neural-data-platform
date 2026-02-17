# dp-023: Test Plan Overview

## Test Strategy

Testing follows the three-wave implementation structure. Each wave has unit tests (within the modified crate) and integration tests (cross-component verification).

### Test Pyramid

| Level | Count (est.) | Where | What |
|-------|------|-------|------|
| Unit | 12-15 | `core/src/silver/transform.rs`, `crates/ndp-lib/src/gold/generators/text_view.rs` | coerce_to_type branches, SQL generation |
| Integration | 4-6 | `tests/integration/`, testbed | End-to-end pipeline, DDL generation, dictionary sync |
| Acceptance | 10 | Manual/scripted | All 10 ACs from SCOPE.md |

### Test Components

| Component | Test Plan | Focus |
|-----------|-----------|-------|
| platform-core | test-plan/platform-core.md | coerce_to_type jsonb, TimescaleOutput text/jsonb binding |
| ndp-lib | test-plan/ndp-lib.md | TextViewGenerator SQL output |
| deploy-sh | test-plan/deploy-sh.md | DDL generator, dictionary sync, deploy.sh Phase 6 |
| ndp-validate | test-plan/ndp-validate.md | Schema acceptance of text/jsonb types |
| config | test-plan/config.md | NWS forecast config validation |

### Integration Surfaces to Test

| Surface | Test Method | AC |
|---------|-------------|-----|
| coerce_to_type("jsonb") produces correct Value | Unit test | AC-03, AC-04 |
| build_upsert_query emits ::jsonb cast | Unit test | AC-03, AC-04 |
| Text value flows through write() correctly | Unit test | AC-05 |
| DDL generator produces TEXT/JSONB columns | Shell test | AC-03 |
| Gold text view SQL is valid | Unit test | AC-06, AC-07 |
| ndp validate accepts text/jsonb | CLI test | AC-01, AC-02 |
| Dictionary sync populates silver_columns | Integration test | AC-09 |
| Existing streams still pass validation | Regression test | AC-08 |
| Full pipeline: config -> Silver -> Gold | Integration test | AC-05, AC-06 |
| Grafana can query Gold text view | Manual/scripted | AC-10 |

### Regression Safeguards

1. Run ALL existing platform-core tests (`cargo test -p platform-core`)
2. Run ALL existing ndp-lib tests (`cargo test -p ndp-lib`)
3. Run `ndp validate` on ALL existing stream configs
4. Verify existing Silver tables are unmodified by DDL generator changes
5. Verify existing Gold CAs and aligned views are unaffected
