# DP-001 Implementation Guide

## Overview
This document specifies the implementation artifacts required for the DuckDB Analytics + Grafana Dashboards feature. This is a **specification document** that describes what needs to be implemented, not the actual implementation files.

## 1. Docker Compose Changes

### Required Service Additions

**File**: `deploy/pi/docker-compose.yml`

Add two new services to the existing compose file:
- DuckDB service for SQL analytics layer
- Grafana service for visualization dashboards

### Service: DuckDB

**Image Recommendation**: `ghcr.io/duckdb/duckdb:latest` or `arm64v8/alpine` with DuckDB binary

**Port Configuration**:
- Internal port: 3000 (DuckDB server mode)
- No external exposure required (Grafana connects internally)

**Volume Mounts**:
```yaml
volumes:
  - ./config/duckdb/init.sql:/docker-entrypoint-initdb.d/init.sql:ro
  - ./config/duckdb/views:/opt/duckdb/views:ro
  - /var/neural-data/bronze:/data/bronze:ro  # Bronze layer Parquet files
  - duckdb_data:/var/lib/duckdb  # Persistent DuckDB state
```

**Memory Limits**:
- `memory: 512M` (Pi 5 constraint)
- `memory_reservation: 256M`

**Environment Variables**:
```yaml
environment:
  - DUCKDB_MAX_MEMORY=256MB
  - DUCKDB_THREADS=2
```

**Health Check Requirements**:
```yaml
healthcheck:
  test: ["CMD", "duckdb", "-c", "SELECT 1"]
  interval: 30s
  timeout: 10s
  retries: 3
  start_period: 10s
```

**Command**:
```yaml
command: duckdb -readonly -init /docker-entrypoint-initdb.d/init.sql
```

### Service: Grafana

**Image Recommendation**: `grafana/grafana:latest` (supports ARM64)

**Port Configuration**:
- Internal: 3000
- External: 3000 (accessible via browser)

**Volume Mounts**:
```yaml
volumes:
  - ./config/grafana/grafana.ini:/etc/grafana/grafana.ini:ro
  - ./config/grafana/provisioning:/etc/grafana/provisioning:ro
  - grafana_data:/var/lib/grafana
```

**Environment Variables**:
```yaml
environment:
  - GF_SERVER_ROOT_URL=http://localhost:3000
  - GF_SECURITY_ADMIN_USER=admin
  - GF_SECURITY_ADMIN_PASSWORD=${GRAFANA_PASSWORD:-admin}
  - GF_AUTH_ANONYMOUS_ENABLED=false
  - GF_INSTALL_PLUGINS=frser-sqlite-datasource
```

**Health Check Requirements**:
```yaml
healthcheck:
  test: ["CMD", "wget", "--no-verbose", "--tries=1", "--spider", "http://localhost:3000/api/health"]
  interval: 30s
  timeout: 10s
  retries: 3
  start_period: 30s
```

**Dependencies**:
```yaml
depends_on:
  duckdb:
    condition: service_healthy
```

### Network Configuration

All services should use the existing `neural-net` network:
```yaml
networks:
  - neural-net
```

### Volume Definitions

Add to the volumes section:
```yaml
volumes:
  duckdb_data:
    driver: local
  grafana_data:
    driver: local
```

---

## 2. DuckDB Configuration Files

### Required Files

| File | Purpose | Read-Only |
|------|---------|-----------|
| `config/duckdb/init.sql` | Bootstrap script to load Parquet and create views | Yes |
| `config/duckdb/views/silver_indoor_air.sql` | Virtual Silver layer for indoor air quality | Yes |
| `config/duckdb/views/silver_outdoor_weather.sql` | Virtual Silver layer for outdoor weather | Yes |
| `config/duckdb/views/silver_outdoor_air.sql` | Virtual Silver layer for outdoor air quality | Yes |
| `config/duckdb/views/cross_stream_aligned.sql` | Time-aligned join across all streams | Yes |

### init.sql Requirements

**Purpose**: Bootstrap DuckDB on container startup

**Required Actions**:
1. Load Parquet file extension
2. Set memory and performance limits
3. Create read-only views from Bronze layer
4. Source individual view SQL files
5. Verify all views are queryable

