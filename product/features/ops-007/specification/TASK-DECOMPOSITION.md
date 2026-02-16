# ops-007: Task Decomposition

## Wave 1: Foundation (WS1 + WS3)

### Task 1.1: Integration Config Layer Audit
- **Files**: `config/integration/base/streams/*/config.json`, `config/integration/domains/indoor-air-quality/domain.json`
- **Actions**:
  - Audit 4 existing stream configs for integration completeness
  - Add `intelligence` block to domain.json (if missing)
  - Verify Gold ETL config sections in stream configs
- **AC**: AC-01 (layer parity)
- **Estimate**: Small

### Task 1.2: etcd Sync Fix in deploy.sh
- **Files**: `deploy/pi/deploy.sh`
- **Actions**:
  - Find `sync_domains_to_data_dictionary()` function
  - After existing TimescaleDB sync logic, add `etcdctl put "/domains/${domain_id}/config" "$(cat $domain_config_file)"`
  - Follow same pattern as `handle_domain()` line ~2075
  - Test: `DEPLOY_ENV=integration ./deploy.sh sync-domains` should populate both TimescaleDB AND etcd
- **AC**: AC-03 (domain config reaches etcd)
- **Estimate**: Small (production fix, surgical edit)

### Task 1.3: Gold DDL Config Path Fix in deploy.sh
- **Files**: `deploy/pi/deploy.sh`
- **Actions**:
  - Replace 4 hardcoded `--config-dir "$REPO_ROOT/config/base"` with `--config-dir "$(dirname "$CONFIG_STREAMS_DIR")"`:
    - `handle_gold_table()` line ~1976 (stream DDL via `ndp gold sync/generate`)
    - `handle_domain()` line ~2094 (aligned views via `ndp gold generate --domain`)
    - `handle_domain()` line ~2120 (events via `ndp gold generate --domain --events`)
    - `handle_domain()` line ~2139 (intelligence via `ndp gold intelligence schema --domain`)
  - This makes all Gold DDL generation respect DEPLOY_ENV=integration
  - Note: deploy.sh uses `ndp gold` (the CLI), not the legacy `ndp-gold-ddl` binary
- **AC**: AC-04 (correct config path)
- **Estimate**: Trivial (4 identical substitutions)

### Task 1.4: Intelligence Integration Verification
- **Files**: `docker-compose.integration.yml`, domain config
- **Actions**:
  - Verify intelligence service profile configuration
  - Verify pgvector extension in init-scripts
  - Document runner invocation with intelligence profile
- **AC**: AC-11 (intelligence daemon in integration)
- **Estimate**: Small

### Task 1.5: MQTT Message Templates
- **Files**: `tests/integration/fixtures/mqtt/airgradient.jsonl` (new)
- **Actions**:
  - Read `config/integration/base/streams/air-quality/config.json` for field definitions
  - Create JSONL template with realistic randomized values
  - Include required fields (pm02, serialno) and nullable fields (rco2, atmp, rhum, wifi)
- **AC**: AC-08 (valid JSON matching stream config)
- **Estimate**: Small

### Task 1.6: Injection Script
- **Files**: `tests/integration/lib/inject.sh` (new)
- **Actions**:
  - Implement `mosquitto_pub` wrapper with parameters: --topic, --template, --count, --rate, --duration
  - Read messages from template JSONL, randomize values per message
  - Default: `airgradient/readings/test-sensor-001` at 1 msg/sec
  - Use `docker exec` to call mosquitto_pub inside the mosquitto container
- **AC**: AC-05 (MQTT injection), AC-08 (valid messages)
- **Estimate**: Medium

## Wave 2: Framework (WS2 + WS4)

### Task 2.1: Testbed Runner Entry Point
- **Files**: `tests/integration/run-testbed.sh` (new)
- **Actions**:
  - Parse arguments: `<type>` (smoke|regression|stress|feature) + options
  - Dispatch to: prep.sh -> inject.sh -> testbeds/<type>/validate.sh
  - Handle compose override merging: `-f base.yml -f override.yml`
  - Report timing and exit code
- **AC**: AC-09 (end-to-end from clean slate)
- **Estimate**: Medium

