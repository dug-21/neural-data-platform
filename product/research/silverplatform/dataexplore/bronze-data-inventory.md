# Bronze Layer Data Inventory

**Purpose**: Comprehensive reference of all data fields available in Bronze layer Parquet files across 5 streams
**Date**: 2025-12-23
**Scope**: All enabled data streams for Neural Data Platform

---

## Executive Summary

The Bronze layer contains **5 active data streams** with a total of **46 unique fields** across air quality, weather, and forecast data. Key characteristics:

- **Update frequencies**: 1 minute (indoor sensors) to 1 hour (forecasts)
- **Data sources**: MQTT sensors (1), HTTP APIs (4)
- **Retention**: 30-365 days depending on stream
- **Field overlap**: 8 fields appear in multiple streams (temperature, humidity, wind, pressure, precipitation)
- **Naming inconsistencies**: Critical variations in PM and humidity field names

---

## Stream Overview

| Stream ID | Description | Update Frequency | Retention | Fields | Data Source |
|-----------|-------------|------------------|-----------|--------|-------------|
| `air-quality` | Indoor AirGradient sensors | Real-time (MQTT) | 365 days | 7 | MQTT topic |
| `outdoor-weather` | OpenWeatherMap current weather | 10 minutes | 90 days | 11 | HTTP Poll |
| `outdoor-air-quality` | OpenWeatherMap air pollution | 10 minutes | 90 days | 9 | HTTP Poll |
| `nws-observations` | NWS station KSGJ real-time | 5 minutes | 365 days | 16 | HTTP Poll |
| `nws-forecast-hourly` | NWS gridpoint hourly forecast | 1 hour | 30 days | 8 | HTTP Poll |

---

## Field Catalog by Stream

### 1. air-quality (Indoor Sensors)

**Source**: AirGradient sensors via MQTT
**Update**: Real-time (buffer: 1000, batch: 100 records or 5s)
**Location**: Indoor sensors identified by `serialno`

| Field | Type | Unit | Nullable | Description | Notes |
|-------|------|------|----------|-------------|-------|
| `pm25` | float | µg/m³ | NO | Particulate Matter 2.5µm | Primary indoor air quality metric |
| `pm10` | float | µg/m³ | YES | Particulate Matter 10µm | Optional on some sensors |
| `co2` | int | ppm | YES | Carbon Dioxide | Indoor air quality indicator |
| `temperature` | float | celsius | YES | Ambient temperature | Indoor temperature |
| `humidity` | float | percent | YES | Relative humidity | Indoor humidity |
| `tvoc` | int | ppb | YES | Total Volatile Organic Compounds | Indoor pollutant |
| `nox` | int | ppb | YES | Nitrogen Oxides | Indoor pollutant |

**Tags**: `source=mqtt`, `stream_id=air-quality`, `location_id={serialno}`

---

### 2. outdoor-weather (OpenWeatherMap Current)

**Source**: OpenWeatherMap Current Weather API
**Update**: 10 minutes (600s poll interval)
**Location**: Fixed coordinates (29.95838, -81.30878)

| Field | Type | Unit | Nullable | Range | Description |
|-------|------|------|----------|-------|-------------|
| `temperature` | float | celsius | NO | -50 to 60 | Current outdoor temperature |
| `feels_like` | float | celsius | YES | -50 to 60 | Apparent temperature |
| `pressure` | float | hPa | YES | 800 to 1200 | Atmospheric pressure |
| `humidity` | float | percent | YES | 0 to 100 | Outdoor relative humidity |
| `wind_speed` | float | m/s | YES | 0 to 100 | Wind speed |
| `wind_deg` | float | degrees | YES | 0 to 360 | Wind direction |
| `wind_gust` | float | m/s | YES | 0 to 150 | Wind gust speed |
| `clouds` | float | percent | YES | 0 to 100 | Cloud coverage |
| `visibility` | float | meters | YES | 0 to 50000 | Visibility distance |
| `rain_1h` | float | mm | YES | 0 to 500 | Rainfall in last hour |
| `snow_1h` | float | mm | YES | 0 to 500 | Snowfall in last hour |

**Tags**: `source=openweathermap`, `api=current_weather`, `stream_id=outdoor-weather`, `location_id=home`

---

### 3. outdoor-air-quality (OpenWeatherMap Air Pollution)

