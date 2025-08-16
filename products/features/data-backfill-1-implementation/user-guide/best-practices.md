# Best Practices Guide

## Overview

This guide provides best practices for running efficient, reliable, and safe data backfill operations based on real-world experience and optimization strategies.

## Planning Your Backfill

### 1. Capacity Planning

#### Storage Requirements
```python
# Calculate storage needs
def estimate_storage(symbols, years, compression_ratio=0.3):
    """
    Estimate storage requirements for backfill.
    
    Assumptions:
    - 1 GB per symbol per year (uncompressed)
    - 30% compression ratio for gzipped data
    - 20% overhead for indexes and metadata
    """
    base_size_gb = symbols * years
    compressed_size_gb = base_size_gb * compression_ratio
    total_with_overhead_gb = compressed_size_gb * 1.2
    
    return {
        'raw_data_gb': base_size_gb,
        'compressed_gb': compressed_size_gb,
        'total_required_gb': total_with_overhead_gb,
        'recommended_free_gb': total_with_overhead_gb * 1.5  # 50% buffer
    }

# Example: 600 symbols, 5 years
estimate = estimate_storage(600, 5)
print(f"Required storage: {estimate['total_required_gb']:.0f} GB")
print(f"Recommended free space: {estimate['recommended_free_gb']:.0f} GB")
```

#### Time Estimation
```python
# Estimate completion time
def estimate_duration(total_gb, download_speed_mbps=400, processing_rate_rps=10000):
    """
    Estimate backfill duration.
    
    Factors:
    - Network download speed
    - Processing rate
    - Database insert speed
    """
    download_hours = (total_gb * 1024 * 8) / (download_speed_mbps * 3600)
    
    # Assume 390 minutes/day * 252 trading days/year
    records_per_symbol_year = 390 * 252
    total_records = estimate['symbols'] * estimate['years'] * records_per_symbol_year
    processing_hours = total_records / (processing_rate_rps * 3600)
    
    # Total time (download and processing can overlap)
    total_hours = max(download_hours, processing_hours * 1.2)  # 20% overhead
    
    return {
        'download_hours': download_hours,
        'processing_hours': processing_hours,
        'total_hours': total_hours,
        'total_days': total_hours / 24
    }
```

### 2. Pre-flight Checklist

Before starting a large backfill:

- [ ] **Storage Space**: Verify sufficient disk space (1.5x estimated requirement)
- [ ] **Database Capacity**: Check database has room for growth
- [ ] **Network Bandwidth**: Confirm network can handle sustained traffic
- [ ] **AWS Credentials**: Test S3 access and permissions
- [ ] **System Resources**: Ensure adequate CPU and memory
- [ ] **Backup Strategy**: Have database backup before starting
- [ ] **Monitoring Setup**: Configure alerts and dashboards
- [ ] **Rollback Plan**: Document how to revert if needed

## Optimization Strategies

### 1. Symbol Prioritization

Process most important symbols first:

```python
# Priority-based symbol ordering
SYMBOL_PRIORITIES = {
    # Tier 1: Most liquid/important
    'tier1': ['AAPL', 'MSFT', 'GOOGL', 'AMZN', 'META', 'TSLA', 'NVDA'],
    
    # Tier 2: Large cap
    'tier2': ['JPM', 'V', 'JNJ', 'WMT', 'PG', 'MA', 'UNH'],
    
    # Tier 3: Rest of S&P 500
    'tier3': [...],
    
    # Tier 4: Additional symbols
    'tier4': [...]
}

# Process in priority order
def get_symbol_order(tiers):
    symbols = []
    for tier in ['tier1', 'tier2', 'tier3', 'tier4']:
        symbols.extend(SYMBOL_PRIORITIES.get(tier, []))
    return symbols
```

### 2. Parallel Processing

Optimize parallelism based on resources:

```python
# Dynamic worker allocation
def calculate_optimal_workers(cpu_cores, memory_gb, network_mbps):
    """
    Calculate optimal number of workers based on system resources.
    """
    # CPU-bound limit
    cpu_workers = cpu_cores - 1  # Leave one core for system
    
    # Memory-bound limit (assume 500MB per worker)
    memory_workers = int(memory_gb * 1024 / 500)
    
    # Network-bound limit (assume 10 Mbps per worker)
    network_workers = int(network_mbps / 10)
    
    # Take minimum to avoid overload
    optimal = min(cpu_workers, memory_workers, network_workers)
    
    # Cap at reasonable maximum
    return min(optimal, 20)

# Example
workers = calculate_optimal_workers(
    cpu_cores=16,
    memory_gb=32,
    network_mbps=1000
)
print(f"Optimal workers: {workers}")
```