**Specification**:
```sql
-- Load Parquet extension
INSTALL parquet;
LOAD parquet;

-- Set memory limits for Pi 5
SET memory_limit='256MB';
SET threads=2;
SET max_memory='256MB';

-- Load view definitions
.read /opt/duckdb/views/silver_indoor_air.sql
.read /opt/duckdb/views/silver_outdoor_weather.sql
.read /opt/duckdb/views/silver_outdoor_air.sql
.read /opt/duckdb/views/cross_stream_aligned.sql

-- Verify views
SELECT 'Views loaded successfully' AS status;
```

### View SQL Requirements

#### silver_indoor_air.sql

**Purpose**: Virtual Silver layer over Bronze Parquet files for indoor air quality

**Data Source**: `/data/bronze/indoor-air-quality/*.parquet`

**View Specification**:
- **Name**: `silver_indoor_air`
- **Columns**:
  - `timestamp` (TIMESTAMP) - parsed from unix_timestamp_ms
  - `stream_id` (VARCHAR) - 'indoor-air-quality'
  - `temperature` (DOUBLE) - °C
  - `humidity` (DOUBLE) - %
  - `co2` (DOUBLE) - ppm
  - `tvoc` (DOUBLE) - ppb
  - `pm25` (DOUBLE) - µg/m³
  - `pm10` (DOUBLE) - µg/m³
- **Transformations**:
  - Convert unix_timestamp_ms to human-readable timestamp
  - Filter out null/invalid readings
  - Order by timestamp DESC

#### silver_outdoor_weather.sql

**Purpose**: Virtual Silver layer for outdoor weather conditions

**Data Source**: `/data/bronze/outdoor-weather/*.parquet`

**View Specification**:
- **Name**: `silver_outdoor_weather`
- **Columns**:
  - `timestamp` (TIMESTAMP)
  - `stream_id` (VARCHAR) - 'outdoor-weather'
  - `temperature` (DOUBLE) - °C
  - `humidity` (DOUBLE) - %
  - `pressure` (DOUBLE) - hPa
  - `wind_speed` (DOUBLE) - m/s
  - `wind_direction` (DOUBLE) - degrees
  - `precipitation` (DOUBLE) - mm
  - `uv_index` (DOUBLE)
- **Transformations**:
  - Parse Open-Meteo JSON fields
  - Convert timestamp
  - Filter invalid data

#### silver_outdoor_air.sql

**Purpose**: Virtual Silver layer for outdoor air quality

**Data Source**: `/data/bronze/outdoor-air-quality/*.parquet`

**View Specification**:
- **Name**: `silver_outdoor_air`
- **Columns**:
  - `timestamp` (TIMESTAMP)
  - `stream_id` (VARCHAR) - 'outdoor-air-quality'
  - `pm10` (DOUBLE) - µg/m³
  - `pm25` (DOUBLE) - µg/m³
  - `co` (DOUBLE) - µg/m³
  - `no2` (DOUBLE) - µg/m³
  - `so2` (DOUBLE) - µg/m³
  - `o3` (DOUBLE) - µg/m³
  - `aqi` (INTEGER) - Air Quality Index (1-5)
- **Transformations**:
  - Parse Open-Meteo air quality JSON
  - Calculate AQI from components
  - Filter invalid readings

#### cross_stream_aligned.sql

**Purpose**: Time-aligned join across all three streams for correlation analysis

**View Specification**:
- **Name**: `cross_stream_aligned`
- **Columns**:
  - `timestamp_bucket` (TIMESTAMP) - 5-minute time bucket
  - `indoor_temp`, `indoor_humidity`, `indoor_co2`, `indoor_pm25`
  - `outdoor_temp`, `outdoor_humidity`, `outdoor_pressure`
  - `outdoor_pm25`, `outdoor_aqi`
- **Transformations**:
  - Bucket timestamps to 5-minute intervals
  - LEFT JOIN all three silver views on time bucket
  - Aggregate multiple readings per bucket (AVG)
  - Handle missing data gracefully

---

## 3. Grafana Configuration Files

### Required Files

