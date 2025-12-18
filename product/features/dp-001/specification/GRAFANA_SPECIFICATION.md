# Grafana Visualization Layer Specification

**Feature**: DP-001 - Silver Layer Foundation
**Component**: Grafana Visualization Layer
**Author**: NDP Grafana Developer
**Date**: 2025-12-18
**Status**: Draft

## Overview

This specification defines the Grafana OSS deployment for visualizing time-series data from the Neural Data Platform Silver Layer (DuckDB). The deployment targets Raspberry Pi 5 (ARM64) with minimal resource usage and GitOps-based provisioning.

## 1. Grafana Container Requirements

### 1.1 Docker Configuration

```yaml
# deploy/docker-compose.yml addition
services:
  grafana:
    image: grafana/grafana-oss:latest
    container_name: ndp-grafana
    restart: unless-stopped
    ports:
      - "3000:3000"
    environment:
      # Authentication
      GF_AUTH_DISABLE_LOGIN_FORM: "true"
      GF_AUTH_ANONYMOUS_ENABLED: "true"
      GF_AUTH_ANONYMOUS_ORG_ROLE: "Admin"

      # Security (home network only)
      GF_SECURITY_ALLOW_EMBEDDING: "true"
      GF_SECURITY_ADMIN_USER: "admin"
      GF_SECURITY_ADMIN_PASSWORD: "admin"

      # Server
      GF_SERVER_ROOT_URL: "http://localhost:3000"
      GF_SERVER_SERVE_FROM_SUB_PATH: "false"

      # Plugins
      GF_INSTALL_PLUGINS: "motherduck-duckdb-datasource"

      # Logging
      GF_LOG_LEVEL: "info"

      # Performance
      GF_DATABASE_WAL: "true"

    volumes:
      # Provisioning (read-only)
      - ./grafana/provisioning:/etc/grafana/provisioning:ro

      # Dashboard JSON files (read-only for provisioning)
      - ./grafana/dashboards:/var/lib/grafana/dashboards:ro

      # Data persistence (read-write for edits)
      - grafana-data:/var/lib/grafana

    depends_on:
      - duckdb

    deploy:
      resources:
        limits:
          memory: 256M
        reservations:
          memory: 128M

    networks:
      - ndp-network

volumes:
  grafana-data:
    driver: local
```

### 1.2 Resource Constraints

| Resource | Limit | Reservation | Rationale |
|----------|-------|-------------|-----------|
| Memory | 256MB | 128MB | Minimal footprint for Pi 5 |
| CPU | Unlimited | None | Bursty dashboard loads |
| Storage | 1GB | N/A | Dashboard configs + metadata |

### 1.3 Health Check

```yaml
    healthcheck:
      test: ["CMD-SHELL", "wget --no-verbose --tries=1 --spider http://localhost:3000/api/health || exit 1"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s
```

## 2. Datasource Configuration

### 2.1 DuckDB Datasource Provisioning

**File**: `/deploy/grafana/provisioning/datasources/duckdb.yaml`

```yaml
apiVersion: 1

datasources:
  - name: DuckDB
    type: motherduck-duckdb-datasource
    access: proxy
    uid: duckdb-ndp
    url: http://duckdb:8080
    isDefault: true
    editable: false
    version: 1
    jsonData:
      # Connection settings
      timeout: 30
      keepAlive: true
      maxOpenConns: 5
      maxIdleConns: 2
      connMaxLifetime: 14400  # 4 hours

      # Query settings
      defaultDatabase: ndp
      readOnly: true

      # Performance
      enableCache: true
      cacheTTL: 300  # 5 minutes
```

### 2.2 DuckDB Plugin Installation

The `motherduck-duckdb-datasource` plugin is installed via the `GF_INSTALL_PLUGINS` environment variable at container startup.

**Plugin Repository**: https://grafana.com/grafana/plugins/motherduck-duckdb-datasource/

**Version**: Latest compatible with Grafana OSS ARM64

### 2.3 Connection Verification

**Test Query** (to be executed via Grafana UI):

```sql
SELECT 'DuckDB Connected' as status, version() as version;
```

Expected result: Returns DuckDB version string.

## 3. Provisioning Structure

### 3.1 Directory Layout

```
deploy/grafana/
├── provisioning/
│   ├── datasources/
│   │   └── duckdb.yaml            # DuckDB datasource config
│   └── dashboards/
│       └── dashboard.yaml         # Dashboard provider config
└── dashboards/
    ├── indoor-air-quality.json    # Dashboard 1
    ├── outdoor-conditions.json    # Dashboard 2
    ├── outdoor-air-quality.json   # Dashboard 3
    └── comparison.json            # Dashboard 4
```

