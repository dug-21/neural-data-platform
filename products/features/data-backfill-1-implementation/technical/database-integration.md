# Database Integration Guide: TimescaleDB

## Overview

The Data Backfill System integrates seamlessly with neural-trader's existing TimescaleDB infrastructure, requiring zero schema changes while providing high-performance historical data storage.

## Integration Architecture

### Existing Infrastructure Reuse
```
┌─────────────────────────────────────────────────┐
│           Neural-Trader TimescaleDB             │
├─────────────────────────────────────────────────┤
│                                                 │
│  ┌───────────────┐     ┌───────────────────┐  │
│  │ Existing Pool │     │  market_data      │  │
│  │ Connections   │────▶│   Hypertable      │  │
│  └───────────────┘     └───────────────────┘  │
│          ▲                      ▲              │
│          │                      │              │
│  ┌───────┴────────┐    ┌───────┴────────┐    │
│  │  Real-time     │    │   Backfill      │    │
│  │  Ingestion     │    │   Adapter       │    │
│  └────────────────┘    └────────────────┘    │
│                                                 │
└─────────────────────────────────────────────────┘
```

## Database Schema

### Existing market_data Table
```sql
-- NO CHANGES REQUIRED - Using existing table
CREATE TABLE IF NOT EXISTS market_data (
    time TIMESTAMPTZ NOT NULL,
    symbol TEXT NOT NULL,
    open DOUBLE PRECISION,
    high DOUBLE PRECISION,
    low DOUBLE PRECISION,
    close DOUBLE PRECISION NOT NULL,
    volume BIGINT,
    provider TEXT,      -- Used to identify data source
    metadata JSONB,
    PRIMARY KEY (time, symbol)
);

-- Already configured as hypertable
SELECT create_hypertable('market_data', 'time', 
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);
```

### Provider Identification
The backfill system uses `provider='polygon_s3'` to distinguish historical data:

```sql
-- Query historical data only
SELECT * FROM market_data 
WHERE provider = 'polygon_s3' 
  AND symbol = 'AAPL'
  AND time >= '2024-01-01'
ORDER BY time;

-- Query all data sources
SELECT DISTINCT provider, COUNT(*) as records
FROM market_data
GROUP BY provider;
```

## Integration Implementation

### Storage Adapter Pattern
```python
class BackfillStorageAdapter:
    """Adapts backfill operations to existing TimescaleDB interface"""
    
    def __init__(self, existing_db: TimescaleDB):
        self.db = existing_db  # Reuse existing connection
        self.provider = 'polygon_s3'
        
    async def store_batch(self, records: List[Dict]) -> int:
        """Store batch of records using existing infrastructure"""
        
        # Transform to expected format
        market_data = [
            MarketData(
                time=record['timestamp'],
                symbol=record['symbol'],
                open=record['open'],
                high=record['high'],
                low=record['low'],
                close=record['close'],
                volume=record['volume'],
                provider=self.provider,
                metadata={'source': 's3', 'date': record['date']}
            )
            for record in records
        ]
        
        # Use existing batch insert method
        return await self.db.insert_market_data(market_data)
```

### Connection Pool Reuse
```python
# No new connections needed - reuse existing pool
from data_ingestion.storage.timescale import get_timescale_connection

async def initialize_storage():
    # Get existing connection from pool
    db = await get_timescale_connection()
    
    # Create adapter for backfill operations
    adapter = BackfillStorageAdapter(db)
    
    return adapter
```

## Performance Optimizations

### Batch Insert Optimization
```python
class OptimizedBatchInserter:
    def __init__(self, db_adapter: BackfillStorageAdapter):
        self.adapter = db_adapter
        self.batch_size = 10000  # Optimal for TimescaleDB
        
    async def insert_dataframe(self, df: pd.DataFrame):
        """Insert DataFrame in optimized batches"""
        
        # Prepare data in batches
        for start_idx in range(0, len(df), self.batch_size):
            batch_df = df.iloc[start_idx:start_idx + self.batch_size]
            
            # Convert to records
            records = batch_df.to_dict('records')
            
            # Insert batch
            await self.adapter.store_batch(records)
            
            # Allow other operations
            await asyncio.sleep(0)
```

