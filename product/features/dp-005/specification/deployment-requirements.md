# DP-005: Bronze MCP Server - Deployment Requirements

**Document Type**: SPARC Specification
**Version**: 1.0.0
**Last Updated**: 2026-01-03
**Status**: Draft

---

## Overview

This document specifies the deployment requirements for the Bronze MCP Server on Raspberry Pi 5 infrastructure. The server integrates with the existing Docker Compose stack defined in `deploy/pi/docker-compose.yml`.

---

## Deployment Architecture

```
                    ┌─────────────────────────────────────────────┐
                    │           Pi 5 Docker Host                   │
                    │                                              │
  ┌─────────────────┼──────────────────────────────────────────────┤
  │ neural-network  │                                              │
  │ (bridge)        │  ┌──────────────┐    ┌──────────────────┐   │
  │                 │  │   etcd       │    │  air-quality-app │   │
  │                 │  │   :2379      │    │      :8080       │   │
  │                 │  └──────┬───────┘    └──────────────────┘   │
  │                 │         │                                    │
  │                 │         │ config read                        │
  │                 │         ▼                                    │
  │                 │  ┌──────────────────────────────────────┐   │
  │                 │  │      ndp-mcp-server                  │   │
  │                 │  │          :9100                       │   │
  │                 │  │                                      │   │
  │                 │  │  Volume: air-quality-data:/data:ro   │   │
  │                 │  └──────────────────────────────────────┘   │
  │                 │                     │                        │
  └─────────────────┼─────────────────────┼────────────────────────┤
                    │                     │                        │
                    │              Port 9100:9100                  │
                    └─────────────────────┼────────────────────────┘
                                          │
                                          ▼
                              ┌───────────────────────┐
                              │   Claude Code (Mac)   │
                              │   MCP HTTP Client     │
                              └───────────────────────┘
```

---

## Container Requirements

### FR-DEP-001: Base Image

| ID | Requirement | Priority | Specification |
|----|-------------|----------|---------------|
| FR-DEP-001.1 | Container MUST use a minimal Rust-compatible base image | Must Have | `rust:slim-bookworm` or equivalent |
| FR-DEP-001.2 | Container MUST support ARM64 architecture | Must Have | aarch64 target for Raspberry Pi 5 |
| FR-DEP-001.3 | Container SHOULD use multi-stage build for minimal size | Should Have | Build stage + runtime stage |
| FR-DEP-001.4 | Runtime image SHOULD be based on `debian:bookworm-slim` | Should Have | Minimal dependencies, glibc compatibility |
| FR-DEP-001.5 | Container image MUST be less than 100MB compressed | Should Have | Network efficiency for Pi deployment |

**Dockerfile Location**: `/workspaces/neural-data-platform/apps/ndp-mcp-server/Dockerfile`

**Build Context**: Repository root (`/workspaces/neural-data-platform`)

**Reference Dockerfile Structure**:
```dockerfile
# Build stage
FROM rust:1.75-slim-bookworm AS builder
WORKDIR /build
COPY . .
RUN cargo build --release --bin ndp-mcp-server

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/ndp-mcp-server /usr/local/bin/
ENTRYPOINT ["/usr/local/bin/ndp-mcp-server"]
```

### FR-DEP-002: Resource Limits

| ID | Requirement | Priority | Specification |
|----|-------------|----------|---------------|
| FR-DEP-002.1 | Container MUST have memory limit of 64MB | Must Have | `deploy.resources.limits.memory: 64M` |
| FR-DEP-002.2 | Container SHOULD have memory reservation of 32MB | Should Have | `deploy.resources.reservations.memory: 32M` |
| FR-DEP-002.3 | Container MUST NOT specify CPU limits | Must Have | Allow burst for request handling |
| FR-DEP-002.4 | Container MUST restart unless stopped | Must Have | `restart: unless-stopped` |

**Rationale**: The 64MB limit accommodates:
- Rust runtime: ~5MB
- etcd client: ~10MB
- Parquet reader (buffered): ~30MB
- HTTP server overhead: ~10MB
- Safety margin: ~9MB

