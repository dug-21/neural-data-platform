# Weather Data Source Comparison: NWS vs OpenWeatherMap

**Analysis Date**: 2025-12-23
**Analyst**: Code Analyzer Agent
**Purpose**: Determine canonical source for each overlapping metric in Silver layer

---

## Executive Summary

The Neural Data Platform ingests outdoor weather data from two sources:
- **NWS (National Weather Service)**: Government station KSGJ, 5-minute polling, hourly updates
- **OWM (OpenWeatherMap)**: Commercial API, 10-minute polling

**Key Finding**: NWS should be the **primary canonical source** for most overlapping metrics due to:
- Official government station data (higher reliability)
- More comprehensive field set (16 fields vs 10)
- Better granularity (wind chill, heat index, dewpoint)
- Longer retention (365 days vs 90 days)
- Consistent unit standards (SI units)

---

## Data Source Overview

| Attribute | NWS | OpenWeatherMap |
|-----------|-----|----------------|
| **Stream ID** | `nws-observations` | `outdoor-weather` |
| **Update Frequency** | Every 5 min (checks), hourly actual | Every 10 min |
| **Location** | KSGJ station (29.95838, -81.30878) | Lat/Lon: 29.95838, -81.30878 |
| **Total Fields** | 16 | 10 |
| **Retention** | 365 days | 90 days |
| **Compression** | After 7 days | After 7 days |
| **API Type** | Government (free) | Commercial (API key required) |
| **Data Format** | GeoJSON | JSON |

---

## Field-by-Field Comparison

### 1. Temperature

| Aspect | NWS | OWM | Winner |
|--------|-----|-----|---------|
| **Field Name** | `temperature` | `temperature` | ✓ Match |
| **Unit** | Celsius | Celsius | ✓ Match |
| **Range** | [-50.0, 60.0] | [-50.0, 60.0] | ✓ Match |
| **Nullable** | true | false | **NWS** (more realistic) |
| **Source Path** | `properties.temperature.value` | `main.temp` | - |

**Recommendation**: **NWS primary**
- Nullable field more realistic (sensors can fail)
- Official station measurement
- OWM can serve as fallback when NWS unavailable

**Unit Conversion**: None required

---

### 2. Humidity

| Aspect | NWS | OWM | Winner |
|--------|-----|-----|---------|
| **Field Name** | `relative_humidity` | `humidity` | - |
| **Unit** | percent | percent | ✓ Match |
| **Range** | [0.0, 100.0] | [0.0, 100.0] | ✓ Match |
| **Nullable** | true | true | ✓ Match |
| **Source Path** | `properties.relativeHumidity.value` | `main.humidity` | - |

**Recommendation**: **NWS primary**
- Explicit "relative_humidity" naming is clearer
- Both measure same physical quantity
- OWM as fallback

**Unit Conversion**: None required

---

### 3. Wind Speed

| Aspect | NWS | OWM | Winner |
|--------|-----|-----|---------|
| **Field Name** | `wind_speed` | `wind_speed` | ✓ Match |
| **Unit** | **km/h** | **m/s** | - |
| **Range** | [0.0, 300.0] | [0.0, 100.0] | **NWS** (wider range) |
| **Nullable** | true | true | ✓ Match |
| **Source Path** | `properties.windSpeed.value` | `wind.speed` | - |

**Recommendation**: **NWS primary**
- Wider range allows for extreme weather
- Official station measurement
- OWM as fallback

**Unit Conversion**: **REQUIRED**
```
1 m/s = 3.6 km/h
1 km/h = 0.277778 m/s

To convert OWM to NWS standard:
wind_speed_kmh = wind_speed_ms * 3.6
```

---

### 4. Wind Direction

| Aspect | NWS | OWM | Winner |
|--------|-----|-----|---------|
| **Field Name** | `wind_direction` | `wind_deg` | - |
| **Unit** | degrees | degrees | ✓ Match |
| **Range** | [0.0, 360.0] | [0.0, 360.0] | ✓ Match |
| **Nullable** | true | true | ✓ Match |
| **Source Path** | `properties.windDirection.value` | `wind.deg` | - |

**Recommendation**: **NWS primary**
- More descriptive field name
- Same measurement methodology
- OWM as fallback

**Unit Conversion**: None required

---

### 5. Pressure

