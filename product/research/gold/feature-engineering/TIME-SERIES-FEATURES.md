# Time-Series Feature Engineering for Neural Data Platform

**Research Date**: 2026-02-02
**Platform**: Raspberry Pi 5 (ARM64, 8GB RAM)
**Context**: Air quality monitoring with PMS5003 sensors, NWS weather forecasts
**Target**: ruv-FANN neural network predictions
**Current State**: Bronze layer operational, TimescaleDB Silver layer in development

---

## Executive Summary

This research catalogs time-series features valuable for environmental sensor prediction, with specific focus on air quality and weather domains. The analysis prioritizes lightweight approaches suitable for edge deployment on Raspberry Pi, leveraging TimescaleDB continuous aggregates for efficient computation.

### Key Findings

| Category | Recommendation | Rationale |
|----------|---------------|-----------|
| **Lag Features** | 1h, 6h, 24h lags | Capture short-term dynamics and daily patterns |
| **Rolling Statistics** | 4h, 24h windows | Balance recency with noise reduction |
| **Seasonal Features** | Hour-of-day, day-of-week, month | Critical for diurnal pollution/weather patterns |
| **Domain Features** | AQI breakpoints, pollutant ratios | Domain expertise improves model interpretability |
| **Computation Strategy** | TimescaleDB continuous aggregates | SQL-native, auto-refresh, minimal resource usage |
| **Feature Selection** | Tree-based importance + L1 regularization | Reduce feature count for edge inference |

---

## 1. Catalog of Time-Series Features

### 1.1 Lag Features (Autoregressive)

Lag features capture temporal dependencies by using previous values as predictors. They are foundational for time-series forecasting.

```sql
-- TimescaleDB implementation
SELECT
    time_bucket('1 hour', timestamp) AS bucket,
    pm25,
    LAG(pm25, 1) OVER (ORDER BY timestamp) AS pm25_lag_1h,
    LAG(pm25, 6) OVER (ORDER BY timestamp) AS pm25_lag_6h,
    LAG(pm25, 24) OVER (ORDER BY timestamp) AS pm25_lag_24h,
    LAG(pm25, 168) OVER (ORDER BY timestamp) AS pm25_lag_1week
FROM air_quality_readings;
```

**Best Practices for Lag Selection:**

| Lag Period | Use Case | Rationale |
|------------|----------|-----------|
| 1h | Short-term dynamics | Captures recent changes |
| 6h | Medium-term trends | Morning/afternoon shift |
| 24h | Daily pattern | Same hour yesterday |
| 168h (1 week) | Weekly pattern | Same hour/day last week |

**Edge Consideration**: Limit to 3-5 lag features to reduce memory footprint during inference.

### 1.2 Rolling Window Statistics

Rolling statistics smooth noise and highlight trends. They are essential for capturing local temporal dynamics.

```sql
-- Rolling window features
SELECT
    time_bucket('1 hour', timestamp) AS bucket,

    -- 4-hour rolling window
    AVG(pm25) OVER w4h AS pm25_mean_4h,
    STDDEV(pm25) OVER w4h AS pm25_std_4h,
    MAX(pm25) OVER w4h AS pm25_max_4h,
    MIN(pm25) OVER w4h AS pm25_min_4h,

    -- 24-hour rolling window
    AVG(pm25) OVER w24h AS pm25_mean_24h,
    STDDEV(pm25) OVER w24h AS pm25_std_24h,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY pm25) OVER w24h AS pm25_p95_24h,

    -- Volatility ratio (short vs long term)
    STDDEV(pm25) OVER w4h / NULLIF(STDDEV(pm25) OVER w24h, 0) AS pm25_volatility_ratio

FROM air_quality_readings
WINDOW
    w4h AS (ORDER BY timestamp ROWS BETWEEN 3 PRECEDING AND CURRENT ROW),
    w24h AS (ORDER BY timestamp ROWS BETWEEN 23 PRECEDING AND CURRENT ROW);
```

**Recommended Rolling Statistics:**

| Statistic | Window Size | Purpose |
|-----------|-------------|---------|
| Mean | 4h, 24h | Central tendency |
| Std Dev | 4h, 24h | Volatility/variability |
| Min/Max | 4h, 24h | Range detection |
| Percentile (P95) | 24h | Extreme value tracking |
| Slope (linear regression) | 4h | Trend direction |

**Window Size Guidelines:**
- **4-hour window**: Captures recent trends, sensitive to changes
- **24-hour window**: Smooths daily variations, robust to outliers
- Avoid windows larger than 7 days on edge devices (memory constraints)

### 1.3 Differencing Features

Differencing removes trends and helps with stationarity.

