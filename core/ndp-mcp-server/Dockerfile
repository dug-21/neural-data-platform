# syntax=docker/dockerfile:1.4
# Multi-stage Dockerfile for ndp-mcp-server (Bronze MCP Server)
# Uses BuildKit cache mounts for incremental compilation
#
# Memory target: <50MB runtime (64MB limit with buffer)
# ADR: product/features/dp-005/architecture/ADR-006-deployment-strategy.md
#
# Build: docker build -t ndp/mcp-server -f core/ndp-mcp-server/Dockerfile .

# Stage 1: Builder - compile with cached dependencies
FROM rust:1-bookworm AS builder
WORKDIR /app

# Install build dependencies
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev protobuf-compiler && \
    rm -rf /var/lib/apt/lists/*

# Copy source code
COPY Cargo.toml Cargo.lock ./
COPY core ./core
COPY apps ./apps
COPY domains ./domains
COPY config-client ./config-client
COPY config ./config
COPY crates ./crates

# Build with cache mounts for incremental compilation
RUN --mount=type=cache,target=/app/target,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    cargo build --release -p ndp-mcp-server && \
    cp /app/target/release/ndp-mcp-server /usr/local/bin/ && \
    strip /usr/local/bin/ndp-mcp-server

# Stage 2: Runtime - minimal final image
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates curl libssl3 && \
    rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -r -u 1000 -s /sbin/nologin ndp && \
    mkdir -p /data && \
    chown -R ndp:ndp /data

# Copy binary
COPY --from=builder /usr/local/bin/ndp-mcp-server /usr/local/bin/
RUN chown ndp:ndp /usr/local/bin/ndp-mcp-server

USER ndp
EXPOSE 9100

HEALTHCHECK --interval=30s --timeout=10s --start-period=15s --retries=3 \
    CMD curl -f http://localhost:9100/health || exit 1

ENV RUST_LOG=info \
    NDP_MCP_LISTEN=0.0.0.0:9100 \
    NDP_RAW_PATH=/data \
    NDP_ETCD_ENDPOINTS=http://etcd:2379

WORKDIR /app
ENTRYPOINT ["/usr/local/bin/ndp-mcp-server"]