| File | Purpose |
|------|---------|
| `config/grafana/grafana.ini` | Server and security configuration |
| `config/grafana/provisioning/datasources/duckdb.yaml` | DuckDB datasource definition |
| `config/grafana/provisioning/dashboards/default.yaml` | Dashboard auto-discovery |
| `config/grafana/dashboards/indoor-air-quality.json` | Indoor air dashboard |
| `config/grafana/dashboards/outdoor-conditions.json` | Outdoor weather dashboard |
| `config/grafana/dashboards/outdoor-air-quality.json` | Outdoor AQI dashboard |
| `config/grafana/dashboards/indoor-vs-outdoor.json` | Cross-stream comparison |

### grafana.ini Requirements

**Purpose**: Server configuration for Pi deployment

**Required Sections**:

```ini
[server]
protocol = http
http_port = 3000
domain = localhost
root_url = %(protocol)s://%(domain)s:%(http_port)s/

[security]
admin_user = admin
admin_password = ${GRAFANA_PASSWORD}
disable_gravatar = true

[analytics]
reporting_enabled = false
check_for_updates = false

[log]
mode = console
level = info

[paths]
data = /var/lib/grafana
logs = /var/log/grafana
plugins = /var/lib/grafana/plugins
provisioning = /etc/grafana/provisioning
```

### DuckDB Datasource YAML Requirements

**File**: `config/grafana/provisioning/datasources/duckdb.yaml`

**Purpose**: Auto-provision DuckDB as a datasource

**Specification**:
```yaml
apiVersion: 1

datasources:
  - name: DuckDB Silver Layer
    type: frser-sqlite-datasource  # SQLite plugin works with DuckDB
    access: proxy
    url: duckdb:3000
    isDefault: true
    editable: false
    jsonData:
      path: /var/lib/duckdb/neural.db
```

**Note**: If SQLite plugin doesn't work, use HTTP API with custom plugin.

### Dashboard Discovery YAML Requirements

**File**: `config/grafana/provisioning/dashboards/default.yaml`

**Purpose**: Auto-load dashboards from JSON files

**Specification**:
```yaml
apiVersion: 1

providers:
  - name: 'NDP Dashboards'
    orgId: 1
    folder: 'Neural Data Platform'
    type: file
    disableDeletion: false
    updateIntervalSeconds: 10
    allowUiUpdates: true
    options:
      path: /etc/grafana/dashboards
      foldersFromFilesStructure: true
```

### Dashboard JSON Requirements

#### indoor-air-quality.json

**Purpose**: Real-time indoor air quality monitoring

**Panels Required**:
1. **Temperature & Humidity Time Series**
   - Query: `SELECT timestamp, temperature, humidity FROM silver_indoor_air ORDER BY timestamp DESC LIMIT 1000`
   - Panel type: Time series
   - Y-axis: Dual (°C, %)

2. **CO2 Levels**
   - Query: `SELECT timestamp, co2 FROM silver_indoor_air ORDER BY timestamp DESC LIMIT 1000`
   - Panel type: Time series
   - Threshold: 1000 ppm (warning), 2000 ppm (critical)

3. **PM2.5 & PM10**
   - Query: `SELECT timestamp, pm25, pm10 FROM silver_indoor_air ORDER BY timestamp DESC LIMIT 1000`
   - Panel type: Time series
   - Threshold: WHO guidelines

4. **Current Readings Stat Panel**
   - Query: `SELECT temperature, humidity, co2, pm25 FROM silver_indoor_air ORDER BY timestamp DESC LIMIT 1`
   - Panel type: Stat (4 separate panels)

**Time Range**: Last 24 hours, auto-refresh 30s

#### outdoor-conditions.json

**Purpose**: Outdoor weather monitoring

**Panels Required**:
1. **Temperature & Humidity**
   - Query: `SELECT timestamp, temperature, humidity FROM silver_outdoor_weather ORDER BY timestamp DESC LIMIT 1000`
   - Panel type: Time series

2. **Pressure**
   - Query: `SELECT timestamp, pressure FROM silver_outdoor_weather ORDER BY timestamp DESC LIMIT 1000`
   - Panel type: Time series

3. **Wind Speed & Direction**
   - Query: `SELECT timestamp, wind_speed, wind_direction FROM silver_outdoor_weather ORDER BY timestamp DESC LIMIT 1000`
   - Panel type: Time series + Wind rose

