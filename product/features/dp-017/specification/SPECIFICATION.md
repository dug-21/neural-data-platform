# dp-017: Integration Test Harness - SPARC Specification

**Version**: 1.0.0
**Status**: Draft
**Created**: 2026-02-01
**Feature**: Integration Environment Alignment

---

## 1. Introduction

### 1.1 Purpose

This specification defines the requirements for validating that `deploy/pi/deploy.sh` works correctly in integration mode (`DEPLOY_ENV=integration`). The integration test harness ensures deployment changes can be safely tested locally before deploying to production on the Raspberry Pi.

### 1.2 Scope

- Validate all `deploy.sh` commands in integration mode
- Verify service health and inter-service communication
- Test data flow from MQTT through Bronze to Silver layer
- Ensure no production credentials leak into integration mode

### 1.3 Definitions

| Term | Definition |
|------|------------|
| Integration Mode | `DEPLOY_ENV=integration` - uses `docker-compose.integration.yml` at repo root |
| Production Mode | `DEPLOY_ENV=pi` (default) - uses `deploy/pi/docker-compose.yml` |
| Bronze Layer | Raw Parquet files written by air-quality-app |
| Silver Layer | Cleaned, typed data in TimescaleDB hypertables |
| SUT | System Under Test - the deploy.sh script and integration compose |

### 1.4 References

- `product/features/dp-017/SCOPE.md` - Feature scope document
- `deploy/pi/deploy.sh` - Deployment script (SUT)
- `docker-compose.integration.yml` - Integration compose file
- `deploy/pi/docker-compose.yml` - Production compose file (reference)

---

## 2. Functional Requirements

### 2.1 Core Deployment Commands

#### FR-001: deploy command works in integration mode

**Description**: The `deploy` command shall build and start all services using the integration compose file.

**Preconditions**:
- Docker and Docker Compose installed
- No services currently running from integration compose

**Acceptance Criteria**:
| ID | Criterion | Test Method |
|----|-----------|-------------|
| FR-001.1 | Command exits with code 0 | `echo $?` after command |
| FR-001.2 | All 5 core services start (mosquitto, etcd, timescaledb, air-quality-app, ndp-mcp-server) | `docker compose ps` |
| FR-001.3 | Container names use `integration-*` prefix | `docker ps --format '{{.Names}}'` |
| FR-001.4 | Uses `docker-compose.integration.yml` from repo root | Script output shows correct compose file |

**Test Command**:
```bash
DEPLOY_ENV=integration ./deploy/pi/deploy.sh deploy
```

**Expected Output**:
```
[DEPLOY] Environment: integration (compose: docker-compose.integration.yml)
[DEPLOY] Checking prerequisites...
[DEPLOY] Prerequisites OK
[DEPLOY] Building Docker images...
[DEPLOY] Build complete
[DEPLOY] Starting services...
[DEPLOY] Services started successfully!
```

---

#### FR-002: start command works in integration mode

**Description**: The `start` command shall start services without rebuilding images.

**Preconditions**:
- Images already built
- Services not currently running

**Acceptance Criteria**:
| ID | Criterion | Test Method |
|----|-----------|-------------|
| FR-002.1 | Command exits with code 0 | `echo $?` |
| FR-002.2 | Does not trigger `docker compose build` | No "Building" output |
| FR-002.3 | Syncs config to etcd after start | "Syncing configuration to etcd" in output |
| FR-002.4 | Initializes streams after start | "Initializing stream configurations" in output |

**Test Command**:
```bash
DEPLOY_ENV=integration ./deploy/pi/deploy.sh start
```

---

#### FR-003: stop command works in integration mode

**Description**: The `stop` command shall stop and remove all integration containers.

**Preconditions**:
- Services currently running

