# fe-005 Pseudocode: database

## gold.text_embeddings Table

See `deploy.md` for the full init-script SQL. This file documents the schema design rationale.

### Column Design

| Column | Type | Nullable | Purpose |
|--------|------|----------|---------|
| id | BIGSERIAL | NOT NULL | Unique row ID (part of composite PK with bucket) |
| bucket | TIMESTAMPTZ | NOT NULL | Time bucket for the source text (from Gold text view) |
| domain_id | TEXT | NOT NULL | Domain identifier (e.g., "indoor-air-quality") |
| source_stream | TEXT | NOT NULL | Stream that produced the text (e.g., "nws-forecast-hourly") |
| source_column | TEXT | NOT NULL | Column name in the Silver table (e.g., "short_forecast") |
| source_text | TEXT | NOT NULL | Original text that was embedded (provenance) |
| embedding | vector(384) | NOT NULL | pgvector embedding (384 dimensions, matching model output) |
| model_id | TEXT | NOT NULL | Model that produced this embedding (e.g., "BAAI/bge-small-en-v1.5") |
| retention_tier | SMALLINT | NULL | Retention tier -- present but unpopulated (fe-006) |
| created_at | TIMESTAMPTZ | NOT NULL | When this embedding was computed |

### Index Design

1. **HNSW index** (`idx_text_embeddings_hnsw`): For vector similarity search (fe-006 composite search). Uses `vector_cosine_ops` for cosine similarity. Parameters: m=16, ef_construction=64.

2. **Domain+bucket index** (`idx_text_embeddings_domain_bucket`): For time-range queries filtered by domain. DESC ordering for efficient "latest N" queries.

### Hypertable Configuration

- Chunk interval: 7 days (matches expected data volume -- 1-4 embeddings per hour per domain)
- Partitioning column: `bucket` (time dimension)
- Composite PK: `(id, bucket)` required by TimescaleDB for distributed hypertables

### Comparison with gold.metric_embeddings

| Aspect | gold.metric_embeddings | gold.text_embeddings |
|--------|----------------------|---------------------|
| Dimensions | 7 (metric z-scores) | 384 (model output) |
| Row size | ~28B vector + metadata | ~1.5KB vector + text |
| Granularity | One per domain per bucket | One per text field per bucket |
| Index | HNSW | HNSW |
| Source | Gold aligned view (numeric) | Gold text view (text) |
| Producer | ndp-intelligence | ndp-embedder |