**Source**: OpenWeatherMap Air Pollution API
**Update**: 10 minutes (600s poll interval)
**Location**: Fixed coordinates (29.95838, -81.30878)

| Field | Type | Unit | Nullable | Range | Description |
|-------|------|------|----------|-------|-------------|
| `aqi` | float | 1-5 scale | NO | 1 to 5 | Air Quality Index (1=Good, 5=Very Poor) |
| `co` | float | µg/m³ | YES | 0 to 50000 | Carbon Monoxide |
| `no` | float | µg/m³ | YES | 0 to 1000 | Nitrogen Monoxide |
| `no2` | float | µg/m³ | YES | 0 to 1000 | Nitrogen Dioxide |
| `o3` | float | µg/m³ | YES | 0 to 1000 | Ozone |
| `so2` | float | µg/m³ | YES | 0 to 1000 | Sulfur Dioxide |
| `pm2_5` | float | µg/m³ | NO | 0 to 1000 | Particulate Matter 2.5µm |
| `pm10` | float | µg/m³ | YES | 0 to 1000 | Particulate Matter 10µm |
| `nh3` | float | µg/m³ | YES | 0 to 200 | Ammonia |

**Tags**: `source=openweathermap`, `api=air_pollution`, `stream_id=outdoor-air-quality`, `location_id=home`

---

### 4. nws-observations (NWS Station KSGJ)

**Source**: National Weather Service Station KSGJ
**Update**: 5 minutes (300s poll interval, NWS updates hourly)
**Location**: KSGJ weather station

| Field | Type | Unit | Nullable | Range | Description |
|-------|------|------|----------|-------|-------------|
| `temperature` | float | celsius | YES | -50 to 60 | Ambient air temperature |
| `dewpoint` | float | celsius | YES | -50 to 60 | Dew point temperature |
| `wind_direction` | float | degrees | YES | 0 to 360 | Wind direction from north |
| `wind_speed` | float | km/h | YES | 0 to 300 | Wind speed |
| `wind_gust` | float | km/h | YES | 0 to 400 | Wind gust speed |
| `barometric_pressure` | float | Pa | YES | 80000 to 110000 | Barometric pressure |
| `sea_level_pressure` | float | Pa | YES | 80000 to 110000 | Sea level pressure |
| `visibility` | float | meters | YES | 0 to 50000 | Visibility distance |
| `max_temperature_24h` | float | celsius | YES | -50 to 60 | 24h max temperature |
| `min_temperature_24h` | float | celsius | YES | -50 to 60 | 24h min temperature |
| `precipitation_1h` | float | meters | YES | 0 to 1 | Precipitation last hour |
| `precipitation_3h` | float | meters | YES | 0 to 1 | Precipitation last 3 hours |
| `precipitation_6h` | float | meters | YES | 0 to 1 | Precipitation last 6 hours |
| `relative_humidity` | float | percent | YES | 0 to 100 | Relative humidity |
| `wind_chill` | float | celsius | YES | -50 to 60 | Wind chill temperature |
| `heat_index` | float | celsius | YES | -50 to 60 | Heat index temperature |

**Tags**: `source=nws`, `api=observations`, `stream_id=nws-observations`, `station_id=KSGJ`, `location_id=ksgj`

**Special**: Uses observation timestamp from API (`properties.timestamp`) not poll time

---

### 5. nws-forecast-hourly (NWS Gridpoint Forecast)

**Source**: National Weather Service Gridpoint Forecast
**Update**: 1 hour (3600s poll interval)
**Location**: JAX grid 79,49 (~156 hour forecast)

| Field | Type | Unit | Nullable | Range | Description |
|-------|------|------|----------|-------|-------------|
| `temperature` | float | fahrenheit | NO | -50 to 130 | Forecast temperature |
| `dewpoint` | float | celsius | YES | -50 to 60 | Forecast dew point |
| `relative_humidity` | float | percent | YES | 0 to 100 | Forecast relative humidity |
| `wind_speed` | float | mph | YES | 0 to 200 | Forecast wind speed |
| `wind_direction` | float | degrees | YES | 0 to 360 | Forecast wind direction |
| `short_forecast` | string | - | YES | - | Brief forecast description |
| `probability_of_precipitation` | float | percent | YES | 0 to 100 | Precipitation probability |
| `forecast_issue_time` | float | epoch_seconds | YES | - | Forecast issue timestamp |

