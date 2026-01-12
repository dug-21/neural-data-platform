# Silver Layer ML Platform Assessment

**Document**: 07-ml-platform-assessment.md
**Version**: 1.0
**Date**: 2026-01-05
**Author**: NDP ML Engineer
**Status**: Complete

---

## Executive Summary

This document assesses the Silver layer design from an ML/Feature Engineering perspective, evaluating its suitability as a **multi-domain data platform** capable of supporting N diverse domains beyond the initial weather/air quality use case.

### Assessment Summary

| Dimension | Current State | Multi-Domain Readiness | Priority |
|-----------|---------------|------------------------|----------|
| **Feature Engineering Portability** | Good foundation | Needs abstraction layer | Critical |
| **Schema Evolution** | Config-driven ETL | Needs domain-agnostic patterns | Important |
| **Training Data Pipeline** | Not addressed | Requires feature store integration | Critical |
| **DQ for ML** | Range checks only | Needs drift/distribution monitoring | Important |

### Verdict

The config-driven ETL approach in `06-refined-synthesis.md` provides an **excellent foundation** for multi-domain extensibility. However, the current design is heavily influenced by the weather/air quality domain semantics. To become a true generic ML-ready data platform, specific enhancements are required.

---

## 1. Feature Engineering Portability

### What Works Well

**1.1 Config-Driven Field Mappings**

The `silver_etl.field_mappings` pattern is highly portable:

```yaml
field_mappings:
  - source_path: raw_payload.pm02
    target_column: pm25
    type: double_precision
    transform:
      type: unit_conversion
      formula: { type: linear, scale: 1.0, offset: 0.0 }
```

This declarative approach can express mappings for ANY domain:

| Domain | Source Path | Target | Transform |
|--------|-------------|--------|-----------|
| Energy | `raw_payload.watts` | `power_w` | Direct |
| Smart Home | `raw_payload.motion_detected` | `motion_event` | Boolean cast |
| Finance | `raw_payload.close_price` | `price_usd` | Currency conversion |
| Industrial | `raw_payload.vibration_hz` | `vibration_freq` | Frequency normalization |

**1.2 Reusable Transform Types**

The transform types are domain-agnostic:
- `unit_conversion` (linear scale+offset) - Universal
- `expression` - SQL-based formulas
- `lookup` - Reference data joins
- `json_extract` - Nested payload access
- `timestamp` - Temporal parsing
- `computed` - Derived fields

**1.3 DQ Rules as Config**

The DQ rule pattern (`range_check`, `not_null`, `pattern`, `one_of`) is portable across domains.

### What Needs Change

**1.4 Missing: Windowed Aggregations**

The current config lacks support for time-series feature engineering:

```yaml
# MISSING: Window functions for ML features
feature_windows:
  - name: pm25_mean_1h
    source_column: pm25
    window: 1 hour
    aggregation: avg

  - name: pm25_trend_4h
    source_column: pm25
    window: 4 hours
    aggregation: linear_slope  # Rate of change
```

**Recommendation**: Add `feature_windows` section to support:
- Rolling aggregations (avg, min, max, stddev, percentiles)
- Lag features (value_at_t_minus_1h)
- Rate of change / trend detection
- Cross-stream joins with temporal alignment

**1.5 Missing: Cross-Stream Feature Joins**

ML models often need features from multiple streams:

```yaml
# MISSING: Cross-stream feature composition
cross_stream_features:
  - name: indoor_outdoor_pm_ratio
    numerator: { stream: air-quality, column: pm25 }
    denominator: { stream: outdoor-air-quality, column: pm25 }
    temporal_align: nearest_within_10min
```

**Recommendation**: Add declarative cross-stream join syntax.

**1.6 Domain-Specific Computed Fields**

Current computed fields like `calculate_aqi_pm25()` and `calculate_heat_index()` are domain-specific SQL functions. For multi-domain support:

**Current (Domain-Coupled)**:
```sql
CREATE FUNCTION calculate_aqi_pm25(pm25 DOUBLE PRECISION) RETURNS SMALLINT ...
```

**Recommended (Domain-Agnostic)**:
```yaml
computed_fields:
  - name: aqi_pm25
    type: lookup_interpolation
    input_column: pm25
    breakpoints:
      - { input_low: 0, input_high: 9.0, output_low: 0, output_high: 50 }
      - { input_low: 9.1, input_high: 35.4, output_low: 51, output_high: 100 }
      # ... EPA breakpoints as config, not code
```

This makes the EPA AQI calculation a **configured pattern**, not a hardcoded function. Future domains can define their own lookup/interpolation rules.