---

## Volume Requirements

### FR-DEP-003: Data Volumes

| ID | Requirement | Priority | Specification |
|----|-------------|----------|---------------|
| FR-DEP-003.1 | Container MUST mount air-quality-data volume | Must Have | Shared with air-quality-app |
| FR-DEP-003.2 | Data volume MUST be mounted read-only | Must Have | `:ro` flag prevents accidental writes |
| FR-DEP-003.3 | Data volume MUST be mounted at `/data` | Must Have | Matches NDP_RAW_PATH default |
| FR-DEP-003.4 | Container MUST NOT require persistent storage | Must Have | Stateless server design |
| FR-DEP-003.5 | Container MUST NOT create additional volumes | Must Have | Minimize resource usage |

**Volume Mapping**:
```yaml
volumes:
  - air-quality-data:/data:ro    # Bronze layer Parquet files
```

**Directory Structure Inside Container**:
```
/data/
├── raw/
│   ├── air-quality/
│   │   └── year=2026/month=01/day=03/data.parquet
│   ├── outdoor-weather/
│   │   └── year=2026/month=01/day=03/data.parquet
│   └── ...
└── (other directories managed by air-quality-app)
```

---

## Network Requirements

### FR-DEP-004: Port Mapping

| ID | Requirement | Priority | Specification |
|----|-------------|----------|---------------|
| FR-DEP-004.1 | Container MUST expose port 9100 | Must Have | MCP HTTP endpoint |
| FR-DEP-004.2 | Port 9100 MUST be mapped to host port 9100 | Must Have | `9100:9100` |
| FR-DEP-004.3 | Container MUST NOT expose additional ports | Must Have | Single endpoint design |
| FR-DEP-004.4 | Port binding MUST allow external access | Must Have | Not localhost-only |

### FR-DEP-005: Network Configuration

| ID | Requirement | Priority | Specification |
|----|-------------|----------|---------------|
| FR-DEP-005.1 | Container MUST join neural-network bridge | Must Have | Default network in docker-compose |
| FR-DEP-005.2 | Container MUST resolve `etcd` by service name | Must Have | Docker DNS resolution |
| FR-DEP-005.3 | Container MUST NOT require external DNS | Must Have | All dependencies internal |
| FR-DEP-005.4 | Container SHOULD support IPv4 only | Should Have | Simplify network config |

**Network Definition**:
```yaml
networks:
  default:
    name: neural-network
    driver: bridge
```

---

## Service Dependencies

### FR-DEP-006: Required Dependencies

| ID | Requirement | Priority | Specification |
|----|-------------|----------|---------------|
| FR-DEP-006.1 | Container MUST depend on etcd being healthy | Must Have | `condition: service_healthy` |
| FR-DEP-006.2 | Container MUST NOT depend on other services | Must Have | Minimal dependency chain |
| FR-DEP-006.3 | Container MUST start after etcd health check passes | Must Have | Ordered startup |
| FR-DEP-006.4 | Container MUST NOT require air-quality-app to be running | Must Have | Independent operation |

**Dependency Specification**:
```yaml
depends_on:
  etcd:
    condition: service_healthy
```

**Rationale**:
- etcd is required for stream configuration - fail fast if unavailable
- air-quality-app populates data but MCP server can start without it (returns empty results)
- mosquitto and timescaledb are not direct dependencies

---

## Environment Variables

### FR-DEP-007: Configuration Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `NDP_RAW_PATH` | No | `/data/raw` | Bronze data directory path |
| `NDP_ETCD_ENDPOINTS` | No | `http://etcd:2379` | etcd cluster endpoints (comma-separated) |
| `NDP_ETCD_PREFIX` | No | `/streams` | etcd key prefix for stream configs |
| `NDP_MCP_LISTEN` | No | `0.0.0.0:9100` | HTTP listen address and port |
| `RUST_LOG` | No | `info` | Log level (error, warn, info, debug, trace) |