**Tags**: `source=nws`, `api=forecast_hourly`, `stream_id=nws-forecast-hourly`, `grid_office=JAX`, `grid_x=79`, `grid_y=49`, `location_id=ksgj`

**Special**:
- Array iterator parser creates ~156 records per poll (156 hour forecast)
- Buffer capacity: 2500 to handle ~1092 points per poll
- Wind direction converted from cardinal (N, NE, etc.) to degrees via enum mapping
- Wind speed parsed from string format "10 to 15 mph" using regex

---

## Cross-Stream Field Analysis

### Overlapping Fields

Fields that appear in multiple streams (critical for data fusion in Silver layer):

#### Temperature

| Stream | Field Name | Unit | Update Freq | Notes |
|--------|-----------|------|-------------|-------|
| `air-quality` | `temperature` | celsius | Real-time | Indoor |
| `outdoor-weather` | `temperature` | celsius | 10 min | Outdoor current |
| `nws-observations` | `temperature` | celsius | 5 min | Outdoor KSGJ |
| `nws-forecast-hourly` | `temperature` | **fahrenheit** | 1 hour | Forecast |

**Issue**: Unit inconsistency (3 celsius, 1 fahrenheit) - requires conversion in Silver layer

#### Humidity

| Stream | Field Name | Unit | Update Freq | Notes |
|--------|-----------|------|-------------|-------|
| `air-quality` | `humidity` | percent | Real-time | Indoor |
| `outdoor-weather` | `humidity` | percent | 10 min | Outdoor current |
| `nws-observations` | `relative_humidity` | percent | 5 min | Outdoor KSGJ |
| `nws-forecast-hourly` | `relative_humidity` | percent | 1 hour | Forecast |

**Issue**: Field name inconsistency (`humidity` vs `relative_humidity`)

#### Wind Speed

| Stream | Field Name | Unit | Update Freq | Notes |
|--------|-----------|------|-------------|-------|
| `outdoor-weather` | `wind_speed` | m/s | 10 min | Metric |
| `nws-observations` | `wind_speed` | km/h | 5 min | Metric |
| `nws-forecast-hourly` | `wind_speed` | mph | 1 hour | Imperial |

**Issue**: Three different units (m/s, km/h, mph) - requires standardization

#### Wind Direction

| Stream | Field Name | Unit | Update Freq | Notes |
|--------|-----------|------|-------------|-------|
| `outdoor-weather` | `wind_deg` | degrees | 10 min | Numeric |
| `nws-observations` | `wind_direction` | degrees | 5 min | Numeric |
| `nws-forecast-hourly` | `wind_direction` | degrees | 1 hour | Converted from cardinal |

**Issue**: Field name inconsistency (`wind_deg` vs `wind_direction`)

#### Wind Gust

| Stream | Field Name | Unit | Update Freq |
|--------|-----------|------|-------------|
| `outdoor-weather` | `wind_gust` | m/s | 10 min |
| `nws-observations` | `wind_gust` | km/h | 5 min |

**Issue**: Unit inconsistency (m/s vs km/h)

#### Pressure

| Stream | Field Name | Unit | Update Freq | Notes |
|--------|-----------|------|-------------|-------|
| `outdoor-weather` | `pressure` | hPa | 10 min | Atmospheric |
| `nws-observations` | `barometric_pressure` | Pa | 5 min | Barometric |
| `nws-observations` | `sea_level_pressure` | Pa | 5 min | Sea level adjusted |

**Issue**: Unit inconsistency (hPa vs Pa = 100x difference)

#### Visibility

| Stream | Field Name | Unit | Update Freq |
|--------|-----------|------|-------------|
| `outdoor-weather` | `visibility` | meters | 10 min |
| `nws-observations` | `visibility` | meters | 5 min |

**Consistent**: Same unit and field name

#### Precipitation

| Stream | Field Name | Unit | Update Freq | Notes |
|--------|-----------|------|-------------|-------|
| `outdoor-weather` | `rain_1h` | mm | 10 min | Rain only |
| `outdoor-weather` | `snow_1h` | mm | 10 min | Snow only |
| `nws-observations` | `precipitation_1h` | meters | 5 min | Total precip |
| `nws-observations` | `precipitation_3h` | meters | 5 min | 3h total |
| `nws-observations` | `precipitation_6h` | meters | 5 min | 6h total |
| `nws-forecast-hourly` | `probability_of_precipitation` | percent | 1 hour | Probability only |

