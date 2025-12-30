# DP-002 Docker Configuration Changes

**Feature**: Online Data Dictionary and HomeAssistant Stream Preparation
**Date**: 2025-12-30
**Author**: NDP Architecture Team
**Status**: Proposed

---

## 1. Overview

DP-002 introduces **TimescaleDB** as the Silver layer database, replacing the DuckDB container for queryable data. This document details the Docker configuration changes required.

### Change Summary

| Change | Type | Rationale |
|--------|------|-----------|
| Remove DuckDB container | Removal | Replaced by TimescaleDB |
| Add TimescaleDB container | Addition | Data Dictionary + Analytics |
| Update Grafana dependencies | Modification | Point to TimescaleDB |
| Add init scripts volume | Addition | Schema creation on startup |
| Update memory allocations | Modification | Rebalance for TimescaleDB |

---

## 2. Current Docker Compose Structure

```yaml
# Current services (deploy/pi/docker-compose.yml)
services:
  mosquitto:        # 128MB - MQTT broker
  etcd:             # 256MB - Configuration store
  air-quality-app:  # 512MB - Ingestion application
  duckdb:           # 512MB - Silver layer (TO BE REMOVED)
  grafana:          # 256MB - Dashboards

# Total: ~1.7GB
```

---

## 3. Target Docker Compose Structure

```yaml
# Target services after DP-002
services:
  mosquitto:        # 128MB - MQTT broker (UNCHANGED)
  etcd:             # 256MB - Configuration store (UNCHANGED)
  air-quality-app:  # 512MB - Ingestion application (UNCHANGED)
  timescaledb:      # 256MB - Silver layer + Data Dictionary (NEW)
  grafana:          # 256MB - Dashboards (MODIFIED dependencies)

# Total: ~1.4GB (reduced from 1.7GB)
```

---

## 4. DuckDB Container Removal

### Verification Checklist

Before removing DuckDB, verify no remaining dependencies:

| Dependency | Location | Resolution |
|------------|----------|------------|
| Grafana DuckDB plugin | docker-compose.yml | Remove plugin install |
| DuckDB volume mount | Grafana volumes | Remove mount |
| SQL view definitions | config/duckdb/ | Migrate to TimescaleDB |
| Dashboard queries | Grafana dashboards | Update data source |

### Files to Modify

1. `deploy/pi/docker-compose.yml` - Remove duckdb service
2. `config/grafana/provisioning/datasources/` - Update/replace data sources
3. Existing dashboards - Update queries for PostgreSQL syntax

### DuckDB Service to Remove

```yaml
# REMOVE THIS ENTIRE SERVICE
  duckdb:
    image: datacatering/duckdb:v1.1.3
    container_name: duckdb
    volumes:
      - air-quality-data:/data:ro
      - duckdb-data:/var/duckdb
      - ../../config/duckdb:/config/duckdb:ro
    restart: unless-stopped
    entrypoint: /bin/sh
    command:
      - -c
      - |
        # ... initialization script ...
    healthcheck:
      test: ["CMD", "test", "-f", "/var/duckdb/neural_platform.db"]
      interval: 60s
      timeout: 10s
      retries: 10
      start_period: 60s
    deploy:
      resources:
        limits:
          memory: 512M
    depends_on:
      air-quality-app:
        condition: service_healthy
```

### Volume to Remove

```yaml
volumes:
  # REMOVE THIS VOLUME
  duckdb-data:
    driver: local
```

---

## 5. TimescaleDB Container Configuration

### New Service Definition

