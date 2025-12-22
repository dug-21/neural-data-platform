# NWS API Integration Research - Saint George Island Area

**Research Date:** 2025-12-21
**Researcher:** Research Agent
**Purpose:** Document NWS API endpoints and data structures for air quality platform integration

---

## Executive Summary

Saint George Island (KSGI) does not have a dedicated NWS weather station. The nearest operational station is **KAAF** (Apalachicola Municipal Airport), located approximately 9 miles from Saint George Island. This station provides comprehensive weather observations and serves as the primary data source for the area.

**Key Finding:** Use KAAF station for Saint George Island weather data integration.

---

## Station Information

### Primary Station: KAAF (Apalachicola Municipal Airport)

| Property | Value |
|----------|-------|
| **Station ID** | KAAF |
| **Station Name** | Apalachicola, Apalachicola |
| **Latitude** | 29.72694°N |
| **Longitude** | 85.02472°W |
| **Elevation** | 6.096 meters (20 feet) |
| **Time Zone** | America/New_York (EST/EDT) |
| **County** | Franklin County, Florida (FLC037) |
| **Forecast Zone** | FLZ115 |
| **Fire Weather Zone** | FLZ115 |

### NWS Grid Point Information

| Property | Value |
|----------|-------|
| **WFO Code** | TAE (Tallahassee) |
| **Grid X** | 58 |
| **Grid Y** | 53 |
| **Radar Station** | KTLH (Tallahassee) |
| **Distance from Grid Point** | 2,960.48 meters (bearing 268°) |

---

## API Endpoints

### 1. Station Metadata
```
GET https://api.weather.gov/stations/KAAF
```

**Response Fields:**
- Station identification and location
- Elevation and time zone
- Links to forecast zones
- County information

---

### 2. Current Observations (Latest)
```
GET https://api.weather.gov/stations/KAAF/observations/latest
```

**Update Frequency:** Every 30-60 minutes
**Data Retention:** Latest observation only

**Available Data Fields:**

| Field | Unit | Example | Description |
|-------|------|---------|-------------|
| `timestamp` | ISO 8601 | `2025-12-21T18:35:00+00:00` | Observation time (UTC) |
| `textDescription` | string | `"Clear"` | Human-readable conditions |
| `temperature` | °C | `19.0` | Air temperature |
| `dewpoint` | °C | `9.0` | Dew point temperature |
| `relativeHumidity` | % | `52.28` | Relative humidity |
| `windDirection` | degrees | `130` | Wind direction (0-360°) |
| `windSpeed` | km/h | `11.124` | Wind speed |
| `windGust` | km/h | `null` | Wind gust (when available) |
| `barometricPressure` | Pa | `102302.8` | Station pressure |
| `seaLevelPressure` | Pa | `null` | Sea level pressure (when available) |
| `visibility` | meters | `16093.44` | Visibility distance |
| `maxTemperatureLast24Hours` | °C | `null` | 24-hour max temp |
| `minTemperatureLast24Hours` | °C | `null` | 24-hour min temp |
| `precipitationLast3Hours` | mm | `null` | 3-hour precipitation |
| `cloudLayers` | array | `[{base: 3810, amount: "CLR"}]` | Cloud coverage |

**Quality Control:** All fields include `qualityControl` status (V=verified, C=controlled, Z=missing)

---

### 3. Standard Forecast (12-Hour Periods)
```
GET https://api.weather.gov/gridpoints/TAE/58,53/forecast
```

**Update Frequency:** Every 1-3 hours
**Coverage:** 7 days (14 periods)

**Period Fields:**

| Field | Type | Example | Description |
|-------|------|---------|-------------|
| `number` | integer | `1` | Period sequence number |
| `name` | string | `"This Afternoon"` | Period name |
| `startTime` | ISO 8601 | `2025-12-21T13:00:00-05:00` | Period start (local time) |
| `endTime` | ISO 8601 | `2025-12-21T18:00:00-05:00` | Period end (local time) |
| `isDaytime` | boolean | `true` | Day or night period |
| `temperature` | integer | `69` | Temperature value |
| `temperatureUnit` | string | `"F"` | Temperature unit (F/C) |
| `temperatureTrend` | string | `null` | Trend (rising/falling) |
| `windSpeed` | string | `"5 mph"` | Wind speed with unit |
| `windDirection` | string | `"E"` | Cardinal direction |
| `icon` | URL | API icon URL | Weather icon |
| `shortForecast` | string | `"Sunny"` | Brief description |
| `detailedForecast` | string | Full sentence | Detailed description |
| `probabilityOfPrecipitation` | object | `{value: 0, unitCode: "wmoUnit:percent"}` | Precipitation chance |

