# Weather API Comparison: WeatherAPI.com vs Weatherbit.io

**Research Date:** 2025-12-23
**Purpose:** Evaluate alternatives to Tomorrow.io with more accessible pricing for the Neural Data Platform

---

## Executive Summary

Both **WeatherAPI.com** and **Weatherbit.io** offer compelling alternatives to Tomorrow.io with significantly lower pricing barriers:

- **WeatherAPI.com**: Superior free tier (1M calls/month vs 50/day), excellent for development and small projects
- **Weatherbit.io**: Better hyperlocal accuracy (30m effective resolution), stronger for production workloads

**Recommendation for NDP:** Start with WeatherAPI.com for immediate integration, evaluate Weatherbit.io Standard tier ($49/month) for production with 25K calls/day.

---

## 1. WeatherAPI.com - Detailed Analysis

### 1.1 Free Tier

| Metric | Value |
|--------|-------|
| **API Calls** | 1,000,000/month (~33,000/day) |
| **Current Weather** | ✅ Real-time |
| **Forecast** | 3 days (hourly/daily) |
| **Historical** | 7 days past |
| **Air Quality** | ❌ Limited |
| **Marine/Tide** | ❌ Not included |
| **Uptime SLA** | 95.5% |
| **Commercial Use** | ❌ Not allowed |
| **Attribution** | Required (link back) |

**Key Limitation:** Free tier restricts commercial use and requires attribution.

### 1.2 Paid Tiers (Developer-Friendly)

| Tier | Monthly Cost | Annual Cost | Calls/Month | Key Features |
|------|--------------|-------------|-------------|--------------|
| **Starter** | $7 | $75 | 3M (~100K/day) | 7-day forecast, Air Quality, Sports API, Commercial ✅ |
| **Pro+** | $25 | $270 | 5M (~167K/day) | 14-day forecast, 365-day historical, 300-day future, Marine (5-day + tide) |
| **Business** | $35 | $378 | 10M (~333K/day) | 7-day marine, 15-min intervals, IP blocking, 99.9% uptime |
| **Enterprise** | Custom | Custom | Custom | Pollen history, Solar irradiance, Evapotranspiration, 100% uptime |

**Best Value:** Starter at $7/month enables commercial use with full air quality and 7-day forecasts.

### 1.3 API Response Format

**Current Weather JSON Structure:**
```json
{
  "location": {
    "name": "Raleigh",
    "region": "North Carolina",
    "country": "USA",
    "lat": 35.77,
    "lon": -78.64,
    "tz_id": "America/New_York",
    "localtime_epoch": 1703345400,
    "localtime": "2023-12-23 14:30"
  },
  "current": {
    "temp_c": 15.0,
    "temp_f": 59.0,
    "feelslike_c": 13.5,
    "feelslike_f": 56.3,
    "condition": {
      "text": "Partly cloudy",
      "icon": "//cdn.weatherapi.com/weather/64x64/day/116.png",
      "code": 1003
    },
    "wind_mph": 8.1,
    "wind_kph": 13.0,
    "wind_degree": 240,
    "wind_dir": "WSW",
    "gust_mph": 12.0,
    "gust_kph": 19.3,
    "pressure_mb": 1013.0,
    "pressure_in": 29.91,
    "humidity": 72,
    "cloud": 50,
    "vis_km": 10.0,
    "vis_miles": 6.0,
    "precip_mm": 0.0,
    "precip_in": 0.0,
    "uv": 4.0,
    "is_day": 1,
    "last_updated": "2023-12-23 14:00"
  }
}
```

**Forecast API Additions (hourly):**
```json
{
  "forecast": {
    "forecastday": [
      {
        "date": "2023-12-23",
        "astro": {
          "sunrise": "07:15 AM",
          "sunset": "05:08 PM",
          "moonrise": "01:42 PM",
          "moonset": "02:54 AM",
          "moon_phase": "Waxing Gibbous",
          "moon_illumination": "78"
        },
        "day": {
          "maxtemp_c": 18.0,
          "mintemp_c": 8.0,
          "avgtemp_c": 13.0,
          "maxwind_kph": 20.0,
          "totalprecip_mm": 2.5,
          "totalsnow_cm": 0.0,
          "avgvis_km": 9.5,
          "avghumidity": 70,
          "daily_will_it_rain": 1,
          "daily_chance_of_rain": 75,
          "daily_will_it_snow": 0,
          "daily_chance_of_snow": 0,
          "condition": {...},
          "uv": 3.0
        },
        "hour": [
          {
            "time": "2023-12-23 00:00",
            "temp_c": 10.0,
            "cloud": 25,
            "vis_km": 10.0,
            "precip_mm": 0.0,
            "snow_cm": 0.0,
            "will_it_rain": 0,
            "chance_of_rain": 10,
            "will_it_snow": 0,
            "chance_of_snow": 0
            // ... all current weather fields
          }
        ]
      }
    ]
  }
}
```

**Air Quality Object (Starter+):**
```json
{
  "air_quality": {
    "co": 230.3,        // Carbon Monoxide (μg/m³)
    "no2": 15.2,        // Nitrogen dioxide (μg/m³)
    "o3": 45.8,         // Ozone (μg/m³)
    "so2": 5.1,         // Sulphur dioxide (μg/m³)
    "pm2_5": 12.4,      // PM2.5 (μg/m³)
    "pm10": 18.7,       // PM10 (μg/m³)
    "us-epa-index": 1,  // EPA AQI (1-6 scale)
    "gb-defra-index": 2 // DEFRA UK index (1-10)
  }
}
```

### 1.4 Hyperlocal Accuracy & Data Sources

**Spatial Resolution:** Not explicitly documented (likely ~5-10km grid)