| Aspect | NWS | OWM | Winner |
|--------|-----|-----|---------|
| **Field Name** | `barometric_pressure` | `pressure` | - |
| **Unit** | **Pa (Pascals)** | **hPa (hectopascals)** | - |
| **Range** | [80000.0, 110000.0] Pa | [800.0, 1200.0] hPa | ✓ Equivalent |
| **Nullable** | true | true | ✓ Match |
| **Additional** | Also has `sea_level_pressure` | N/A | **NWS** |
| **Source Path** | `properties.barometricPressure.value` | `main.pressure` | - |

**Recommendation**: **NWS primary**
- SI unit (Pascals) is more standard
- Provides both barometric AND sea-level pressure
- More precise range specification

**Unit Conversion**: **REQUIRED**
```
1 hPa = 100 Pa
1 Pa = 0.01 hPa

To convert OWM to NWS standard:
pressure_pa = pressure_hpa * 100
```

---

### 6. Visibility

| Aspect | NWS | OWM | Winner |
|--------|-----|-----|---------|
| **Field Name** | `visibility` | `visibility` | ✓ Match |
| **Unit** | meters | meters | ✓ Match |
| **Range** | [0.0, 50000.0] | [0.0, 50000.0] | ✓ Match |
| **Nullable** | true | true | ✓ Match |
| **Source Path** | `properties.visibility.value` | `visibility` | - |

**Recommendation**: **NWS primary**
- Identical specification
- Official station measurement more reliable
- OWM as fallback

**Unit Conversion**: None required

---

### 7. Wind Gust

| Aspect | NWS | OWM | Winner |
|--------|-----|-----|---------|
| **Field Name** | `wind_gust` | `wind_gust` | ✓ Match |
| **Unit** | **km/h** | **m/s** | - |
| **Range** | [0.0, 400.0] | [0.0, 150.0] | **NWS** (wider range) |
| **Nullable** | true | true | ✓ Match |
| **Source Path** | `properties.windGust.value` | `wind.gust` | - |

**Recommendation**: **NWS primary**
- Wider range for extreme weather events
- Same unit conversion as wind_speed
- OWM as fallback

**Unit Conversion**: **REQUIRED**
```
wind_gust_kmh = wind_gust_ms * 3.6
```

---

## Unique Fields by Source

### NWS-Only Fields (9 fields)

| Field | Unit | Description | Value for Silver Layer |
|-------|------|-------------|------------------------|
| `dewpoint` | Celsius | Dew point temperature | **HIGH** - Critical for comfort/HVAC |
| `wind_chill` | Celsius | Wind chill temperature | **HIGH** - Important for weather perception |
| `heat_index` | Celsius | Heat index temperature | **HIGH** - Important for weather perception |
| `sea_level_pressure` | Pa | Pressure normalized to sea level | **MEDIUM** - Useful for weather patterns |
| `max_temperature_24h` | Celsius | 24-hour maximum | **MEDIUM** - Useful for daily summaries |
| `min_temperature_24h` | Celsius | 24-hour minimum | **MEDIUM** - Useful for daily summaries |
| `precipitation_1h` | meters | 1-hour precipitation | **HIGH** - Important for rain tracking |
| `precipitation_3h` | meters | 3-hour precipitation | **MEDIUM** - Useful for trend analysis |
| `precipitation_6h` | meters | 6-hour precipitation | **MEDIUM** - Useful for trend analysis |

**Analysis**: NWS provides critical weather comfort metrics (dewpoint, wind chill, heat index) and precipitation data that OWM lacks. These are essential for a comprehensive weather data platform.

### OWM-Only Fields (3 fields)

| Field | Unit | Description | Value for Silver Layer |
|-------|------|-------------|------------------------|
| `feels_like` | Celsius | Apparent temperature | **MEDIUM** - Similar to wind_chill/heat_index |
| `clouds` | percent | Cloud coverage | **MEDIUM** - Useful for solar/visibility |
| `rain_1h` | mm | 1-hour rain | **LOW** - Overlaps with NWS precipitation |
| `snow_1h` | mm | 1-hour snow | **MEDIUM** - NWS doesn't separate snow |

**Analysis**: OWM's unique fields are less critical. `feels_like` overlaps with NWS's `wind_chill`/`heat_index`. `clouds` is useful but not essential. `snow_1h` is the only truly unique valuable field.

---

## Unit Conversion Requirements for Silver Layer

### Required Conversions