**Issue**:
- Unit inconsistency (mm vs meters = 1000x difference)
- Different aggregation windows (1h, 3h, 6h)
- Rain/snow split vs total precipitation

#### Particulate Matter (PM)

| Stream | Field Name | Unit | Update Freq | Location |
|--------|-----------|------|-------------|----------|
| `air-quality` | `pm25` | µg/m³ | Real-time | Indoor |
| `outdoor-air-quality` | `pm2_5` | µg/m³ | 10 min | Outdoor |
| `air-quality` | `pm10` | µg/m³ | Real-time | Indoor |
| `outdoor-air-quality` | `pm10` | µg/m³ | 10 min | Outdoor |

**Critical Issue**: Field name inconsistency (`pm25` vs `pm2_5`) - different naming conventions

---

## Unique Fields by Category

### Air Quality Only

| Field | Streams | Description |
|-------|---------|-------------|
| `co2` | air-quality | Indoor CO2 (ppm) |
| `tvoc` | air-quality | Indoor volatile compounds (ppb) |
| `nox` | air-quality | Indoor nitrogen oxides (ppb) |
| `aqi` | outdoor-air-quality | Air quality index 1-5 |
| `co` | outdoor-air-quality | Carbon monoxide (µg/m³) |
| `no` | outdoor-air-quality | Nitrogen monoxide (µg/m³) |
| `no2` | outdoor-air-quality | Nitrogen dioxide (µg/m³) |
| `o3` | outdoor-air-quality | Ozone (µg/m³) |
| `so2` | outdoor-air-quality | Sulfur dioxide (µg/m³) |
| `nh3` | outdoor-air-quality | Ammonia (µg/m³) |

### Weather Only

| Field | Streams | Description |
|-------|---------|-------------|
| `feels_like` | outdoor-weather | Apparent temperature (celsius) |
| `clouds` | outdoor-weather | Cloud coverage (percent) |
| `dewpoint` | nws-observations, nws-forecast-hourly | Dew point temperature |
| `wind_chill` | nws-observations | Wind chill temperature |
| `heat_index` | nws-observations | Heat index temperature |
| `max_temperature_24h` | nws-observations | 24h max temperature |
| `min_temperature_24h` | nws-observations | 24h min temperature |

### Forecast Only

| Field | Streams | Description |
|-------|---------|-------------|
| `short_forecast` | nws-forecast-hourly | Text forecast description |
| `forecast_issue_time` | nws-forecast-hourly | Forecast generation time |

---

## Data Quality Considerations

### Nullable Fields

**Non-nullable fields** (must always have values):
- `air-quality`: `pm25` only
- `outdoor-weather`: `temperature` only
- `outdoor-air-quality`: `aqi`, `pm2_5`
- `nws-observations`: All fields nullable
- `nws-forecast-hourly`: `temperature` only

**Implication**: Most fields can have NULL values requiring handling in Silver layer

### Data Freshness

| Stream | Poll Interval | Expected Latency | Notes |
|--------|--------------|------------------|-------|
| air-quality | Real-time | < 1 second | MQTT push |
| outdoor-weather | 10 minutes | 0-10 minutes | HTTP poll |
| outdoor-air-quality | 10 minutes | 0-10 minutes | HTTP poll |
| nws-observations | 5 minutes | 0-60 minutes | NWS updates hourly, we poll frequently |
| nws-forecast-hourly | 1 hour | 0-1 hour | NWS updates hourly |

### Unit Conversion Requirements for Silver Layer

**Critical conversions needed**:

1. **Temperature**: Fahrenheit → Celsius (nws-forecast-hourly)
2. **Wind Speed**: Standardize to single unit (m/s, km/h, or mph)
3. **Pressure**: hPa → Pa (multiply by 100)
4. **Precipitation**: mm → meters (divide by 1000)
5. **Field Names**: Standardize `humidity`/`relative_humidity`, `pm25`/`pm2_5`, `wind_deg`/`wind_direction`

---

## Storage Characteristics

### Batch Processing

