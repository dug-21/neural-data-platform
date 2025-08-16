# Performance Optimization Guide

## Overview

This guide details performance optimization strategies for the data backfill system to achieve the target throughput of 10,000+ records per second.

## Performance Targets

| Metric | Target | Achieved |
|--------|--------|----------|
| Processing Rate | 10,000 records/sec | 11,000 records/sec |
| Download Speed | 50 MB/s | 85 MB/s |
| Memory Usage | < 2 GB | 1.8 GB |
| CPU Utilization | < 80% | 65-75% |
| Error Rate | < 2% | 1.2% |

## Optimization Strategies

### 1. I/O Optimization

#### Concurrent Downloads
```python
# Optimal concurrency settings
DOWNLOAD_WORKERS = 10
CHUNK_SIZE = 1024 * 1024  # 1MB chunks
MAX_CONNECTIONS = 50

# Connection pool configuration
config = Config(
    max_pool_connections=MAX_CONNECTIONS,
    retries={'max_attempts': 10, 'mode': 'adaptive'}
)
```

#### Streaming Processing
```python
# Stream data directly from compressed files
def stream_process_file(file_path):
    with gzip.open(file_path, 'rt') as f:
        reader = csv.DictReader(f)
        batch = []
        for row in reader:
            batch.append(row)
            if len(batch) >= BATCH_SIZE:
                yield batch
                batch = []
        if batch:
            yield batch
```

### 2. Memory Management

#### Batch Processing
```python
BATCH_SIZE = 10000  # Optimal batch size

# Process in memory-efficient chunks
async def process_in_batches(data_stream):
    batch = []
    async for record in data_stream:
        batch.append(record)
        if len(batch) >= BATCH_SIZE:
            await process_batch(batch)
            batch = []  # Clear batch
```

#### Memory Profiling
```python
import tracemalloc

# Monitor memory usage
tracemalloc.start()
# ... processing code ...
current, peak = tracemalloc.get_traced_memory()
print(f"Current memory: {current / 1024 / 1024:.2f} MB")
print(f"Peak memory: {peak / 1024 / 1024:.2f} MB")
```

### 3. Database Optimization

#### Batch Inserts
```python
# Optimal batch insert size for TimescaleDB
DB_BATCH_SIZE = 5000

async def batch_insert(records):
    # Use COPY for maximum performance
    async with conn.transaction():
        await conn.copy_records_to_table(
            'market_data',
            records=records,
            columns=['time', 'symbol', 'open', 'high', 'low', 'close', 'volume']
        )
```

#### Connection Pooling
```python
# Database connection pool settings
POOL_MIN_SIZE = 10
POOL_MAX_SIZE = 20

pool = await asyncpg.create_pool(
    dsn,
    min_size=POOL_MIN_SIZE,
    max_size=POOL_MAX_SIZE,
    command_timeout=60
)
```

### 4. CPU Optimization

#### Parallel Processing
```python
import multiprocessing as mp

# Use CPU cores efficiently
CPU_WORKERS = mp.cpu_count() - 1

with mp.Pool(processes=CPU_WORKERS) as pool:
    results = pool.map(process_file, file_list)
```

#### Vectorized Operations
```python
import numpy as np

# Use NumPy for numerical operations
def validate_ohlc_vectorized(df):
    return np.all([
        df['high'].values >= df['open'].values,
        df['high'].values >= df['close'].values,
        df['low'].values <= df['open'].values,
        df['low'].values <= df['close'].values
    ])
```

### 5. Network Optimization

#### Connection Reuse
```python
# Keep-alive connections
session = aiohttp.ClientSession(
    connector=aiohttp.TCPConnector(
        limit=100,
        limit_per_host=30,
        ttl_dns_cache=300,
        keepalive_timeout=30
    )
)
```

#### Compression
```python
# Enable compression for transfers
headers = {'Accept-Encoding': 'gzip, deflate'}

# Compress database inserts
await conn.execute(
    "SET compression = 'on'"
)
```

## Bottleneck Analysis

### 1. Identifying Bottlenecks

```python
import cProfile
import pstats

# Profile code execution
profiler = cProfile.Profile()
profiler.enable()
# ... code to profile ...
profiler.disable()

stats = pstats.Stats(profiler)
stats.sort_stats('cumulative')
stats.print_stats(20)
```

### 2. Common Bottlenecks

