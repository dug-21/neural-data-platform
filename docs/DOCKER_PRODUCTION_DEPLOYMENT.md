# Docker Production Deployment Architecture

## Overview

The Neural Trader platform deploys as a containerized microservices architecture with the following key characteristics:

- **Security-first design**: Non-root users, resource limits, isolated networks
- **No filesystem dependencies**: All configurations baked into images
- **Named volumes only**: No bind mounts for better portability
- **Self-contained images**: Each service has embedded configuration
- **Health monitoring**: Comprehensive health checks and metrics

## Container Architecture

### Core Services

#### 1. Neural Trader Application (neural-trader:prod)
- **Image**: Multi-stage Rust build (~151MB)
- **User**: Non-root `trader` user (UID 1000)
- **Ports**: 8080 (API), 9092 (Metrics)
- **Health Check**: `neural-trader health` command
- **Configuration**: Production config symlinked at `/config/platform.toml`
- **Models**: Real FANN neural networks with NHITS, TCN, DeepAR, MLP

#### 2. Data Ingestion Service (neural-trader/data-ingestion:prod)
- **Image**: Python 3.11-slim (~500MB)
- **User**: Non-root `ingester` user (UID 1000)
- **Ports**: 8001 (API), 9090 (Metrics - exposed as 9091)
- **Health Check**: HTTP GET to `/health` endpoint
- **Providers**: Yahoo Finance, Finnhub, Alpha Vantage, Alpaca, Polygon
- **Startup**: Environment-driven symbol and provider configuration

#### 3. TimescaleDB (neural-trader/timescaledb:prod)
- **Image**: TimescaleDB with PostgreSQL 16 (~1.27GB)
- **Schema**: Pre-initialized with market data, predictions, trading decisions
- **Features**: Hypertables, continuous aggregates, retention policies
- **Health Check**: `pg_isready` command
- **Performance**: Tuned for time-series data

#### 4. Prometheus (neural-trader/prometheus:prod)
- **Image**: Prometheus with embedded config (~370MB)
- **Scrape Targets**: Neural trader, data ingestion, exporters
- **Alert Rules**: Neural prediction alerts, system monitoring
- **Retention**: 30 days of metrics data

#### 5. Grafana (neural-trader/grafana:prod)
- **Image**: Grafana with pre-configured dashboards (~847MB)
- **Dashboards**: Neural trader overview, infrastructure monitoring
- **Data Sources**: Prometheus (pre-configured)
- **Authentication**: Admin user with configurable password

### Support Services

#### 6. PostgreSQL Exporter (postgres-exporter:9187)
- **Purpose**: Database metrics for Prometheus
- **Target**: TimescaleDB connection metrics

#### 7. Redis Exporter (redis-exporter:9121)
- **Purpose**: Cache metrics for Prometheus
- **Target**: Redis performance monitoring

#### 8. Node Exporter (node-exporter:9100)
- **Purpose**: System metrics (CPU, memory, disk)
- **Scope**: Host-level monitoring

#### 9. Nginx (Optional)
- **Purpose**: Reverse proxy and SSL termination
- **Configuration**: Production-ready with security headers

## Network Architecture

### Network Topology
```
neural-trader-frontend (external access)
├── nginx:80,443
└── grafana:3000

neural-trader-backend (internal services)
├── neural-trader:8080,9092
├── data-ingestion:8001,9090
├── timescaledb:5432
└── redis:6379

neural-trader-monitoring (metrics collection)
├── prometheus:9090
├── postgres-exporter:9187
├── redis-exporter:9121
└── node-exporter:9100
```

### Port Mapping (VSCode Dev Container Compatible)
| Service | Internal Port | External Port | Purpose |
|---------|---------------|---------------|---------|
| Neural Trader | 8080 | 8080 | Main API |
| Neural Trader | 9092 | 9092 | Metrics |
| Data Ingestion | 8001 | 8001 | API & Health |
| Data Ingestion | 9090 | 9091 | Metrics |
| TimescaleDB | 5432 | 5432 | Database |
| Redis | 6379 | 6379 | Cache |
| Prometheus | 9090 | 9090 | Monitoring |
| Grafana | 3000 | 3000 | Dashboards |