**Data Sources:**
- Thousands of global private and public weather stations
- Personal weather stations (updated every 10-15 minutes)
- Weather data providers worldwide
- AI and machine learning models

**Real-time Updates:** Every 10-15 minutes from live stations

**Strength:** Excellent station network coverage, but lower resolution than specialized providers.

### 1.5 Available Metrics

| Category | Metrics | Availability |
|----------|---------|--------------|
| **Temperature** | temp, feels_like, heat_index, wind_chill, dewpoint | All tiers |
| **Wind** | speed, direction, gust | All tiers |
| **Precipitation** | rain amount, snow amount, probability (hourly/daily) | All tiers |
| **Cloud Cover** | Percentage (0-100) | All tiers |
| **Visibility** | km/miles | All tiers |
| **Atmospheric** | pressure, humidity, UV index | All tiers |
| **Air Quality** | CO, NO2, O3, SO2, PM2.5, PM10, AQI indices | Starter+ |
| **Astronomy** | sunrise, sunset, moon phase/illumination | All tiers |
| **Marine** | Tide data, wave height, swell period | Pro+ |
| **Pollen** | 7 types (Hazel, Birch, Oak, Grass, etc.) | Enterprise |
| **Solar** | Irradiance (short_rad, diff_rad, DNI, GHI) | Enterprise |
| **Evapotranspiration** | ET0 | Business+ |

**Precipitation Types:** Rain and snow amounts tracked separately. Hourly `will_it_rain`/`will_it_snow` binary flags + probability percentages.

### 1.6 Historical Data Access

| Tier | Historical Range | Resolution | Notes |
|------|------------------|------------|-------|
| **Free** | 7 days past | Hourly/Daily | Limited to past week |
| **Starter** | 7 days past | Hourly/Daily | Same as free |
| **Pro+** | 365 days | Hourly/15-min | Full year back from Jan 1, 2010 onwards |
| **Business** | 365 days | Hourly/15-min | 15-min intervals available |
| **Enterprise** | From Jan 1, 2010 | Hourly/15-min | Full archive with pollen, AQI, solar |

**Data Available Since:** January 1, 2010 for all historical parameters.

### 1.7 Unique Features

1. **Sports API** - Weather for sports events and venues (Starter+)
2. **Future Weather** - Up to 365 days ahead forecast (Pro+)
3. **Astronomy Data** - Comprehensive moon/sun data (all tiers)
4. **Bulk Requests** - Batch API calls (Pro+)
5. **IP Lookup** - Automatic location from IP (all tiers)
6. **Search/Autocomplete** - Location search API (all tiers)
7. **Marine & Tide** - 5-7 day marine forecasts with tide data (Pro+/Business)
8. **15-Minute Intervals** - Sub-hourly resolution (Business+)

**Standout:** Best astronomy API, excellent free tier, sports integration unique.

---

## 2. Weatherbit.io - Detailed Analysis

### 2.1 Free Tier

| Metric | Value |
|--------|-------|
| **API Calls** | 50/day (~1,500/month) |
| **Current Weather** | ✅ Real-time |
| **Forecast** | 7 days (daily only) |
| **Historical** | ❌ Not included |
| **Air Quality** | ❌ Not included |
| **Marine/Tide** | ❌ Not included |
| **Uptime SLA** | None specified |
| **Commercial Use** | ❌ Not allowed |
| **Trial** | 21-day business trial available |

**Key Limitation:** Extremely limited at 50 calls/day. Primarily for evaluation only.

### 2.2 Paid Tiers (Production-Focused)

| Tier | Monthly Cost | Annual Cost | Calls/Day | Key Features |
|------|--------------|-------------|-----------|--------------|
| **Standard** | $49 | $45/mo ($540/yr) | 25,000 | 16-day forecast, Hourly/Minutely, Lightning, Commercial ✅ |
| **Plus** | $195 | $185/mo ($2,220/yr) | 250,000 | 5-year historical, Climate normals, Maps |
| **Business** | $495 | $475/mo ($5,700/yr) | 2,000,000 | 25-year historical, Air Quality, Ag-Weather, Energy APIs |
| **Enterprise** | $995+ | $950+/mo | 2M+ | Custom historical (25+ years), Dedicated support |

**Best Value:** Standard at $49/month provides production-ready 25K calls/day with hourly/minutely forecasts.

### 2.2 API Response Format

**Current Weather JSON Structure:**
```json
{
  "count": 1,
  "data": [
    {
      "lat": 35.7721,
      "lon": -78.6386,
      "timezone": "America/New_York",
      "ob_time": "2023-12-23 14:30",
      "ts": 1703345400,
      "city_name": "Raleigh",
      "country_code": "US",
      "state_code": "NC",
      "station": "K1RDU",
      "sources": ["K1RDU", "METAR"],

      // Temperature
      "temp": 15.0,
      "app_temp": 13.5,
      "dewpt": 8.5,

      // Wind
      "wind_spd": 3.6,
      "gust": 5.2,
      "wind_dir": 240,
      "wind_cdir": "WSW",
      "wind_cdir_full": "West-Southwest",

      // Atmospheric
      "pres": 1013.0,
      "slp": 1014.9,
      "rh": 72,

      // Sky & Visibility
      "clouds": 50,
      "vis": 10,

      // Precipitation
      "precip": 0.0,
      "snow": 0.0,

      // Weather Condition
      "weather": {
        "icon": "c02d",
        "code": 802,
        "description": "Scattered clouds"
      },

      // Solar & Other
      "pod": "d",
      "uv": 4.0,
      "aqi": 46,
      "dhi": 125.5,
      "dni": 680.3,
      "ghi": 425.8,
      "solar_rad": 425.8,
      "elev_angle": 35.2,

      // Time
      "sunrise": "07:15",
      "sunset": "17:08"
    }
  ]
}
```