### 3.2 Dashboard Provider Configuration

**File**: `/deploy/grafana/provisioning/dashboards/dashboard.yaml`

```yaml
apiVersion: 1

providers:
  - name: NDP Dashboards
    orgId: 1
    folder: Neural Data Platform
    type: file
    disableDeletion: false
    updateIntervalSeconds: 10
    allowUiUpdates: true
    options:
      path: /var/lib/grafana/dashboards
      foldersFromFilesStructure: false
```

**Key Settings**:
- `disableDeletion: false` - Allow dashboard deletion via UI
- `allowUiUpdates: true` - Allow dashboard edits to persist
- `updateIntervalSeconds: 10` - Reload provisioned dashboards every 10s

### 3.3 GitOps Workflow

1. **Development**: Edit dashboards via Grafana UI
2. **Export**: Use Grafana API or UI to export JSON
3. **Commit**: Save JSON to `deploy/grafana/dashboards/`
4. **Deploy**: Container restart picks up changes automatically

## 4. Dashboard Specifications

### 4.1 Dashboard 1: Indoor Air Quality

**UID**: `ndp-indoor-air-quality`
**Title**: Indoor Air Quality Monitoring
**Tags**: `ndp`, `indoor`, `air-quality`

#### Panels

| Panel | Type | Query | Visualization | Thresholds |
|-------|------|-------|---------------|-----------|
| PM2.5 Trend | Time Series | Hourly PM2.5 from `readings_hourly` | Line graph | Green: <12, Yellow: 12-35, Orange: 35-55, Red: >55 µg/m³ |
| Current PM2.5 | Stat | Latest PM2.5 from `readings_hourly` | Large number with trend arrow | Same as above |
| CO2 Levels | Time Series | Hourly CO2 from `readings_hourly` | Line graph | Green: <1000, Yellow: 1000-2000, Red: >2000 ppm |
| Temperature | Time Series | Hourly temp from `readings_hourly` | Line graph with min/max band | None |
| Humidity | Time Series | Hourly humidity from `readings_hourly` | Line graph | None |
| VOC Index | Time Series | Hourly VOC from `readings_hourly` | Line graph | Green: <100, Yellow: 100-300, Red: >300 |

#### Example Query (PM2.5 Trend)

```sql
SELECT
    bucket as time,
    avg_pm25 as "PM2.5 (µg/m³)"
FROM readings_hourly
WHERE
    stream_id = 'air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

#### Layout

```
┌─────────────────────────────────────────────────────────┐
│ Indoor Air Quality Monitoring                           │
├──────────────┬──────────────┬──────────────────────────┤
│ Current PM2.5│ Current CO2  │ Current Temperature      │
│ (Stat)       │ (Stat)       │ (Stat)                   │
├──────────────┴──────────────┴──────────────────────────┤
│ PM2.5 Trend (Time Series - 7 days)                     │
├─────────────────────────────────────────────────────────┤
│ CO2 Levels (Time Series - 7 days)                      │
├─────────────────────────────────────────────────────────┤
│ Temperature & Humidity (Time Series - 7 days)          │
├─────────────────────────────────────────────────────────┤
│ VOC Index (Time Series - 7 days)                       │
└─────────────────────────────────────────────────────────┘
```

#### Settings

- **Time Range**: Last 7 days (default)
- **Refresh**: 5 minutes
- **Timezone**: Browser
- **Time Picker**: Enabled
- **Variables**: `stream_id` (for future multi-location support)

### 4.2 Dashboard 2: Outdoor Conditions

**UID**: `ndp-outdoor-conditions`
**Title**: Outdoor Weather Conditions
**Tags**: `ndp`, `outdoor`, `weather`

#### Panels

| Panel | Type | Query | Visualization | Thresholds |
|-------|------|-------|---------------|-----------|
| Temperature | Time Series | Hourly temp from `readings_hourly` | Line graph | None |
| Feels Like | Time Series | Hourly apparent temp | Line graph (overlay) | None |
| Wind Speed | Time Series | Hourly wind speed | Line graph | Green: <20, Yellow: 20-40, Red: >40 km/h |
| Wind Direction | Time Series | Hourly wind direction | Compass/rose (or degree text) | None |
| Pressure | Time Series | Hourly pressure | Line graph | None |
| Cloud Cover | Time Series | Hourly cloud cover % | Area graph | None |
| Precipitation | Time Series | Hourly precip probability | Bar graph | None |
| UV Index | Time Series | Hourly UV index | Line graph | Green: 0-2, Yellow: 3-5, Orange: 6-7, Red: 8-10, Purple: 11+ |

#### Example Query (Temperature)

```sql
SELECT
    bucket as time,
    avg_temperature as "Temperature (°C)",
    avg_apparent_temperature as "Feels Like (°C)"
