# Multi-stage Dockerfile for ndp-mcp-server (Bronze MCP Server)
# Supports: linux/amd64 (Mac Intel, cloud), linux/arm64 (Mac M-series, Pi 5)
#
# Memory target: <50MB runtime (64MB limit with buffer)
# ADR: product/features/dp-005/architecture/ADR-006-deployment-strategy.md

# Stage 1: Chef - prepare build environment with cargo-chef
FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

# Stage 2: Planner - analyze dependencies and generate recipe
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./

# Copy all workspace members required for build
COPY core ./core
COPY apps ./apps
COPY domains ./domains
COPY config-client ./config-client

RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Builder - compile with cached dependencies
FROM chef AS builder

# Install build dependencies
RUN apt-get update && \
    apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Build dependencies first (cached layer)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy source code
COPY Cargo.toml Cargo.lock ./
COPY core ./core
COPY apps ./apps
COPY domains ./domains
COPY config-client ./config-client
COPY config ./config

# Build ndp-mcp-server binary with optimizations
# Uses -j 2 for memory-constrained Pi 5 builds
RUN cargo build --release -p ndp-mcp-server -j 2 && \
    strip /app/target/release/ndp-mcp-server

# Stage 4: Runtime - minimal final image
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies only
RUN apt-get update && \
    apt-get install -y \
    ca-certificates \
    curl \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create non-root user for security
RUN useradd -r -u 1000 -s /sbin/nologin ndp && \
    mkdir -p /data && \
    chown -R ndp:ndp /data

# Copy binary from builder
COPY --from=builder /app/target/release/ndp-mcp-server /usr/local/bin/ndp-mcp-server

# Set ownership
RUN chown ndp:ndp /usr/local/bin/ndp-mcp-server

# Switch to non-root user
USER ndp

# Expose MCP HTTP endpoint
EXPOSE 9100

# Health check - MCP server health endpoint
# 15s start_period allows for initial schema discovery
HEALTHCHECK --interval=30s --timeout=10s --start-period=15s --retries=3 \
    CMD curl -f http://localhost:9100/health || exit 1

# Environment variables with defaults
ENV RUST_LOG=info \
    NDP_MCP_LISTEN=0.0.0.0:9100 \
    NDP_RAW_PATH=/data \
    NDP_ETCD_ENDPOINTS=http://etcd:2379

# Working directory for any runtime files
WORKDIR /app

# Entrypoint
ENTRYPOINT ["/usr/local/bin/ndp-mcp-server"]