**16-Day Forecast Response:**
```json
{
  "data": [
    {
      "valid_date": "2023-12-23",
      "ts": 1703289600,
      "datetime": "2023-12-23",

      // Temperature
      "temp": 13.0,
      "max_temp": 18.0,
      "min_temp": 8.0,
      "app_max_temp": 16.5,
      "app_min_temp": 6.0,
      "high_temp": 18.5,
      "low_temp": 7.5,

      // Precipitation
      "precip": 2.5,
      "snow": 0.0,
      "snow_depth": 0.0,
      "pop": 75,

      // Cloud & Visibility
      "clouds": 65,
      "clouds_hi": 20,
      "clouds_mid": 35,
      "clouds_low": 45,
      "vis": 9.5,

      // Wind
      "wind_spd": 4.2,
      "wind_gust_spd": 8.5,
      "wind_dir": 235,
      "wind_cdir": "SW",

      // Atmospheric
      "rh": 70,
      "dewpt": 7.5,
      "pres": 1012.5,
      "slp": 1015.0,

      // Solar & UV
      "uv": 3.0,
      "ozone": 285.5,
      "dhi": 45.2,
      "dni": 520.3,
      "ghi": 180.5,

      // Weather
      "weather": {
        "icon": "c04d",
        "code": 803,
        "description": "Broken clouds"
      },

      // Time
      "sunrise_ts": 1703336100,
      "sunset_ts": 1703370480,
      "moonrise_ts": 1703355720,
      "moonset_ts": 1703310840,
      "moon_phase": 0.78,
      "moon_phase_lunation": 0.45
    }
  ],
  "city_name": "Raleigh",
  "country_code": "US",
  "state_code": "NC",
  "timezone": "America/New_York"
}
```

**Hourly Forecast Fields (48 hours):**
- All daily fields at hourly resolution
- Adds: `precip6h`, `snow6h` (6-hour accumulation)

**Minutely Forecast (60 minutes):**
```json
{
  "data": [
    {
      "timestamp_utc": "2023-12-23T14:35:00",
      "timestamp_local": "2023-12-23T09:35:00",
      "ts": 1703345700,
      "precip": 0.5,  // mm/hr precipitation rate
      "snow": 0.0     // mm/hr snowfall rate
    }
  ]
}
```

### 2.4 Hyperlocal Accuracy & Data Sources

**Spatial Resolution:**
- **North America:** 1 km
- **Europe:** 1-6 km
- **Other Regions:** 9-13 km
- **Effective Resolution:** 30 meters (with elevation adjustments)

**How 30m is Achieved:**
- Base forecast from high-resolution models (1-13km grid)
- SRTM elevation data adjustment (30m resolution)
- Radar and satellite integration (<1km for precipitation/clouds)
- Machine learning bias correction
- Real-time backtesting and model verification

**Forecast Models:**
- **NOAA HRRR:** 3km resolution (North America)
- **NOAA GFS:** 13km resolution (Global)
- **DWD ICON-Europe:** 6.5km resolution (Europe)
- **ECMWF:** European Centre model
- Statistical/ML post-processing

**Historical Data Sources:**
- Weather stations (average ≤25km resolution)
- Doppler radar (<1km for precipitation)
- Satellite (1-13km for cloud cover)
- GLDAS reanalysis (0.25° = ~28km grid)
- ERA5 reanalysis

**Data Availability Guarantee:** 99.5% for major parameters (temp, dewpoint, RH, wind, cloud cover, solar, precipitation, snow)

**Strength:** Industry-leading hyperlocal accuracy with ML bias correction reducing errors up to 50%.

### 2.5 Available Metrics

| Category | Metrics | Availability |
|----------|---------|--------------|
| **Temperature** | temp, app_temp (feels-like), high_temp, low_temp, max/min | All paid tiers |
| **Wind** | speed, gust, direction (degrees + cardinal) | All paid tiers |
| **Precipitation** | rain, snow, snow_depth, probability (pop), 6h accumulation | All paid tiers |
| **Cloud Cover** | Total, high, mid, low clouds (%) | All paid tiers |
| **Visibility** | Distance | All paid tiers |
| **Atmospheric** | pressure, sea-level pressure, humidity, dewpoint, ozone | All paid tiers |
| **UV & Solar** | UV index, DNI, DHI, GHI, solar_rad, elevation angle | All paid tiers |
| **Air Quality** | AQI, pollutant concentrations | Business+ |
| **Lightning** | Strike data | Standard+ |
| **Minutely Precip** | 1-minute precipitation/snow rate | Standard+ |
| **Agriculture** | Soil temp (0-10cm, 10-40cm, 40-100cm, 100-200cm), soil moisture (0-10cm, 10-40cm, 40-100cm, 100-200cm), evapotranspiration | Business+ |
| **Degree Days** | Heating degree days, cooling degree days | Business+ |
| **Climate Normals** | 30-year averages | Plus+ |

**Precipitation Types:** Liquid equivalent (precip), snowfall (snow), snow depth on ground. Minutely API provides rainfall rate and snowfall rate.

### 2.6 Historical Data Access

| Tier | Historical Range | Resolution | Parameters |
|------|------------------|------------|------------|
| **Free** | ❌ None | N/A | N/A |
| **Standard** | ❌ None | N/A | N/A |
| **Plus** | 5 years | Daily/Hourly | Temp, precip, wind, cloud, humidity, pressure |
| **Business** | 25 years | Daily/Hourly/Sub-hourly | All params + soil, evapotranspiration, solar |
| **Enterprise** | 25+ years (custom) | Daily/Hourly/Sub-hourly | All parameters |

