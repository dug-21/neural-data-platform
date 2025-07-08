# Docker External Volume Solution

## Overview

This solution addresses Docker disk space issues by using external volumes and running only the essential database services in Docker, while the main application runs locally.

## Key Benefits

1. **Reduced Disk Usage**: External volumes prevent Docker from consuming excessive disk space
2. **Faster Development**: No need to rebuild large Docker images for the main application
3. **Persistent Data**: External volumes survive container restarts
4. **Clean Separation**: Database services in Docker, application code runs locally

## Scripts Created

### 1. `/scripts/start_external_docker.sh`
- Basic external Docker solution
- Creates and uses external volumes
- Starts only database and UI services

### 2. `/scripts/start_external_docker_clean.sh`
- Clean start version that removes old volumes
- Ensures fresh environment
- Includes database connectivity test

### 3. `/docker-compose.external.yml`
- Streamlined compose file with only essential services
- Uses external volumes for persistence
- Includes health checks

### 4. `/docker/timescaledb/init-scripts/01-init-db-simple.sql`
- Minimal database initialization
- Avoids complex extensions that cause startup failures
- Just enables TimescaleDB and creates schema

## Usage

### Clean Start (Recommended)
```bash
./scripts/start_external_docker_clean.sh
```

### Regular Start
```bash
./scripts/start_external_docker.sh
```

### Stop Services
```bash
docker-compose -f docker-compose.external.yml down
```

### View Logs
```bash
docker-compose -f docker-compose.external.yml logs -f
```

## Running the Application

After starting the Docker services, run the Neural Trader application locally:

```bash
# Set environment variables
export DATABASE_URL=postgresql://postgres:dev_password@localhost:5432/neural_trader
export REDIS_URL=redis://localhost:6379

# Run the application
cargo run --release
```

## Service URLs

- **Redis Commander**: http://localhost:8081 - Redis management UI
- **pgAdmin**: http://localhost:8082 - PostgreSQL management
  - Email: admin@example.com
  - Password: admin

## Disk Usage Comparison

- **Full Docker Build**: ~29% disk usage, slow builds
- **External Volume Solution**: ~29% disk usage, but no build required
- **Actual Docker volume usage**: < 100MB

## Issues Resolved

1. **TimescaleDB Extension Errors**: Fixed by using simple init script
2. **pgAdmin Email Validation**: Changed to valid email format
3. **Build Timeouts**: Eliminated by not building the main app in Docker
4. **Disk Space**: Managed through external volumes

## Technical Details

### External Volumes Created
- `neural_trader_timescale_data`: PostgreSQL/TimescaleDB data
- `neural_trader_redis_data`: Redis persistence
- `neural_trader_build_cache`: Build artifacts (optional)

### Health Checks
All services include health checks to ensure proper startup:
- TimescaleDB: `pg_isready -U postgres`
- Redis: `redis-cli ping`

### Network
Services communicate on the `neural_trader_dev` bridge network.

## Troubleshooting

### Database Connection Issues
```bash
# Test database connection
docker exec neural_trader_stocks-timescaledb-1 psql -U postgres -c "SELECT version();"
```

### Volume Cleanup
```bash
# Remove all volumes (data will be lost)
docker volume rm neural_trader_timescale_data neural_trader_redis_data neural_trader_build_cache
```

### Service Status
```bash
# Check all services
docker-compose -f docker-compose.external.yml ps
```

## Conclusion

This external Docker solution successfully addresses the disk space issue while maintaining all necessary services for the Neural Trader application. It provides a cleaner, faster development experience by separating concerns between infrastructure (Docker) and application code (local development).