# Neural Trader Production Docker Setup

## Prerequisites

1. Docker and Docker Compose installed
2. At least 8GB of available RAM
3. API keys for data providers (see `.env.example`)

## Quick Start

1. **Create environment file**:
   ```bash
   cp .env.example .env
   ```

2. **Edit the `.env` file** with your configuration:
   - Set secure passwords for `POSTGRES_PASSWORD` and `GRAFANA_ADMIN_PASSWORD`
   - Add your API keys for data providers (at least one is required)
   - Adjust trading symbols and other settings as needed

3. **Build and start the services**:
   ```bash
   docker-compose -f docker-compose.prod.yml up -d
   ```

4. **Check service status**:
   ```bash
   docker-compose -f docker-compose.prod.yml ps
   ```

## Services

- **TimescaleDB**: Time-series database (port 5433)
- **Redis**: Cache and pub/sub (internal only)
- **Neural Trader**: Main trading application (port 8080)
- **Data Ingestion**: Market data collection (port 8002)
- **Prometheus**: Metrics collection (port 9093)
- **Grafana**: Visualization dashboard (port 3000)

## Accessing Services

All services are bound to localhost only for security:

- Neural Trader API: http://localhost:8080
- Data Ingestion API: http://localhost:8002
- Grafana Dashboard: http://localhost:3000 (admin/your_password)
- Prometheus: http://localhost:9093

## Monitoring

1. Access Grafana at http://localhost:3000
2. Login with username `admin` and the password from your `.env` file
3. Import the pre-configured dashboards from `/configs/grafana/dashboards/`

## Troubleshooting

### Missing Environment Variables
If you see warnings about missing environment variables, ensure your `.env` file exists and contains all required variables from `.env.example`.

### Port Conflicts
The production setup uses different ports to avoid conflicts:
- TimescaleDB: 5433 (instead of 5432)
- Prometheus: 9093 (instead of 9090)

### Memory Issues
If services are being killed, increase Docker's memory allocation or adjust the memory limits in `docker-compose.prod.yml`.

## Data Persistence

All data is stored in named Docker volumes:
- `timescaledb_data`: Database files
- `redis_data`: Redis persistence
- `prometheus_data`: Metrics history
- `grafana_data`: Dashboards and settings
- `neural_trader_data`: Application data
- `neural_trader_logs`: Application logs

## Backup

To backup your data:
```bash
# Backup TimescaleDB
docker-compose -f docker-compose.prod.yml exec timescaledb pg_dump -U $POSTGRES_USER $POSTGRES_DB > backup.sql

# Backup volumes
docker run --rm -v neural-trader_timescaledb_data:/data -v $(pwd):/backup alpine tar czf /backup/timescaledb_backup.tar.gz /data
```

## Security Notes

1. All services are bound to `127.0.0.1` to prevent external access
2. Use strong passwords in production
3. Keep your API keys secure and never commit them to version control
4. Consider using Docker secrets for sensitive data in production
5. Enable TLS/SSL for any external-facing services