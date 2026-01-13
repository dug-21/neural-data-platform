# syntax=docker/dockerfile:1.4
# Multi-stage Dockerfile for air-quality-app
# Uses BuildKit cache mounts for incremental compilation
#
# Build: docker build -t ndp/air-quality-app .
# Build (no cache): docker build --no-cache -t ndp/air-quality-app .

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

# Build with cache mounts for incremental compilation
# - /app/target: compiled artifacts persist across builds
# - /usr/local/cargo/registry: downloaded crates persist
RUN --mount=type=cache,target=/app/target,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    cargo build --release -p air-quality-app && \
    cp /app/target/release/air-quality-server /usr/local/bin/ && \
    strip /usr/local/bin/air-quality-server

# Stage 2: Runtime - minimal final image
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates curl libssl3 && \
    rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash appuser && \
    mkdir -p /data /config && \
    chown -R appuser:appuser /data /config

# Copy binary and configs
COPY --from=builder /usr/local/bin/air-quality-server /usr/local/bin/
COPY config/base/streams /config/streams

RUN chown appuser:appuser /usr/local/bin/air-quality-server && \
    chown -R appuser:appuser /config

USER appuser
EXPOSE 8080 9090

HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

ENV RUST_LOG=info \
    ETCD_ENDPOINT=http://etcd:2379 \
    STREAM_CONFIG_DIR=/config/streams

WORKDIR /app
ENTRYPOINT ["/usr/local/bin/air-quality-server"]
