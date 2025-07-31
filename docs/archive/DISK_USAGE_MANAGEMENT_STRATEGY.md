# Disk Usage Management Strategy for Neural Trader

## Executive Summary

This document outlines a comprehensive disk usage management strategy for the Neural Trader platform, designed to operate efficiently in containerized environments with limited disk space. The strategy focuses on streaming data processing, intelligent caching, and automatic cleanup mechanisms to minimize disk footprint while maintaining system performance.

## Current Disk Usage Analysis

### Data Storage Components

1. **TimescaleDB (PostgreSQL)**
   - Market data: ~96 bytes per OHLCV record
   - Tick data: ~64 bytes per tick
   - Technical indicators: ~120 bytes per indicator
   - Estimated growth: 2-5 GB/day for 100 symbols

2. **Redis (In-Memory)**
   - Real-time price cache: ~500 bytes per symbol
   - Order book snapshots: ~2 KB per symbol
   - Tick buffer: ~50 MB for recent data
   - Total: ~200 MB typical usage

3. **Application Logs**
   - Data ingestion logs: ~100 MB/day
   - Trading system logs: ~50 MB/day
   - System monitoring: ~20 MB/day

4. **Container Images**
   - Base images: ~2 GB total
   - Application layers: ~500 MB
   - Dependencies: ~1 GB

## Disk Usage Management Strategy

### 1. Streaming Data Architecture

```yaml
streaming_pipeline:
  ingestion:
    - Stream data directly from providers
    - Process in-memory without disk buffering
    - Use Redis for temporary buffering only
  
  processing:
    - Apply transformations on-the-fly
    - Aggregate data before storage
    - Compress data in transit
  
  storage:
    - Direct write to TimescaleDB
    - Skip intermediate file storage
    - Use batch inserts for efficiency
```

### 2. Rolling Window Strategy

```python
class RollingWindowManager:
    """Manages data retention with rolling windows"""
    
    retention_policies = {
        'tick_data': {'window': '24 hours', 'compression': '1 hour'},
        'minute_data': {'window': '7 days', 'compression': '1 day'},
        'hourly_data': {'window': '30 days', 'compression': '7 days'},
        'daily_data': {'window': '1 year', 'compression': '30 days'},
        'logs': {'window': '3 days', 'compression': None}
    }
    
    async def apply_retention(self):
        """Apply retention policies to all data types"""
        for data_type, policy in self.retention_policies.items():
            await self.cleanup_old_data(data_type, policy['window'])
            if policy['compression']:
                await self.compress_data(data_type, policy['compression'])
```

### 3. Intelligent Caching System

```yaml
cache_strategy:
  redis_cache:
    hot_data:
      - Latest prices (1 hour TTL)
      - Active order books (1 minute TTL)
      - Recent trades (5 minute TTL)
    
    eviction_policy: allkeys-lru
    max_memory: 512MB
    
  disk_cache:
    technical_indicators:
      - Pre-calculated values
      - Compressed format
      - Max size: 100MB
    
    cleanup_policy:
      - Remove unused indicators daily
      - Compress after 24 hours
```

### 4. Data Compression Strategy

```sql
-- TimescaleDB compression policies
SELECT add_compression_policy('market_data', 
    compress_after => INTERVAL '7 days',
    cascade_to_materializations => true
);

SELECT add_compression_policy('tick_data',
    compress_after => INTERVAL '1 day'
);

-- Compression ratios: 10:1 for market data, 5:1 for tick data
```

### 5. Temporary File Management