---

### 4. Hourly Forecast
```
GET https://api.weather.gov/gridpoints/TAE/58,53/forecast/hourly
```

**Update Frequency:** Every 1 hour
**Coverage:** 156 hours (6.5 days)

**Hourly Fields (Same as Standard + Additional):**

| Field | Unit | Example | Description |
|-------|------|---------|-------------|
| `startTime` | ISO 8601 | `2025-12-21T13:00:00-05:00` | Hour start time |
| `endTime` | ISO 8601 | `2025-12-21T14:00:00-05:00` | Hour end time |
| `temperature` | integer | `67` | Hourly temperature |
| `dewpoint` | object | `{value: 10, unitCode: "wmoUnit:degC"}` | Dew point |
| `relativeHumidity` | object | `{value: 54, unitCode: "wmoUnit:percent"}` | Relative humidity |
| `windSpeed` | string | `"5 mph"` | Wind speed |
| `windDirection` | string | `"E"` | Wind direction |
| `shortForecast` | string | `"Sunny"` | Conditions |
| `probabilityOfPrecipitation` | object | `{value: 0}` | Precip probability |

---

### 5. Gridpoint Raw Data (Advanced)
```
GET https://api.weather.gov/gridpoints/TAE/58,53
```

**Update Frequency:** Hourly
**Coverage:** 7 days at hourly resolution

**Raw Grid Data Includes:**
- Temperature (hourly grid)
- Dew point (hourly)
- Relative humidity (hourly)
- Wind speed and direction (hourly)
- Precipitation probability (hourly)
- Quantitative precipitation forecast (QPF)
- Snow amount
- Ice accumulation
- Weather hazards
- Sky cover percentage
- Wave height (for coastal locations)
- Transport wind speed and direction

**Use Case:** Advanced modeling, ML features, trend analysis

---

### 6. Available Observation Stations (Discovery)
```
GET https://api.weather.gov/gridpoints/TAE/58,53/stations
```

**Returns:** List of all weather stations within the grid area
**Use Case:** Discovery of alternative stations, backup data sources

---

## Sample API Responses

### Current Observation Response Structure
```json
{
  "@context": [...],
  "id": "https://api.weather.gov/stations/KAAF/observations/2025-12-21T18:35:00+00:00",
  "type": "Feature",
  "geometry": {
    "type": "Point",
    "coordinates": [-85.02472, 29.72694, 6.096]
  },
  "properties": {
    "timestamp": "2025-12-21T18:35:00+00:00",
    "textDescription": "Clear",
    "temperature": {
      "value": 19.0,
      "unitCode": "wmoUnit:degC",
      "qualityControl": "V"
    },
    "dewpoint": {
      "value": 9.0,
      "unitCode": "wmoUnit:degC",
      "qualityControl": "V"
    },
    "relativeHumidity": {
      "value": 52.28,
      "unitCode": "wmoUnit:percent",
      "qualityControl": "C"
    },
    "windDirection": {
      "value": 130,
      "unitCode": "wmoUnit:degree_(angle)",
      "qualityControl": "V"
    },
    "windSpeed": {
      "value": 11.124,
      "unitCode": "wmoUnit:km_h-1",
      "qualityControl": "V"
    },
    "barometricPressure": {
      "value": 102302.8,
      "unitCode": "wmoUnit:Pa",
      "qualityControl": "V"
    },
    "visibility": {
      "value": 16093.44,
      "unitCode": "wmoUnit:m",
      "qualityControl": "C"
    }
  }
}
```

---

## Integration Recommendations

### For Neural Data Platform

**1. Primary Data Source:**
- Station: KAAF
- Endpoint: `/stations/KAAF/observations/latest`
- Poll Frequency: Every 15-30 minutes
- Fallback: Check `/gridpoints/TAE/58,53/stations` for alternatives

**2. Forecast Integration:**
- Hourly Forecast: For short-term predictions (next 6-12 hours)
- Standard Forecast: For daily summaries and longer-term trends
- Gridpoint Data: For ML feature engineering and advanced modeling

