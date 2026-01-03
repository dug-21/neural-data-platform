# DP-005: Bronze MCP Server - Dependencies

**Document Type**: SPARC Specification
**Version**: 1.0.0
**Last Updated**: 2026-01-03
**Status**: Draft

---

## Overview

This document defines the external dependencies required for the Bronze MCP Server, including crate versions, runtime services, and infrastructure requirements.

---

## Rust Crate Dependencies

### Core Dependencies

| Crate | Version | Purpose | Notes |
|-------|---------|---------|-------|
| `axum` | 0.7 | HTTP server framework | Async, tower-based |
| `tokio` | 1.x | Async runtime | Full features required |
| `tower-http` | 0.5 | HTTP middleware | CORS, tracing |
| `serde` | 1.x | Serialization | derive feature |
| `serde_json` | 1.x | JSON handling | |
| `tracing` | 0.1 | Structured logging | |
| `tracing-subscriber` | 0.3 | Log formatting | env-filter feature |

### etcd Dependencies

| Crate | Version | Purpose | Notes |
|-------|---------|---------|-------|
| `etcd-client` | 0.14 | etcd v3 API client | gRPC-based |

**etcd-client Notes:**
- Requires tokio runtime
- Supports TLS (optional)
- Watch/lease capabilities not needed for read-only use

### Parquet/Arrow Dependencies

| Crate | Version | Purpose | Notes |
|-------|---------|---------|-------|
| `parquet` | 53 | Parquet file reading | |
| `arrow` | 53 | Arrow array types | Schema introspection |

**Version Alignment:**
- `parquet` and `arrow` must use matching versions
- NDP core uses arrow 53.x for consistency
- Do NOT mix arrow versions (causes compile errors)

### Utility Dependencies

| Crate | Version | Purpose | Notes |
|-------|---------|---------|-------|
| `chrono` | 0.4 | Date/time handling | serde feature |
| `thiserror` | 1.x | Error types | |
| `anyhow` | 1.x | Error propagation | Alternative to thiserror |
| `async-trait` | 0.1 | Async trait support | For BronzeStorage trait |

### Optional Dependencies (Future)

| Crate | Version | Purpose | When Needed |
|-------|---------|---------|-------------|
| `object_store` | 0.11 | S3/GCS access | Cloud deployment |
| `rustls` | 0.23 | TLS support | HTTPS transport |
| `jsonwebtoken` | 9.x | JWT validation | Authentication |
| `metrics` | 0.23 | Prometheus metrics | Observability |

---

## Cargo.toml Template

```toml
[package]
name = "ndp-mcp-server"
version = "0.1.0"
edition = "2021"
authors = ["NDP Team"]
description = "MCP server for Bronze layer data exploration"

[dependencies]
# HTTP Server
axum = "0.7"
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0.5", features = ["cors", "trace"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# etcd
etcd-client = "0.14"

# Parquet/Arrow - MUST match core crate versions
parquet = "53"
arrow = "53"

# Utilities
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1"
async-trait = "0.1"

# Observability
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

[dev-dependencies]
# Testing
tokio-test = "0.4"
tempfile = "3"

# Future: Cloud storage
# [features]
# s3 = ["object_store/aws"]
# gcs = ["object_store/gcp"]
```

---

## Runtime Service Dependencies

### etcd

| Property | Requirement |
|----------|-------------|
| Version | 3.5+ |
| Protocol | gRPC (HTTP/2) |
| Port | 2379 (client) |
| Endpoint | `http://localhost:2379` or `http://etcd:2379` |
| Auth | None (MVP) |

**etcd Availability:**
- Server fails fast if etcd unavailable at startup
- Individual requests timeout after 5 seconds
- No automatic reconnection (new connection per request)

**Configuration Keys Required:**
```
/streams/{stream_id}/stream_id
/streams/{stream_id}/description
/streams/{stream_id}/version
/streams/{stream_id}/enabled
/streams/{stream_id}/sources/*
/streams/{stream_id}/entity_schemas/*
```

### Filesystem

| Property | Requirement |
|----------|-------------|
| Path | `/data/raw` (configurable) |
| Access | Read-only |
| Format | Parquet files |
| Structure | Hive-style partitions |

**Filesystem Layout:**
```
/data/raw/
├── air-quality/
│   └── year=2026/
│       └── month=01/
│           └── day=03/
│               └── data.parquet
├── outdoor-weather/
│   └── year=2026/
│       └── month=01/
│           └── day=03/
│               └── data.parquet
└── ...
```

---

## Infrastructure Requirements

### Raspberry Pi 5 (Edge Deployment)

| Resource | Requirement | Notes |
|----------|-------------|-------|
| Architecture | aarch64 (ARM64) | Cross-compile target |
| RAM | 8GB total | <50MB for MCP server |
| Disk | Shared `/data` volume | Read access to raw/ |
| Network | Local network | Port 9100 exposed |
| OS | Debian/Ubuntu ARM64 | Docker compatible |

### Container Requirements

| Property | Value |
|----------|-------|
| Base Image | `rust:1.75-slim` (build) / `debian:bookworm-slim` (runtime) |
| User | Non-root recommended |
| Volumes | `/data:ro` |
| Network | Host or bridge |
| Memory Limit | 128MB recommended |
| CPU Limit | 0.5 cores sufficient |

