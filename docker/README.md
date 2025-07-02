# Neural Trader Docker Setup

This directory contains the Docker configuration for the Neural Trader autonomous trading platform, including TimescaleDB for time-series data storage and Redis for caching and real-time data management.

## Overview

The Docker setup includes:
- **TimescaleDB**: PostgreSQL with time-series optimization for market data and predictions
- **Redis**: High-performance caching and pub/sub for real-time data

## Quick Start

1. **Start the services**:
   ```bash
   cd /Users/dmf/repos/neural-trader/docker
   docker-compose up -d
   ```

2. **Verify services are running**:
   ```bash
   docker-compose ps
   ```

3. **Check logs**:
   ```bash
   docker-compose logs -f timescaledb
   docker-compose logs -f redis
   ```

## Services

### TimescaleDB

- **Connection URL**: `postgresql://neural_trader:neural_trader_pass@localhost:5432/neural_trader_db`
- **Port**: 5432
- **Database**: neural_trader_db
- **Username**: neural_trader
- **Password**: neural_trader_pass
- **Schema**: neural_trader

#### Tables Created:
- `time_series_data`: Market data (OHLCV + order book)
- `predictions`: Model predictions with confidence scores
- `trades`: Executed trades tracking
- `performance_metrics`: Strategy performance metrics

#### Features:
- Hypertables for automatic time-based partitioning
- Continuous aggregates for fast OHLCV queries
- Data retention policies (90 days for raw data)
- Optimized indexes for symbol and time-based queries

### Redis

- **Connection URL**: `redis://localhost:6379`
- **Port**: 6379
- **Max Memory**: 4GB
- **Persistence**: AOF + RDB enabled
- **Eviction Policy**: LRU (Least Recently Used)

#### Use Cases:
- Real-time price caching
- Order book state management
- Pub/sub for price updates
- Strategy state caching
- Rate limiting
- Session management

## Common Operations

### Connect to TimescaleDB

```bash
# Using psql
docker exec -it neural_trader_timescaledb psql -U neural_trader -d neural_trader_db

# Using docker-compose
docker-compose exec timescaledb psql -U neural_trader -d neural_trader_db
```

### Connect to Redis

```bash
# Using redis-cli
docker exec -it neural_trader_redis redis-cli

# Using docker-compose
docker-compose exec redis redis-cli
```

### View Continuous Aggregates

```sql
-- In psql
SET search_path TO neural_trader;
\d+ hourly_ohlcv
SELECT * FROM hourly_ohlcv WHERE symbol = 'BTC/USD' ORDER BY hour DESC LIMIT 10;
```

### Monitor Redis Memory

```bash
docker-compose exec redis redis-cli info memory
```

## Data Management

### Backup TimescaleDB

```bash
# Full backup
docker-compose exec timescaledb pg_dump -U neural_trader neural_trader_db > backup_$(date +%Y%m%d_%H%M%S).sql

# Compressed backup
docker-compose exec timescaledb pg_dump -U neural_trader -Fc neural_trader_db > backup_$(date +%Y%m%d_%H%M%S).dump
```

### Restore TimescaleDB

```bash
# From SQL file
docker-compose exec -T timescaledb psql -U neural_trader neural_trader_db < backup.sql

# From compressed dump
docker-compose exec -T timescaledb pg_restore -U neural_trader -d neural_trader_db < backup.dump
```

### Backup Redis

```bash
# Trigger manual save
docker-compose exec redis redis-cli BGSAVE

# Copy backup files
docker cp neural_trader_redis:/data/dump.rdb ./redis_backup_$(date +%Y%m%d_%H%M%S).rdb
docker cp neural_trader_redis:/data/appendonly.aof ./redis_backup_$(date +%Y%m%d_%H%M%S).aof
```

## Performance Tuning

### TimescaleDB Tuning

For production workloads, consider:

1. Increase shared_buffers (25% of RAM)
2. Tune work_mem for complex queries
3. Adjust chunk_time_interval based on data volume
4. Create additional indexes for specific query patterns

### Redis Tuning

For high-frequency trading:

1. Increase `hz` value for faster expiry handling
2. Tune `maxmemory` based on available RAM
3. Consider using Redis Cluster for horizontal scaling
4. Enable Redis Modules like RedisTimeSeries for tick data

## Monitoring

### TimescaleDB Monitoring Queries

```sql
-- Check chunk sizes
SELECT hypertable_name, 
       chunk_name, 
       range_start, 
       range_end, 
       total_bytes/1024/1024 as size_mb 
FROM timescaledb_information.chunks 
ORDER BY range_start DESC;

-- Monitor continuous aggregate refresh
SELECT * FROM timescaledb_information.continuous_aggregate_stats;

-- Check compression status (if enabled)
SELECT * FROM timescaledb_information.compressed_chunk_stats;
```

### Redis Monitoring Commands

```bash
# Monitor commands in real-time
docker-compose exec redis redis-cli monitor

# Check slow queries
docker-compose exec redis redis-cli slowlog get 10

# Memory stats
docker-compose exec redis redis-cli memory stats
```

## Troubleshooting

### Container won't start

1. Check ports are not in use:
   ```bash
   lsof -i :5432
   lsof -i :6379
   ```

2. Check docker logs:
   ```bash
   docker-compose logs --tail=50
   ```

3. Reset volumes (WARNING: Deletes all data):
   ```bash
   docker-compose down -v
   docker-compose up -d
   ```

### Performance Issues

1. Check TimescaleDB chunk sizes:
   ```sql
   SELECT pg_size_pretty(pg_database_size('neural_trader_db'));
   ```

2. Monitor Redis memory:
   ```bash
   docker-compose exec redis redis-cli --latency
   ```

### Connection Issues

1. Verify containers are on the same network:
   ```bash
   docker network inspect docker_neural_trader_net
   ```

2. Test connectivity:
   ```bash
   docker-compose exec timescaledb pg_isready
   docker-compose exec redis redis-cli ping
   ```

## Security Considerations

For production deployment:

1. Change default passwords in docker-compose.yml
2. Enable Redis password authentication (uncomment in redis.conf)
3. Use SSL/TLS for connections
4. Implement network isolation
5. Regular security updates for base images
6. Enable audit logging

## Additional Resources

- [TimescaleDB Documentation](https://docs.timescale.com/)
- [Redis Documentation](https://redis.io/documentation)
- [Docker Compose Reference](https://docs.docker.com/compose/)
- [PostgreSQL Connection Strings](https://www.postgresql.org/docs/current/libpq-connect.html#LIBPQ-CONNSTRING)