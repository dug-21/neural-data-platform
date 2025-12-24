# NWS API Complete Data Analysis

*Research Date: 2025-12-23*

## Executive Summary

The National Weather Service (NWS) API provides **comprehensive, free, unlimited** access to weather data for US locations. However, the data is split across multiple endpoints with different formats and completeness levels.

| Endpoint | Data Type | Fields | Format | Parser Compatible |
|----------|-----------|--------|--------|-------------------|
| `/stations/{id}/observations` | Current | 15+ | Row-oriented | ArrayIteratorParser |
| `/gridpoints/{wfo}/{x},{y}` | Forecast | 40+ | Column-oriented | Needs new parser |
| `/gridpoints/.../forecast/hourly` | Forecast | 12 | Row-oriented | ArrayIteratorParser |

**Key Finding:** To get complete data from NWS, you need BOTH:
1. Station observations for current conditions
2. Raw gridpoints for comprehensive forecasts

---

## Part 1: Current Observations

### Endpoint
```
GET https://api.weather.gov/stations/{stationId}/observations/latest
GET https://api.weather.gov/stations/{stationId}/observations
```

### Available Stations (Jacksonville Area)

| Station | Name | Distance |
|---------|------|----------|
| **KSGJ** | NE Florida Regional Airport | 1.3 km |
| **KNIP** | Jacksonville NAS | 44.6 km |
| **KCRG** | Craig Municipal Airport | 45.9 km |
| **KJAX** | Jacksonville Intl Airport | ~50 km |

*43 stations available within the JAX grid coverage*

### Observation Fields Available

| Field | Unit | Always Present | Notes |
|-------|------|----------------|-------|
| **temperature** | °C | Yes | |
| **dewpoint** | °C | Yes | |
| **relativeHumidity** | % | Yes | Calculated |
| **windDirection** | degrees | Yes | 0-360 |
| **windSpeed** | km/h | Yes | |
| **windGust** | km/h | No | Null if calm |
| **barometricPressure** | Pa | Yes | |
| **seaLevelPressure** | Pa | No | Often null |
| **visibility** | m | Yes | |
| **cloudLayers** | array | Yes | Base height + coverage |
| **textDescription** | string | Yes | "Clear", "Partly Cloudy" |
| **heatIndex** | °C | No | Null if not applicable |
| **windChill** | °C | No | Null if not applicable |
| **precipitationLastHour** | mm | No | Often null |
| **precipitationLast3Hours** | mm | No | Often null |
| **precipitationLast6Hours** | mm | No | Often null |

### Sample Response Structure

```json
{
  "properties": {
    "timestamp": "2025-12-23T12:53:00+00:00",
    "textDescription": "Clear",
    "temperature": {"unitCode": "wmoUnit:degC", "value": 19},
    "dewpoint": {"unitCode": "wmoUnit:degC", "value": 13},
    "windDirection": {"unitCode": "wmoUnit:degree_(angle)", "value": 70},
    "windSpeed": {"unitCode": "wmoUnit:km_h-1", "value": 5.544},
    "barometricPressure": {"unitCode": "wmoUnit:Pa", "value": 102573.72},
    "visibility": {"unitCode": "wmoUnit:m", "value": 16093.44},
    "relativeHumidity": {"unitCode": "wmoUnit:percent", "value": 68.18},
    "cloudLayers": [
      {"base": {"unitCode": "wmoUnit:m", "value": 3810}, "amount": "CLR"}
    ]
  }
}
```

### Parser Compatibility

**ArrayIteratorParser: COMPATIBLE** (with some considerations)

- For latest observation: Single object, use FlatJsonParser or custom
- For observation history: Array at `features[]`, ArrayIteratorParser works
- Timestamp: `properties.timestamp`
- All metrics nested under `properties`

---

## Part 2: Forecast Data

### Option A: Hourly Forecast (Current Implementation)

**Endpoint:** `GET /gridpoints/{wfo}/{x},{y}/forecast/hourly`

**Fields Available:**

| Field | Unit | Notes |
|-------|------|-------|
| temperature | °F | Integer |
| dewpoint | °C | Object with value |
| relativeHumidity | % | Object with value |
| windSpeed | string | "5 mph" format |
| windDirection | string | "N", "NE", etc. |
| probabilityOfPrecipitation | % | Object with value |
| shortForecast | string | "Partly Cloudy" |
| detailedForecast | string | Full description |

**Missing from Hourly Forecast:**
- skyCover (cloud %)
- visibility
- windGust
- pressure
- quantitativePrecipitation
- snowfall/ice amounts
- UV index
- all fire weather indices

### Option B: Raw Gridpoints (Full Data)

**Endpoint:** `GET /gridpoints/{wfo}/{x},{y}`