### Docker Compose Integration

```yaml
services:
  ndp-mcp-server:
    image: ndp-mcp-server:latest
    container_name: ndp-mcp-server
    ports:
      - "9100:9100"
    environment:
      - NDP_MCP_LISTEN=0.0.0.0:9100
      - NDP_MCP_LOG_LEVEL=info
      - NDP_ETCD_ENDPOINTS=http://etcd:2379
      - NDP_RAW_PATH=/data/raw
    volumes:
      - ndp-data:/data:ro
    depends_on:
      - etcd
    networks:
      - ndp-network
    mem_limit: 128m
    restart: unless-stopped
```

---

## Environment Variables

### Required

| Variable | Description | Example |
|----------|-------------|---------|
| `NDP_ETCD_ENDPOINTS` | etcd cluster endpoints | `http://localhost:2379` |
| `NDP_RAW_PATH` | Bronze layer data path | `/data/raw` |

### Optional (with defaults)

| Variable | Default | Description |
|----------|---------|-------------|
| `NDP_MCP_LISTEN` | `0.0.0.0:9100` | Server listen address |
| `NDP_MCP_LOG_LEVEL` | `info` | Log level (trace, debug, info, warn, error) |
| `NDP_ETCD_TIMEOUT_MS` | `5000` | etcd operation timeout |
| `NDP_AUTH_ENABLED` | `false` | Enable authentication (future) |

### Future (Cloud Deployment)

| Variable | Description |
|----------|-------------|
| `NDP_RAW_PATH` | `s3://bucket/raw` for S3 storage |
| `NDP_AUTH_ISSUER` | OAuth issuer URL |
| `NDP_TLS_CERT` | TLS certificate path |
| `NDP_TLS_KEY` | TLS private key path |

---

## Build Dependencies

### Cross-Compilation (x86_64 to aarch64)

```bash
# Install target
rustup target add aarch64-unknown-linux-gnu

# Install linker (Ubuntu/Debian)
sudo apt install gcc-aarch64-linux-gnu

# Build
cargo build --release --target aarch64-unknown-linux-gnu
```

### Docker Build

```dockerfile
# Build stage
FROM rust:1.75-slim AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y protobuf-compiler libssl-dev pkg-config
COPY . .
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/ndp-mcp-server /usr/local/bin/
EXPOSE 9100
CMD ["ndp-mcp-server"]
```

### CI/CD Requirements

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | 1.75+ | Compilation |
| protoc | 3.x | etcd-client gRPC |
| Docker | 20.10+ | Container builds |
| Cross | 0.2+ | Cross-compilation (optional) |

---

## Security Dependencies

### MVP (No Authentication)

- No external auth dependencies
- Network-level security assumed
- Localhost or VPN access only

### Future Authentication

| Dependency | Purpose |
|------------|---------|
| OIDC/OAuth2 provider | Token issuance |
| `jsonwebtoken` crate | JWT validation |
| TLS certificates | HTTPS transport |

---

## Testing Dependencies

### Unit Tests

```toml
[dev-dependencies]
tokio-test = "0.4"
tempfile = "3"
assert_json_diff = "2"
```

### Integration Tests

| Dependency | Purpose |
|------------|---------|
| Docker Compose | etcd + test fixtures |
| Sample Parquet files | `tests/fixtures/*.parquet` |
| Mock etcd data | Pre-populated keys |

### Test Fixtures Required

```
tests/
├── fixtures/
│   ├── air-quality/
│   │   └── year=2026/month=01/day=01/data.parquet
│   ├── outdoor-weather/
│   │   └── year=2026/month=01/day=01/data.parquet
│   └── etcd-data/
│       └── streams.json
└── integration/
    ├── mcp_protocol_test.rs
    └── tool_tests.rs
```

---

## Version Compatibility Matrix

### Rust Ecosystem

| NDP Component | arrow | parquet | tokio |
|---------------|-------|---------|-------|
| neural-core | 53 | 53 | 1.x |
| ndp-mcp-server | 53 | 53 | 1.x |
| air-quality-app | 53 | 53 | 1.x |

**CRITICAL:** All crates must use the same arrow/parquet major version.

### External Services

| Service | Minimum | Tested | Notes |
|---------|---------|--------|-------|
| etcd | 3.5.0 | 3.5.12 | v3 API required |
| Docker | 20.10 | 24.x | BuildKit recommended |
| Parquet | 2.6 | 2.6 | Arrow 53 default |

---

## Dependency Update Policy

### Security Updates

- Apply security patches within 48 hours
- Use `cargo audit` in CI pipeline
- Subscribe to RustSec advisories

### Version Updates

- Minor versions: Update monthly
- Major versions: Evaluate breaking changes, test thoroughly
- Lock versions in Cargo.lock for reproducible builds

### Deprecation Handling

- Monitor crate deprecation notices
- Plan migration 3+ months before EOL
- Test with latest stable before updates

---

## References

- [Cargo.toml Documentation](https://doc.rust-lang.org/cargo/reference/manifest.html)
- [etcd-client Docs](https://docs.rs/etcd-client)
- [Arrow Rust Docs](https://docs.rs/arrow)
- [axum Docs](https://docs.rs/axum)

---

*This document is part of the SPARC Specification phase for DP-005.*