---

## 2. Schema Evolution

### What Works Well

**2.1 Hypertable Pattern is Universal**

The TimescaleDB hypertable with time-based partitioning works for ANY time-series domain:

```sql
CREATE TABLE silver.{domain}_{entity} (
    observation_time TIMESTAMPTZ NOT NULL,
    ndp_id TEXT NOT NULL,
    -- Domain-specific metrics (config-driven)
    dq_flags TEXT[],
    PRIMARY KEY (observation_time, ndp_id)
);
SELECT create_hypertable(...);
```

**2.2 Continuous Aggregates are Domain-Agnostic**

The aggregation pattern applies universally:

| Domain | Raw Table | Hourly Aggregate | Daily Aggregate |
|--------|-----------|------------------|-----------------|
| Air Quality | `air_quality_observations` | `air_quality_hourly` | `air_quality_daily` |
| Energy | `energy_observations` | `energy_hourly` | `energy_daily` |
| Finance | `price_ticks` | `price_hourly` | `price_daily` |

### What Needs Change

**2.3 Dynamic Schema Generation**

Currently, table schemas are manually defined per domain. For N domains, we need:

```yaml
# Stream config should GENERATE schema
silver_etl:
  target_schema: silver
  target_table_suffix: observations  # silver.{stream_id}_observations

  # Schema derived from field_mappings
  field_mappings:
    - target_column: temperature_c
      type: double_precision
      # ... generates column in CREATE TABLE
```

**Recommendation**: The ETL should auto-generate `CREATE TABLE` from config, with:
1. Standard columns (observation_time, ndp_id, dq_flags)
2. Domain columns from field_mappings
3. Hypertable creation
4. Index generation

**2.4 Schema Versioning**

For ML reproducibility, schema changes must be versioned:

```yaml
silver_etl:
  schema_version: "1.2.0"
  migration_strategy: additive  # never drop columns

  # Track schema lineage
  changelog:
    - version: "1.1.0"
      date: "2026-01-01"
      changes: ["Added pm25_compensated column"]
```

**Recommendation**: Add schema versioning with migration tracking.

**2.5 Entity Registry**

A central registry of domain entities aids discoverability:

```yaml
# config/entities/registry.yaml
entities:
  - id: indoor-air-quality
    domain: environment
    category: air_quality
    frequency: ~1 min
    ml_use_cases: [anomaly_detection, forecasting]

  - id: outdoor-weather
    domain: environment
    category: weather
    frequency: ~10 min
    ml_use_cases: [forecasting, correlation_analysis]
```

---

## 3. Training Data Pipeline

### Current Gap: No Feature Store

The Silver layer design stops at "analytics-ready" tables. For ML, we need the **Gold layer** (feature store):

```
Bronze (Raw)
    |
    v
Silver (Clean, Typed)       <-- Current design stops here
    |
    v
Gold (Features for ML)      <-- MISSING
    |
    v
Model Training / Inference
```

### Recommendations

**3.1 Feature Table Pattern**

Add config support for feature tables:

```yaml
# config/features/air-quality-forecast-features.yaml
feature_set:
  id: air-quality-forecast-v1
  target_table: gold.air_quality_features

  # Point-in-time features (no leakage)
  features:
    - name: pm25_current
      source_table: silver.air_quality_observations
      column: pm25
      temporal_join: exact

    - name: pm25_mean_1h
      source_table: silver.air_quality_hourly
      column: avg_pm25
      temporal_join: lag_1h  # Value from 1 hour ago

    - name: pm25_trend_4h
      source_table: silver.air_quality_observations
      window: 4 hours
      aggregation: linear_slope

    - name: outdoor_pm25
      source_table: silver.outdoor_air_quality
      column: pm25
      temporal_join: nearest_within_10min

  # Labels for supervised learning
  label:
    name: pm25_1h_ahead
    source_table: silver.air_quality_observations
    column: pm25
    offset: +1 hour  # Future value
```

**3.2 Batch Export for Training**

```yaml
training_export:
  format: parquet  # Or CSV, Feather
  output_path: /data/training/air-quality-forecast/
  partitioning: daily
  compression: snappy

  # Time-based splits
  splits:
    train: { start: "2025-01-01", end: "2025-10-31" }
    validation: { start: "2025-11-01", end: "2025-11-30" }
    test: { start: "2025-12-01", end: "2025-12-31" }
```

**3.3 Online Feature Serving**

For real-time inference on Pi 5:

```rust
// Feature retrieval for inference
pub struct FeatureStore {
    timescale: Pool<Postgres>,
    cache: HashMap<String, CachedFeatures>,
}

impl FeatureStore {
    pub async fn get_features(
        &self,
        entity_id: &str,
        feature_set: &str,
        timestamp: DateTime<Utc>,
    ) -> Result<FeatureVector, CoreError> {
        // Point-in-time lookup avoiding data leakage
    }
}
```

**3.4 ruv-FANN Integration Path**

The data dictionary already identifies ML use cases:

| Use Case | Silver Tables | Gold Features | Model |
|----------|---------------|---------------|-------|
| Air Quality Forecast | `air_quality_observations`, `outdoor_weather` | pm25 history + weather | ruv-FANN regression |
| Anomaly Detection | All observations | Multi-variate features | ruv-FANN autoencoder |
| HVAC Optimization | Indoor + outdoor | Temp differentials | ruv-FANN classifier |

---

## 4. Data Quality for ML

### Current DQ Rules

The config-driven DQ rules are a good start:

| Rule | Purpose | ML Impact |
|------|---------|-----------|
| `range_check` | Physical bounds | Removes impossible values |
| `not_null` | Required fields | Ensures feature completeness |
| `pattern` | Format validation | String/ID consistency |
| `one_of` | Categorical validation | Known categories only |

### Missing: ML-Specific DQ

**4.1 Feature Distribution Monitoring**

```yaml
ml_dq_rules:
  - type: distribution_drift
    column: pm25
    reference_window: 30 days
    alert_threshold: 0.1  # KL divergence

  - type: null_rate
    column: temperature_c
    max_rate: 0.05  # Max 5% nulls
    window: 24 hours

  - type: value_frequency
    column: wind_direction_deg
    expected_distribution: uniform  # Should be roughly uniform
```

**4.2 Outlier Detection**

```yaml
ml_dq_rules:
  - type: z_score_outlier
    column: pm25
    threshold: 4.0  # Flag if |z| > 4
    action: flag  # Don't remove, just flag

  - type: iqr_outlier
    column: temperature_c
    multiplier: 3.0  # Flag if outside Q1-3*IQR to Q3+3*IQR
```

**4.3 Temporal Consistency**

```yaml
ml_dq_rules:
  - type: temporal_gap
    max_gap: 10 minutes
    alert_on_gap: true

  - type: rate_of_change
    column: pm25
    max_delta_per_minute: 50  # Physical plausibility
```

**4.4 Training Data Quality Report**

Before model training, generate a DQ report:

```yaml
training_dq_report:
  output: /data/training/dq-report-{date}.json

  checks:
    - feature_completeness  # % non-null per column
    - feature_correlation   # Detect highly correlated features
    - label_distribution    # Class balance for classification
    - temporal_coverage     # Gaps in time series
    - outlier_summary       # Flagged outlier counts
```

---

## 5. Specific Recommendations

### Priority: Critical

| ID | Recommendation | Effort | Impact |
|----|----------------|--------|--------|
| ML-01 | Add `feature_windows` to config schema | 3 days | Enables time-series ML features |
| ML-02 | Implement Gold layer feature tables | 5 days | Training data pipeline |
| ML-03 | Add batch export to Parquet | 2 days | Model training workflow |

### Priority: Important

| ID | Recommendation | Effort | Impact |
|----|----------------|--------|--------|
| ML-04 | Distribution drift monitoring | 3 days | Early warning on data changes |
| ML-05 | Cross-stream join syntax | 2 days | Multi-source features |
| ML-06 | Schema versioning | 2 days | ML reproducibility |

### Priority: Nice-to-Have

| ID | Recommendation | Effort | Impact |
|----|----------------|--------|--------|
| ML-07 | Entity registry | 1 day | Discoverability |
| ML-08 | Computed field config (vs SQL functions) | 2 days | Domain portability |
| ML-09 | Online feature serving cache | 3 days | Real-time inference |

---

## 6. MLOps Best Practices to Adopt

### 6.1 Feature Store Pattern (Feast-Inspired)

```
                    Offline Store              Online Store
                    (TimescaleDB)             (Redis/Cache)
                         |                         |
     Training Job -------|                         |-----> Inference
                         |                         |
                    Feature Registry (etcd)
                         |
                    Feature Definitions (YAML)
```

For Pi 5, simplify to:
- **Offline**: TimescaleDB (already planned)
- **Online**: In-memory cache in Rust binary
- **Registry**: etcd (already used)
- **Definitions**: YAML (already pattern)

### 6.2 Point-in-Time Correctness

Training data must avoid **data leakage**:

```sql
-- WRONG: Uses future data
SELECT pm25, outdoor_temp
FROM silver.air_quality_observations a
JOIN silver.outdoor_weather w ON a.ndp_id = w.ndp_id
  AND a.observation_time = w.observation_time;  -- May join with slightly newer data

-- CORRECT: Point-in-time join
SELECT pm25,
       (SELECT outdoor_temp
        FROM silver.outdoor_weather w
        WHERE w.observation_time <= a.observation_time
        ORDER BY w.observation_time DESC
        LIMIT 1) as outdoor_temp
FROM silver.air_quality_observations a;
```

**Recommendation**: Feature config should specify `temporal_join` semantics.

### 6.3 Model Versioning Integration

```yaml
# Future: Link features to model versions
model_registry:
  - model_id: air-quality-forecast-v1
    feature_set: air-quality-forecast-features-v1
    silver_schema_version: "1.2.0"
    training_data:
      start: "2025-01-01"
      end: "2025-10-31"
    metrics:
      mse: 3.2
      mae: 1.4
```

### 6.4 A/B Testing Infrastructure

For comparing model versions in production:

```yaml
inference_config:
  models:
    - id: air-quality-forecast-v1
      weight: 0.9  # 90% traffic
    - id: air-quality-forecast-v2
      weight: 0.1  # 10% traffic (A/B test)

  metrics_table: gold.model_predictions
  # Compare predictions vs actuals
```

---

## 7. Domain Extension Examples

To validate genericity, here's how the config pattern would extend to other domains:

### 7.1 Energy Monitoring

```yaml
stream_id: solar-inverter
silver_etl:
  target_table: silver.energy_observations

  field_mappings:
    - source_path: raw_payload.dc_power
      target_column: power_dc_w
      type: double_precision
      dq_rules:
        - rule: range_check
          min: 0
          max: 10000  # 10kW max

    - source_path: raw_payload.efficiency
      target_column: efficiency_pct
      type: double_precision

  computed_fields:
    - name: daily_energy_kwh
      expression: "SUM(power_dc_w) / 1000 / 60"  # Minute data to kWh
```

### 7.2 Smart Home Motion

```yaml
stream_id: motion-sensors
silver_etl:
  target_table: silver.motion_events

  field_mappings:
    - source_path: raw_payload.motion
      target_column: motion_detected
      type: boolean

    - source_path: raw_payload.battery_pct
      target_column: battery_pct
      type: smallint
      dq_rules:
        - rule: range_check
          min: 0
          max: 100
```

### 7.3 Financial Ticks

```yaml
stream_id: stock-ticks
silver_etl:
  target_table: silver.price_observations

  field_mappings:
    - source_path: raw_payload.last_price
      target_column: price_usd
      type: double_precision

    - source_path: raw_payload.volume
      target_column: volume
      type: bigint

  feature_windows:  # ML-specific
    - name: price_sma_20
      source_column: price_usd
      window: 20 ticks
      aggregation: avg

    - name: volume_spike
      source_column: volume
      window: 10 ticks
      aggregation: stddev
```

---

## 8. Conclusion

### Strengths

1. **Config-driven ETL** is fundamentally sound and portable
2. **TimescaleDB hypertables** are domain-agnostic
3. **DQ transparency** (`dq_flags`) is ML-friendly
4. **GitOps workflow** enables versioned, reproducible pipelines

### Gaps for Multi-Domain ML

1. **No feature windowing** - Critical for time-series ML
2. **No Gold layer** - Training data pipeline missing
3. **No distribution monitoring** - Drift detection needed
4. **Domain-coupled computed fields** - Hardcoded SQL functions

### Recommended Next Steps

1. **Phase 1 (with Silver)**: Add `feature_windows` to config schema
2. **Phase 2 (post-Silver)**: Design Gold layer feature store
3. **Phase 3 (ML launch)**: Implement batch export + ruv-FANN integration

The foundation is solid. With targeted enhancements, NDP can become a **multi-domain ML-ready platform**.

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-05 | NDP ML Engineer | Initial assessment |

---

## References

1. `03-data-dictionary.md` - Silver layer schemas
2. `06-refined-synthesis.md` - Config-driven ETL design
3. `PLATFORM_ARCHITECTURE_OVERVIEW.md` - System context
4. Feast Feature Store documentation - MLOps patterns
5. TimescaleDB continuous aggregates - Time-series optimization
6. ruv-FANN documentation - Neural network integration

---

*Assessment completed: 2026-01-05*
*Reviewer: NDP ML Engineer*