**Data Available Since:** Varies by parameter, primary data >20 years back. Agricultural data (GLDAS) goes back 10 years hourly/daily.

**Historical Resolution Options:**
- **Daily:** 1 day intervals
- **Hourly:** 1 hour intervals
- **Sub-hourly:** 15-minute intervals (Business+)

**Per-Request Limits:**
- Daily: 15 days per request (paid tiers), 1 day (trial)
- Hourly: 10 days per request (paid tiers), 1 day (trial)
- Sub-hourly: 5 days per request (paid tiers), 1 day (trial)

### 2.7 Unique Features

1. **Minutely Nowcasts** - 60-minute radar-backed precipitation forecasts at 1-min intervals (Standard+)
2. **30m Effective Resolution** - Industry-leading hyperlocal accuracy with SRTM elevation integration
3. **Agriculture API** - Soil temperature/moisture at 4 depths, evapotranspiration, solar radiation (Business+)
4. **Degree Days API** - Heating/cooling degree days for energy modeling (Business+)
5. **Lightning Data** - Real-time lightning strike information (Standard+)
6. **Climate Normals** - 30-year baseline averages (Plus+)
7. **Multi-Cloud Layers** - Separate high/mid/low cloud cover percentages (all paid tiers)
8. **Machine Learning Bias Correction** - 50% error reduction vs raw models
9. **99.5% Data Availability** - Guaranteed for historical data
10. **Maps API** - Weather map tiles and overlays (Plus+)

**Standout:** Best-in-class hyperlocal accuracy, excellent for agriculture/energy sectors, strong ML enhancement.

---

## 3. Head-to-Head Comparison

### 3.1 Free Tier Comparison

| Feature | WeatherAPI.com | Weatherbit.io | Winner |
|---------|----------------|---------------|--------|
| **Daily Calls** | 33,333 | 50 | **WeatherAPI** (666x more) |
| **Forecast Days** | 3 | 7 | **Weatherbit** |
| **Historical** | 7 days | None | **WeatherAPI** |
| **Resolution** | Hourly | Daily only | **WeatherAPI** |
| **Air Quality** | Limited | None | **WeatherAPI** |
| **Commercial Use** | ❌ | ❌ | Tie |
| **Best For** | Development & testing | Quick evaluation | **WeatherAPI** |

**Clear Winner for Free Tier:** WeatherAPI.com - massively higher call limits make it viable for actual development work.

### 3.2 Entry-Level Paid Tier Comparison

| Feature | WeatherAPI Starter ($7/mo) | Weatherbit Standard ($49/mo) | Better Value |
|---------|---------------------------|------------------------------|--------------|
| **Daily Calls** | 100,000 | 25,000 | WeatherAPI (4x) |
| **$/1000 calls** | $0.0023 | $0.0653 | WeatherAPI (28x cheaper) |
| **Forecast Days** | 7 | 16 | Weatherbit |
| **Resolution** | Hourly | Hourly + Minutely | Weatherbit |
| **Historical** | 7 days | None | WeatherAPI |
| **Hyperlocal Accuracy** | Standard | 30m effective | Weatherbit |
| **Air Quality** | ✅ Full | ❌ | WeatherAPI |
| **Lightning** | ❌ | ✅ | Weatherbit |
| **Best For** | High-volume hobby projects | Serious production apps | Depends on needs |

**Budget Winner:** WeatherAPI Starter at $7/month
**Production Winner:** Weatherbit Standard for hyperlocal accuracy

### 3.3 Mid-Tier Comparison

| Feature | WeatherAPI Pro+ ($25/mo) | Weatherbit Plus ($195/mo) | Better Value |
|---------|--------------------------|---------------------------|--------------|
| **Daily Calls** | 167,000 | 250,000 | Similar (Weatherbit 1.5x) |
| **$/1000 calls** | $0.0050 | $0.0260 | WeatherAPI (5.2x cheaper) |
| **Forecast Days** | 14 | 16 | Weatherbit |
| **Historical** | 365 days | 5 years | Weatherbit |
| **Marine/Tide** | 5 days | ❌ | WeatherAPI |
| **Maps** | ❌ | ✅ | Weatherbit |
| **Climate Normals** | ❌ | ✅ | Weatherbit |
| **Future Forecast** | 300 days | ❌ | WeatherAPI |

**Budget Winner:** WeatherAPI Pro+ at $25/month (7.8x cheaper)
**Data Depth Winner:** Weatherbit Plus for 5-year historical archive

### 3.4 Feature Comparison Matrix

| Feature | WeatherAPI.com | Weatherbit.io |
|---------|----------------|---------------|
| **Hyperlocal Resolution** | ~5-10km (estimated) | 1-13km grid → 30m effective |
| **Forecast Range** | 3-14 days (tier dependent) | 7-16 days |
| **Hourly Forecast** | ✅ (all paid tiers) | ✅ (Standard+) |
| **Minutely Forecast** | ❌ | ✅ (Standard+, 60-min) |
| **15-Min Intervals** | ✅ (Business+) | ✅ (Business+, historical) |
| **Historical Depth** | From Jan 1, 2010 | >20 years |
| **Future Forecast** | 300 days (Pro+) | ❌ |
| **Air Quality** | ✅ CO, NO2, O3, SO2, PM2.5, PM10, AQI (Starter+) | ✅ AQI (Business+) |
| **Pollen** | ✅ 7 types (Enterprise) | ❌ |
| **Astronomy** | ✅ Comprehensive (all tiers) | ✅ Basic (sunrise/sunset, moon) |
| **Sports API** | ✅ (Starter+) | ❌ |
| **Marine & Tide** | ✅ 5-7 days (Pro+/Business) | ❌ |
| **Agriculture** | ❌ (only ET0 on Business+) | ✅ Comprehensive (Business+) |
| **Degree Days** | ❌ | ✅ (Business+) |
| **Lightning** | ❌ | ✅ (Standard+) |
| **Climate Normals** | ❌ | ✅ (Plus+) |
| **Maps API** | ✅ (all tiers) | ✅ (Plus+) |
| **ML Bias Correction** | ✅ (undocumented) | ✅ (up to 50% error reduction) |
| **Uptime SLA** | 95.5%-100% (tier dependent) | Not specified |

