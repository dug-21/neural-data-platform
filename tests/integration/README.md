# Integration Testbed Framework

Run composable integration tests against the full NDP data pipeline: MQTT -> Bronze WAL -> Silver ETL -> Gold CAs -> Domain config -> Intelligence.

## Quick Start

```bash
# Smoke test (< 2 min): inject 10 messages, validate data reaches Silver
./tests/integration/run-testbed.sh smoke

# With intelligence daemon enabled
./tests/integration/run-testbed.sh smoke --intelligence

# Skip clean slate (reuse existing environment)
./tests/integration/run-testbed.sh smoke --skip-clean
```

## Prerequisites

- Docker and Docker Compose
- The integration compose file: `docker-compose.integration.yml`
- No host-level installs needed (mosquitto_pub runs inside the container)

## Testbed Types

| Type | What it tests | Duration | Command |
|------|--------------|----------|---------|
| **smoke** | Basic pipeline: 1 stream, Silver rows, etcd sync | < 2 min | `./run-testbed.sh smoke` |
| **regression** | Full layer coverage: all streams, Gold, domains, intelligence | ~10 min | `./run-testbed.sh regression --intelligence` |
| **stress** | Sustained load: RSS monitoring, memory leak detection | 30 min | `./run-testbed.sh stress --timeout 1800 --count 18000 --rate 10` |
| **feature** | Feature-specific validation from a feature testbed dir | Variable | `./run-testbed.sh feature --path product/features/fe-005/testbed` |

## How It Works

Each testbed run follows four phases:

```
Phase 1: Prep        docker compose down -v + up -d (clean slate)
Phase 2: Config      DEPLOY_ENV=integration deploy.sh apply <manifest>
Phase 3: Inject      mosquitto_pub via docker exec (MQTT messages)
Phase 4: Validate    Testbed-specific assertions (PASS/FAIL per check)
```

## Options

```
./tests/integration/run-testbed.sh <type> [options]

Options:
  --intelligence    Enable intelligence service (docker compose --profile intelligence)
  --timeout N       Health check timeout in seconds (default: 120)
  --count N         Number of MQTT messages to inject (default: 10)
  --rate N          Messages per second (default: 1)
  --path DIR        Feature testbed directory (required for 'feature' type)
  --skip-clean      Skip clean slate — reuse existing environment
  --help            Show help
```

## Directory Structure

```
tests/integration/
├── run-testbed.sh              # Entry point
├── lib/
│   ├── prep.sh                 # Clean slate, health wait, config sync, manifest apply
│   ├── inject.sh               # MQTT injection with rate control and randomization
│   └── assert.sh               # Assertion helpers (PASS/FAIL + summary)
├── fixtures/
│   └── mqtt/
│       └── airgradient.jsonl   # 10 MQTT message templates (randomized per injection)
└── testbeds/
    ├── smoke/
    │   ├── manifest.json       # 1 stream, 1 silver table, 1 domain
    │   ├── validate.sh         # Health + etcd + Silver rows + Bronze WAL
    │   └── compose-override.yml
    ├── regression/
    │   ├── manifest.json       # All streams, all layers, intelligence
    │   ├── validate.sh         # Smoke + Gold objects + embeddings
    │   └── compose-override.yml
    └── stress/
        ├── manifest.json       # Minimal surface for sustained load
        ├── validate.sh         # RSS sampling loop, 256 MiB threshold
        └── compose-override.yml
```

## Assertion Library

Available assertions (defined in `lib/assert.sh`):

| Function | What it checks |
|----------|---------------|
| `assert_service_healthy <container>` | Docker health status = "healthy" |
| `assert_etcd_key <key>` | etcd key exists with non-empty value |
| `assert_silver_rows <table> <min>` | Silver table has >= N rows |
| `assert_bronze_wal_exists <stream>` | WAL directory exists for stream |
| `assert_embedding_exists <domain>` | Intelligence embeddings table has rows |
| `assert_container_rss_below <container> <mb>` | Container RSS < threshold |
| `assert_gold_object_exists <name>` | Gold table/materialized view exists |
| `assert_summary` | Prints totals, returns exit 0 (all pass) or 1 (any fail) |

## Adding a Feature Testbed

Features store their testbed alongside the feature:

```
product/features/{id}/testbed/
├── manifest.json       # Same format as production .deploy/releases/*.manifest.json
├── data/               # Feature-specific MQTT fixtures or SQL seed data
└── validate.sh         # Feature-specific assertions (source lib/assert.sh)
```

Run it:
```bash
./tests/integration/run-testbed.sh feature --path product/features/fe-005/testbed
```

Manifest format (same as production releases):
```json
{
  "$schema": "../../../../schemas/manifest.schema.json",
  "version": "1.0",
  "description": "Feature testbed for fe-005",
  "changes": [
    {"type": "stream", "id": "air-quality", "action": "update"},
    {"type": "domain", "domain_id": "indoor-air-quality", "action": "sync"}
  ]
}
```

## Container Names

The integration environment uses these container names (from `docker-compose.integration.yml`):

| Container | Service |
|-----------|---------|
| `integration-mosquitto` | MQTT broker |
| `integration-etcd` | Config store |
| `integration-timescaledb` | Silver/Gold database |
| `integration-air-quality` | Domain app (Bronze + Silver ETL) |
| `integration-intelligence` | Similarity search (requires `--intelligence` flag) |
| `integration-mcp-server` | MCP interface |
| `integration-grafana` | Dashboards |

## Troubleshooting

**"Service did not become healthy"**: Check container logs:
```bash
docker logs integration-air-quality --tail 50
docker logs integration-timescaledb --tail 50
```

**"query failed" in Silver assertions**: TimescaleDB init-scripts may not have run. Ensure clean slate:
```bash
docker compose -f docker-compose.integration.yml down -v
docker compose -f docker-compose.integration.yml up -d
```

**MQTT injection fails**: Verify mosquitto is running:
```bash
docker exec integration-mosquitto mosquitto_pub -t test -m "hello"
```

**Stale containers**: If images were rebuilt, always use `up -d` (not `restart`):
```bash
docker compose -f docker-compose.integration.yml up -d
```

## Feature Testbed Process

Every feature that touches integration boundaries (database queries, container services, cross-layer data flow) should include a feature testbed as part of its deliverable. The testbed validates the feature's specific integration surface before code reaches production.

### When a Feature Needs a Testbed

| Feature touches... | Testbed needed? | Why |
|---------------------|-----------------|-----|
| New or modified SQL queries | Yes | Column types, view schemas, and cast behavior differ between unit mocks and real PostgreSQL |
| New container or service | Yes | Startup, health checks, and inter-service communication only manifest in the full stack |
| Bronze/Silver/Gold data flow | Yes | Data type promotions, NULL handling, and continuous aggregate behavior require real TimescaleDB |
| Configuration (etcd, domain JSON) | Yes | Config loading, schema validation, and struct deserialization interact with real data |
| Library-only changes (no runtime) | No | Unit tests are sufficient when there's no deployed artifact |
| Documentation or SPARC artifacts | No | No runtime behavior to validate |

### Process

**1. Identify the integration surface**

Before writing the testbed, list every integration point the feature introduces or modifies. Ask:

- What SQL queries does this feature execute? Against which tables/views?
- What PostgreSQL column types will the query results have?
- Does this feature read from or write to a new table?
- Does a container need to start, stay healthy, and complete a full cycle?
- What data needs to exist before the feature's code path executes?

This list becomes the basis for `validate.sh`.

**2. Create the testbed directory**

```
product/features/{id}/testbed/
├── manifest.json           # What to deploy (same format as .deploy/releases/)
├── compose-override.yml    # Environment overrides (thresholds, intervals, etc.)
├── data/                   # Feature-specific fixtures (MQTT payloads, SQL seeds)
│   └── seed.sql            # Optional: pre-populate tables for features that need history
└── validate.sh             # Feature-specific assertions
```

**3. Write the manifest**

The manifest declares what the testbed deploys — same schema as production release manifests. Include only what the feature requires:

```json
{
  "$schema": "../../../../schemas/manifest.schema.json",
  "version": "1.0",
  "description": "Feature testbed for {id}",
  "changes": [
    {"type": "stream", "id": "air-quality", "action": "update"},
    {"type": "domain", "domain_id": "indoor-air-quality", "action": "sync"},
    {"type": "container", "target": "ndp-intelligence", "action": "build"},
    {"type": "container", "target": "ndp-intelligence", "action": "restart"}
  ]
}
```