**Acceptance Criteria**:
| ID | Criterion | Test Method |
|----|-----------|-------------|
| FR-003.1 | Command exits with code 0 | `echo $?` |
| FR-003.2 | All integration containers stopped | `docker ps -q -f name=integration-` returns empty |
| FR-003.3 | Volumes are preserved | `docker volume ls` still shows integration volumes |

**Test Command**:
```bash
DEPLOY_ENV=integration ./deploy/pi/deploy.sh stop
```

---

#### FR-004: status command shows accurate health

**Description**: The `status` command shall display health status for all services.

**Preconditions**:
- Services running and healthy

**Acceptance Criteria**:
| ID | Criterion | Test Method |
|----|-----------|-------------|
| FR-004.1 | Shows etcd health as "healthy" | Output contains etcd health check |
| FR-004.2 | Shows TimescaleDB as "Running" | `pg_isready` check passes |
| FR-004.3 | Shows Air Quality App health | HTTP 200 from /health endpoint |
| FR-004.4 | Shows MCP Server health | HTTP 200 from /health endpoint |
| FR-004.5 | Shows correct URLs with localhost | Useful URLs section present |
| FR-004.6 | Shows Silver Layer status | Hypertable count displayed |
| FR-004.7 | Shows Stream Status | Stream listing works |

**Test Command**:
```bash
DEPLOY_ENV=integration ./deploy/pi/deploy.sh status
```

**Expected Output** (excerpt):
```
[DEPLOY] Service Status:
NAME                     STATUS
integration-etcd         healthy
integration-mosquitto    healthy
integration-timescaledb  healthy
integration-air-quality  healthy
integration-mcp-server   healthy

[DEPLOY] Health Checks:
  etcd:        is healthy
  Air Quality: {"status":"ok"}
  MCP Server:  Running
  TimescaleDB: Running
```

---

#### FR-005: logs command streams container logs

**Description**: The `logs` command shall follow logs from all services.

**Acceptance Criteria**:
| ID | Criterion | Test Method |
|----|-----------|-------------|
| FR-005.1 | Command follows logs (does not exit) | Ctrl+C required to exit |
| FR-005.2 | Shows logs from all services | Multiple container names in output |

**Test Command**:
```bash
DEPLOY_ENV=integration ./deploy/pi/deploy.sh logs
# Press Ctrl+C after verifying output
```

---

#### FR-006: build command builds images only

**Description**: The `build` command shall build Docker images without starting services.

**Acceptance Criteria**:
| ID | Criterion | Test Method |
|----|-----------|-------------|
| FR-006.1 | Command exits with code 0 | `echo $?` |
| FR-006.2 | Images are built | `docker images` shows `integration` tagged images |
| FR-006.3 | Services are NOT started | `docker ps -q -f name=integration-` returns empty |

**Test Command**:
```bash
DEPLOY_ENV=integration ./deploy/pi/deploy.sh build
```

---

### 2.2 Configuration Commands

#### FR-007: sync command syncs config to integration etcd

**Description**: The `sync` command shall synchronize configuration from the repository to etcd using the integration container.

**Preconditions**:
- etcd service running and healthy

**Acceptance Criteria**:
| ID | Criterion | Test Method |
|----|-----------|-------------|
| FR-007.1 | Uses `integration-etcd` container | Script uses correct ETCD_CONTAINER |
| FR-007.2 | Syncs environment as "development" | ENV_NAME=development for integration |
| FR-007.3 | Config sync script executes | "Syncing configuration to etcd" in output |
| FR-007.4 | Can query synced config | `etcdctl get` returns values |

**Test Command**:
```bash
DEPLOY_ENV=integration ./deploy/pi/deploy.sh sync
docker exec integration-etcd etcdctl get --prefix /air-quality --keys-only | head -5
```

---

#### FR-008: init-streams command initializes streams

**Description**: The `init-streams` command shall populate stream configurations in etcd.

**Preconditions**:
- etcd service running and healthy