FROM readings_hourly
WHERE
    stream_id = 'outdoor-conditions'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

#### Layout

```
┌─────────────────────────────────────────────────────────┐
│ Outdoor Weather Conditions                              │
├──────────────┬──────────────┬──────────────────────────┤
│ Current Temp │ Wind Speed   │ Pressure                 │
│ (Stat)       │ (Stat)       │ (Stat)                   │
├──────────────┴──────────────┴──────────────────────────┤
│ Temperature & Feels Like (Time Series - 7 days)        │
├─────────────────────────────────────────────────────────┤
│ Wind Speed & Direction (Time Series - 7 days)          │
├─────────────────────────────────────────────────────────┤
│ Pressure (Time Series - 7 days)                        │
├─────────────────────────────────────────────────────────┤
│ Cloud Cover & Precipitation (Time Series - 7 days)     │
├─────────────────────────────────────────────────────────┤
│ UV Index (Time Series - 7 days)                        │
└─────────────────────────────────────────────────────────┘
```

#### Settings

- **Time Range**: Last 7 days (default)
- **Refresh**: 5 minutes
- **Timezone**: Browser
- **Time Picker**: Enabled
- **Variables**: None (single location)

### 4.3 Dashboard 3: Outdoor Air Quality

**UID**: `ndp-outdoor-air-quality`
**Title**: Outdoor Air Quality Monitoring
**Tags**: `ndp`, `outdoor`, `air-quality`

#### Panels

| Panel | Type | Query | Visualization | Thresholds |
|-------|------|-------|---------------|-----------|
| Current AQI | Gauge | Latest AQI from `readings_hourly` | Gauge (0-500 scale) | Green: 0-50, Yellow: 51-100, Orange: 101-150, Red: 151-200, Purple: 201-300, Maroon: 301+ |
| AQI Trend | Time Series | Hourly AQI | Line graph with color bands | Same as above |
| PM2.5 Comparison | Time Series | Indoor vs Outdoor PM2.5 | Dual-axis line graph | Indoor: SGV-30 sensor, Outdoor: OpenMeteo |
| PM10 Levels | Time Series | Hourly PM10 | Line graph | Green: 0-50, Yellow: 51-100, Red: >100 µg/m³ |
| Pollutant Breakdown | Bar Chart | Latest values for all pollutants | Horizontal bars | Per-pollutant thresholds |
| Ozone | Time Series | Hourly O3 | Line graph | Green: <100, Yellow: 100-200, Red: >200 µg/m³ |
| Nitrogen Dioxide | Time Series | Hourly NO2 | Line graph | Green: <40, Yellow: 40-200, Red: >200 µg/m³ |

#### Example Query (AQI Trend)

```sql
SELECT
    bucket as time,
    avg_us_aqi as "US AQI",
    avg_european_aqi as "European AQI"
FROM readings_hourly
WHERE
    stream_id = 'outdoor-air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

#### Example Query (PM2.5 Comparison)

```sql
-- Indoor PM2.5
SELECT
    bucket as time,
    avg_pm25 as "Indoor PM2.5"
FROM readings_hourly
WHERE
    stream_id = 'air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()

UNION ALL

-- Outdoor PM2.5
SELECT
    bucket as time,
    avg_pm2_5 as "Outdoor PM2.5"
FROM readings_hourly
WHERE
    stream_id = 'outdoor-air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY time
