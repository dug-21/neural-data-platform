# fe-006: Composite Intelligence + Retention

## Vision

Combine metric embeddings (sensor state) with event embeddings (forecast text) into composite embeddings that capture both "what's happening" and "what's being said." K-NN search on composite embeddings finds past hours where both the sensor readings AND the forecast context were similar — a much more specific match than metrics alone.

Also delivers quantization validation (PQ8) and tiered retention (hot/warm/cold) for event embeddings, proving these capabilities work before high-volume domains need them.

## Tracking

- Feature: fe-006
- GitHub Issue: TBD
- Parent roadmap: `product/features/gold-001/FEATURE-ROADMAPv1.2.md` (Track C: v12-E05 through E09)
- Predecessor: fe-005 (event embeddings)
- Version target: v1.2.x

## What fe-005 Delivers (Prerequisites)

- EventEmbedder with MiniLM ONNX inference
- Template cache
- NWS forecast text as event stream
- `gold.event_embeddings` storage with retention_tier column

## Deliverables

| ID | Task | Description |
|----|------|-------------|
| C-01 | CompositeEmbedder | Combines MetricEmbedder output (~32D) + EventEmbedder centroid (PCA-reduced ~16D) into composite vector (~48D) |
| C-02 | PCA reduction | Reduce 384D event centroid to configurable dimensions (default 16D) for composite |
| C-03 | Composite K-NN search | Search on composite embeddings, return neighbors matching both metrics and text context |
| C-04 | Forecast-aware predictions | Predictions that account for forecast context ("stagnation advisory" vs "incoming front") |
| C-05 | Quantization validation | PQ8 quantize forecast embeddings, benchmark recall vs f32 (>95% target) |
| C-06 | Tiered retention | Background job aging event embeddings: hot (24h, all) -> warm (30d, anomalous) -> cold (forever, centroids only) |
| C-07 | Forecast validation report | Compare forecast text predictions with actual sensor outcomes |
| C-08 | Embedding config schema | Per-stream config for embedding_type (metric/event/composite), quantization, retention |

## Constraints

- Composite embedding dimensions are config-driven per domain
- PCA trained on accumulated event embeddings (needs minimum sample size before meaningful)
- Retention job runs as TimescaleDB scheduled action (not in intelligence binary)
- Quantization is optional per-stream — `none` is valid (and default for low-volume)
- Must support metric-only domains unchanged (composite is additive)

## Acceptance Criteria

| Criterion | Target |
|-----------|--------|
| CompositeEmbedder produces vectors | ~48D composite from metric + event |
| Composite search returns results | K-NN on composite finds relevant neighbors |
| Composite >= metric-only accuracy | A/B comparison on prediction accuracy |
| PQ8 recall vs f32 | >95% on forecast embeddings |
| Tiered retention runs | Hot/warm/cold lifecycle executes on schedule |
| Config-driven pipeline | Stream config determines embedding type and quantization |
| Metric-only path unbroken | Existing metric predictions still work |

## Out of Scope

- Granger causality (fe-007)
- Anomaly detection / dashboards (fe-008)
- SONA learning (fe-009)
- Action framework (fe-010)

## Release

v1.2.x — Composite intelligence. Forecast-aware predictions. Quantization and retention validated.
