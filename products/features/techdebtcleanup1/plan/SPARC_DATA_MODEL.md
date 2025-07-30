# SPARC Specification: Data Model Design
## Neural Trading Platform Data Architecture

**Project**: Neural Trading Platform  
**Version**: 1.0.0  
**Date**: 2025-07-30  
**Phase**: SPARC Specification - Data Model  
**Status**: PLANNING DOCUMENT

---

## 1. Data Model Overview

The Neural Trading Platform employs a hybrid data architecture combining:
- **TimescaleDB**: Time-series data storage with hypertables for market data
- **Redis**: Real-time caching and event streaming
- **File System**: Model storage and configuration management

## 2. TimescaleDB Schema Design

### 2.1 Core Tables

#### market_data
```sql
CREATE TABLE market_data (
    time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(10) NOT NULL,
    provider VARCHAR(50) NOT NULL,
    price DECIMAL(10, 4) NOT NULL,
    volume BIGINT NOT NULL,
    bid DECIMAL(10, 4),
    ask DECIMAL(10, 4),
    bid_size INTEGER,
    ask_size INTEGER,
    vwap DECIMAL(10, 4),
    trade_count INTEGER,
    metadata JSONB,
    PRIMARY KEY (time, symbol, provider)
);

-- Convert to hypertable for time-series optimization
SELECT create_hypertable('market_data', 'time', 
    chunk_time_interval => INTERVAL '1 day');

-- Create indexes for common queries
CREATE INDEX idx_market_data_symbol_time ON market_data (symbol, time DESC);
CREATE INDEX idx_market_data_provider ON market_data (provider, time DESC);
```

#### predictions
```sql
CREATE TABLE predictions (
    time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(10) NOT NULL,
    model_id VARCHAR(50) NOT NULL,
    model_version VARCHAR(20) NOT NULL,
    horizon_minutes INTEGER NOT NULL,
    predicted_price DECIMAL(10, 4) NOT NULL,
    confidence DECIMAL(5, 4) NOT NULL,
    confidence_lower DECIMAL(10, 4),
    confidence_upper DECIMAL(10, 4),
    features_used JSONB,
    computation_time_ms INTEGER,
    PRIMARY KEY (time, symbol, model_id, horizon_minutes)
);

SELECT create_hypertable('predictions', 'time',
    chunk_time_interval => INTERVAL '1 day');

CREATE INDEX idx_predictions_symbol_model ON predictions (symbol, model_id, time DESC);
CREATE INDEX idx_predictions_confidence ON predictions (confidence DESC);
```

#### trading_decisions
```sql
CREATE TABLE trading_decisions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(10) NOT NULL,
    decision_type VARCHAR(20) NOT NULL, -- 'BUY', 'SELL', 'HOLD', 'CLOSE'
    strategy_name VARCHAR(50) NOT NULL,
    signal_strength DECIMAL(5, 4) NOT NULL,
    position_size DECIMAL(10, 4),
    risk_score DECIMAL(5, 4),
    agent_consensus JSONB NOT NULL, -- Agent votes and reasoning
    market_conditions JSONB,
    execution_status VARCHAR(20) DEFAULT 'PENDING',
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_decisions_symbol_time ON trading_decisions (symbol, time DESC);
CREATE INDEX idx_decisions_status ON trading_decisions (execution_status, created_at DESC);
```