**4. Write compose overrides**

Use `compose-override.yml` to adjust thresholds and timing for the test environment. Production defaults (e.g., 168 warmup observations, 20-minute poll intervals) are too slow for a testbed. Override them so the feature's code path executes within the injection window:

```yaml
services:
  intelligence:
    environment:
      INTELLIGENCE_WARMUP_THRESHOLD: "5"
      INTELLIGENCE_POLL_INTERVAL_SECS: "10"
```

**5. Write validate.sh**

Source the assertion library first, then check each integration point from step 1. Structure the assertions in dependency order — check prerequisites before checking the feature's output:

```bash
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../../../../tests/integration/lib/assert.sh"

# --- Prerequisites ---
assert_service_healthy integration-timescaledb
assert_service_healthy integration-air-quality
assert_silver_rows silver.indoor_air_quality 5

# --- Feature-specific checks ---
# (assertions specific to what this feature introduces)

# --- Summary ---
assert_summary
```

**6. Choose assertion strategies per integration point**

| What to validate | Strategy | Example |
|------------------|----------|---------|
| Container doesn't crash | Compare restart count before and after injection | `docker inspect --format='{{.RestartCount}}' <container>` |
| SQL query succeeds against real types | Assert rows exist in the target table | `assert_silver_rows`, `assert_embedding_exists`, or custom SQL |
| Data values are correct (not silently zero/NULL) | Query specific columns and check for non-null, non-zero values | `psql -c "SELECT count(*) FROM table WHERE column IS NOT NULL AND column != 0"` |
| Full processing cycle completes | Grep container logs for cycle completion | `docker logs <container> 2>&1 \| grep -c "Cycle complete"` |
| Cross-layer data flow | Assert output layer has rows after input layer is populated | Inject MQTT, then check Silver, then check Gold, then check embeddings |
| Config loading from etcd | Assert etcd key exists and service reads it | `assert_etcd_key` + service health check |

**7. Run it**

```bash
./tests/integration/run-testbed.sh feature --path product/features/{id}/testbed --intelligence
```

Use `--intelligence` when the feature involves the intelligence service. Use `--count` and `--rate` to control injection volume if the feature needs more data than the default 10 messages.

**8. Iterate locally before release**

Use `--skip-clean` to rerun validation without tearing down the environment. This is the fast inner loop for debugging failures:

```bash
# First run: full clean slate
./tests/integration/run-testbed.sh feature --path product/features/fe-004/testbed --intelligence

# Fix code, rebuild container
docker compose -f docker-compose.integration.yml build intelligence

# Rerun validation only
./tests/integration/run-testbed.sh feature --path product/features/fe-004/testbed --intelligence --skip-clean
```

### Lessons from fe-004

The fe-004 (Similarity Intelligence) feature shipped without a feature testbed. Five production bugs (v1.2.7 through v1.2.11) were discovered on the Pi and fixed iteratively. Every one of them would have been caught by a feature testbed that validated:

| Bug | What a testbed would have caught |
|-----|----------------------------------|
| pgvector serialization (v1.2.7) | `assert_embedding_exists` — no embeddings stored |
| block_on panic (v1.2.8) | Restart count > 0 after cycle |
| ID format mismatch (v1.2.9) | Prediction table empty after cycle with neighbors |
| Column name mismatch (v1.2.10) | Embedding values all zero for direct fields |
| numeric type panic (v1.2.11) | Restart count > 0 on first warmup query |

The common thread: all five bugs involved real PostgreSQL types, real view schemas, or real container lifecycle — none reproducible in unit tests. A feature testbed running `validate.sh` with the assertions above would have caught all five before the first deploy.

### Checklist

Before merging a feature that touches integration boundaries:

- [ ] `product/features/{id}/testbed/` directory exists
- [ ] `manifest.json` declares what to deploy
- [ ] `compose-override.yml` adjusts thresholds for test timing
- [ ] `validate.sh` checks every integration point identified in step 1
- [ ] `./run-testbed.sh feature --path ... ` passes on a clean slate
- [ ] Assertions cover both existence (rows exist) and correctness (values are valid)
