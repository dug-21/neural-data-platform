# fe-005 Pseudocode: deploy

## Dockerfile: `docker/embedder/Dockerfile`

```dockerfile
# syntax=docker/dockerfile:1.4
# Multi-stage Dockerfile for ndp-embedder
# Follows docker/intelligence/Dockerfile pattern

# Stage 1: Builder
FROM rust:1-bookworm AS builder
WORKDIR /app

RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev protobuf-compiler && \
    rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY core ./core
COPY apps ./apps
COPY domains ./domains
COPY config-client ./config-client
COPY config ./config
COPY crates ./crates
COPY tools ./tools

RUN --mount=type=cache,target=/app/target,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    cargo build --release -p ndp-embedder && \
    cp /app/target/release/ndp-embedder /usr/local/bin/ && \
    strip /usr/local/bin/ndp-embedder

# Stage 2: Runtime
FROM debian:bookworm-slim AS runtime

RUN apt-get update && \
    apt-get install -y ca-certificates curl libssl3 && \
    rm -rf /var/lib/apt/lists/*

RUN useradd -m -u 1000 -s /bin/bash appuser

COPY --from=builder /usr/local/bin/ndp-embedder /usr/local/bin/
RUN chown appuser:appuser /usr/local/bin/ndp-embedder

# Create model directory (volume mount target)
RUN mkdir -p /models && chown appuser:appuser /models

USER appuser

ENV RUST_LOG=info,ndp_embedder=debug
ENV EMBEDDER_POLL_INTERVAL_SECS=1200
ENV EMBEDDER_POOL_SIZE=2
ENV MODEL_VOLUME_PATH=/models

ENTRYPOINT ["ndp-embedder"]
CMD ["daemon"]
```

## Compose Entry: `deploy/pi/docker-compose.yml`

```yaml
# Add to services section:
ndp-embedder:
  build:
    context: ../..
    dockerfile: docker/embedder/Dockerfile
  image: neural-data-platform/ndp-embedder:latest
  container_name: ndp-embedder
  environment:
    - RUST_LOG=info,ndp_embedder=debug
    - DATABASE_URL=postgresql://postgres:${POSTGRES_PASSWORD:-ndp_secure_password}@timescaledb:5432/ndp
    - EMBEDDER_DOMAIN=indoor-air-quality
    - ETCD_ENDPOINTS=http://etcd:2379
    - MODEL_VOLUME_PATH=/models
    - EMBEDDER_POLL_INTERVAL_SECS=1200
    - EMBEDDER_POOL_SIZE=2
  volumes:
    - embedder-models:/models
  depends_on:
    timescaledb:
      condition: service_healthy
    etcd:
      condition: service_healthy
  restart: unless-stopped
  healthcheck:
    test: ["CMD-SHELL", "pgrep -x ndp-embedder || exit 1"]
    interval: 60s
    timeout: 10s
    retries: 3
    start_period: 60s
  deploy:
    resources:
      limits:
        memory: 512M
  profiles:
    - intelligence

# Add to volumes section:
# embedder-models:
#   driver: local
```

## Init-Script: `deploy/pi/init-scripts/004-text-embeddings.sql`

```sql
-- fe-005: Text embedding storage
-- Runs at database bootstrap (ops-008 pattern)

-- Ensure pgvector extension exists (from fe-004)
CREATE EXTENSION IF NOT EXISTS vector;

-- Create text embeddings table
CREATE TABLE IF NOT EXISTS gold.text_embeddings (
    id              BIGSERIAL,
    bucket          TIMESTAMPTZ NOT NULL,
    domain_id       TEXT NOT NULL,
    source_stream   TEXT NOT NULL,
    source_column   TEXT NOT NULL,
    source_text     TEXT NOT NULL,
    embedding       vector(384) NOT NULL,
    model_id        TEXT NOT NULL,
    retention_tier  SMALLINT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (id, bucket)
);

-- Convert to hypertable
SELECT create_hypertable('gold.text_embeddings', 'bucket',
    chunk_time_interval => INTERVAL '7 days',
    if_not_exists => TRUE
);

-- HNSW index for vector similarity search
CREATE INDEX IF NOT EXISTS idx_text_embeddings_hnsw
    ON gold.text_embeddings
    USING hnsw (embedding vector_cosine_ops)
    WITH (m = 16, ef_construction = 64);

-- Lookup index for domain + time range queries
CREATE INDEX IF NOT EXISTS idx_text_embeddings_domain_bucket
    ON gold.text_embeddings (domain_id, bucket DESC);
```

## deploy.sh Integration

No changes to `deploy/pi/deploy.sh` Phase 6 are needed. The init-script runs at database bootstrap (Phase 4.5 in ops-008 pattern). The ndp-embedder container starts via `docker compose --profile intelligence up -d`.

If deploy.sh needs to build the ndp-embedder image:
```bash
# Add to Phase 2.5 or equivalent:
docker build -f docker/embedder/Dockerfile -t neural-data-platform/ndp-embedder:latest .
```
