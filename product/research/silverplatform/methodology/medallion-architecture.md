# Medallion Architecture Research: Traditional and Modern Patterns

**Research Date**: 2025-12-23
**Focus**: Bronze/Silver/Gold data layers for Neural Data Platform
**Context**: 5 IoT/air quality streams in Parquet (Bronze layer), evaluating Silver layer options

---

## Executive Summary

Medallion architecture is a proven, layered data design pattern that organizes data in a lakehouse through three progressive quality stages: **Bronze** (raw ingestion), **Silver** (cleaned/refined), and **Gold** (business-ready aggregates). Originally popularized by Databricks, this pattern has become the standard for modern data lakehouses and is particularly well-suited for IoT streaming data.

**Key Finding for NDP**: For a small-scale personal project with 5 IoT streams, medallion architecture provides clear data quality progression without requiring enterprise-scale infrastructure. **TimescaleDB** emerges as the optimal Silver layer technology over DuckDB and ClickHouse for this use case.

---

## 1. Medallion Architecture: Core Principles

### 1.1 The Three Layers

#### Bronze Layer (Raw/Landing)
- **Purpose**: Ingestion zone for raw, unprocessed data exactly as received
- **Storage**: Append-only, preserving all source data with metadata (timestamps, source IDs, batch IDs)
- **Format**: Typically Parquet, Delta Lake, or Iceberg for ACID compliance
- **Schema**: Schema-on-read or schema evolution to accept new columns automatically
- **Error Handling**: Quarantine mechanisms for corrupted/malformed records to prevent pipeline failures

**Best Practices**:
- Preserve all source data changes (including deletions/updates via CDC)
- Capture rich metadata: source details, ingestion timestamps, data lineage
- Implement data cataloging for discoverability
- No transformations—store exactly as received

#### Silver Layer (Refined/Curated)
- **Purpose**: Enterprise view of validated, cleaned, and enriched data
- **Transformations**: Deduplication, normalization, data quality checks, enrichment
- **Schema**: Conformed schemas, matched and merged from Bronze sources
- **Organization**: Often denormalized for query efficiency
- **Access Pattern**: Designed for analytical queries, dashboards, and ML feature engineering

**Best Practices**:
- Ingest each Bronze source separately, then combine in Silver
- Apply "just-enough" transformations—not business logic yet
- Maintain data lineage and provenance tracking
- Use idempotent transformations for reprocessability
- Join reference data and enrich with contextual information

#### Gold Layer (Business-Ready/Curated)
- **Purpose**: Project-specific, consumption-ready datasets for reporting and ML
- **Structure**: Highly denormalized, optimized for specific use cases
- **Aggregations**: Pre-computed KPIs, metrics, and business logic applied
- **Access Pattern**: Read-optimized with minimal joins for dashboard performance
- **Storage**: Only recent/relevant data—historical analysis uses Silver

**Best Practices**:
- Optimize for specific consumption patterns (dashboards, ML models, reports)
- Apply business logic and domain-specific transformations
- Implement data retention policies (archive or delete stale Gold data)
- Use materialized views or continuous aggregates for performance

### 1.2 Data Flow

```
IoT Sensors → Bronze (Parquet/Delta) → Silver (Structured DB) → Gold (Aggregated Views)
   |              |                           |                        |
   Raw         Validated                 Enriched              Business-Ready
   Append       Cleaned                  Normalized              Optimized
   Schema-free  Typed                    Conformed              Denormalized
```

### 1.3 Key Benefits

1. **Incremental Quality Improvement**: Data progressively refined through each layer
2. **Reprocessability**: Bronze preserves raw data for re-transformation if needed
3. **Separation of Concerns**: Each layer has clear responsibilities
4. **Scalability**: Process only necessary transformations at each stage
5. **Auditability**: Full data lineage from source to consumption
6. **Cost Optimization**: Store expensive transformations only where needed

---

## 2. Streaming and IoT Use Cases

### 2.1 Why Medallion for IoT?

IoT generates high-volume, diverse data types (structured sensor readings, unstructured logs, images) with continuous streaming. Medallion architecture addresses key IoT challenges:

- **Scalability**: Effortlessly scales with growing device counts
- **Flexibility**: Handles heterogeneous data types without schema rigidity
- **Real-Time Processing**: Enables streaming ingestion with batch transformations
- **Cost-Effectiveness**: Consolidates data lakes and warehouses, reducing infrastructure
- **Late-Arriving Data**: Bronze layer accepts out-of-order events; Silver handles temporal ordering

### 2.2 Streaming Data Flow

**Bronze Ingestion**:
- **Apache Kafka**: Common streaming backbone for IoT events (MQTT → Kafka → Bronze)
- **Event Hubs/IoT Hub**: Azure-native ingestion for sensor telemetry
- **Direct File Writes**: For batch-style IoT devices (write Parquet directly)