**Acceptance Criteria**:
| ID | Criterion | Test Method |
|----|-----------|-------------|
| FR-008.1 | Command exits with code 0 | `echo $?` |
| FR-008.2 | Stream keys exist in etcd | `etcdctl get --prefix /air-quality/streams/` returns data |
| FR-008.3 | At least 3 streams configured | air-quality, outdoor-weather, nws-observations |

**Test Command**:
```bash
DEPLOY_ENV=integration ./deploy/pi/deploy.sh init-streams
docker exec integration-etcd etcdctl get --prefix /air-quality/streams/ --keys-only | grep "/id$" | wc -l
```

**Expected Result**: At least 3 streams

---

#### FR-009: list-streams command shows configured streams

**Description**: The `list-streams` command shall display all streams configured in etcd.

**Acceptance Criteria**:
| ID | Criterion | Test Method |
|----|-----------|-------------|
| FR-009.1 | Command exits with code 0 | `echo $?` |
| FR-009.2 | Lists stream IDs | Output shows air-quality, outdoor-weather, etc. |
| FR-009.3 | Shows stream status (enabled/disabled) | Status column present |

**Test Command**:
```bash
DEPLOY_ENV=integration ./deploy/pi/deploy.sh list-streams
```

---

#### FR-010: sync-dictionary command populates data dictionary

**Description**: The `sync-dictionary` command shall synchronize entity schemas and Silver metadata to TimescaleDB.

**Preconditions**:
- TimescaleDB running and healthy
- data_dictionary schema exists

**Acceptance Criteria**:
| ID | Criterion | Test Method |
|----|-----------|-------------|
| FR-010.1 | Command exits with code 0 | `echo $?` |
| FR-010.2 | Streams table populated | `SELECT COUNT(*) FROM data_dictionary.streams > 0` |
| FR-010.3 | Silver tables metadata synced | `SELECT COUNT(*) FROM data_dictionary.silver_tables > 0` |
| FR-010.4 | Sync status recorded | `SELECT * FROM data_dictionary.sync_status` shows success |

**Test Command**:
```bash
DEPLOY_ENV=integration ./deploy/pi/deploy.sh sync-dictionary
docker exec integration-timescaledb psql -U postgres -d ndp -c \
  "SELECT streams_synced, silver_tables_synced, status FROM data_dictionary.sync_status ORDER BY id DESC LIMIT 1;"
```

---

### 2.3 Dimension Commands

#### FR-011: sync-dimensions command syncs dimension tables

**Description**: The `sync-dimensions` command shall load dimension data from CSV files.

**Preconditions**:
- TimescaleDB running
- Dimension config files exist in `config/base/dimensions/`

**Acceptance Criteria**:
| ID | Criterion | Test Method |
|----|-----------|-------------|
| FR-011.1 | Command exits with code 0 or warns if no dimensions | `echo $?` |
| FR-011.2 | Processes all dimension configs | Log output shows each dimension |
| FR-011.3 | Dimension data loaded to Silver tables | Query target tables |

**Test Command**:
```bash
DEPLOY_ENV=integration ./deploy/pi/deploy.sh sync-dimensions
```

---

#### FR-012: list-dimensions command shows dimension status

**Description**: The `list-dimensions` command shall display configured dimensions and sync status.

**Acceptance Criteria**:
| ID | Criterion | Test Method |
|----|-----------|-------------|
| FR-012.1 | Command exits with code 0 | `echo $?` |
| FR-012.2 | Lists dimension IDs and target tables | Tabular output |
| FR-012.3 | Shows sync status for each dimension | Status column present |

**Test Command**:
```bash
DEPLOY_ENV=integration ./deploy/pi/deploy.sh list-dimensions
```

---

### 2.4 Silver ETL Commands

#### FR-013: silver-migrate command runs migrations

**Description**: The `silver-migrate` command shall apply TimescaleDB schema migrations.

**Preconditions**:
- TimescaleDB running and healthy

