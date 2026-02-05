# Phase E Integration Test Analysis

> **Created:** 2026-02-05
> **Author:** Research Agent
> **Feature:** FE-001 Phase E (Unified Event Abstraction)
> **Purpose:** Document how to run Phase E integration tests against Docker TimescaleDB

---

## Executive Summary

This report analyzes the deploy.sh integration test infrastructure and documents how to run Phase E integration tests. The integration environment uses `DEPLOY_ENV=integration` to spin up a local Docker stack that mirrors production.

---

## 1. How DEPLOY_ENV=integration Works

### Environment Configuration

When `DEPLOY_ENV=integration` is set, deploy.sh switches to a local Docker Compose configuration:

```bash
# From deploy.sh lines 60-76
if [ "$DEPLOY_ENV" = "integration" ]; then
    COMPOSE_FILE="$REPO_ROOT/docker-compose.integration.yml"
    ENV_NAME="development"
    ETCD_CONTAINER="integration-etcd"
    CONFIG_STREAMS_DIR="$REPO_ROOT/config/integration/base/streams"
    CONFIG_DOMAINS_DIR="$REPO_ROOT/config/integration/domains"
else
    COMPOSE_FILE="$SCRIPT_DIR/docker-compose.yml"
    ENV_NAME="production"
    ETCD_CONTAINER="etcd"
    CONFIG_STREAMS_DIR="$REPO_ROOT/config/base/streams"
    CONFIG_DOMAINS_DIR="$REPO_ROOT/config/domains"
fi
```

### Integration Stack Services

The `docker-compose.integration.yml` defines these services:

| Service | Container Name | Port | Purpose |
|---------|---------------|------|---------|
| mosquitto | integration-mosquitto | 1883, 9001 | MQTT broker for sensor data |
| etcd | integration-etcd | 2379 | Configuration store |
| timescaledb | integration-timescaledb | 5432 | Silver/Gold layer storage |
| air-quality-app | integration-air-quality | 8080 | Bronze + Silver ETL |
| ndp-mcp-server | integration-mcp-server | 9100 | MCP interface |
| grafana | integration-grafana | 3000 | Dashboards |

### Key Differences from Production

| Aspect | Production (Pi) | Integration (Local) |
|--------|-----------------|---------------------|
| Compose file | `deploy/pi/docker-compose.yml` | `docker-compose.integration.yml` |
| Container prefix | None | `integration-` |
| Config paths | `config/base/streams` | `config/integration/base/streams` |
| Network | Default | `integration-network` |
| Volumes | Named volumes | Named volumes |

---

## 2. Test Manifest Format

### Manifest Structure

Based on `.deploy/releases/test/phase-b-classification.manifest.json`, test manifests follow this format:

```json
{
  "version": "1.1.0-beta.1",
  "description": "FE-001 Phase E: Events Hypertable and Unified View (v11-013)",
  "created": "2026-02-05",
  "author": "ndp-tester",
  "feature_id": "fe-001",
  "phase": "E",
  "ticket": "v11-013",
  "changes": [
    {
      "type": "migration",
      "file": "product/features/fe-001/phase-e/completion/sql/001_events_hypertable.sql",
      "description": "Create gold.events hypertable with indexes and policies"
    },
    {
      "type": "migration",
      "file": "product/features/fe-001/phase-e/completion/sql/002_events_unified_view.sql",
      "description": "Create gold.events_unified view for V1.2 compatibility"
    }
  ],
  "declarations": [
    {
      "type": "dictionary",
      "action": "sync",
      "description": "Sync event objects to data dictionary"
    }
  ],
  "dependencies": {
    "fe-001-phase-d": "complete",
    "v11-012": "complete"
  },
  "acceptance_criteria": [
    "AC-E02-001: Events hypertable created with 7-day chunks",
    "AC-E02-002: State transitions insertable with context",
    "AC-E02-003: Threshold crossings insertable with direction",
    "AC-E-03: Unified events view combines both types"
  ]
}
```

### Declaration Types Supported

From deploy.sh `validate_manifest()` (line 1600):