```yaml
  # TimescaleDB - Silver Layer + Data Dictionary (DP-002)
  # PostgreSQL 15 with TimescaleDB extension for time-series analytics
  timescaledb:
    image: timescale/timescaledb:latest-pg15
    container_name: pi5-timescaledb
    ports:
      - "127.0.0.1:5432:5432"    # Localhost only for security
    environment:
      - POSTGRES_USER=postgres
      - POSTGRES_PASSWORD=${POSTGRES_PASSWORD:-ndp_secure_password}
      - POSTGRES_DB=ndp
      # Memory optimization for Raspberry Pi 5
      - TIMESCALEDB_TELEMETRY=off
    volumes:
      - timescaledb-data:/var/lib/postgresql/data
      - ./init-scripts:/docker-entrypoint-initdb.d:ro
    restart: unless-stopped
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres -d ndp"]
      interval: 30s
      timeout: 10s
      retries: 5
      start_period: 30s
    deploy:
      resources:
        limits:
          memory: 256M
        reservations:
          memory: 128M
```

### Resource Limits Explanation

| Resource | Value | Rationale |
|----------|-------|-----------|
| Memory Limit | 256MB | Sufficient for Data Dictionary + light analytics |
| Memory Reservation | 128MB | Minimum guaranteed memory |
| Port Binding | 127.0.0.1:5432 | Security: localhost only in production |

### Volume Definition

```yaml
volumes:
  timescaledb-data:
    driver: local
```

---

## 6. Init Scripts

### Directory Structure

```
deploy/pi/
  init-scripts/
    01-create-data-dictionary.sql
    02-create-analytics-schema.sql
    03-create-users.sql
```

### 01-create-data-dictionary.sql

