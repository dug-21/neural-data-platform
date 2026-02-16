# ops-007: Architecture Decisions

## ADR-007-001: Testbed Runner Composition Pattern

### Context

The testbed framework needs to support multiple test types (smoke, regression, stress, feature) that share common infrastructure (prep, injection, validation) but differ in configuration, data volume, and validation criteria. We need a pattern that is composable without being over-engineered for shell scripts.

### Decision

Use a **dispatch-and-compose** pattern where `run-testbed.sh` is the single entry point that dispatches to type-specific configuration while composing shared library functions.

```
run-testbed.sh <type> [options]
    |
    +-- sources lib/prep.sh, lib/inject.sh, lib/assert.sh
    |
    +-- reads testbeds/<type>/manifest.json
    |
    +-- merges testbeds/<type>/compose-override.yml (if exists)
    |
    +-- calls: prep -> inject -> testbeds/<type>/validate.sh
```

Each testbed type defines three things:
1. `manifest.json` -- what to deploy (same format as production manifests)
2. `compose-override.yml` -- environment tuning (optional)
3. `validate.sh` -- type-specific assertions (sources lib/assert.sh)

The runner orchestrates the pipeline; testbed types define the parameters. Shared libraries provide reusable functions that all types call.

Feature testbeds follow the same pattern but live at `product/features/{id}/testbed/` instead of `tests/integration/testbeds/`.

### Consequences

- **Enables**: Adding new testbed types without modifying the runner. Feature teams create a testbed directory and it works.
- **Enables**: Consistent test execution -- all types go through the same prep -> inject -> validate pipeline.
- **Costs**: Shell scripts have limited error handling compared to a real test framework. Acceptable for MVP.
- **Rules out**: Complex DAG-based test orchestration. Each testbed is a linear pipeline.


## ADR-007-002: deploy.sh etcd Sync Fix Approach

### Context

`sync_domains_to_data_dictionary()` in deploy.sh pushes domain config to the TimescaleDB data dictionary but does NOT push to etcd. The intelligence daemon reads domain config from etcd at `/domains/{id}/config`. This means `deploy.sh sync-domains` leaves etcd stale -- intelligence cannot discover domain configuration until a full `deploy.sh apply` runs (which calls `handle_domain()` that does write to etcd).

This is a real production gap that caused issues during fe-004 deployment. The fix benefits both production and integration environments.

### Decision

Add etcd write logic to `sync_domains_to_data_dictionary()` in deploy.sh, using the same `etcdctl put` pattern already established in `handle_domain()` (line ~2075).

```bash
# In sync_domains_to_data_dictionary(), after the existing TimescaleDB sync:
for domain_dir in "$CONFIG_DOMAINS_DIR"/*/; do
    domain_id=$(basename "$domain_dir")
    domain_config="$domain_dir/domain.json"
    if [ -f "$domain_config" ]; then
        log_info "Syncing domain config to etcd: $domain_id"
        docker exec "$ETCD_CONTAINER" etcdctl put \
            "/domains/${domain_id}/config" \
            "$(cat "$domain_config")"
    fi
done
```

The fix uses `CONFIG_DOMAINS_DIR` which already respects `DEPLOY_ENV`, so integration and production both benefit automatically.

### Consequences

- **Enables**: `deploy.sh sync-domains` becomes a complete sync (TimescaleDB + etcd). Intelligence daemon can read config after sync.
- **Enables**: Integration testbed can use `sync-domains` instead of full `apply` for config propagation.
- **Costs**: Adds ~5 seconds to sync-domains execution (etcd writes are fast).
- **Risk**: If domain.json is malformed, etcdctl put will store invalid JSON. Mitigated by: domain config is already validated by ndp-validate before deploy.


## ADR-007-003: Gold DDL Config Path Resolution

### Context