### 3. Batch Size Tuning

Find optimal batch size:

```python
# Adaptive batch sizing
class AdaptiveBatcher:
    def __init__(self, initial_size=10000):
        self.batch_size = initial_size
        self.min_size = 1000
        self.max_size = 100000
        self.performance_history = []
    
    def adjust_batch_size(self, records_processed, time_taken):
        """Adjust batch size based on performance."""
        rate = records_processed / time_taken
        self.performance_history.append(rate)
        
        if len(self.performance_history) >= 3:
            recent_avg = sum(self.performance_history[-3:]) / 3
            overall_avg = sum(self.performance_history) / len(self.performance_history)
            
            if recent_avg > overall_avg * 1.1:
                # Performance improving, increase batch size
                self.batch_size = min(int(self.batch_size * 1.2), self.max_size)
            elif recent_avg < overall_avg * 0.9:
                # Performance degrading, decrease batch size
                self.batch_size = max(int(self.batch_size * 0.8), self.min_size)
        
        return self.batch_size
```

## Error Handling

### 1. Retry Strategy

Implement intelligent retry logic:

```python
from functools import wraps
import time
import random

def smart_retry(max_attempts=5, base_delay=1, max_delay=300):
    """
    Decorator for intelligent retry with exponential backoff and jitter.
    """
    def decorator(func):
        @wraps(func)
        async def wrapper(*args, **kwargs):
            last_exception = None
            
            for attempt in range(max_attempts):
                try:
                    return await func(*args, **kwargs)
                except Exception as e:
                    last_exception = e
                    
                    # Determine if error is retryable
                    if not is_retryable_error(e):
                        raise
                    
                    # Calculate delay with exponential backoff and jitter
                    delay = min(base_delay * (2 ** attempt), max_delay)
                    jitter = random.uniform(0, delay * 0.1)
                    total_delay = delay + jitter
                    
                    logger.warning(
                        f"Attempt {attempt + 1}/{max_attempts} failed: {e}. "
                        f"Retrying in {total_delay:.1f}s..."
                    )
                    
                    await asyncio.sleep(total_delay)
            
            raise last_exception
        return wrapper
    return decorator

def is_retryable_error(error):
    """Determine if an error should trigger a retry."""
    retryable_errors = [
        ConnectionError,
        TimeoutError,
        'RequestLimitExceeded',
        'ServiceUnavailable',
        'ThrottlingException'
    ]
    
    return any(
        isinstance(error, err) if isinstance(err, type) else err in str(error)
        for err in retryable_errors
    )
```

### 2. Circuit Breaker

Prevent cascading failures:

```python
class CircuitBreaker:
    def __init__(self, failure_threshold=5, recovery_timeout=60):
        self.failure_threshold = failure_threshold
        self.recovery_timeout = recovery_timeout
        self.failure_count = 0
        self.last_failure_time = None
        self.state = 'closed'  # closed, open, half-open
    
    async def call(self, func, *args, **kwargs):
        if self.state == 'open':
            if time.time() - self.last_failure_time > self.recovery_timeout:
                self.state = 'half-open'
            else:
                raise Exception("Circuit breaker is open")
        
        try:
            result = await func(*args, **kwargs)
            if self.state == 'half-open':
                self.state = 'closed'
                self.failure_count = 0
            return result
        except Exception as e:
            self.failure_count += 1
            self.last_failure_time = time.time()
            
            if self.failure_count >= self.failure_threshold:
                self.state = 'open'
                logger.error(f"Circuit breaker opened after {self.failure_count} failures")
            
            raise
```

## Database Optimization

### 1. Pre-backfill Optimization

Prepare database for bulk loading:

```sql
-- Disable autovacuum during bulk load
ALTER TABLE market_data SET (autovacuum_enabled = false);

-- Increase maintenance memory
SET maintenance_work_mem = '2GB';

-- Disable synchronous commit for speed
SET synchronous_commit = 'off';

-- Increase checkpoint segments
SET checkpoint_segments = 100;

-- Drop non-critical indexes
DROP INDEX IF EXISTS idx_market_data_provider;
DROP INDEX IF EXISTS idx_market_data_metadata;

-- Keep only essential indexes
-- Primary key and symbol-time index
```

### 2. Batch Insert Optimization

Use optimal insert strategies:

```python
async def optimized_batch_insert(conn, records, table='market_data'):
    """
    Optimized batch insert using COPY command.
    """
    # Create temporary file
    import tempfile
    import csv
    
    with tempfile.NamedTemporaryFile(mode='w', delete=False) as tmp:
        writer = csv.writer(tmp)
        
        # Write records
        for record in records:
            writer.writerow([
                record['time'],
                record['symbol'],
                record['open'],
                record['high'],
                record['low'],
                record['close'],
                record['volume'],
                record['provider']
            ])
        
        tmp_path = tmp.name
    
    # Use COPY for fastest insert
    try:
        await conn.copy_to_table(
            table,
            source=tmp_path,
            columns=['time', 'symbol', 'open', 'high', 'low', 'close', 'volume', 'provider'],
            format='csv'
        )
    finally:
        os.unlink(tmp_path)
```

### 3. Post-backfill Optimization

Restore performance after bulk load:

```sql
-- Re-enable autovacuum
ALTER TABLE market_data SET (autovacuum_enabled = true);

-- Create indexes concurrently (doesn't block reads)
CREATE INDEX CONCURRENTLY idx_market_data_provider ON market_data(provider);
CREATE INDEX CONCURRENTLY idx_market_data_volume ON market_data(volume) WHERE volume > 1000000;

-- Update statistics
ANALYZE market_data;

-- Create continuous aggregates for common queries
CREATE MATERIALIZED VIEW market_data_daily
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 day', time) AS day,
    symbol,
    first(open, time) AS open,
    max(high) AS high,
    min(low) AS low,
    last(close, time) AS close,
    sum(volume) AS volume
FROM market_data
GROUP BY day, symbol;

-- Add compression policy
SELECT add_compression_policy('market_data', INTERVAL '7 days');
```

## Monitoring and Alerting

### 1. Key Metrics to Monitor

Set up monitoring for these critical metrics:

```yaml
# Prometheus alerts
groups:
  - name: backfill_alerts
    rules:
      - alert: BackfillSlowProcessing
        expr: rate(backfill_records_processed_total[5m]) < 5000
        for: 10m
        annotations:
          summary: "Backfill processing rate below threshold"
          
      - alert: BackfillHighErrorRate
        expr: rate(backfill_errors_total[5m]) / rate(backfill_records_processed_total[5m]) > 0.01
        for: 5m
        annotations:
          summary: "Backfill error rate above 1%"
          
      - alert: BackfillDiskSpaceLow
        expr: node_filesystem_avail_bytes{mountpoint="/mnt/data"} < 100*1024*1024*1024
        for: 5m
        annotations:
          summary: "Less than 100GB free on data drive"
          
      - alert: BackfillDatabaseConnectionsHigh
        expr: pg_stat_database_numbackends{datname="trading"} > 90
        for: 5m
        annotations:
          summary: "Database connections near limit"
```

### 2. Dashboard Setup

Essential Grafana panels:

1. **Processing Rate**: Records/second over time
2. **Error Rate**: Percentage of failed records
3. **Progress**: Percentage complete by symbol
4. **System Resources**: CPU, memory, disk I/O
5. **Database Metrics**: Connections, query time, disk usage
6. **Network Traffic**: Download speed, S3 requests

## Security Considerations

### 1. Credential Management

Never expose credentials:

```python
# Use environment variables or secure vaults
import os
from cryptography.fernet import Fernet

class SecureCredentialManager:
    def __init__(self, key_file='/etc/neural_trader/master.key'):
        with open(key_file, 'rb') as f:
            self.cipher = Fernet(f.read())
    
    def get_credential(self, name):
        """Retrieve and decrypt credential."""
        encrypted = os.environ.get(f'ENCRYPTED_{name}')
        if encrypted:
            return self.cipher.decrypt(encrypted.encode()).decode()
        
        # Fallback to unencrypted (dev only)
        return os.environ.get(name)
```

### 2. Access Control