```sql
-- First and second order differences
SELECT
    timestamp,
    pm25,
    pm25 - LAG(pm25, 1) OVER (ORDER BY timestamp) AS pm25_diff_1h,
    pm25 - LAG(pm25, 24) OVER (ORDER BY timestamp) AS pm25_diff_24h,
    (pm25 - LAG(pm25, 1) OVER (ORDER BY timestamp)) -
    (LAG(pm25, 1) OVER (ORDER BY timestamp) - LAG(pm25, 2) OVER (ORDER BY timestamp))
        AS pm25_diff2_1h,

    -- Rate of change (per hour)
    (pm25 - LAG(pm25, 1) OVER (ORDER BY timestamp)) / 1.0 AS pm25_rate_of_change

FROM air_quality_readings;
```

**Differencing Types:**

| Type | Formula | Use Case |
|------|---------|----------|
| First difference | y(t) - y(t-1) | Remove linear trend |
| Seasonal difference | y(t) - y(t-24) | Remove daily seasonality |
| Second difference | diff(diff(y)) | Remove quadratic trend |
| Rate of change | diff / time_delta | Velocity of change |

### 1.4 Trend Features

Trend features capture the direction and strength of movement.

```sql
-- Trend features using linear regression
SELECT
    time_bucket('1 hour', timestamp) AS bucket,

    -- Linear regression slope (trend direction)
    REGR_SLOPE(pm25, EXTRACT(EPOCH FROM timestamp)) OVER w4h AS pm25_trend_4h,
    REGR_SLOPE(pm25, EXTRACT(EPOCH FROM timestamp)) OVER w24h AS pm25_trend_24h,

    -- R-squared (trend strength)
    POWER(REGR_R2(pm25, EXTRACT(EPOCH FROM timestamp)) OVER w4h, 2) AS pm25_trend_strength_4h,

    -- Momentum (price change over n periods)
    pm25 - LAG(pm25, 4) OVER (ORDER BY timestamp) AS pm25_momentum_4h

FROM air_quality_readings
WINDOW
    w4h AS (ORDER BY timestamp ROWS BETWEEN 3 PRECEDING AND CURRENT ROW),
    w24h AS (ORDER BY timestamp ROWS BETWEEN 23 PRECEDING AND CURRENT ROW);
```

### 1.5 Seasonal Decomposition Features

For environmental data, seasonal patterns at multiple scales are critical.

**Multi-Seasonal Decomposition (MSTL):**

Environmental sensor data typically exhibits:
- **Diurnal (24h)**: Temperature peaks at 4 PM, pollution varies with traffic
- **Weekly (168h)**: Lower pollution on weekends, different HVAC patterns
- **Annual (8760h)**: Seasonal temperature and pollution variations

```python
# Python implementation using statsmodels MSTL
from statsmodels.tsa.seasonal import MSTL

# For hourly data with daily and weekly seasonality
mstl = MSTL(data, periods=(24, 168))  # 24h and 168h periods
result = mstl.fit()

# Extract components
trend = result.trend
seasonal_daily = result.seasonal[:, 0]  # 24h component
seasonal_weekly = result.seasonal[:, 1]  # 168h component
residual = result.resid
```

**SQL Approximation for Edge:**

```sql
-- Capture seasonal pattern averages
CREATE MATERIALIZED VIEW seasonal_patterns AS
SELECT
    EXTRACT(HOUR FROM timestamp) AS hour_of_day,
    EXTRACT(DOW FROM timestamp) AS day_of_week,
    AVG(pm25) AS pm25_seasonal_mean,
    STDDEV(pm25) AS pm25_seasonal_std
FROM air_quality_readings
WHERE timestamp >= NOW() - INTERVAL '90 days'
GROUP BY hour_of_day, day_of_week;

-- Join back to compute deseasonalized values
SELECT
    a.timestamp,
    a.pm25,
    a.pm25 - s.pm25_seasonal_mean AS pm25_deseasonalized,
    (a.pm25 - s.pm25_seasonal_mean) / NULLIF(s.pm25_seasonal_std, 0) AS pm25_zscore_seasonal
FROM air_quality_readings a
JOIN seasonal_patterns s ON
    EXTRACT(HOUR FROM a.timestamp) = s.hour_of_day AND
    EXTRACT(DOW FROM a.timestamp) = s.day_of_week;
```

### 1.6 Time-Based (Calendar) Features

Calendar features capture cyclical patterns inherent in time.

```sql
SELECT
    timestamp,

    -- Cyclical encoding (preserves continuity at boundaries)
    SIN(2 * PI() * EXTRACT(HOUR FROM timestamp) / 24) AS hour_sin,
    COS(2 * PI() * EXTRACT(HOUR FROM timestamp) / 24) AS hour_cos,
    SIN(2 * PI() * EXTRACT(DOW FROM timestamp) / 7) AS dow_sin,
    COS(2 * PI() * EXTRACT(DOW FROM timestamp) / 7) AS dow_cos,
    SIN(2 * PI() * EXTRACT(DOY FROM timestamp) / 365) AS doy_sin,
    COS(2 * PI() * EXTRACT(DOY FROM timestamp) / 365) AS doy_cos,

    -- Binary features
    CASE WHEN EXTRACT(DOW FROM timestamp) IN (0, 6) THEN 1 ELSE 0 END AS is_weekend,
    CASE WHEN EXTRACT(HOUR FROM timestamp) BETWEEN 6 AND 9
         OR EXTRACT(HOUR FROM timestamp) BETWEEN 16 AND 19
         THEN 1 ELSE 0 END AS is_rush_hour,
    CASE WHEN EXTRACT(HOUR FROM timestamp) BETWEEN 6 AND 18
         THEN 1 ELSE 0 END AS is_daytime

FROM air_quality_readings;
```

