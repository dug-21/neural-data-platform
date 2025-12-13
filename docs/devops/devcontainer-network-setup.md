# DevContainer Network Configuration

## Overview

The Neural Trader platform runs multiple services in Docker containers on a custom bridge network (`neural-trader_neural-net`). When developing inside a devcontainer, network isolation can prevent access to these services.

## Problem

- DevContainer runs on Docker's default bridge network (`172.17.0.0/16`)
- Neural Trader services run on custom network (`neural-trader_neural-net`, `172.21.0.0/16`)
- Services like Redis and TimescaleDB are unreachable from devcontainer

## Solution

Configure the devcontainer to join the Neural Trader network by updating `.devcontainer/devcontainer.json`:

```json
{
  "runArgs": [
    "--privileged",
    "--network=neural-trader_neural-net"
  ],
  "containerEnv": {
    "REDIS_HOST": "redis",
    "REDIS_URL": "redis://redis:6379",
    "POSTGRES_HOST": "timescaledb",
    "DATABASE_URL": "postgresql://postgres:postgres@timescaledb:5432/neural_trader"
  }
}
```

## Service Discovery

Once on the same network, services are accessible via their container names:
- `redis` - Redis cache (port 6379)
- `timescaledb` - PostgreSQL/TimescaleDB (port 5432)
- `config-store` - Configuration service (port 50051)
- `data-ingestion` - Data ingestion service (port 8081)

## Testing Connectivity

Use the provided test script after rebuilding the devcontainer:

```bash
./scripts/v2/test-redis-connection.sh
```

This will verify:
1. Hostname resolution
2. Environment variable configuration
3. Rust application connectivity

## Fallback Mode

The Redis configuration store includes a fallback mode that allows development to continue even without Redis connectivity:

```rust
Warning: Redis connection failed, running in fallback mode
```

In fallback mode:
- All operations use in-memory cache only
- Data is not persisted to Redis
- Tests will still pass
- Development can continue

## Rebuilding DevContainer

After updating `devcontainer.json`:

1. **Command Palette**: `Dev Containers: Rebuild Container`
2. Or from terminal: `devcontainer rebuild`
3. Or manually: Exit and reopen the folder in container

## Environment Variables

The following environment variables are automatically set:

| Variable | Value | Description |
|----------|-------|-------------|
| `REDIS_URL` | `redis://redis:6379` | Redis connection string |
| `REDIS_HOST` | `redis` | Redis hostname |
| `DATABASE_URL` | `postgresql://...` | TimescaleDB connection |
| `POSTGRES_HOST` | `timescaledb` | PostgreSQL hostname |

## Troubleshooting

### Check Network Status
```bash
# List Docker networks
docker network ls | grep neural

# Check container networks
docker inspect <container> --format='{{.NetworkSettings.Networks}}'

# Check devcontainer IP
hostname -I
```

### Verify Service Availability
```bash
# Test Redis
redis-cli -h redis ping

# Test PostgreSQL
psql $DATABASE_URL -c "SELECT 1"

# Test gRPC service
grpcurl -plaintext config-store:50051 list
```

### Common Issues

1. **"Connection refused"**: Devcontainer on wrong network
2. **"Unknown host"**: DNS resolution issue, check network config
3. **"Timeout"**: Firewall or network policy blocking traffic

## Security Considerations

- The `--privileged` flag is required for certain operations
- Network isolation is reduced when joining the service network
- Use environment-specific credentials in production
- Never hardcode credentials in source code

## Benefits of This Approach

1. **Simple**: Single network configuration change
2. **Safe**: No complex network bridging or port forwarding
3. **Consistent**: Same hostname resolution as production
4. **Flexible**: Services can still run in fallback mode
5. **Transparent**: No code changes required

## Alternative Approaches (Not Recommended)

- **Port forwarding**: Complex, requires mapping all service ports
- **Host networking**: Security risk, not portable
- **DevPod**: Doesn't solve Docker network isolation
- **Multiple networks**: Complex routing and DNS issues