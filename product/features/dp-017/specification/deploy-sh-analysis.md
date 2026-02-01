# deploy.sh Analysis for Integration Test Harness

**Feature**: dp-017 - Integration Test Harness for Deployment Evolution
**Date**: 2026-02-01
**Analyst**: Research Agent

## Executive Summary

The `deploy/pi/deploy.sh` script is a comprehensive deployment tool that already supports `DEPLOY_ENV=integration` mode. This analysis documents all commands, their dependencies, environment-specific behavior, and identifies gaps that need to be addressed for full integration testing support.

---

## 1. Command Reference

### 1.1 Core Commands

| Command | Description | Dependencies |
|---------|-------------|--------------|
| `deploy` | Full deploy (build + start all services) | docker, docker compose |
| `start` | Start all services | docker compose |
| `stop` | Stop all services | docker compose |
| `logs` | View logs (follows all services) | docker compose |
| `status` | Check service health and display URLs | etcd, curl |
| `build` | Build Docker images only | docker compose |

### 1.2 Update Commands

| Command | Description | Options |
|---------|-------------|---------|
| `update` | Pull latest from git and rebuild | `--no-cache`, targets: `app`, `mcp`, `silver`, `all` |
| `refresh` | Pull latest configs only (no rebuild) | Restarts Grafana |

### 1.3 Configuration Commands

| Command | Description | External Scripts |
|---------|-------------|------------------|
| `sync` | Sync configuration to etcd | `scripts/sync-config-to-etcd.sh` |
| `init-streams` | Initialize stream configurations in etcd | `deploy/pi/configs/streams/init-streams.sh` |
| `list-streams` | List configured streams from etcd | `deploy/pi/configs/streams/list-streams.sh` |
| `sync-dictionary` | Sync entity schemas to TimescaleDB data dictionary | inline function |

### 1.4 Dimension Commands (dp-013)

| Command | Description | Dependencies |
|---------|-------------|--------------|
| `sync-dimensions` | Sync dimension tables from `config/base/dimensions/` | TimescaleDB, YAML configs |
| `list-dimensions` | List configured dimensions and sync status | Local state file |
| `dimension-status` | Show dimension sync status history | Local state file |

### 1.5 Analytics Commands (DEPRECATED)

| Command | Description | Status |
|---------|-------------|--------|
| `analytics` | Start DuckDB + Grafana analytics stack | ⚠️ DEPRECATED - duckdb not in production |
| `rollback` | Stop and remove analytics stack | ⚠️ DEPRECATED - dead code |

**Note**: These commands reference a `duckdb` service that does not exist in production. They are dead code and should NOT be implemented in integration mode.

### 1.6 Silver ETL Commands

| Command | Description | Profile Required |
|---------|-------------|------------------|
| `silver-migrate` | Run Silver Layer TimescaleDB schema migrations | `silver` |
| `silver-etl` | Run Silver ETL once (Bronze -> TimescaleDB) | `silver` |
| `silver-daemon` | Start Silver ETL in daemon mode (continuous) | `silver-daemon` |
| `silver-daemon-stop` | Stop Silver ETL daemon | `silver-daemon` |
| `silver-daemon-logs` | View Silver ETL daemon logs (follows) | - |
| `silver-daemon-status` | Check Silver ETL daemon status | - |

---

## 2. Environment-Specific Behavior

### 2.1 Environment Detection

```bash
DEPLOY_ENV="${DEPLOY_ENV:-pi}"  # Default to pi (production)

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

### 2.2 Key Differences by Environment

| Aspect | Pi (Production) | Integration |
|--------|-----------------|-------------|
| Compose File | `deploy/pi/docker-compose.yml` | `docker-compose.integration.yml` |
| ENV_NAME | `production` | `development` |
| etcd Container | `etcd` | `integration-etcd` |
| Network | `neural-network` | `integration-network` |
| TimescaleDB Container | `pi5-timescaledb` | `integration-timescaledb` |
| Air Quality Container | `air-quality-app` | `integration-air-quality` |
| MCP Server Container | `ndp-mcp-server` | `integration-mcp-server` |
| Grafana Container | `grafana` | `integration-grafana` |

### 2.3 Container Name Usage

The script uses two approaches for Docker operations:

1. **`dc()` helper** - Uses `docker compose -f $COMPOSE_FILE` which operates on service names (consistent)
2. **`dcx()` helper** - Uses `dc exec -T` which also uses service names (consistent)
3. **External scripts** - Use `ETCD_CONTAINER` variable for direct `docker exec` commands

---

## 3. External Scripts Analysis

### 3.1 sync-config-to-etcd.sh

**Location**: `/workspaces/neural-data-platform/scripts/sync-config-to-etcd.sh`

**Purpose**: Sync YAML configuration files to etcd

**Parameters**:
- `$1` - Environment name (passed as `$ENV_NAME`)
- `ETCD_CONTAINER` - Container name for docker exec

**Behavior**:
1. Processes `config/base/*/config.yaml` files
2. Handles `config/base/streams/*/config.yaml` specially (nested directories)
3. Applies environment overlays from `config/overlays/$ENVIRONMENT/`
4. Flattens YAML to key-value pairs using Python

**Integration Mode Impact**: Uses `ETCD_CONTAINER=integration-etcd` correctly

### 3.2 init-streams.sh

**Location**: `/workspaces/neural-data-platform/deploy/pi/configs/streams/init-streams.sh`

**Purpose**: Initialize hardcoded stream configurations in etcd

**Parameters**:
- `$1` - etcd container name (defaults to `etcd`)

**Behavior**:
1. Waits for etcd health check
2. Loads two hardcoded streams:
   - `airgradient-001` (enabled)
   - `airgradient-002` (disabled)
3. Sets global multi-stream configuration

**Integration Mode Impact**: Receives correct container name via `$ETCD_CONTAINER`

### 3.3 list-streams.sh

**Location**: `/workspaces/neural-data-platform/deploy/pi/configs/streams/list-streams.sh`

**Purpose**: List all configured streams from etcd

**Parameters**:
- `$1` - etcd container name (defaults to `etcd`)

**Integration Mode Impact**: Receives correct container name via `$ETCD_CONTAINER`

---

## 4. Service Dependencies

### 4.1 Dependency Graph

```
mosquitto (MQTT)
    |
    v
