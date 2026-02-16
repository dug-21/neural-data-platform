# Implementation Brief: ops-007 -- Integration Testbed Framework

## SPARC Artifacts

| Artifact | Path |
|----------|------|
| Scope | product/features/ops-007/SCOPE.md |
| Specification | product/features/ops-007/specification/SPECIFICATION.md |
| Task Decomposition | product/features/ops-007/specification/TASK-DECOMPOSITION.md |
| Architecture (ADRs) | product/features/ops-007/architecture/ARCHITECTURE.md |
| Pseudocode | product/features/ops-007/pseudocode/PSEUDOCODE.md |
| Alignment Report | product/features/ops-007/ALIGNMENT-REPORT.md |
| Acceptance Map | product/features/ops-007/ACCEPTANCE-MAP.md |

## Goal

Deliver a shell-based integration testbed framework that exercises the full NDP data pipeline (MQTT -> Bronze WAL -> Silver ETL -> Gold CAs -> Domain config -> Intelligence) in a local Docker environment. Fix two production plumbing gaps in deploy.sh (etcd sync and Gold DDL config path) and provide composable test scenarios (smoke, regression, stress, feature-specific) so that issues like BUG-004/BUG-005 are caught locally in minutes rather than requiring weeks of production soak testing.

## GitHub Issue

https://github.com/dug-21/neural-data-platform/issues/21

## Resolved Decisions

| Decision | Resolution | Source | Pattern ID |
|----------|-----------|--------|------------|
| Testbed composition pattern | Dispatch-and-compose: runner dispatches to type-specific config, composes shared libs | ADR-007-001 | 17 |
| etcd sync gap fix | Add etcdctl put to sync_domains_to_data_dictionary() in deploy.sh | ADR-007-002 | 18 |
| Gold DDL config path | Replace 4 hardcoded config/base refs in handle_gold_table() + handle_domain() with $(dirname CONFIG_STREAMS_DIR) | ADR-007-003 | 19 |
| MQTT injection method | mosquitto_pub via docker exec, JSONL templates with randomization tokens | ADR-007-004 | 20 |
| Database reset strategy | docker compose down -v + up -d (clean slate, guaranteed clean) | ADR-007-005 | 21 |
| Manifest convention | Each testbed has manifest.json in production format, deploy.sh apply | ADR-007-006 | 22 |
| Assertion library design | lib/assert.sh with uniform assert_* contract (PASS/FAIL, exit codes) | ADR-007-007 | 23 |

## Files to Create/Modify

### New Files (tests/integration/)

| File | Purpose |
|------|---------|
| `tests/integration/run-testbed.sh` | Entry point: dispatches to testbed type, orchestrates prep/inject/validate |
| `tests/integration/lib/prep.sh` | Environment preparation: clean slate, health wait, config sync, manifest apply |
| `tests/integration/lib/inject.sh` | MQTT injection: mosquitto_pub wrapper with rate control and template randomization |
| `tests/integration/lib/assert.sh` | Assertion helpers: silver rows, WAL exists, service health, etcd key, embeddings, RSS |
| `tests/integration/fixtures/mqtt/airgradient.jsonl` | MQTT message template for air-quality stream (randomizable values) |
| `tests/integration/testbeds/smoke/manifest.json` | Minimal deploy manifest: 1 stream, 1 silver table, 1 domain |
| `tests/integration/testbeds/smoke/validate.sh` | Smoke validation: service health, etcd key, silver rows |
| `tests/integration/testbeds/smoke/compose-override.yml` | Smoke env overrides (INTELLIGENCE_WARMUP_THRESHOLD=5) |
| `tests/integration/testbeds/regression/manifest.json` | Full deploy manifest: all streams, silver, gold, domains, intelligence |
| `tests/integration/testbeds/regression/validate.sh` | Regression validation: smoke + all layer checks |
| `tests/integration/testbeds/regression/compose-override.yml` | Regression env overrides |
| `tests/integration/testbeds/stress/manifest.json` | Stress deploy manifest (same as smoke) |
| `tests/integration/testbeds/stress/validate.sh` | Stress validation: RSS monitoring, growth rate |
| `tests/integration/testbeds/stress/compose-override.yml` | Stress env overrides (longer timeouts) |

### Modified Files

| File | Change | Lines |
|------|--------|-------|
| `deploy/pi/deploy.sh` | WS1-02: Add etcdctl put to sync_domains_to_data_dictionary() | ~10 lines added |
| `deploy/pi/deploy.sh` | WS1-03: Fix 4 hardcoded `--config-dir` refs in handle_gold_table() (line ~1976) and handle_domain() (lines ~2094, ~2120, ~2139) to use `$(dirname "$CONFIG_STREAMS_DIR")` | 4 lines changed |
| `config/integration/domains/indoor-air-quality/domain.json` | Add `intelligence` block if missing | ~5 lines added |

## Data Structures

No new Rust data structures. All data flows through existing structures:
- MQTT messages: JSON matching stream config field definitions
- Manifests: JSON matching production `.deploy/releases/*.manifest.json` format
- Compose overrides: Standard docker-compose YAML

### Manifest Format (testbed)

```json
{
  "version": "integration-smoke",
  "streams": ["air-quality"],
  "silver": { "tables": ["air_quality_readings"] },
  "domains": ["indoor-air-quality"],
  "gold": { "tables": [] },
  "intelligence": false
}
```

