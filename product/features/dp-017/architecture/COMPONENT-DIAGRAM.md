# dp-017: Component Diagram

## Integration Environment Architecture

This document shows the relationship between deployment components in the integration test environment.

---

## High-Level Flow

```
+---------------------------+
|     Developer/CI          |
+-------------+-------------+
              |
              | DEPLOY_ENV=integration
              v
+---------------------------+
|   deploy/pi/deploy.sh     |  <-- Single entry point
+-------------+-------------+
              |
              | Selects compose file based on DEPLOY_ENV
              v
+-----------------------------+     +-----------------------------+
| docker-compose.integration  |     | deploy/pi/docker-compose    |
|        .yml                 |     |        .yml                 |
| (root level)                |     | (production)                |
+-------------+---------------+     +-------------+---------------+
              |                                   |
              | DEPLOY_ENV=integration            | DEPLOY_ENV=pi
              v                                   v
+-------------+---------------+     +-------------+---------------+
| integration-* containers    |     | production containers       |
+-----------------------------+     +-----------------------------+
```

---

## deploy.sh Command Flow

```
deploy/pi/deploy.sh
    |
    +-- Environment Detection
    |       |
    |       +-- DEPLOY_ENV=integration --> docker-compose.integration.yml
    |       +-- DEPLOY_ENV=pi (default) --> deploy/pi/docker-compose.yml
    |
    +-- Container Name Resolution
    |       |
    |       +-- integration: ETCD_CONTAINER=integration-etcd
    |       +-- pi: ETCD_CONTAINER=etcd
    |
    +-- Commands
            |
            +-- deploy/start/stop/status/build
            |       Uses: docker compose -f $COMPOSE_FILE
            |
            +-- sync
            |       Calls: scripts/sync-config-to-etcd.sh
            |       Uses: ETCD_CONTAINER variable
            |
            +-- init-streams
            |       Calls: deploy/pi/configs/streams/init-streams.sh
            |       Uses: ETCD_CONTAINER variable
            |
            +-- sync-dictionary
            |       Calls: internal sync_to_data_dictionary()
            |       Uses: docker compose exec timescaledb
            |
            +-- sync-dimensions
            |       Calls: internal sync_dimensions()
            |       Uses: docker compose exec timescaledb
            |
            +-- silver-migrate/silver-etl
                    Uses: docker compose --profile silver run
```

---

## Compose File Comparison

```
docker-compose.integration.yml         deploy/pi/docker-compose.yml
(repo root)                            (deploy/pi/)
================================       ================================

Services:                              Services:
  mosquitto                              mosquitto
  etcd                                   etcd
  timescaledb                            timescaledb
  air-quality-app                        air-quality-app
  ndp-mcp-server                         ndp-mcp-server
  grafana (profile: dashboards)          grafana
                                         silver-etl (profile: silver)
                                         silver-etl-daemon (profile: silver-daemon)

Container Names:                       Container Names:
  integration-mosquitto                  mqtt-broker
  integration-etcd                       etcd
  integration-timescaledb                pi5-timescaledb
  integration-air-quality                air-quality-app
  integration-mcp-server                 ndp-mcp-server
  integration-grafana                    grafana

Network:                               Network:
  integration-network                    neural-network

Init Scripts Volume:                   Init Scripts Volume:
  ./deploy/pi/init-scripts               ./init-scripts
  (relative to repo root)                (relative to deploy/pi/)

Config Volume:                         Config Volume:
  ./config/base/streams                  ../../config/base/streams
  (relative to repo root)                (relative to deploy/pi/)
```

---

## Shared Configuration Components

```
Repository Root
    |
    +-- config/
    |       |
    |       +-- base/
    |       |       |
    |       |       +-- streams/                  <-- Stream configs (YAML/JSON)
    |       |       |       |
    |       |       |       +-- air-quality/
    |       |       |       +-- outdoor-weather/
    |       |       |       +-- nws-observations/
    |       |       |
    |       |       +-- dimensions/               <-- Dimension table configs
    |       |       +-- processors/               <-- Threshold alert configs
    |       |
    |       +-- grafana/
    |               |
    |               +-- provisioning/             <-- Datasources, dashboards
    |               +-- dashboards/               <-- Dashboard JSON files
    |
    +-- deploy/pi/
    |       |
    |       +-- init-scripts/                     <-- TimescaleDB init SQL
    |       |       |
    |       |       +-- 001_silver_schema.sql
    |       |       +-- 002_data_dictionary.sql
    |       |       +-- 003_etl_runs.sql
    |       |       +-- 004_grafana_access.sql
    |       |
    |       +-- configs/streams/
    |       |       |
    |       |       +-- init-streams.sh           <-- Stream initialization
    |       |       +-- list-streams.sh           <-- Stream listing
    |       |
    |       +-- mosquitto/
    |               |
    |               +-- mosquitto.conf            <-- MQTT broker config
    |
    +-- scripts/
            |
            +-- sync-config-to-etcd.sh            <-- Config sync to etcd
            +-- integration-test.sh               <-- Test harness
```

