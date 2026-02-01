# ADR-017-002: Test Harness Strategy

**Status**: Accepted
**Date**: 2026-02-01
**Decision Makers**: Human + AI Architecture Review
**Feature**: dp-017 Integration Test Harness for Deployment Evolution

---

## Context

The `deploy/pi/deploy.sh` script provides 30+ commands for deployment operations:

```
Core:     deploy, start, stop, logs, status, build
Update:   update, refresh
Config:   sync, init-streams, list-streams, sync-dictionary
Silver:   silver-migrate, silver-etl, silver-daemon[-*]
Dims:     sync-dimensions, list-dimensions, dimension-status
```

We need a strategy to validate these commands work correctly in the integration environment before deploying to production. This is critical for dp-016's declarative deployment work.

**Key Questions**:
1. Which commands need testing?
2. Smoke test vs full validation?
3. Manual vs automated?
4. How to inject test data?

---

## Decision

**Implement a tiered test strategy: smoke tests for fast feedback, validation tests for correctness.**

### Tier 1: Smoke Tests (Fast, Always Run)

Purpose: Quick confidence that basic operations work.

| Test | Command | Success Criteria | Duration |
|------|---------|------------------|----------|
| Stack starts | `deploy.sh deploy` | All containers healthy | <2 min |
| Status works | `deploy.sh status` | No errors, URLs printed | <5s |
| Stack stops | `deploy.sh stop` | Containers removed | <30s |

**Run**: Before every PR merge, after every compose change.

### Tier 2: Configuration Tests (Medium, Critical Path)

Purpose: Validate configuration sync operations.

| Test | Command | Success Criteria | Duration |
|------|---------|------------------|----------|
| Config sync | `deploy.sh sync` | etcd contains expected keys | <30s |
| Stream init | `deploy.sh init-streams` | Streams visible in etcd | <30s |
| Dictionary sync | `deploy.sh sync-dictionary` | Tables populated in TimescaleDB | <60s |
| Dimension sync | `deploy.sh sync-dimensions` | Dimension tables loaded | <60s |

**Run**: After any config structure change, before dp-016 deployment work.

### Tier 3: Data Flow Tests (Slow, Integration)

Purpose: Validate end-to-end data flow.

| Test | Steps | Success Criteria | Duration |
|------|-------|------------------|----------|
| Bronze ingestion | Publish MQTT message | Parquet file created | <30s |
| Silver ETL | Wait for event subscriber | Row in TimescaleDB | <60s |
| MCP query | Call MCP endpoint | Data returned | <10s |

**Run**: Before major releases, after ETL changes.

### Test Data Injection

Use the existing `scripts/integration-test.sh inject` command:

```bash
# Inject single reading
mosquitto_pub -h localhost -p 1883 \
    -t "airgradient/integration-test/measures" \
    -m '{"wifi":-45,"pm02":12,"rco2":650,"atmp":22.5,"rhum":55}'

# Inject multiple readings
for i in {1..5}; do
    mosquitto_pub -h localhost -p 1883 \
        -t "airgradient/integration-test/measures" \
        -m "{\"wifi\":-45,\"pm02\":$((10+RANDOM%20)),\"rco2\":$((600+RANDOM%200))}"
    sleep 1
done
```

### Test Runner: scripts/integration-test.sh

The existing script provides the test harness framework:

```bash
./scripts/integration-test.sh start    # Start stack + sync configs
./scripts/integration-test.sh status   # Health checks
./scripts/integration-test.sh inject   # Test data
./scripts/integration-test.sh query    # Query Silver layer
./scripts/integration-test.sh stop     # Cleanup
./scripts/integration-test.sh clean    # Full cleanup with volumes
```

### deploy.sh Invocation Pattern

For testing deploy.sh commands, use environment prefix:

```bash
# Pattern: DEPLOY_ENV=integration ./deploy/pi/deploy.sh <command>

# Examples:
DEPLOY_ENV=integration ./deploy/pi/deploy.sh deploy
DEPLOY_ENV=integration ./deploy/pi/deploy.sh sync
DEPLOY_ENV=integration ./deploy/pi/deploy.sh status
DEPLOY_ENV=integration ./deploy/pi/deploy.sh silver-migrate
DEPLOY_ENV=integration ./deploy/pi/deploy.sh sync-dictionary
```