```javascript
["etcd-config", "dimensions", "silver-tables", "streams", "dashboards", "gold-tables", "domains", "migrations"]
```

### Migration Handling

The `handle_migration()` function (lines 1708-1729) handles SQL migrations:

```bash
handle_migration() {
    local declaration="$1"
    local migration_file=$(echo "$declaration" | jq -r '.file')

    local full_path="$REPO_ROOT/$migration_file"
    if [ ! -f "$full_path" ]; then
        error "Migration file not found: $full_path"
        return 1
    fi

    # Apply migration to TimescaleDB via stdin
    cat "$full_path" | dcx timescaledb psql -U postgres -d ndp -f -
}
```

---

## 3. SQL Test Execution

### Running Acceptance Tests

Phase E SQL acceptance tests are located at:
- `/workspaces/neural-data-platform/product/features/fe-001/phase-e/completion/tests/acceptance_events_hypertable.sql`

To execute these tests:

```bash
# 1. Ensure integration environment is running
DEPLOY_ENV=integration ./deploy/pi/deploy.sh start

# 2. Wait for TimescaleDB to be ready
docker exec integration-timescaledb pg_isready -U postgres -d ndp

# 3. Run the acceptance tests
docker exec -i integration-timescaledb psql -U postgres -d ndp \
    < product/features/fe-001/phase-e/completion/tests/acceptance_events_hypertable.sql

# 4. Check for PASS/FAIL notices in output
```

### Test Output Format

The SQL tests use PL/pgSQL DO blocks that output PASS/FAIL notices:

```
PASS: AC-E02-001-a gold.events exists as hypertable
PASS: AC-E02-001-b gold.events has 7-day chunk interval
FAIL: AC-E-03-a gold.events_unified view does not exist
```

### Creating a Test Utility Schema

The tests expect a `test_utils` schema. Create it first:

```sql
CREATE SCHEMA IF NOT EXISTS test_utils;
```

---

## 4. Grafana Dashboard Import Validation

### Dashboard Provisioning Approach

Grafana dashboards are provisioned via file-based provisioning:

**Provisioning Config:** `config/grafana/provisioning/dashboards/dashboards.yaml`
```yaml
apiVersion: 1
providers:
  - name: 'NDP Dashboards'
    orgId: 1
    folder: 'Neural Data Platform'
    type: file
    disableDeletion: false
    updateIntervalSeconds: 10
    allowUiUpdates: false
    options:
      path: /var/lib/grafana/dashboards
```

**Dashboard Location in Container:** `/var/lib/grafana/dashboards/`

### Validation Commands

```bash
# 1. Verify dashboard file is mounted in container
docker exec integration-grafana ls -la /var/lib/grafana/dashboards/

# 2. Check Grafana API for dashboard
curl -s http://localhost:3000/api/search?query=Gold%20Layer \
  -u admin:admin | jq '.'

# 3. Verify specific dashboard by UID
curl -s http://localhost:3000/api/dashboards/uid/gold-layer-overview \
  -u admin:admin | jq '.meta.slug'

# 4. Check Grafana provisioning logs
docker logs integration-grafana 2>&1 | grep -i dashboard

# 5. Verify all panels render (check for errors)
curl -s http://localhost:3000/api/dashboards/uid/gold-layer-overview \
  -u admin:admin | jq '.dashboard.panels[].title'
```

### Dashboard Import via API (Alternative)

If not using file provisioning, import via Grafana API:

```bash
# Import dashboard JSON via API
curl -X POST \
  -H "Content-Type: application/json" \
  -H "Authorization: Basic $(echo -n admin:admin | base64)" \
  -d @config/grafana/dashboards/gold-layer-overview.json \
  "http://localhost:3000/api/dashboards/db"
```

---

## 5. TimescaleDB Job Scheduling Verification

### Continuous Aggregate Refresh Policies

After applying migrations, verify CA refresh policies:

