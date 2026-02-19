# fe-008: Anomaly Detection + Intelligence Dashboard

## Vision

Surface the intelligence layer's output to users via Grafana dashboards and add anomaly detection as a new intelligence product. Anomaly detection flags hours where the current embedding is unusually distant from known clusters — "this situation doesn't look like anything I've seen before." Combined with the prediction and causal panels, this delivers the first complete observability experience for the intelligence layer.

Consolidates all Grafana visualization work for the intelligence layer into one feature.

## Tracking

- Feature: fe-008
- GitHub Issue: TBD
- Parent roadmap: `product/features/gold-001/FEATURE-ROADMAPv1.2.md` (v12-S08, v12-S09, v12-G07, v12-E09)
- Predecessors: fe-004 (predictions), fe-006 (composite, optional), fe-007 (Granger, optional)
- Version target: v1.2.x

## Deliverables

| ID | Task | Description |
|----|------|-------------|
| A-01 | Anomaly detection | Flag hours where embedding distance from nearest cluster exceeds configurable threshold |
| A-02 | Anomaly storage | Store anomaly flags in gold.metric_embeddings or separate table |
| A-03 | Anomaly config | Threshold and sensitivity settings in domain intelligence config |
| D-01 | Prediction dashboard | Panels: prediction timeline, accuracy over time, confidence distribution |
| D-02 | Similarity dashboard | Panels: embedding clusters (2D projection), neighbor distances, warmup progress |
| D-03 | Anomaly dashboard | Panels: anomaly timeline, flagged hours, distance histogram |
| D-04 | Causal dashboard | Panels: validated relationships (if Granger enabled), lag heatmap, evidence strength |
| D-05 | Forecast validation | Panel: forecast text predictions vs actual sensor outcomes (if composite enabled) |
| D-06 | Intelligence CLI | `ndp intelligence status/search` CLI extensions for operational queries |

## Constraints

- Dashboard panels must degrade gracefully — Granger panels show "Granger disabled" when feature flag off, composite panels show "metric-only mode" when no event streams
- Anomaly thresholds are config-driven, not hardcoded
- 2D projection for cluster visualization uses simple PCA (not t-SNE or UMAP — those are too expensive for Pi)
- Dashboard must work with the existing Grafana instance (no additional services)

## Acceptance Criteria

| Criterion | Target |
|-----------|--------|
| Anomaly detection flags threshold crossings | >80% of known threshold crossings flagged |
| Dashboard loads without errors | All panels render with current data |
| Panels degrade gracefully | No errors when Granger disabled or composite unavailable |
| CLI status command works | `ndp intelligence status` shows current state |
| Anomaly threshold configurable | Change threshold without code changes |

## Out of Scope

- SONA learning (fe-009)
- Action framework (fe-010)
- Cross-domain dashboards (future)

## Release

v1.2.x — Intelligence dashboard + anomaly detection. Completes V1.2.