### 3.5 Data Quality Comparison

| Aspect | WeatherAPI.com | Weatherbit.io | Winner |
|--------|----------------|---------------|--------|
| **Spatial Resolution** | ~5-10km | 1-13km → 30m effective | **Weatherbit** |
| **Forecast Models** | Not disclosed | HRRR 3km, GFS 13km, ICON 6.5km, ECMWF | **Weatherbit** (transparent) |
| **Station Network** | Thousands global + personal stations | METAR, MADIS, radar, satellite | Tie (different strengths) |
| **Update Frequency** | 10-15 minutes | Sub-hourly from stations | Tie |
| **Historical Availability** | Not specified | 99.5% guaranteed | **Weatherbit** |
| **ML Enhancement** | Yes (AI/ML mentioned) | Yes (50% error reduction) | **Weatherbit** (quantified) |
| **Precipitation Source** | Stations | Radar + satellite + stations | **Weatherbit** (radar-backed) |
| **Cloud Cover Source** | Not disclosed | Satellite + models | **Weatherbit** |

**Data Quality Winner:** Weatherbit.io - superior spatial resolution, transparent data sources, quantified ML improvements.

---

## 4. Use Case Recommendations

### 4.1 For Neural Data Platform (NDP)

**Phase 1 - Development & Testing:**
- **Recommendation:** WeatherAPI.com Free Tier
- **Rationale:** 33K calls/day allows extensive development without cost
- **Limitations:** Must upgrade before commercial deployment

**Phase 2 - MVP & Early Production:**
- **Recommendation:** WeatherAPI.com Starter ($7/month)
- **Rationale:** Extremely affordable, enables commercial use, includes air quality
- **When to Switch:** If hyperlocal accuracy becomes critical or >100K calls/day needed

**Phase 3 - Production with Hyperlocal Requirements:**
- **Recommendation:** Weatherbit.io Standard ($49/month)
- **Rationale:** 30m effective resolution, 25K calls/day, minutely forecasts
- **Trade-off:** Higher cost but better accuracy for air quality correlation

**Hybrid Approach:**
- Use WeatherAPI.com for general weather context (cheaper)
- Use Weatherbit.io for hyperlocal precipitation/cloud cover (better accuracy)
- Correlate NWS data with both for validation

### 4.2 Cost Analysis for NDP Scenarios

**Scenario A: Low-Volume Monitoring (5K calls/day)**
- WeatherAPI Starter: $7/month ✅
- Weatherbit Standard: $49/month (overkill)
- **Recommendation:** WeatherAPI Starter

**Scenario B: Medium-Volume Research (20K calls/day)**
- WeatherAPI Starter: $7/month (still under 100K/day limit) ✅
- Weatherbit Standard: $49/month (within 25K/day limit)
- **Recommendation:** WeatherAPI Starter for budget, Weatherbit for accuracy

**Scenario C: High-Volume Production (200K calls/day)**
- WeatherAPI Pro+: $25/month (under 167K/day limit - would need Business at $35/month)
- Weatherbit Plus: $195/month (within 250K/day limit)
- **Recommendation:** WeatherAPI Business ($35/month) for 333K/day capacity

**Scenario D: Historical Analysis (1-year backfill)**
- WeatherAPI Pro+: $25/month (365-day access) ✅
- Weatherbit Plus: $195/month (5-year access)
- **Recommendation:** WeatherAPI Pro+ unless deeper history needed

### 4.3 Comparison to Tomorrow.io

**Tomorrow.io Pricing (for context):**
- Free tier: 25 calls/hour (~600/day)
- Starter: ~$150/month (estimated)
- Business: Custom pricing (typically $500-2000/month)

**Cost Comparison:**

| Provider | Entry Tier | Cost | Calls/Day | $/1K Calls |
|----------|-----------|------|-----------|------------|
| Tomorrow.io | Starter | ~$150/mo | ~50K | ~$0.10 |
| WeatherAPI | Starter | $7/mo | 100K | $0.0023 |
| Weatherbit | Standard | $49/mo | 25K | $0.065 |

**WeatherAPI.com:** 95% cheaper than Tomorrow.io
**Weatherbit.io:** 67% cheaper than Tomorrow.io

**Feature Parity:**
- WeatherAPI.com matches Tomorrow.io on most features except minutely forecasts
- Weatherbit.io matches or exceeds Tomorrow.io on hyperlocal accuracy
- Both alternatives provide excellent air quality data

---

## 5. Integration Considerations for NDP

### 5.1 API Authentication

**WeatherAPI.com:**
```
https://api.weatherapi.com/v1/current.json?key={API_KEY}&q={location}&aqi=yes
```
- Simple API key in query parameter
- No rate limit headers documented

**Weatherbit.io:**
```
https://api.weatherbit.io/v2.0/current?lat={lat}&lon={lon}&key={API_KEY}
```
- API key in query parameter
- Rate limit tracking via response headers
- HTTP 429 on rate limit exceeded

### 5.2 Location Specification

