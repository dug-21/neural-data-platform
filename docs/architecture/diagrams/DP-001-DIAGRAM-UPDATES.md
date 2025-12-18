# DP-001: C4 Diagram Updates Required

**Document**: `neural-data-platform-c4.drawio`
**Feature**: DP-001 - Silver Layer Query Infrastructure
**Date**: 2025-12-18

## Overview

This document describes the updates needed to the C4 architecture diagram to reflect the DP-001 Silver Layer implementation with DuckDB and Grafana.

## Containers to Add

### 1. DuckDB Container

**Type**: Container (Analytics Engine)
**Technology**: marcboeker/duckdb-http (Docker)
**Description**: Virtual Silver Layer - provides SQL query interface over Bronze Parquet files

**Properties**:
- Port: 9090 (HTTP REST API)
- Memory: 512MB limit
- CPU: 2 cores max
- Access Mode: Read-only to Bronze layer

**Connections**:
- **FROM**: Grafana container → HTTP REST queries
- **TO**: air-quality-data volume → Read Parquet files (read-only mount)
- **TO**: duckdb-data volume → Store database catalog
- **TO**: config/duckdb volume → Load SQL view definitions

**Responsibilities**:
1. Execute SQL queries from Grafana
2. Read Parquet files from Bronze layer
3. Apply data quality rules via SQL views
4. Return time-series result sets
5. Provide health check endpoint

### 2. Grafana Container

**Type**: Container (Visualization)
**Technology**: Grafana OSS (Docker)
**Description**: Real-time analytics dashboards for air quality monitoring

**Properties**:
- Port: 3000 (Web UI)
- Memory: 256MB limit
- CPU: 1 core max
- Authentication: Anonymous viewer enabled

**Connections**:
- **FROM**: Web Browser → HTTPS :3000
- **TO**: DuckDB container → SQL queries via datasource plugin
- **TO**: grafana-data volume → Store dashboard edits
- **TO**: config/grafana volume → Load provisioning configs

**Responsibilities**:
1. Render dashboards (4 provisioned)
2. Execute queries against DuckDB datasource
3. Cache query results (5-minute TTL)
4. Auto-refresh panels (5-minute interval)
5. Provide time range selection

## Components to Add

### DuckDB SQL Views (Component within DuckDB Container)

**Type**: Component (Data Layer)
**Technology**: SQL View Definitions
**Location**: `config/duckdb/views/*.sql`

**Views**:
1. `silver_indoor_air` - AirGradient sensor data with DQ filtering
2. `silver_outdoor_weather` - OpenWeatherMap weather data with DQ filtering
3. `silver_outdoor_air` - OpenWeatherMap air quality with DQ filtering
4. `cross_stream_aligned` - 10-minute time-bucketed cross-stream JOIN

**Purpose**: Apply data quality rules at query time without ETL

### Grafana Dashboards (Component within Grafana Container)

**Type**: Component (UI Layer)
**Technology**: Grafana Dashboard JSON
**Location**: `config/grafana/dashboards/*.json`

**Dashboards**:
1. `indoor-air-quality.json` - Indoor sensor readings
2. `outdoor-weather.json` - Weather conditions
3. `outdoor-air-quality.json` - AQI and pollutants
4. `indoor-vs-outdoor.json` - Comparative analysis

**Purpose**: Visualize time-series data from Silver layer

## Data Flow Updates

### New Flow: Bronze → DuckDB → Grafana → User