### Container Name Resolution

deploy.sh uses the `ETCD_CONTAINER` variable to target the correct container:

| DEPLOY_ENV | ETCD_CONTAINER | Used By |
|------------|----------------|---------|
| `pi` (default) | `etcd` | sync-config-to-etcd.sh, init-streams.sh |
| `integration` | `integration-etcd` | Same scripts, different target |

This is already implemented in deploy.sh:

```bash
if [ "$DEPLOY_ENV" = "integration" ]; then
    COMPOSE_FILE="$REPO_ROOT/docker-compose.integration.yml"
    ETCD_CONTAINER="integration-etcd"
else
    COMPOSE_FILE="$SCRIPT_DIR/docker-compose.yml"
    ETCD_CONTAINER="etcd"
fi
```

---

## Consequences

### Positive

1. **Tiered approach** - Fast feedback for common cases, thorough testing when needed
2. **Reuses existing tooling** - integration-test.sh, deploy.sh already exist
3. **Clear success criteria** - Each test has measurable outcomes
4. **CI-compatible** - All tests can run in GitHub Actions

### Negative

1. **Manual execution** - No automated test suite yet (future work)
2. **Data cleanup** - Tests may leave data in volumes
3. **External dependencies** - mosquitto_pub required for MQTT injection

### Neutral

1. **Time investment** - Full validation takes ~5 minutes
2. **Volume state** - Tests run against persistent volumes (use `clean` for fresh state)

---

## Implementation Requirements

### Smoke Test Script (Recommended Addition)

Add to `scripts/integration-smoke-test.sh`:

```bash
#!/bin/bash
# Quick smoke test for integration environment
set -e

export DEPLOY_ENV=integration
DEPLOY="./deploy/pi/deploy.sh"

echo "[smoke] Starting stack..."
$DEPLOY deploy

echo "[smoke] Checking status..."
$DEPLOY status

echo "[smoke] Running config sync..."
$DEPLOY sync

echo "[smoke] Running stream init..."
$DEPLOY init-streams

echo "[smoke] Stopping stack..."
$DEPLOY stop

echo "[smoke] All tests passed!"
```

### CI Integration (Future)

```yaml
# .github/workflows/integration-test.yml
jobs:
  integration:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Smoke test
        run: ./scripts/integration-smoke-test.sh
```

### Validation Points

For each deploy.sh command:

| Command | Validation Point | How to Check |
|---------|------------------|--------------|
| `sync` | Keys in etcd | `etcdctl get --prefix /` |
| `init-streams` | Stream keys | `etcdctl get --prefix /air-quality/streams/` |
| `sync-dictionary` | Rows in tables | `psql -c "SELECT COUNT(*) FROM data_dictionary.streams"` |
| `silver-migrate` | Schema exists | `psql -c "SELECT 1 FROM silver.air_quality_observations LIMIT 0"` |

---

## Alternatives Considered

### Alternative 1: Unit Tests for deploy.sh

Write bash unit tests (bats) for each function.

**Rejected because**:
- Most value is in integration, not unit testing
- Functions depend on Docker/etcd state
- Would duplicate integration tests

### Alternative 2: Docker-in-Docker Testing

Run tests inside a Docker container with Docker socket.

**Rejected because**:
- Adds complexity
- Slower startup
- Integration test already runs in container (devcontainer)

### Alternative 3: Mock Services

Replace etcd/TimescaleDB with mocks for faster tests.

**Rejected because**:
- Defeats purpose of integration testing
- Mocks can diverge from real behavior
- Real services start quickly enough

---

## Related Decisions

- **ADR-017-001**: Integration Environment Design (topology parity)
- **ADR-016-001**: Config Source of Truth (what sync commands sync)
- **ADR-016-002**: Declarative Deploy (future manifest-driven testing)

---

## References

- `deploy/pi/deploy.sh` - Deployment entry point
- `scripts/integration-test.sh` - Test harness script
- `scripts/sync-config-to-etcd.sh` - Config sync implementation
- `deploy/pi/configs/streams/init-streams.sh` - Stream initialization
- `product/features/dp-017/SCOPE.md` - Feature scope with success criteria