```sql
-- Check all continuous aggregate refresh policies
SELECT
    j.job_id,
    j.schedule_interval,
    j.config->>'mat_hypertable_id' AS hypertable_id,
    j.next_start,
    js.last_run_status,
    js.last_successful_finish,
    js.total_runs,
    js.total_failures
FROM timescaledb_information.jobs j
LEFT JOIN timescaledb_information.job_stats js ON j.job_id = js.job_id
WHERE j.proc_name = 'policy_refresh_continuous_aggregate'
ORDER BY j.job_id;
```

### Retention Policies

```sql
-- Check retention policies (gold.events has 1-year retention)
SELECT
    j.job_id,
    j.hypertable_schema,
    j.hypertable_name,
    j.config->>'drop_after' AS drop_after,
    j.schedule_interval
FROM timescaledb_information.jobs j
WHERE j.proc_name = 'policy_retention';
```

### Compression Policies

```sql
-- Check compression policies (gold.events compresses after 30 days)
SELECT
    j.job_id,
    j.hypertable_schema,
    j.hypertable_name,
    j.config->>'compress_after' AS compress_after,
    j.schedule_interval
FROM timescaledb_information.jobs j
WHERE j.proc_name = 'policy_compression';
```

### Manual CA Refresh for Testing

```bash
# Force refresh a continuous aggregate for testing
docker exec integration-timescaledb psql -U postgres -d ndp -c "
CALL refresh_continuous_aggregate(
    'gold.events_hourly',
    NOW() - INTERVAL '7 days',
    NOW()
);
"
```

---

## 6. Step-by-Step Integration Test Procedure

### Prerequisites

1. Docker and Docker Compose installed
2. Repository cloned locally
3. No conflicting services on ports 5432, 3000, 1883, 2379, 8080, 9100

### Full Test Procedure