```
┌─────────────────────────────────────────────────────────────┐
│ Bronze Layer (Parquet Files)                                 │
│ /data/air-quality/*.parquet                                  │
│ /data/outdoor-weather/*.parquet                              │
│ /data/outdoor-air-quality/*.parquet                          │
└─────────────────────────────────────────────────────────────┘
                        │
                        │ read_parquet() - read-only mount
                        ▼
┌─────────────────────────────────────────────────────────────┐
│ DuckDB Container                                             │
│                                                              │
│  ┌───────────────────────────────────────────────────────┐ │
│  │ SQL Views (Virtual Silver Layer)                      │ │
│  │ - silver_indoor_air                                   │ │
│  │ - silver_outdoor_weather                              │ │
│  │ - silver_outdoor_air                                  │ │
│  │ - cross_stream_aligned                                │ │
│  └───────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                        │
                        │ HTTP :9090 (SQL over REST)
                        ▼
┌─────────────────────────────────────────────────────────────┐
│ Grafana Container                                            │
│                                                              │
│  ┌───────────────────────────────────────────────────────┐ │
│  │ Dashboards                                            │ │
│  │ - Indoor Air Quality                                  │ │
│  │ - Outdoor Weather                                     │ │
│  │ - Outdoor AQI                                         │ │
│  │ - Indoor vs Outdoor                                   │ │
│  └───────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                        │
                        │ HTTP :3000 (Web UI)
                        ▼
                   Web Browser
```

## Volume Updates

Add these volumes to the diagram:

1. **duckdb-data**
   - Type: Docker Named Volume
   - Purpose: Persistent DuckDB catalog
   - Size: ~100MB
   - Mounted to: DuckDB container at `/duckdb`

2. **grafana-data**
   - Type: Docker Named Volume
   - Purpose: Grafana dashboard storage
   - Size: ~1GB limit
   - Mounted to: Grafana container at `/var/lib/grafana`

3. **config/duckdb** (Host Mount)
   - Type: Bind Mount (read-only)
   - Purpose: SQL view definitions
   - Mounted to: DuckDB container at `/config/duckdb`

4. **config/grafana** (Host Mount)
   - Type: Bind Mount (read-only)
   - Purpose: Provisioning configs and dashboards
   - Mounted to: Grafana container at `/etc/grafana`

## Dependency Chain Updates

Update the container startup sequence:

```
mosquitto + etcd (parallel start)
        ↓
  air-quality-app (depends on mosquitto + etcd)
        ↓
      duckdb (depends on air-quality-app - waits for Parquet files)
        ↓
     grafana (depends on duckdb health check)
```

## C4 Levels to Update

### Level 1: System Context
- Add "Data Analyst" persona (uses Grafana dashboards)
- Show Grafana as external interface for visualization

### Level 2: Container Diagram
- Add DuckDB container
- Add Grafana container
- Show data flow: Bronze → DuckDB → Grafana → Browser
- Update resource allocation totals (1664MB limit, ~750MB actual)

### Level 3: Component Diagram (for DuckDB)
- Show SQL View components
- Show HTTP API interface
- Show Parquet reader component
- Show query optimizer

### Level 3: Component Diagram (for Grafana)
- Show Dashboard components (4 dashboards)
- Show DuckDB datasource plugin
- Show Query editor
- Show Panel renderers

## Annotations to Add

1. **DP-001 Feature Boundary**
   - Draw a boundary box around DuckDB + Grafana containers
   - Label: "DP-001: Silver Layer Query Infrastructure"
   - Color: Distinguish from AIR-00X features

2. **Performance Metrics**
   - Annotate DuckDB: "Query Latency: 7-day < 1s"
   - Annotate Grafana: "4 Dashboards, 5-min auto-refresh"

3. **Resource Allocation**
   - Annotate total memory: "1664MB limit (10.4% of 16GB Pi)"
   - Annotate network: "neural-network bridge (172.28.0.0/16)"

## Notes for Diagram Tool

Since `neural-data-platform-c4.drawio` is a Draw.io XML file:

1. Open in Draw.io desktop or web editor
2. Add new pages for DP-001 views if helpful
3. Use consistent color scheme:
   - Bronze layer: Brown/tan
   - Silver layer: Silver/gray
   - Visualization: Blue
4. Use C4 notation:
   - Rectangle with rounded corners for containers
   - Nested rectangles for components
   - Dashed lines for read-only connections
   - Solid lines for read-write

## Verification

After updating the diagram:
1. Export PNG version for quick reference
2. Verify all containers from docker-compose.yml are shown
3. Verify all volume mounts are documented
4. Verify dependency chain matches `depends_on` clauses
5. Verify resource limits match docker-compose.yml

---

**Action Required**: Update `neural-data-platform-c4.drawio` with the above elements using Draw.io editor.