### Compression Configuration
```sql
-- Enable compression for historical data (already configured)
SELECT add_compression_policy('market_data',
    compress_after => INTERVAL '30 days',
    if_not_exists => TRUE
);

-- Check compression status
SELECT 
    hypertable_name,
    compression_enabled,
    compressed_hypertable_size,
    uncompressed_hypertable_size,
    compression_ratio
FROM timescaledb_information.compression_settings
WHERE hypertable_name = 'market_data';
```

## Continuous Aggregates

### 5-Minute Aggregates
```sql
-- Create 5-minute continuous aggregate for historical data
CREATE MATERIALIZED VIEW IF NOT EXISTS market_data_5min
WITH (timescaledb.continuous) AS
SELECT 
    time_bucket('5 minutes', time) AS bucket,
    symbol,
    first(open, time) as open,
    max(high) as high,
    min(low) as low,
    last(close, time) as close,
    sum(volume) as volume,
    provider,
    count(*) as samples
FROM market_data
WHERE provider = 'polygon_s3'
GROUP BY bucket, symbol, provider;

-- Add refresh policy
SELECT add_continuous_aggregate_policy('market_data_5min',
    start_offset => INTERVAL '1 day',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);
```

### 15-Minute Aggregates
```sql
-- Create 15-minute aggregate
CREATE MATERIALIZED VIEW IF NOT EXISTS market_data_15min
WITH (timescaledb.continuous) AS
SELECT 
    time_bucket('15 minutes', time) AS bucket,
    symbol,
    first(open, time) as open,
    max(high) as high,
    min(low) as low,
    last(close, time) as close,
    sum(volume) as volume,
    provider,
    count(*) as samples
FROM market_data
WHERE provider = 'polygon_s3'
GROUP BY bucket, symbol, provider;
```

## Data Management

### Duplicate Prevention
```python
async def check_existing_data(db, symbol: str, date: date) -> bool:
    """Check if data already exists for symbol/date"""
    
    query = """
    SELECT EXISTS(
        SELECT 1 FROM market_data
        WHERE symbol = $1
          AND date_trunc('day', time) = $2
          AND provider = 'polygon_s3'
        LIMIT 1
    )
    """
    
    return await db.fetchval(query, symbol, date)
```

### Gap Detection
```sql
-- Find gaps in historical data
WITH date_series AS (
    SELECT generate_series(
        '2020-07-23'::date,
        '2025-07-23'::date,
        '1 day'::interval
    )::date AS trading_date
),
existing_data AS (
    SELECT DISTINCT 
        date_trunc('day', time)::date as data_date,
        symbol
    FROM market_data
    WHERE provider = 'polygon_s3'
)
SELECT 
    ds.trading_date,
    s.symbol
FROM date_series ds
CROSS JOIN (SELECT DISTINCT symbol FROM market_data) s
LEFT JOIN existing_data ed 
    ON ds.trading_date = ed.data_date 
    AND s.symbol = ed.symbol
WHERE ed.data_date IS NULL
  AND EXTRACT(dow FROM ds.trading_date) BETWEEN 1 AND 5
ORDER BY ds.trading_date, s.symbol;
```

## Monitoring Queries

### Data Quality Metrics
```sql
-- Data quality dashboard query
SELECT 
    symbol,
    date_trunc('day', time) as date,
    COUNT(*) as minute_count,
    COUNT(DISTINCT time) as unique_minutes,
    AVG(high - low) as avg_range,
    SUM(CASE WHEN high < low THEN 1 ELSE 0 END) as invalid_ohlc,
    MAX(volume) as max_volume,
    MIN(volume) as min_volume
FROM market_data
WHERE provider = 'polygon_s3'
  AND time >= CURRENT_DATE - INTERVAL '7 days'
GROUP BY symbol, date_trunc('day', time)
ORDER BY date DESC, symbol;
```

### Performance Metrics
```sql
-- Insert performance tracking
WITH insert_stats AS (
    SELECT 
        date_trunc('hour', time) as hour,
        COUNT(*) as records_inserted,
        COUNT(DISTINCT symbol) as symbols,
        pg_size_pretty(
            pg_total_relation_size('market_data') - 
            lag(pg_total_relation_size('market_data')) 
                OVER (ORDER BY date_trunc('hour', time))
        ) as size_increase
    FROM market_data
    WHERE provider = 'polygon_s3'
      AND time >= CURRENT_TIMESTAMP - INTERVAL '24 hours'
    GROUP BY hour
)
SELECT 
    hour,
    records_inserted,
    symbols,
    size_increase,
    records_inserted / 3600.0 as records_per_second
FROM insert_stats
ORDER BY hour DESC;
```

