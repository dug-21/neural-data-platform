# Layered Data Quality Strategy

## Overview

Data quality is applied in layers, with increasing sophistication and different transparency requirements at each level.

## DQ Layer Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                       LAYERED DQ STRATEGY                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ LAYER 1: EXTRACT DQ                                          │   │
│  │ Location: config/base/streams/*/config.yaml                  │   │
│  │ Goal: Reject obvious garbage before Bronze                   │   │
│  │ Actions: reject, warn                                        │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│                              ▼                                      │
│                    ┌─────────────────┐                              │
│                    │  BRONZE LAYER   │                              │
│                    │  (Raw JSON)     │                              │
│                    └─────────────────┘                              │
│                              │                                      │
│                              ▼                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ LAYER 2: TRANSFORM DQ                                        │   │
│  │ Location: config/silver/streams/*/dq.yaml                    │   │
│  │ Goal: Validate during Bronze → Silver ETL                    │   │
│  │ Actions: reject, flag, clamp, set_null, warn                 │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│                              ▼                                      │
│                    ┌─────────────────┐                              │
│                    │  SILVER LAYER   │                              │
│                    │  (TimescaleDB)  │                              │
│                    └─────────────────┘                              │
│                              │                                      │
│                              ▼                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ LAYER 3: ANALYTICS DQ                                        │   │
│  │ Location: Continuous aggregates, monitoring queries          │   │
│  │ Goal: Detect anomalies, drift, completeness issues           │   │
│  │ Actions: alert, report                                       │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Layer 1: Extract DQ

**Location**: `config/base/streams/*/config.yaml`

**Purpose**: Catch obvious garbage at ingestion time, before writing to Bronze.

**Principle**: Be conservative - only reject what is clearly invalid. When in doubt, write to Bronze and let Transform DQ handle it.

### Configuration Example

```yaml
# config/base/streams/nws-gridpoints-forecast/config.yaml
stream_id: nws-gridpoints-forecast

# Extract-level DQ: Applied BEFORE writing to Bronze
extract_dq:
  # Structural validation - payload must have these paths
  required_paths:
    - properties.temperature
    - properties.updateTime

  # Hard rejections (these don't go to Bronze)
  reject_on:
    - missing_required_paths
    - json_parse_error
    - http_error_response

  # Soft warnings (log but still write to Bronze)
  warn_on:
    - properties.temperature.values: { min_length: 1 }
    - properties.windSpeed.values: { min_length: 1 }
```

### Transparency: Rejected Payloads

Rejected payloads should be written to a quarantine location for debugging:

```
/data/bronze_rejected/
  └── nws-gridpoints-forecast/
      └── 2026-01-01/
          └── rejected_12345.json  # With reason code
```

Or a dedicated table:
```sql
CREATE TABLE bronze.rejected_payloads (
    timestamp       TIMESTAMPTZ,
    stream_id       TEXT,
    rejection_reason TEXT,
    raw_payload     JSONB
);
```

---

## Layer 2: Transform DQ

**Location**: `config/silver/streams/*/dq.yaml`

**Purpose**: Validate data during Bronze → Silver ETL. More sophisticated rules, domain-specific logic.

**Principle**: Transparency is paramount. Every DQ decision should be auditable.

### Configuration Example

```yaml
# config/silver/streams/nws-gridpoints-forecast/dq.yaml
transform_dq:
  # Row-level rules (applied to each value during ETL)
  row_rules:
    - name: temperature_range
      column: temperature_c
      rule: between(-60, 60)
      on_violation: set_null_and_flag
      # Don't reject - might be valid extreme weather

    - name: valid_time_reasonable
      expression: "valid_time <= issue_time + interval '8 days'"
      on_violation: reject_row
      # NWS only forecasts 7 days out; 8+ is data error

    - name: humidity_range
      column: humidity_pct
      rule: between(0, 100)
      on_violation: clamp
      # Force to valid range (0 or 100)

    - name: precip_prob_range
      column: precip_prob_pct
      rule: between(0, 100)
      on_violation: clamp

    - name: wind_direction_range
      column: wind_direction_deg
      rule: between(0, 360)
      on_violation: modulo(360)
      # Wrap around: 365 → 5

  # Batch-level rules (checked after ETL batch completes)
  batch_rules:
    - name: completeness_temperature
      rule: "COUNT(temperature_c) / COUNT(*) >= 0.95"
      on_violation: warn_alert
      # Expect 95% of forecasts to have temperature

    - name: forecast_horizon
      rule: "MAX(lead_time_hours) >= 168"
      on_violation: warn_alert
      # Should have full 7-day forecast

    - name: reasonable_row_count
      rule: "COUNT(*) BETWEEN 1000 AND 10000"
      on_violation: warn_alert
      # Typical NWS gridpoint has ~5000 data points

  # Transparency output
  dq_output:
    table: silver.dq_results
    include_sample_failures: true
    max_samples_per_rule: 10
```