**Why Cyclical Encoding?**
- Hour 23 and hour 0 are adjacent, but raw encoding treats them as distant
- Sin/cos encoding preserves circular continuity
- Reduces feature count vs. one-hot encoding (2 features vs. 24)

---

## 2. Domain-Specific Features: Air Quality

### 2.1 AQI Calculation Features

The EPA Air Quality Index uses breakpoint-based calculations with specific health categories.

```sql
-- AQI calculation for PM2.5 (EPA breakpoints)
CREATE OR REPLACE FUNCTION calculate_aqi_pm25(pm25_concentration DOUBLE PRECISION)
RETURNS INTEGER AS $$
DECLARE
    c_low DOUBLE PRECISION;
    c_high DOUBLE PRECISION;
    i_low INTEGER;
    i_high INTEGER;
    aqi INTEGER;
BEGIN
    -- EPA breakpoints for PM2.5 (24-hour average)
    IF pm25_concentration <= 12.0 THEN
        c_low := 0.0; c_high := 12.0; i_low := 0; i_high := 50;
    ELSIF pm25_concentration <= 35.4 THEN
        c_low := 12.1; c_high := 35.4; i_low := 51; i_high := 100;
    ELSIF pm25_concentration <= 55.4 THEN
        c_low := 35.5; c_high := 55.4; i_low := 101; i_high := 150;
    ELSIF pm25_concentration <= 150.4 THEN
        c_low := 55.5; c_high := 150.4; i_low := 151; i_high := 200;
    ELSIF pm25_concentration <= 250.4 THEN
        c_low := 150.5; c_high := 250.4; i_low := 201; i_high := 300;
    ELSIF pm25_concentration <= 500.4 THEN
        c_low := 250.5; c_high := 500.4; i_low := 301; i_high := 500;
    ELSE
        RETURN 500; -- Hazardous
    END IF;

    -- Linear interpolation formula
    aqi := ROUND(((i_high - i_low) / (c_high - c_low)) * (pm25_concentration - c_low) + i_low);
    RETURN aqi;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Usage in feature view
SELECT
    timestamp,
    pm25,
    calculate_aqi_pm25(pm25) AS aqi,
    CASE
        WHEN calculate_aqi_pm25(pm25) <= 50 THEN 'Good'
        WHEN calculate_aqi_pm25(pm25) <= 100 THEN 'Moderate'
        WHEN calculate_aqi_pm25(pm25) <= 150 THEN 'USG'
        WHEN calculate_aqi_pm25(pm25) <= 200 THEN 'Unhealthy'
        WHEN calculate_aqi_pm25(pm25) <= 300 THEN 'Very Unhealthy'
        ELSE 'Hazardous'
    END AS aqi_category
FROM air_quality_readings;
```

### 2.2 Pollutant Ratio Features

Ratios between pollutants reveal emission sources and atmospheric chemistry.

```sql
SELECT
    timestamp,

    -- PM ratios (source identification)
    pm25 / NULLIF(pm10, 0) AS pm25_pm10_ratio,  -- <0.5 indicates coarse particles (dust)
                                                  -- >0.7 indicates fine particles (combustion)

    -- Indoor/outdoor penetration
    indoor_pm25 / NULLIF(outdoor_pm25, 0) AS indoor_outdoor_ratio,
    indoor_pm25 - outdoor_pm25 AS indoor_outdoor_diff,

    -- CO2-based occupancy proxy (indoor)
    co2 - 420 AS co2_above_baseline,  -- 420 ppm outdoor baseline (2026)
    (co2 - 420) / NULLIF(occupancy_estimate, 0) AS co2_per_person,

    -- VOC/CO2 ratio (ventilation indicator)
    tvoc / NULLIF(co2, 0) AS tvoc_co2_ratio

FROM air_quality_readings a
LEFT JOIN outdoor_air_quality o ON time_bucket('1 hour', a.timestamp) = time_bucket('1 hour', o.timestamp);
```

### 2.3 PMS5003 Sensor-Specific Features

The PMS5003 provides multiple particle size bins that enable derived features.