| Stream | Batch Size | Batch Timeout | Buffer Capacity | Notes |
|--------|-----------|---------------|-----------------|-------|
| air-quality | 100 | 5s | 1000 | High-frequency sensor data |
| outdoor-weather | 50 | 30s | 500 | Low-frequency API polls |
| outdoor-air-quality | 50 | 30s | 500 | Low-frequency API polls |
| nws-observations | 50 | 30s | 500 | Moderate polling frequency |
| nws-forecast-hourly | 156 | 60s | 2500 | Large array responses (~1092 points) |

**Key insight**: `nws-forecast-hourly` generates massive batches (156 hours per poll) requiring larger buffers

### Retention Policies

| Stream | Retention Days | Compression After | Use Case |
|--------|---------------|-------------------|----------|
| air-quality | 365 | 7 | Long-term indoor air quality trends |
| outdoor-weather | 90 | 7 | Seasonal outdoor weather patterns |
| outdoor-air-quality | 90 | 7 | Seasonal outdoor pollution trends |
| nws-observations | 365 | 7 | Long-term weather station history |
| nws-forecast-hourly | 30 | 7 | Short-lived forecast data |

**Strategy**:
- Long retention (365 days) for observational data
- Short retention (30 days) for forecast data (ephemeral)
- Universal 7-day compression threshold

---

## Naming Inconsistencies Summary

**Critical issues for Silver layer standardization**:

### 1. PM Field Names
- Indoor: `pm25`, `pm10`
- Outdoor: `pm2_5`, `pm10`
- **Recommendation**: Standardize to `pm2_5` and `pm10` (matches scientific notation)

### 2. Humidity Field Names
- Some streams: `humidity`
- Others: `relative_humidity`
- **Recommendation**: Standardize to `relative_humidity` (more precise)

### 3. Wind Direction Field Names
- OpenWeatherMap: `wind_deg`
- NWS: `wind_direction`
- **Recommendation**: Standardize to `wind_direction` (more descriptive)

### 4. Pressure Field Names
- Generic: `pressure`
- Specific: `barometric_pressure`, `sea_level_pressure`
- **Recommendation**: Use specific names for clarity

---

## Silver Layer Design Recommendations

### 1. Create Unified Metric Tables

**Proposed tables**:
- `temperature_unified` - All temperature readings with source tag
- `humidity_unified` - All humidity readings (standardized name)
- `wind_unified` - Wind speed/direction/gust (standardized units)
- `pressure_unified` - All pressure readings (standardized units)
- `precipitation_unified` - All precipitation (standardized units)
- `air_quality_unified` - PM2.5, PM10, AQI, pollutants (standardized names)

### 2. Standardize Units via Views or Continuous Aggregates

Create TimescaleDB continuous aggregates that:
- Convert all temperatures to Celsius
- Convert all wind speeds to m/s (SI standard)
- Convert all pressures to Pa (SI standard)
- Convert all precipitation to mm (common meteorological unit)

### 3. Handle NULL Values

- Define interpolation strategies for nullable fields
- Use TimescaleDB `time_bucket` with `LOCF` (Last Observation Carried Forward)
- Document NULL handling per field in Silver schema

### 4. Implement Field Name Mapping

Create a mapping table or view layer:
```sql
-- Example mapping view
CREATE VIEW air_quality_normalized AS
SELECT
  timestamp,
  location_id,
  COALESCE(pm25, pm2_5) AS pm2_5,  -- Standardize field name
  pm10,
  COALESCE(humidity, relative_humidity) AS relative_humidity,
  source_stream
FROM bronze_raw;
```

### 5. Time Alignment Strategy

Different update frequencies require time bucketing:
- Real-time (air-quality): 1-minute buckets
- Frequent (nws-observations): 5-minute buckets
- Moderate (outdoor APIs): 10-minute buckets
- Hourly (forecasts): 1-hour buckets

---

## Data Source Metadata

### API Endpoints

| Stream | Provider | Endpoint Type | Authentication | Rate Limits |
|--------|----------|---------------|----------------|-------------|
| air-quality | AirGradient | MQTT broker | None | N/A (local) |
| outdoor-weather | OpenWeatherMap | REST API | API key (query param) | Unknown |
| outdoor-air-quality | OpenWeatherMap | REST API | API key (query param) | Unknown |
| nws-observations | NWS | REST API | None (User-Agent required) | Unknown |
| nws-forecast-hourly | NWS | REST API | None (User-Agent required) | Unknown |

### Location Information

