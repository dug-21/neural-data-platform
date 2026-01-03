# ADR-006: Deployment Strategy

## Status

Proposed

## Date

2026-01-03

## Context

The dp-005 Bronze MCP Server is a Rust-based service that needs to be deployed alongside existing NDP services on Raspberry Pi 5. The server exposes Bronze layer data and configuration validation tools to development agents via HTTP on port 9100.

### Current Service Landscape

The Pi deployment already includes these services with their resource allocations:

| Service | Container Name | Memory Limit | Port | Purpose |
|---------|---------------|--------------|------|---------|
| Mosquitto | mqtt-broker | 128MB | 1883, 9001 | MQTT broker |
| etcd | etcd | 256MB | 2379 | Configuration store |
| Air Quality App | air-quality-app | 512MB | 8080 | Bronze ingestion |
| TimescaleDB | pi5-timescaledb | 256MB | 5432 (local) | Silver storage |
| Grafana | grafana | 256MB | 3000 | Dashboards |
| **Total** | - | **1,408MB** | - | - |

### Requirements

| Requirement | Priority | Notes |
|-------------|----------|-------|
| Memory budget < 50MB | Must | Tight constraint for edge deployment |
| etcd dependency | Must | Config source for stream metadata |
| Bronze data access | Must | Read-only access to `/data/raw` |
| Health monitoring | Must | Integration with deploy.sh status |
| Graceful lifecycle | Should | Start after dependencies, clean shutdown |
| Consistent patterns | Should | Follow existing docker-compose conventions |

### Constraint: Resource Budget

The Raspberry Pi 5 has approximately 8GB RAM, but with services and OS overhead, available memory for new services is limited. The MCP server must operate efficiently:

- Target: < 50MB memory overhead
- Buffer: 64MB limit (allows for spikes during Parquet reads)
- CPU: Shared with other services (low priority, on-demand)

## Decision

**Deploy ndp-mcp-server as a Docker container in the existing docker-compose.yml stack with 64MB memory limit and read-only Bronze volume access.**

### Service Definition

```yaml
# Bronze MCP Server - Agent data exploration (DP-005)
ndp-mcp-server:
  build:
    context: ../..
    dockerfile: Dockerfile.mcp
  image: neural-data-platform/ndp-mcp-server:latest
  container_name: ndp-mcp-server
  ports:
    - "9100:9100"     # MCP HTTP endpoint
  volumes:
    - air-quality-data:/data:ro   # Read-only Bronze access
  environment:
    # Server configuration
    - RUST_LOG=info
    - NDP_MCP_LISTEN=0.0.0.0:9100
    # etcd configuration
    - NDP_ETCD_ENDPOINTS=http://etcd:2379
    - NDP_ETCD_PREFIX=/streams
    # Storage configuration
    - NDP_RAW_PATH=/data/raw
  depends_on:
    etcd:
      condition: service_healthy
  restart: unless-stopped
  healthcheck:
    test: ["CMD", "curl", "-f", "http://localhost:9100/health"]
    interval: 30s
    timeout: 10s
    retries: 3
    start_period: 15s
  deploy:
    resources:
      limits:
        memory: 64M
  networks:
    - default
```

### Key Design Choices

#### 1. Memory Limit: 64MB

```
Target: <50MB runtime
Limit:   64MB (provides 28% buffer)
```

Justification:
- Rust runtime is memory-efficient
- Parquet reads are streamed, not fully loaded
- Small JSON responses (tool outputs)
- 64MB allows for occasional peaks without OOM kills
- Can be tuned down after production profiling

#### 2. Port Selection: 9100

Port 9100 was selected (per SCOPE.md) because:
- Not in use by existing services (1883, 2379, 3000, 5432, 8080)
- Above 1024 (no root required)
- Standard metrics port range (Prometheus-adjacent, easy to remember)
- Consistent with "9xxx = internal services" convention

#### 3. Volume Configuration

```yaml
volumes:
  - air-quality-data:/data:ro   # Read-only access
```

- Uses existing `air-quality-data` volume (where Bronze layer writes)
- `:ro` flag ensures MCP server cannot accidentally modify data
- Path inside container: `/data/raw/{stream_id}/...`
- Same volume Grafana uses for DuckDB Parquet queries

#### 4. Service Dependencies

```yaml
depends_on:
  etcd:
    condition: service_healthy
```