## Best Practices

### 1. Connection Management
```python
# Always reuse existing connections
async with get_db_pool() as pool:
    async with pool.acquire() as conn:
        # Perform operations
        await conn.execute(query, params)
```

### 2. Transaction Handling
```python
async def insert_with_transaction(conn, records):
    """Use transactions for consistency"""
    async with conn.transaction():
        try:
            await insert_batch(conn, records)
            await update_checkpoint(conn, checkpoint)
        except Exception as e:
            # Transaction automatically rolled back
            logger.error(f"Transaction failed: {e}")
            raise
```

### 3. Index Usage
```sql
-- Ensure optimal index usage for backfill queries
EXPLAIN (ANALYZE, BUFFERS) 
SELECT * FROM market_data
WHERE symbol = 'AAPL'
  AND provider = 'polygon_s3'
  AND time BETWEEN '2024-01-01' AND '2024-01-31'
ORDER BY time;
```

### 4. Maintenance Operations
```sql
-- Regular maintenance for optimal performance
-- Run during off-peak hours

-- Update statistics
ANALYZE market_data;

-- Reindex if needed (rarely required with TimescaleDB)
REINDEX TABLE market_data;

-- Compress old chunks
SELECT compress_chunk(c.table_name)
FROM timescaledb_information.chunks c
WHERE c.hypertable_name = 'market_data'
  AND c.range_end < NOW() - INTERVAL '30 days'
  AND NOT c.is_compressed;
```

## Troubleshooting

### Common Issues and Solutions

#### 1. Slow Inserts
```sql
-- Check for lock contention
SELECT 
    pid,
    usename,
    application_name,
    state,
    query_start,
    query
FROM pg_stat_activity
WHERE wait_event_type = 'Lock'
  AND query LIKE '%market_data%';
```

#### 2. Storage Growth
```sql
-- Monitor chunk sizes
SELECT 
    hypertable_name,
    chunk_name,
    pg_size_pretty(total_bytes) as size,
    compression_status
FROM timescaledb_information.chunks
WHERE hypertable_name = 'market_data'
ORDER BY total_bytes DESC
LIMIT 10;
```

#### 3. Query Performance
```sql
-- Find slow queries
SELECT 
    query,
    calls,
    mean_exec_time,
    total_exec_time
FROM pg_stat_statements
WHERE query LIKE '%market_data%'
ORDER BY mean_exec_time DESC
LIMIT 10;
```

## Migration Considerations

### Adding New Columns (if needed)
```sql
-- Safe column addition without downtime
ALTER TABLE market_data 
ADD COLUMN IF NOT EXISTS backfill_timestamp TIMESTAMPTZ 
DEFAULT CURRENT_TIMESTAMP;

-- Update only new records
UPDATE market_data 
SET backfill_timestamp = metadata->>'import_time'
WHERE provider = 'polygon_s3'
  AND backfill_timestamp IS NULL;
```

### Data Migration
```python
# Migrate data between providers if needed
async def migrate_provider_data(old_provider: str, new_provider: str):
    """Safely migrate data between providers"""
    
    batch_size = 10000
    offset = 0
    
    while True:
        # Read batch
        records = await db.fetch(
            """
            SELECT * FROM market_data
            WHERE provider = $1
            ORDER BY time, symbol
            LIMIT $2 OFFSET $3
            """,
            old_provider, batch_size, offset
        )
        
        if not records:
            break
            
        # Update provider
        await db.execute(
            """
            UPDATE market_data
            SET provider = $1
            WHERE provider = $2
              AND time >= $3
              AND time <= $4
            """,
            new_provider, old_provider,
            records[0]['time'], records[-1]['time']
        )
        
        offset += batch_size
```

## Conclusion

The Data Backfill System's database integration demonstrates best practices for extending existing infrastructure without disruption. By reusing the current schema and connection management, the system achieves high performance while maintaining compatibility and reliability.

---

*Document Version: 1.0.0 | Last Updated: July 2024*