**Complete Field Inventory (40+ metrics):**

#### Temperature Suite
| Field | Unit | Coverage |
|-------|------|----------|
| temperature | °C | 7+ days hourly |
| dewpoint | °C | 7+ days hourly |
| maxTemperature | °C | Daily |
| minTemperature | °C | Daily |
| apparentTemperature | °C | Hourly |
| wetBulbGlobeTemperature | °C | Hourly |
| heatIndex | °C | When applicable |
| windChill | °C | When applicable |

#### Wind Suite
| Field | Unit | Coverage |
|-------|------|----------|
| windSpeed | km/h | Hourly |
| windDirection | degrees | Hourly |
| windGust | km/h | Hourly |
| transportWindSpeed | km/h | Hourly |
| transportWindDirection | degrees | Hourly |
| twentyFootWindSpeed | km/h | Fire weather |
| twentyFootWindDirection | degrees | Fire weather |

#### Precipitation Suite
| Field | Unit | Coverage |
|-------|------|----------|
| probabilityOfPrecipitation | % | Hourly |
| quantitativePrecipitation | mm | Hourly |
| snowfallAmount | mm | Hourly |
| iceAccumulation | mm | Hourly |

#### Sky & Visibility
| Field | Unit | Coverage |
|-------|------|----------|
| **skyCover** | % | Hourly |
| **visibility** | m | Hourly |
| **ceilingHeight** | m | Hourly |
| weather | object | Conditions array |

#### Fire Weather & Indices
| Field | Unit | Coverage |
|-------|------|----------|
| dispersionIndex | index | Hourly |
| stability | class | Hourly |
| lowVisibilityOccurrenceRiskIndex | index | Hourly |
| probabilityOfThunder | % | Hourly |
| mixingHeight | m | Hourly |

#### Marine (Coastal Areas)
| Field | Unit | Coverage |
|-------|------|----------|
| waveHeight | m | Limited |
| wavePeriod | sec | Limited |
| waveDirection | degrees | Limited |

### Raw Gridpoints Data Structure (Column-Oriented)

```json
{
  "properties": {
    "updateTime": "2025-12-23T08:38:36+00:00",
    "validTimes": "2025-12-23T02:00:00+00:00/P7DT23H",
    "elevation": {"unitCode": "wmoUnit:m", "value": 3.048},
    "temperature": {
      "uom": "wmoUnit:degC",
      "values": [
        {"validTime": "2025-12-23T02:00:00+00:00/PT3H", "value": 16.1},
        {"validTime": "2025-12-23T05:00:00+00:00/PT1H", "value": 15.6},
        ...
      ]
    },
    "skyCover": {
      "uom": "wmoUnit:percent",
      "values": [
        {"validTime": "2025-12-23T02:00:00+00:00/PT1H", "value": 5},
        {"validTime": "2025-12-23T03:00:00+00:00/PT2H", "value": 7},
        ...
      ]
    }
  }
}
```

**Key Differences from Hourly Forecast:**
- Each metric has its own `values[]` array (column-oriented)
- Time periods may vary between metrics (PT1H, PT3H, PT6H)
- Values use ISO 8601 duration format
- Much more complete data set

---

## Part 3: Parser Requirements

### Current ArrayIteratorParser Limitation

The ArrayIteratorParser expects:
```json
{
  "periods": [
    {"time": "...", "temp": 15, "humidity": 80, "cloud": 50},
    {"time": "...", "temp": 14, "humidity": 82, "cloud": 55}
  ]
}
```

Raw gridpoints provides:
```json
{
  "temperature": {"values": [{"validTime": "...", "value": 15}, ...]},
  "humidity": {"values": [{"validTime": "...", "value": 80}, ...]},
  "skyCover": {"values": [{"validTime": "...", "value": 50}, ...]}
}
```

### Solutions for Raw Gridpoints

| Option | Effort | Flexibility |
|--------|--------|-------------|
| **ColumnOrientedParser** | Medium | High - reusable for Open-Meteo |
| **Pre-transform middleware** | Low | Medium - HTTP layer pivot |
| **Custom NWS parser** | Low | Low - NWS-specific |

**Recommended:** Build `ColumnOrientedParser` - both NWS raw gridpoints AND Open-Meteo use this pattern.

---

## Part 4: Data Completeness Comparison

### Current Observations