```

#### Layout

```
┌─────────────────────────────────────────────────────────┐
│ Outdoor Air Quality Monitoring                          │
├──────────────┬──────────────────────────────────────────┤
│ Current AQI  │ AQI Trend (Time Series - 7 days)        │
│ (Gauge)      │                                          │
├──────────────┴──────────────────────────────────────────┤
│ PM2.5 Indoor vs Outdoor Comparison (Time Series)       │
├─────────────────────────────────────────────────────────┤
│ PM10 Levels (Time Series - 7 days)                     │
├─────────────────────────────────────────────────────────┤
│ Ozone & Nitrogen Dioxide (Time Series - 7 days)        │
├─────────────────────────────────────────────────────────┤
│ Pollutant Breakdown (Current Values - Bar Chart)       │
└─────────────────────────────────────────────────────────┘
```

#### AQI Color Coding

| AQI Range | Color | Category |
|-----------|-------|----------|
| 0-50 | Green | Good |
| 51-100 | Yellow | Moderate |
| 101-150 | Orange | Unhealthy for Sensitive Groups |
| 151-200 | Red | Unhealthy |
| 201-300 | Purple | Very Unhealthy |
| 301-500 | Maroon | Hazardous |

#### Settings

- **Time Range**: Last 7 days (default)
- **Refresh**: 5 minutes
- **Timezone**: Browser
- **Time Picker**: Enabled
- **Variables**: `aqi_standard` (US vs European)

### 4.4 Dashboard 4: Indoor vs Outdoor Comparison

**UID**: `ndp-comparison`
**Title**: Indoor vs Outdoor Comparison
**Tags**: `ndp`, `comparison`

#### Panels

| Panel | Type | Query | Visualization | Layout |
|-------|------|-------|---------------|--------|
| Temperature Comparison | Time Series | Indoor vs Outdoor temp | Dual-line graph | Top left |
| PM2.5 Comparison | Time Series | Indoor vs Outdoor PM2.5 | Dual-line graph | Top right |
| Humidity Comparison | Time Series | Indoor vs Outdoor humidity | Dual-line graph | Middle left |
| CO2 Levels | Time Series | Indoor CO2 only | Line graph | Middle right |
| Correlation Matrix | Heatmap | Cross-correlation of all metrics | Color-coded grid | Bottom |

#### Example Query (Temperature Comparison)

```sql
SELECT
    bucket as time,
    stream_id,
    CASE
        WHEN stream_id = 'air-quality' THEN avg_temperature
        WHEN stream_id = 'outdoor-conditions' THEN avg_temperature
    END as temperature,
    CASE
        WHEN stream_id = 'air-quality' THEN 'Indoor'
        WHEN stream_id = 'outdoor-conditions' THEN 'Outdoor'
    END as location
FROM readings_hourly
WHERE
    stream_id IN ('air-quality', 'outdoor-conditions')
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket, stream_id
```

#### Layout

```
┌─────────────────────────────────────────────────────────┐
│ Indoor vs Outdoor Comparison                            │
├──────────────────────────┬──────────────────────────────┤
│ Temperature Comparison   │ PM2.5 Comparison             │
│ (Time Series - Aligned)  │ (Time Series - Aligned)      │
├──────────────────────────┼──────────────────────────────┤
│ Humidity Comparison      │ CO2 Levels (Indoor Only)     │
│ (Time Series - Aligned)  │ (Time Series)                │
├──────────────────────────┴──────────────────────────────┤
│ Correlation Matrix (Heatmap - All Metrics)             │
└─────────────────────────────────────────────────────────┘
```

#### Timeline Alignment

All time-series panels must use **synchronized time range** to enable visual correlation analysis.

**Settings**:
- Shared crosshair enabled
- Shared tooltip enabled
- Same X-axis scale across all panels

#### Settings

- **Time Range**: Last 7 days (default)
- **Refresh**: 5 minutes
- **Timezone**: Browser
- **Time Picker**: Enabled (shared across all panels)
- **Variables**: None

## 5. Acceptance Criteria

### AC-001: Grafana Container Accessibility

**Criteria**: Grafana UI is accessible on port 3000 without authentication.

**Verification**:
```bash
curl -I http://localhost:3000/
# Expected: HTTP 200 OK
```

**Status**: Pending

---

### AC-002: DuckDB Datasource Connection

**Criteria**: DuckDB datasource connects successfully and can execute queries.

**Verification**:
1. Navigate to Configuration > Data Sources > DuckDB
2. Click "Test" button
3. Execute test query: `SELECT 'Connected' as status`

**Expected Result**: "Data source is working" message with query results.

**Status**: Pending

---

### AC-003: Dashboard Loading

**Criteria**: All four dashboards load without errors.

**Verification**:
1. Navigate to Dashboards > Neural Data Platform folder
2. Open each dashboard:
   - Indoor Air Quality Monitoring
   - Outdoor Weather Conditions
   - Outdoor Air Quality Monitoring
   - Indoor vs Outdoor Comparison
3. Verify no error panels or "No data" messages (assuming data exists)

**Status**: Pending

---

### AC-004: Time Range Picker Functionality

**Criteria**: Time range picker updates all panels correctly.

**Verification**:
1. Open any dashboard
2. Change time range to "Last 24 hours"
3. Verify all panels update with 24h data
4. Change to "Last 30 days"
5. Verify all panels update with 30d data

**Status**: Pending

---

### AC-005: Dashboard Edit Persistence

**Criteria**: Dashboard edits made via UI persist after container restart.

**Verification**:
1. Edit a dashboard (e.g., rename a panel)
2. Save dashboard
3. Restart Grafana container: `docker restart ndp-grafana`
4. Re-open dashboard
5. Verify edit is still present

**Status**: Pending

---

### AC-006: Provisioned Dashboards on Startup

**Criteria**: All provisioned dashboards appear in the UI after container startup.

**Verification**:
1. Stop Grafana: `docker stop ndp-grafana`
2. Remove Grafana data volume: `docker volume rm ndp_grafana-data`
3. Start Grafana: `docker compose up -d grafana`
4. Wait 60 seconds for provisioning
5. Navigate to Dashboards > Neural Data Platform folder
6. Verify all 4 dashboards are present

**Status**: Pending

---

## 6. Implementation Notes

### 6.1 DuckDB Query Patterns

#### Aggregated Time Series

```sql
-- Use readings_hourly for efficient queries
SELECT
    bucket as time,
    avg_<field> as "<Field Name>"