| ID | Requirement | Priority | Specification |
|----|-------------|----------|---------------|
| FR-DEP-007.1 | All config MUST have sensible defaults | Must Have | Zero-config startup possible |
| FR-DEP-007.2 | etcd endpoint MUST use Docker service name | Must Have | Not hardcoded IP |
| FR-DEP-007.3 | Environment variables MUST match SCOPE.md specification | Must Have | Consistent naming |
| FR-DEP-007.4 | No secrets required for MVP | Must Have | Auth disabled |

**Docker Compose Environment Block**:
```yaml
environment:
  - RUST_LOG=info
  - NDP_RAW_PATH=/data/raw
  - NDP_ETCD_ENDPOINTS=http://etcd:2379
  - NDP_MCP_LISTEN=0.0.0.0:9100
```

---

## Health Check Configuration

### FR-DEP-008: Health Check Specification

| ID | Requirement | Priority | Specification |
|----|-------------|----------|---------------|
| FR-DEP-008.1 | Container MUST define Docker health check | Must Have | Enables orchestration |
| FR-DEP-008.2 | Health check MUST use GET /health endpoint | Must Have | Matches FR-001.4 |
| FR-DEP-008.3 | Health check interval MUST be 30 seconds | Must Have | Balance responsiveness vs overhead |
| FR-DEP-008.4 | Health check timeout MUST be 10 seconds | Must Have | Allow slow responses |
| FR-DEP-008.5 | Health check MUST retry 3 times before unhealthy | Must Have | Avoid flapping |
| FR-DEP-008.6 | Health check start period MUST be 30 seconds | Must Have | Allow startup time |
| FR-DEP-008.7 | Health check MUST use curl | Must Have | Available in slim images |

**Health Check Specification**:
```yaml
healthcheck:
  test: ["CMD", "curl", "-f", "http://localhost:9100/health"]
  interval: 30s
  timeout: 10s
  retries: 3
  start_period: 30s
```

**Expected Health Response**:
```json
{
  "status": "ok",
  "version": "0.1.0",
  "uptime_seconds": 3600,
  "etcd_connected": true,
  "data_path": "/data/raw"
}
```

---

## deploy.sh Integration

### FR-DEP-009: Script Integration

| ID | Requirement | Priority | Specification |
|----|-------------|----------|---------------|
| FR-DEP-009.1 | status() function MUST include MCP server health | Must Have | Add to health check output |
| FR-DEP-009.2 | MCP server MUST NOT require init_* functions | Must Have | Stateless, no initialization |
| FR-DEP-009.3 | Logs MUST be accessible via standard docker logs | Must Have | `docker logs ndp-mcp-server` |
| FR-DEP-009.4 | Useful URLs MUST include MCP endpoint | Must Have | Add to status output |

**status() Function Addition**:
```bash
# In status() function, add after existing health checks:
echo "  MCP Server: $(curl -s http://localhost:9100/health 2>/dev/null | jq -r '.status // "Not running"' || echo 'Not running')"
```

**Useful URLs Addition**:
```bash
# In status() function, add to Useful URLs section:
echo "  MCP Server:      http://${PI_IP}:9100"
```

### FR-DEP-010: No Special Initialization Required

| ID | Requirement | Priority | Specification |
|----|-------------|----------|---------------|
| FR-DEP-010.1 | Server MUST NOT require sync_config() call | Must Have | Reads etcd directly |
| FR-DEP-010.2 | Server MUST NOT require init_streams() call | Must Have | Discovers streams dynamically |
| FR-DEP-010.3 | Server MUST start cleanly with docker compose up | Must Have | Standard lifecycle |
| FR-DEP-010.4 | Server MUST handle missing data gracefully | Must Have | Return empty results, not errors |

---

## Deployment Verification

### FR-DEP-011: Verification Procedures

| ID | Requirement | Priority | Verification Command |
|----|-------------|----------|---------------------|
| FR-DEP-011.1 | Health endpoint MUST return HTTP 200 | Must Have | `curl -f http://localhost:9100/health` |
| FR-DEP-011.2 | Health response MUST contain status:ok | Must Have | Response body check |
| FR-DEP-011.3 | MCP tools/list MUST respond correctly | Must Have | JSON-RPC request |
| FR-DEP-011.4 | Container MUST show healthy status | Must Have | `docker ps` health column |
| FR-DEP-011.5 | Container memory MUST be under limit | Should Have | `docker stats` |

