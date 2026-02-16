# ops-007: Integration Testbed Framework -- Specification

## Overview

ops-007 delivers a shell-based integration testbed framework that exercises the full NDP data pipeline (MQTT -> Bronze WAL -> Silver ETL -> Gold CAs -> Domain config -> Intelligence embeddings/predictions) in a local Docker environment. It fixes two production plumbing gaps (etcd sync and Gold DDL config path) and provides composable test scenarios (smoke, regression, stress, feature-specific).

**Implementation approach**: Shell scripts only. No new Rust code. All fixes are in `deploy/pi/deploy.sh` and new shell scripts in `tests/integration/`.

## Functional Requirements

### FR-01: Integration Environment Completion (WS1)

**FR-01.1: Layer Audit and Config Completion**
- Audit existing integration configs: `config/integration/base/streams/` has 4 of 7 streams (air-quality, home-assistant-state, outdoor-air-quality, outdoor-weather)
- Verify at least 1 MQTT stream config exists (air-quality -- confirmed present)
- Add HTTP-poll stream config if missing from integration
- Add `intelligence` block to `config/integration/domains/indoor-air-quality/domain.json`
- Verify Gold ETL configs in existing stream configs work in integration mode

**FR-01.2: etcd Sync Fix (Production Gap)**
- `sync_domains_to_data_dictionary()` in deploy.sh currently only syncs to TimescaleDB
- Must ALSO push domain config to etcd via `etcdctl put` at `/domains/{id}/config`
- Use same pattern as `handle_domain()` (deploy.sh line ~2075)
- This is a production fix that cascades to all environments

**FR-01.3: Gold DDL Config Path Fix**
- Four `ndp gold` calls in deploy.sh hardcode `--config-dir "$REPO_ROOT/config/base"`:
  - `handle_gold_table()` line ~1976 (stream DDL)
  - `handle_domain()` line ~2094 (aligned views), ~2120 (events), ~2139 (intelligence)
- Replace all 4 with `--config-dir "$(dirname "$CONFIG_STREAMS_DIR")"` to respect `DEPLOY_ENV=integration`
- Note: deploy.sh uses `ndp gold` (the CLI), not the legacy `ndp-gold-ddl` binary

**FR-01.4: Intelligence in Integration Stack**
- Intelligence daemon uses `profiles: [intelligence]` in docker-compose.integration.yml
- Testbed runner must explicitly start it when needed
- Verify: pgvector extension (init-script 006), daemon connectivity, etcd domain config read, embedding generation from Gold data

### FR-02: Testbed Framework (WS2)

**FR-02.1: Testbed Runner (`run-testbed.sh`)**
- Entry point: `./tests/integration/run-testbed.sh <type> [options]`
- Supported types: `smoke`, `regression`, `stress`, `feature`
- Options: `--no-clean` (skip prep), `--verbose`, `--timeout <seconds>`
- Dispatches to: prep -> inject -> validate pipeline
- Exit code: 0 = all pass, 1 = any fail

**FR-02.2: Environment Prep (`lib/prep.sh`)**
- Clean slate: `docker compose down -v` + `docker compose up -d`
- Wait for service health checks (TimescaleDB, etcd, mosquitto)
- Run init-scripts (via compose entrypoint)
- Sync configs via `DEPLOY_ENV=integration ./deploy.sh sync`
- Apply testbed manifest via `DEPLOY_ENV=integration ./deploy.sh apply <manifest>`
- Modular: testbed type specifies which prep steps

**FR-02.3: Testbed Types**
| Type | Prep | Config | Data | Validation | Duration |
|------|------|--------|------|------------|----------|
| Smoke | Clean slate, minimal | 1 MQTT stream + silver + domain | 10 messages | Data in Silver? | <2 min |
| Regression | Clean slate, full | All layers | Smoke + feature data | Smoke + feature validations | ~10 min |
| Stress | Clean slate, minimal | Same as smoke | 10 msg/sec sustained | RSS bounds, no leaks | 30 min |
| Feature | Incremental on regression | Feature additions | Feature-specific | Feature-specific | Variable |