4. **UV Index**
   - Query: `SELECT timestamp, uv_index FROM silver_outdoor_weather ORDER BY timestamp DESC LIMIT 1000`
   - Panel type: Gauge

**Time Range**: Last 24 hours, auto-refresh 5 minutes

#### outdoor-air-quality.json

**Purpose**: Outdoor air quality monitoring

**Panels Required**:
1. **AQI Over Time**
   - Query: `SELECT timestamp, aqi FROM silver_outdoor_air ORDER BY timestamp DESC LIMIT 1000`
   - Panel type: Time series
   - Threshold: AQI color bands (1=green, 5=red)

2. **Pollutant Breakdown**
   - Query: `SELECT timestamp, pm25, pm10, no2, o3 FROM silver_outdoor_air ORDER BY timestamp DESC LIMIT 1000`
   - Panel type: Time series (multi-line)

3. **Current AQI Stat**
   - Query: `SELECT aqi FROM silver_outdoor_air ORDER BY timestamp DESC LIMIT 1`
   - Panel type: Gauge with color thresholds

**Time Range**: Last 24 hours, auto-refresh 1 hour

#### indoor-vs-outdoor.json

**Purpose**: Cross-stream correlation analysis

**Panels Required**:
1. **Temperature Comparison**
   - Query: `SELECT timestamp_bucket, indoor_temp, outdoor_temp FROM cross_stream_aligned ORDER BY timestamp_bucket DESC LIMIT 288`
   - Panel type: Time series (dual line)

2. **Humidity Comparison**
   - Query: `SELECT timestamp_bucket, indoor_humidity, outdoor_humidity FROM cross_stream_aligned ORDER BY timestamp_bucket DESC LIMIT 288`
   - Panel type: Time series

3. **PM2.5 Indoor vs Outdoor**
   - Query: `SELECT timestamp_bucket, indoor_pm25, outdoor_pm25 FROM cross_stream_aligned ORDER BY timestamp_bucket DESC LIMIT 288`
   - Panel type: Time series

4. **CO2 vs Outdoor AQI Scatter**
   - Query: `SELECT indoor_co2, outdoor_aqi FROM cross_stream_aligned WHERE indoor_co2 IS NOT NULL AND outdoor_aqi IS NOT NULL LIMIT 500`
   - Panel type: Scatter plot

**Time Range**: Last 24 hours (288 5-minute buckets), auto-refresh 5 minutes

---

## 4. Deployment Script Updates

### deploy.sh Changes

**File**: `deploy/pi/deploy.sh`

**Required Modifications**:

#### 1. Add DuckDB/Grafana to Start Sequence

```bash
start_services() {
  echo "Starting Neural Data Platform services..."

  # Existing services
  docker-compose up -d mosquitto etcd
  wait_for_health "mosquitto" 30
  wait_for_health "etcd" 30

  # NEW: Start DuckDB
  docker-compose up -d duckdb
  wait_for_health "duckdb" 60

  # NEW: Start Grafana
  docker-compose up -d grafana
  wait_for_health "grafana" 60

  # Start application
  docker-compose up -d air-quality-app

  echo "✅ All services started"
}
```

#### 2. Add Health Check Verification

```bash
wait_for_health() {
  local service=$1
  local timeout=$2
  local elapsed=0

  echo "Waiting for $service to be healthy..."

  while [ $elapsed -lt $timeout ]; do
    if docker-compose ps $service | grep -q "healthy"; then
      echo "✅ $service is healthy"
      return 0
    fi
    sleep 2
    elapsed=$((elapsed + 2))
  done

  echo "❌ $service failed to become healthy within ${timeout}s"
  return 1
}
```

#### 3. Add Rollback Capability

```bash
rollback() {
  echo "Rolling back to previous version..."

  # Stop new services
  docker-compose down duckdb grafana

  # Remove new volumes (optional)
  read -p "Delete DuckDB and Grafana data? (y/N): " -n 1 -r
  if [[ $REPLY =~ ^[Yy]$ ]]; then
    docker volume rm pi_duckdb_data pi_grafana_data
  fi

  echo "✅ Rollback complete"
}
```

#### 4. Add Status Command

