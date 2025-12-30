# Database Comparison: InfluxDB vs DuckDB/TimescaleDB for IoT Analytics

**Date**: 2025-12-30
**Author**: NDP Research Agent
**Status**: Analysis Complete

---

## Executive Summary

This analysis compares Home Assistant's InfluxDB addon with NDP's DuckDB/TimescaleDB architecture for IoT time-series analytics. The conclusion is that **these systems are complementary, not competing**, serving different layers of the data analytics pipeline.

| Aspect | HA InfluxDB | NDP DuckDB | NDP TimescaleDB (Planned) |
|--------|-------------|------------|---------------------------|
| **Primary Role** | Real-time visualization | Analytical queries | ML feature engineering |
| **Query Latency** | Sub-second | Sub-second to seconds | Sub-second |
| **Data Retention** | Days to weeks | Weeks to months | Months to years |
| **Use Case** | Dashboards, alerts | Cross-stream analytics | Predictions, ML training |
| **Complexity** | Low (HA addon) | Medium (virtual views) | Higher (ETL pipeline) |

**Recommendation**: Use InfluxDB for Home Assistant visualization, DuckDB for Bronze/Silver analytics, with optional TimescaleDB for Gold layer ML features. Data flows: HA -> InfluxDB (display) AND HA -> NDP (analytics).

---

## 1. InfluxDB (Home Assistant Addon)

### 1.1 What InfluxDB Does Well

InfluxDB is a purpose-built time-series database optimized for:

**Real-Time Ingestion**
- Write-optimized for continuous sensor data
- High throughput ingestion (hundreds of thousands of points/second)
- Built-in retention policies with automatic expiration
- Line protocol is extremely efficient for IoT data

**Fast Dashboard Queries**
- Sub-millisecond queries for recent data (last hour/day)
- Continuous queries for pre-aggregated rollups
- Native time-series functions (derivative, moving_average, etc.)
- Tight Grafana integration via native plugin

**Home Assistant Integration**
- Official addon with zero-config setup
- Automatic entity discovery and ingestion
- Native support for HA's state/event model
- Pre-built Grafana dashboards available

### 1.2 InfluxDB Limitations

**Short-Term Focus**
- Performance degrades with large historical datasets
- Cardinality issues with many entities/tags
- Not designed for complex analytical queries
- Limited JOIN capabilities across measurements

**Query Language (Flux/InfluxQL)**
- Learning curve for complex analytics
- Less expressive than SQL for ad-hoc analysis
- Limited window functions compared to SQL
- Difficult to correlate across different data sources

**ML/Analytics Gaps**
- No native ML integration
- Limited statistical functions
- Not suitable for training data export
- Cross-entity correlation is cumbersome

### 1.3 Typical HA InfluxDB Configuration

```yaml
# Home Assistant InfluxDB addon
default_measurement: state
include:
  domains:
    - sensor
    - binary_sensor
    - climate
    - weather
retention_policy: 7d  # Common for home use
```

**Storage Footprint**: ~100MB-1GB for typical home (depending on polling frequency)

---

## 2. NDP DuckDB Architecture (Current)

### 2.1 Virtual Silver Layer Design

NDP uses DuckDB as a **query layer** over Bronze Parquet files, not as a primary storage engine:

```
Bronze Layer (Parquet)              Silver Layer (DuckDB Views)
-----------------------             ---------------------------
/data/air-quality/*.parquet    -->  silver_indoor_air
/data/outdoor-weather/*.parquet --> silver_outdoor_weather
/data/home-events/*.parquet    -->  silver_entity_state
                                    cross_stream_aligned
```

**Key Characteristics**:
- **No ETL**: Views query Parquet directly at read time
- **No Duplication**: Single source of truth in Parquet
- **Zero Staleness**: Always queries latest data
- **SQL Interface**: Full analytical SQL with window functions

### 2.2 What DuckDB Does Well