**Example**: Air Quality Sensors
```
Sensor → MQTT Broker → Kafka Topic → Spark Streaming → Bronze Parquet Files
                                                              ↓
                                                         (5-minute batches)
```

**Silver Transformation**:
- **Micro-batching**: Process Bronze files every N minutes (5-15 min typical)
- **Stream Processing**: Use Flink/Spark Structured Streaming for true real-time
- **Change Data Capture (CDC)**: Track updates/deletes if Bronze supports ACID transactions

**Gold Aggregation**:
- **Continuous Aggregates**: Pre-compute hourly/daily rollups (TimescaleDB strength)
- **Materialized Views**: Refresh on schedule or trigger-based
- **Feature Engineering**: Time-series features for ML (lag, rolling averages, seasonality)

### 2.3 Real-World IoT Patterns

**Manufacturing (OEE)**:
- Bronze: Raw telemetry from factory sensors
- Silver: Cleaned readings, anomaly detection, joined with parts catalog
- Gold: Overall Equipment Effectiveness (OEE), downtime root-cause analysis

**Smart Cities (Traffic)**:
- Bronze: Vehicle counts, speed sensors, camera feeds
- Silver: Normalized traffic flow, incident detection
- Gold: Congestion predictions, route optimization

**Environmental Monitoring (Air Quality)**:
- Bronze: Raw PM2.5, temperature, humidity from sensors + external API data
- Silver: Calibrated readings, outlier removal, enriched with weather context
- Gold: AQI calculations, health alerts, trend forecasts

---

## 3. Technology Choices for Small-Scale Projects

### 3.1 Storage Formats

#### Delta Lake
- **Pros**: ACID transactions, time travel, schema evolution, open-source
- **Cons**: Requires Spark for full features (or delta-rs for Python-only)
- **Small-Scale Fit**: Excellent if already using Spark; delta-rs enables Python/Pandas usage without Java dependencies
- **NDP Consideration**: Adds complexity for 5 streams; Parquet + TimescaleDB simpler

#### Apache Iceberg
- **Pros**: Scalability, ACID compliance, schema evolution, multi-engine support
- **Cons**: Emerging technology, fewer mature tools than Delta Lake
- **Small-Scale Fit**: Good for future-proofing, but overkill for personal projects

