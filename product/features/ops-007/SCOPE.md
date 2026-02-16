# ops-007: Integration Testbed Framework

## Vision

The integration environment (`docker-compose.integration.yml`) exists with all services, and `deploy.sh` already supports `DEPLOY_ENV=integration` for config paths, etcd sync, and declarative deploy. What's missing is the ability to **exercise the full pipeline with data** — inject synthetic MQTT messages, validate data flows through every layer (Bronze -> Silver -> Gold -> Intelligence), and confirm nothing is broken.

BUG-004/BUG-005 required 6 patch releases over 3 weeks because each hypothesis needed overnight production soak tests. A vestigial snapshot timer survived two releases because nothing in the dev environment exercised the pipeline under load. If a smoke testbed had existed, both issues would have been caught locally in minutes.

When ops-007 is done:
- A **testbed framework** provides composable test scenarios (smoke, regression, stress, feature-specific)
- Each testbed is a unit: **manifest + config + data + validation**
- `deploy.sh apply` is the universal deploy verb — testbed manifests validate the same pipeline as production
- Features add their own testbed, which automatically folds into regression coverage over time
- The integration environment exercises the full layer stack: MQTT -> Bronze WAL -> Silver ETL -> Gold CAs -> Domain config -> Intelligence embeddings/predictions

**Implementation approach**: Shell scripts for testbed runner, MQTT injection, and validation. Integration-only config files and manifests. Fix plumbing gaps in deploy.sh (etcd sync, one hardcoded path). No new Rust code — all fixes are in deploy.sh and shell scripts.

## Tracking