**Analytical Queries**
- Full SQL:2016 support including window functions
- Efficient columnar scanning of Parquet files
- Partition pruning for time-range queries
- Complex JOINs across different data streams

**Cross-Stream Correlation**
```sql
-- Example: Correlate indoor air quality with outdoor weather
SELECT
  time_bucket(INTERVAL '10 minutes', timestamp) AS bucket,
  AVG(indoor.pm25) AS avg_indoor_pm25,
  AVG(outdoor.temperature) AS avg_outdoor_temp,
  AVG(outdoor.humidity) AS avg_outdoor_humidity
FROM silver_indoor_air indoor
JOIN silver_outdoor_weather outdoor
  ON time_bucket(INTERVAL '10 minutes', indoor.timestamp) =
     time_bucket(INTERVAL '10 minutes', outdoor.timestamp)
GROUP BY bucket
ORDER BY bucket;
```

**Edge Deployment**
- Single binary, no external dependencies
- 512MB memory limit sufficient for Pi 5
- Query latency: <500ms for 24h, <2s for 7 days
- Embedded in HTTP container (marcboeker/duckdb-http)

### 2.3 DuckDB Limitations

**Not a Primary Database**
- Read-optimized, not write-optimized
- No native ingestion pipeline (relies on Parquet files)
- No built-in replication or high availability
- Views recompute on every query (no materialized views yet)

**Resource Constraints**
- Memory usage scales with query complexity
- Large historical queries may exceed Pi memory
- No native streaming/real-time capabilities

---

## 3. TimescaleDB (Planned Silver/Gold Layer)

### 3.1 Why TimescaleDB for NDP

TimescaleDB is planned for the Silver/Gold layer when NDP needs:

**Materialized Continuous Aggregates**
```sql
-- Auto-updated hourly rollups
CREATE MATERIALIZED VIEW hourly_air_quality
WITH (timescaledb.continuous) AS
SELECT
  time_bucket('1 hour', timestamp) AS bucket,
  location_id,
  AVG(pm25) AS avg_pm25,
  MAX(pm25) AS max_pm25,
  percentile_cont(0.95) WITHIN GROUP (ORDER BY pm25) AS p95_pm25
FROM bronze_air_quality
GROUP BY bucket, location_id;
```

**ML Feature Engineering**
- Complex window functions for feature extraction
- Time-weighted averages and gap filling
- Statistical aggregates (percentiles, stddev)
- Native integration with Python/pandas

**Long-Term Storage**
- Hypertable compression (10-20x reduction)
- Automated tiering (hot/warm/cold)
- Months-to-years retention for ML training
- Efficient historical queries

### 3.2 TimescaleDB vs DuckDB Tradeoffs

| Aspect | DuckDB | TimescaleDB |
|--------|--------|-------------|
| **Setup Complexity** | Low (container) | Medium (Postgres) |
| **Memory Usage** | ~250MB | ~500MB+ |
| **Write Performance** | N/A (read-only) | High |
| **Materialized Views** | No | Yes (continuous) |
| **ML Integration** | Limited | Strong (pgml, psycopg) |
| **Compression** | Parquet native | Built-in columnar |

---

## 4. Comparison Matrix

### 4.1 Feature Comparison

| Feature | InfluxDB (HA) | DuckDB (NDP) | TimescaleDB (Planned) |
|---------|--------------|--------------|----------------------|
| **Write Throughput** | Excellent | N/A | Excellent |
| **Read Latency (recent)** | Excellent | Good | Excellent |
| **Read Latency (historical)** | Poor | Good | Excellent |
| **SQL Support** | Limited | Full | Full + Extensions |
| **Window Functions** | Basic | Full | Full |
| **JOINs** | Limited | Full | Full |
| **Compression** | Good | Excellent (Parquet) | Excellent |
| **Continuous Aggregates** | Yes (basic) | No | Yes (advanced) |
| **ML Integration** | None | Limited | Strong |
| **Edge Deployment** | Yes | Yes | Possible (more memory) |

