# Docker Compose Changes for DP-001

**Feature**: DP-001 - Silver Layer Foundation
**Date**: 2025-12-18
**Author**: ndp-architect

## Summary

Added DuckDB and Grafana services to the Neural Data Platform stack to enable analytical querying and visualization of air quality data.

## Changes Made

### 1. DuckDB Service

```yaml
duckdb:
  image: marcboeker/duckdb-http:latest
  container_name: duckdb
  volumes:
    - air-quality-data:/data:ro
    - duckdb-data:/duckdb
  environment:
    - DUCKDB_HTTP_PORT=9090
    - DUCKDB_DATABASE=/duckdb/neural_platform.db
  healthcheck:
    test: ["CMD", "wget", "--spider", "-q", "http://localhost:9090/health"]
    interval: 30s
    timeout: 10s
    retries: 3
    start_period: 20s
  deploy:
    resources:
      limits:
        memory: 512M
  depends_on:
    air-quality-app:
      condition: service_healthy
```

**Why This Image**:
- `marcboeker/duckdb-http` provides HTTP API for remote querying
- Enables Grafana to connect via HTTP data source
- Lightweight and optimized for analytical workloads
- Active maintenance and ARM64 support for Raspberry Pi

**Alternative Considered**: Building custom DuckDB container with Python/HTTP wrapper was rejected due to increased complexity and maintenance burden.

### 2. Grafana Service

```yaml
grafana:
  image: grafana/grafana-oss:latest
  container_name: grafana
  ports:
    - "3000:3000"
  volumes:
    - grafana-data:/var/lib/grafana
  environment:
    - GF_SECURITY_ADMIN_PASSWORD=${GRAFANA_ADMIN_PASSWORD:-admin}
    - GF_USERS_ALLOW_SIGN_UP=false
    - GF_DATABASE_TYPE=sqlite3
    - GF_DATABASE_PATH=/var/lib/grafana/grafana.db
    - GF_SERVER_ROOT_URL=http://localhost:3000
    - GF_LOG_LEVEL=info
  healthcheck:
    test: ["CMD", "wget", "--spider", "-q", "http://localhost:3000/api/health"]
    interval: 30s
    timeout: 10s
    retries: 3
    start_period: 30s
  deploy:
    resources:
      limits:
        memory: 256M
  depends_on:
    duckdb:
      condition: service_healthy
```

**Why This Image**:
- Official Grafana OSS (Open Source) edition
- Enterprise features not needed for this use case
- Proven stability and ARM64 support
- Built-in SQLite for dashboards and configurations
- Active community and plugin ecosystem

**Alternative Considered**: Grafana Enterprise was rejected due to licensing costs and unnecessary features.

### 3. New Volumes

```yaml
volumes:
  duckdb-data:
    driver: local
  grafana-data:
    driver: local
```

## Volume Mount Strategy

### Read-Only Bronze Layer Access

```yaml
volumes:
  - air-quality-data:/data:ro  # DuckDB gets read-only access
```

**Rationale**:
- **Data Integrity**: Prevents accidental modification of Bronze layer Parquet files
- **Separation of Concerns**: DuckDB queries but does not write to Bronze
- **Safety**: ETL processes can safely assume Bronze is append-only
- **Performance**: Read-only mounts have lower overhead

### Dedicated DuckDB Storage

```yaml
volumes:
  - duckdb-data:/duckdb  # Persistent DuckDB database
```

**Rationale**:
- **Persistence**: Stores DuckDB metadata, indexes, and cached query results
- **Performance**: Local database for aggregation results
- **Isolation**: Separate from Bronze and Grafana data
- **Backup**: Can be backed up independently

### Grafana Data Volume

```yaml
volumes:
  - grafana-data:/var/lib/grafana  # Dashboards, users, data sources
```

**Rationale**:
- **Configuration Persistence**: Dashboards survive container restarts
- **User Management**: Admin password and user settings stored
- **Data Sources**: DuckDB connection configuration persisted
- **Plugins**: Custom plugins and settings retained

## Health Check Approach

### DuckDB Health Check

```yaml
healthcheck:
  test: ["CMD", "wget", "--spider", "-q", "http://localhost:9090/health"]
  interval: 30s
  timeout: 10s
  retries: 3
  start_period: 20s
```

**Why This Works**:
- Uses `wget --spider` to check HTTP endpoint without downloading
- DuckDB HTTP server provides `/health` endpoint
- Minimal overhead (no database query required)
- Fast startup detection (20s start period)

### Grafana Health Check

```yaml
healthcheck:
  test: ["CMD", "wget", "--spider", "-q", "http://localhost:3000/api/health"]
  interval: 30s
  timeout: 10s
  retries: 3
  start_period: 30s
```

**Why This Works**:
- Official Grafana `/api/health` endpoint
- Returns JSON health status
- Checks database connectivity and plugin status
- 30s start period allows SQLite initialization

## Dependency Chain

```
air-quality-app (Bronze Layer)
       ↓
    duckdb (Silver Layer)
       ↓
    grafana (Visualization)
```

