# Data Exploration Research Synthesis

**Date**: 2025-12-23
**Purpose**: Synthesize research findings to guide Silver layer design decisions

---

## Research Documents Produced

| Document | Purpose |
|----------|---------|
| `bronze-data-inventory.md` | Complete field inventory across 5 streams |
| `dashboard-review.md` | Gap analysis of existing Grafana dashboards |
| `nws-vs-owm-comparison.md` | Weather source comparison and recommendations |
| `airgradient-field-analysis.md` | Indoor sensor field analysis (raw vs calibrated) |
| `exploration-dashboard-recommendations.md` | New dashboard designs for data exploration |

---

## Key Findings

### 1. Data Inventory Summary

**5 Streams, 46+ Unique Fields**

| Stream | Source | Fields | Update Freq | Key Metrics |
|--------|--------|--------|-------------|-------------|
| air-quality | AirGradient MQTT | 7 config, 29 raw | Real-time | pm25, co2, temp, humidity |
| outdoor-weather | OpenWeatherMap | 10 | 10 min | temp, humidity, wind, pressure |
| outdoor-air-quality | OpenWeatherMap | 9 | 10 min | pm2_5, aqi, o3, no2 |
| nws-observations | NWS Station KSGJ | 15 | 5 min | temp, dewpoint, wind, pressure |
| nws-forecast-hourly | NWS API | 8 | 1 hour | temp, wind, precip_prob |

### 2. Critical Issues Discovered

#### Issue 1: Field Name Chaos
```
PM2.5:      pm02 (AirGradient) → pm25 (transformed) vs pm2_5 (OWM)
Temperature: atmp (AirGradient) → temperature vs temperature (OWM/NWS)
Humidity:   rhum (AirGradient) → humidity vs relative_humidity (NWS)
CO2:        rco2 (AirGradient) → co2
```

**Impact**: Dashboard queries use inconsistent names, causing potential misses.

#### Issue 2: Unit Inconsistencies
| Metric | OWM | NWS Obs | NWS Forecast | Recommendation |
|--------|-----|---------|--------------|----------------|
| Wind Speed | m/s | km/h | mph | Convert to m/s (SI) |
| Pressure | hPa | Pa | - | Standardize to hPa |
| Temperature | °C | °C | °F | Store °C, display °F |

#### Issue 3: Data Overlap Without Clear Primary
- Temperature: 3 sources (OWM, NWS obs, NWS forecast)
- Humidity: 3 sources
- Wind: 3 sources
- PM2.5: 2 sources (indoor AirGradient, outdoor OWM)

### 3. Weather Source Comparison (NWS vs OWM)

| Factor | NWS | OWM | Winner |
|--------|-----|-----|--------|
| Data source | Official govt station | Commercial API | NWS |
| Field coverage | 15 fields | 10 fields | NWS |
| Unique metrics | Dewpoint, heat index, wind chill, precip | Feels like, clouds, snow | NWS |
| Update frequency | Hourly (5 min polls) | 10 min | OWM |
| Reliability | High (govt) | Medium (commercial) | NWS |
| Cost | Free, no limits | Free tier limited | NWS |

**Recommendation**: NWS as primary, OWM as supplementary for unique fields (feels_like, clouds).

### 4. AirGradient Field Analysis

**29 fields total, grouped by Silver layer recommendation:**

| Category | Promote to Silver | Keep in Bronze Only |
|----------|-------------------|---------------------|
| Core PM | pm01, pm02, pm10, pm02Compensated | pm*Standard, pm*Count |
| Gas | tvocIndex, noxIndex | tvocRaw, noxRaw |
| Environment | atmp, atmpCompensated, rhum, rhumCompensated | - |
| CO2 | rco2 | - |
| Diagnostic | wifi | boot, boot_count, led_mode |

**Key Insight**: Compensated values (atmpCompensated, pm02Compensated) are more accurate for indoor monitoring. Promote both raw and compensated to Silver for flexibility.

### 5. Dashboard Gaps Identified

**Missing Capabilities:**
1. No data quality/completeness monitoring
2. No multi-source correlation analysis
3. No gap detection or anomaly visualization
4. No temporal resolution comparison
5. No source reliability metrics (NWS vs OWM uptime)
6. All dashboards hardcode 10-minute aggregation

---

## Recommended Exploration Dashboards

### Priority 1: Build These First

| Dashboard | Purpose | Key Decision It Informs |
|-----------|---------|-------------------------|
| **Data Quality & Completeness** | Gap detection, null rates, freshness | Which streams are reliable for Silver? |
| **Weather Source Reliability** | NWS vs OWM accuracy, uptime | Which source should be canonical? |

### Priority 2: Build After Silver ETL

| Dashboard | Purpose | Key Decision It Informs |
|-----------|---------|-------------------------|
| **Indoor-Outdoor Correlation** | Cross-stream analysis | Which features matter for ML? |
| **Anomaly Detection** | Outlier identification | What validation rules for ETL? |
| **Temporal Resolution Analysis** | Aggregation window impact | What continuous aggregates to create? |

---

## Decisions Needed Before Silver Design

### Must Decide Now

| Decision | Options | Data Needed |
|----------|---------|-------------|
| **Canonical weather source** | NWS primary vs OWM primary | Reliability dashboard |
| **Temperature storage unit** | °C (convert on display) vs °F | User preference (decided: °C) |
| **Wind speed unit** | m/s vs mph vs km/h | User preference |
| **Include compensated fields?** | Yes (both) vs No (raw only) | AirGradient analysis |

### Can Decide Later

| Decision | Why Later |
|----------|-----------|
| Raw field retention | After seeing actual query patterns |
| Continuous aggregate windows | After understanding query needs |
| Gap-filling strategy | After seeing actual gap frequency |

---

## Recommended Next Steps

### Step 1: Build Data Quality Dashboard
Deploy the "Data Quality & Completeness" dashboard to answer:
- How complete is each stream?
- Where are the gaps?
- What's the null rate per field?

### Step 2: Build Weather Comparison Dashboard
Deploy "Weather Source Reliability" dashboard to answer:
- Which source has better uptime?
- How much do NWS and OWM diverge?
- When does OWM have data but NWS doesn't (and vice versa)?

### Step 3: Make Canonical Source Decision
Based on dashboard data:
- If NWS reliability > 95%: NWS primary, OWM supplementary
- If NWS has significant gaps: Consider OWM fallback logic

### Step 4: Design Silver Schema
With decisions made, create:
- `silver.indoor_environment` (AirGradient data)
- `silver.outdoor_conditions` (NWS + OWM merged)
- `silver.outdoor_air_quality` (OWM AQ data)
- `silver.weather_forecast` (NWS forecast)

### Step 5: Build ETL with Transforms
- Field name normalization
- Unit conversions
- Primary/fallback source logic

---

## Summary: What You Need to Do

1. **Now**: Build 2 exploration dashboards (Data Quality, Weather Comparison)
2. **Review**: Analyze the data for 1-2 weeks
3. **Decide**: Canonical sources, field selection
4. **Design**: Silver schema with informed decisions
5. **Build**: ETL pipeline

The exploration dashboards ARE the tool to make Silver layer decisions. Build them first, analyze, then design.