## Data Persistence

### Docker Volumes
All data persists in named Docker volumes:

#### Primary Data Volumes
- **`timescaledb_data`**: Market data, predictions, trading decisions
- **`redis_data`**: Cache data and session state
- **`neural_trader_data`**: Application state and configuration
- **`neural_trader_logs`**: Application logs

#### Monitoring Volumes  
- **`prometheus_data`**: Metrics history (30-day retention)
- **`grafana_data`**: Dashboards and user settings

#### Model Storage
- **`neural_trader_models`**: Neural network models
  - `/app/models/checkpoints/` - Training checkpoints
  - `/app/models/production/` - Production models
  - `/app/models/archive/` - Compressed archives
  - `/app/models/backups/` - Model backups

### Database Schema
TimescaleDB automatically initializes with:

#### Core Tables (Hypertables)
- **`market_data`**: OHLCV data with metadata
- **`predictions`**: Model predictions with confidence intervals
- **`trading_decisions`**: Autonomous trading decisions
- **`performance_metrics`**: System performance data

#### Advanced Features
- **Continuous Aggregates**: `market_data_1h` for OHLC rollups
- **Retention Policies**: 1 year raw data, 3 months predictions
- **Indexes**: Optimized for symbol and time-based queries

## Security Architecture

### Container Security
- **Non-root users**: All services run as dedicated users (UID 1000)
- **Resource limits**: CPU and memory constraints prevent abuse
- **Network isolation**: Internal networks prevent external access
- **Health checks**: Monitor service availability and security

### Secrets Management
Production deployment uses Docker secrets from files:
- `postgres_password.txt`
- `redis_password.txt`
- `jwt_secret.txt`
- `grafana_password.txt`
- API keys for data providers

### Network Security
- **Bind to localhost**: Services only accessible locally
- **Internal networks**: Database and cache on isolated networks
- **Reverse proxy**: Nginx for SSL termination and security headers

## Deployment Procedures

### 1. Build Images
```bash
cd docker/production
./build.sh
```

### 2. Configure Environment
Required environment variables (see `REQUIRED_ENV_VARS.md`):
```bash
export POSTGRES_USER=neural_trader
export POSTGRES_PASSWORD=secure_password
export GRAFANA_PASSWORD=grafana_password
export TRADING_SYMBOLS_PRIMARY=AAPL,MSFT,GOOGL,AMZN,NVDA
export PRIMARY_PROVIDER=alpaca
export ALPACA_API_KEY=your_key
export ALPACA_API_SECRET=your_secret
```

### 3. Deploy Services
```bash
# Option 1: Use deployment script
./deploy.sh

# Option 2: Direct compose
docker-compose -f docker-compose.prod.yml up -d
```

### 4. Verify Deployment
```bash
# Check service health
docker-compose -f docker-compose.prod.yml ps

# Verify Prometheus targets
curl -s http://localhost:9090/api/v1/targets | jq '.data.activeTargets[] | {job: .labels.job, health: .health}'

# Test Neural Trader API
curl http://localhost:8080/health

# Access Grafana
curl http://localhost:3000
```

## Operational Procedures

### Starting the Platform
```bash
cd docker/production
./deploy.sh
```

### Monitoring Container Health
```bash
# Overall status
docker-compose -f docker-compose.prod.yml ps

# Service logs
docker-compose -f docker-compose.prod.yml logs -f neural-trader

# Health endpoints
curl http://localhost:8080/health    # Neural trader
curl http://localhost:8001/health    # Data ingestion
```

### Log Management
```bash
# View all logs
docker-compose -f docker-compose.prod.yml logs

# Follow specific service
docker-compose -f docker-compose.prod.yml logs -f [service]

# Log rotation handled by Docker daemon
```

### Backup and Recovery

#### Create Backups
```bash
# Database backup
docker run --rm -v timescaledb_data:/data -v $(pwd):/backup alpine \
  tar czf /backup/timescaledb_$(date +%Y%m%d).tar.gz -C /data .

# All volumes backup
for vol in timescaledb_data redis_data prometheus_data grafana_data neural_trader_data; do
  docker run --rm -v ${vol}:/data -v $(pwd):/backup alpine \
    tar czf /backup/${vol}_$(date +%Y%m%d).tar.gz -C /data .
done
```