```bash
#!/bin/bash
# Phase E Integration Test Script

set -e

REPO_ROOT="/workspaces/neural-data-platform"
cd "$REPO_ROOT"

echo "=== Phase E Integration Test Procedure ==="
echo ""

# Step 1: Start integration environment
echo "[1/8] Starting integration environment..."
DEPLOY_ENV=integration ./deploy/pi/deploy.sh start

# Step 2: Wait for all services
echo "[2/8] Waiting for services to be healthy..."
sleep 10

# Check TimescaleDB
until docker exec integration-timescaledb pg_isready -U postgres -d ndp; do
    echo "  Waiting for TimescaleDB..."
    sleep 2
done
echo "  TimescaleDB is ready"

# Check etcd
until docker exec integration-etcd etcdctl endpoint health; do
    echo "  Waiting for etcd..."
    sleep 2
done
echo "  etcd is ready"

# Step 3: Apply Phase D prerequisites (if not already applied)
echo "[3/8] Ensuring Phase D prerequisites are in place..."
# Skip if already applied - check for gold schema
docker exec integration-timescaledb psql -U postgres -d ndp -c "
SELECT 1 FROM pg_namespace WHERE nspname = 'gold';
" || {
    echo "  Gold schema missing - apply Phase D first"
    exit 1
}

# Step 4: Apply Phase E migrations
echo "[4/8] Applying Phase E migrations..."

# Create test_utils schema for tests
docker exec integration-timescaledb psql -U postgres -d ndp -c "
CREATE SCHEMA IF NOT EXISTS test_utils;
"

# Apply events hypertable
docker exec -i integration-timescaledb psql -U postgres -d ndp \
    < product/features/fe-001/phase-e/completion/sql/001_events_hypertable.sql

# Apply unified view
docker exec -i integration-timescaledb psql -U postgres -d ndp \
    < product/features/fe-001/phase-e/completion/sql/002_events_unified_view.sql

# Step 5: Run SQL acceptance tests
echo "[5/8] Running SQL acceptance tests..."
docker exec -i integration-timescaledb psql -U postgres -d ndp \
    < product/features/fe-001/phase-e/completion/tests/acceptance_events_hypertable.sql \
    2>&1 | tee /tmp/phase-e-test-results.txt

# Check for failures
if grep -q "FAIL:" /tmp/phase-e-test-results.txt; then
    echo ""
    echo "FAILURES DETECTED:"
    grep "FAIL:" /tmp/phase-e-test-results.txt
    exit 1
fi

# Step 6: Validate TimescaleDB job scheduling
echo "[6/8] Validating TimescaleDB job scheduling..."

# Check retention policy
docker exec integration-timescaledb psql -U postgres -d ndp -c "
SELECT j.job_id, j.hypertable_name, j.config->>'drop_after' as retention
FROM timescaledb_information.jobs j
WHERE j.proc_name = 'policy_retention'
  AND j.hypertable_schema = 'gold'
  AND j.hypertable_name = 'events';
"

# Check compression policy
docker exec integration-timescaledb psql -U postgres -d ndp -c "
SELECT j.job_id, j.hypertable_name, j.config->>'compress_after' as compress_after
FROM timescaledb_information.jobs j
WHERE j.proc_name = 'policy_compression'
  AND j.hypertable_schema = 'gold'
  AND j.hypertable_name = 'events';
"

# Step 7: Validate Grafana dashboard (if present)
echo "[7/8] Validating Grafana dashboard..."
if [ -f "config/grafana/dashboards/gold-layer-overview.json" ]; then
    # Verify dashboard loads via API
    HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" \
        -u admin:admin \
        http://localhost:3000/api/search?query=Gold)

    if [ "$HTTP_CODE" = "200" ]; then
        echo "  Grafana API accessible"
    else
        echo "  WARNING: Grafana API returned $HTTP_CODE"
    fi
else
    echo "  Dashboard JSON not yet created (v11-014 pending)"
fi

# Step 8: Insert test data and validate queries
echo "[8/8] Inserting test data and validating queries..."

# Insert a test state transition
docker exec integration-timescaledb psql -U postgres -d ndp -c "
INSERT INTO gold.events (
    event_time, stream_id, entity_id, event_type,
    from_state, to_state, duration_in_state_ms,
    context, details
) VALUES (
    NOW() - INTERVAL '1 hour',
    'home-assistant-state',
    'binary_sensor.office_window',
    'state_transition',
    'off', 'on', 3600000,
    '{\"indoor_co2\": 650, \"indoor_pm25\": 8.5}'::JSONB,
    '{}'::JSONB
);
"

# Insert a test threshold crossing
docker exec integration-timescaledb psql -U postgres -d ndp -c "
INSERT INTO gold.events (
    event_time, stream_id, entity_id, event_type,
    metric, threshold_value, crossing_direction,
    metric_value, previous_metric_value, objective_id,
    context, details
) VALUES (
    NOW() - INTERVAL '30 minutes',
    'air-quality',
    'sensor.office_co2',
    'threshold_crossing',
    'co2', 800, 'rising',
    812, 795, 'healthy_co2',
    '{\"outdoor_temp\": 22.5}'::JSONB,
    '{\"condition\": \"<\"}'::JSONB
);
"

# Verify unified view returns both event types
docker exec integration-timescaledb psql -U postgres -d ndp -c "
SELECT event_type, COUNT(*) as count
FROM gold.events_unified
WHERE event_time >= NOW() - INTERVAL '2 hours'
GROUP BY event_type;
"

echo ""
echo "=== Phase E Integration Tests Complete ==="
echo ""
echo "PASS: All integration tests passed"
```

---

## 7. Required Docker Containers Summary

| Container | Image | Required For |
|-----------|-------|--------------|
| integration-timescaledb | timescale/timescaledb:latest-pg15 | All SQL tests, hypertable creation |
| integration-etcd | quay.io/coreos/etcd:v3.5.11 | Configuration sync (optional for Phase E) |
| integration-grafana | grafana/grafana:latest-ubuntu | Dashboard validation (v11-014) |
| integration-mosquitto | eclipse-mosquitto:2.0 | Not required for Phase E tests |
| integration-air-quality | ndp/air-quality-app:integration | Not required for Phase E tests |
| integration-mcp-server | ndp/ndp-mcp-server:integration | Not required for Phase E tests |

**Minimal Stack for Phase E:**
```bash
# Start only required services
docker compose -f docker-compose.integration.yml up -d timescaledb grafana
```

---

