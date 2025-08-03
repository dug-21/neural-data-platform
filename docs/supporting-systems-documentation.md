# Supporting Systems Documentation - Neural Trader

## Overview

The Neural Trader system relies on four critical supporting systems that provide data persistence, caching, monitoring, and observability. This document provides comprehensive documentation of each system's configuration, usage patterns, and operational characteristics.

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Neural Trader Platform                       │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────┐ │
│  │ TimescaleDB │  │    Redis    │  │ Prometheus  │  │ Grafana │ │
│  │ (Time-Series│  │  (Cache &   │  │ (Metrics    │  │ (Visual-│ │
│  │  Database)  │  │  Pub/Sub)   │  │ Collection) │  │ ization)│ │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## 1. TimescaleDB Configuration and Usage

### Current Schema and Hypertables

TimescaleDB serves as the primary time-series database for storing market data, predictions, and trading records.

#### Core Tables

**market_data** - Main market data storage
```sql
CREATE TABLE market_data (
    time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(10) NOT NULL,
    open DECIMAL(10, 4) NOT NULL CHECK (open > 0),
    high DECIMAL(10, 4) NOT NULL CHECK (high > 0),
    low DECIMAL(10, 4) NOT NULL CHECK (low > 0),
    close DECIMAL(10, 4) NOT NULL CHECK (close > 0),
    volume BIGINT NOT NULL CHECK (volume >= 0),
    provider VARCHAR(50) NOT NULL,
    metadata JSONB,
    PRIMARY KEY (time, symbol, provider)
);
```

**time_series_data** - Enhanced time-series storage
```sql
CREATE TABLE time_series_data (
    time TIMESTAMPTZ NOT NULL,
    symbol TEXT NOT NULL,
    exchange TEXT NOT NULL,
    open NUMERIC(20, 8) NOT NULL,
    high NUMERIC(20, 8) NOT NULL,
    low NUMERIC(20, 8) NOT NULL,
    close NUMERIC(20, 8) NOT NULL,
    volume NUMERIC(30, 8) NOT NULL,
    -- Technical indicators (calculated on insert)
    rsi_14 NUMERIC(8, 4),
    macd_signal NUMERIC(20, 8),
    bollinger_upper NUMERIC(20, 8),
    bollinger_lower NUMERIC(20, 8),
    -- Market microstructure
    order_imbalance NUMERIC(8, 4),
    spread NUMERIC(20, 8) GENERATED ALWAYS AS (ask - bid) STORED,
    spread_pct NUMERIC(8, 6) GENERATED ALWAYS AS ((ask - bid) / NULLIF(bid, 0) * 100) STORED
);
```

**predictions** - Neural model predictions
```sql
CREATE TABLE predictions (
    prediction_id UUID DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    symbol TEXT NOT NULL,
    model_name TEXT NOT NULL,
    model_version TEXT NOT NULL,
    prediction_time TIMESTAMPTZ NOT NULL,
    prediction_type TEXT NOT NULL,
    prediction_horizon INTEGER NOT NULL,
    predicted_value JSONB NOT NULL,
    confidence NUMERIC(5, 4) NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    features_used JSONB,
    model_metadata JSONB,
    actual_value JSONB,
    error_metrics JSONB
);
```

#### Hypertable Configuration

All time-series tables are converted to hypertables for optimized performance:

```sql
-- Main market data with 1-hour chunks
SELECT create_hypertable('market_data', 'time', if_not_exists => TRUE);

-- Enhanced time-series with 6-hour chunks  
SELECT create_hypertable('time_series_data', 'time', 
    chunk_time_interval => INTERVAL '6 hours', if_not_exists => TRUE);

-- Predictions with daily chunks
SELECT create_hypertable('predictions', 'created_at', 
    chunk_time_interval => INTERVAL '1 day', if_not_exists => TRUE);
```

### Continuous Aggregates

Pre-computed aggregations for improved query performance:

**1-Minute OHLCV Aggregates**
```sql
CREATE MATERIALIZED VIEW mv_1min_ohlcv
WITH (timescaledb.continuous) AS
SELECT 
    time_bucket('1 minute', time) AS bucket,
    symbol, exchange,
    FIRST(open, time) AS open,
    MAX(high) AS high,
    MIN(low) AS low,
    LAST(close, time) AS close,
    SUM(volume) AS volume,
    AVG(spread_pct) AS avg_spread_pct
FROM time_series_data
GROUP BY bucket, symbol, exchange;
```

**Hourly and Daily Aggregates**
- `mv_5min_ohlcv` - 5-minute bars
- `mv_hourly_ohlcv` - 1-hour bars with volatility calculations
- `market_data_daily` - Daily aggregates

### Compression Policies

Automatic compression for storage optimization:

```sql
-- Compress market data older than 7 days
ALTER TABLE market_data SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol,provider',
    timescaledb.compress_orderby = 'time DESC'
);
SELECT add_compression_policy('market_data', INTERVAL '7 days');

-- Compress predictions older than 30 days
ALTER TABLE predictions SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol,model_name',
    timescaledb.compress_orderby = 'created_at DESC'
);
SELECT add_compression_policy('predictions', INTERVAL '30 days');
```

### Retention Policies

Automatic data lifecycle management:

- **market_data**: 1 year retention
- **tick_data**: 30 days retention  
- **order_book**: 7 days retention
- **predictions**: 180 days retention
- **trades**: 365 days retention

### Query Patterns

**Real-time Price Queries**
```sql
SELECT time, close as price, volume
FROM market_data
WHERE symbol = 'AAPL'
ORDER BY time DESC
LIMIT 1;
```

**Historical Analysis**
```sql
SELECT bucket, symbol, open, high, low, close, volume
FROM mv_hourly_ohlcv
WHERE symbol = 'AAPL' 
  AND bucket >= NOW() - INTERVAL '7 days'
ORDER BY bucket;
```

**Model Performance Analysis**
```sql
SELECT model_name, AVG(confidence), COUNT(*)
FROM predictions
WHERE created_at >= NOW() - INTERVAL '24 hours'
GROUP BY model_name;
```

## 2. Redis Configuration and Usage

### Instance Configuration

Redis is configured for high-performance real-time data operations:

```conf
# Memory Management
maxmemory 4gb
maxmemory-policy allkeys-lru
maxmemory-samples 5

# Threading for high throughput
io-threads 8
io-threads-do-reads yes

# Persistence (RDB + AOF)
save 900 1
save 300 10
save 60 10000
appendonly yes
appendfsync everysec

# Performance optimizations
lazyfree-lazy-eviction yes
activedefrag yes
jemalloc-bg-thread yes
```

### Pub/Sub Channels and Purposes

**Price Update Channels**
- `price_updates:{symbol}` - Real-time price changes
- `tick_updates:{symbol}` - Trade-level tick data
- `orderbook_updates:{symbol}` - Order book snapshots

**System Coordination Channels**
- `neural_predictions` - Neural model predictions
- `trading_signals` - Strategy signals
- `system_alerts` - Health and error notifications

**Data Pipeline Channels**
- `data_ingestion_status` - Provider health updates
- `market_data_quality` - Data validation results
- `backfill_progress` - Historical data loading status

### Caching Strategies

**Latest Price Cache**
```python
# Key: price:latest:{symbol}
# TTL: 1 hour
await redis.hset(f"price:latest:{symbol}", mapping={
    'price': price,
    'volume': volume,
    'timestamp': datetime.utcnow().isoformat()
})
```

**Recent Tick Data Cache**
```python
# Key: ticks:{symbol} (sorted set)
# TTL: Automatic cleanup of data older than 1 hour
await redis.zadd(f"ticks:{symbol}", {json.dumps(tick): timestamp})
```

**Order Book Cache**
```python
# Key: orderbook:{symbol}
# TTL: 60 seconds
await redis.set(f"orderbook:{symbol}", json.dumps(orderbook), ex=60)
```

**Application Cache**
```python
# Key: cache:{key}
# TTL: Configurable (default 1 hour)
await redis.set(f"cache:{key}", json.dumps(value), ex=ttl)
```