etcd (config store) <----- sync-config-to-etcd.sh
    |                       init-streams.sh
    v                       list-streams.sh
timescaledb
    |
    +---> air-quality-app (depends: mosquitto, etcd, timescaledb)
    |
    +---> ndp-mcp-server (depends: etcd, timescaledb)
    |
    +---> grafana (depends: timescaledb)
    |
    +---> silver-etl (depends: etcd, timescaledb) [profile: silver]
    |
    +---> silver-etl-daemon (depends: etcd, timescaledb) [profile: silver-daemon]
```

### 4.2 Health Check Requirements

| Service | Health Check Command |
|---------|---------------------|
| etcd | `etcdctl endpoint health` |
| timescaledb | `pg_isready -U postgres -d ndp` |
| air-quality-app | `curl -f http://localhost:8080/health` |
| ndp-mcp-server | `curl -f http://localhost:9100/health` |
| grafana | `wget --spider -q http://localhost:3000/api/health` (pi) / Not defined (integration) |
| mosquitto | `mosquitto_sub -t $$SYS/# -C 1 -i healthcheck -W 3` |

---

## 5. Gap Analysis

### 5.1 Missing Services in Integration Compose

| Service | In Production | In Integration | Impact |
|---------|--------------|----------------|--------|
| `silver-etl` | Yes (profile: silver) | No | Cannot test `silver-etl` command |
| `silver-etl-daemon` | Yes (profile: silver-daemon) | No | Cannot test `silver-daemon*` commands |

**Note**: `duckdb` is NOT in production. The `analytics` and `rollback` commands are deprecated dead code.

### 5.2 Command Compatibility Matrix

| Command | Pi Mode | Integration Mode | Notes |
|---------|---------|------------------|-------|
| `deploy` | Works | Works | |
| `start` | Works | Works | |
| `stop` | Works | Works | |
| `logs` | Works | Works | |
| `status` | Works | Partial | Uses hardcoded `air-quality-app` container name |
| `build` | Works | Works | |
| `update` | Works | Partial | Git operations may conflict in dev |
| `refresh` | Works | Partial | Same git concerns |
| `sync` | Works | Works | Uses ETCD_CONTAINER |
| `init-streams` | Works | Works | Uses ETCD_CONTAINER |
| `list-streams` | Works | Works | Uses ETCD_CONTAINER |
| `sync-dictionary` | Works | Works | Uses dcx (service names) |
| `sync-dimensions` | Works | Works | Uses dcx (service names) |
| `list-dimensions` | Works | Works | Local file only |
| `dimension-status` | Works | Works | Local file only |
| `analytics` | N/A | N/A | ⚠️ DEPRECATED - duckdb not in production |
| `rollback` | N/A | N/A | ⚠️ DEPRECATED - dead code |
| `silver-migrate` | Works | Fails | Missing silver-etl service |
| `silver-etl` | Works | Fails | Missing silver-etl service |
| `silver-daemon` | Works | Fails | Missing silver-etl-daemon service |
| `silver-daemon-stop` | Works | Fails | Missing silver-etl-daemon service |
| `silver-daemon-logs` | Works | Fails | Missing container |
| `silver-daemon-status` | Works | Fails | Missing container |

### 5.3 Hardcoded Container Names

The following locations use hardcoded container names instead of the compose service names:

1. **status() function** (line 1158):
   ```bash
   docker exec air-quality-app du -sh /data 2>/dev/null
   ```
   - Should use: `dcx air-quality-app du -sh /data`