#### Parquet (Plain)
- **Pros**: Columnar compression, wide tool support, simple, no dependencies
- **Cons**: No ACID transactions, no time travel, manual schema management
- **Small-Scale Fit**: Perfect for Bronze layer in small projects (NDP's current state)
- **NDP Consideration**: Already using Parquet successfully for Bronze

### 3.2 Silver Layer Database Options

#### DuckDB (Embedded OLAP)
- **Architecture**: Columnar, embedded in-process database
- **Pros**:
  - Zero-ops (no server), embedded in Rust apps
  - Excellent for analytics queries, fast aggregations
  - Direct Parquet file querying (no ETL needed)
  - SQL interface familiar to developers
- **Cons**:
  - Not optimized for time-series (lacks linear interpolation extensions)
  - No native streaming support (batch-oriented)
  - Requires virtual tables and window functions for time-series joins
  - Single-process (no concurrent writes from multiple apps)
- **Small-Scale Fit**: Good for analytics, but lacks time-series ergonomics
- **NDP Experience**: Attempted DuckDB; failed due to time-series limitations

#### TimescaleDB (Time-Series PostgreSQL)
- **Architecture**: PostgreSQL extension with hypertables and continuous aggregates
- **Pros**:
  - **Purpose-built for time-series**: Automatic partitioning (chunks), time-aware query planning
  - **Continuous aggregates**: Materialized views for hourly/daily rollups (perfect for Gold layer)
  - **PostgreSQL compatibility**: Full SQL, ACID, mature ecosystem
  - **Compression**: Native time-series compression (10-90% storage reduction)
  - **Retention policies**: Automatic old data removal
  - **Real-time ingest**: Handles high write throughput for streaming
- **Cons**:
  - Row-based storage (slower than columnar for wide scans)
  - Requires PostgreSQL server (not embedded)
  - ~4x faster than vanilla PostgreSQL but 2x slower than ClickHouse on analytics
- **Small-Scale Fit**: **EXCELLENT**—designed for IoT monitoring, small-scale deployment friendly
- **NDP Recommendation**: **TOP CHOICE** for Silver layer

#### ClickHouse (Columnar OLAP)
- **Architecture**: Columnar, distributed OLAP database
- **Pros**:
  - **Fastest analytics performance**: 2x faster than TimescaleDB on aggregations
  - **Best storage efficiency**: 1.7x less disk than competitors
  - **Excellent compression**: Columnar format ideal for numeric time-series
  - **Scalable**: Handles billions of rows with ease
- **Cons**:
  - **Not time-series optimized**: Lacks ASOF joins with timestamp-only conditions
  - **Batch-oriented writes**: Struggles with high-frequency small writes (IoT pattern)
  - **Complex operations**: More setup than TimescaleDB for time-series features
  - **Overkill for small projects**: Designed for enterprise scale
- **Small-Scale Fit**: Overkill—shines at 10M+ rows/day, not 5 IoT streams
- **NDP Consideration**: Future option if scaling to 100+ sensors

### 3.3 Recommendation Matrix

| Database | Best For | Small-Scale Fit | IoT Streaming | NDP Suitability |
|----------|----------|-----------------|---------------|-----------------|
| **TimescaleDB** | Time-series, IoT, monitoring | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | **BEST CHOICE** |
| DuckDB | Embedded analytics, ad-hoc queries | ⭐⭐⭐⭐ | ⭐⭐ | Not ideal for time-series |
| ClickHouse | Large-scale OLAP, data warehousing | ⭐⭐ | ⭐⭐⭐ | Overkill for 5 streams |

### 3.4 Why TimescaleDB for NDP Silver Layer

1. **Time-Series Native**: Hypertables automatically partition by time (ideal for air quality sensors)
2. **Continuous Aggregates**: Gold layer hourly/daily rollups with automatic refresh
3. **PostgreSQL Ecosystem**: Familiar SQL, mature connectors (Rust `tokio-postgres`, Grafana native support)
4. **Small-Scale Friendly**: Runs on Raspberry Pi, no cluster required
5. **Real-Time Ingest**: Handles streaming writes from Bronze ETL pipeline
6. **Compression**: Reduces storage costs for long-term historical data
7. **Retention Policies**: Automatic old data cleanup (Bronze → Silver → Archive)

---

## 4. Industry Patterns: Netflix, Uber, Airbnb

### 4.1 Common Infrastructure

All three tech giants rely on **Apache Kafka** as the distributed event streaming backbone for moving large data volumes in real-time, despite having different business models.

### 4.2 Netflix

**Architecture**:
- **Microservices**: Thousands of independent services on AWS (recommendations, billing, metadata)
- **CDC Tool (DBLog)**: Captures MySQL/PostgreSQL changes, streams to analytics stores
- **Open Connect CDN**: Edge servers near ISPs for low-latency streaming
- **Streaming Pipeline**: Kafka → Flink → Hadoop/Cassandra → Data Lakehouse

**Key Lesson**: High availability and real-time synchronization with minimal operational database impact. Microservices enable independent failure isolation.

### 4.3 Uber

**Architecture**:
- **Real-Time Streaming**: User location → Kafka → Flink → Driver matching + fare calculation
- **Storage**: Hybrid Hadoop + Cassandra for high availability and low latency
- **DataCentral**: Custom data observability platform for monitoring data health
- **Multi-Cloud**: Chose multiple cloud vendors for resilience

**Key Lesson**: Immediacy is critical (ride waiting times). Flink chosen over Spark for true real-time processing with "exactly-once" guarantees. Zero data loss requirement.

### 4.4 Airbnb

**Architecture**:
- **SpinalTap (CDC)**: Ensures flawless accuracy for dynamic pricing, availability, reservations
- **Dataportal**: Metadata management and discovery layer (Neo4J graph + ElasticSearch)
- **Stack**: Kafka, Spark, Hadoop, EMR, S3
- **Philosophy**: Start with monolith, then microservices; radical automation; config-as-code

**Key Lesson**: Metadata graph connects data assets (tables, dashboards, users, teams, outcomes) for discoverability. Consistency across all systems is paramount.

### 4.5 Patterns for Small Projects

- **Start Simple**: Airbnb's "monolith first" approach applies—don't over-engineer
- **Kafka for Streaming**: Industry standard for decoupling producers/consumers
- **CDC for Sync**: Ensure data consistency across systems (Bronze → Silver)
- **Observability**: Build monitoring early (NDP: Grafana dashboards)
- **Flink vs Spark**: Flink for true real-time (<100ms latency); Spark for micro-batches (5-15 min)

---

## 5. Lakehouse Architecture for Small-Scale IoT

### 5.1 What is a Lakehouse?

A lakehouse combines the **flexibility and scalability of a data lake** (store raw, unstructured data) with the **performance and reliability of a data warehouse** (ACID transactions, SQL queries). It eliminates the need for separate systems, reducing complexity and cost.

**Key Features**:
- **Unified Storage**: Single location for raw and curated data (Bronze/Silver/Gold)
- **ACID Transactions**: Delta Lake/Iceberg enable reliable updates/deletes
- **Schema Evolution**: Add columns without breaking existing queries
- **Time Travel**: Revert to previous data versions (audit, rollback)
- **Open Formats**: Parquet-based, no vendor lock-in

### 5.2 Lakehouse for IoT

**Why Lakehouse > Traditional Data Lake**:
- IoT devices generate diverse data types (structured sensor readings, images, logs)
- Volume scales rapidly (1 sensor → 100 sensors → 1000 sensors)
- Real-time processing requirements (alerts, dashboards)
- Cost-effective cloud storage (S3, ADLS) with compute-on-demand

**Architecture Layers**:
1. **Ingestion**: Apache NiFi, Kafka Connect, cloud-native (AWS DMS, Azure Data Factory)
2. **Storage**: Cloud object storage (S3, ADLS, GCS) with Delta Lake/Iceberg metadata
3. **Processing**: Spark, Flink, DuckDB (query Parquet directly)
4. **Serving**: TimescaleDB for time-series, PostgreSQL for relational, Grafana for viz

### 5.3 Small-Scale Implementation

**Minimal Lakehouse Stack**:
- **Bronze**: Parquet files on local disk or S3 (append-only)
- **Silver**: TimescaleDB for cleaned time-series data
- **Gold**: Continuous aggregates in TimescaleDB (hourly/daily views)
- **Orchestration**: Rust application with tokio runtime (no Airflow/Spark needed)

**Example: 5 Air Quality Sensors**:
```
Sensors → Parquet (Bronze) → Rust ETL → TimescaleDB (Silver) → Continuous Aggregates (Gold)
                                             ↓
                                        Grafana Dashboards
```

**Cost**: Lakehouse architecture on small scale = $0 (local) or $5-20/month (S3 + small VM)

---

## 6. Modern Alternatives: Data Mesh, Data Fabric

### 6.1 Data Mesh

**Philosophy**: Decentralized, domain-driven data ownership. Instead of a central data team, each business domain owns their data as a "product."

**Key Principles**:
1. **Domain Ownership**: Each team (e.g., "Air Quality", "Weather") owns their data pipeline
2. **Data as a Product**: Teams treat data as a product with SLAs, documentation, quality guarantees
3. **Self-Serve Platform**: Centralized infrastructure, but domains operate independently
4. **Federated Governance**: Policies set centrally, enforced locally

**When to Use**:
- Large organizations with multiple independent teams
- Each team has unique data needs and expertise
- Need to scale data ownership beyond a central team

**Small-Scale Fit**: ❌ **NOT SUITABLE**—requires organizational structure and coordination overhead. For a personal project with 5 streams, a centralized approach is simpler.

### 6.2 Data Fabric

**Philosophy**: Unified data access layer that connects all data sources (databases, lakes, warehouses, APIs) for real-time querying without data movement.

**Key Features**:
- **Federated Queries**: Query across multiple sources (TimescaleDB + Parquet + external APIs) in one SQL statement
- **Metadata-Driven**: Automatic data discovery, lineage, and cataloging
- **Real-Time Access**: No ETL delay—query live data sources
- **Tools**: Presto/Trino, Starburst, Dremio, Denodo

**When to Use**:
- Data siloed across many systems (ERP, CRM, HR, legacy databases)
- Need to avoid duplicating data (cost/compliance constraints)
- Real-time queries more important than performance optimization

**Small-Scale Fit**: ⚠️ **OVERKILL**—for 5 IoT streams, ETL into TimescaleDB is simpler and faster. Data Fabric shines with 10+ heterogeneous sources.

### 6.3 How They Relate to Medallion

**Medallion + Data Mesh**: Compatible! Each domain team owns their Bronze/Silver/Gold layers. Medallion provides structure; Data Mesh provides ownership model.

**Medallion + Data Fabric**: Hybrid approach—use Medallion for core data layers (Bronze/Silver/Gold in lakehouse), but query external sources via federated engine when needed.

**Recommendation for NDP**: Stick with **Medallion Architecture** for simplicity. Consider Data Mesh/Fabric if scaling to 10+ data domains or 100+ external sources.

---

## 7. Implementation Complexity Analysis

### 7.1 Complexity Dimensions

| Dimension | DuckDB | TimescaleDB | ClickHouse |
|-----------|--------|-------------|------------|
| **Setup** | ⭐ Trivial (embed in app) | ⭐⭐ Easy (Docker/apt-get) | ⭐⭐⭐ Moderate (cluster config) |
| **Schema Design** | ⭐⭐ Manual tables | ⭐ Hypertables (automatic) | ⭐⭐⭐ Merge trees, partitions |
| **Time-Series Features** | ⭐⭐⭐ DIY window functions | ⭐ Native (time_bucket, etc.) | ⭐⭐ ASOF joins, interpolation |
| **Continuous Aggregates** | ⭐⭐⭐ Materialized views (manual refresh) | ⭐ Built-in (auto refresh) | ⭐⭐ Materialized views |
| **Rust Integration** | ⭐⭐ duckdb-rs (limited) | ⭐ tokio-postgres (mature) | ⭐⭐ clickhouse-rs |
| **Monitoring/Observability** | ⭐⭐⭐ Limited tooling | ⭐ Grafana native support | ⭐⭐ Grafana plugins |
| **Production Ops** | ⭐ Zero-ops (embedded) | ⭐⭐ Standard PostgreSQL | ⭐⭐⭐⭐ Cluster management |

### 7.2 Bronze → Silver ETL Complexity

**Parquet to DuckDB**:
```rust
// Option 1: Query Parquet directly (no ETL)
let conn = duckdb::Connection::open_in_memory()?;
conn.execute("SELECT * FROM read_parquet('bronze/*.parquet')", [])?;

// Option 2: Insert into DuckDB tables
conn.execute("CREATE TABLE sensors AS SELECT * FROM read_parquet('bronze/*.parquet')", [])?;
```
**Complexity**: ⭐⭐ Low—DuckDB can query Parquet directly without ETL

**Parquet to TimescaleDB**:
```rust
// Read Parquet with arrow-rs
let file = File::open("bronze/sensors.parquet")?;
let reader = ParquetRecordBatchReaderBuilder::try_new(file)?.build()?;

// Insert into TimescaleDB
let client = tokio_postgres::connect("host=localhost dbname=ndp", NoTls).await?;
for batch in reader {
    for row in batch {
        client.execute(
            "INSERT INTO sensors (timestamp, pm25, temp) VALUES ($1, $2, $3)",
            &[&row.timestamp, &row.pm25, &row.temp],
        ).await?;
    }
}
```
**Complexity**: ⭐⭐⭐ Moderate—requires ETL pipeline, but tokio-postgres is mature

**Parquet to ClickHouse**:
```rust
// Similar to TimescaleDB, but with ClickHouse-specific INSERT format
let client = clickhouse::Client::default();
client.insert("sensors")?
    .write(&row)
    .await?;
```
**Complexity**: ⭐⭐⭐ Moderate—ClickHouse prefers batch inserts (10k+ rows)

### 7.3 Maintenance Overhead

| Task | DuckDB | TimescaleDB | ClickHouse |
|------|--------|-------------|------------|
| **Backups** | Copy .db file | pg_dump or WAL | Replicas + backups |
| **Compression** | N/A (Parquet already compressed) | Built-in chunk compression | Automatic columnar compression |
| **Retention Policies** | Manual DELETE | Automatic drop_chunks() | TTL on tables |
| **Schema Evolution** | ALTER TABLE | ALTER TABLE (standard SQL) | ALTER TABLE (limited) |
| **Monitoring** | Minimal tools | PostgreSQL ecosystem | Clickhouse-client, metrics |
| **Upgrades** | Cargo update | apt-get upgrade | Complex cluster upgrades |

**Winner**: **TimescaleDB**—balances features with operational simplicity.

---

## 8. Recommendations for Neural Data Platform

### 8.1 Immediate Next Steps (Silver Layer)

1. **Choose TimescaleDB** for Silver layer:
   - Install via Docker: `docker run -d -p 5432:5432 timescale/timescaledb:latest-pg16`
   - Create hypertable: `SELECT create_hypertable('sensors', 'timestamp');`
   - Enable compression: `ALTER TABLE sensors SET (timescaledb.compress, timescaledb.compress_segmentby='sensor_id');`

2. **Build Bronze → Silver ETL Pipeline**:
   - Rust application with `tokio-postgres` and `arrow-rs`
   - Read Parquet files from Bronze layer
   - Transform: deduplicate, validate ranges, enrich with metadata
   - Insert into TimescaleDB hypertables
   - Run every 5-15 minutes (cron or systemd timer)

3. **Design Silver Schema**:
   ```sql
   CREATE TABLE sensors (
       timestamp TIMESTAMPTZ NOT NULL,
       sensor_id TEXT NOT NULL,
       stream_id TEXT NOT NULL,
       pm25 DOUBLE PRECISION,
       temperature DOUBLE PRECISION,
       humidity DOUBLE PRECISION,
       metadata JSONB,
       PRIMARY KEY (timestamp, sensor_id)
   );

   SELECT create_hypertable('sensors', 'timestamp');

   CREATE INDEX idx_sensor ON sensors (sensor_id, timestamp DESC);
   ```

4. **Create Gold Layer (Continuous Aggregates)**:
   ```sql
   CREATE MATERIALIZED VIEW sensors_hourly
   WITH (timescaledb.continuous) AS
   SELECT
       time_bucket('1 hour', timestamp) AS hour,
       sensor_id,
       AVG(pm25) AS pm25_avg,
       MAX(pm25) AS pm25_max,
       MIN(pm25) AS pm25_min,
       COUNT(*) AS sample_count
   FROM sensors
   GROUP BY hour, sensor_id;

   -- Auto-refresh every 15 minutes
   SELECT add_continuous_aggregate_policy('sensors_hourly',
       start_offset => INTERVAL '2 hours',
       end_offset => INTERVAL '15 minutes',
       schedule_interval => INTERVAL '15 minutes');
   ```

### 8.2 Architecture Diagram (NDP Target State)

```
┌─────────────────────────────────────────────────────────────┐
│ Bronze Layer (Current State)                                │
│   - 5 IoT streams (air quality sensors + APIs)              │
│   - Parquet files (append-only)                             │
│   - Rust ingestion coordinator                              │
└──────────────────────┬──────────────────────────────────────┘
                       │ ETL Pipeline (New)
                       ↓
┌─────────────────────────────────────────────────────────────┐
│ Silver Layer (Proposed)                                      │
│   - TimescaleDB hypertables                                  │
│   - Cleaned, validated, enriched data                        │
│   - Compression + retention policies                         │
└──────────────────────┬──────────────────────────────────────┘
                       │ Continuous Aggregates
                       ↓
┌─────────────────────────────────────────────────────────────┐
│ Gold Layer (Future)                                          │
│   - Hourly/daily aggregates                                  │
│   - ML features (lag, rolling avg, seasonality)              │
│   - Grafana dashboards                                       │
└─────────────────────────────────────────────────────────────┘
```

### 8.3 Technology Stack Summary

| Layer | Technology | Rationale |
|-------|-----------|-----------|
| **Bronze** | Parquet (current) | Simple, columnar, no dependencies |
| **Silver** | **TimescaleDB** | Time-series native, PostgreSQL compatibility, small-scale friendly |
| **Gold** | TimescaleDB continuous aggregates | Auto-refreshing materialized views for dashboards |
| **ETL** | Rust (tokio-postgres + arrow-rs) | Matches current NDP stack, async/await for efficiency |
| **Monitoring** | Grafana + PostgreSQL data source | Native TimescaleDB support, SQL-based dashboards |

### 8.4 Why NOT DuckDB (Lessons Learned)

1. **Time-Series Ergonomics**: Lacks native time_bucket(), ASOF joins require workarounds
2. **Streaming Mismatch**: Designed for batch analytics, not continuous ingestion
3. **Concurrent Writes**: Single-process model conflicts with multi-stream ingestion
4. **No Continuous Aggregates**: Materialized views require manual refresh
5. **Limited Monitoring**: No Grafana data source, DIY dashboards

### 8.5 When to Reconsider ClickHouse

**Thresholds**:
- 100+ sensors (vs. current 5)
- 10M+ rows per day (vs. current ~10k)
- Need for wide scans (100+ columns per query)
- Analytics queries taking >10 seconds in TimescaleDB

**At that scale**:
- ClickHouse's columnar compression becomes significant
- 2x query performance gain justifies operational complexity
- Consider dual storage: TimescaleDB for recent data (7 days), ClickHouse for historical analysis

---

## 9. Alternative Patterns Considered

### 9.1 Lambda Architecture (Batch + Stream)

**Design**: Separate batch and real-time processing paths that merge at query time.

```
IoT Sensors → Kafka → ┌─ Batch Layer (Spark) → HDFS → Batch Views
                       └─ Speed Layer (Flink) → Redis → Real-Time Views
                                                          ↓
                                                    Serving Layer (merge)
```

**Pros**: Comprehensive coverage (batch for accuracy, stream for low latency)
**Cons**: Complex (maintain two codebases), eventual consistency issues
**NDP Fit**: ❌ Overkill—medallion + TimescaleDB achieves similar results with one pipeline

### 9.2 Kappa Architecture (Stream-Only)

**Design**: Everything is a stream; reprocess by replaying Kafka topics.

```
IoT Sensors → Kafka → Stream Processing (Flink) → Storage → Queries
                       ↑ (replay for reprocessing)
```

**Pros**: Simpler than Lambda (one codebase), true real-time
**Cons**: Requires Kafka retention, stream processing expertise
**NDP Fit**: ⚠️ Possible but unnecessary—micro-batch ETL (5-15 min) meets NDP latency needs

### 9.3 Reverse ETL (Operational Analytics)

**Design**: Push Gold layer data back to operational systems (CRM, email, alerts).

```
Gold Layer → Reverse ETL → ┌─ Slack alerts
                            ├─ Email reports
                            └─ Mobile push notifications
```

**NDP Use Case**: Air quality alerts when PM2.5 > threshold
**Future Consideration**: ✅ Useful for alert system (Phase 6)

---

## 10. Sources

### Medallion Architecture Best Practices
- [Medallion Architecture: Practices for Managing Levels | Dot Labs](https://dotlabs.ai/blogs/2024/05/13/medallion-architecture-best-practices-for-managing-bronze-silver-and-gold-levels/)
- [What is the medallion lakehouse architecture? - Azure Databricks | Microsoft Learn](https://learn.microsoft.com/en-us/azure/databricks/lakehouse/medallion)
- [What is a Medallion Architecture? | Databricks](https://www.databricks.com/glossary/medallion-architecture)
- [Best practices for using medallion architecture - Sigmoid](https://www.sigmoid.com/wp-content/uploads/2024/09/Infographic_Best-practices-for-data-management-using-medallion-architecture.pdf)
- [The Medallion Architecture: How It Works & Why It's Useful | Nimbleway](https://www.nimbleway.com/blog/what-is-medallion-architecture)
- [Medallion architecture: best practices for managing Bronze, Silver and Gold | by Piethein Strengholt | Medium](https://piethein.medium.com/medallion-architecture-best-practices-for-managing-bronze-silver-and-gold-486de7c90055)
- [Medallion Layers: Best Practices and Pitfalls | Weld](https://weld.app/blog/medallion-layers)

### Streaming and IoT Use Cases
- [Bronze, Silver, and Gold Data Layers - The Agile Brand Guide®](https://agilebrandguide.com/wiki/data/bronze-silver-and-gold-data-layers/)
- [Implementing the Medallion Architecture with Redpanda](https://www.redpanda.com/blog/medallion-architecture-redpanda)
- [What goes into bronze, silver, and gold layers of a medallion data architecture? | by Lak Lakshmanan | Medium](https://lakshmanok.medium.com/what-goes-into-bronze-silver-and-gold-layers-of-a-medallion-data-architecture-4b6fdfb405fc)
- [Build ETL pipelines with Azure Databricks and Delta Lake - Azure Architecture Center | Microsoft Learn](https://learn.microsoft.com/en-us/azure/architecture/solution-ideas/articles/ingest-etl-stream-with-adb)
- [Designing Robust Data Pipelines with the Medallion Architecture | BairesDev](https://www.bairesdev.com/blog/data-pipeline-design/)

### Lakehouse Architecture for IoT
- [Lakehouse reference architectures (download) | Databricks on AWS](https://docs.databricks.com/aws/en/lakehouse-architecture/reference)
- [Lakehouse and IoT: Managing and Analyzing Real-time Data | Dview.io](https://dview.io/blog/iot-data-management-lakehouse-integration)
- [Understanding Lakehouse Architecture: Components, Benefits & Design | Athena Solutions](https://athena-solutions.com/decoding-the-lakehouse-architecture/)
- [Data Lakehouse Gradually Forms the Future of IoT Analytics | 1NCE](https://www.1nce.com/en-us/resources/news/blog/data-lakehouse-iot)
- [How a Lakehouse would have saved me much headache with IoT data a few years ago | by Felix Mutzl | Medium](https://medium.com/@felix.mutzl/how-a-lakehouse-would-have-saved-me-much-headache-with-iot-data-a-few-years-ago-873419e90927)
- [Unlocking End-to-End IoT Analytics with Databricks Postgres Serverless and the Lakehouse Platform | by THE BRICK LEARNING | Towards Data Engineering | Medium](https://medium.com/towards-data-engineering/unlocking-end-to-end-iot-analytics-with-databricks-postgres-serverless-and-the-lakehouse-platform-07ac659dd38d)

### Delta Lake Implementation
- [Databricks Delta Lake: A Scalable Data Lake Solution | ProjectPro](https://www.projectpro.io/article/databricks-delta-lake/742)
- [GitHub - delta-io/delta: An open-source storage framework](https://github.com/delta-io/delta)
- [Home | Delta Lake](https://delta.io/)
- [Delta Lake: The Definitive Guide | Amazon](https://www.amazon.com/Delta-Lake-Definitive-Lakehouse-Architectures/dp/1098151941)
- [Getting Started with Delta Lake | Delta Lake](https://delta.io/learn/getting-started/)

### Data Platform Architecture Patterns
- [Exploring the Best Data Warehouse Alternatives in 2025 | Integrate.io](https://www.integrate.io/blog/the-future-of-data-architecture/)
- [Data Platform Architecture Patterns: A Primer for Professionals | Gable](https://www.gable.ai/blog/data-platform-architecture-patterns)
- [Data Platform Architectures & Design Patterns: A Comparative Analysis | LinkedIn](https://www.linkedin.com/pulse/data-platform-architectures-design-patterns-comparative-tfwoc)
- [The Essential Modern Data Stack Tools for 2025 | Complete Guide | Airbyte](https://airbyte.com/top-etl-tools-for-sources/the-essential-modern-data-stack-tools)
- [3 Modern Data Architecture Paradigms (Pros & Cons) | Keboola](https://www.keboola.com/blog/which-modern-data-architecture-should-you-choose)

### Industry Examples (Netflix, Uber, Airbnb)
- [They Handle 500B Events Daily. Here's Their Data Engineering Architecture. | Monte Carlo](https://www.montecarlodata.com/blog-data-engineering-architecture/)
- [Architecture of Giants: Data Stacks at Facebook, Netflix, Airbnb, and Pinterest - Keen](https://keen.io/blog/architecture-of-giants-data-stacks-at-facebook-netflix-airbnb-and-pinterest/)
- [How LinkedIn, Uber, Lyft, Airbnb and Netflix are Solving Data Management and Discovery for Machine Learning Solutions - KDnuggets](https://www.kdnuggets.com/2019/08/linkedin-uber-lyft-airbnb-netflix-solving-data-management-discovery-machine-learning-solutions.html)
- [Change data capture: The critical link for Airbnb, Netflix and Uber | VentureBeat](https://venturebeat.com/data-infrastructure/change-data-capture-the-critical-link-for-airbnb-netflix-and-uber)
- [What Uber, Netflix & Instagram Taught Me About System Design | by Saikiran Kalidindi | Medium](https://medium.com/@saikirankalidindi/what-uber-netflix-instagram-taught-me-about-system-design-68e478e926c2)

### Database Comparisons (DuckDB vs TimescaleDB vs ClickHouse)
- [ClickHouse vs TimescaleDB. What is the difference? A detailed comparison | by Data Engineer | DoubleCloud | Medium](https://medium.com/doublecloud-insights/clickhouse-vs-timescaledb-what-is-the-difference-a-detailed-comparison-62127a989d8d)
- [Benchmarking databases for real-time analytics applications | Tigerdata](https://www.tigerdata.com/blog/benchmarking-databases-for-real-time-analytics-applications)
- [Timeseries Databases Performance — Testing 7 alternatives | by Everton Kozloski | Medium](https://medium.com/@ev_kozloski/timeseries-databases-performance-testing-7-alternatives-56a3415e6e9e)
- [Time-Series Databases 2025: InfluxDB vs TimescaleDB vs ClickHouse | Markaicode](https://markaicode.com/time-series-databases-2025-comparison/)
- [Comparing ClickHouse to PostgreSQL and TimescaleDB for time-series data | Hacker News](https://news.ycombinator.com/item?id=28945903)

### Data Mesh and Data Fabric
- [Medallion Model vs. Data Mesh vs. Data Fabric | Singdata](https://www.singdata.com/trending/medallion-model-vs-data-mesh-vs-data-fabric-comparison/)
- [Simplifying Data Mesh with Microsoft Fabric's Medallion Architecture | by Exult Global | Medium](https://medium.com/@exult.global/simplifying-data-mesh-with-microsoft-fabrics-medallion-architecture-2455670d7b57)
- [Data Vault & Data Mesh in a Data Fabric | Scalefree](https://www.scalefree.com/blog/data-vault/data-vault-data-mesh-in-a-data-fabric-a-modern-architecture-guide/)
- [Implement medallion lakehouse architecture in Fabric - Microsoft Fabric | Microsoft Learn](https://learn.microsoft.com/en-us/fabric/onelake/onelake-medallion-lakehouse-architecture)

---

## Appendix A: Glossary

- **ACID**: Atomicity, Consistency, Isolation, Durability—guarantees for database transactions
- **CDC**: Change Data Capture—tracking inserts/updates/deletes in source databases
- **Hypertable**: TimescaleDB's automatic time-based partitioning mechanism
- **OLAP**: Online Analytical Processing—optimized for complex queries and aggregations
- **OLTP**: Online Transaction Processing—optimized for high-volume inserts/updates
- **Schema-on-Read**: Define schema when querying data, not when storing it
- **Time Travel**: Ability to query historical versions of data

## Appendix B: Next Steps for NDP Team

1. **Architecture Decision**: Document Silver layer choice (TimescaleDB) in ADR
2. **Schema Design**: Design hypertables for all 5 air quality streams
3. **ETL Pipeline**: Build Bronze → Silver Rust application
4. **Continuous Aggregates**: Define hourly/daily rollups for Gold layer
5. **Grafana Integration**: Configure TimescaleDB data source and dashboards
6. **Testing**: Integration tests for ETL pipeline with mock Parquet files
7. **Documentation**: Update data flow diagrams and deployment guides

---

**Research Complete**: 2025-12-23
**Recommendation**: Proceed with **TimescaleDB** for Silver layer implementation using medallion architecture principles.