#### Restore from Backup
```bash
# Stop services
docker-compose -f docker-compose.prod.yml down

# Restore volume
docker run --rm -v timescaledb_data:/data -v $(pwd):/backup alpine \
  tar xzf /backup/timescaledb_20250803.tar.gz -C /data

# Restart services
docker-compose -f docker-compose.prod.yml up -d
```

## Troubleshooting Common Issues

### Port Conflicts
```bash
# Check port usage
sudo lsof -i :9090

# Change ports in docker-compose.prod.yml if needed
```

### Service Won't Start
```bash
# Check logs
docker-compose -f docker-compose.prod.yml logs [service]

# Check resource usage
docker stats

# Verify environment variables
docker-compose -f docker-compose.prod.yml config
```

### Database Connection Issues
```bash
# Test database connection
docker-compose -f docker-compose.prod.yml exec timescaledb \
  psql -U neural_trader -d neural_trader_db

# Check database logs
docker-compose -f docker-compose.prod.yml logs timescaledb
```

### Missing Metrics
```bash
# Check Prometheus targets
curl http://localhost:9090/targets

# Test metrics endpoints
curl http://localhost:9092/metrics  # Neural trader
curl http://localhost:9091/metrics  # Data ingestion
```

### Model Storage Issues
```bash
# Check model directory
docker-compose -f docker-compose.prod.yml exec neural-trader \
  ls -la /app/models/

# Verify initialization
docker-compose -f docker-compose.prod.yml logs neural-trader | grep "model"
```

## Performance Optimization

### Resource Allocation
Services are configured with resource limits:
- **Neural Trader**: 2 CPU, 4GB RAM (2 replicas possible)
- **TimescaleDB**: 4 CPU, 8GB RAM
- **Data Ingestion**: 2 CPU, 2GB RAM
- **Monitoring**: 1 CPU, 1GB RAM each

### Scaling Options
```bash
# Scale neural-trader instances
docker-compose -f docker-compose.prod.yml up -d --scale neural-trader=3

# Scale data ingestion (if needed)
docker-compose -f docker-compose.prod.yml up -d --scale data-ingestion=2
```

### Database Optimization
TimescaleDB is pre-tuned with:
- Optimized PostgreSQL settings
- Proper indexing strategies
- Continuous aggregates for common queries
- Retention policies for data management

## Maintenance Procedures

### Update Application
```bash
# Rebuild with new code
./build.sh

# Rolling update (zero downtime)
docker-compose -f docker-compose.prod.yml up -d --no-deps neural-trader
```

### Clean Up Resources
```bash
# Remove unused images
docker image prune -f

# Remove unused volumes (CAUTION: This removes data!)
docker volume prune -f

# Complete reset (DESTROYS ALL DATA)
docker-compose -f docker-compose.prod.yml down -v
```

### Health Monitoring
Regular checks should include:
- All services healthy: `docker-compose ps`
- Prometheus targets UP: `http://localhost:9090/targets`
- Grafana connectivity: `http://localhost:3000`
- Application logs: No critical errors
- Database connectivity: TimescaleDB accessible
- Model availability: Neural models loaded

## Integration with External Systems

### Registry Deployment
```bash
# Tag for registry
export REGISTRY=your-registry.com
docker tag neural-trader:prod $REGISTRY/neural-trader:prod

# Push images
docker push $REGISTRY/neural-trader:prod

# Deploy from registry
DOCKER_REGISTRY=$REGISTRY docker-compose -f docker-compose.prod.yml pull
DOCKER_REGISTRY=$REGISTRY docker-compose -f docker-compose.prod.yml up -d
```

### Load Balancer Integration
The nginx service provides:
- SSL termination
- Load balancing across neural-trader replicas
- Security headers
- Rate limiting (configurable)

### Monitoring Integration
Export metrics to external systems:
- Prometheus federation
- Grafana cloud connectivity
- Custom metric exporters

This architecture provides a robust, scalable, and secure foundation for autonomous neural trading operations.