### 4.2 Use Case Fit

| Use Case | Best Choice | Why |
|----------|-------------|-----|
| Real-time dashboards | InfluxDB | Sub-ms queries, HA integration |
| Historical trend analysis | DuckDB | Efficient Parquet scanning |
| Cross-stream correlation | DuckDB | Full SQL JOIN support |
| ML feature extraction | TimescaleDB | Continuous aggregates, percentiles |
| Anomaly detection training | TimescaleDB | Long-term storage, statistical functions |
| Simple alerting | InfluxDB | Built-in threshold checks |
| Complex pattern detection | DuckDB/TimescaleDB | Window functions, SQL |

---

## 5. Data Architecture Recommendation

### 5.1 Complementary Architecture

The recommended architecture uses **both InfluxDB and NDP**, serving different purposes:

```
                        Home Assistant
                              |
              +---------------+---------------+
              |                               |
              v                               v
        InfluxDB (HA Addon)           NDP (Neural Data Platform)
              |                               |
              |                               v
              |                   +---------------------------+
              |                   |    Bronze Layer (Parquet)  |
              |                   |   - air-quality/*.parquet  |
              |                   |   - home-events/*.parquet  |
              |                   +---------------------------+
              |                               |
              |                               v
              |                   +---------------------------+
              |                   |    Silver Layer (DuckDB)   |
              |                   |   - Virtual views          |
              |                   |   - Cross-stream alignment |
              |                   +---------------------------+
              |                               |
              v                               v
        +----------+              +---------------------------+
        |  Grafana |              |   Gold Layer (TimescaleDB) |
        | (Real-   |              |   - ML features            |
        |  Time)   |              |   - Continuous aggregates  |
        +----------+              +---------------------------+
                                              |
                                              v
                                  +---------------------------+
                                  |    Prediction Layer        |
                                  |   - ruv-FANN models        |
                                  |   - Forecasting            |
                                  +---------------------------+
```

### 5.2 Data Flow Pattern

**Path 1: Real-Time Visualization (InfluxDB)**
```
HA Entity State Change
    -> InfluxDB (immediate write)
    -> Grafana Dashboard (sub-second refresh)
    -> Alerting (threshold-based)
```

**Path 2: Analytics & ML (NDP)**
```
HA Entity State Change
    -> NDP MQTT/Webhook Source
    -> Bronze Layer (Parquet, append-only)
    -> Silver Layer (DuckDB views, on-demand)
    -> Gold Layer (TimescaleDB, scheduled)
    -> ML Training/Inference
```

### 5.3 When to Use Which

| Scenario | Use InfluxDB | Use NDP |
|----------|--------------|---------|
| "What's the current temperature?" | Yes | No |
| "Show me the last 24 hours" | Yes | Yes |
| "Correlate PM2.5 with window state" | No | Yes |
| "When should I open windows?" | No | Yes (ML) |
| "Alert if CO2 > 1000ppm" | Yes | No |
| "Predict tomorrow's air quality" | No | Yes |
| "What patterns precede high PM2.5?" | No | Yes |

---

## 6. Integration Strategies

### 6.1 Option A: Parallel Ingestion (Recommended)

Both systems receive data independently:

```yaml
# Home Assistant configuration.yaml
influxdb:
  host: influxdb
  database: homeassistant
  include:
    domains: [sensor, binary_sensor]

# NDP receives same data via MQTT/webhook
# No dependency between systems
```

**Pros**:
- Complete independence
- Either can fail without affecting the other
- Optimal for each use case

**Cons**:
- Data duplication
- More storage (but storage is cheap)

### 6.2 Option B: InfluxDB as Source

NDP queries InfluxDB for historical data:

```yaml
# NDP stream config
stream_id: ha-influxdb
sources:
  - source_type: HttpPoll
    params:
      url: "http://influxdb:8086/query"
      parser: influxdb
      interval_seconds: 60
```

