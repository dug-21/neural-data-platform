# Multi-stage Dockerfile for air-quality-app
# Supports: linux/amd64 (Mac Intel, cloud), linux/arm64 (Mac M-series, Pi 5)

# Stage 1: Chef - prepare build environment
FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

# Stage 2: Planner - analyze dependencies and generate recipe
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./

# Copy all workspace members
COPY core ./core
COPY apps ./apps
COPY domains ./domains
COPY config-store ./config-store
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
COPY config-store ./config-store
COPY config-client ./config-client
COPY config ./config

# Build application
RUN cargo build --release -p air-quality-app -j 2 && \
    strip /app/target/release/air-quality-server

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
RUN useradd -m -u 1000 -s /bin/bash appuser && \
    mkdir -p /data /config && \
    chown -R appuser:appuser /data /config

# Copy binary from builder
COPY --from=builder /app/target/release/air-quality-server /usr/local/bin/air-quality-server

# Copy stream configs for GitOps sync (AIR-005)
COPY --from=builder /app/config/base/streams /config/streams

# Set ownership
RUN chown appuser:appuser /usr/local/bin/air-quality-server && \
    chown -R appuser:appuser /config

# Switch to non-root user
USER appuser

# Expose ports
EXPOSE 8080 9090

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
  CMD curl -f http://localhost:8080/health || exit 1

# Environment variables with defaults
ENV RUST_LOG=info \
    ETCD_ENDPOINT=http://etcd:2379 \
    STREAM_CONFIG_DIR=/config/streams

# Set working directory
WORKDIR /app

# Entrypoint
ENTRYPOINT ["/usr/local/bin/air-quality-server"]