#### orders
```sql
CREATE TABLE orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    decision_id UUID REFERENCES trading_decisions(id),
    symbol VARCHAR(10) NOT NULL,
    order_type VARCHAR(20) NOT NULL, -- 'MARKET', 'LIMIT', 'STOP'
    side VARCHAR(10) NOT NULL, -- 'BUY', 'SELL'
    quantity DECIMAL(10, 4) NOT NULL,
    limit_price DECIMAL(10, 4),
    stop_price DECIMAL(10, 4),
    time_in_force VARCHAR(10) DEFAULT 'DAY',
    status VARCHAR(20) NOT NULL DEFAULT 'NEW',
    broker_order_id VARCHAR(100),
    filled_quantity DECIMAL(10, 4) DEFAULT 0,
    average_fill_price DECIMAL(10, 4),
    commission DECIMAL(10, 4),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    metadata JSONB
);

CREATE INDEX idx_orders_status ON orders (status, created_at DESC);
CREATE INDEX idx_orders_symbol ON orders (symbol, created_at DESC);
CREATE INDEX idx_orders_decision ON orders (decision_id);
```

#### positions
```sql
CREATE TABLE positions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    symbol VARCHAR(10) NOT NULL,
    quantity DECIMAL(10, 4) NOT NULL,
    average_cost DECIMAL(10, 4) NOT NULL,
    current_price DECIMAL(10, 4),
    unrealized_pnl DECIMAL(10, 4),
    realized_pnl DECIMAL(10, 4) DEFAULT 0,
    opened_at TIMESTAMPTZ NOT NULL,
    closed_at TIMESTAMPTZ,
    max_position_value DECIMAL(10, 4),
    min_position_value DECIMAL(10, 4),
    risk_metrics JSONB,
    status VARCHAR(20) DEFAULT 'OPEN',
    strategy_name VARCHAR(50)
);

CREATE INDEX idx_positions_status ON positions (status, symbol);
CREATE INDEX idx_positions_pnl ON positions (unrealized_pnl DESC);
```

#### performance_metrics
```sql
CREATE TABLE performance_metrics (
    time TIMESTAMPTZ NOT NULL,
    metric_type VARCHAR(50) NOT NULL, -- 'NEURAL', 'STRATEGY', 'SYSTEM', 'AGENT'
    metric_name VARCHAR(100) NOT NULL,
    metric_value DECIMAL(20, 6) NOT NULL,
    component_id VARCHAR(100),
    tags JSONB,
    PRIMARY KEY (time, metric_type, metric_name, component_id)
);

SELECT create_hypertable('performance_metrics', 'time',
    chunk_time_interval => INTERVAL '1 hour');

CREATE INDEX idx_metrics_type_time ON performance_metrics (metric_type, time DESC);
CREATE INDEX idx_metrics_component ON performance_metrics (component_id, time DESC);
```

### 2.2 Continuous Aggregates

```sql
-- 1-minute OHLCV aggregation
CREATE MATERIALIZED VIEW market_data_1m
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 minute', time) AS bucket,
    symbol,
    provider,
    FIRST(price, time) AS open,
    MAX(price) AS high,
    MIN(price) AS low,
    LAST(price, time) AS close,
    SUM(volume) AS volume,
    AVG(bid) AS avg_bid,
    AVG(ask) AS avg_ask,
    COUNT(*) AS tick_count
FROM market_data
GROUP BY bucket, symbol, provider;

-- 5-minute aggregation
CREATE MATERIALIZED VIEW market_data_5m
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('5 minutes', time) AS bucket,
    symbol,
    provider,
    FIRST(price, time) AS open,
    MAX(price) AS high,
    MIN(price) AS low,
    LAST(price, time) AS close,
    SUM(volume) AS volume
FROM market_data
GROUP BY bucket, symbol, provider;

-- Prediction accuracy tracking
CREATE MATERIALIZED VIEW prediction_accuracy_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', p.time) AS bucket,
    p.symbol,
    p.model_id,
    p.horizon_minutes,
    COUNT(*) AS prediction_count,
    AVG(ABS(p.predicted_price - m.price) / m.price) AS mean_absolute_percentage_error,
    STDDEV(ABS(p.predicted_price - m.price) / m.price) AS error_stddev,
    AVG(p.confidence) AS avg_confidence,
    AVG(CASE WHEN SIGN(p.predicted_price - LAG(m.price) OVER w) = 
             SIGN(m.price - LAG(m.price) OVER w) THEN 1 ELSE 0 END) AS directional_accuracy
FROM predictions p
JOIN market_data m ON m.symbol = p.symbol 
    AND m.time = p.time + (p.horizon_minutes * INTERVAL '1 minute')
WINDOW w AS (PARTITION BY p.symbol ORDER BY p.time)
GROUP BY bucket, p.symbol, p.model_id, p.horizon_minutes;
```