**WeatherAPI.com supports:**
- Latitude/Longitude: `q=35.77,-78.64`
- City name: `q=Raleigh`
- ZIP code: `q=27601`
- IP address: `q=auto:ip` (auto-detect)
- Weather station ID: `q=iata:RDU`

**Weatherbit.io supports:**
- Latitude/Longitude: `lat=35.77&lon=-78.64`
- City name: `city=Raleigh&country=US`
- City ID: `city_id=4487042`
- Postal code: `postal_code=27601`
- Weather station ID: `station=KRDU`

**For NDP:** Use lat/lon for consistency with existing Purple Air station locations.

### 5.3 Response Parsing

**WeatherAPI.com:**
- Clean nested JSON structure
- Consistent field naming (snake_case)
- Dual units (metric + imperial) by default
- Condition codes for mapping to icons

**Weatherbit.io:**
- Flat array structure (`data[0]`)
- Snake_case naming
- Metric by default (add `units=I` for imperial)
- Weather codes for condition mapping

**For NDP:** Both easy to parse. Weatherbit requires array indexing even for single location.

### 5.4 Rate Limit Management

**WeatherAPI.com:**
- Monthly limit tracking in dashboard
- No real-time rate limit headers
- Recommend implementing client-side counter

**Weatherbit.io:**
- Daily limit resets at 00:00 UTC
- HTTP 429 on exceed
- Recommend exponential backoff + retry logic

**For NDP:** Implement local rate limiting for both to avoid overages.

### 5.5 Error Handling

**WeatherAPI.com Error Response:**
```json
{
  "error": {
    "code": 1006,
    "message": "No matching location found."
  }
}
```

**Common Error Codes:**
- 1002: API key not provided
- 1003: Invalid location parameter
- 1005: Invalid API request URL
- 1006: No location found
- 2006: Invalid API key
- 2007: API key quota exceeded
- 2008: API key disabled

**Weatherbit.io Error Response:**
```json
{
  "status_code": 429,
  "status_message": "Request count exceeded!"
}
```

**HTTP Status Codes:**
- 200: Success
- 204: No data found
- 400: Bad request
- 401: Unauthorized (invalid key)
- 429: Rate limit exceeded
- 500: Server error

**For NDP:** Implement retry logic for 429, alert on 401/403, log 500 errors.

### 5.6 Data Mapping to NDP Schema

**NDP Air Quality Schema Fields:**
```rust
struct AirQuality {
    timestamp: DateTime<Utc>,
    location: Location,
    pm25: f32,
    pm10: f32,
    temperature: f32,
    humidity: f32,
    pressure: f32,
    // ... external weather context
}
```

**Mapping from WeatherAPI.com:**
```rust
// Current weather → NDP context
temperature: current.temp_c,
humidity: current.humidity,
pressure: current.pressure_mb,
cloud_cover: current.cloud,
visibility: current.vis_km,
wind_speed: current.wind_kph,
wind_direction: current.wind_degree,
precipitation: current.precip_mm,

// Air quality (if aqi=yes)
pm25: air_quality.pm2_5,
pm10: air_quality.pm10,
// Note: WeatherAPI uses different pollutant units than Purple Air
```

**Mapping from Weatherbit.io:**
```rust
// Current weather → NDP context
temperature: data[0].temp,
humidity: data[0].rh,
pressure: data[0].pres,
cloud_cover: data[0].clouds,
visibility: data[0].vis,
wind_speed: data[0].wind_spd,
wind_direction: data[0].wind_dir,
precipitation: data[0].precip,

// Air quality (Business+ tier only)
aqi: data[0].aqi,
// Full pollutant breakdown requires separate AQ API endpoint
```

**Challenge:** Neither API provides PM2.5/PM10 at Standard tier for Weatherbit. WeatherAPI includes it at Starter+.

### 5.7 Caching Strategy

**Recommendation for NDP:**

1. **Current Weather:** Cache for 10-15 minutes (aligns with update frequency)
2. **Hourly Forecast:** Cache for 1 hour (updates hourly)
3. **Daily Forecast:** Cache for 6 hours (minimal changes)
4. **Historical Data:** Cache indefinitely (immutable)

**Storage:**
- Use Redis/Memcached for hot cache
- PostgreSQL for historical archive
- Parquet for bulk historical analysis

**Cache Key Pattern:**
```
weather:{provider}:{endpoint}:{lat}:{lon}:{params}
Example: weather:weatherapi:current:35.77:-78.64:aqi=yes
```

---

## 6. Recommendations

### 6.1 Immediate Action Items for NDP

1. **Sign up for WeatherAPI.com Free Tier** ✅
   - Start integration immediately with 1M calls/month
   - Test correlation with Purple Air data
   - Validate cloud cover impact on PM2.5

2. **Request Weatherbit.io 21-Day Trial** ✅
   - Test Business tier features (minutely, agriculture)
   - Compare hyperlocal accuracy vs WeatherAPI
   - Benchmark ML-enhanced forecasts

3. **Implement Dual-Provider Support** 🔧
   - Create abstraction layer for weather data sources
   - Allow runtime switching between providers
   - Facilitate cost optimization and redundancy

### 6.2 Migration Path from Tomorrow.io

**Step 1:** Add WeatherAPI.com as secondary source (free tier)
**Step 2:** Run parallel comparison for 30 days
**Step 3:** Analyze correlation quality and cost savings
**Step 4:** Switch primary to WeatherAPI Starter ($7/month)
**Step 5:** Optionally add Weatherbit for hyperlocal accuracy

**Cost Savings:** $143-1993/month depending on Tomorrow.io tier

### 6.3 Final Recommendation

**For Neural Data Platform:**