Only etcd is a hard dependency:
- MCP server reads config from etcd on startup
- Bronze data access is filesystem-based (no service dependency)
- If etcd is down, server should fail fast (not serve stale config)

Not dependent on:
- `air-quality-app`: Can read existing Parquet files even if ingestion stopped
- `mosquitto`: No MQTT interaction
- `timescaledb`: No Silver layer access in MVP
- `grafana`: Independent visualization layer

#### 5. Health Check Configuration

```yaml
healthcheck:
  test: ["CMD", "curl", "-f", "http://localhost:9100/health"]
  interval: 30s
  timeout: 10s
  retries: 3
  start_period: 15s
```

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| interval | 30s | Matches other services, avoids overhead |
| timeout | 10s | Generous for slow starts (Parquet scan) |
| retries | 3 | Allows brief transient failures |
| start_period | 15s | Time for initial etcd connection + data discovery |

The health endpoint (`GET /health`) returns:
- 200 OK with JSON status when healthy
- 503 if etcd connection lost
- 503 if Bronze data path inaccessible

#### 6. Environment Variables

| Variable | Value | Purpose |
|----------|-------|---------|
| `RUST_LOG` | `info` | Logging level (debug for troubleshooting) |
| `NDP_MCP_LISTEN` | `0.0.0.0:9100` | Bind address (all interfaces) |
| `NDP_ETCD_ENDPOINTS` | `http://etcd:2379` | etcd service URL (Docker DNS) |
| `NDP_ETCD_PREFIX` | `/streams` | etcd key prefix for stream configs |
| `NDP_RAW_PATH` | `/data/raw` | Bronze layer root inside container |

### Network Configuration

```yaml
networks:
  - default
```

Uses the existing `neural-network` bridge network defined in docker-compose.yml:
- Service discovery via container names (`http://etcd:2379`)
- Isolated from host network (except exposed ports)
- Consistent with other services

### Deployment Sequence Integration

#### deploy.sh Integration

Add to the `status()` function in `deploy.sh`:

```bash
echo "  MCP Server: $(curl -s http://localhost:9100/health 2>/dev/null || echo 'Not running')"
```

The MCP server starts automatically with `docker compose up -d` (no special handling needed).

#### Startup Order

Docker Compose handles dependency ordering:

```
1. mosquitto (no deps)     ─┐
2. etcd (no deps)          ─┼─► parallel
3. timescaledb (no deps)   ─┘
4. air-quality-app (waits for mosquitto + etcd)
5. ndp-mcp-server (waits for etcd)
6. grafana (waits for timescaledb)
```

MCP server can start as soon as etcd is healthy - no need to wait for air-quality-app or existing data.

#### Graceful Shutdown

The Rust server implements graceful shutdown:
- Catches SIGTERM from Docker
- Completes in-flight requests (configurable timeout)
- Closes etcd connection
- Exits cleanly

Docker Compose default stop timeout (10s) is sufficient.

### Dockerfile Strategy

Create a minimal Dockerfile for the MCP server:

```dockerfile
# Dockerfile.mcp - Bronze MCP Server
FROM rust:1.75-slim-bookworm AS builder

WORKDIR /app
COPY core/ndp-mcp-server ./ndp-mcp-server
COPY core/Cargo.toml core/Cargo.lock ./

WORKDIR /app/ndp-mcp-server
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/ndp-mcp-server/target/release/ndp-mcp-server /usr/local/bin/

EXPOSE 9100
CMD ["ndp-mcp-server"]
```

Key choices:
- Multi-stage build (small final image)
- Debian slim base (minimal footprint)
- curl included (for health check)
- Single binary deployment

### Updated Resource Totals

| Service | Memory Limit |
|---------|--------------|
| Mosquitto | 128MB |
| etcd | 256MB |
| Air Quality App | 512MB |
| TimescaleDB | 256MB |
| Grafana | 256MB |
| **ndp-mcp-server** | **64MB** |
| **New Total** | **1,472MB** |

Increase: +64MB (4.5% of previous total)

## Consequences

### Positive

1. **Minimal resource impact**: 64MB limit is < 5% of existing allocation
2. **Consistent patterns**: Follows established docker-compose conventions
3. **Read-only safety**: Cannot corrupt Bronze data
4. **Simple dependency**: Only requires etcd (already present)
5. **Zero-config networking**: Docker DNS handles service discovery
6. **Integrated health monitoring**: Works with existing deploy.sh status
7. **Fast startup**: No database migrations or heavy initialization
8. **Graceful integration**: No changes to existing services