### Task 2.2: Environment Prep Module
- **Files**: `tests/integration/lib/prep.sh` (new)
- **Actions**:
  - `prep_clean_slate()`: docker compose down -v, docker compose up -d, wait for health
  - `prep_wait_healthy()`: poll service health endpoints with timeout
  - `prep_sync_configs()`: DEPLOY_ENV=integration ./deploy.sh sync
  - `prep_apply_manifest()`: DEPLOY_ENV=integration ./deploy.sh apply <manifest>
  - Modular functions, testbed type calls what it needs
- **AC**: AC-10 (clean database step)
- **Estimate**: Medium

### Task 2.3: Smoke Testbed
- **Files**: `tests/integration/testbeds/smoke/manifest.json` (new), `tests/integration/testbeds/smoke/validate.sh` (new), `tests/integration/testbeds/smoke/compose-override.yml` (new)
- **Actions**:
  - Manifest: 1 stream (air-quality), 1 silver table, 1 domain
  - Compose override: INTELLIGENCE_WARMUP_THRESHOLD=5
  - Validate: assert_silver_rows, assert_etcd_key, assert_service_healthy
- **AC**: AC-02 (deploy.sh apply works), AC-05 (data in Silver), AC-06 (validate.sh exit 0)
- **Estimate**: Medium

### Task 2.4: Regression Testbed Manifest
- **Files**: `tests/integration/testbeds/regression/manifest.json` (new), `tests/integration/testbeds/regression/validate.sh` (new), `tests/integration/testbeds/regression/compose-override.yml` (new)
- **Actions**:
  - Manifest: all streams, silver, gold, domains, intelligence
  - Validate: smoke checks + all layer checks
- **AC**: AC-01 (layer parity via regression)
- **Estimate**: Medium

### Task 2.5: Stress Testbed
- **Files**: `tests/integration/testbeds/stress/manifest.json` (new), `tests/integration/testbeds/stress/validate.sh` (new), `tests/integration/testbeds/stress/compose-override.yml` (new)
- **Actions**:
  - Manifest: same as smoke (minimal surface)
  - Validate: RSS monitoring via docker stats, growth rate check
  - Duration: 30 min sustained at 10 msg/sec
- **AC**: AC-07 (RSS within bounds)
- **Estimate**: Medium

### Task 2.6: Feature Testbed Convention Documentation
- **Files**: Documentation in IMPLEMENTATION-BRIEF.md
- **Actions**:
  - Document `product/features/{id}/testbed/` structure
  - Document manifest format reference
  - Document how feature testbeds fold into regression
- **AC**: AC-12 (convention documented)
- **Estimate**: Small

## Wave 3: Validation (WS5 + End-to-End)

### Task 3.1: Assertion Library
- **Files**: `tests/integration/lib/assert.sh` (new)
- **Actions**:
  - `assert_silver_rows <table> <min>`: docker exec psql query
  - `assert_bronze_wal_exists <stream>`: docker exec ls check
  - `assert_service_healthy <service>`: docker inspect health
  - `assert_etcd_key <key>`: docker exec etcdctl get
  - `assert_embedding_exists <domain>`: docker exec psql pgvector query
  - Track PASS/FAIL count, output summary
- **AC**: AC-05 (Silver validation), AC-06 (validate exit code)
- **Estimate**: Medium

### Task 3.2: Smoke End-to-End Integration
- **Files**: All smoke testbed files + runner
- **Actions**:
  - Wire runner -> prep -> inject -> validate for smoke type
  - Test: `./tests/integration/run-testbed.sh smoke` from clean slate
  - Verify <2 min completion, exit 0 on healthy stack
- **AC**: AC-09 (end-to-end), AC-05 (data in Silver), AC-06 (exit 0)
- **Estimate**: Medium (integration testing)

## Task-to-AC Coverage Matrix

| AC | Tasks | Wave |
|----|-------|------|
| AC-01 | 1.1, 2.4 | 1, 2 |
| AC-02 | 2.3 | 2 |
| AC-03 | 1.2 | 1 |
| AC-04 | 1.3 | 1 |
| AC-05 | 1.6, 3.1, 3.2 | 1, 3 |
| AC-06 | 2.3, 3.1, 3.2 | 2, 3 |
| AC-07 | 2.5 | 2 |
| AC-08 | 1.5, 1.6 | 1 |
| AC-09 | 2.1, 3.2 | 2, 3 |
| AC-10 | 2.2 | 2 |
| AC-11 | 1.4 | 1 |
| AC-12 | 2.6 | 2 |
