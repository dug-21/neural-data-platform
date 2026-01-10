# Multi-stage build for Silver ETL
# Transforms Bronze (Parquet) to Silver (TimescaleDB)

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

# Build silver-etl application
RUN cargo build --release -p silver-etl -j 2 && \
    strip /app/target/release/silver-etl

# Stage 4: Runtime - minimal final image
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies only
RUN apt-get update && \
    apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create non-root user for security
RUN useradd -m -u 1000 -s /bin/bash appuser && \
    mkdir -p /data && \
    chown -R appuser:appuser /data

# Copy binary from builder
COPY --from=builder /app/target/release/silver-etl /usr/local/bin/silver-etl

# Set ownership
RUN chown appuser:appuser /usr/local/bin/silver-etl

# Switch to non-root user
USER appuser

# Environment variables with defaults
ENV RUST_LOG=info \
    ETCD_ENDPOINT=http://etcd:2379 \
    BRONZE_DATA_DIR=/data/raw \
    TIMESCALE_URL=postgresql://postgres:ndp_secure_password@timescaledb:5432/ndp

# Set working directory
WORKDIR /app

# Entrypoint
ENTRYPOINT ["silver-etl"]
CMD ["run"]