### Data Structures Used

**Hash Maps** - Latest price data, configuration settings
**Sorted Sets** - Time-ordered tick data, leaderboards
**Lists** - Task queues, message buffers
**Pub/Sub** - Real-time event distribution
**Strings** - Simple key-value cache, session data

### Performance Characteristics

- **Throughput**: 100,000+ operations/second
- **Latency**: Sub-millisecond for cache hits
- **Memory Usage**: ~4GB allocated, LRU eviction
- **Persistence**: Dual RDB + AOF for data safety
- **Connection Pool**: 50 max connections with multiplexing

### Queue Operations

**Task Processing Queues**
```python
# Push tasks to queue
await redis.lpush("queue:data_processing", json.dumps(task))

# Pop tasks with blocking
result = await redis.brpop("queue:data_processing", timeout=1)
```

## 3. Prometheus Configuration and Monitoring

### Metrics Collection

Prometheus scrapes metrics from multiple components with specific intervals:

**Neural Trader Application Metrics** (10s interval)
- `neural_trader_prediction_accuracy` - Model accuracy rates
- `neural_trader_prediction_latency_ms` - Prediction latency
- `neural_trader_models_available` - Available model count
- `neural_trader_daa_decisions_total` - DAA coordinator decisions
- `neural_trader_memory_usage_gb` - Memory consumption
- `neural_trader_cpu_usage_percent` - CPU utilization

**Data Ingestion Metrics** (10s interval)  
- `data_ingestion_records_processed_total` - Processing throughput
- `data_ingestion_api_requests_total` - API call tracking
- `data_ingestion_errors_total` - Error rate monitoring
- `data_ingestion_latency_seconds` - Processing latency

**Infrastructure Metrics**
- **TimescaleDB**: Connection pools, query performance, disk usage
- **Redis**: Memory usage, operations/sec, connection count
- **System**: CPU, memory, disk, network via node_exporter

### Scrape Configurations

```yaml
scrape_configs:
  - job_name: 'neural-trader'
    static_configs:
      - targets: ['neural_trader_app:9092']
    metrics_path: '/metrics'
    scrape_interval: 10s
    
  - job_name: 'data-ingestion'  
    static_configs:
      - targets: ['neural_trader_data_ingestion:8001']
    metrics_path: '/metrics'
    scrape_interval: 10s
    
  - job_name: 'postgres'
    static_configs:
      - targets: ['postgres-exporter:9187']
      
  - job_name: 'redis'
    static_configs:
      - targets: ['redis-exporter:9121']
```

### Alert Rules

**Neural Model Performance Alerts**
```yaml
- alert: NeuralPredictionAccuracyLow
  expr: neural_trader_prediction_accuracy < 0.7
  for: 5m
  labels:
    severity: warning
  annotations:
    summary: "Neural prediction accuracy below 70%"

- alert: NeuralModelsNotAvailable  
  expr: neural_trader_models_available == 0
  for: 1m
  labels:
    severity: critical
  annotations:
    summary: "No neural models available"
```

**System Health Alerts**
```yaml
- alert: DAACoordinatorInactive
  expr: rate(neural_trader_daa_decisions_total[5m]) == 0
  for: 10m
  labels:
    severity: critical

- alert: NeuralMemoryUsageHigh
  expr: neural_trader_memory_usage_gb > 3.5
  for: 5m
  labels:
    severity: warning
```

### Retention Policies

- **Raw metrics**: 15 days retention
- **Aggregated data**: 90 days retention  
- **Alert history**: 30 days retention
- **Evaluation interval**: 15 seconds global

## 4. Grafana Dashboards and Visualization

### Dashboard Configurations

**Neural Trader Overview Dashboard**
- System health status indicators
- Real-time prediction accuracy metrics
- Model performance comparisons
- Trading strategy effectiveness
- Resource utilization panels

**Market Data Dashboard**  
- Real-time price feeds from TimescaleDB
- Order book depth visualizations
- Volume and volatility analysis
- Multi-timeframe OHLCV charts
- Data quality metrics