Implement least privilege:

```sql
-- Create read-only user for verification
CREATE USER backfill_reader WITH PASSWORD 'secure_password';
GRANT CONNECT ON DATABASE trading TO backfill_reader;
GRANT USAGE ON SCHEMA public TO backfill_reader;
GRANT SELECT ON ALL TABLES IN SCHEMA public TO backfill_reader;

-- Create write-only user for backfill
CREATE USER backfill_writer WITH PASSWORD 'secure_password';
GRANT CONNECT ON DATABASE trading TO backfill_writer;
GRANT USAGE ON SCHEMA public TO backfill_writer;
GRANT INSERT ON market_data TO backfill_writer;
-- No UPDATE or DELETE permissions
```

## Recovery Procedures

### 1. Checkpoint Recovery

Resume from interruption:

```python
async def safe_resume(checkpoint_file):
    """Safely resume backfill from checkpoint."""
    # Validate checkpoint
    checkpoint = load_checkpoint(checkpoint_file)
    
    if not validate_checkpoint(checkpoint):
        logger.error("Invalid checkpoint, starting fresh")
        return None
    
    # Verify last successful state
    last_record = await verify_last_record(
        checkpoint['last_symbol'],
        checkpoint['last_timestamp']
    )
    
    if not last_record:
        # Rollback to previous checkpoint
        checkpoint = rollback_checkpoint(checkpoint)
    
    logger.info(f"Resuming from: {checkpoint['last_symbol']} at {checkpoint['last_timestamp']}")
    return checkpoint
```

### 2. Data Validation

Post-backfill verification:

```python
async def validate_backfill(symbols, start_date, end_date):
    """Comprehensive validation of backfilled data."""
    issues = []
    
    for symbol in symbols:
        # Check completeness
        missing_dates = await find_missing_dates(symbol, start_date, end_date)
        if missing_dates:
            issues.append(f"{symbol}: Missing {len(missing_dates)} days")
        
        # Check consistency
        invalid_records = await validate_ohlc_consistency(symbol, start_date, end_date)
        if invalid_records:
            issues.append(f"{symbol}: {len(invalid_records)} invalid OHLC records")
        
        # Check duplicates
        duplicates = await find_duplicates(symbol, start_date, end_date)
        if duplicates:
            issues.append(f"{symbol}: {len(duplicates)} duplicate records")
    
    return issues
```

## Common Pitfalls to Avoid

1. **Don't skip the dry run** - Always test with a small dataset first
2. **Don't ignore warnings** - Address all warnings before proceeding
3. **Don't disable checkpoints** - The small performance gain isn't worth the risk
4. **Don't overload the system** - Leave headroom for other processes
5. **Don't forget monitoring** - Set up alerts before starting
6. **Don't skip validation** - Always verify data integrity after backfill
7. **Don't hardcode credentials** - Use proper credential management
8. **Don't ignore failed records** - Investigate and reprocess failures

## Recommended Workflow

1. **Development Environment**
   - Test with 1 symbol, 1 week of data
   - Verify all components work correctly
   - Document any issues

2. **Staging Environment**
   - Test with 10 symbols, 1 month of data
   - Measure performance metrics
   - Tune parameters

3. **Production - Phase 1**
   - Start with tier 1 symbols (most important)
   - Monitor closely for first 24 hours
   - Adjust parameters based on performance

4. **Production - Phase 2**
   - Expand to remaining symbols
   - Run in batches of 100 symbols
   - Validate each batch before proceeding

5. **Post-Backfill**
   - Run comprehensive validation
   - Create performance report
   - Document lessons learned

## Performance Benchmarks

Expected performance under optimal conditions:

| Metric | Target | Minimum Acceptable |
|--------|--------|-------------------|
| Download Speed | 100 MB/s | 50 MB/s |
| Processing Rate | 15,000 rec/s | 10,000 rec/s |
| Error Rate | < 0.1% | < 1% |
| Memory Usage | < 4 GB | < 8 GB |
| CPU Usage | 60-80% | < 95% |

## Maintenance Schedule

Regular maintenance for optimal performance:

- **Daily**: Check error logs, monitor progress
- **Weekly**: Validate recent data, update statistics
- **Monthly**: Archive old checkpoints, analyze performance trends
- **Quarterly**: Review and update configuration, plan capacity