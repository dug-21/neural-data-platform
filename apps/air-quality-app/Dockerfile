# syntax=docker/dockerfile:1.4
# Alternate Dockerfile for air-quality-app (builds from apps/ context)
# Uses BuildKit cache mounts for incremental compilation
#
# Note: Primary Dockerfile is at repo root. Use this for app-specific builds.
# Build: docker build -t ndp/air-quality-app -f apps/air-quality-app/Dockerfile ../..

FROM rust:1-bookworm AS builder
WORKDIR /app
COPY . .

RUN --mount=type=cache,target=/app/target,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    cargo build --release --bin air-quality-server && \
    cp /app/target/release/air-quality-server /usr/local/bin/

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/bin/air-quality-server /usr/local/bin/
WORKDIR /app
HEALTHCHECK --interval=30s --timeout=10s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1
CMD ["air-quality-server"]