```sql
-- Data Dictionary Schema for DP-002
-- Executed on container first start

-- Create schema
CREATE SCHEMA IF NOT EXISTS data_dictionary;

-- Streams table
CREATE TABLE IF NOT EXISTS data_dictionary.streams (
    stream_id           TEXT PRIMARY KEY,
    description         TEXT,
    version             TEXT NOT NULL DEFAULT '1.0.0',
    enabled             BOOLEAN NOT NULL DEFAULT true,
    retention_days      INTEGER DEFAULT 90,
    partitioning_strategy TEXT DEFAULT 'daily',
    compression_after_days INTEGER DEFAULT 7,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata            JSONB
);

-- Fields table
CREATE TABLE IF NOT EXISTS data_dictionary.fields (
    id                  SERIAL PRIMARY KEY,
    stream_id           TEXT NOT NULL REFERENCES data_dictionary.streams(stream_id) ON DELETE CASCADE,
    field_name          TEXT NOT NULL,
    field_type          TEXT NOT NULL,
    nullable            BOOLEAN NOT NULL DEFAULT true,
    unit                TEXT,
    description         TEXT,
    validation_min      DOUBLE PRECISION,
    validation_max      DOUBLE PRECISION,
    validation_pattern  TEXT,
    sort_order          INTEGER NOT NULL DEFAULT 0,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(stream_id, field_name)
);

-- Sources table
CREATE TABLE IF NOT EXISTS data_dictionary.sources (
    id                  SERIAL PRIMARY KEY,
    stream_id           TEXT NOT NULL REFERENCES data_dictionary.streams(stream_id) ON DELETE CASCADE,
    source_id           TEXT NOT NULL,
    source_type         TEXT NOT NULL,
    enabled             BOOLEAN NOT NULL DEFAULT true,
    config              JSONB NOT NULL,
    parser_type         TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(stream_id, source_id)
);

-- Entity schemas table (for HomeAssistant)
CREATE TABLE IF NOT EXISTS data_dictionary.entity_schemas (
    id                  SERIAL PRIMARY KEY,
    stream_id           TEXT NOT NULL REFERENCES data_dictionary.streams(stream_id) ON DELETE CASCADE,
    entity_pattern      TEXT NOT NULL,
    entity_domain       TEXT NOT NULL,
    device_class        TEXT,
    unit_of_measurement TEXT,
    state_mapping       JSONB,
    description         TEXT,
    protocol            TEXT,
    enabled             BOOLEAN NOT NULL DEFAULT true,
    priority            INTEGER NOT NULL DEFAULT 0,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Sync status table
CREATE TABLE IF NOT EXISTS data_dictionary.sync_status (
    id                  SERIAL PRIMARY KEY,
    sync_type           TEXT NOT NULL,
    started_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at        TIMESTAMPTZ,
    status              TEXT NOT NULL DEFAULT 'running',
    streams_synced      INTEGER DEFAULT 0,
    fields_synced       INTEGER DEFAULT 0,
    entities_synced     INTEGER DEFAULT 0,
    error_message       TEXT,
    etcd_revision       BIGINT
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_fields_stream_id ON data_dictionary.fields(stream_id);
CREATE INDEX IF NOT EXISTS idx_sources_stream_id ON data_dictionary.sources(stream_id);
CREATE INDEX IF NOT EXISTS idx_entity_schemas_stream_id ON data_dictionary.entity_schemas(stream_id);
CREATE INDEX IF NOT EXISTS idx_entity_schemas_pattern ON data_dictionary.entity_schemas(entity_pattern);
CREATE INDEX IF NOT EXISTS idx_entity_schemas_domain ON data_dictionary.entity_schemas(entity_domain);
CREATE INDEX IF NOT EXISTS idx_sources_config ON data_dictionary.sources USING GIN (config);
CREATE INDEX IF NOT EXISTS idx_entity_schemas_state_mapping ON data_dictionary.entity_schemas USING GIN (state_mapping);

-- Views
CREATE OR REPLACE VIEW data_dictionary.stream_overview AS
SELECT
    s.stream_id,
    s.description,
    s.version,
    s.enabled,
    s.retention_days,
    COUNT(DISTINCT f.id) AS field_count,
    COUNT(DISTINCT src.id) AS source_count,
    COUNT(DISTINCT e.id) AS entity_schema_count,
    s.created_at,
    s.updated_at
FROM data_dictionary.streams s
LEFT JOIN data_dictionary.fields f ON s.stream_id = f.stream_id
LEFT JOIN data_dictionary.sources src ON s.stream_id = src.stream_id
LEFT JOIN data_dictionary.entity_schemas e ON s.stream_id = e.stream_id
GROUP BY s.stream_id, s.description, s.version, s.enabled,
         s.retention_days, s.created_at, s.updated_at;

CREATE OR REPLACE VIEW data_dictionary.field_details AS
SELECT
    s.stream_id,
    s.description AS stream_description,
    s.enabled AS stream_enabled,
    f.field_name,
    f.field_type,
    f.nullable,
    f.unit,
    f.description AS field_description,
    f.validation_min,
    f.validation_max,
    f.sort_order
FROM data_dictionary.streams s
JOIN data_dictionary.fields f ON s.stream_id = f.stream_id
ORDER BY s.stream_id, f.sort_order, f.field_name;

CREATE OR REPLACE VIEW data_dictionary.entity_schema_details AS
SELECT
    e.id,
    s.stream_id,
    e.entity_pattern,
    e.entity_domain,
    e.device_class,
    e.unit_of_measurement,
    e.state_mapping,
    e.protocol,
    e.enabled,
    e.priority,
    e.description
FROM data_dictionary.entity_schemas e
JOIN data_dictionary.streams s ON e.stream_id = s.stream_id
ORDER BY s.stream_id, e.priority DESC, e.entity_pattern;

-- Grant message
DO $$
BEGIN
    RAISE NOTICE 'Data Dictionary schema created successfully';
END $$;
```

### 03-create-users.sql

```sql
-- Create application users for DP-002

-- Read-only user for Grafana
DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'grafana_reader') THEN
        CREATE USER grafana_reader WITH PASSWORD 'grafana_read_only';
    END IF;
END $$;

GRANT USAGE ON SCHEMA data_dictionary TO grafana_reader;
GRANT SELECT ON ALL TABLES IN SCHEMA data_dictionary TO grafana_reader;
ALTER DEFAULT PRIVILEGES IN SCHEMA data_dictionary GRANT SELECT ON TABLES TO grafana_reader;

-- Grant message
DO $$
BEGIN
    RAISE NOTICE 'Application users created successfully';
END $$;
```

---

## 7. Grafana Configuration Updates

### Updated Dependencies