---

## External Script Dependencies

### sync-config-to-etcd.sh

```
Inputs:
  - ETCD_CONTAINER (from deploy.sh)
  - ETCD_ENDPOINT
  - CONFIG_DIR (default: ./config)
  - ENVIRONMENT (development/production)

Reads:
  - config/base/streams/*/config.yaml
  - config/overlays/$ENVIRONMENT/streams/*/config.yaml

Writes to:
  - etcd: /streams/{stream-id}/*

Uses:
  - python3 (YAML parsing)
  - docker exec $ETCD_CONTAINER etcdctl
```

### init-streams.sh

```
Inputs:
  - $1: ETCD_CONTAINER name

Reads:
  - Hardcoded stream definitions (airgradient-001, airgradient-002)

Writes to:
  - etcd: /air-quality/streams/{stream-id}/*
  - etcd: /air-quality/multi_stream/*

Uses:
  - docker exec $ETCD_CONTAINER etcdctl
```

### sync_to_data_dictionary() (internal to deploy.sh)

```
Inputs:
  - $REPO_ROOT/config/base/streams/*/config.yaml

Reads:
  - Stream configs (entity_schemas, silver_etl)

Writes to:
  - TimescaleDB: data_dictionary.streams
  - TimescaleDB: data_dictionary.entity_schemas
  - TimescaleDB: data_dictionary.entity_schema_attributes
  - TimescaleDB: data_dictionary.silver_tables
  - TimescaleDB: data_dictionary.silver_columns
  - TimescaleDB: data_dictionary.silver_lineage
  - TimescaleDB: data_dictionary.silver_dq_rules

Uses:
  - yaml_get, yaml_array_len, yaml_array_get helpers
  - docker compose exec timescaledb psql
```

---

## Network Topology (Integration)

```
+-------------------+
|   Host Network    |
|                   |
|  localhost:1883 -----> mosquitto (MQTT)
|  localhost:2379 -----> etcd (Config)
|  localhost:5432 -----> timescaledb (Silver)
|  localhost:8080 -----> air-quality-app (Bronze/API)
|  localhost:9100 -----> ndp-mcp-server (MCP)
|  localhost:3000 -----> grafana (Dashboards)
|                   |
+-------------------+
         |
         v
+-------------------+
| integration-network |
| (Docker bridge)    |
|                    |
| Services discover  |
| each other by name:|
|                    |
| mosquitto:1883     |
| etcd:2379          |
| timescaledb:5432   |
+--------------------+
```

---

## Data Flow (Integration)

```
External Test Data
    |
    | mosquitto_pub -h localhost -t "airgradient/..."
    v
+-------------------+
|    mosquitto      |
|  (MQTT Broker)    |
+--------+----------+
         |
         | MQTT subscription
         v
+-------------------+
| air-quality-app   |
| (Bronze + ETL)    |
+--------+----------+
         |
         +----------------+
         |                |
         v                v
+--------+--------+ +-----+---------+
| /data/raw/      | | timescaledb   |
| (Bronze Parquet)| | (Silver SQL)  |
+-----------------+ +-----+---------+
                          |
                          | Query via
                          | PostgreSQL protocol
                          v
                   +------+--------+
                   | ndp-mcp-server|
                   | (MCP tools)   |
                   +---------------+
```

---

## Key Relationships

| Component | Depends On | Purpose |
|-----------|-----------|---------|
| deploy.sh | docker compose | Container orchestration |
| sync | etcd, sync-config-to-etcd.sh | Config to runtime cache |
| init-streams | etcd, init-streams.sh | Stream registry population |
| sync-dictionary | timescaledb, config files | Data dictionary metadata |
| air-quality-app | mosquitto, etcd, timescaledb | Bronze ingestion + Silver ETL |
| ndp-mcp-server | etcd, timescaledb | MCP interface to data |
| grafana | timescaledb | Visualization |

---

## Related Documents

- `ADR-017-001-integration-environment-design.md` - Topology parity principles
- `ADR-017-002-test-harness-strategy.md` - Testing approach
- `product/features/dp-017/SCOPE.md` - Feature scope