| Field | NWS Station | Open-Meteo | WeatherAPI | Weatherbit |
|-------|-------------|------------|------------|------------|
| temperature | Yes | No* | Yes | Yes |
| humidity | Yes | No* | Yes | Yes |
| dewpoint | Yes | No* | Yes | Yes |
| pressure | Yes | No* | Yes | Yes |
| wind_speed | Yes | No* | Yes | Yes |
| wind_direction | Yes | No* | Yes | Yes |
| wind_gust | Yes | No* | Yes | Yes |
| visibility | Yes | No* | Yes | Yes |
| cloud_cover | Yes (layers) | No* | Yes | Yes |
| precip_rate | Limited | No* | Yes | Yes |
| uv_index | No | No* | Yes | Yes |
| air_quality | No | No* | Yes | Limited |

*Open-Meteo is forecast-only, no real-time observations

**Winner for Current Observations:** NWS (free, official) or WeatherAPI (integrated AQI)

### Forecast Data

| Field | NWS Hourly | NWS Raw | Open-Meteo | WeatherAPI |
|-------|------------|---------|------------|------------|
| temperature | Yes | Yes | Yes | Yes |
| humidity | Yes | Yes | Yes | Yes |
| dewpoint | Yes | Yes | Yes | Yes |
| wind_speed | Yes | Yes | Yes | Yes |
| wind_direction | Yes | Yes | Yes | Yes |
| wind_gust | No | Yes | Yes | Yes |
| **sky_cover** | No | **Yes** | Yes | Yes |
| **visibility** | No | **Yes** | Yes | Yes |
| pressure | No | No | Yes | Yes |
| precip_probability | Yes | Yes | Yes | Yes |
| precip_amount | No | Yes | Yes | Yes |
| snow_amount | No | Yes | Yes | Yes |
| uv_index | No | No | Yes | Yes |
| air_quality | No | No | Yes | Yes |
| fire_indices | No | **Yes** | No | No |
| marine_data | No | Limited | No | No |

**Winner for Forecast:** NWS Raw Gridpoints (most complete for free) or Open-Meteo (easier format + AQI)

---

## Part 5: Recommended NWS Strategy for NDP

### Option 1: NWS-Only (Free, Complete)

```
Current Observations: /stations/KSGJ/observations/latest
                      → Use FlatJsonParser or custom single-object parser
                      → Poll every 15-20 minutes

Forecast Data:        /gridpoints/JAX/79,49
                      → Build ColumnOrientedParser
                      → Poll every 1 hour (matches NWS update cycle)
```

**Pros:**
- 100% free, unlimited calls
- Official government data source
- Most complete forecast data (40+ fields)
- Fire weather indices (unique)

**Cons:**
- Requires new parser for raw gridpoints
- No air quality data
- Observations limited to airport stations
- 20-minute observation delay (MADIS processing)

### Option 2: Hybrid (NWS + Open-Meteo)

```
Observations:         /stations/KSGJ/observations/latest (NWS)
Forecast:             api.open-meteo.com/v1/forecast (Open-Meteo)
Air Quality:          api.open-meteo.com/v1/air-quality (Open-Meteo)
```

**Pros:**
- Best of both worlds
- Open-Meteo has easier format (still needs column parser)
- Integrated air quality
- Historical data from Open-Meteo

**Cons:**
- Two data sources to maintain
- Open-Meteo also column-oriented

### Option 3: Commercial Alternative

```
All Data:             api.weatherapi.com (WeatherAPI.com)
                      → Single endpoint for current + forecast + AQI
                      → ArrayIteratorParser compatible
                      → $7/mo for 100K calls/day
```

**Pros:**
- Simplest integration
- Current parser works
- Includes air quality
- Generous free tier for testing

**Cons:**
- Not as complete as NWS raw gridpoints
- Commercial dependency
- Lower resolution than HRRR

---

## Part 6: Implementation Effort Matrix

| Task | NWS Raw | Open-Meteo | WeatherAPI |
|------|---------|------------|------------|
| New parser needed | Yes (Column) | Yes (Column) | No |
| Config complexity | Medium | Low | Low |
| Free tier adequate | Yes (unlimited) | Yes (10K/day) | Yes (33K/day) |
| Air quality | Separate source | Included | Included |
| Observations | Separate endpoint | N/A (forecast only) | Included |

---

## Appendix: NWS API Best Practices

### Rate Limits
- Generous but unspecified threshold
- Retry after ~5 seconds if rate limited
- Direct requests preferred over proxies

### Update Frequency
| Data Type | Update Interval |
|-----------|-----------------|
| Observations | ~20 min delay (MADIS QC) |
| Hourly Forecast | Hourly |
| Raw Gridpoints | Every 1-6 hours (model dependent) |

### Required Headers
```yaml
User-Agent: "(your-app/version, contact@email.com)"
Accept: "application/geo+json"
```

### Caching Recommendations
- Cache gridpoint coordinates (they rarely change)
- Periodically verify via `/points` endpoint
- Cache forecast data for 1 hour between refreshes