**Infrastructure Monitoring Dashboard**
- TimescaleDB performance metrics
- Redis operations and memory usage
- System resource consumption
- Network and disk I/O statistics
- Container health status

### Key Visualizations

**Time Series Panels**
- Price movements with technical indicators
- Prediction accuracy over time
- System performance metrics
- Error rates and latency trends

**Single Stat Panels**
- Current model accuracy
- Available models count
- System uptime
- Active connections

**Table Panels**
- Top performing models
- Recent predictions
- Alert summary
- Resource usage breakdown

### Alert Integrations

Grafana integrates with Prometheus alerts for:
- Visual alert status on dashboards
- Alert history and annotations
- Custom notification channels
- Alert rule management interface

### User Access

**Admin Users**: Full dashboard editing and alerting
**Operators**: View-only access to operational dashboards  
**Developers**: Access to technical and debugging dashboards
**Stakeholders**: Business metrics and performance summaries

## System Integration and Data Flow

### Data Pipeline Flow

```
Market Data → Redis Cache → TimescaleDB Storage
      ↓              ↓              ↓
Prometheus ← Metrics ← Application ← Queries
      ↓
   Grafana Dashboards
```

### Real-time Processing

1. **Data Ingestion**: Market data flows through Redis pub/sub channels
2. **Caching**: Latest prices cached in Redis with TTL
3. **Persistence**: Bulk writes to TimescaleDB hypertables
4. **Monitoring**: Metrics exported to Prometheus
5. **Visualization**: Real-time updates in Grafana dashboards

### Backup and Recovery

**TimescaleDB Backups**
- Daily automated backups via pg_dump
- Continuous WAL archiving
- Point-in-time recovery capability
- Backup retention: 30 days

**Redis Persistence**
- RDB snapshots every 15 minutes
- AOF logging for durability
- Automatic failover support
- Memory-optimized storage

**Configuration Backups**
- Prometheus configuration versioned
- Grafana dashboards exported as JSON
- Alert rules stored in version control

## Performance Optimization

### TimescaleDB Optimizations

- **Chunking**: 6-hour chunks for optimal query performance
- **Compression**: 7-day compression window, 3-5x storage reduction
- **Indexing**: Optimized B-tree and GIN indexes
- **Parallel Queries**: Multi-core query execution
- **Connection Pooling**: 10 max connections per service

### Redis Optimizations  

- **Memory Management**: 4GB limit with LRU eviction
- **Threading**: 8 I/O threads for concurrent operations
- **Pipelining**: Batched operations for throughput
- **Defragmentation**: Active memory defragmentation
- **Persistence**: Optimized RDB + AOF configuration

### Monitoring Optimizations

- **Scrape Intervals**: Balanced between accuracy and overhead
- **Metric Filtering**: Only relevant metrics collected
- **Retention Tuning**: Appropriate retention for each metric type  
- **Dashboard Optimization**: Efficient queries and refresh rates

## Troubleshooting and Maintenance

### Common Issues

**TimescaleDB**
- Connection pool exhaustion → Increase pool size
- Slow queries → Check indexes and compression
- Disk space → Review retention policies

**Redis**
- Memory pressure → Adjust maxmemory settings
- Connection timeouts → Check network and pooling
- Pub/sub lag → Monitor channel backlog

**Prometheus**  
- Scrape failures → Verify target health
- High cardinality → Review metric labels
- Storage growth → Adjust retention

**Grafana**
- Dashboard load times → Optimize queries
- Alert delays → Check evaluation intervals
- Access issues → Review user permissions

### Maintenance Procedures

**Daily**
- Monitor system health dashboards
- Review error logs and alerts
- Check backup completion status

**Weekly**  
- Analyze performance trends
- Review disk usage and growth
- Update dashboard configurations

**Monthly**
- Performance optimization review
- Backup and recovery testing
- Capacity planning assessment

This comprehensive documentation covers all supporting systems in the Neural Trader platform, providing operators and developers with detailed information for configuration, monitoring, and troubleshooting.