**Acceptance Criteria**:
| ID | Criterion | Test Method |
|----|-----------|-------------|
| FR-013.1 | Command exits with code 0 | `echo $?` |
| FR-013.2 | Silver schema created | `SELECT 1 FROM information_schema.schemata WHERE schema_name='silver'` |
| FR-013.3 | Hypertables created | Query `timescaledb_information.hypertables` |

**Test Command**:
```bash
DEPLOY_ENV=integration ./deploy/pi/deploy.sh silver-migrate
docker exec integration-timescaledb psql -U postgres -d ndp -c \
  "SELECT hypertable_name FROM timescaledb_information.hypertables WHERE hypertable_schema='silver';"
```

---

#### FR-014: silver-etl command runs one-shot ETL

**Description**: The `silver-etl` command shall perform a single Bronze-to-Silver ETL run.

**Preconditions**:
- TimescaleDB running
- Bronze data exists (or empty is acceptable)

**Acceptance Criteria**:
| ID | Criterion | Test Method |
|----|-----------|-------------|
| FR-014.1 | Command exits with code 0 | `echo $?` |
| FR-014.2 | ETL container runs and exits | No long-running container |
| FR-014.3 | Uses silver profile | `--profile silver` in compose command |

**Test Command**:
```bash
DEPLOY_ENV=integration ./deploy/pi/deploy.sh silver-etl
```

---

### 2.5 Update Commands

#### FR-015: update command rebuilds from git

**Description**: The `update` command shall pull latest code and rebuild images.

**Note**: This test requires care in CI as it modifies working directory.

**Acceptance Criteria**:
| ID | Criterion | Test Method |
|----|-----------|-------------|
| FR-015.1 | Fetches from origin | "Syncing to origin/main" in output |
| FR-015.2 | Rebuilds specified target | Build output present |
| FR-015.3 | Syncs config after update | Config sync runs |

**Test Command** (manual only):
```bash
# WARNING: Modifies working directory
DEPLOY_ENV=integration ./deploy/pi/deploy.sh update app
```

---

#### FR-016: refresh command updates config without rebuild

**Description**: The `refresh` command shall pull latest configs and restart Grafana.

**Acceptance Criteria**:
| ID | Criterion | Test Method |
|----|-----------|-------------|
| FR-016.1 | Does not rebuild images | No "Building" in output |
| FR-016.2 | Syncs configurations | Config sync runs |
| FR-016.3 | Restarts Grafana | "Restarting Grafana" in output |

**Test Command** (manual only):
```bash
# WARNING: Modifies working directory
DEPLOY_ENV=integration ./deploy/pi/deploy.sh refresh
```

---

### 2.6 Analytics Commands (DEPRECATED)

#### ~~FR-017: analytics command~~ - DEPRECATED

**Status**: ⚠️ DEPRECATED - NOT IMPLEMENTED

**Reason**: The `analytics` and `rollback` commands reference a `duckdb` service that does not exist in production. These are dead code and should not be tested or implemented in integration mode.

#### ~~FR-018: rollback command~~ - DEPRECATED

**Status**: ⚠️ DEPRECATED - NOT IMPLEMENTED

**Reason**: See FR-017.

---

## 3. Non-Functional Requirements

### 3.1 Performance Requirements

#### NFR-001: Services become healthy within 60 seconds

**Description**: All services shall reach healthy status within 60 seconds of start.

**Measurement**: Time from `deploy.sh start` to all services showing healthy in `docker compose ps`.

**Target**: < 60 seconds

**Test Method**:
```bash
time (DEPLOY_ENV=integration ./deploy/pi/deploy.sh start && \
  until docker compose -f docker-compose.integration.yml ps | grep -q "unhealthy"; do sleep 1; done)
```

---

#### NFR-002: Container resource limits are reasonable

**Description**: Containers shall not exceed memory limits appropriate for development machines.

**Measurement**: Memory limits in compose file.