### MQTT Template Format (JSONL with randomization tokens)

```jsonl
{"wifi":-{{RAND_INT:30:80}},"serialno":"{{SERIAL:test-}}","rco2":{{RAND_INT:400:2000}},"pm02":{{RAND_INT:1:50}},"atmp":{{RAND_INT:18:30}},"rhum":{{RAND_INT:30:70}}}
```

## Function Signatures

No Rust functions. Shell function signatures:

```bash
# lib/prep.sh
prep_clean_slate(compose_cmd)        # docker compose down -v + up -d
prep_wait_healthy([timeout=60])      # poll service health endpoints
prep_sync_configs()                  # DEPLOY_ENV=integration deploy.sh sync
prep_apply_manifest(manifest_path)   # DEPLOY_ENV=integration deploy.sh apply

# lib/inject.sh
inject_messages(topic, template, count, rate)   # mosquitto_pub in loop
randomize_message(json_string)                  # replace tokens with random values

# lib/assert.sh
assert_silver_rows(table, min_count)            # psql COUNT(*) check
assert_bronze_wal_exists(stream, [container])   # WAL directory check
assert_service_healthy(service)                 # docker inspect health
assert_etcd_key(key)                            # etcdctl get check
assert_embedding_exists(domain)                 # pgvector table check
assert_container_rss_below(container, max_mb)   # docker stats RSS check
assert_summary()                                # print results, return exit code
```

## Test Expectations

### Integration Tests (the feature IS integration tests)

This feature creates the integration test framework itself. Validation:
- `./tests/integration/run-testbed.sh smoke` passes on a healthy integration stack
- `./tests/integration/run-testbed.sh stress --timeout 1800` passes (RSS within bounds)
- Each assertion function returns correct exit codes
- deploy.sh etcd sync fix verified by `etcdctl get` after `sync-domains`
- Gold DDL path fix verified by `grep` in deploy.sh

### Unit Tests

None -- shell scripts are tested by running them. No Rust unit tests.

## Wave Structure

### Wave 1: Foundation (WS1 + WS3)
**Tasks**: 1.1-1.6 (6 tasks)
**Focus**: Fix plumbing gaps, create injection capability
- Integration config audit and completion (AC-01)
- deploy.sh etcd sync fix (AC-03) -- PRODUCTION FIX
- deploy.sh Gold DDL path fix (AC-04) -- PRODUCTION FIX
- Intelligence integration verification (AC-11)
- MQTT message templates (AC-08)
- Injection script (AC-05, AC-08)

### Wave 2: Framework (WS2 + WS4)
**Tasks**: 2.1-2.6 (6 tasks)
**Focus**: Testbed runner, prep module, manifests, all testbed types
- Testbed runner entry point (AC-09)
- Environment prep module (AC-10)
- Smoke testbed (AC-02, AC-05, AC-06)
- Regression testbed manifest (AC-01)
- Stress testbed (AC-07)
- Feature testbed convention documentation (AC-12)

### Wave 3: Validation + End-to-End (WS5)
**Tasks**: 3.1-3.2 (2 tasks)
**Focus**: Assertion library, end-to-end integration
- Assertion library (AC-05, AC-06)
- Smoke end-to-end integration (AC-09, AC-05, AC-06)

## Constraints

- **No new Rust code** -- shell scripts and deploy.sh edits only
- **No CI/CD integration** -- local-first, CI is a future concern
- **No custom MQTT tools** -- mosquitto_pub via docker exec only
- **No dynamic regression composition** -- static manifest for MVP
- **ARM64 compatible** -- shell scripts are architecture-independent
- **Config-driven** -- no hardcoded IPs, ports, or paths (use variables/compose config)
- **Integration-only test code** -- clean slate logic lives in tests/integration/, never in deploy.sh

## Dependencies

- `docker-compose.integration.yml` -- exists, functional
- `deploy/pi/deploy.sh` with `DEPLOY_ENV=integration` -- exists, needs 2 fixes
- `config/integration/base/streams/` -- 4 streams exist
- `config/integration/domains/indoor-air-quality/domain.json` -- exists, may need intelligence block
- `eclipse-mosquitto` image -- includes mosquitto_pub

## NOT in Scope

- New Rust code (no crate changes)
- 100% config parity with production (layer parity only)
- CI/CD integration (GitHub Actions)
- Docker MCP server
- Chaos engineering (kill containers, network partitions)
- Performance benchmarking framework
- Dynamic regression manifest composition
- Custom MQTT publishing tools (no new binaries)
- Production deploy.sh changes for test-only concerns (test prep lives in test code)

## Alignment Status

All 7 vision principles: **PASS**. No variances requiring approval. Self-Learning marked N/A (infrastructure/ops feature). See ALIGNMENT-REPORT.md for full details.

## Feature Testbed Convention

Features store their integration testbed at:

```
product/features/{id}/testbed/
  manifest.json          # Same format as production release manifests
  data/                  # Feature-specific MQTT fixtures or SQL seed data
  validate.sh            # Feature-specific assertions (sources lib/assert.sh)
```

The regression testbed incorporates all feature testbeds. Feature developers validate through:
```bash
DEPLOY_ENV=integration ./deploy/pi/deploy.sh apply product/features/{id}/testbed/manifest.json
```