```yaml
  grafana:
    image: grafana/grafana:latest-ubuntu
    container_name: pi5-grafana
    ports:
      - "3000:3000"
    volumes:
      - grafana-data:/var/lib/grafana
      - ../../config/grafana/grafana.ini:/etc/grafana/grafana.ini:ro
      - ../../config/grafana/provisioning:/etc/grafana/provisioning:ro
      - ../../config/grafana/dashboards:/var/lib/grafana/dashboards:ro
      - air-quality-data:/data:ro             # Keep for Parquet access if needed
      # REMOVED: duckdb-data:/duckdb
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=${GRAFANA_ADMIN_PASSWORD:-admin}
      - GF_USERS_ALLOW_SIGN_UP=false
      - GF_DATABASE_TYPE=sqlite3
      - GF_DATABASE_PATH=/var/lib/grafana/grafana.db
      - GF_SERVER_ROOT_URL=http://localhost:3000
      - GF_LOG_LEVEL=info
      # REMOVED: GF_INSTALL_PLUGINS for DuckDB
      # Keep DuckDB plugin if some dashboards still need it (optional)
      - GF_INSTALL_PLUGINS=https://github.com/motherduckdb/grafana-duckdb-datasource/releases/download/v0.2.1/motherduck-duckdb-datasource-0.2.1.zip;motherduck-duckdb-datasource
    restart: unless-stopped
    depends_on:
      # CHANGED: Now depends on TimescaleDB instead of DuckDB
      timescaledb:
        condition: service_healthy
      air-quality-app:
        condition: service_healthy
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
```

### New Data Source Provisioning

Create `config/grafana/provisioning/datasources/timescaledb.yaml`:

```yaml
apiVersion: 1

datasources:
  - name: NDP-TimescaleDB
    type: postgres
    url: pi5-timescaledb:5432
    database: ndp
    user: grafana_reader
    secureJsonData:
      password: grafana_read_only
    jsonData:
      sslmode: disable
      maxOpenConns: 10
      connMaxLifetime: 14400
      postgresVersion: 1500
      timescaledb: true
    isDefault: true
    editable: false
```

---

## 8. Memory Budget Comparison

### Before DP-002

| Service | Memory Limit | Notes |
|---------|--------------|-------|
| mosquitto | 128MB | MQTT broker |
| etcd | 256MB | Configuration |
| air-quality-app | 512MB | Ingestion |
| duckdb | 512MB | Silver layer |
| grafana | 256MB | Dashboards |
| **Total** | **1,664MB** | |

### After DP-002

| Service | Memory Limit | Notes |
|---------|--------------|-------|
| mosquitto | 128MB | MQTT broker (unchanged) |
| etcd | 256MB | Configuration (unchanged) |
| air-quality-app | 512MB | Ingestion (unchanged) |
| timescaledb | 256MB | Silver layer + Data Dictionary |
| grafana | 256MB | Dashboards (unchanged) |
| **Total** | **1,408MB** | 256MB reduction |

**Benefit**: 256MB memory freed by replacing DuckDB with TimescaleDB.

---

## 9. Network Configuration

No changes to network configuration required. All services continue to use the `neural-network` bridge network.

```yaml
networks:
  default:
    name: neural-network
    driver: bridge
```

### Service Discovery

| Service | DNS Name | Port |
|---------|----------|------|
| TimescaleDB | pi5-timescaledb | 5432 |
| MQTT | mqtt-broker | 1883 |
| etcd | etcd | 2379 |
| Grafana | pi5-grafana | 3000 |

---

## 10. Complete Updated docker-compose.yml

