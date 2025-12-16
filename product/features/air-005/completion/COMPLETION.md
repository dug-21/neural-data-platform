# SPARC Completion: AIR-005 - Outdoor Weather Data Integration

**Feature**: Adding Outdoor Weather Data to Neural Data Platform
**SPARC Phase**: Completion
**Version**: 1.1.0
**Date**: 2025-12-16
**Status**: Ready for Implementation

> **Implementation Note**: This feature requires refactoring `core/src/sources/http_poll.rs`
> from a hardcoded implementation to a generic HTTP polling system. See the updated
> [Architecture](../architecture/ARCHITECTURE.md) document for details on:
> - `ResponseParser` trait for pluggable response parsing
> - `AuthMethod` enum for flexible authentication
> - `RetryConfig` for exponential backoff with jitter
> - `EndpointConfig` replacing `SensorConfig`

---

## Table of Contents

1. [Pre-Deployment Checklist](#1-pre-deployment-checklist)
2. [Configuration Steps](#2-configuration-steps)
3. [Deployment Procedure](#3-deployment-procedure)
4. [Stream Configuration](#4-stream-configuration)
5. [Verification Steps](#5-verification-steps)
6. [Monitoring Setup](#6-monitoring-setup)
7. [Troubleshooting Guide](#7-troubleshooting-guide)
8. [Rollback Procedure](#8-rollback-procedure)
9. [Post-Deployment Validation](#9-post-deployment-validation)
10. [Sign-off Criteria](#10-sign-off-criteria)

---

## 1. Pre-Deployment Checklist

### 1.1 Environment Requirements

- [ ] **Raspberry Pi 5 Running**: Ubuntu 25.04, Docker installed
- [ ] **Network Access**: Outbound HTTPS access to OpenWeatherMap API (`api.openweathermap.org`)
- [ ] **API Key Available**: OpenWeatherMap API key in `.env` file
- [ ] **Disk Space**: Minimum 10GB free space for data storage
- [ ] **Git Repository**: Latest code from `main` branch pulled

### 1.2 Configuration Files Ready

- [ ] `.env` file with `OPENWEATHER_API_KEY` set
- [ ] Weather stream config YAML prepared
- [ ] Air quality stream config YAML prepared
- [ ] Stream loader scripts verified

### 1.3 Dependencies Installed

```bash
# Verify Docker and Docker Compose
docker --version
# Expected: Docker version 20.10 or higher

docker compose version
# Expected: Docker Compose version 2.0 or higher

# Verify etcd client
command -v etcdctl
# Should be available in etcd container
```

### 1.4 Backup Current State

```bash
# Backup existing etcd data
docker exec etcd etcdctl snapshot save /tmp/etcd-backup-$(date +%Y%m%d-%H%M%S).db

# Copy backup to host
docker cp etcd:/tmp/etcd-backup-$(date +%Y%m%d-%H%M%S).db ~/backups/

# Backup existing Parquet data
tar -czf ~/backups/data-backup-$(date +%Y%m%d-%H%M%S).tar.gz /var/lib/docker/volumes/air-quality-data/
```

---

## 2. Configuration Steps

### 2.1 Environment Variables Setup

**File**: `/workspaces/neural-data-platform/.env`

```bash
# Verify API key is present
grep OPENWEATHER_API_KEY /workspaces/neural-data-platform/.env

# Expected output:
# OPENWEATHER_API_KEY=$OPENWEATHERMAP_API_KEY
```

**Add Additional Configuration** (if not present):

```bash
cat >> /workspaces/neural-data-platform/.env <<'EOF'

# Weather Data Configuration
WEATHER_LATITUDE=37.7749
WEATHER_LONGITUDE=-122.4194
WEATHER_POLL_INTERVAL=600
WEATHER_ENABLED=true

# Air Quality Monitoring
AIR_QUALITY_POLL_INTERVAL=600
AIR_QUALITY_ENABLED=true
EOF
```

### 2.2 Create Stream Configuration Directory

```bash
cd /workspaces/neural-data-platform/deploy/pi

# Create stream config directories
mkdir -p configs/streams/weather
mkdir -p configs/streams/air-pollution
```

### 2.3 etcd Configuration Keys

The following keys will be populated during stream initialization:

```bash
# Weather stream metadata
/streams/weather/config                  # Stream configuration YAML
/streams/weather/poll_interval           # 600 (seconds)
/streams/weather/api_url                 # https://api.openweathermap.org/data/2.5/weather
/streams/weather/latitude                # From .env
/streams/weather/longitude               # From .env
/streams/weather/enabled                 # true

# Air pollution stream metadata
/streams/air-pollution/config            # Stream configuration YAML
/streams/air-pollution/poll_interval     # 600 (seconds)
/streams/air-pollution/api_url           # https://api.openweathermap.org/data/2.5/air_pollution
/streams/air-pollution/latitude          # From .env
/streams/air-pollution/longitude         # From .env
/streams/air-pollution/enabled           # true
```

---

## 3. Deployment Procedure

### 3.1 Pre-Deployment Verification

```bash
cd /workspaces/neural-data-platform/deploy/pi

# Check current system status
./deploy.sh status

# Verify etcd is healthy
docker exec etcd etcdctl endpoint health
# Expected: 127.0.0.1:2379 is healthy: successfully committed proposal

# Check existing streams
docker exec etcd etcdctl get /streams/ --prefix --keys-only
```

### 3.2 Deploy Stream Configurations

**Step 1: Create Weather Stream Config**

```bash
cat > /workspaces/neural-data-platform/deploy/pi/configs/streams/weather/config.yaml <<'EOF'
stream_id: weather
description: Outdoor weather conditions from OpenWeatherMap API
version: "1.0.0"
enabled: true
retention_days: 365
compression_after_days: 7
partitioning_strategy: daily

fields:
  - name: temperature
    type: float
    unit: celsius
    nullable: false
    range: [-50, 60]
    display_precision: 1
    description: Current temperature

  - name: feels_like
    type: float
    unit: celsius
    nullable: false
    range: [-50, 60]
    display_precision: 1
    description: Feels-like temperature

  - name: temp_min
    type: float
    unit: celsius
    nullable: true
    range: [-50, 60]
    display_precision: 1
    description: Minimum temperature

  - name: temp_max
    type: float
    unit: celsius
    nullable: true
    range: [-50, 60]
    display_precision: 1
    description: Maximum temperature

  - name: pressure
    type: int
    unit: hPa
    nullable: false
    range: [800, 1200]
    description: Atmospheric pressure at sea level

  - name: humidity
    type: int
    unit: percent
    nullable: false
    range: [0, 100]
    description: Relative humidity

  - name: visibility
    type: int
    unit: meters
    nullable: true
    range: [0, 20000]
    description: Visibility distance

  - name: wind_speed
    type: float
    unit: m/s
    nullable: true
    range: [0, 100]
    display_precision: 1
    description: Wind speed

  - name: wind_deg
    type: int
    unit: degrees
    nullable: true
    range: [0, 360]
    description: Wind direction

  - name: wind_gust
    type: float
    unit: m/s
    nullable: true
    range: [0, 150]
    display_precision: 1
    description: Wind gust speed

  - name: clouds
    type: int
    unit: percent
    nullable: false
    range: [0, 100]
    description: Cloudiness percentage

  - name: rain_1h
    type: float
    unit: mm
    nullable: true
    range: [0, 500]
    display_precision: 2
    description: Rain volume for last 1 hour

  - name: rain_3h
    type: float
    unit: mm
    nullable: true
    range: [0, 1500]
    display_precision: 2
    description: Rain volume for last 3 hours

  - name: snow_1h
    type: float
    unit: mm
    nullable: true
    range: [0, 500]
    display_precision: 2
    description: Snow volume for last 1 hour

  - name: snow_3h
    type: float
    unit: mm
    nullable: true
    range: [0, 1500]
    display_precision: 2
    description: Snow volume for last 3 hours

  - name: weather_id
    type: int
    nullable: false
    description: Weather condition code

  - name: weather_main
    type: string
    nullable: false
    description: Group of weather parameters (Rain, Snow, etc)

  - name: weather_description
    type: string
    nullable: false
    description: Weather condition description

  - name: sunrise
    type: int
    unit: unix_timestamp
    nullable: false
    description: Sunrise time (Unix UTC)

  - name: sunset
    type: int
    unit: unix_timestamp
    nullable: false
    description: Sunset time (Unix UTC)

  - name: timezone_offset
    type: int
    unit: seconds
    nullable: false
    description: Shift in seconds from UTC

sources:
  - type: http_poll
    enabled: true
    url: https://api.openweathermap.org/data/2.5/weather
    interval_seconds: 600
    auth:
      type: api_key
      key_param: appid
      key_env: OPENWEATHER_API_KEY
    params:
      lat: "${WEATHER_LATITUDE}"
      lon: "${WEATHER_LONGITUDE}"
      units: metric
EOF
```

**Step 2: Create Air Pollution Stream Config**

```bash
cat > /workspaces/neural-data-platform/deploy/pi/configs/streams/air-pollution/config.yaml <<'EOF'
stream_id: air-pollution
description: Outdoor air quality data from OpenWeatherMap Air Pollution API
version: "1.0.0"
enabled: true
retention_days: 365
compression_after_days: 7
partitioning_strategy: daily

fields:
  - name: aqi
    type: int
    nullable: false
    range: [1, 5]
    description: Air Quality Index (1=Good, 2=Fair, 3=Moderate, 4=Poor, 5=Very Poor)

  - name: co
    type: float
    unit: μg/m3
    nullable: false
    range: [0, 50000]
    display_precision: 2
    description: Carbon monoxide concentration

  - name: no
    type: float
    unit: μg/m3
    nullable: false
    range: [0, 1000]
    display_precision: 2
    description: Nitrogen monoxide concentration

  - name: no2
    type: float
    unit: μg/m3
    nullable: false
    range: [0, 1000]
    display_precision: 2
    description: Nitrogen dioxide concentration

  - name: o3
    type: float
    unit: μg/m3
    nullable: false
    range: [0, 1000]
    display_precision: 2
    description: Ozone concentration

  - name: so2
    type: float
    unit: μg/m3
    nullable: false
    range: [0, 1000]
    display_precision: 2
    description: Sulphur dioxide concentration

  - name: pm2_5
    type: float
    unit: μg/m3
    nullable: false
    range: [0, 1000]
    display_precision: 2
    description: Fine particles matter (PM2.5)

  - name: pm10
    type: float
    unit: μg/m3
    nullable: false
    range: [0, 1000]
    display_precision: 2
    description: Coarse particles matter (PM10)

  - name: nh3
    type: float
    unit: μg/m3
    nullable: false
    range: [0, 1000]
    display_precision: 2
    description: Ammonia concentration

sources:
  - type: http_poll
    enabled: true
    url: https://api.openweathermap.org/data/2.5/air_pollution
    interval_seconds: 600
    auth:
      type: api_key
      key_param: appid
      key_env: OPENWEATHER_API_KEY
    params:
      lat: "${WEATHER_LATITUDE}"
      lon: "${WEATHER_LONGITUDE}"
EOF
```

**Step 3: Create Stream Initialization Script**

```bash
cat > /workspaces/neural-data-platform/deploy/pi/configs/streams/init-weather-streams.sh <<'EOF'
#!/bin/bash
# Initialize weather-related streams in etcd
# Usage: ./init-weather-streams.sh [etcd_container_name]

set -e

ETCD_CONTAINER="${1:-etcd}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log() { echo -e "${GREEN}[WEATHER-INIT]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

# Wait for etcd
log "Waiting for etcd to be ready..."
until docker exec "$ETCD_CONTAINER" etcdctl endpoint health >/dev/null 2>&1; do
    warn "etcd not ready, retrying in 2s..."
    sleep 2
done

log "etcd is ready, loading weather stream configurations..."

# Load environment variables
if [ -f "$SCRIPT_DIR/../../../.env" ]; then
    export $(grep -v '^#' "$SCRIPT_DIR/../../../.env" | xargs)
else
    warn ".env file not found, using defaults"
    export WEATHER_LATITUDE=${WEATHER_LATITUDE:-37.7749}
    export WEATHER_LONGITUDE=${WEATHER_LONGITUDE:-122.4194}
fi

# Function to load stream config with variable substitution
load_stream() {
    local stream_id=$1
    local config_file="$SCRIPT_DIR/$stream_id/config.yaml"

    if [ ! -f "$config_file" ]; then
        warn "Config file not found: $config_file"
        return 1
    fi

    log "Loading stream: $stream_id"

    # Substitute environment variables in config
    local config_content=$(envsubst < "$config_file")

    # Store in etcd
    docker exec "$ETCD_CONTAINER" sh -c "etcdctl put '/streams/$stream_id/config' '$config_content'"

    # Store configuration parameters
    docker exec "$ETCD_CONTAINER" etcdctl put "/streams/$stream_id/latitude" "${WEATHER_LATITUDE}"
    docker exec "$ETCD_CONTAINER" etcdctl put "/streams/$stream_id/longitude" "${WEATHER_LONGITUDE}"
    docker exec "$ETCD_CONTAINER" etcdctl put "/streams/$stream_id/poll_interval" "600"
    docker exec "$ETCD_CONTAINER" etcdctl put "/streams/$stream_id/enabled" "true"
    docker exec "$ETCD_CONTAINER" etcdctl put "/streams/$stream_id/created_at" "$(date -Iseconds)"
}

# Load weather streams
load_stream "weather"
load_stream "air-pollution"

log "Weather stream configurations loaded successfully!"

# Verify
log "Verifying stream configurations..."
echo ""
log "Registered weather streams:"
docker exec "$ETCD_CONTAINER" etcdctl get /streams/ --prefix --keys-only | grep -E '(weather|air-pollution)' | grep config

echo ""
log "Weather stream initialization complete!"
EOF

chmod +x /workspaces/neural-data-platform/deploy/pi/configs/streams/init-weather-streams.sh
```

### 3.3 Execute Deployment

```bash
cd /workspaces/neural-data-platform/deploy/pi

# Step 1: Stop services
./deploy.sh stop

# Step 2: Pull latest code (if needed)
cd ../..
git pull origin main
cd deploy/pi

# Step 3: Build with latest changes
./deploy.sh build

# Step 4: Start services
./deploy.sh start

# Step 5: Initialize weather streams
./configs/streams/init-weather-streams.sh etcd

# Step 6: Verify deployment
./deploy.sh status
```

---

## 4. Stream Configuration

### 4.1 Weather Stream YAML (Complete)

See **Step 1** in section 3.2 above for the complete `weather/config.yaml`.

**Key Characteristics**:
- **Stream ID**: `weather`
- **Fields**: 21 fields covering temperature, pressure, humidity, wind, precipitation, clouds, and weather conditions
- **Source**: HTTP polling of OpenWeatherMap Current Weather API
- **Poll Interval**: 600 seconds (10 minutes)
- **Retention**: 365 days (1 year)
- **Compression**: After 7 days

### 4.2 Air Pollution Stream YAML (Complete)

See **Step 2** in section 3.2 above for the complete `air-pollution/config.yaml`.

**Key Characteristics**:
- **Stream ID**: `air-pollution`
- **Fields**: 9 fields covering AQI and pollutant concentrations (CO, NO, NO2, O3, SO2, PM2.5, PM10, NH3)
- **Source**: HTTP polling of OpenWeatherMap Air Pollution API
- **Poll Interval**: 600 seconds (10 minutes)
- **Retention**: 365 days (1 year)
- **Compression**: After 7 days

### 4.3 Stream Configuration Parameters

Both streams share these configuration parameters in etcd:

```bash
# Global weather configuration
/config/weather/latitude              # Location latitude
/config/weather/longitude             # Location longitude
/config/weather/api_key_env           # Environment variable name (OPENWEATHER_API_KEY)
/config/weather/update_interval       # 600 (seconds)

# Per-stream configuration
/streams/{stream_id}/enabled          # true/false
/streams/{stream_id}/poll_interval    # 600 (seconds)
/streams/{stream_id}/latitude         # Override global if needed
/streams/{stream_id}/longitude        # Override global if needed
/streams/{stream_id}/config           # Full YAML configuration
```

---

## 5. Verification Steps

### 5.1 etcd Configuration Verification

```bash
# Verify weather stream loaded
docker exec etcd etcdctl get /streams/weather/config

# Verify air pollution stream loaded
docker exec etcd etcdctl get /streams/air-pollution/config

# List all registered streams
docker exec etcd etcdctl get /streams/ --prefix --keys-only

# Check stream parameters
docker exec etcd etcdctl get /streams/weather/latitude --print-value-only
docker exec etcd etcdctl get /streams/weather/longitude --print-value-only
docker exec etcd etcdctl get /streams/weather/enabled --print-value-only
```

**Expected Output**:
```
/streams/air-pollution/config
/streams/air-quality/config
/streams/weather/config
```

### 5.2 API Health Checks

```bash
# Get Raspberry Pi IP
PI_IP=$(hostname -I | awk '{print $1}')

# Check main application health
curl -s http://${PI_IP}:8080/health | jq .

# Expected output:
{
  "status": "healthy",
  "services": {
    "etcd": "connected",
    "mqtt": "connected",
    "storage": "operational"
  }
}

# Check streams endpoint (if implemented)
curl -s http://${PI_IP}:8080/api/v1/streams | jq .

# Expected: List of streams including "weather" and "air-pollution"
```

### 5.3 Data Flow Verification

**Test OpenWeatherMap API Access**:

```bash
# Test weather API
curl -s "https://api.openweathermap.org/data/2.5/weather?lat=37.7749&lon=-122.4194&units=metric&appid=$OPENWEATHERMAP_API_KEY" | jq .

# Expected: JSON response with weather data

# Test air pollution API
curl -s "https://api.openweathermap.org/data/2.5/air_pollution?lat=37.7749&lon=-122.4194&appid=$OPENWEATHERMAP_API_KEY" | jq .

# Expected: JSON response with air quality data
```

**Monitor Application Logs**:

```bash
# Watch for weather data ingestion
docker logs -f air-quality-app | grep -i weather

# Expected to see:
# [INFO] Starting HttpPollSource for stream: weather
# [INFO] Fetched weather data: 21 fields
# [INFO] Stored weather data to /data/weather/YYYY-MM-DD.parquet

# Watch for air pollution data
docker logs -f air-quality-app | grep -i "air-pollution"

# Expected to see:
# [INFO] Starting HttpPollSource for stream: air-pollution
# [INFO] Fetched air pollution data: 9 fields
# [INFO] Stored air pollution data to /data/air-pollution/YYYY-MM-DD.parquet
```

**Verify Parquet Files Created**:

```bash
# Check weather data directory
docker exec air-quality-app ls -lh /data/weather/

# Expected: Parquet files with current date
# -rw-r--r-- 1 root root 12K Dec 16 14:30 2025-12-16.parquet

# Check air pollution data directory
docker exec air-quality-app ls -lh /data/air-pollution/

# Expected: Parquet files with current date
# -rw-r--r-- 1 root root 8K Dec 16 14:30 2025-12-16.parquet

# Inspect parquet file structure (if parquet-tools available)
docker exec air-quality-app parquet-tools schema /data/weather/2025-12-16.parquet
```

### 5.4 Timestamp Normalization Verification

**Critical**: Verify timestamps are in UTC and consistent with existing data.

```bash
# Query recent weather data (if query API implemented)
curl -s "http://${PI_IP}:8080/api/v1/data/weather?limit=1" | jq '.data[0].timestamp'

# Expected format: "2025-12-16T14:30:00Z" (UTC timezone)

# Compare with air quality data timestamp format
curl -s "http://${PI_IP}:8080/api/v1/data/air-quality?limit=1" | jq '.data[0].timestamp'

# Timestamps should be in same format and timezone
```

---

## 6. Monitoring Setup

### 6.1 Log Monitoring

**Create log monitoring script**:

```bash
cat > /workspaces/neural-data-platform/deploy/pi/scripts/monitor-weather.sh <<'EOF'
#!/bin/bash
# Monitor weather data ingestion

echo "=== Weather Stream Monitoring ==="
echo ""

echo "Latest weather logs:"
docker logs --tail 20 air-quality-app | grep -i weather
echo ""

echo "Latest air pollution logs:"
docker logs --tail 20 air-quality-app | grep -i "air-pollution"
echo ""

echo "Weather data files:"
docker exec air-quality-app ls -lh /data/weather/ | tail -5
echo ""

echo "Air pollution data files:"
docker exec air-quality-app ls -lh /data/air-pollution/ | tail -5
echo ""

echo "Stream health:"
docker exec etcd etcdctl get /streams/weather/enabled --print-value-only
docker exec etcd etcdctl get /streams/air-pollution/enabled --print-value-only
EOF

chmod +x /workspaces/neural-data-platform/deploy/pi/scripts/monitor-weather.sh
```

### 6.2 Performance Metrics

**Key Metrics to Monitor**:

| Metric | Target | Alert Threshold |
|--------|--------|-----------------|
| API Response Time | < 2s | > 5s |
| Poll Success Rate | > 99% | < 95% |
| Data Points/Hour | 12 (per stream) | < 10 |
| Parquet File Size | 10-50KB/day | > 100KB/day |
| Memory Usage | < 100MB | > 200MB |
| Disk Growth | ~1MB/day/stream | > 10MB/day |

**Monitor Commands**:

```bash
# Check memory usage
docker stats air-quality-app --no-stream

# Check disk usage
docker exec air-quality-app du -sh /data/weather /data/air-pollution

# Check API latency (add custom metrics if needed)
docker logs air-quality-app | grep "API call duration" | tail -10
```

### 6.3 Alerting Setup (Optional)

**Create basic alert script**:

```bash
cat > /workspaces/neural-data-platform/deploy/pi/scripts/weather-health-check.sh <<'EOF'
#!/bin/bash
# Health check script for weather streams
# Run via cron every 15 minutes

ALERT_EMAIL="admin@example.com"
ERROR_COUNT=0

# Check if weather data is recent
LATEST_WEATHER=$(docker exec air-quality-app ls -t /data/weather/*.parquet 2>/dev/null | head -1)
if [ -z "$LATEST_WEATHER" ]; then
    echo "ERROR: No weather data found" | tee -a /var/log/weather-alerts.log
    ((ERROR_COUNT++))
else
    AGE=$(stat -c %Y "$LATEST_WEATHER" 2>/dev/null || echo 0)
    NOW=$(date +%s)
    if [ $((NOW - AGE)) -gt 1800 ]; then  # 30 minutes
        echo "WARNING: Weather data is stale (> 30 min)" | tee -a /var/log/weather-alerts.log
    fi
fi

# Check if air pollution data is recent
LATEST_AIR=$(docker exec air-quality-app ls -t /data/air-pollution/*.parquet 2>/dev/null | head -1)
if [ -z "$LATEST_AIR" ]; then
    echo "ERROR: No air pollution data found" | tee -a /var/log/weather-alerts.log
    ((ERROR_COUNT++))
fi

# Check API errors in logs
API_ERRORS=$(docker logs --since 15m air-quality-app 2>&1 | grep -i "weather.*error" | wc -l)
if [ "$API_ERRORS" -gt 5 ]; then
    echo "ERROR: High API error rate ($API_ERRORS errors in 15min)" | tee -a /var/log/weather-alerts.log
    ((ERROR_COUNT++))
fi

# Send alert if errors detected
if [ $ERROR_COUNT -gt 0 ]; then
    # Uncomment to enable email alerts
    # echo "Weather stream health check failed: $ERROR_COUNT errors" | mail -s "Weather Alert" $ALERT_EMAIL
    exit 1
fi

exit 0
EOF

chmod +x /workspaces/neural-data-platform/deploy/pi/scripts/weather-health-check.sh

# Add to cron (optional)
# */15 * * * * /workspaces/neural-data-platform/deploy/pi/scripts/weather-health-check.sh
```

---

## 7. Troubleshooting Guide

### 7.1 Streams Not Loading

**Symptoms**: etcd shows no weather/air-pollution streams

**Diagnosis**:
```bash
# Check if init script ran
docker logs air-quality-app | grep "WEATHER-INIT"

# Check etcd keys
docker exec etcd etcdctl get /streams/ --prefix --keys-only
```

**Solutions**:
1. Re-run stream initialization: `./configs/streams/init-weather-streams.sh etcd`
2. Check YAML syntax: `cat configs/streams/weather/config.yaml | yq eval`
3. Verify etcd is healthy: `docker exec etcd etcdctl endpoint health`

### 7.2 API Access Failures

**Symptoms**: Logs show "Failed to fetch weather data" or HTTP errors

**Diagnosis**:
```bash
# Check API key
echo $OPENWEATHER_API_KEY

# Test API manually
curl -v "https://api.openweathermap.org/data/2.5/weather?lat=37.7749&lon=-122.4194&appid=$OPENWEATHER_API_KEY"

# Check container network
docker exec air-quality-app ping -c 3 api.openweathermap.org
```

**Common Errors**:

| HTTP Code | Meaning | Solution |
|-----------|---------|----------|
| 401 | Invalid API key | Check `OPENWEATHER_API_KEY` in `.env` |
| 429 | Rate limit exceeded | Free tier: 60 calls/min, reduce poll frequency |
| 404 | Invalid coordinates | Verify `WEATHER_LATITUDE` and `WEATHER_LONGITUDE` |
| 503 | Service unavailable | OpenWeatherMap downtime, wait and retry |

**Solutions**:
1. Verify API key: `grep OPENWEATHER_API_KEY /workspaces/neural-data-platform/.env`
2. Check rate limits: Free tier = 1,000 calls/day (our 10min interval = 288 calls/day)
3. Verify coordinates are valid: `-90 <= lat <= 90`, `-180 <= lon <= 180`

### 7.3 Data Not Being Stored

**Symptoms**: API calls succeed but no Parquet files created

**Diagnosis**:
```bash
# Check storage writer logs
docker logs air-quality-app | grep -i "storage\|parquet"

# Check data directory permissions
docker exec air-quality-app ls -la /data/

# Check disk space
docker exec air-quality-app df -h /data
```

**Solutions**:
1. Ensure data directory exists: `docker exec air-quality-app mkdir -p /data/weather /data/air-pollution`
2. Check file permissions: `docker exec air-quality-app chmod 755 /data/weather /data/air-pollution`
3. Verify storage writer is running: `docker logs air-quality-app | grep "StorageWriter started"`
4. Check for disk space issues: `docker exec air-quality-app df -h`

### 7.4 Timestamp Issues

**Symptoms**: Timestamps in wrong timezone or format

**Diagnosis**:
```bash
# Check recent data timestamps
docker exec air-quality-app parquet-tools cat --json /data/weather/$(date +%Y-%m-%d).parquet | jq '.timestamp' | head -5

# Expected: "2025-12-16T14:30:00Z"
# Wrong: "2025-12-16T14:30:00-08:00" or "1734361800" (Unix timestamp)
```

**Solutions**:
1. Verify timezone conversion in HTTP poller
2. Ensure UTC normalization in data transformation
3. Check that `timezone_offset` field is properly handled

### 7.5 Memory Issues

**Symptoms**: Container crashes or OOM (Out of Memory) errors

**Diagnosis**:
```bash
# Check memory usage
docker stats air-quality-app --no-stream

# Check OOM events
docker inspect air-quality-app | jq '.[0].State.OOMKilled'
```

**Solutions**:
1. Increase memory limit in `docker-compose.yml`: `memory: 512M` → `memory: 768M`
2. Reduce poll frequency: 600s → 900s (15 minutes)
3. Check for memory leaks in logs

---

## 8. Rollback Procedure

### 8.1 Quick Rollback (Disable Streams)

**If weather streams are causing issues, disable them without full rollback**:

```bash
# Disable weather stream
docker exec etcd etcdctl put /streams/weather/enabled "false"

# Disable air pollution stream
docker exec etcd etcdctl put /streams/air-pollution/enabled "false"

# Restart application to pick up changes
docker restart air-quality-app

# Verify streams are disabled
docker logs air-quality-app | grep -i weather
# Should see: [INFO] Stream weather is disabled, skipping
```

### 8.2 Full Rollback

**If complete rollback is required**:

```bash
cd /workspaces/neural-data-platform/deploy/pi

# Step 1: Stop services
./deploy.sh stop

# Step 2: Remove weather streams from etcd
docker start etcd
docker exec etcd etcdctl del /streams/weather/ --prefix
docker exec etcd etcdctl del /streams/air-pollution/ --prefix

# Step 3: Restore etcd backup (if needed)
BACKUP_FILE=$(ls -t ~/backups/etcd-backup-*.db | head -1)
docker cp "$BACKUP_FILE" etcd:/tmp/etcd-restore.db
docker exec etcd sh -c "ETCDCTL_API=3 etcdctl snapshot restore /tmp/etcd-restore.db --data-dir=/etcd-data-restore"
docker stop etcd
# Update docker-compose.yml to point to /etcd-data-restore
docker start etcd

# Step 4: Remove weather data files (optional)
docker exec air-quality-app rm -rf /data/weather
docker exec air-quality-app rm -rf /data/air-pollution

# Step 5: Revert code changes (if code was deployed)
cd /workspaces/neural-data-platform
git checkout main
git pull origin main

# Step 6: Rebuild and restart
cd deploy/pi
./deploy.sh build
./deploy.sh start

# Step 7: Verify rollback
./deploy.sh status
docker exec etcd etcdctl get /streams/ --prefix --keys-only
# Should NOT show /streams/weather or /streams/air-pollution
```

### 8.3 Rollback Verification

```bash
# Verify weather streams removed
docker exec etcd etcdctl get /streams/weather/config
# Expected: empty (no output)

# Verify air quality stream still works
docker exec etcd etcdctl get /streams/air-quality/config
# Expected: air quality config YAML

# Check application logs
docker logs --tail 50 air-quality-app
# Should NOT see weather-related errors

# Verify existing data intact
docker exec air-quality-app ls -lh /data/air-quality/
# Should show existing air quality Parquet files
```

---

## 9. Post-Deployment Validation

### 9.1 24-Hour Validation Checklist

**Run these checks 24 hours after deployment**:

- [ ] **Data Continuity**: Weather data files created for yesterday and today
- [ ] **No Gaps**: No missing data points (expect ~144 points per stream per day at 10min interval)
- [ ] **API Success Rate**: > 99% successful API calls
- [ ] **Storage Growth**: ~1-2MB total growth per day (both streams)
- [ ] **No Errors**: No critical errors in application logs
- [ ] **Memory Stable**: Memory usage < 150MB and not growing
- [ ] **Timestamp Accuracy**: All timestamps in UTC, consistent with existing data

**Validation Commands**:

```bash
# Check data files for last 2 days
docker exec air-quality-app ls -lh /data/weather/
docker exec air-quality-app ls -lh /data/air-pollution/

# Count data points (if parquet-tools available)
docker exec air-quality-app parquet-tools rowcount /data/weather/$(date +%Y-%m-%d).parquet
# Expected: ~144 rows (for 24 hours at 10min interval)

# Check for gaps (look for consistent timestamps)
docker exec air-quality-app parquet-tools cat --json /data/weather/$(date +%Y-%m-%d).parquet | \
  jq -r '.timestamp' | sort | head -20

# Review error rate
docker logs --since 24h air-quality-app 2>&1 | grep -i error | wc -l
# Expected: < 10 errors in 24 hours

# Check storage growth
docker exec air-quality-app du -sh /data/weather /data/air-pollution
```

### 9.2 Data Quality Validation

**Validate data ranges and quality**:

```bash
# Sample weather data
curl -s "http://localhost:8080/api/v1/data/weather?limit=10" | jq '.data[] | {timestamp, temperature, humidity}'

# Verify ranges:
# - Temperature: -50 to 60°C
# - Humidity: 0 to 100%
# - Pressure: 800 to 1200 hPa

# Sample air pollution data
curl -s "http://localhost:8080/api/v1/data/air-pollution?limit=10" | jq '.data[] | {timestamp, aqi, pm2_5}'

# Verify ranges:
# - AQI: 1 to 5
# - PM2.5: 0 to 1000 μg/m³
# - CO, NO2, etc.: 0 to 1000 μg/m³
```

### 9.3 Integration Testing

**Test correlation queries (if implemented)**:

```bash
# Query weather and air quality for same time period
curl -s "http://localhost:8080/api/v1/data/weather?start=2025-12-16T00:00:00Z&end=2025-12-16T23:59:59Z" > weather.json
curl -s "http://localhost:8080/api/v1/data/air-pollution?start=2025-12-16T00:00:00Z&end=2025-12-16T23:59:59Z" > air-pollution.json

# Verify both have data for the same time range
jq '.data | length' weather.json
jq '.data | length' air-pollution.json
# Should be approximately equal (144 points each)
```

---

## 10. Sign-off Criteria

### 10.1 Technical Sign-off

**All criteria must be met before marking deployment as complete**:

- [ ] **Stream Configuration**: Weather and air-pollution streams registered in etcd
- [ ] **API Access**: Successful API calls to OpenWeatherMap (both Current Weather and Air Pollution APIs)
- [ ] **Data Ingestion**: Parquet files created for both streams
- [ ] **Timestamp Normalization**: All timestamps in UTC format consistent with existing streams
- [ ] **Data Validation**: All field values within expected ranges
- [ ] **No Data Gaps**: Continuous data collection for 24+ hours
- [ ] **Performance**: API response time < 5s, memory usage < 200MB
- [ ] **Error Rate**: < 1% API call failures
- [ ] **Storage**: Data files growing at expected rate (~1-2MB/day total)
- [ ] **Monitoring**: Health check script running and passing
- [ ] **Documentation**: This completion document reviewed and updated with actual deployment details

### 10.2 Functional Sign-off

- [ ] **Weather Data Complete**: All 21 weather fields populated correctly
- [ ] **Air Quality Data Complete**: All 9 air pollution fields populated correctly
- [ ] **Location Correct**: Latitude/longitude match intended monitoring location
- [ ] **Update Frequency**: Data updating every 10 minutes as configured
- [ ] **Retention Working**: Old data retained according to policy (365 days)
- [ ] **Queries Working**: Data queryable via API (if query endpoint implemented)
- [ ] **Integration Ready**: Data format compatible with existing platform analytics

### 10.3 Operational Sign-off

- [ ] **Deployment Documented**: All deployment steps executed and documented
- [ ] **Rollback Tested**: Rollback procedure tested and verified
- [ ] **Monitoring Active**: Log monitoring and health checks in place
- [ ] **Alerts Configured**: Alert thresholds set for critical metrics
- [ ] **Backup Verified**: etcd and data backups completed successfully
- [ ] **Team Trained**: Operations team familiar with new streams and troubleshooting
- [ ] **Runbook Created**: This document accessible to operations team

### 10.4 Sign-off Form

**To be completed by deployment team**:

```
Deployment Sign-off: AIR-005 - Outdoor Weather Data Integration

Date: _______________
Deployment By: _______________
Reviewed By: _______________

Technical Validation:
✓ All technical criteria met: [ ]
✓ 24-hour stability confirmed: [ ]
✓ No critical errors: [ ]

Functional Validation:
✓ Weather data verified: [ ]
✓ Air pollution data verified: [ ]
✓ Integration tested: [ ]

Operational Validation:
✓ Monitoring active: [ ]
✓ Runbook complete: [ ]
✓ Team trained: [ ]

Deployment Status: [ ] SUCCESS  [ ] PARTIAL  [ ] ROLLBACK

Notes:
_________________________________________________________________
_________________________________________________________________
_________________________________________________________________

Approved By: _____________________  Date: _______________
```

---

## Appendix A: Quick Reference Commands

### Stream Management

```bash
# List all streams
docker exec etcd etcdctl get /streams/ --prefix --keys-only

# Enable/disable weather stream
docker exec etcd etcdctl put /streams/weather/enabled "true"
docker exec etcd etcdctl put /streams/weather/enabled "false"

# View weather stream config
docker exec etcd etcdctl get /streams/weather/config --print-value-only

# Reload streams (restart app)
docker restart air-quality-app
```

### Data Inspection

```bash
# List weather data files
docker exec air-quality-app ls -lh /data/weather/

# View latest weather data (if parquet-tools available)
docker exec air-quality-app parquet-tools cat /data/weather/$(date +%Y-%m-%d).parquet | head

# Check data size
docker exec air-quality-app du -sh /data/weather /data/air-pollution
```

### Monitoring

```bash
# Watch weather logs live
docker logs -f air-quality-app | grep -i weather

# Check health
curl http://localhost:8080/health | jq .

# View recent errors
docker logs --since 1h air-quality-app 2>&1 | grep -i error

# Monitor resources
docker stats air-quality-app
```

### Troubleshooting

```bash
# Test API connectivity
curl "https://api.openweathermap.org/data/2.5/weather?lat=37.7749&lon=-122.4194&appid=$OPENWEATHER_API_KEY"

# Check etcd health
docker exec etcd etcdctl endpoint health

# Restart services
docker restart air-quality-app

# View full logs
docker logs air-quality-app --tail 100
```

---

## Appendix B: Data Schema Reference

### Weather Stream Fields

| Field | Type | Unit | Range | Nullable | Description |
|-------|------|------|-------|----------|-------------|
| temperature | float | celsius | [-50, 60] | No | Current temperature |
| feels_like | float | celsius | [-50, 60] | No | Feels-like temperature |
| temp_min | float | celsius | [-50, 60] | Yes | Min temperature |
| temp_max | float | celsius | [-50, 60] | Yes | Max temperature |
| pressure | int | hPa | [800, 1200] | No | Atmospheric pressure |
| humidity | int | percent | [0, 100] | No | Relative humidity |
| visibility | int | meters | [0, 20000] | Yes | Visibility distance |
| wind_speed | float | m/s | [0, 100] | Yes | Wind speed |
| wind_deg | int | degrees | [0, 360] | Yes | Wind direction |
| wind_gust | float | m/s | [0, 150] | Yes | Wind gust speed |
| clouds | int | percent | [0, 100] | No | Cloudiness |
| rain_1h | float | mm | [0, 500] | Yes | Rain (1 hour) |
| rain_3h | float | mm | [0, 1500] | Yes | Rain (3 hours) |
| snow_1h | float | mm | [0, 500] | Yes | Snow (1 hour) |
| snow_3h | float | mm | [0, 1500] | Yes | Snow (3 hours) |
| weather_id | int | - | - | No | Weather condition code |
| weather_main | string | - | - | No | Weather group |
| weather_description | string | - | - | No | Weather description |
| sunrise | int | unix_ts | - | No | Sunrise time (UTC) |
| sunset | int | unix_ts | - | No | Sunset time (UTC) |
| timezone_offset | int | seconds | - | No | UTC offset |

### Air Pollution Stream Fields

| Field | Type | Unit | Range | Nullable | Description |
|-------|------|------|-------|----------|-------------|
| aqi | int | - | [1, 5] | No | Air Quality Index |
| co | float | μg/m³ | [0, 50000] | No | Carbon monoxide |
| no | float | μg/m³ | [0, 1000] | No | Nitrogen monoxide |
| no2 | float | μg/m³ | [0, 1000] | No | Nitrogen dioxide |
| o3 | float | μg/m³ | [0, 1000] | No | Ozone |
| so2 | float | μg/m³ | [0, 1000] | No | Sulphur dioxide |
| pm2_5 | float | μg/m³ | [0, 1000] | No | PM2.5 particles |
| pm10 | float | μg/m³ | [0, 1000] | No | PM10 particles |
| nh3 | float | μg/m³ | [0, 1000] | No | Ammonia |

**AQI Scale**:
1. Good
2. Fair
3. Moderate
4. Poor
5. Very Poor

---

## Document Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-16 | AI Assistant | Initial completion document created |
| 1.1.0 | 2025-12-16 | SPARC Agent | Updated to reflect generic HTTP polling architecture with ResponseParser trait |

---

**End of Completion Document**