**Development Phase:**
- **Primary:** WeatherAPI.com Free Tier (33K calls/day)
- **Backup:** NWS API (unlimited but lower resolution)
- **Cost:** $0/month

**Production Phase:**
- **Primary:** WeatherAPI.com Starter ($7/month, 100K calls/day)
- **Hyperlocal Supplement:** Weatherbit.io Standard ($49/month, 25K calls/day)
- **Total Cost:** $56/month vs $150-2000/month for Tomorrow.io
- **Savings:** 63-97% cost reduction

**When to Choose WeatherAPI.com:**
- Budget-constrained projects
- Need air quality data early
- Want comprehensive astronomy/marine data
- High API call volumes (100K-333K/day affordable)
- Sports weather integration needed

**When to Choose Weatherbit.io:**
- Hyperlocal accuracy critical (30m resolution)
- Agriculture/soil data needed
- Minutely precipitation forecasts required
- Deep historical analysis (5+ years)
- Lightning data needed
- Climate normals for baselines

**Best Strategy:** Use both
- WeatherAPI for general context + air quality (cheaper)
- Weatherbit for hyperlocal precipitation/cloud cover (more accurate)
- Aggregate in Silver layer for ML feature engineering

---

## 7. API Response Examples

### 7.1 WeatherAPI.com - Full Current Weather Response

```json
{
  "location": {
    "name": "Raleigh",
    "region": "North Carolina",
    "country": "United States of America",
    "lat": 35.77,
    "lon": -78.64,
    "tz_id": "America/New_York",
    "localtime_epoch": 1703345400,
    "localtime": "2023-12-23 14:30"
  },
  "current": {
    "last_updated_epoch": 1703345400,
    "last_updated": "2023-12-23 14:00",
    "temp_c": 15.0,
    "temp_f": 59.0,
    "is_day": 1,
    "condition": {
      "text": "Partly cloudy",
      "icon": "//cdn.weatherapi.com/weather/64x64/day/116.png",
      "code": 1003
    },
    "wind_mph": 8.1,
    "wind_kph": 13.0,
    "wind_degree": 240,
    "wind_dir": "WSW",
    "pressure_mb": 1013.0,
    "pressure_in": 29.91,
    "precip_mm": 0.0,
    "precip_in": 0.0,
    "humidity": 72,
    "cloud": 50,
    "feelslike_c": 13.5,
    "feelslike_f": 56.3,
    "windchill_c": 13.0,
    "windchill_f": 55.4,
    "heatindex_c": 15.0,
    "heatindex_f": 59.0,
    "dewpoint_c": 9.8,
    "dewpoint_f": 49.6,
    "vis_km": 10.0,
    "vis_miles": 6.0,
    "uv": 4.0,
    "gust_mph": 12.0,
    "gust_kph": 19.3,
    "air_quality": {
      "co": 230.3,
      "no2": 15.2,
      "o3": 45.8,
      "so2": 5.1,
      "pm2_5": 12.4,
      "pm10": 18.7,
      "us-epa-index": 1,
      "gb-defra-index": 2
    }
  }
}
```

### 7.2 Weatherbit.io - Full Current Weather Response

```json
{
  "count": 1,
  "data": [
    {
      "app_temp": 13.5,
      "aqi": 46,
      "city_name": "Raleigh",
      "clouds": 50,
      "country_code": "US",
      "datetime": "2023-12-23:14",
      "dewpt": 9.8,
      "dhi": 125.5,
      "dni": 680.3,
      "elev_angle": 35.2,
      "ghi": 425.8,
      "gust": 5.2,
      "h_angle": -15.5,
      "lat": 35.7721,
      "lon": -78.6386,
      "ob_time": "2023-12-23 14:30",
      "pod": "d",
      "precip": 0.0,
      "pres": 1013.0,
      "rh": 72,
      "slp": 1014.9,
      "snow": 0.0,
      "solar_rad": 425.8,
      "sources": ["K1RDU", "METAR"],
      "state_code": "NC",
      "station": "K1RDU",
      "sunrise": "07:15",
      "sunset": "17:08",
      "temp": 15.0,
      "timezone": "America/New_York",
      "ts": 1703345400,
      "uv": 4.0,
      "vis": 10,
      "weather": {
        "icon": "c02d",
        "code": 802,
        "description": "Scattered clouds"
      },
      "wind_cdir": "WSW",
      "wind_cdir_full": "West-Southwest",
      "wind_dir": 240,
      "wind_spd": 3.6
    }
  ]
}
```

---

## 8. Technical Integration Patterns for NDP

### 8.1 Domain Adapter Pattern

Following NDP's established architecture, create a `WeatherSource` trait:

```rust
// core/src/sources/weather/mod.rs

#[async_trait]
pub trait WeatherSource: Send + Sync {
    async fn current_weather(&self, location: &Location) -> Result<CurrentWeather>;
    async fn hourly_forecast(&self, location: &Location, hours: u8) -> Result<Vec<HourlyForecast>>;
    async fn air_quality(&self, location: &Location) -> Result<AirQuality>;
}

// Implementations
pub struct WeatherApiSource { /* ... */ }
pub struct WeatherbitSource { /* ... */ }
pub struct NWSSource { /* ... */ }
```

### 8.2 Configuration Schema