```python
class TempFileManager:
    """Manages temporary files with automatic cleanup"""
    
    def __init__(self, max_disk_usage_mb=500):
        self.temp_dir = Path("/tmp/neural_trader")
        self.max_disk_usage = max_disk_usage_mb * 1024 * 1024
        
    async def cleanup_old_files(self):
        """Remove files older than threshold"""
        threshold = datetime.now() - timedelta(hours=1)
        
        for file in self.temp_dir.glob("*"):
            if file.stat().st_mtime < threshold.timestamp():
                file.unlink()
                
    async def enforce_disk_limit(self):
        """Ensure disk usage stays within limits"""
        total_size = sum(f.stat().st_size for f in self.temp_dir.glob("*"))
        
        if total_size > self.max_disk_usage:
            # Remove oldest files first
            files = sorted(self.temp_dir.glob("*"), 
                         key=lambda f: f.stat().st_mtime)
            
            for file in files:
                file.unlink()
                total_size -= file.stat().st_size
                if total_size <= self.max_disk_usage * 0.8:
                    break
```

### 6. Container-Specific Optimizations

```dockerfile
# Multi-stage build to reduce image size
FROM rust:1.75 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

# Minimal runtime image
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/neural-trader /usr/local/bin/

# Use tmpfs for temporary data
VOLUME ["/tmp"]

# Configure small log rotation
ENV LOG_MAX_SIZE="10M"
ENV LOG_MAX_FILES="3"
```

### 7. Monitoring and Alerting

```yaml
disk_monitoring:
  metrics:
    - disk_usage_percentage
    - inodes_usage
    - write_throughput
    - compression_ratio
  
  alerts:
    - threshold: 80%
      action: trigger_cleanup
    - threshold: 90%
      action: pause_non_critical_operations
    - threshold: 95%
      action: emergency_cleanup
```

## Implementation Plan

### Phase 1: Immediate Optimizations (Week 1)
1. Implement streaming data pipeline
2. Configure Redis memory limits
3. Set up basic retention policies
4. Enable TimescaleDB compression

### Phase 2: Advanced Features (Week 2-3)
1. Implement rolling window manager
2. Create intelligent cache system
3. Add temporary file management
4. Optimize container images

### Phase 3: Monitoring and Automation (Week 4)
1. Deploy disk usage monitoring
2. Implement automated cleanup
3. Add performance metrics
4. Create alerting system

## Expected Disk Usage

### Minimal Configuration
- Container images: 1.5 GB
- Database (7 days): 2 GB
- Redis: 200 MB
- Logs (compressed): 100 MB
- **Total: ~4 GB**

### Standard Configuration
- Container images: 2 GB
- Database (30 days): 10 GB
- Redis: 512 MB
- Logs (compressed): 500 MB
- Cache: 1 GB
- **Total: ~14 GB**

### Performance Considerations

1. **Write Performance**
   - Batch inserts: 10,000 records/second
   - Compression overhead: <5% CPU
   - Cleanup overhead: <2% CPU

2. **Query Performance**
   - Hot data in Redis: <1ms latency
   - Recent data (uncompressed): <10ms
   - Historical data (compressed): <50ms

## Disaster Recovery

```yaml
backup_strategy:
  continuous:
    - Redis snapshots every hour
    - TimescaleDB WAL archiving
    - Log rotation and compression
  
  daily:
    - Full database backup (compressed)
    - Archive to external storage
    - Cleanup local backups > 3 days
```

## Best Practices

1. **Development Environment**
   - Use Docker volumes for persistent data
   - Enable aggressive cleanup policies
   - Limit historical data to 7 days

2. **Production Environment**
   - Use dedicated storage volumes
   - Implement tiered storage strategy
   - Monitor disk usage continuously

3. **Emergency Procedures**
   - Automatic pause of data ingestion at 95% disk usage
   - Emergency cleanup script for critical situations
   - Fallback to in-memory only mode

## Conclusion

This disk usage management strategy ensures the Neural Trader platform can operate efficiently in containerized environments with limited disk space. By implementing streaming data processing, intelligent caching, and automatic cleanup mechanisms, the system maintains a minimal disk footprint while preserving performance and reliability.

The strategy is designed to scale from minimal deployments (4 GB) to standard configurations (14 GB) while providing automatic safeguards against disk exhaustion. Regular monitoring and automated cleanup ensure the system remains responsive and efficient regardless of deployment constraints.