```yaml
# Production Docker Compose for Raspberry Pi 5
# Neural Data Platform - Air Quality Monitoring Stack
# Updated for DP-002: Data Dictionary with TimescaleDB
#
# Usage:
#   docker compose up -d          # Start all services
#   docker compose logs -f        # View logs
#   docker compose down           # Stop all services

services:
  # MQTT Broker - receives data from AirGradient sensors
  mosquitto:
    image: eclipse-mosquitto:2.0
    container_name: mqtt-broker
    ports:
      - "1883:1883"     # MQTT
      - "9001:9001"     # WebSocket (optional)
    volumes:
      - ./mosquitto/mosquitto.conf:/mosquitto/config/mosquitto.conf:ro
      - mosquitto-data:/mosquitto/data
      - mosquitto-logs:/mosquitto/log
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "mosquitto_sub", "-t", "$$SYS/#", "-C", "1", "-i", "healthcheck", "-W", "3"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 10s
    deploy:
      resources:
        limits:
          memory: 128M

  # etcd - Configuration Store
  etcd:
    image: quay.io/coreos/etcd:v3.5.11
    container_name: etcd
    ports:
      - "2379:2379"
    environment:
      - ETCD_NAME=etcd0
      - ETCD_DATA_DIR=/etcd-data
      - ETCD_LISTEN_CLIENT_URLS=http://0.0.0.0:2379
      - ETCD_ADVERTISE_CLIENT_URLS=http://etcd:2379
      - ETCD_LISTEN_PEER_URLS=http://0.0.0.0:2380
      - ETCD_INITIAL_ADVERTISE_PEER_URLS=http://etcd:2380
      - ETCD_INITIAL_CLUSTER=etcd0=http://etcd:2380
      - ETCD_INITIAL_CLUSTER_TOKEN=neural-cluster
      - ETCD_INITIAL_CLUSTER_STATE=new
      - ETCD_QUOTA_BACKEND_BYTES=536870912
    volumes:
      - etcd-data:/etcd-data
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "etcdctl", "endpoint", "health"]
      interval: 30s
      timeout: 10s
      retries: 5
    deploy:
      resources:
        limits:
          memory: 256M

  # Air Quality App - Multi-Stream Ingestion (MQTT + HTTP Polling)
  air-quality-app:
    build:
      context: ../..
      dockerfile: Dockerfile
    image: neural-data-platform/air-quality-app:latest
    container_name: air-quality-app
    ports:
      - "8080:8080"     # HTTP API
    volumes:
      - air-quality-data:/data
    environment:
      - RUST_LOG=info
      - DATA_DIR=/data
      - ETCD_ENDPOINT=http://etcd:2379
      - MQTT_BROKER_URL=mosquitto
      - MQTT_PORT=1883
      - OPENWEATHERMAP_API_KEY=${OPENWEATHERMAP_API_KEY}
      - WEATHER_LATITUDE=${WEATHER_LATITUDE:-29.95838}
      - WEATHER_LONGITUDE=${WEATHER_LONGITUDE:--81.30878}
    depends_on:
      mosquitto:
        condition: service_healthy
      etcd:
        condition: service_healthy
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 30s
    deploy:
      resources:
        limits:
          memory: 512M

  # TimescaleDB - Silver Layer + Data Dictionary (DP-002)
  timescaledb:
    image: timescale/timescaledb:latest-pg15
    container_name: pi5-timescaledb
    ports:
      - "127.0.0.1:5432:5432"    # Localhost only for security
    environment:
      - POSTGRES_USER=postgres
      - POSTGRES_PASSWORD=${POSTGRES_PASSWORD:-ndp_secure_password}
      - POSTGRES_DB=ndp
      - TIMESCALEDB_TELEMETRY=off
    volumes:
      - timescaledb-data:/var/lib/postgresql/data
      - ./init-scripts:/docker-entrypoint-initdb.d:ro
    restart: unless-stopped
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres -d ndp"]
      interval: 30s
      timeout: 10s
      retries: 5
      start_period: 30s
    deploy:
      resources:
        limits:
          memory: 256M
        reservations:
          memory: 128M

  # Grafana - Data Visualization & Dashboards (DP-001, DP-002)
  grafana:
    image: grafana/grafana:latest-ubuntu
    container_name: pi5-grafana
    ports:
      - "3000:3000"
    volumes:
      - grafana-data:/var/lib/grafana
      - ../../config/grafana/grafana.ini:/etc/grafana/grafana.ini:ro
      - ../../config/grafana/provisioning:/etc/grafana/provisioning:ro
      - ../../config/grafana/dashboards:/var/lib/grafana/dashboards:ro
      - air-quality-data:/data:ro
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=${GRAFANA_ADMIN_PASSWORD:-admin}
      - GF_USERS_ALLOW_SIGN_UP=false
      - GF_DATABASE_TYPE=sqlite3
      - GF_DATABASE_PATH=/var/lib/grafana/grafana.db
      - GF_SERVER_ROOT_URL=http://localhost:3000
      - GF_LOG_LEVEL=info
      # Keep DuckDB plugin for existing dashboards (optional, can be removed later)
      - GF_INSTALL_PLUGINS=https://github.com/motherduckdb/grafana-duckdb-datasource/releases/download/v0.2.1/motherduck-duckdb-datasource-0.2.1.zip;motherduck-duckdb-datasource
    restart: unless-stopped
    depends_on:
      timescaledb:
        condition: service_healthy
      air-quality-app:
        condition: service_healthy
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

volumes:
  mosquitto-data:
    driver: local
  mosquitto-logs:
    driver: local
  etcd-data:
    driver: local
  air-quality-data:
    driver: local
  timescaledb-data:
    driver: local
  grafana-data:
    driver: local

networks:
  default:
    name: neural-network
    driver: bridge
```