| Metric | Source | Source Unit | Target Unit | Conversion Formula |
|--------|--------|-------------|-------------|-------------------|
| Wind Speed | OWM | m/s | km/h | `value * 3.6` |
| Wind Gust | OWM | m/s | km/h | `value * 3.6` |
| Pressure | OWM | hPa | Pa | `value * 100` |
| Precipitation | OWM | mm | meters | `value / 1000` |

### No Conversion Needed

- Temperature (both Celsius)
- Humidity (both percent)
- Wind Direction (both degrees)
- Visibility (both meters)

---

## Update Frequency Analysis

| Source | Poll Interval | Actual Update Frequency | Freshness |
|--------|---------------|------------------------|-----------|
| **NWS** | 5 minutes | ~1 hour (government station) | Lower |
| **OWM** | 10 minutes | ~10 minutes (model-based) | Higher |

**Implication**:
- **NWS**: More stable, less frequent updates from physical station
- **OWM**: More frequent updates but model-interpolated between stations
- **Silver Layer Strategy**: Use NWS as authoritative, OWM for high-frequency monitoring

---

## Data Quality & Reliability

### NWS Strengths
1. **Official Source**: Government-operated weather station
2. **Physical Station**: Direct measurements (KSGJ)
3. **Comprehensive**: 16 fields including comfort indices
4. **Long Retention**: 365 days
5. **Free**: No API costs or rate limits
6. **Standardized**: Follows NOAA standards

### NWS Weaknesses
1. **Update Frequency**: Only hourly actual updates
2. **Nullable Everything**: All fields can be null (sensor failures)
3. **Station-Specific**: Single point measurement

### OWM Strengths
1. **Update Frequency**: 10-minute updates
2. **Always Available**: Commercial SLA
3. **Feels Like**: User-friendly apparent temperature
4. **Cloud Coverage**: Additional atmospheric data

### OWM Weaknesses
1. **Model-Based**: Interpolated between stations
2. **Commercial**: API key required, potential costs
3. **Limited Fields**: Only 10 fields
4. **Shorter Retention**: 90 days
5. **Less Precise**: Missing critical metrics (dewpoint, wind chill, precipitation)

---

## Recommendations for Silver Layer

### Primary Source Selection by Metric

| Metric | Primary Source | Fallback Source | Rationale |
|--------|---------------|-----------------|-----------|
| **Temperature** | NWS | OWM | Official station, nullable realistic |
| **Humidity** | NWS | OWM | Official measurement |
| **Wind Speed** | NWS | OWM | Wider range, official |
| **Wind Direction** | NWS | OWM | Official measurement |
| **Pressure** | NWS | OWM | More precise unit, has sea-level |
| **Visibility** | NWS | OWM | Official measurement |
| **Wind Gust** | NWS | OWM | Wider range for extremes |
| **Dewpoint** | NWS | - | NWS-only |
| **Wind Chill** | NWS | OWM feels_like | NWS more specific |
| **Heat Index** | NWS | OWM feels_like | NWS more specific |
| **Precipitation** | NWS | - | NWS-only (1h, 3h, 6h) |
| **Clouds** | - | OWM | OWM-only |
| **Snow** | - | OWM | OWM-only |

### Silver Layer Schema Design

**Canonical Weather Table**: `weather.observations`

```sql
CREATE TABLE weather.observations (
    timestamp TIMESTAMPTZ NOT NULL,
    location_id TEXT NOT NULL,

    -- Core measurements (NWS primary)
    temperature REAL,
    dewpoint REAL,
    relative_humidity REAL,

    -- Wind (NWS primary, km/h standard)
    wind_speed REAL,           -- km/h
    wind_direction REAL,       -- degrees
    wind_gust REAL,            -- km/h

    -- Pressure (NWS primary, Pa standard)
    barometric_pressure REAL,  -- Pa
    sea_level_pressure REAL,   -- Pa

    -- Visibility (NWS primary)
    visibility REAL,           -- meters

    -- Comfort indices (NWS primary)
    wind_chill REAL,
    heat_index REAL,
    feels_like REAL,           -- OWM or computed

    -- Precipitation (NWS primary)
    precipitation_1h REAL,     -- meters
    precipitation_3h REAL,     -- meters
    precipitation_6h REAL,     -- meters
    rain_1h REAL,              -- meters (OWM)
    snow_1h REAL,              -- meters (OWM)

    -- Aggregates (NWS primary)
    max_temperature_24h REAL,
    min_temperature_24h REAL,

    -- Atmospheric (OWM only)
    cloud_coverage REAL,       -- percent

    -- Metadata
    primary_source TEXT,       -- 'nws' or 'owm'
    fallback_used BOOLEAN,     -- true if primary unavailable

    PRIMARY KEY (timestamp, location_id)
);
```

