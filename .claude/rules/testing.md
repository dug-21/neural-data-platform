---
paths:
  - "tools/**/*.rs"
  - "crates/**/*.rs"
  - "core/**/*.rs"
  - "apps/**/*.rs"
  - "tests/**/*"
  - "product/features/**/refinement/**/*"
  - "product/features/**/completion/**/*"
---

# Testing and Integration Environment

## Integration Environment (USE IT)

A fully functioning integration stack exists. All SPARC Refinement and Completion phases MUST validate against it.

- **Config**: `docker-compose.integration.yml`
- **Switch**: `DEPLOY_ENV=integration` changes config paths and container names
- **Start**: `docker-compose -f docker-compose.integration.yml up -d`

### Services

| Service | Port | Purpose |
|---------|------|---------|
| etcd | 2379 | Configuration store |
| TimescaleDB | 5432 | Silver/Gold layer database |
| mosquitto | 1883 | MQTT broker |
| air-quality-app | 8080 | Domain application |
| MCP server | 9100 | Management Control Plane |
| Grafana | 3000 | Dashboards |

## When to use integration env

- All SPARC Refinement phases (TDD against live stack)
- All SPARC Completion phases (integration verification)
- Any schema change (verify DDL against TimescaleDB)
- Any ETL change (verify data flow end-to-end)
- Any config change that affects runtime behavior

## Testing Conventions

- London TDD style (mock-driven, outside-in)
- Tests live alongside source in standard Rust locations
- Integration tests use the integration environment
- `cargo test --workspace` runs all unit tests
- See AgentDB pattern ID 16 for London TDD details

## Test Baseline and Flaky Test Management

### Test Baseline

The current passing test count baseline is stored in `.ndp/test-baseline.txt`. This file contains a single integer (e.g., `908`). The `/validate` skill compares the current test count against this baseline and warns on regression.

**Update process**: Update the baseline manually after each confirmed successful release. Do not auto-update.

### Flaky Test Manifest

Known flaky tests are listed in `.ndp/flaky-tests.txt` (one test name per line, `#` comments). The `/validate` skill uses this to separate known flaky failures from real failures in test output.

**Current known flaky tests (6):**
- 5 `weather_polling_integration` tests (wiremock timing issues)
- 1 `acceptance_partition_structure` test (hourly vs daily partitioning mismatch)

**Update process**: Add newly identified flaky tests with a comment explaining the root cause. Remove tests once the underlying issue is fixed.