2. **refresh() function** (line 1274):
   ```bash
   docker restart grafana
   ```
   - Should use: `dc restart grafana`

3. **silver-daemon-logs** (line 1453):
   ```bash
   docker logs -f silver-etl-daemon
   ```
   - Should use: `dc logs -f silver-etl-daemon`

4. **silver-daemon-status** (lines 1457-1462):
   ```bash
   docker ps -q -f name=silver-etl-daemon
   docker exec silver-etl-daemon ps aux
   docker logs --tail 20 silver-etl-daemon
   ```
   - Needs profile-aware docker compose commands

### 5.4 Volume Name Differences

| Volume | Pi Compose | Integration Compose |
|--------|-----------|---------------------|
| Bronze data | `air-quality-data` | `bronze-data` |
| Mosquitto | `mosquitto-data`, `mosquitto-logs` | `mosquitto-data`, `mosquitto-logs` |
| etcd | `etcd-data` | `etcd-data` |
| TimescaleDB | `timescaledb-data` | `timescaledb-data` |
| Grafana | `grafana-data` | `grafana-data` |

### 5.5 Profile Support

Production compose defines profiles:
- `silver` - For one-shot Silver ETL
- `silver-daemon` - For continuous Silver ETL

Integration compose only defines:
- `dashboards` - For Grafana (optional)

---

## 6. Required Changes for Full Integration Support

### 6.1 docker-compose.integration.yml Additions

1. **Add silver-etl service** (profile: silver) - if silver-* commands need testing
2. **Add silver-etl-daemon service** (profile: silver-daemon) - if silver-daemon-* commands need testing
3. **Rename volume** - `bronze-data` → `air-quality-data` to match production

**NOT required**: `duckdb` service - analytics/rollback commands are deprecated

### 6.2 deploy.sh Fixes

1. **status() function** - Replace `docker exec air-quality-app` with `dcx air-quality-app`
2. **refresh() function** - Replace `docker restart grafana` with `dc restart grafana`
3. **silver-daemon-logs** - Use `dc --profile silver-daemon logs -f silver-etl-daemon`
4. **silver-daemon-status** - Use `dc --profile silver-daemon ps` and compose-aware commands

### 6.3 Test Data Strategy

Integration mode should support:
1. **MQTT injection** for Bronze layer testing:
   ```bash
   mosquitto_pub -h localhost -t "airgradient/test/measures" \
     -m '{"wifi":-50,"pm02":15,"rco2":650,"atmp":22.5,"rhum":55}'
   ```
2. **Synthetic time-series data** for Silver layer testing
3. **Pre-populated Parquet files** for ETL testing

---

## 7. Environment Variables

### 7.1 Core Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DEPLOY_ENV` | `pi` | Environment: `pi` or `integration` |
| `SILVER_ETL_INTERVAL` | `300` | Daemon ETL interval in seconds |
| `SILVER_ETL_PERSISTENCE` | `false` | Enable daemon run stats persistence |

### 7.2 Production-Only Variables (from .env)

| Variable | Description |
|----------|-------------|
| `POSTGRES_PASSWORD` | TimescaleDB password |
| `GRAFANA_ADMIN_PASSWORD` | Grafana admin password |
| `OPENWEATHERMAP_API_KEY` | Weather API key |
| `WEATHER_LATITUDE` | Location latitude |
| `WEATHER_LONGITUDE` | Location longitude |
| `EVENT_NOTIFIER_ENABLED` | Enable MQTT event notifications |
| `THRESHOLD_PROCESSOR_ENABLED` | Enable threshold alerts |

---

## 8. Recommendations

### 8.1 High Priority

1. Add `silver-etl` and `silver-etl-daemon` services to `docker-compose.integration.yml`
2. Fix hardcoded container names in `deploy.sh`
3. Create integration test script that validates all commands

### 8.2 Medium Priority

1. Rename volume `bronze-data` → `air-quality-data` to match production
2. Add health check to Grafana in integration compose
3. Consider removing deprecated `analytics`/`rollback` commands from deploy.sh

### 8.3 Low Priority

1. Add test data injection helpers to deploy.sh
2. Create automated integration test suite
3. Add CI/CD pipeline for integration testing

---

## 9. File References

| File | Path |
|------|------|
| Main deployment script | `/workspaces/neural-data-platform/deploy/pi/deploy.sh` |
| Production compose | `/workspaces/neural-data-platform/deploy/pi/docker-compose.yml` |
| Integration compose | `/workspaces/neural-data-platform/docker-compose.integration.yml` |
| Config sync script | `/workspaces/neural-data-platform/scripts/sync-config-to-etcd.sh` |
| Stream init script | `/workspaces/neural-data-platform/deploy/pi/configs/streams/init-streams.sh` |
| Stream list script | `/workspaces/neural-data-platform/deploy/pi/configs/streams/list-streams.sh` |