```sql
-- PMS5003 particle count features
SELECT
    timestamp,

    -- Standard measurements
    pm1_0_standard,
    pm2_5_standard,
    pm10_standard,

    -- Particle count features (per 0.1L)
    particles_03um,
    particles_05um,
    particles_10um,
    particles_25um,
    particles_50um,
    particles_100um,

    -- Derived ratios
    particles_25um / NULLIF(particles_03um, 0) AS coarse_fine_ratio,
    (particles_10um - particles_25um) / NULLIF(particles_10um, 0) AS fine_fraction,

    -- Mass density estimate (particles per ug)
    pm2_5_standard / NULLIF(particles_25um, 0) AS particle_mass_density,

    -- Change detection (spike indicator)
    (pm2_5_standard - LAG(pm2_5_standard, 1) OVER (ORDER BY timestamp)) /
        NULLIF(LAG(pm2_5_standard, 1) OVER (ORDER BY timestamp), 0) AS pm25_pct_change

FROM pms5003_readings;
```

### 2.4 Sensor Calibration Features

Low-cost sensors require calibration adjustments, often using environmental features.

```sql
-- Humidity-corrected PM2.5 (common correction factor)
-- Based on research: PM_corrected = PM_raw / (1 + k * RH / (100 - RH))
SELECT
    timestamp,
    pm25_raw,
    humidity,
    temperature,

    -- Humidity correction (k = 0.23 typical for PMS sensors)
    pm25_raw / (1.0 + 0.23 * humidity / NULLIF(100 - humidity, 0)) AS pm25_humidity_corrected,

    -- Temperature-based density correction
    pm25_raw * (293.15 / (273.15 + temperature)) AS pm25_temp_corrected,

    -- Combined correction
    (pm25_raw / (1.0 + 0.23 * humidity / NULLIF(100 - humidity, 0))) *
    (293.15 / (273.15 + temperature)) AS pm25_fully_corrected

FROM air_quality_readings;
```

---

## 3. Domain-Specific Features: Weather

### 3.1 Atmospheric Stability Features

Weather stability indices help predict air quality dispersion.

```sql
SELECT
    timestamp,
    temperature,
    pressure,
    humidity,
    wind_speed,

    -- Pressure gradient (3-hour change)
    pressure - LAG(pressure, 3) OVER (ORDER BY timestamp) AS pressure_gradient_3h,

    -- Temperature gradient (vertical stability proxy)
    temperature - LAG(temperature, 1) OVER (ORDER BY timestamp) AS temp_gradient_1h,

    -- Mixing height proxy (simplified)
    -- Higher temps + lower pressure = better mixing
    temperature * 100 / NULLIF(pressure, 0) AS mixing_index,

    -- Wind chill / heat index
    CASE
        WHEN temperature < 10 AND wind_speed > 3 THEN
            13.12 + 0.6215 * temperature -
            11.37 * POWER(wind_speed, 0.16) +
            0.3965 * temperature * POWER(wind_speed, 0.16)
        ELSE temperature
    END AS feels_like_temp,

    -- Dewpoint (Magnus formula)
    temperature - ((100 - humidity) / 5.0) AS dewpoint_simple,

    -- Vapor pressure deficit (plant stress indicator)
    0.6108 * EXP(17.27 * temperature / (temperature + 237.3)) *
    (1 - humidity / 100.0) AS vpd

FROM weather_readings;
```

### 3.2 NWS Forecast Features

Features derived from NWS gridpoint forecasts for prediction models.

```sql
-- NWS forecast features
SELECT
    reference_time,
    valid_time,

    -- Temporal features
    EXTRACT(EPOCH FROM valid_time - reference_time) / 3600 AS forecast_lead_hours,

    -- Raw forecast values
    temperature_value,
    wind_speed_value,
    wind_direction_value,
    probability_of_precipitation,
    relative_humidity,

    -- Wind components (for vector operations)
    wind_speed_value * SIN(RADIANS(wind_direction_value)) AS wind_u,
    wind_speed_value * COS(RADIANS(wind_direction_value)) AS wind_v,

    -- Forecast uncertainty (from quantiles if available)
    temperature_max_value - temperature_min_value AS temperature_spread,

    -- Frontal activity indicator (pressure + wind change)
    ABS(pressure - LAG(pressure, 6) OVER (ORDER BY valid_time)) +
    ABS(wind_speed_value - LAG(wind_speed_value, 6) OVER (ORDER BY valid_time))
        AS frontal_activity_index

FROM nws_forecast_readings;
```

### 3.3 Diurnal Pattern Features

Environmental data shows strong diurnal (daily) patterns that should be explicitly modeled.

```sql
-- Diurnal pattern extraction
WITH hourly_normals AS (
    SELECT
        EXTRACT(HOUR FROM timestamp) AS hour,
        EXTRACT(MONTH FROM timestamp) AS month,
        AVG(temperature) AS temp_normal,
        AVG(pm25) AS pm25_normal,
        STDDEV(temperature) AS temp_std,
        STDDEV(pm25) AS pm25_std
    FROM combined_readings
    WHERE timestamp >= NOW() - INTERVAL '1 year'
    GROUP BY hour, month
)
SELECT
    r.timestamp,
    r.temperature,
    r.pm25,

    -- Anomaly from diurnal normal
    r.temperature - n.temp_normal AS temp_diurnal_anomaly,
    r.pm25 - n.pm25_normal AS pm25_diurnal_anomaly,

    -- Z-score relative to diurnal pattern
    (r.temperature - n.temp_normal) / NULLIF(n.temp_std, 0) AS temp_diurnal_zscore,
    (r.pm25 - n.pm25_normal) / NULLIF(n.pm25_std, 0) AS pm25_diurnal_zscore,

    -- Phase in diurnal cycle (0-1)
    EXTRACT(HOUR FROM r.timestamp) / 24.0 AS diurnal_phase

FROM combined_readings r
JOIN hourly_normals n ON
    EXTRACT(HOUR FROM r.timestamp) = n.hour AND
    EXTRACT(MONTH FROM r.timestamp) = n.month;
```