### 2.3 Compression Policies

```sql
-- Compress data older than 7 days
ALTER TABLE market_data SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol,provider',
    timescaledb.compress_orderby = 'time DESC'
);

SELECT add_compression_policy('market_data', INTERVAL '7 days');

-- Compress predictions after 3 days
ALTER TABLE predictions SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol,model_id',
    timescaledb.compress_orderby = 'time DESC'
);

SELECT add_compression_policy('predictions', INTERVAL '3 days');
```

## 3. Redis Data Structures

### 3.1 Real-time Market Data
```yaml
# Current market snapshot
market:snapshot:{symbol}:
  type: HASH
  fields:
    price: decimal
    bid: decimal
    ask: decimal
    volume: integer
    last_update: timestamp
  ttl: 60 seconds

# Market data stream
market:stream:{symbol}:
  type: STREAM
  fields:
    price: decimal
    volume: integer
    timestamp: unix_timestamp
  retention: 1 hour
  max_length: 10000
```

### 3.2 Trading Signals
```yaml
# Active trading signals
signals:active:{symbol}:
  type: SORTED_SET
  members: signal_id
  scores: signal_strength
  ttl: 300 seconds

# Signal details
signal:detail:{signal_id}:
  type: HASH
  fields:
    symbol: string
    strategy: string
    direction: BUY|SELL
    strength: decimal
    confidence: decimal
    expires_at: timestamp
  ttl: 300 seconds
```

### 3.3 Neural Network Cache
```yaml
# Model predictions cache
predictions:{symbol}:{model_id}:{horizon}:
  type: STRING
  value: JSON encoded prediction
  ttl: 300 seconds

# Feature cache for models
features:{symbol}:latest:
  type: HASH
  fields:
    sma_20: decimal
    sma_50: decimal
    rsi_14: decimal
    macd: decimal
    volume_ratio: decimal
    # ... 50+ features
  ttl: 60 seconds
```

### 3.4 Agent Communication
```yaml
# Agent decision pub/sub
channel:agent:decisions:
  type: PUBSUB
  message_format:
    agent_id: string
    decision_type: string
    symbol: string
    confidence: decimal
    reasoning: object

# Shared agent memory
agent:memory:{topic}:
  type: LIST
  max_length: 100
  message_format:
    timestamp: unix_timestamp
    agent_id: string
    data: object
```

### 3.5 System State
```yaml
# Active positions tracking
positions:active:
  type: HASH
  fields:
    {symbol}: JSON position data
  persistent: true

# Risk metrics
risk:current:
  type: HASH
  fields:
    total_exposure: decimal
    daily_pnl: decimal
    var_95: decimal
    max_drawdown: decimal
  ttl: none (persistent)

# Circuit breaker states
circuit_breaker:{component}:
  type: STRING
  values: CLOSED|OPEN|HALF_OPEN
  ttl: 300 seconds
```

## 4. File System Storage

### 4.1 Model Storage Structure
```
/models/
├── {symbol}/
│   ├── prediction/
│   │   ├── current/          # Active model
│   │   ├── v1/              # Version history
│   │   └── v2/
│   ├── momentum/
│   │   └── current/
│   └── reversal/
│       └── current/
├── ensemble/
│   ├── v1/
│   └── v2/
├── shared/
│   ├── preprocessors/
│   ├── feature_extractors/
│   └── configs/
└── metadata/
    ├── performance/
    └── deployments/
```

