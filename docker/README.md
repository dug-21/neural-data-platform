# Neural Trader Docker Setup

This directory contains all Docker configurations for running the Neural Trader platform.

## Architecture Overview

The platform consists of the following services:

- **TimescaleDB**: Time-series database optimized for trading data
- **Redis**: In-memory data store for real-time market data and caching
- **Data Ingestion**: Python service for fetching and processing market data
- **Neural Trader**: Main Rust application for trading logic and neural networks
- **Prometheus**: Metrics collection and monitoring
- **Grafana**: Visualization and dashboards
- **Nginx**: Reverse proxy and load balancer (production only)

## Quick Start

1. **Clone the repository**:
   ```bash
   git clone https://github.com/your-org/neural-trader.git
   cd neural-trader
   ```

2. **Copy environment variables**:
   ```bash
   cp .env.example .env
   # Edit .env with your configuration
   ```

3. **Start the platform**:
   ```bash
   # Development
   docker-compose up -d

   # Production
   docker-compose -f docker-compose.yml -f docker-compose.prod.yml up -d
   ```

4. **Access the services**:
   - Neural Trader API: http://localhost:3030
   - Grafana: http://localhost:3000 (admin/neural_trader_admin)
   - Prometheus: http://localhost:9090
   - pgAdmin: http://localhost:8082 (development only)
   - Redis Commander: http://localhost:8081 (development only)

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

## Development Setup

For development with hot-reloading:

```bash
docker-compose -f docker-compose.yml -f docker-compose.dev.yml up
```

This enables:
- Source code mounting for hot-reloading
- Debug ports exposed
- Lower resource limits
- Development tools (pgAdmin, Redis Commander)

## Production Deployment

For production deployment:

```bash
# Set production environment variables
export GRAFANA_ADMIN_PASSWORD=secure_password
export POSTGRES_PASSWORD=secure_password
# ... other production configs

# Deploy with production settings
docker-compose -f docker-compose.yml -f docker-compose.prod.yml up -d
```

Production features:
- Optimized resource allocation
- Automated backups
- SSL/TLS termination with Nginx
- Log rotation
- Health checks and auto-restart
- Multiple replicas for high availability

## Service Configuration

### TimescaleDB
- Custom PostgreSQL configuration optimized for time-series data
- Automatic hypertable creation
- Compression policies for historical data
- Continuous aggregates for performance

### Redis
- Persistence enabled (RDB + AOF)
- Optimized for high-frequency updates
- Redis Streams for real-time data
- Memory limits and eviction policies

### Data Ingestion
- Multi-stage Docker build for efficiency
- Non-root user for security
- Health checks
- Prometheus metrics exposed

### Neural Trader
- Multi-stage Rust build
- Optimized release binary
- Health endpoints
- Configurable via environment variables

## Monitoring

The platform includes comprehensive monitoring:

1. **Metrics Collection**: Prometheus scrapes metrics from all services
2. **Visualization**: Grafana dashboards for real-time monitoring
3. **Alerts**: Configure alerts in Grafana for critical events

### Available Dashboards
- Neural Trader Overview
- Market Data Ingestion
- Trading Performance
- System Resources
- Database Performance

## Backup and Recovery

Automated backups run hourly in production:

```bash
# Manual backup
docker exec neural_trader_backup /backup.sh

# Restore from backup
docker exec -it neural_trader_timescaledb \
  pg_restore -U neural_trader -d neural_trader_db \
  /backups/postgres/neural_trader_20240101_120000.dump.gz
```

## Scaling

To scale services horizontally:

```bash
# Scale data ingestion service to 5 replicas
docker-compose -f docker-compose.yml -f docker-compose.prod.yml \
  up -d --scale data-ingestion=5

# Scale neural trader to 3 replicas
docker-compose -f docker-compose.yml -f docker-compose.prod.yml \
  up -d --scale neural-trader=3
```

## Maintenance

### Update services
```bash
# Pull latest images
docker-compose pull

# Recreate containers
docker-compose up -d --force-recreate
```

### Clean up
```bash
# Remove stopped containers
docker-compose rm -f

# Remove unused volumes
docker volume prune

# Remove unused images
docker image prune
```

## Support

For issues or questions:
1. Check the logs first
2. Verify environment variables
3. Ensure sufficient resources
4. Review health check status

## Additional Resources

- [TimescaleDB Documentation](https://docs.timescale.com/)
- [Redis Documentation](https://redis.io/documentation)
- [Docker Compose Reference](https://docs.docker.com/compose/)
- [PostgreSQL Connection Strings](https://www.postgresql.org/docs/current/libpq-connect.html#LIBPQ-CONNSTRING)

## License

See the main project LICENSE file.