# Weather Domain Model

## Overview

The weather domain (including air quality) consists of two primary entity types with a critical relationship for analytics.

## Domain Model Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                       WEATHER DOMAIN MODEL                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────┐          ┌─────────────────┐                   │
│  │  OBSERVATIONS   │          │    FORECASTS    │                   │
│  │  (Ground Truth) │          │   (Predictions) │                   │
│  ├─────────────────┤          ├─────────────────┤                   │
│  │ • observation_  │          │ • issue_time    │ ← When forecast   │
│  │   time          │          │ • valid_time    │   was generated   │
│  │ • location      │          │ • lead_time     │ ← valid - issue   │
│  │ • metrics       │          │ • location      │                   │
│  │                 │          │ • metrics       │                   │
│  │                 │          │ • duration      │ ← validity window │
│  └────────┬────────┘          └────────┬────────┘                   │
│           │                            │                            │
│           │     JOIN ON                │                            │
│           │  observation_time =        │                            │
│           │  valid_time + location     │                            │
│           │                            │                            │
│           └──────────┬─────────────────┘                            │
│                      ▼                                              │
│           ┌─────────────────────┐                                   │
│           │  FORECAST ACCURACY  │                                   │
│           ├─────────────────────┤                                   │
│           │ • forecast_value    │                                   │
│           │ • observed_value    │                                   │
│           │ • lead_time         │ ← Key dimension!                  │
│           │ • error             │                                   │
│           │ • metric            │                                   │
│           └─────────────────────┘                                   │
│                      │                                              │
│                      ▼                                              │
│           ┌─────────────────────┐                                   │
│           │   DECISION MODELS   │                                   │
│           ├─────────────────────┤                                   │
│           │ • Window open/close │                                   │
│           │ • Ventilation       │                                   │
│           │ • Activity planning │                                   │
│           └─────────────────────┘                                   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Entity: Observations

**Definition**: Ground truth measurements at a specific time and location.

**Sources**:
- NWS Observations (outdoor weather stations)
- AirGradient sensors (indoor air quality)
- HomeAssistant sensors (indoor temperature/humidity)

**Key Attributes**:
| Attribute | Type | Description |
|-----------|------|-------------|
| observation_time | TIMESTAMPTZ | When measurement was taken |
| location/ndp_id | TEXT | Sensor or station identifier |
| metrics | FLOAT[] | Temperature, humidity, PM2.5, CO2, etc. |

## Entity: Forecasts

**Definition**: Predictions for future conditions, issued at a specific time.

**Sources**:
- NWS Gridpoints Forecast (7-day grid-based forecast)
- NWS Hourly Forecast (48-hour point forecast)

**Key Attributes**:
| Attribute | Type | Description |
|-----------|------|-------------|
| issue_time | TIMESTAMPTZ | When the forecast was generated |
| valid_time | TIMESTAMPTZ | When the prediction applies |
| valid_duration | INTERVAL | How long the prediction is valid (PT1H, PT6H) |
| lead_time | INTERVAL | valid_time - issue_time (computed) |
| location | TEXT | Grid cell or station identifier |
| metrics | FLOAT[] | Predicted temperature, precip prob, etc. |

## The Lead Time Dimension

This is **critical** for forecast evaluation. When NWS issues a forecast at `2026-01-01T06:00Z`:

| valid_time | lead_time | Interpretation |
|------------|-----------|----------------|
| 2026-01-01T07:00Z | 1 hour | Near-term (very accurate) |
| 2026-01-02T06:00Z | 24 hours | Day-ahead (moderately accurate) |
| 2026-01-08T06:00Z | 168 hours | Week-ahead (less accurate) |

### Forecast Updates

The same target time gets updated as new forecasts are issued:

```
Target: valid_time = 2026-01-02T12:00Z

issue_time=2026-01-01T06:00 → lead_time=30h → temp=22°C
issue_time=2026-01-02T06:00 → lead_time=6h  → temp=21°C  (revised)
issue_time=2026-01-02T12:00 → lead_time=0h  → temp=20°C  (now observation)
```

## Derived Entity: Forecast Accuracy

Created by joining forecasts to observations:

```sql
SELECT
    f.valid_time,
    f.lead_time_hours,
    f.temperature_c AS forecast_temp,
    o.temperature_c AS observed_temp,
    ABS(f.temperature_c - o.temperature_c) AS temp_error
FROM forecasts f
JOIN observations o
  ON f.valid_time = o.observation_time
 AND f.location = o.location;
```

**Key Analysis**: "At lead_time=N hours, what's the typical forecast error?"

## Use Cases

### Primary Use Case: Optimal Window Management

**Goal**: Predict when to open/close windows for optimal indoor air quality.

**Required Data**:
1. Outdoor weather forecasts (temperature, humidity, wind, precip)
2. Outdoor air quality forecasts (if available) or observations
3. Indoor air quality observations (PM2.5, CO2, VOC)
4. Indoor temperature/humidity

**Analytics**:
- Forecast accuracy by lead_time (how far ahead can we trust predictions?)
- Indoor/outdoor differential modeling
- Trigger threshold optimization

### Secondary Use Cases

- Activity planning (outdoor exercise when AQI favorable)
- HVAC optimization (pre-conditioning based on forecasts)
- Alert generation (air quality warnings)

## Data Sources Mapped to Domain

| Source | Domain Entity | Notes |
|--------|---------------|-------|
| nws-observations | Observation | Official weather station data |
| nws-gridpoints-forecast | Forecast | 7-day gridded forecast, 60+ metrics |
| nws-forecast-hourly | Forecast | 48-hour point forecast |
| air-quality (AirGradient) | Observation | Indoor air quality |
| outdoor-air-quality (OWM) | Observation | Outdoor AQI |
| homeassistant | Observation | Indoor temp/humidity |