---

## 4. Feature Engineering Libraries and Tools

### 4.1 Automated Feature Engineering

#### tsfresh (Python)

**Capabilities:**
- Extracts 1,200+ features automatically
- Built-in statistical hypothesis testing for feature selection
- Integrates with scikit-learn pipelines

**Best For:** Exploratory feature engineering, finding unexpected patterns

**Limitations for Edge:**
- Heavy computation (not suitable for real-time)
- Python dependency
- Generates too many features for constrained environments

```python
# Example tsfresh usage for offline feature discovery
from tsfresh import extract_features
from tsfresh.feature_selection.relevance import calculate_relevance_table

# Extract features
features = extract_features(df, column_id="sensor_id", column_sort="timestamp")

# Select relevant features
relevance = calculate_relevance_table(features, y)
relevant_features = relevance[relevance.p_value < 0.05].feature.tolist()
```

**Recommendation:** Use tsfresh for initial feature discovery, then implement selected features manually in SQL/Rust.

#### Featuretools (Python)

**Capabilities:**
- Deep Feature Synthesis across related tables
- Automated aggregation and transformation primitives
- Entity relationship modeling

**Best For:** Multi-table feature engineering, automated feature discovery

**Edge Consideration:** Not suitable for real-time; use for offline feature template generation.

### 4.2 Recommended Approach for NDP

Given the edge constraints (Raspberry Pi, limited RAM), we recommend a **hybrid approach**:

| Phase | Tool | Purpose |
|-------|------|---------|
| Discovery | tsfresh/featuretools | Identify valuable features offline |
| Production | TimescaleDB continuous aggregates | Compute features in real-time |
| Training | Polars (Rust) | High-performance batch feature generation |
| Inference | SQL views + Redis cache | Low-latency feature serving |

---

## 5. Feature Selection for Resource-Constrained Environments

### 5.1 Why Feature Selection Matters on Edge

- **Memory**: Each feature adds to inference memory footprint
- **Latency**: More features = longer computation time
- **Storage**: Feature storage costs scale with feature count
- **Overfitting**: Fewer features often generalize better

**Target:** 15-30 features for edge inference (vs. 100+ for cloud)

### 5.2 Feature Selection Methods

#### Embedded Methods (Recommended for Edge)

Embedded methods perform selection during model training, providing efficiency and accuracy.

```python
# L1 Regularization (Lasso) - forces sparse feature weights
from sklearn.linear_model import LassoCV

lasso = LassoCV(cv=5)
lasso.fit(X_train, y_train)
selected_features = X_train.columns[lasso.coef_ != 0].tolist()
```

```python
# Tree-based feature importance
from sklearn.ensemble import RandomForestRegressor

rf = RandomForestRegressor(n_estimators=100)
rf.fit(X_train, y_train)

# Select top-k features
importance = pd.Series(rf.feature_importances_, index=X_train.columns)
top_features = importance.nlargest(20).index.tolist()
```

#### Filter Methods (Fast Pre-screening)

```python
# Correlation-based filtering
correlation_matrix = X_train.corr()

# Remove highly correlated features (keep one)
highly_correlated = set()
for i in range(len(correlation_matrix.columns)):
    for j in range(i):
        if abs(correlation_matrix.iloc[i, j]) > 0.95:
            highly_correlated.add(correlation_matrix.columns[i])

X_filtered = X_train.drop(columns=highly_correlated)
```

### 5.3 Recommended Feature Selection Pipeline

```
1. Generate candidate features (50-100)
        |
        v
2. Remove highly correlated features (>0.95)
        |
        v
3. Apply tree-based importance ranking
        |
        v
4. Select top 20-30 features
        |
        v
5. Validate on held-out data
        |
        v
6. Implement selected features in production SQL
```

### 5.4 Ensemble Feature Selection for IoT

Recent research shows ensemble methods provide robust selection for resource-constrained environments:

```python
# Ensemble feature selection (7 filter methods)
from sklearn.feature_selection import (
    mutual_info_regression, f_regression,
    SelectKBest, VarianceThreshold
)

# Run multiple selection methods
scores = {}
scores['mutual_info'] = mutual_info_regression(X, y)
scores['f_regression'] = f_regression(X, y)[0]
scores['variance'] = VarianceThreshold().fit(X).variances_

# Aggregate rankings
rankings = pd.DataFrame(scores).rank(ascending=False)
consensus_rank = rankings.mean(axis=1)
final_features = consensus_rank.nsmallest(20).index.tolist()
```