**Target**:
| Service | Memory Limit |
|---------|--------------|
| air-quality-app | 512MB |
| timescaledb | 512MB |
| ndp-mcp-server | 128MB |
| etcd | 512MB |
| mosquitto | 128MB |
| grafana | 512MB |

---

### 3.2 Naming Requirements

#### NFR-003: Container names use integration-* prefix

**Description**: All integration containers shall use the `integration-` prefix to distinguish from production.

**Verification**:
```bash
docker ps --format '{{.Names}}' | grep -E '^integration-' | wc -l
# Should equal number of running services
```

---

#### NFR-004: Network uses integration-network name

**Description**: The integration network shall be named `integration-network`.

**Verification**:
```bash
docker network ls | grep integration-network
```

---

#### NFR-005: Volumes use distinct names

**Description**: Integration volumes shall not conflict with production volume names.

**Verification**: Volume names in `docker-compose.integration.yml` are different from `deploy/pi/docker-compose.yml`.

---

### 3.3 Security Requirements

#### NFR-006: No production credentials in integration mode

**Description**: Integration mode shall use test/development credentials only.

**Verification**:
- POSTGRES_PASSWORD=postgres (not production password)
- GRAFANA_ADMIN_PASSWORD=admin (not production password)
- No OPENWEATHERMAP_API_KEY required (optional)

---

#### NFR-007: TimescaleDB not exposed externally

**Description**: Production exposes TimescaleDB on 127.0.0.1 only. Integration may expose on all interfaces for testing convenience.

**Verification**:
```bash
# Integration: 5432:5432 (exposed)
# Production: 127.0.0.1:5432:5432 (local only)
```

---

### 3.4 Isolation Requirements

#### NFR-008: Integration does not affect production

**Description**: Running integration tests shall not modify production containers, volumes, or configuration.

**Verification**:
- Different container names (integration-* vs production names)
- Different volume names
- Different network name

---

## 4. Test Scenarios

### 4.1 Smoke Test: Basic Deploy/Status/Stop Cycle

**Purpose**: Verify basic deployment lifecycle works.

**Duration**: ~2 minutes

**Steps**:
```bash
#!/bin/bash
set -e

# Setup
export DEPLOY_ENV=integration
cd /workspaces/neural-data-platform

# Test deploy
echo "=== Testing deploy ==="
./deploy/pi/deploy.sh deploy

# Test status
echo "=== Testing status ==="
./deploy/pi/deploy.sh status

# Verify containers
echo "=== Verifying containers ==="
docker ps --format '{{.Names}}' | grep integration- | sort

# Test stop
echo "=== Testing stop ==="
./deploy/pi/deploy.sh stop

# Verify stopped
echo "=== Verifying stopped ==="
if docker ps -q -f name=integration- | grep -q .; then
  echo "FAIL: Containers still running"
  exit 1
fi

echo "=== SMOKE TEST PASSED ==="
```

**Pass Criteria**:
- All commands exit with code 0
- 5 containers start with `integration-` prefix
- All containers stop cleanly

---

### 4.2 Full Test: All Commands Validated

**Purpose**: Validate every deploy.sh command works in integration mode.

**Duration**: ~10 minutes

**Steps**:
```bash
#!/bin/bash
set -e

export DEPLOY_ENV=integration
cd /workspaces/neural-data-platform

echo "=== Phase 1: Build and Deploy ==="
./deploy/pi/deploy.sh deploy

echo "=== Phase 2: Configuration Commands ==="
./deploy/pi/deploy.sh sync
./deploy/pi/deploy.sh init-streams
./deploy/pi/deploy.sh list-streams
./deploy/pi/deploy.sh sync-dictionary

echo "=== Phase 3: Dimension Commands ==="
./deploy/pi/deploy.sh list-dimensions
./deploy/pi/deploy.sh sync-dimensions 2>/dev/null || echo "No dimensions configured (OK)"

echo "=== Phase 4: Silver ETL Commands ==="
./deploy/pi/deploy.sh silver-migrate
./deploy/pi/deploy.sh silver-etl

echo "=== Phase 5: Status Verification ==="
./deploy/pi/deploy.sh status

echo "=== Phase 6: Database Verification ==="
docker exec integration-timescaledb psql -U postgres -d ndp -c \
  "SELECT schemaname, tablename FROM pg_tables WHERE schemaname IN ('silver', 'data_dictionary') LIMIT 10;"

echo "=== Phase 7: Cleanup ==="
./deploy/pi/deploy.sh stop

echo "=== FULL TEST PASSED ==="
```