```yaml
# config/base/streams/external-weather/config.yaml

stream_id: external-weather
source_type: multi-weather
enabled: true

sources:
  primary:
    provider: weatherapi
    api_key: ${WEATHER_API_KEY}
    base_url: https://api.weatherapi.com/v1
    rate_limit: 100000  # per day
    cache_ttl: 900      # 15 minutes

  secondary:
    provider: weatherbit
    api_key: ${WEATHERBIT_API_KEY}
    base_url: https://api.weatherbit.io/v2.0
    rate_limit: 25000   # per day
    cache_ttl: 900

  fallback:
    provider: nws
    base_url: https://api.weather.gov
    rate_limit: null    # unlimited
    cache_ttl: 3600     # 1 hour

collection:
  interval_seconds: 900  # every 15 minutes
  locations:
    - lat: 35.7796
      lon: -78.6382
      name: raleigh_downtown
    - lat: 35.8715
      lon: -78.7883
      name: rdu_airport

storage:
  parquet:
    enabled: true
    partition_by: [date, location]
  timescale:
    enabled: true
    table: weather_observations
```

### 8.3 Data Pipeline Flow

```
┌─────────────────────────────────────────────────────────┐
│ Weather Ingestion Coordinator                           │
│                                                          │
│ ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │
│ │ WeatherAPI   │  │ Weatherbit   │  │ NWS API      │   │
│ │ Source       │  │ Source       │  │ Source       │   │
│ └──────┬───────┘  └──────┬───────┘  └──────┬───────┘   │
│        │                 │                  │           │
│        └─────────┬───────┴──────────────────┘           │
│                  ▼                                       │
│         ┌─────────────────┐                             │
│         │ Weather Router  │                             │
│         │ (merge/validate)│                             │
│         └────────┬─────────┘                            │
└──────────────────┼──────────────────────────────────────┘
                   ▼
         ┌─────────────────┐
         │ Bronze Layer    │
         │ (Parquet WAL)   │
         └────────┬─────────┘
                   ▼
         ┌─────────────────┐
         │ Silver Layer    │
         │ (TimescaleDB)   │
         └────────┬─────────┘
                   ▼
         ┌─────────────────┐
         │ Gold Layer      │
         │ (ML Features)   │
         └──────────────────┘
```

### 8.4 Sample Implementation

```rust
// core/src/sources/weather/weatherapi.rs

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct WeatherApiSource {
    client: Client,
    api_key: String,
    base_url: String,
    rate_limiter: RateLimiter,
}

#[derive(Deserialize)]
struct WeatherApiResponse {
    location: WeatherApiLocation,
    current: WeatherApiCurrent,
}

#[derive(Deserialize)]
struct WeatherApiCurrent {
    temp_c: f32,
    humidity: f32,
    pressure_mb: f32,
    cloud: f32,
    vis_km: f32,
    wind_kph: f32,
    wind_degree: f32,
    precip_mm: f32,
    #[serde(default)]
    air_quality: Option<WeatherApiAirQuality>,
}

#[async_trait]
impl WeatherSource for WeatherApiSource {
    async fn current_weather(&self, location: &Location) -> Result<CurrentWeather> {
        // Rate limit check
        self.rate_limiter.acquire().await?;

        // Build request
        let url = format!(
            "{}/current.json?key={}&q={},{}&aqi=yes",
            self.base_url,
            self.api_key,
            location.lat,
            location.lon
        );

        // Execute with retry logic
        let response = self.client
            .get(&url)
            .timeout(Duration::from_secs(10))
            .send()
            .await?
            .error_for_status()?
            .json::<WeatherApiResponse>()
            .await?;

        // Map to domain model
        Ok(CurrentWeather {
            timestamp: Utc::now(),
            location: location.clone(),
            temperature: response.current.temp_c,
            humidity: response.current.humidity,
            pressure: response.current.pressure_mb,
            cloud_cover: Some(response.current.cloud),
            visibility: Some(response.current.vis_km),
            wind_speed: Some(response.current.wind_kph),
            wind_direction: Some(response.current.wind_degree),
            precipitation: Some(response.current.precip_mm),
            air_quality: response.current.air_quality.map(|aq| AirQuality {
                pm25: Some(aq.pm2_5),
                pm10: Some(aq.pm10),
                aqi: Some(aq.us_epa_index),
                ..Default::default()
            }),
        })
    }
}
```

---

## 9. Sources & References

### WeatherAPI.com Documentation
- [Pricing](https://www.weatherapi.com/pricing.aspx)
- [API Documentation](https://www.weatherapi.com/docs/)
- [Interactive API Explorer](https://www.weatherapi.com/api-explorer.aspx)
- [Free Weather API Overview](https://www.weatherapi.com/)

### Weatherbit.io Documentation
- [Pricing](https://www.weatherbit.io/pricing)
- [API Documentation](https://www.weatherbit.io/api)
- [Current Weather API](https://www.weatherbit.io/api/weather-current)
- [16-Day Forecast API](https://www.weatherbit.io/api/weather-forecast-16-day)
- [Historical Weather API](https://www.weatherbit.io/api/historical-weather-daily)
- [Agriculture API](https://www.weatherbit.io/api/ag-weather-api)
- [Minutely Forecast API](https://www.weatherbit.io/api/weather-forecast-minutely)

### Third-Party Comparisons
- [36 Best Weather APIs in 2025: Free and Paid Options](https://www.getambee.com/blogs/best-weather-apis)
- [Best Weather API for 2025: Free & Paid Options Compared](https://www.visualcrossing.com/resources/blog/best-weather-api-for-2025/)
- [The Best Weather APIs for 2025](https://www.tomorrow.io/blog/top-weather-apis/)

### Technical Resources
- [WeatherAPI.com Python SDK](https://github.com/weatherapicom/python)
- [Weatherbit.io Python Wrapper](https://github.com/weatherbit/weatherbit-python)

---

**Research completed:** 2025-12-23
**Next steps:** Implement Domain Adapter pattern, integrate WeatherAPI.com free tier, evaluate correlation with Purple Air data.