### Violation Actions

| Action | Behavior | Use When |
|--------|----------|----------|
| `reject_row` | Don't load row to Silver | Logically impossible values |
| `set_null_and_flag` | Set value to NULL, flag in DQ table | Suspicious but possible |
| `clamp` | Force to valid range | Physical constraints (0-100%) |
| `modulo` | Wrap around | Circular values (degrees) |
| `warn` | Log but load as-is | Unusual but valid |

### Transparency Table

```sql
CREATE TABLE silver.dq_results (
    check_time      TIMESTAMPTZ DEFAULT NOW(),
    batch_id        TEXT,           -- Links to ETL run
    stream_id       TEXT,
    rule_name       TEXT,
    rule_level      TEXT,           -- 'row' or 'batch'
    violation_type  TEXT,           -- 'reject', 'flag', 'clamp', 'warn'
    row_count       INTEGER,        -- How many rows affected
    sample_payload  JSONB,          -- Example of failing data
    context         JSONB           -- Additional debugging info
);

-- Index for dashboard queries
CREATE INDEX idx_dq_results_stream_time
ON silver.dq_results (stream_id, check_time DESC);
```

### DQ Dashboard Query

```sql
-- What's failing DQ in the last 24 hours?
SELECT
    stream_id,
    rule_name,
    violation_type,
    SUM(row_count) as total_violations,
    COUNT(*) as batch_count
FROM silver.dq_results
WHERE check_time > NOW() - INTERVAL '24 hours'
GROUP BY 1, 2, 3
ORDER BY total_violations DESC;
```

---

## Layer 3: Analytics DQ

**Location**: Continuous aggregates, scheduled queries, monitoring.

**Purpose**: Detect anomalies, data drift, and completeness issues over time.

### Examples

**Completeness Monitoring**:
```sql
-- Continuous aggregate: hourly completeness by stream
CREATE MATERIALIZED VIEW analytics.hourly_completeness
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', valid_time) AS hour,
    ndp_id,
    COUNT(*) AS row_count,
    COUNT(temperature_c) AS temp_count,
    COUNT(wind_speed_kmh) AS wind_count
FROM silver.weather_forecasts
GROUP BY 1, 2;

-- Alert if completeness drops
SELECT * FROM analytics.hourly_completeness
WHERE temp_count::float / row_count < 0.9
  AND hour > NOW() - INTERVAL '4 hours';
```

**Anomaly Detection**:
```sql
-- Flag temperature values outside 3 standard deviations
WITH stats AS (
    SELECT
        AVG(temperature_c) as mean_temp,
        STDDEV(temperature_c) as std_temp
    FROM silver.weather_forecasts
    WHERE valid_time > NOW() - INTERVAL '30 days'
)
SELECT *
FROM silver.weather_forecasts, stats
WHERE ABS(temperature_c - mean_temp) > 3 * std_temp
  AND valid_time > NOW() - INTERVAL '1 day';
```

**Freshness Monitoring**:
```sql
-- Alert if no new data in 2 hours
SELECT
    stream_id,
    MAX(ingestion_time) as last_ingestion,
    NOW() - MAX(ingestion_time) as staleness
FROM silver.weather_forecasts
GROUP BY stream_id
HAVING NOW() - MAX(ingestion_time) > INTERVAL '2 hours';
```

---

## Summary: DQ by Layer

| Layer | Location | Goal | Actions | Transparency |
|-------|----------|------|---------|--------------|
| **Extract** | Stream config | Reject garbage | reject, warn | Quarantine table |
| **Transform** | Silver DQ config | Validate/clean | reject, flag, clamp, null | DQ results table |
| **Analytics** | Continuous aggs | Monitor quality | alert, report | Dashboards |

## Key Principles

1. **Be conservative in Extract**: Only reject what is clearly invalid
2. **Be transparent in Transform**: Every decision is auditable
3. **Be proactive in Analytics**: Detect issues before users notice
4. **Bronze is sacred**: Never modify raw data; DQ happens on read/transform