- GitHub Issue: https://github.com/dug-21/neural-data-platform/issues/21
- Retrospective: BUG-004/BUG-005 root cause chain (GH Issue #16, closed)
- Prior art: `dp-017/architecture/ADR-017-002-test-harness-strategy.md` (deploy.sh command testing)

## Scope

### Workstream 1: Integration Environment Completion

Achieve layer parity — at least one representative config for every pipeline layer. Not 100% config parity with production; additive over time.

**WS1-01: Integration config — layer audit and completion**
- Audit: which layers have integration config today, which are missing
- Current state: 4 of 7 stream configs exist in `config/integration/base/streams/`
- Need at minimum: 1 MQTT stream (air-quality — exists), 1 HTTP-poll stream (need to add)
- Domain config (`config/integration/domains/indoor-air-quality/domain.json`) missing `intelligence` block — add it
- Gold ETL config exists in stream configs — verify it works in integration

**WS1-02: etcd sync plumbing — vet and fix**
- `deploy.sh sync-domains` only syncs to TimescaleDB data dictionary, does NOT push domain config to etcd
- Intelligence daemon reads domain config from etcd at `/domains/{id}/config`
- Only `handle_domain()` (called by `deploy.sh apply` with domain declaration) writes to etcd (line 2075)
- **Decision**: Fix `sync_domains_to_data_dictionary()` in deploy.sh to ALSO push domain config to etcd via `etcdctl put` (same pattern as `handle_domain()` line 2075). This is a real production gap — already caused issues. Cascades to prod.
- Vet every sync command for integration correctness: `sync`, `sync-dictionary`, `sync-domains`

**WS1-03: Gold DDL config path fix**
- `handle_gold_table()` hardcodes `--config-dir "$REPO_ROOT/config/base"` (line 1976)
- Should use `$(dirname $CONFIG_STREAMS_DIR)` to respect `DEPLOY_ENV=integration`
- One-line fix

**WS1-04: Intelligence in integration stack**
- Intelligence daemon is behind `profiles: [intelligence]` in docker-compose.integration.yml
- Testbed runner must explicitly start it (or certain testbeds include it)
- Verify: pgvector extension created (init-script 006), intelligence daemon connects, reads domain config from etcd, generates embeddings from Gold data

### Workstream 2: Testbed Framework

The organizing abstraction. A testbed is a composable unit that testbed types compose differently.

**WS2-01: Testbed structure and runner**

```
tests/integration/
├── run-testbed.sh              # Entry point: ./run-testbed.sh <type> [options]
├── lib/
│   ├── prep.sh                 # Environment prep (modular, scenario-dependent)
│   ├── inject.sh               # MQTT data injection (rate, duration, topic, template)
│   └── assert.sh               # Assertion helpers (silver row count, WAL, embeddings)
├── fixtures/
│   └── mqtt/                   # Reusable message templates per stream
│       └── airgradient.jsonl   # One message per line, randomizable values
├── testbeds/
│   ├── smoke/
│   │   ├── manifest.json       # Minimal: 1 stream + silver + domain
│   │   ├── compose-override.yml # Env overrides (e.g., INTELLIGENCE_WARMUP_THRESHOLD=5)
│   │   └── validate.sh         # Pipeline works? Data reached Silver?
│   ├── regression/
│   │   ├── manifest.json       # Full: all streams, gold, domains, intelligence (static)
│   │   ├── compose-override.yml
│   │   └── validate.sh         # Smoke + all accumulated feature validations
│   └── stress/
│       ├── manifest.json       # Minimal surface, high volume
│       ├── compose-override.yml
│       └── validate.sh         # RSS thresholds, growth rate
```

**Compose override pattern**: Each testbed may include a `compose-override.yml` with environment-specific overrides (warmup thresholds, rate limits, etc.). The runner invokes: `docker compose -f docker-compose.integration.yml -f testbeds/<type>/compose-override.yml up -d`. This avoids duplicating the full compose file while allowing per-testbed tuning.

Feature testbeds live with the feature:
```
product/features/{id}/
├── testbed/
│   ├── manifest.json           # Feature-specific deploy manifest
│   ├── data/                   # Feature-specific fixtures
│   └── validate.sh             # Feature-specific assertions
```

**WS2-02: Environment prep (modular)**
- Each testbed type has a prep phase that may differ by scenario
- Common prep: ensure services are healthy, clean database (test-only — NOT in production deploy.sh)
- **Decision**: Clean slate = volume prune + container restart for MVP. Guaranteed clean over fast. Future optimization can explore surgical `DROP` + re-run init-scripts.
- Database reset is integration-only code — lives in `tests/integration/lib/prep.sh`, never touched by production
- Prep may include: stop/start services, volume prune, run init-scripts, sync configs, apply manifest
- Modular: testbed type specifies which prep steps to run

**WS2-03: Testbed types**

| Type | Prep | Config | Data | Validation | Duration |
|------|------|--------|------|------------|----------|
| **Smoke** | Clean slate, minimal config | 1 MQTT stream, silver, domain | 10 messages | Data in Silver? | <2 min |
| **Regression** | Clean slate, full config | All layers | Smoke + all feature data | Smoke + all feature validations | ~10 min |
| **Stress** | Clean slate, minimal config | Same as smoke | High volume (10 msg/sec) | RSS within bounds, no leaks | 30 min |
| **Feature** | Incremental on regression | Feature additions | Feature-specific | Feature-specific | Variable |

### Workstream 3: Data Injection

The core new capability — publish synthetic MQTT messages into the integration mosquitto broker.

**WS3-01: Message templates**
- JSON message templates derived from stream config field definitions (`config/integration/base/streams/*/config.json`)
- Templates use realistic value ranges from the stream config `range` fields
- Randomized within ranges — not static values
- At minimum: `airgradient.jsonl` template (matches air-quality MQTT stream)
- Template includes required fields: `pm02`, `serialno`, plus nullable fields: `rco2`, `atmp`, `rhum`, `wifi`, etc.

**WS3-02: Injection script (`lib/inject.sh`)**
- Publishes messages via `mosquitto_pub` at configurable rate and duration
- Parameters: `--topic`, `--template`, `--count`, `--rate` (msg/sec), `--duration` (seconds)
- Reads MQTT topic from stream config or explicit parameter
- Default: `airgradient/readings/test-sensor-001` at 1 msg/sec

### Workstream 4: Manifest-Per-Testbed

Every testbed has a manifest. `deploy.sh apply` is the deploy verb for both production and integration.

**WS4-01: Smoke manifest**
- Minimal manifest exercising: 1 stream config sync, 1 silver table, 1 domain (with etcd sync)
- Stored at `tests/integration/testbeds/smoke/manifest.json`
- Must work with `DEPLOY_ENV=integration ./deploy.sh apply tests/integration/testbeds/smoke/manifest.json`

**WS4-02: Regression manifest**
- Full manifest covering all integration layers: streams, silver, gold, domains, intelligence startup
- **Decision**: Static, hand-maintained manifest for MVP. Dynamic composition from feature testbeds deferred to next iteration.

**WS4-03: Feature manifest convention**
- Features store their testbed manifest at `product/features/{id}/testbed/manifest.json`
- This is the same manifest format as production releases — tests the deploy pipeline
- Feature development validates through: `deploy.sh apply product/features/{id}/testbed/manifest.json`

### Workstream 5: Validation Helpers

Lightweight assertion library for testbed validation scripts.

**WS5-01: Assertion helpers (`lib/assert.sh`)**
- `assert_silver_rows <table> <min_count>` — query TimescaleDB, check row count
- `assert_bronze_wal_exists <stream>` — check WAL file exists in container volume
- `assert_service_healthy <service>` — check container health endpoint
- `assert_etcd_key <key>` — verify key exists in etcd
- Helpers output PASS/FAIL per check, return exit code

**WS5-02: Testbed report**
- Each testbed run produces a summary: per-check PASS/FAIL, duration, exit code
- Exit code: 0 (all pass), 1 (any fail)
- Human-readable stdout output — no files written unless explicitly requested

## NOT in Scope

- **New Rust code** — all fixes are in deploy.sh and shell scripts; no crate changes
- **100% config parity** with production — layer parity only, additive over time
- **CI/CD integration** (GitHub Actions) — local-first; CI is a future concern
- **Docker MCP server** — separate infrastructure concern
- **Chaos engineering** — kill containers, network partitions, etc.
- **Performance benchmarking framework** — stress testbed checks bounds, not optimizes
- **Dynamic regression manifest composition** — static for MVP, dynamic deferred to next iteration
- **Custom MQTT publishing tools** — use shell loops or existing utilities, no new binaries
- **Production deploy.sh changes** for test-only concerns — test prep lives in test code only (note: WS1-02 etcd fix and WS1-03 Gold path fix ARE production fixes that benefit integration)

## Acceptance Criteria

- [ ] **AC-01**: Integration config has layer parity — at least 1 representative for MQTT, Silver, Gold, Domain, Intelligence
- [ ] **AC-02**: `DEPLOY_ENV=integration ./deploy.sh apply .../smoke/manifest.json` deploys successfully
- [ ] **AC-03**: Domain config reaches etcd via both `sync-domains` AND manifest apply (intelligence daemon can read it)
- [ ] **AC-04**: Gold DDL generation uses correct config path in integration mode
- [ ] **AC-05**: Smoke testbed: inject 10 MQTT messages, data appears in Silver within 2 minutes
- [ ] **AC-06**: Smoke testbed: validate.sh returns exit 0 on a healthy, data-populated stack
- [ ] **AC-07**: Stress testbed: 10 msg/sec for 30 minutes, RSS stays within configured bounds
- [ ] **AC-08**: Message templates produce valid JSON matching stream config field definitions
- [ ] **AC-09**: `tests/integration/run-testbed.sh smoke` runs end-to-end from clean slate
- [ ] **AC-10**: Environment prep includes clean database step (test-only, not in production code)
- [ ] **AC-11**: Intelligence daemon starts, reads domain config from etcd, and is reachable in integration
- [ ] **AC-12**: Feature testbed convention documented — `product/features/{id}/testbed/` structure

## Planning Guidance

**Resolved decisions** (owner input, pre-planning):

1. **etcd sync gap** — Fix `sync_domains_to_data_dictionary()` in deploy.sh to also `etcdctl put` each domain config (same pattern as `handle_domain()` line 2075). This is a real production gap, not test-only. Cascades to prod.
2. **Database clean slate** — Volume prune + container restart for MVP. Guaranteed clean over fast. Future: explore surgical `DROP` + re-run init-scripts for speed.
3. **Intelligence warmup** — Use compose override files per testbed type. Each testbed can set `INTELLIGENCE_WARMUP_THRESHOLD` (e.g., `5` for smoke) via `compose-override.yml`. Runner: `docker compose -f base.yml -f override.yml up -d`.
4. **Regression composition** — Static, hand-maintained manifest for MVP. Dynamic composition deferred.
5. **mosquitto_pub** — Keep simple. Shell loop with `sleep` or existing utility. No custom tooling.
6. **Silver ETL trigger** — Follow production flow. air-quality-app handles continuous ETL (silver-etl batch app is deprecated). Inject MQTT messages, wait, validate. No manual trigger needed.

**Wave sequencing**:
- **Wave 1**: WS1 (environment completion + plumbing fixes) + WS3 (data injection) — foundation
- **Wave 2**: WS2 (testbed framework + runner) + WS4 (manifests) — make it structured
- **Wave 3**: WS5 (validation helpers) + smoke testbed end-to-end — make it work

## Dependencies

- **Integration environment** (`docker-compose.integration.yml`): exists, functional
- **deploy.sh** with `DEPLOY_ENV=integration`: exists, mostly correct (WS1 fixes needed)
- **Stream configs**: `config/integration/base/streams/` partially populated
- **Intelligence crate**: `crates/ndp-intelligence` + `apps/ndp-intelligence-app` exist (fe-003 complete)
- **mosquitto_pub**: available in `eclipse-mosquitto` image

## Success Metric

After ops-007: `./tests/integration/run-testbed.sh smoke` validates the full data pipeline from clean slate in under 2 minutes. Feature developers create a testbed with their feature, test deployment through the same `deploy.sh apply` pipeline as production, and automatically add to regression coverage. Issues like BUG-004/BUG-005 are caught locally before deploying to Pi.