**3. Data Fields for Air Quality Correlation:**

| Priority | Field | Use Case |
|----------|-------|----------|
| **HIGH** | Temperature | Primary AQ correlation factor |
| **HIGH** | Relative Humidity | Particle formation, dispersion |
| **HIGH** | Wind Speed | Pollutant dispersion modeling |
| **HIGH** | Wind Direction | Source attribution, transport |
| **MEDIUM** | Barometric Pressure | Atmospheric stability, mixing height |
| **MEDIUM** | Dew Point | Humidity calculations, particle growth |
| **LOW** | Visibility | AQ proxy (when PM sensors unavailable) |
| **LOW** | Precipitation | Wet deposition, air cleaning events |

**4. Error Handling:**
- Handle `null` values (common for gusts, precipitation)
- Check `qualityControl` flags (V=verified, C=controlled, Z=missing)
- Implement retry logic for 404/500 errors
- Cache last known good observation for gaps

**5. Unit Conversions:**
- Temperature: °C to °F if needed
- Wind Speed: km/h to m/s or mph
- Pressure: Pa to hPa/mbar (divide by 100)
- Visibility: meters to miles (divide by 1609.34)

---

## Alternative Stations Investigated

| Station ID | Status | Notes |
|------------|--------|-------|
| KSGI | **404 Not Found** | Does not exist in NWS API |
| SGOF1 | **404 Not Found** | May be NOAA buoy/tide station only |
| KAAF | **Active** | Primary station for area |

**Recommendation:** Use KAAF exclusively. SGOF1 appears in marine/tide databases but not NWS meteorological network.

---

## API Best Practices

### Headers
```http
User-Agent: (YourApp, contact@example.com)
Accept: application/geo+json
```

### Rate Limits
- No official rate limit published
- Recommended: Max 1 request per endpoint per minute
- Implement exponential backoff for errors

### Caching
- Current observations: Cache for 10-15 minutes
- Forecasts: Cache for 30-60 minutes
- Grid metadata: Cache for 24 hours

### Error Responses

| Code | Meaning | Action |
|------|---------|--------|
| 200 | Success | Parse response |
| 404 | Not found | Check station ID, try alternative |
| 500 | Server error | Retry with exponential backoff |
| 503 | Service unavailable | Use cached data, retry later |

---

## Data Quality Notes

**Timestamp Handling:**
- All timestamps are ISO 8601 format
- Observation times are UTC (`+00:00`)
- Forecast times are local time zone (`-05:00` EST)
- Always convert to common timezone for storage

**Missing Data:**
- Fields may be `null` when not observed
- `qualityControl: "Z"` indicates missing/suspect data
- Implement interpolation or fallback strategies

**Station Maintenance:**
- Occasional outages for maintenance
- Data gaps may occur during severe weather
- Consider multi-station averaging for critical applications

---

## Integration Checklist

- [ ] Implement KAAF current observation fetcher
- [ ] Add hourly forecast retrieval for next 24 hours
- [ ] Set up error handling and retry logic
- [ ] Implement unit conversion utilities
- [ ] Create timestamp normalization functions
- [ ] Add quality control flag checking
- [ ] Set up caching layer (10-15 min for obs, 30-60 min for forecasts)
- [ ] Configure monitoring for API availability
- [ ] Document data schema in stream configuration
- [ ] Test null value handling for all fields
- [ ] Implement fallback to cached data on errors

---

## References

- NWS API Documentation: https://weather-gov.github.io/api/
- Station KAAF Metadata: https://api.weather.gov/stations/KAAF
- Grid Point TAE/58,53: https://api.weather.gov/points/29.72694,-85.02472
- Current Observations: https://api.weather.gov/stations/KAAF/observations/latest
- Hourly Forecast: https://api.weather.gov/gridpoints/TAE/58,53/forecast/hourly
- Standard Forecast: https://api.weather.gov/gridpoints/TAE/58,53/forecast

---

## Research Sources

- [NWS API General FAQs](https://weather-gov.github.io/api/general-faqs)
- [US Harbors - Apalachicola Weather](https://www.usharbors.com/harbor/florida/apalachicola-fl/weather/)
- [NOAA Tides and Currents - St. George Island Station](https://tidesandcurrents.noaa.gov/stationhome.html?id=8728626)
- Live API endpoint testing conducted 2025-12-21

---

**End of Research Document**