FROM readings_hourly
WHERE
    stream_id = '<stream-id>'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

#### Latest Values

```sql
-- Use readings_hourly for current stats
SELECT
    <field> as "<Field Name>"
FROM readings_hourly
WHERE stream_id = '<stream-id>'
ORDER BY bucket DESC
LIMIT 1
```

#### Multi-Stream Comparisons

```sql
-- Use UNION ALL for indoor vs outdoor
SELECT bucket as time, avg_<field> as "Indoor"
FROM readings_hourly
WHERE stream_id = 'air-quality' AND bucket >= $__timeFrom()

UNION ALL

SELECT bucket as time, avg_<field> as "Outdoor"
FROM readings_hourly
WHERE stream_id = 'outdoor-conditions' AND bucket >= $__timeFrom()

ORDER BY time
```

### 6.2 Performance Considerations

- **Hourly Rollups**: Use `readings_hourly` for time ranges > 24 hours
- **Daily Rollups**: Use `readings_daily` for time ranges > 7 days
- **Query Timeout**: Set to 30 seconds to prevent long-running queries
- **Cache TTL**: 5 minutes for dashboards with 5-minute refresh rate

### 6.3 Dashboard Export Workflow

**To save dashboard edits back to Git**:

```bash
# Export dashboard JSON via Grafana API
curl -H "Authorization: Bearer <api-key>" \
  http://localhost:3000/api/dashboards/uid/<dashboard-uid> \
  | jq '.dashboard' > deploy/grafana/dashboards/<name>.json

# Commit to Git
git add deploy/grafana/dashboards/<name>.json
git commit -m "feat(dp-001): update <dashboard-name> dashboard"
```

### 6.4 Resource Monitoring

**Monitor Grafana memory usage**:

```bash
docker stats ndp-grafana --no-stream
```

If memory exceeds 200MB consistently, consider:
- Reducing dashboard complexity
- Increasing cache TTL
- Adding query result limits

## 7. Future Enhancements

### Phase 2: Alerting (AL-001)

- Grafana Alerting integration with Rust alert engine
- Notification channels (email, webhook)
- Alert history dashboard

### Phase 3: Predictions (ML-001)

- Forecast panels overlaid on historical data
- Model performance metrics dashboard
- Prediction confidence bands

### Phase 4: Multi-Location

- Location variable for multi-room/multi-building support
- Comparison across locations
- Location-specific thresholds

## 8. Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| Grafana OSS | latest (ARM64) | Visualization platform |
| motherduck-duckdb-datasource | latest | DuckDB integration |
| DuckDB | 1.1+ | Data source (from DP-001) |
| Docker | 20.10+ | Container runtime |

## 9. References

- [Grafana Documentation](https://grafana.com/docs/grafana/latest/)
- [Grafana Provisioning](https://grafana.com/docs/grafana/latest/administration/provisioning/)
- [DuckDB Datasource Plugin](https://grafana.com/grafana/plugins/motherduck-duckdb-datasource/)
- [DP-001 Feature Overview](../SCOPE.md)
- [DuckDB Schema Specification](./DUCKDB_SPECIFICATION.md)

---

**Next Steps**:
1. Create dashboard JSON files in `/deploy/grafana/dashboards/`
2. Create provisioning YAML files
3. Test locally with Docker Compose
4. Validate against acceptance criteria
5. Document in COMPLETION.md