### FR-03: Data Injection (WS3)

**FR-03.1: Message Templates**
- JSON message templates in `tests/integration/fixtures/mqtt/`
- At minimum: `airgradient.jsonl` (one message per line)
- Fields derived from stream config: `pm02`, `serialno`, plus nullable: `rco2`, `atmp`, `rhum`, `wifi`
- Values randomized within realistic ranges from stream config `range` fields

**FR-03.2: Injection Script (`lib/inject.sh`)**
- Publishes via `mosquitto_pub` to integration mosquitto broker
- Parameters: `--topic <topic>`, `--template <file>`, `--count <N>`, `--rate <msg/sec>`, `--duration <seconds>`
- Default: `airgradient/readings/test-sensor-001` at 1 msg/sec
- Reads MQTT connection details from integration compose config

### FR-04: Manifests (WS4)

**FR-04.1: Smoke Manifest**
- Minimal manifest: 1 stream sync, 1 silver table, 1 domain (with etcd sync)
- Location: `tests/integration/testbeds/smoke/manifest.json`
- Compatible with `DEPLOY_ENV=integration ./deploy.sh apply`

**FR-04.2: Regression Manifest**
- Full manifest: all integration streams, silver, gold, domains, intelligence startup
- Static, hand-maintained for MVP
- Location: `tests/integration/testbeds/regression/manifest.json`

**FR-04.3: Feature Manifest Convention**
- Location: `product/features/{id}/testbed/manifest.json`
- Same format as production release manifests
- Validates via `deploy.sh apply`

### FR-05: Validation Helpers (WS5)

**FR-05.1: Assertion Library (`lib/assert.sh`)**
- `assert_silver_rows <table> <min_count>` -- query TimescaleDB via docker exec
- `assert_bronze_wal_exists <stream>` -- check WAL file in container volume
- `assert_service_healthy <service>` -- check container health status
- `assert_etcd_key <key>` -- verify key exists via etcdctl
- `assert_embedding_exists <domain>` -- check pgvector table has rows
- Each outputs PASS/FAIL with description, returns exit code

**FR-05.2: Testbed Report**
- Summary: per-check PASS/FAIL, duration, overall exit code
- Human-readable stdout -- no files written
- Machine-parseable exit code (0 = all pass, 1 = any fail)

## Non-Functional Requirements

- **NFR-01**: Smoke testbed completes in under 2 minutes on a healthy integration stack
- **NFR-02**: All shell scripts are POSIX-compatible bash (#!/bin/bash)
- **NFR-03**: No hard-coded IP addresses or ports -- read from compose config or environment
- **NFR-04**: Stress testbed RSS monitoring uses container stats, not external tools
- **NFR-05**: Testbed framework is self-contained -- no dependencies outside docker, bash, jq, mosquitto_pub

## Interfaces

### Input
- SCOPE.md workstream definitions
- Stream config files (`config/integration/base/streams/*/config.json`)
- Domain config files (`config/integration/domains/*/domain.json`)
- Production manifests (`.deploy/releases/*.manifest.json`) as format reference

### Output
- Shell scripts in `tests/integration/`
- Testbed manifests in `tests/integration/testbeds/*/manifest.json`
- Message templates in `tests/integration/fixtures/mqtt/`
- Compose overrides in `tests/integration/testbeds/*/compose-override.yml`
- Deploy.sh fixes (2 targeted edits)

## Constraints

- No new Rust code -- shell scripts and deploy.sh edits only
- No CI/CD integration -- local-first
- No custom MQTT publishing tools -- shell loops with `mosquitto_pub`
- Database clean slate = volume prune + container restart (not surgical DROP)
- Static regression manifest for MVP (no dynamic composition)
- Must work with existing `docker-compose.integration.yml` -- extend via compose overrides only