**Verification Script**:
```bash
#!/bin/bash
# verify-mcp-server.sh

echo "=== MCP Server Deployment Verification ==="

# Check 1: Container running
echo -n "Container running: "
if docker ps --format '{{.Names}}' | grep -q '^ndp-mcp-server$'; then
  echo "PASS"
else
  echo "FAIL"
  exit 1
fi

# Check 2: Health endpoint
echo -n "Health endpoint: "
HEALTH=$(curl -s http://localhost:9100/health)
if echo "$HEALTH" | jq -e '.status == "ok"' > /dev/null 2>&1; then
  echo "PASS"
else
  echo "FAIL - Response: $HEALTH"
  exit 1
fi

# Check 3: MCP tools/list
echo -n "MCP tools/list: "
TOOLS=$(curl -s -X POST http://localhost:9100/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}')
TOOL_COUNT=$(echo "$TOOLS" | jq '.result.tools | length' 2>/dev/null)
if [ "$TOOL_COUNT" = "4" ]; then
  echo "PASS (4 tools)"
else
  echo "FAIL - Expected 4 tools, got: $TOOL_COUNT"
  exit 1
fi

# Check 4: Memory usage
echo -n "Memory usage: "
MEM=$(docker stats --no-stream --format '{{.MemUsage}}' ndp-mcp-server | cut -d'/' -f1)
echo "$MEM"

echo ""
echo "=== All checks passed ==="
```

---

## Complete Docker Compose Service Definition

```yaml
# Add to deploy/pi/docker-compose.yml

  # Bronze MCP Server - Data exploration and validation (DP-005)
  ndp-mcp-server:
    build:
      context: ../..
      dockerfile: apps/ndp-mcp-server/Dockerfile
    image: neural-data-platform/ndp-mcp-server:latest
    container_name: ndp-mcp-server
    ports:
      - "9100:9100"    # MCP HTTP endpoint
    volumes:
      - air-quality-data:/data:ro    # Read-only access to Bronze layer
    environment:
      - RUST_LOG=info
      - NDP_RAW_PATH=/data/raw
      - NDP_ETCD_ENDPOINTS=http://etcd:2379
      - NDP_MCP_LISTEN=0.0.0.0:9100
    depends_on:
      etcd:
        condition: service_healthy
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:9100/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 30s
    deploy:
      resources:
        limits:
          memory: 64M
        reservations:
          memory: 32M
```

---

## Post-Deployment Configuration

### Client Configuration (Development Machine)

After deployment, configure Claude Code to connect:

**File**: `~/.claude/mcp.json` (or project `.claude/mcp.json`)

```json
{
  "mcpServers": {
    "ndp-bronze": {
      "type": "http",
      "url": "http://${NDP_PI_HOST}:9100/mcp",
      "description": "NDP Bronze layer data exploration and validation"
    }
  }
}
```

**Environment Variable**:
```bash
export NDP_PI_HOST=pi5.local  # or IP address
```

---

## Rollback Procedure

If deployment fails:

```bash
# Stop and remove MCP server only
docker stop ndp-mcp-server
docker rm ndp-mcp-server

# Other services remain unaffected
docker compose -f deploy/pi/docker-compose.yml ps

# View logs for debugging
docker logs ndp-mcp-server 2>&1 | tail -100
```

---

## References

- [DP-005 SCOPE](/workspaces/neural-data-platform/product/features/dp-005/SCOPE.md)
- [DP-005 Requirements](/workspaces/neural-data-platform/product/features/dp-005/specification/requirements.md)
- [Docker Compose Configuration](/workspaces/neural-data-platform/deploy/pi/docker-compose.yml)
- [deploy.sh Script](/workspaces/neural-data-platform/deploy/pi/deploy.sh)
- [Existing air-quality-app deployment patterns](/workspaces/neural-data-platform/Dockerfile)

---

*This document is part of the SPARC Specification phase for DP-005.*
