# syntax=docker/dockerfile:1.4
# Multi-stage Dockerfile for ndp-intelligence
# Builds the intelligence daemon binary for the Neural Data Platform
#
# Build: docker build -f docker/intelligence/Dockerfile -t ndp/intelligence .
# Build (no cache): docker build --no-cache -f docker/intelligence/Dockerfile -t ndp/intelligence .

# Stage 1: Builder - compile with cached dependencies
FROM rust:1.82-bookworm AS builder
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
COPY tools ./tools

# Build with cache mounts for incremental compilation
RUN --mount=type=cache,target=/app/target,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    cargo build --release -p ndp-intelligence-app && \
    cp /app/target/release/ndp-intelligence /usr/local/bin/ && \
    strip /usr/local/bin/ndp-intelligence

# Stage 2: Runtime - minimal final image
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash appuser

# Copy binary
COPY --from=builder /usr/local/bin/ndp-intelligence /usr/local/bin/

RUN chown appuser:appuser /usr/local/bin/ndp-intelligence

USER appuser

# Default environment variables
ENV RUST_LOG=info,ndp_intelligence=debug
ENV INTELLIGENCE_POLL_INTERVAL_SECS=1200
ENV INTELLIGENCE_POOL_SIZE=2
ENV INTELLIGENCE_WARMUP_THRESHOLD=168

# Default command: run daemon
ENTRYPOINT ["ndp-intelligence"]
CMD ["daemon"]