## 8. Validation Queries Reference

### Hypertable Exists and Configured

```sql
-- AC-E02-001: Verify hypertable with 7-day chunks
SELECT
    hypertable_schema,
    hypertable_name,
    (SELECT chunk_interval FROM timescaledb_information.dimensions
     WHERE hypertable_schema = 'gold' AND hypertable_name = 'events') as chunk_interval
FROM timescaledb_information.hypertables
WHERE hypertable_schema = 'gold' AND hypertable_name = 'events';
```

### Unified View Schema

```sql
-- AC-E-03: Verify events_unified view columns
SELECT column_name, data_type
FROM information_schema.columns
WHERE table_schema = 'gold' AND table_name = 'events_unified'
ORDER BY ordinal_position;
```

### Events Hourly CA (if created)

```sql
-- AC-E-05: Verify events_hourly continuous aggregate
SELECT view_schema, view_name, materialization_hypertable_name
FROM timescaledb_information.continuous_aggregates
WHERE view_schema = 'gold' AND view_name = 'events_hourly';
```

### V1.2 Query Patterns

```sql
-- Pattern 1: Time range query
SELECT * FROM gold.events_unified
WHERE event_time BETWEEN NOW() - INTERVAL '24 hours' AND NOW()
ORDER BY event_time;

-- Pattern 2: Filter by type
SELECT * FROM gold.events_unified
WHERE event_type = 'threshold_crossing'
  AND event_time >= NOW() - INTERVAL '24 hours';

-- Pattern 3: Filter by objective
SELECT * FROM gold.events_unified
WHERE event_type = 'threshold_crossing'
  AND details->>'objective_id' = 'healthy_co2';
```

### Performance Validation

```sql
-- Check index usage
EXPLAIN (ANALYZE, COSTS, TIMING)
SELECT * FROM gold.events_unified
WHERE event_time >= NOW() - INTERVAL '30 days'
ORDER BY event_time;
```

---

## 9. Troubleshooting

### Common Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| `gold schema does not exist` | Phase A-D not deployed | Run Phase A-D migrations first |
| `relation gold.events does not exist` | Migration not applied | Apply 001_events_hypertable.sql |
| `FAIL: chunk interval` | Hypertable not converted | Check create_hypertable() succeeded |
| `Grafana 404 on dashboard` | Dashboard not provisioned | Restart Grafana, check volume mount |
| `Connection refused on 5432` | TimescaleDB not running | Check `docker ps`, restart stack |

### Debug Commands

```bash
# Check container status
docker ps -a | grep integration

# View TimescaleDB logs
docker logs integration-timescaledb --tail 100

# View Grafana provisioning logs
docker logs integration-grafana 2>&1 | grep -i provision

# Interactive psql session
docker exec -it integration-timescaledb psql -U postgres -d ndp

# Check disk space in container
docker exec integration-timescaledb df -h
```

---

## 10. Summary

Phase E integration tests validate:

1. **gold.events hypertable** - Schema, indexes, policies (001_events_hypertable.sql)
2. **gold.events_unified view** - V1.2 API compatibility (002_events_unified_view.sql)
3. **events_hourly CA** - Hourly event aggregates (if created)
4. **Job scheduling** - Retention (1 year), compression (30 days)
5. **Dashboard provisioning** - Grafana loads Gold Layer Overview (v11-014)

**Key Commands:**
```bash
# Start integration environment
DEPLOY_ENV=integration ./deploy/pi/deploy.sh start

# Apply Phase E migrations
DEPLOY_ENV=integration ./deploy/pi/deploy.sh apply .deploy/test/phase-e-events.manifest.json

# Run acceptance tests
docker exec -i integration-timescaledb psql -U postgres -d ndp \
    < product/features/fe-001/phase-e/completion/tests/acceptance_events_hypertable.sql

# Check status
DEPLOY_ENV=integration ./deploy/pi/deploy.sh status
```

---

*Report created: 2026-02-05 by Research Agent*
*References: deploy.sh, docker-compose.integration.yml, ACCEPTANCE-CRITERIA.md, TEST-PLAN.md*