**Pros**:
- Single ingestion path
- InfluxDB handles real-time

**Cons**:
- Dependency on InfluxDB availability
- Query overhead for historical data
- InfluxDB retention limits NDP data

### 6.3 Option C: NDP Primary, InfluxDB Secondary

NDP is the source of truth, InfluxDB for dashboards only:

```yaml
# NDP exports to InfluxDB for visualization
# Custom export service (not recommended)
```

**Pros**:
- Single source of truth

**Cons**:
- Added complexity
- InfluxDB loses native HA integration

---

## 7. Practical Recommendations

### 7.1 For Home Users

If you have Home Assistant with InfluxDB addon already:

1. **Keep InfluxDB** for real-time dashboards and simple alerts
2. **Add NDP** for advanced analytics when you need:
   - Cross-stream correlation (indoor vs outdoor)
   - ML predictions (when to ventilate)
   - Long-term pattern analysis
   - Custom feature engineering

### 7.2 For Power Users / ML Focus

If ML and predictions are the primary goal:

1. **Use NDP as primary** with Bronze/Silver/Gold architecture
2. **InfluxDB optional** for real-time HA dashboards
3. **Focus on TimescaleDB** for feature engineering
4. **Train ruv-FANN models** on historical patterns

### 7.3 For Resource-Constrained Edge

If running on Raspberry Pi with limited resources:

1. **Choose one primary**: Either InfluxDB OR NDP
2. **InfluxDB alone**: Good for dashboards, limited analytics
3. **NDP alone**: Good for analytics, use DuckDB for dashboards
4. **Both**: Possible on Pi 5 (16GB RAM), but monitor memory

### 7.4 Memory Budget (Raspberry Pi 5, 8GB)

| Configuration | Memory Budget | Feasibility |
|---------------|---------------|-------------|
| InfluxDB only | ~500MB | Easy |
| NDP (DuckDB) only | ~750MB | Easy |
| InfluxDB + NDP | ~1.3GB | Comfortable |
| InfluxDB + NDP + TimescaleDB | ~2.5GB | Possible but tight |

---

## 8. Summary

### Key Findings

1. **InfluxDB excels at real-time visualization** but struggles with complex analytics
2. **DuckDB provides powerful SQL analytics** without ETL overhead
3. **TimescaleDB enables ML feature engineering** with continuous aggregates
4. **These are complementary systems**, not competing alternatives

### Recommended Architecture

```
InfluxDB (HA) -----> Real-time dashboards, simple alerts
     |
     +-- parallel ingestion --+
                              |
                              v
NDP Bronze (Parquet) -----> Raw data storage (source of truth)
     |
     v
NDP Silver (DuckDB) -----> Analytics, correlation, ad-hoc queries
     |
     v
NDP Gold (TimescaleDB) --> ML features, predictions, forecasting
```

### Decision Matrix

| If you need... | Then use... |
|----------------|-------------|
| Quick HA dashboard | InfluxDB |
| Cross-stream analysis | DuckDB |
| ML predictions | TimescaleDB + ruv-FANN |
| Simple alerts | InfluxDB |
| Pattern detection | DuckDB/TimescaleDB |
| Long-term storage | Parquet (Bronze) |

---

## References

- [NDP Platform Architecture Overview](/workspaces/neural-data-platform/docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md)
- [Data Architecture Analysis](/workspaces/neural-data-platform/product/research/dp-analysis/data-architecture-analysis.md)
- [InfluxDB Documentation](https://docs.influxdata.com/)
- [DuckDB Documentation](https://duckdb.org/docs/)
- [TimescaleDB Documentation](https://docs.timescale.com/)
- [Home Assistant InfluxDB Integration](https://www.home-assistant.io/integrations/influxdb/)

---

## Document History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2025-12-30 | Initial analysis |