**Why This Order**:
1. **Bronze First**: Data must exist before querying
2. **Silver Second**: DuckDB must be ready before Grafana connects
3. **Grafana Last**: UI depends on data source availability

**Health Conditions**:
- DuckDB waits for `air-quality-app` to be `service_healthy`
- Grafana waits for `duckdb` to be `service_healthy`
- This ensures proper startup sequence without race conditions

## Memory Budget

| Service | Memory Limit | Rationale |
|---------|--------------|-----------|
| mosquitto | 128M | Lightweight MQTT broker |
| etcd | 256M | Configuration store with 512MB backend quota |
| air-quality-app | 512M | Rust app with Parquet writing |
| duckdb | 512M | Analytical queries and aggregations |
| grafana | 256M | Visualization and dashboards |
| **TOTAL** | **1664M** | Fits within Pi 5's 8GB RAM with headroom |

**Justification**:
- Pi 5 has 8GB RAM
- Total stack uses ~1.7GB
- Leaves ~6.3GB for OS, caching, and future services
- DuckDB gets 512MB for complex analytical queries
- Grafana needs 256MB for rendering and SQLite

## Network Configuration

Both services use the existing `neural-network` bridge network:

```yaml
networks:
  default:
    name: neural-network
    driver: bridge
```

**Why No Host Port for DuckDB**:
- DuckDB is internal-only (no external access needed)
- Only Grafana communicates with DuckDB via `http://duckdb:9090`
- Reduces attack surface
- Grafana uses container-to-container networking

**Why Expose Grafana Port 3000**:
- User needs web UI access
- Dashboard visualization requires browser access
- Secured via admin password
- Standard Grafana convention

## Rollback Instructions

### Quick Rollback

If the new services cause issues:

```bash
# Stop and remove new services
docker compose stop duckdb grafana
docker compose rm -f duckdb grafana

# Restore previous docker-compose.yml
git checkout HEAD~1 -- deploy/pi/docker-compose.yml

# Restart stack
docker compose up -d
```

### Data Preservation

Before rollback, backup new data:

```bash
# Backup DuckDB database
docker run --rm -v neural-data-platform_duckdb-data:/data \
  -v $(pwd)/backups:/backup \
  alpine tar czf /backup/duckdb-backup-$(date +%Y%m%d).tar.gz /data

# Backup Grafana dashboards
docker run --rm -v neural-data-platform_grafana-data:/data \
  -v $(pwd)/backups:/backup \
  alpine tar czf /backup/grafana-backup-$(date +%Y%m%d).tar.gz /data
```

### Clean Removal

To completely remove DP-001 additions:

```bash
# Stop stack
docker compose down

# Remove volumes
docker volume rm neural-data-platform_duckdb-data
docker volume rm neural-data-platform_grafana-data

# Remove images
docker rmi marcboeker/duckdb-http:latest
docker rmi grafana/grafana-oss:latest

# Restart original stack
git checkout HEAD~1 -- deploy/pi/docker-compose.yml
docker compose up -d
```

## Testing the Changes

### Verify Stack Health

```bash
# Start updated stack
docker compose up -d

# Check all services healthy
docker compose ps

# Expected output:
# NAME               IMAGE                              STATUS
# air-quality-app    neural-data-platform/air-quality   Up (healthy)
# duckdb            marcboeker/duckdb-http             Up (healthy)
# etcd              quay.io/coreos/etcd                Up (healthy)
# grafana           grafana/grafana-oss                Up (healthy)
# mqtt-broker       eclipse-mosquitto                  Up (healthy)
```

### Test DuckDB Connectivity

```bash
# Query Bronze layer via DuckDB HTTP API
curl -X POST http://localhost:9090/query \
  -H "Content-Type: application/json" \
  -d '{"query": "SELECT COUNT(*) FROM read_parquet('\''/data/**/*.parquet'\'')"}'
```

### Test Grafana Access

```bash
# Check Grafana web UI
curl -I http://localhost:3000

# Login credentials (default):
# Username: admin
# Password: admin (or value from GRAFANA_ADMIN_PASSWORD env var)
```

## Environment Variables

Add to `.env` file:

```bash
# Grafana Admin Password (REQUIRED - change from default!)
GRAFANA_ADMIN_PASSWORD=your-secure-password-here
```

## Security Considerations

1. **Change Default Password**: Set `GRAFANA_ADMIN_PASSWORD` before first run
2. **Disable Signup**: `GF_USERS_ALLOW_SIGN_UP=false` prevents unauthorized accounts
3. **No External DuckDB**: DuckDB is internal-only (no host port)
4. **Read-Only Bronze**: DuckDB cannot modify source Parquet files

## Next Steps

1. Configure Grafana data source to connect to DuckDB
2. Create initial dashboard for air quality metrics
3. Test query performance with real Parquet data
4. Document dashboard creation in `DASHBOARD_DEVELOPMENT.md`

## References

- DP-001 Feature Specification: `product/features/dp-001/SCOPE.md`
- Docker Integration Architecture: `product/features/dp-001/architecture/DOCKER_INTEGRATION.md`
- DuckDB HTTP API: https://github.com/marcboeker/duckdb-http
- Grafana Documentation: https://grafana.com/docs/grafana/latest/