---

## 6. Online vs. Batch Feature Computation

### 6.1 Computation Strategy Decision Matrix

| Feature Type | Computation | Update Frequency | Implementation |
|-------------|-------------|------------------|----------------|
| Lag features | Online | Per-reading | SQL window function |
| Rolling mean/std | Near real-time | 10 min | Continuous aggregate |
| Seasonal decomposition | Batch | Daily | Cron job |
| AQI calculation | Online | Per-reading | SQL function |
| Trend features | Near real-time | 10 min | Continuous aggregate |
| Cross-stream correlation | Batch | Hourly | Materialized view |
| Sensor calibration | Online | Per-reading | SQL function |

### 6.2 TimescaleDB Continuous Aggregates (Primary Approach)

**Architecture:**
```
Raw Data (hypertable)
        |
        v
Continuous Aggregate (10-min refresh)
        |
        v
Real-time Query (merges materialized + recent raw)
```

**Implementation:**
```sql
-- Create continuous aggregate for hourly features
CREATE MATERIALIZED VIEW features_hourly
WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS
SELECT
    time_bucket('1 hour', timestamp) AS bucket,
    location_id,

    -- Current values
    FIRST(pm25, timestamp) AS pm25_first,
    LAST(pm25, timestamp) AS pm25_last,
    AVG(pm25) AS pm25_mean,
    STDDEV(pm25) AS pm25_std,
    MAX(pm25) AS pm25_max,
    MIN(pm25) AS pm25_min,
    COUNT(*) AS reading_count,

    -- Temperature features
    AVG(temperature) AS temp_mean,
    AVG(humidity) AS humidity_mean

FROM air_quality_readings
GROUP BY bucket, location_id;

-- Auto-refresh policy (every 10 minutes, covers last 4 hours)
SELECT add_continuous_aggregate_policy('features_hourly',
    start_offset => INTERVAL '4 hours',
    end_offset => INTERVAL '10 minutes',
    schedule_interval => INTERVAL '10 minutes');

-- Real-time query (merges materialized + recent)
SELECT * FROM features_hourly
WHERE bucket >= NOW() - INTERVAL '7 days';
```

**Benefits:**
- Automatic refresh in background
- Real-time aggregation option (merges unmaterialized data)
- Compression support (80-95% storage reduction)
- SQL-native (no external processing)

### 6.3 Hierarchical Continuous Aggregates

For multi-scale features, stack aggregates:

```sql
-- Hourly aggregate (base)
CREATE MATERIALIZED VIEW features_hourly
WITH (timescaledb.continuous) AS
SELECT time_bucket('1 hour', timestamp) AS bucket, ...
FROM air_quality_readings
GROUP BY bucket;

-- Daily aggregate (on top of hourly)
CREATE MATERIALIZED VIEW features_daily
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 day', bucket) AS bucket,
    AVG(pm25_mean) AS pm25_daily_mean,
    MAX(pm25_max) AS pm25_daily_max,
    SUM(reading_count) AS daily_readings
FROM features_hourly
GROUP BY time_bucket('1 day', bucket);

-- Monthly aggregate (on top of daily)
CREATE MATERIALIZED VIEW features_monthly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 month', bucket) AS bucket,
    AVG(pm25_daily_mean) AS pm25_monthly_mean
FROM features_daily
GROUP BY time_bucket('1 month', bucket);
```

### 6.4 Batch Feature Computation (Rust/Polars)

For complex features that cannot be expressed in SQL:

```rust
use polars::prelude::*;

fn compute_advanced_features(df: LazyFrame) -> Result<LazyFrame, PolarsError> {
    df
        // Exponential moving average (not available in SQL)
        .with_columns([
            col("pm25")
                .ewm_mean(EWMOptions {
                    alpha: 0.1,
                    ..Default::default()
                })
                .alias("pm25_ema_10"),
        ])
        // Fourier components (seasonality)
        .with_columns([
            (lit(2.0) * lit(std::f64::consts::PI) * col("hour") / lit(24.0))
                .sin()
                .alias("hour_sin"),
            (lit(2.0) * lit(std::f64::consts::PI) * col("hour") / lit(24.0))
                .cos()
                .alias("hour_cos"),
        ])
        // Custom transformations
        .with_columns([
            when(col("pm25").gt(lit(35.0)))
                .then(lit(1))
                .otherwise(lit(0))
                .alias("pm25_above_moderate"),
        ])
}
```

**Schedule:** Run batch computation daily at 3 AM via cron.

---

## 7. Feature Store Considerations

### 7.1 Feature Store Options

| Feature Store | Deployment | Edge Suitability | Notes |
|---------------|------------|------------------|-------|
| **Feast** | Self-hosted, Cloud | Limited | Python-based, requires Redis/DynamoDB |
| **Tecton** | Managed | No | Enterprise, cloud-only |
| **Hopsworks** | Self-hosted, Cloud | Limited | Heavy infrastructure |
| **Custom (TimescaleDB + Redis)** | Self-hosted | Yes | Recommended for NDP |