---

## 11. Migration Steps

### Step 1: Backup Current Data

```bash
# Backup DuckDB data (if needed)
docker exec duckdb /duckdb /var/duckdb/neural_platform.db ".backup /var/duckdb/backup.db"
docker cp duckdb:/var/duckdb/backup.db ./duckdb-backup.db

# Backup Grafana dashboards
docker cp grafana:/var/lib/grafana/dashboards ./grafana-backup/
```

### Step 2: Create Init Scripts

```bash
mkdir -p deploy/pi/init-scripts
# Create SQL files as documented above
```

### Step 3: Update docker-compose.yml

```bash
# Apply the changes documented in this file
# Or use the complete updated docker-compose.yml
```

### Step 4: Redeploy

```bash
cd deploy/pi

# Stop current stack
docker compose down

# Remove DuckDB volume (after confirming backup)
docker volume rm pi_duckdb-data

# Start new stack
docker compose up -d

# Verify TimescaleDB is healthy
docker compose ps
docker exec pi5-timescaledb psql -U postgres -d ndp -c "SELECT * FROM data_dictionary.streams;"
```

### Step 5: Run Initial Sync

```bash
./deploy.sh sync
```

### Step 6: Verify Dashboard

1. Open Grafana at http://localhost:3000
2. Navigate to Data Quality dashboard
3. Verify stream overview shows synced data

---

## 12. Rollback Procedure

If issues occur:

```bash
# Stop stack
docker compose down

# Restore original docker-compose.yml from git
git checkout deploy/pi/docker-compose.yml

# Restore DuckDB backup if needed
docker volume create pi_duckdb-data
docker run --rm -v pi_duckdb-data:/var/duckdb -v $(pwd):/backup busybox cp /backup/duckdb-backup.db /var/duckdb/neural_platform.db

# Restart original stack
docker compose up -d
```

---

## 13. Related Documents

| Document | Description |
|----------|-------------|
| [ADR-001-TIMESCALEDB-SCHEMA.md](./ADR-001-TIMESCALEDB-SCHEMA.md) | Database schema |
| [ADR-003-SYNC-MECHANISM.md](./ADR-003-SYNC-MECHANISM.md) | Sync implementation |
| [ADR-004-DQ-DASHBOARD.md](./ADR-004-DQ-DASHBOARD.md) | Dashboard architecture |
| [SYSTEM_DESIGN.md](./SYSTEM_DESIGN.md) | Overall system design |

---

**Last Updated**: 2025-12-30
**Next Review**: After deployment to production Pi