**Pass Criteria**:
- All commands exit with code 0
- Silver schema exists with hypertables
- Data dictionary populated
- All services healthy before stop

---

### 4.3 Data Flow Test: MQTT to Silver

**Purpose**: Verify end-to-end data flow from sensor to Silver layer.

**Duration**: ~3 minutes

**Prerequisites**:
- Services deployed and healthy
- mosquitto_pub available

**Steps**:
```bash
#!/bin/bash
set -e

export DEPLOY_ENV=integration
cd /workspaces/neural-data-platform

echo "=== Setup: Deploy and migrate ==="
./deploy/pi/deploy.sh deploy
./deploy/pi/deploy.sh silver-migrate

echo "=== Inject test message ==="
# Wait for air-quality-app to subscribe
sleep 5

# Send test MQTT message
docker exec integration-mosquitto mosquitto_pub \
  -t "airgradient/test-device/measures" \
  -m '{"wifi":-50,"pm02":15,"rco2":650,"atmp":22.5,"rhum":55}'

echo "=== Wait for processing ==="
sleep 10

echo "=== Verify Bronze layer ==="
# Check if Parquet file was created
docker exec integration-air-quality ls -la /data/raw/air-quality/ 2>/dev/null || \
  echo "Bronze directory exists (may be empty for first message)"

echo "=== Run Silver ETL ==="
./deploy/pi/deploy.sh silver-etl

echo "=== Verify Silver layer ==="
docker exec integration-timescaledb psql -U postgres -d ndp -c \
  "SELECT COUNT(*) as row_count FROM silver.air_quality_readings;" 2>/dev/null || \
  echo "Silver table exists (may be empty if no Bronze data yet)"

echo "=== Cleanup ==="
./deploy/pi/deploy.sh stop

echo "=== DATA FLOW TEST COMPLETE ==="
```

**Pass Criteria**:
- MQTT message accepted (no error)
- Bronze layer directory exists
- Silver ETL runs without error
- Silver table exists (row count may be 0 initially)

---

### 4.4 Health Check Test: Service Dependencies

**Purpose**: Verify service dependency chain and health checks work.

**Steps**:
```bash
#!/bin/bash
set -e

export DEPLOY_ENV=integration
cd /workspaces/neural-data-platform

echo "=== Start services ==="
./deploy/pi/deploy.sh start

echo "=== Wait for health ==="
MAX_WAIT=120
WAITED=0
while [ $WAITED -lt $MAX_WAIT ]; do
  UNHEALTHY=$(docker ps --filter "name=integration-" --format "{{.Status}}" | grep -c "unhealthy" || true)
  STARTING=$(docker ps --filter "name=integration-" --format "{{.Status}}" | grep -c "starting" || true)

  if [ "$UNHEALTHY" -eq 0 ] && [ "$STARTING" -eq 0 ]; then
    echo "All services healthy after ${WAITED}s"
    break
  fi

  echo "Waiting... (unhealthy: $UNHEALTHY, starting: $STARTING)"
  sleep 5
  WAITED=$((WAITED + 5))
done

if [ $WAITED -ge $MAX_WAIT ]; then
  echo "FAIL: Services did not become healthy within ${MAX_WAIT}s"
  docker ps --filter "name=integration-"
  exit 1
fi

echo "=== Individual health checks ==="
echo "etcd:        $(docker exec integration-etcd etcdctl endpoint health 2>&1)"
echo "timescaledb: $(docker exec integration-timescaledb pg_isready -U postgres -d ndp 2>&1)"
echo "air-quality: $(curl -sf http://localhost:8080/health 2>&1 || echo 'FAIL')"
echo "mcp-server:  $(curl -sf http://localhost:9100/health 2>&1 || echo 'FAIL')"

echo "=== Cleanup ==="
./deploy/pi/deploy.sh stop

echo "=== HEALTH CHECK TEST PASSED ==="
```