Four places in deploy.sh hardcode `--config-dir "$REPO_ROOT/config/base"` when calling `ndp gold` (the ndp CLI's gold subcommand). This means Gold DDL generation always reads production configs, even when `DEPLOY_ENV=integration` is set. The variable `CONFIG_STREAMS_DIR` already points to the correct environment-specific path (e.g., `config/integration/base/streams`).

The hardcoded references are in two functions:
- `handle_gold_table()` line ~1976: `ndp gold sync/generate --stream`
- `handle_domain()` line ~2094: `ndp gold generate --domain` (aligned views)
- `handle_domain()` line ~2120: `ndp gold generate --domain --events` (events DDL)
- `handle_domain()` line ~2139: `ndp gold intelligence schema --domain` (intelligence DDL)

Note: deploy.sh correctly uses `ndp gold` (the CLI), not the legacy `ndp-gold-ddl` binary. Other deploy.sh callers (dictionary sync, validate) already use `$CONFIG_STREAMS_DIR` correctly -- only these 4 gold calls are hardcoded.

The `ndp` CLI already supports `DEPLOY_ENV` auto-resolution via its `--env` flag (reads from env var), but the explicit `--config-dir` overrides it. Rather than removing `--config-dir` (which would introduce a CWD dependency on relative paths), we derive the correct absolute path from the existing `CONFIG_STREAMS_DIR` variable.

### Decision

Replace all 4 hardcoded paths with a dynamic derivation from `CONFIG_STREAMS_DIR`:

```bash
# Before (hardcoded, 4 occurrences):
--config-dir "$REPO_ROOT/config/base"

# After (environment-aware, 4 occurrences):
--config-dir "$(dirname "$CONFIG_STREAMS_DIR")"
```

`CONFIG_STREAMS_DIR` is set early in deploy.sh based on `DEPLOY_ENV`:
- Production: `$REPO_ROOT/config/base/streams` -> dirname = `$REPO_ROOT/config/base`
- Integration: `$REPO_ROOT/config/integration/base/streams` -> dirname = `$REPO_ROOT/config/integration/base`

### Consequences

- **Enables**: Gold DDL generation in integration uses integration configs.
- **Enables**: Integration testbed can validate Gold layer end-to-end.
- **Costs**: None -- 4 identical substitutions, uses existing variable.
- **Risk**: Minimal -- `CONFIG_STREAMS_DIR` is already validated at deploy.sh startup. Downstream impact is contained: `--config-dir` flows only from deploy.sh to the `ndp` CLI binary. No other tool or crate reads this flag.


## ADR-007-004: MQTT Injection via mosquitto_pub

### Context

The testbed framework needs to inject synthetic MQTT messages into the integration broker to exercise the data pipeline. Options: (a) custom Rust tool, (b) python paho-mqtt, (c) shell + mosquitto_pub. The scope explicitly excludes new Rust code and custom tools.

### Decision

Use `mosquitto_pub` via `docker exec` against the integration mosquitto container. Messages are read from JSONL template files and injected in a shell loop with configurable rate control.

```bash
inject_messages() {
    local topic="$1" template="$2" count="$3" rate="$4"
    local delay=$(echo "scale=3; 1/$rate" | bc)
    local line_count=$(wc -l < "$template")

    for i in $(seq 1 "$count"); do
        local line_idx=$(( (i - 1) % line_count + 1 ))
        local msg=$(sed -n "${line_idx}p" "$template" | randomize_values)
        docker exec "$MOSQUITTO_CONTAINER" mosquitto_pub \
            -t "$topic" -m "$msg"
        sleep "$delay"
    done
}
```

Template files are JSONL (one JSON message per line) stored in `tests/integration/fixtures/mqtt/`. The `randomize_values` function substitutes placeholders with random values within configured ranges.

### Consequences

- **Enables**: No new dependencies -- mosquitto_pub is already in the mosquitto container image.
- **Enables**: Rate control via simple sleep-based pacing. Sufficient for smoke (1 msg/sec) and stress (10 msg/sec).
- **Costs**: Shell-based rate control is imprecise at high rates. At 10 msg/sec with docker exec overhead, actual rate may be lower. Acceptable for testbed purposes.
- **Rules out**: Sub-millisecond injection precision. Not needed for this use case.


## ADR-007-005: Clean Slate Database Reset Strategy

### Context

Each testbed run needs a known starting state. Options: (a) surgical DROP + re-init, (b) docker volume prune + full restart, (c) database-level TRUNCATE. The scope specifies volume prune for MVP.

### Decision

Clean slate uses `docker compose down -v` (removes volumes) followed by `docker compose up -d` (fresh start). This guarantees complete reset because:

1. TimescaleDB data is on a named volume -- `-v` removes it
2. etcd data is on a named volume -- `-v` removes it
3. Bronze WAL files are on a named volume -- `-v` removes it
4. Init-scripts re-run on fresh container start (TimescaleDB extensions, base schemas)

```bash
prep_clean_slate() {
    local compose_files="$1"  # "-f base.yml -f override.yml"

    log_info "Tearing down integration stack..."
    docker compose $compose_files down -v --remove-orphans

    log_info "Starting fresh integration stack..."
    docker compose $compose_files up -d

    prep_wait_healthy
}
```

### Consequences

- **Enables**: Guaranteed clean state -- no stale data, no partial schemas, no cached etcd keys.
- **Costs**: Slow (~30-60 seconds for full cycle). Container images must re-initialize.
- **Costs**: Cannot do incremental tests without full restart (addressed by `--no-clean` flag).
- **Future**: Surgical DROP + re-init can be added as optimization once the slow path is proven reliable.


## ADR-007-006: Manifest-Per-Testbed Convention

### Context

Production deployments use manifest files (`.deploy/releases/vX.Y.Z.manifest.json`) with `deploy.sh apply`. The testbed framework should use the same mechanism so that tests validate the same deployment pipeline as production.

### Decision

Each testbed includes a `manifest.json` that follows the production manifest format. The testbed runner invokes `DEPLOY_ENV=integration ./deploy.sh apply <testbed>/manifest.json`.

Smoke manifest (minimal):
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

Regression manifest (full):
```json
{
  "version": "integration-regression",
  "streams": ["air-quality", "outdoor-air-quality", "outdoor-weather", "home-assistant-state"],
  "silver": { "tables": ["air_quality_readings", "outdoor_air_quality_readings", "weather_observations", "ha_sensor_state"] },
  "domains": ["indoor-air-quality"],
  "gold": { "tables": ["indoor_air_quality_aligned"] },
  "intelligence": true
}
```

Feature manifests live at `product/features/{id}/testbed/manifest.json` and are additive to the regression manifest.

### Consequences

- **Enables**: Tests validate the same `deploy.sh apply` code path as production.
- **Enables**: Manifest format consistency -- feature developers already know the format.
- **Costs**: manifest.json must be manually maintained (no dynamic generation for MVP).
- **Risk**: Manifest format may drift from production format. Mitigated by: using the same deploy.sh parser for both.


## ADR-007-007: Assertion Library Design

### Context

Testbed validation scripts need to check multiple conditions (Silver row counts, WAL files, service health, etcd keys, embeddings). Each testbed type composes different assertions. We need a consistent assertion pattern that is easy to extend.

### Decision

Implement `lib/assert.sh` as a library of assertion functions that follow a uniform contract:

```bash
# Contract: assert_* functions
# - Print "PASS: <description>" or "FAIL: <description>" to stdout
# - Return 0 on pass, 1 on fail
# - Accept descriptive parameters
# - Use docker exec for container-internal checks

ASSERT_PASS=0
ASSERT_FAIL=0

assert_silver_rows() {
    local table="$1" min_count="$2"
    local count=$(docker exec "$TIMESCALE_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -tAc \
        "SELECT COUNT(*) FROM $table")
    if [ "$count" -ge "$min_count" ]; then
        echo "PASS: $table has $count rows (>= $min_count)"
        ((ASSERT_PASS++))
        return 0
    else
        echo "FAIL: $table has $count rows (expected >= $min_count)"
        ((ASSERT_FAIL++))
        return 1
    fi
}

assert_summary() {
    echo "---"
    echo "Results: $ASSERT_PASS passed, $ASSERT_FAIL failed"
    [ "$ASSERT_FAIL" -eq 0 ] && return 0 || return 1
}
```

Validation scripts source this library and call assertions in sequence. The summary function provides the final verdict.

### Consequences

- **Enables**: Any testbed type can compose assertions from the library.
- **Enables**: New assertions are easy to add -- follow the contract, add a function.
- **Costs**: No parallel assertion execution -- sequential shell calls. Acceptable for the assertion count.
- **Rules out**: Structured test output (TAP, JUnit XML). Human-readable stdout only for MVP.
