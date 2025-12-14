# Multi-stage Dockerfile for air-quality-app
# Supports: linux/amd64 (Mac Intel, cloud), linux/arm64 (Mac M-series, Pi 5)
# Target: <100MB compressed image size

# Stage 1: Chef - prepare build environment
FROM lukemathwalker/cargo-chef:latest-rust-1.75 AS chef
WORKDIR /app

# Stage 2: Planner - analyze dependencies and generate recipe
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Builder - compile with cached dependencies
FROM chef AS builder

# Install build dependencies
RUN apt-get update && \
    apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Build dependencies first (cached layer)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy source code and build application
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release -p air-quality-app && \
    strip /app/target/release/air-quality-app

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
    mkdir -p /data /models /config && \
    chown -R appuser:appuser /data /models /config

# Copy binary from builder
COPY --from=builder /app/target/release/air-quality-app /usr/local/bin/air-quality-app

# Copy default configuration
COPY config/base/air-quality.yaml /config/air-quality.yaml

# Set ownership
RUN chown appuser:appuser /usr/local/bin/air-quality-app

# Switch to non-root user
USER appuser

# Expose ports
EXPOSE 8080 9090

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:8080/health || exit 1

# Environment variables with defaults
ENV RUST_LOG=info \
    CONFIG_PATH=/config/air-quality.yaml \
    DATA_DIR=/data \
    MODELS_DIR=/models

# Set working directory
WORKDIR /app

# Entrypoint
ENTRYPOINT ["/usr/local/bin/air-quality-app"]

# Default command (can be overridden)
CMD ["--config", "/config/air-quality.yaml"]