### ETL Strategy

1. **Primary Ingestion**: NWS data every 5 minutes
2. **Fallback Ingestion**: OWM data every 10 minutes
3. **Merge Logic**:
   ```
   IF NWS.temperature IS NOT NULL:
       USE NWS.temperature
   ELSE IF OWM.temperature IS NOT NULL:
       USE OWM.temperature (with fallback_used = true)
   ```

4. **Unit Conversion** (in ETL pipeline):
   - Convert OWM wind speeds: `m/s → km/h`
   - Convert OWM pressure: `hPa → Pa`
   - Convert OWM rain/snow: `mm → meters`

5. **Data Quality Flags**:
   - Track which source provided each field
   - Flag when fallback source used
   - Compute deltas between NWS/OWM for validation

---

## Data Validation Strategy

### Cross-Source Validation

When both sources available, validate by comparing:

| Metric | Expected Delta | Action if Exceeded |
|--------|----------------|-------------------|
| Temperature | ±2°C | Log warning, flag for review |
| Humidity | ±10% | Log warning |
| Wind Speed | ±5 km/h | Acceptable (different measurement points) |
| Pressure | ±200 Pa | Log warning |

### Missing Data Handling

| Scenario | Strategy |
|----------|----------|
| NWS null, OWM available | Use OWM with `fallback_used = true` |
| Both null | Store NULL, flag gap for later interpolation |
| OWM only field | Store directly (clouds, snow) |
| NWS only field | No fallback available |

---

## Implementation Priorities

### Phase 1: Core Metrics (High Priority)
- Temperature, humidity, pressure
- Wind speed, wind direction
- Visibility
- **Use**: NWS primary, OWM fallback

### Phase 2: Comfort Indices (High Priority)
- Dewpoint, wind chill, heat index
- **Use**: NWS only (compute feels_like if needed)

### Phase 3: Precipitation (Medium Priority)
- 1h, 3h, 6h precipitation from NWS
- Rain/snow separation from OWM
- **Use**: Both sources, complementary

### Phase 4: Extended Metrics (Low Priority)
- Cloud coverage (OWM only)
- 24h min/max temperature (NWS only)
- **Use**: Single source

---

## Cost & Operational Considerations

| Aspect | NWS | OWM |
|--------|-----|-----|
| **API Cost** | Free | Paid (after limits) |
| **Rate Limits** | None specified | Yes (varies by tier) |
| **SLA** | None | Commercial SLA |
| **Reliability** | High (government) | Very High (commercial) |
| **Support** | Community | Commercial support |

**Recommendation**: Use NWS as primary to minimize API costs and dependency on commercial service. OWM provides valuable fallback and unique fields (clouds, snow).

---

## Future Enhancements

1. **Quality Scoring**: Assign confidence scores to each source
2. **Machine Learning**: Train model to predict which source more accurate
3. **Gap Filling**: Interpolate missing NWS data using OWM patterns
4. **Ensemble Methods**: Average/weighted combination when both available
5. **Additional Sources**: Consider adding Weather Underground, NOAA, etc.

---

## Conclusion

**Canonical Source Hierarchy**:
1. **Primary**: NWS (KSGJ station) - Official, comprehensive, free
2. **Fallback**: OWM - High frequency, commercial reliability
3. **Complementary**: Use both for cloud coverage and precipitation detail

**Key Benefits**:
- Authoritative government data as foundation
- Commercial API as reliable fallback
- Comprehensive field coverage (19 total fields)
- Cost-effective (primary source free)
- Data validation through cross-source comparison

**Next Steps**:
1. Implement ETL pipeline with unit conversions
2. Create TimescaleDB hypertables with proposed schema
3. Build continuous aggregates for hourly/daily rollups
4. Set up data quality monitoring and alerting
5. Create Grafana dashboards showing source reliability

---

## Appendix: Configuration Files

- **NWS Config**: `/workspaces/neural-data-platform/config/base/streams/nws-observations/config.yaml`
- **OWM Config**: `/workspaces/neural-data-platform/config/base/streams/outdoor-weather/config.yaml`

## Appendix: API Documentation

- **NWS API**: https://www.weather.gov/documentation/services-web-api
- **OWM API**: https://openweathermap.org/current

---

**Document Version**: 1.0
**Last Updated**: 2025-12-23
**Review Date**: 2026-01-23
