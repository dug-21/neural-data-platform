# syntax=docker/dockerfile:1.4
# Multi-stage build for Silver ETL
# Transforms Bronze (Parquet) to Silver (TimescaleDB)
# Uses BuildKit cache mounts for incremental compilation
#
# Build: docker build -t ndp/silver-etl -f apps/silver-etl/Dockerfile .
# Build (no cache): docker build --no-cache -t ndp/silver-etl -f apps/silver-etl/Dockerfile .

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
    cargo build --release -p silver-etl && \
    cp /app/target/release/silver-etl /usr/local/bin/ && \
    strip /usr/local/bin/silver-etl

# Stage 2: Runtime - minimal final image
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates libssl3 libstdc++6 && \
    rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash appuser && \
    mkdir -p /data && \
    chown -R appuser:appuser /data

# Copy binary
COPY --from=builder /usr/local/bin/silver-etl /usr/local/bin/
RUN chown appuser:appuser /usr/local/bin/silver-etl

USER appuser

ENV RUST_LOG=info \
    ETCD_ENDPOINT=http://etcd:2379 \
    BRONZE_DATA_DIR=/data/raw \
    TIMESCALE_URL=postgresql://postgres:ndp_secure_password@timescaledb:5432/ndp

WORKDIR /app
ENTRYPOINT ["silver-etl"]
CMD ["run"]