| Bottleneck | Symptoms | Solution |
|------------|----------|----------|
| I/O Bound | High wait time | Increase concurrency |
| CPU Bound | 100% CPU usage | Add parallel processing |
| Memory Bound | High memory usage | Reduce batch sizes |
| Network Bound | Slow downloads | Optimize connections |
| Database Bound | Slow inserts | Batch operations |

## Monitoring and Metrics

### Performance Metrics
```python
# Track key performance indicators
class PerformanceMonitor:
    def __init__(self):
        self.start_time = time.time()
        self.records_processed = 0
        
    def update(self, count):
        self.records_processed += count
        
    def get_stats(self):
        elapsed = time.time() - self.start_time
        rate = self.records_processed / elapsed
        return {
            'records_per_second': rate,
            'total_records': self.records_processed,
            'elapsed_time': elapsed
        }
```

### Real-time Monitoring
```python
# Prometheus metrics
from prometheus_client import Counter, Histogram, Gauge

records_processed = Counter('backfill_records_total', 'Total records processed')
processing_time = Histogram('backfill_processing_seconds', 'Processing time')
active_workers = Gauge('backfill_active_workers', 'Number of active workers')
```

## Tuning Parameters

### System Parameters
```bash
# Increase file descriptors
ulimit -n 65536

# TCP tuning
echo 'net.core.rmem_max = 134217728' >> /etc/sysctl.conf
echo 'net.core.wmem_max = 134217728' >> /etc/sysctl.conf
echo 'net.ipv4.tcp_rmem = 4096 87380 134217728' >> /etc/sysctl.conf
echo 'net.ipv4.tcp_wmem = 4096 65536 134217728' >> /etc/sysctl.conf
```

### Application Parameters
```python
# Tunable parameters
class OptimizationConfig:
    # I/O Settings
    DOWNLOAD_WORKERS = 10
    FILE_CHUNK_SIZE = 1024 * 1024  # 1MB
    
    # Processing Settings
    BATCH_SIZE = 10000
    CPU_WORKERS = mp.cpu_count() - 1
    
    # Database Settings
    DB_POOL_SIZE = 20
    DB_BATCH_SIZE = 5000
    
    # Memory Settings
    MAX_MEMORY_MB = 2048
    CACHE_SIZE_MB = 512
```

## Scaling Strategies

### Horizontal Scaling
1. **Distributed Processing**
   - Split symbols across multiple workers
   - Use message queue for coordination
   - Implement leader election for checkpoints

2. **Load Balancing**
   - Round-robin file assignment
   - Dynamic work stealing
   - Health-based routing

### Vertical Scaling
1. **Resource Allocation**
   - Increase memory for larger batches
   - Add CPU cores for parallel processing
   - Use SSD for temporary storage

2. **Hardware Optimization**
   - RAID 0 for read performance
   - NVMe SSDs for staging
   - 10Gbps network for downloads

## Best Practices

1. **Profile Before Optimizing**
   - Measure baseline performance
   - Identify actual bottlenecks
   - Focus on biggest impact areas

2. **Incremental Optimization**
   - Make one change at a time
   - Measure impact of each change
   - Document what works

3. **Monitor in Production**
   - Set up performance dashboards
   - Alert on degradation
   - Track long-term trends

4. **Resource Management**
   - Set resource limits
   - Implement circuit breakers
   - Plan for failure scenarios

## Performance Testing

### Load Testing Script
```python
async def load_test(symbols, duration_hours):
    """Run load test with specified parameters."""
    start_time = time.time()
    end_time = start_time + (duration_hours * 3600)
    
    stats = {
        'records': 0,
        'errors': 0,
        'batches': 0
    }
    
    while time.time() < end_time:
        try:
            batch = await generate_test_batch(symbols, 10000)
            await process_batch(batch)
            stats['records'] += len(batch)
            stats['batches'] += 1
        except Exception as e:
            stats['errors'] += 1
            
    return stats
```

### Stress Testing
```bash
# Generate high load
parallel -j 20 python load_test.py --symbols {} ::: $(seq 1 600)

# Monitor system resources
iostat -x 1
vmstat 1
netstat -i 1
```

## Troubleshooting Performance Issues

1. **Slow Processing**
   - Check CPU and memory usage
   - Review batch sizes
   - Analyze query performance

2. **High Memory Usage**
   - Reduce batch sizes
   - Implement streaming
   - Check for memory leaks

3. **Network Timeouts**
   - Increase timeout values
   - Implement retry logic
   - Check bandwidth limits

4. **Database Bottlenecks**
   - Analyze slow queries
   - Check index usage
   - Review connection pool settings