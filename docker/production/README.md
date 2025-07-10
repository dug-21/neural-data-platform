# Production Docker Setup

This directory contains the production-ready Docker configuration for the Neural Trader autonomous platform.

## Key Features

- **No filesystem dependencies**: All configurations are baked into images
- **Named volumes only**: No bind mounts for better portability
- **Security hardened**: Non-root users, resource limits, isolated networks
- **Self-contained images**: Each service has its configuration embedded

## Directory Structure

```
docker/production/
├── images/                    # Dockerfiles for each service
│   ├── neural-trader.dockerfile
│   ├── timescaledb.dockerfile
│   ├── prometheus.dockerfile
│   └── grafana.dockerfile
├── configs/                   # Configuration files (baked into images)
│   ├── timescaledb/
│   ├── prometheus/
│   └── grafana/
├── docker-compose.prod.yml    # Production compose file
├── .env.template             # Environment variable template
├── build.sh                  # Build script
└── README.md                 # This file
```

## Building Images

From within the devcontainer:

```bash
cd docker/production
./build.sh
```

This will build all images with the `prod` tag.

## Deployment

### Option 1: Direct Docker Compose

```bash
# Copy and configure environment
cp .env.template .env
# Edit .env with your values

# Start services
docker-compose -f docker-compose.prod.yml up -d

# Check status
docker-compose -f docker-compose.prod.yml ps

# View logs
docker-compose -f docker-compose.prod.yml logs -f neural-trader
```

### Option 2: Push to Registry

```bash
# Set your registry
export DOCKER_REGISTRY=your-registry.com

# Build and tag
./build.sh

# Push images
docker push $DOCKER_REGISTRY/neural-trader:prod
docker push $DOCKER_REGISTRY/neural-trader/timescaledb:prod
docker push $DOCKER_REGISTRY/neural-trader/prometheus:prod
docker push $DOCKER_REGISTRY/neural-trader/grafana:prod

# On production host
docker-compose -f docker-compose.prod.yml pull
docker-compose -f docker-compose.prod.yml up -d
```

## Volumes

The following persistent volumes are created:

- `timescaledb_data`: PostgreSQL/TimescaleDB data
- `redis_data`: Redis persistence
- `prometheus_data`: Metrics history
- `grafana_data`: Dashboards and settings
- `neural_trader_data`: Application data
- `neural_trader_logs`: Application logs

## Monitoring

- Prometheus: http://localhost:9090
- Grafana: http://localhost:3000 (admin/changeme)
- Neural Trader API: http://localhost:8080

## Security Notes

1. All services bind to `127.0.0.1` by default
2. Change all default passwords in `.env`
3. Services run as non-root users
4. Networks are isolated (internal network for data services)
5. Resource limits prevent runaway containers

## Backup

To backup data volumes:

```bash
# Backup TimescaleDB
docker run --rm -v timescaledb_data:/data -v $(pwd):/backup alpine tar czf /backup/timescaledb_backup.tar.gz -C /data .

# Backup all volumes
for vol in timescaledb_data redis_data prometheus_data grafana_data neural_trader_data; do
    docker run --rm -v ${vol}:/data -v $(pwd):/backup alpine tar czf /backup/${vol}_backup.tar.gz -C /data .
done
```

## Maintenance

### Update Images

```bash
# Rebuild with latest code
./build.sh

# Rolling update
docker-compose -f docker-compose.prod.yml up -d --no-deps neural-trader
```

### Database Migrations

```bash
# Run migrations
docker-compose -f docker-compose.prod.yml exec neural-trader neural-trader migrate
```

### View Logs

```bash
# All services
docker-compose -f docker-compose.prod.yml logs -f

# Specific service
docker-compose -f docker-compose.prod.yml logs -f neural-trader
```