### 7.2 Feast Evaluation for Edge

**Feast Architecture:**
- **Offline Store**: Historical features for training (DuckDB, Parquet)
- **Online Store**: Low-latency features for inference (Redis, SQLite)
- **Feature Server**: REST API for serving

**Edge Limitations:**
- Requires Python runtime
- Online store options (Redis/DynamoDB) add infrastructure
- Feature server adds HTTP overhead

**Recommendation:** For Raspberry Pi, use a **custom lightweight feature store**:

```
TimescaleDB (offline + online)
        |
        v
Redis Cache (optional, for <10ms latency)
        |
        v
Rust Feature Server (gRPC or direct library call)
```

### 7.3 Lightweight Custom Feature Store

**Architecture:**

```rust
// Feature registry (define features)
struct FeatureDefinition {
    name: String,
    sql_query: String,
    refresh_interval: Duration,
    ttl: Option<Duration>,
}

// Feature store implementation
struct LightweightFeatureStore {
    postgres: Pool<Postgres>,
    redis: Option<redis::Client>,
    features: Vec<FeatureDefinition>,
}

impl LightweightFeatureStore {
    async fn get_features(&self, location_id: &str) -> Result<FeatureVector> {
        // Try Redis cache first
        if let Some(redis) = &self.redis {
            if let Some(cached) = self.get_cached(redis, location_id).await? {
                return Ok(cached);
            }
        }

        // Fallback to TimescaleDB
        let features = self.query_timescale(location_id).await?;

        // Cache for future requests
        if let Some(redis) = &self.redis {
            self.cache_features(redis, location_id, &features).await?;
        }

        Ok(features)
    }
}
```

**Benefits:**
- No Python dependency
- Direct database access (low latency)
- Optional Redis for hot features
- Minimal memory footprint

---

## 8. Recommended Feature Set for NDP

### 8.1 Core Feature Set (20 features)

Based on domain expertise and feature importance analysis:

| Feature | Category | Computation | Priority |
|---------|----------|-------------|----------|
| `pm25_lag_1h` | Lag | Online | High |
| `pm25_lag_6h` | Lag | Online | High |
| `pm25_lag_24h` | Lag | Online | High |
| `pm25_mean_4h` | Rolling | Continuous aggregate | High |
| `pm25_std_4h` | Rolling | Continuous aggregate | High |
| `pm25_mean_24h` | Rolling | Continuous aggregate | Medium |
| `pm25_trend_4h` | Trend | Continuous aggregate | High |
| `pm25_diff_1h` | Differencing | Online | Medium |
| `temp_current` | Raw | Online | High |
| `humidity_current` | Raw | Online | High |
| `pressure_gradient_3h` | Weather | Continuous aggregate | Medium |
| `wind_speed_mean_4h` | Weather | Continuous aggregate | Medium |
| `hour_sin` | Calendar | Online | High |
| `hour_cos` | Calendar | Online | High |
| `dow_sin` | Calendar | Online | Medium |
| `dow_cos` | Calendar | Online | Medium |
| `is_weekend` | Calendar | Online | Low |
| `pm25_outdoor_mean_4h` | Cross-stream | Continuous aggregate | High |
| `indoor_outdoor_ratio` | Domain | Online | High |
| `aqi_category` | Domain | Online | Medium |

### 8.2 TimescaleDB Implementation

```sql
-- Master feature view combining all feature sources
CREATE MATERIALIZED VIEW ml_features
WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS
SELECT
    time_bucket('1 hour', a.timestamp) AS bucket,
    a.location_id,

    -- Lag features (computed at query time via window)
    a.pm25 AS pm25_current,

    -- Rolling features
    AVG(a.pm25) AS pm25_mean_1h,
    STDDEV(a.pm25) AS pm25_std_1h,
    MAX(a.pm25) AS pm25_max_1h,
    MIN(a.pm25) AS pm25_min_1h,

    -- Raw environmental
    AVG(a.temperature) AS temp_mean,
    AVG(a.humidity) AS humidity_mean,

    -- Time features
    EXTRACT(HOUR FROM time_bucket('1 hour', a.timestamp)) AS hour_of_day,
    EXTRACT(DOW FROM time_bucket('1 hour', a.timestamp)) AS day_of_week,

    -- Reading quality
    COUNT(*) AS reading_count

FROM air_quality_readings a
GROUP BY bucket, a.location_id;

-- Refresh every 10 minutes
SELECT add_continuous_aggregate_policy('ml_features',
    start_offset => INTERVAL '4 hours',
    end_offset => INTERVAL '10 minutes',
    schedule_interval => INTERVAL '10 minutes');

-- Query with computed lag and calendar features
CREATE VIEW inference_features AS
SELECT
    bucket,
    location_id,
    pm25_current,
    pm25_mean_1h,
    pm25_std_1h,

    -- Lag features (using window over continuous aggregate)
    LAG(pm25_mean_1h, 1) OVER w AS pm25_lag_1h,
    LAG(pm25_mean_1h, 6) OVER w AS pm25_lag_6h,
    LAG(pm25_mean_1h, 24) OVER w AS pm25_lag_24h,

    -- Trend (approximation via difference)
    pm25_mean_1h - LAG(pm25_mean_1h, 4) OVER w AS pm25_trend_4h,

    -- Differencing
    pm25_mean_1h - LAG(pm25_mean_1h, 1) OVER w AS pm25_diff_1h,

    -- Calendar features (cyclical)
    SIN(2 * PI() * hour_of_day / 24) AS hour_sin,
    COS(2 * PI() * hour_of_day / 24) AS hour_cos,
    SIN(2 * PI() * day_of_week / 7) AS dow_sin,
    COS(2 * PI() * day_of_week / 7) AS dow_cos,

    -- Binary calendar
    CASE WHEN day_of_week IN (0, 6) THEN 1 ELSE 0 END AS is_weekend,

    -- Environmental
    temp_mean,
    humidity_mean

FROM ml_features
WINDOW w AS (PARTITION BY location_id ORDER BY bucket);
```