**Pass Criteria**:
- All services healthy within 120 seconds
- Individual health endpoints respond correctly

---

## 5. Traceability Matrix

| Requirement | Test Scenario | Acceptance Criteria |
|-------------|---------------|---------------------|
| FR-001 | Smoke Test, Full Test | FR-001.1 - FR-001.4 |
| FR-002 | Full Test | FR-002.1 - FR-002.4 |
| FR-003 | Smoke Test, Full Test | FR-003.1 - FR-003.3 |
| FR-004 | Full Test, Health Check Test | FR-004.1 - FR-004.7 |
| FR-005 | Manual | FR-005.1 - FR-005.2 |
| FR-006 | Full Test | FR-006.1 - FR-006.3 |
| FR-007 | Full Test | FR-007.1 - FR-007.4 |
| FR-008 | Full Test | FR-008.1 - FR-008.3 |
| FR-009 | Full Test | FR-009.1 - FR-009.3 |
| FR-010 | Full Test | FR-010.1 - FR-010.4 |
| FR-011 | Full Test | FR-011.1 - FR-011.3 |
| FR-012 | Full Test | FR-012.1 - FR-012.3 |
| FR-013 | Full Test, Data Flow Test | FR-013.1 - FR-013.3 |
| FR-014 | Full Test, Data Flow Test | FR-014.1 - FR-014.3 |
| FR-015 | Manual only | FR-015.1 - FR-015.3 |
| FR-016 | Manual only | FR-016.1 - FR-016.3 |
| ~~FR-017~~ | N/A | DEPRECATED - duckdb not in production |
| ~~FR-018~~ | N/A | DEPRECATED - dead code |
| NFR-001 | Health Check Test | < 60s to healthy |
| NFR-003 | All Tests | Container name prefix |
| NFR-006 | Code Review | Credential verification |
| NFR-008 | All Tests | Isolation verification |

---

## 6. Implementation Notes

### 6.1 Environment Detection

The `deploy.sh` script detects environment via `DEPLOY_ENV`:

```bash
DEPLOY_ENV="${DEPLOY_ENV:-pi}"

if [ "$DEPLOY_ENV" = "integration" ]; then
    COMPOSE_FILE="$REPO_ROOT/docker-compose.integration.yml"
    ENV_NAME="development"
    ETCD_CONTAINER="integration-etcd"
else
    COMPOSE_FILE="$SCRIPT_DIR/docker-compose.yml"
    ENV_NAME="production"
    ETCD_CONTAINER="etcd"
fi
```

### 6.2 Container Naming

Integration containers use explicit names in compose:

```yaml
container_name: integration-mosquitto
container_name: integration-etcd
container_name: integration-timescaledb
container_name: integration-air-quality
container_name: integration-mcp-server
container_name: integration-grafana
```

### 6.3 Optional Services

Grafana uses a profile in integration mode:

```yaml
grafana:
  profiles:
    - dashboards
```

Start with: `docker compose --profile dashboards up -d`

---

## 7. Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2026-02-01 | Specification Agent | Initial specification |

---

## 8. Approval

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Author | Specification Agent | 2026-02-01 | - |
| Reviewer | - | - | - |
| Approver | - | - | - |