### 4.2 Configuration Storage
```
/config/
├── platform.toml            # Core platform config
├── trading.yaml            # Trading strategies config
├── development.toml        # Development overrides
├── production.toml         # Production settings
└── agents.yaml             # Agent configurations
```

## 5. Data Access Patterns

### 5.1 Write Patterns
- **Market Data**: Bulk inserts every second (batch size: 100-1000)
- **Predictions**: Single row inserts with 5-minute intervals
- **Decisions**: Event-driven inserts on signal generation
- **Metrics**: Batch inserts every minute

### 5.2 Read Patterns
- **Recent Data**: Last 1 hour from Redis cache
- **Historical Analysis**: Time-range queries on TimescaleDB
- **Model Training**: Bulk export of historical data
- **Real-time Monitoring**: Redis pub/sub streams

### 5.3 Query Optimization
```sql
-- Partition-wise aggregation for performance
SET timescaledb.optimize_aggregation = on;

-- Parallel query execution
SET max_parallel_workers_per_gather = 4;

-- Memory allocation for sorting
SET work_mem = '256MB';
```

## 6. Data Retention Policies

### 6.1 TimescaleDB Retention
```sql
-- Drop raw market data after 90 days
SELECT add_retention_policy('market_data', INTERVAL '90 days');

-- Keep predictions for 30 days
SELECT add_retention_policy('predictions', INTERVAL '30 days');

-- Keep aggregates longer
SELECT add_retention_policy('market_data_1m', INTERVAL '1 year');
SELECT add_retention_policy('market_data_5m', INTERVAL '2 years');
```

### 6.2 Redis Expiration
- Market snapshots: 60 seconds
- Trading signals: 5 minutes
- Prediction cache: 5 minutes
- Feature cache: 60 seconds
- Agent memory: 1 hour

## 7. Data Integrity Constraints

### 7.1 Business Rules
- Prices must be positive
- Volumes must be non-negative
- Confidence scores between 0 and 1
- Timestamps must be UTC
- Symbol format validation

### 7.2 Referential Integrity
- Orders reference trading_decisions
- Positions track order history
- Predictions reference valid models
- Metrics reference existing components

## 8. Backup and Recovery

### 8.1 Backup Strategy
```bash
# Continuous WAL archiving
archive_mode = on
archive_command = 'cp %p /backup/wal/%f'

# Daily base backups
pg_basebackup -D /backup/base -Ft -z -P

# Redis persistence
save 900 1      # After 900 sec if at least 1 key changed
save 300 10     # After 300 sec if at least 10 keys changed
save 60 10000   # After 60 sec if at least 10000 keys changed
```

### 8.2 Recovery Procedures
- Point-in-time recovery for TimescaleDB
- Redis AOF replay for cache recovery
- Model rollback from versioned storage
- Configuration restore from Git

## 9. Performance Considerations

### 9.1 Indexing Strategy
- Time-based indexes for all time-series tables
- Symbol indexes for market data queries
- Composite indexes for common query patterns
- Partial indexes for status fields

### 9.2 Connection Pooling
```yaml
timescale:
  max_connections: 20
  min_connections: 5
  connection_timeout: 5s

redis:
  pool_size: 20
  max_retries: 3
  retry_delay: 100ms
```

### 9.3 Caching Strategy
- L1 Cache: Redis for hot data
- L2 Cache: TimescaleDB continuous aggregates
- L3 Cache: Compressed historical data
- Cache invalidation on updates

## 10. Migration Strategy

### 10.1 Schema Versioning
```sql
CREATE TABLE schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TIMESTAMPTZ DEFAULT NOW(),
    description TEXT
);
```

### 10.2 Data Migration Tools
- TimescaleDB native backup/restore
- Redis RDB/AOF migration
- Custom ETL scripts for format changes
- Incremental migration support

---

**Document Status**: This is a planning document for the SPARC specification phase. The data model should be validated against use cases before implementation.