### Negative

1. **Additional container overhead**: Slight increase in Docker daemon memory
   - Mitigation: Rust binary is small, minimal container layers

2. **Port exposure**: Another port (9100) accessible from network
   - Mitigation: Bind to Pi IP only in production, not 0.0.0.0
   - Future: Add authentication layer

3. **Build time increase**: Additional Rust compilation during deploy
   - Mitigation: Cached layers reduce incremental builds
   - Mitigation: Consider pre-built images for Pi

4. **Volume sharing**: Uses same volume as air-quality-app
   - Mitigation: Read-only mount prevents conflicts
   - Mitigation: Parquet files are append-only, concurrent reads safe

### Deployment Checklist

- [ ] Create `Dockerfile.mcp` in repository root
- [ ] Add service definition to `docker-compose.yml`
- [ ] Update `deploy.sh` status function
- [ ] Test memory consumption under load
- [ ] Verify health check works end-to-end
- [ ] Document in `deploy/pi/README.md`

## Alternatives Considered

### Alternative 1: Embedded in air-quality-app

**How it works**: Add MCP endpoints to the existing air-quality-app service.

```rust
// In air-quality-app
app.route("/mcp", post(mcp_handler))
```

**Rejected because**:
- Violates single-responsibility principle
- Increases air-quality-app memory footprint
- Harder to scale or redeploy independently
- MCP is agent-facing, ingestion is sensor-facing (different lifecycles)

### Alternative 2: Systemd Service (no Docker)

**How it works**: Run as native systemd service outside Docker.

```ini
[Unit]
Description=NDP MCP Server
After=docker.service

[Service]
ExecStart=/usr/local/bin/ndp-mcp-server
Environment=NDP_ETCD_ENDPOINTS=http://localhost:2379
```

**Rejected because**:
- Inconsistent with existing Docker-based deployment
- Harder to manage (different lifecycle tools)
- No resource limits by default
- Complicates network configuration (not in Docker network)

### Alternative 3: Sidecar Container

**How it works**: Run as sidecar to air-quality-app in same pod.

**Rejected because**:
- Docker Compose doesn't have native pod concept
- Unnecessary coupling between unrelated services
- Complicates individual scaling/updates
- No benefit over separate container

### Alternative 4: Separate Volume

**How it works**: Create dedicated volume for MCP server.

```yaml
volumes:
  mcp-data:
    driver: local
```

**Rejected because**:
- Requires data duplication or complex volume sharing
- air-quality-app already writes to air-quality-data
- Read-only mount of existing volume is simpler
- No isolation benefit (both read same Parquet files)

## Implementation Notes

### Testing Deployment

```bash
# Build and start just the MCP server
docker compose build ndp-mcp-server
docker compose up -d ndp-mcp-server

# Verify health
curl http://localhost:9100/health

# Check memory usage
docker stats ndp-mcp-server --no-stream

# Test MCP endpoint
curl -X POST http://localhost:9100/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"tools/list","id":1}'
```

### Memory Profiling

After deployment, monitor actual memory usage:

```bash
# Peak memory during tool execution
docker exec ndp-mcp-server cat /sys/fs/cgroup/memory.current

# Over time
docker stats ndp-mcp-server --format "{{.MemUsage}}"
```

If actual usage is consistently <32MB, consider reducing limit to 48MB.

### Future Considerations

1. **TLS termination**: Add nginx/traefik reverse proxy for HTTPS
2. **Authentication**: Implement bearer token validation
3. **Rate limiting**: Add tower-http rate limiting middleware
4. **Metrics**: Expose Prometheus endpoint on separate port (9101)

## Related Decisions

- [ADR-001: MCP Transport](./ADR-001-mcp-transport.md) - HTTP transport justification
- [ADR-002: Storage Abstraction](./ADR-002-storage-abstraction.md) - BronzeStorage trait
- [ADR-003: Config Source](./ADR-003-config-source.md) - etcd integration
- [SCOPE.md](../SCOPE.md) - Feature requirements and port selection

## References

- [Docker Compose Specification](https://docs.docker.com/compose/compose-file/)
- [Docker Resource Constraints](https://docs.docker.com/config/containers/resource_constraints/)
- [Pi 5 Memory Management](https://www.raspberrypi.com/documentation/computers/raspberry-pi-5.html)
- [Existing deploy/pi/docker-compose.yml](../../../deploy/pi/docker-compose.yml)