```bash
status() {
  echo "Neural Data Platform Status:"
  docker-compose ps

  echo ""
  echo "Service Health:"
  for service in mosquitto etcd duckdb grafana air-quality-app; do
    health=$(docker-compose ps $service | grep -oP '(?<=\().*(?=\))' || echo "unknown")
    echo "  $service: $health"
  done

  echo ""
  echo "Grafana URL: http://$(hostname -I | awk '{print $1}'):3000"
}
```

#### 5. Update Help Text

```bash
usage() {
  echo "Usage: $0 {start|stop|restart|status|logs|sync|rollback}"
  echo ""
  echo "Commands:"
  echo "  start     - Start all services (mosquitto, etcd, duckdb, grafana, app)"
  echo "  stop      - Stop all services"
  echo "  restart   - Restart all services"
  echo "  status    - Show service health and URLs"
  echo "  logs      - Follow logs from all services"
  echo "  sync      - Sync stream configs to etcd"
  echo "  rollback  - Remove DuckDB/Grafana and restore previous state"
}
```

---

## 5. Implementation Checklist

### Docker Infrastructure
- [ ] Docker Compose updated with DuckDB service
- [ ] Docker Compose updated with Grafana service
- [ ] Volume definitions added
- [ ] Health checks configured
- [ ] Memory limits set appropriately

### DuckDB Configuration
- [ ] `config/duckdb/init.sql` created
- [ ] `config/duckdb/views/silver_indoor_air.sql` created
- [ ] `config/duckdb/views/silver_outdoor_weather.sql` created
- [ ] `config/duckdb/views/silver_outdoor_air.sql` created
- [ ] `config/duckdb/views/cross_stream_aligned.sql` created
- [ ] All views query Bronze Parquet files correctly

### Grafana Configuration
- [ ] `config/grafana/grafana.ini` created
- [ ] `config/grafana/provisioning/datasources/duckdb.yaml` created
- [ ] `config/grafana/provisioning/dashboards/default.yaml` created
- [ ] Dashboard JSON files created (4 total)
- [ ] All panels have correct SQL queries
- [ ] Thresholds and alerts configured

### Deployment Scripts
- [ ] `deploy.sh` updated with DuckDB/Grafana start sequence
- [ ] Health check functions implemented
- [ ] Rollback command added
- [ ] Status command enhanced
- [ ] Help text updated

### Testing & Verification
- [ ] All services start in correct order
- [ ] Health checks pass for DuckDB
- [ ] Health checks pass for Grafana
- [ ] Grafana UI accessible at :3000
- [ ] DuckDB views return data
- [ ] All dashboards load successfully
- [ ] Data refreshes correctly (auto-refresh working)
- [ ] Memory usage under 512MB for each service

### Documentation
- [ ] IMPLEMENTATION_GUIDE.md complete (this document)
- [ ] STATUS.md updated with Refinement phase completion
- [ ] ADR created if architectural decisions made

---

## 6. Implementation Notes

### Read-Only Philosophy
DuckDB views are **read-only** over Bronze Parquet files. This ensures:
- Bronze layer remains immutable
- No write contention
- Simple deployment (no ETL processes)
- Fast queries with DuckDB's Parquet optimization

### Memory Constraints
Pi 5 has limited memory. Each service must:
- Set explicit memory limits in docker-compose.yml
- Configure internal limits (DuckDB memory_limit)
- Monitor with `docker stats`

### Plugin Compatibility
Grafana's DuckDB support may require:
- Custom plugin installation
- SQLite plugin as workaround
- HTTP API with JSON datasource

Test plugin availability during implementation.

### Security
- Grafana admin password must use environment variable
- No hardcoded credentials
- DuckDB should run in read-only mode
- Network isolation via docker network

---

## 7. Next Steps

After this specification is approved:

1. **Implementation Phase** (SPARC Refinement)
   - Create all specified files
   - Test locally with mock data
   - Verify health checks

2. **Testing Phase** (SPARC Completion)
   - Deploy to Pi 5
   - Verify Bronze data loads
   - Confirm dashboards display correctly
   - Performance testing

3. **Documentation Phase**
   - Update STATUS.md to Completion
   - Create user guide for dashboards
   - Document troubleshooting steps

---

**Document Status**: SPECIFICATION (Ready for Implementation)
**Last Updated**: 2025-12-18
**Owner**: ndp-architect