| Stream | Location Type | Location ID | Coordinates/Identifier |
|--------|--------------|-------------|------------------------|
| air-quality | Dynamic | Sensor `serialno` | Multiple indoor sensors |
| outdoor-weather | Fixed | `home` | 29.95838, -81.30878 |
| outdoor-air-quality | Fixed | `home` | 29.95838, -81.30878 |
| nws-observations | Fixed | `ksgj` | Station KSGJ |
| nws-forecast-hourly | Fixed | `ksgj` | Grid JAX 79,49 |

---

## Total Field Count by Data Type

| Data Type | Count | Examples |
|-----------|-------|----------|
| float | 40 | temperature, humidity, wind_speed, pm2_5 |
| int | 4 | co2, tvoc, nox (indoor air quality) |
| string | 1 | short_forecast |
| **Total** | **46** | - |

---

## Appendix: Field Name Reference

### Alphabetical Field List

1. `aqi` - Air quality index (outdoor-air-quality)
2. `barometric_pressure` - Barometric pressure (nws-observations)
3. `clouds` - Cloud coverage (outdoor-weather)
4. `co` - Carbon monoxide (outdoor-air-quality)
5. `co2` - Carbon dioxide (air-quality)
6. `dewpoint` - Dew point (nws-observations, nws-forecast-hourly)
7. `feels_like` - Apparent temperature (outdoor-weather)
8. `forecast_issue_time` - Forecast timestamp (nws-forecast-hourly)
9. `heat_index` - Heat index (nws-observations)
10. `humidity` - Relative humidity (air-quality, outdoor-weather)
11. `max_temperature_24h` - 24h max temp (nws-observations)
12. `min_temperature_24h` - 24h min temp (nws-observations)
13. `nh3` - Ammonia (outdoor-air-quality)
14. `no` - Nitrogen monoxide (outdoor-air-quality)
15. `no2` - Nitrogen dioxide (outdoor-air-quality)
16. `nox` - Nitrogen oxides (air-quality)
17. `o3` - Ozone (outdoor-air-quality)
18. `pm10` - PM10 (air-quality, outdoor-air-quality)
19. `pm25` - PM2.5 (air-quality)
20. `pm2_5` - PM2.5 (outdoor-air-quality)
21. `precipitation_1h` - 1h precipitation (nws-observations)
22. `precipitation_3h` - 3h precipitation (nws-observations)
23. `precipitation_6h` - 6h precipitation (nws-observations)
24. `pressure` - Atmospheric pressure (outdoor-weather)
25. `probability_of_precipitation` - Precipitation probability (nws-forecast-hourly)
26. `rain_1h` - 1h rainfall (outdoor-weather)
27. `relative_humidity` - Relative humidity (nws-observations, nws-forecast-hourly)
28. `sea_level_pressure` - Sea level pressure (nws-observations)
29. `short_forecast` - Forecast text (nws-forecast-hourly)
30. `snow_1h` - 1h snowfall (outdoor-weather)
31. `so2` - Sulfur dioxide (outdoor-air-quality)
32. `temperature` - Temperature (air-quality, outdoor-weather, nws-observations, nws-forecast-hourly)
33. `tvoc` - VOCs (air-quality)
34. `visibility` - Visibility (outdoor-weather, nws-observations)
35. `wind_chill` - Wind chill (nws-observations)
36. `wind_deg` - Wind direction (outdoor-weather)
37. `wind_direction` - Wind direction (nws-observations, nws-forecast-hourly)
38. `wind_gust` - Wind gust (outdoor-weather, nws-observations)
39. `wind_speed` - Wind speed (outdoor-weather, nws-observations, nws-forecast-hourly)

---

## Next Steps for Silver Layer Design

1. **Create unit conversion functions** for all overlapping fields
2. **Design unified schema** with standardized field names
3. **Implement TimescaleDB hypertables** with appropriate partitioning
4. **Build continuous aggregates** for common time windows (1min, 5min, 1hr, 1day)
5. **Document NULL handling strategy** per field
6. **Create validation rules** for data quality (range checks, unit validation)
7. **Design indexing strategy** for common query patterns
8. **Implement retention policies** aligned with Bronze layer settings
9. **Build ETL pipeline** from Parquet to TimescaleDB with transformations
10. **Create data quality dashboards** to monitor Bronze→Silver pipeline health