---

## 9. Implementation Roadmap

### Phase 1: Foundation (Week 1-2)
- [ ] Create `ml_features` continuous aggregate
- [ ] Implement `inference_features` view
- [ ] Validate feature computation on test data
- [ ] Benchmark query performance

### Phase 2: Feature Validation (Week 3-4)
- [ ] Export historical features to Parquet
- [ ] Run feature importance analysis (tsfresh/sklearn)
- [ ] Prune low-importance features
- [ ] Document final feature set

### Phase 3: Feature Serving (Week 5-6)
- [ ] Implement Rust feature retrieval
- [ ] Add Redis caching (optional)
- [ ] Create feature monitoring dashboard
- [ ] Document feature API

### Phase 4: ML Integration (Week 7-8)
- [ ] Connect features to ruv-FANN training pipeline
- [ ] Implement feature drift detection
- [ ] Setup automated retraining triggers
- [ ] End-to-end validation

---

## 10. References

### Project Documentation
- [ML Feature Engineering Research](/workspaces/neural-data-platform/product/research/Silver/ml-feature-engineering.md)
- [TimescaleDB Specification](/workspaces/neural-data-platform/product/features/dp-003/specification/)
- [Air Quality Domain Spec](/workspaces/neural-data-platform/product/research/08-air-quality-domain-spec.md)

### External Sources
- [Practical Guide for Feature Engineering of Time Series Data](https://dotdata.com/blog/practical-guide-for-feature-engineering-of-time-series-data/)
- [Advanced Feature Engineering for Time Series Data](https://medium.com/@rahulholla1/advanced-feature-engineering-for-time-series-data-5f00e3a8ad29)
- [Feature Engineering for Time-Series Data](https://www.statsig.com/perspectives/feature-engineering-timeseries)
- [TimescaleDB Continuous Aggregates](https://docs.timescale.com/getting-started/latest/aggregation/)
- [Real-Time Analytics for Time Series: Continuous Aggregates](https://www.tigerdata.com/blog/real-time-analytics-for-time-series-continuous-aggregates)
- [tsfresh Documentation](https://tsfresh.readthedocs.io/en/latest/)
- [Featuretools Documentation](https://featuretools.alteryx.com/en/stable/guides/time_series.html)
- [MSTL: Multi-Seasonal Time Series Decomposition](https://arxiv.org/abs/2107.13462)
- [Time Series Decomposition](https://otexts.com/fpp2/decomposition.html)
- [Complex Seasonality](https://otexts.com/fpp2/complexseasonality.html)
- [Feast Feature Store](https://docs.feast.dev)
- [Batch vs Real-time Feature Computation](https://apxml.com/courses/feature-stores-for-ml/chapter-2-advanced-feature-engineering-computation/batch-real-time-computation)
- [What is Real-time Feature Engineering?](https://quix.io/blog/what-is-real-time-featuring-engineering)
- [Deep Learning with Raspberry Pi](https://qengineering.eu/deep-learning-with-raspberry-pi-and-alternatives.html)
- [Efficient CNNs on Raspberry Pi](https://link.springer.com/article/10.1007/s11554-023-01271-1)
- [Deep-learning Architecture for PM2.5 Prediction](https://www.sciencedirect.com/science/article/pii/S2666498424000140)
- [Air Quality Prediction by Machine Learning Models](https://www.sciencedirect.com/science/article/pii/S004565352301785X)
- [Ensemble Feature Selection for Lightweight IDS](https://www.mdpi.com/1999-5903/16/10/368)
- [Optimizing ML Models for Resource-Constrained Embedded Devices](https://www.wevolver.com/article/optimizing-machine-learning-models-for-resource-constrained-embedded-devices)

---

**Document Version**: 1.0
**Last Updated**: 2026-02-02
**Author**: Research Agent
**Status**: Complete
