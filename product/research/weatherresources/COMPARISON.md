# Weather API Provider Comparison for NDP

*Research Date: 2025-12-23*

## Executive Summary

For hyperlocal, accurate weather data with freemium pricing, **Open-Meteo** emerges as the top recommendation, with **WeatherAPI.com** as an excellent budget alternative.

| Provider | Free Tier | Paid Entry | Resolution | Best For |
|----------|-----------|------------|------------|----------|
| **Open-Meteo** | 10K/day | $29/mo (1M calls) | 1-3km (HRRR) | Primary source |
| **WeatherAPI.com** | 33K/day | $7/mo | ~5-10km | Budget + AQI |
| **Weatherbit.io** | 50/day | $49/mo | 30m effective | Hyperlocal |
| **NWS Raw Gridpoints** | Unlimited | Free | 2.5km grid | Most complete (40+ fields) |
| **NWS Observations** | Unlimited | Free | Station-based | Real-time current conditions |
| Tomorrow.io | 500/day | B2B only | 1km | ❌ No hobbyist tier |
| OpenWeatherMap | 1K/day | $40/mo | ~varies | Legacy option |

---

## Top 3 Recommendations

### 1. Open-Meteo (Primary Recommendation)

**Why:** Best balance of quality, coverage, and cost

| Aspect | Details |
|--------|---------|
| **Free Tier** | 10,000 calls/day (no API key needed) |
| **Paid Tier** | $29/month for 1M calls |
| **Resolution** | 3km (HRRR for US), 1-2km (Europe) |
| **Data Sources** | 15+ agencies: NOAA, ECMWF, DWD, MeteoFrance |
| **Update Freq** | Hourly (HRRR), 6-hourly (global) |
| **Historical** | 1940-present (free!) |
| **Air Quality** | Integrated (PM2.5, PM10, O3, NO2, AQI) |

**API Format:** Row-oriented, ArrayIteratorParser compatible
```json
{
  "hourly": {
    "time": ["2025-12-23T00:00", ...],
    "temperature_2m": [15.2, ...],
    "cloud_cover": [85, ...]
  }
}
```

**Unique Advantages:**
- Open source (CC BY 4.0)
- No vendor lock-in
- Combines best model for each location
- 15-minute data available (US/Europe)

---

### 2. WeatherAPI.com (Budget Option)

**Why:** Extremely generous free tier, great for development

| Aspect | Details |
|--------|---------|
| **Free Tier** | 33,333 calls/day (1M/month) |
| **Starter** | $7/month for 100K/day |
| **Resolution** | ~5-10km |
| **Air Quality** | Full (CO, NO2, O3, PM2.5, PM10, AQI) |
| **Historical** | 7 days free, 365 days at Pro tier |
| **Forecast** | 3 days (free), 14 days (paid) |

**API Format:** Row-oriented, compatible
```json
{
  "forecast": {
    "forecastday": [{
      "hour": [{
        "time": "2025-12-23 00:00",
        "temp_c": 15.2,
        "cloud": 85,
        "vis_km": 10
      }]
    }]
  }
}
```

**Cost Comparison:**
- WeatherAPI Starter: $7/mo for 100K calls/day
- Tomorrow.io equivalent: $150+/mo (enterprise only)
- **Savings: 95%**

---

### 3. Weatherbit.io (Hyperlocal Accuracy)

**Why:** Best resolution for precision applications

| Aspect | Details |
|--------|---------|
| **Free Tier** | 50 calls/day (testing only) |
| **Standard** | $49/month for 25K/day |
| **Resolution** | **30m effective** (ML-enhanced) |
| **Minutely** | 60-min nowcast with radar |
| **Cloud Layers** | High/mid/low breakdown |
| **Agriculture** | Soil temp/moisture at 4 depths |

**Best for:** Hyperlocal precipitation, cloud cover correlation with sensors

---

## Feature Matrix

### Forecast Data

| Feature | Open-Meteo | WeatherAPI | NWS Raw | NWS Hourly | Tomorrow.io |
|---------|------------|------------|---------|------------|-------------|
| temperature | ✅ | ✅ | ✅ | ✅ | ✅ |
| humidity | ✅ | ✅ | ✅ | ✅ | ✅ |
| dewpoint | ✅ | ✅ | ✅ | ✅ | ✅ |
| **cloud_cover** | ✅ | ✅ | ✅ | ❌ | ✅ |
| **visibility** | ✅ | ✅ | ✅ | ❌ | ✅ |
| precip_probability | ✅ | ✅ | ✅ | ✅ | ✅ |
| precip_amount | ✅ | ✅ | ✅ | ❌ | ✅ |
| wind_speed | ✅ | ✅ | ✅ | ✅ | ✅ |
| **wind_gust** | ✅ | ✅ | ✅ | ❌ | ✅ |
| pressure | ✅ | ✅ | ❌ | ❌ | ✅ |
| uv_index | ✅ | ✅ | ❌ | ❌ | ✅ |
| air_quality | ✅ | ✅ | ❌ | ❌ | ❌ |
| snow/ice amounts | ✅ | ✅ | ✅ | ❌ | ✅ |
| **fire_indices** | ❌ | ❌ | ✅ | ❌ | ❌ |
| **ceiling_height** | ❌ | ❌ | ✅ | ❌ | ✅ |
| historical | ✅ (1940+) | ✅ (7d-365d) | ❌ | ❌ | ✅ |

**Bold** = Fields missing from NWS hourly but available in raw gridpoints

### Current Observations

| Feature | NWS Stations | WeatherAPI | Open-Meteo | Weatherbit |
|---------|--------------|------------|------------|------------|
| temperature | ✅ | ✅ | ❌* | ✅ |
| humidity | ✅ | ✅ | ❌* | ✅ |
| dewpoint | ✅ | ✅ | ❌* | ✅ |
| cloud_layers | ✅ (multi) | ✅ | ❌* | ✅ (multi) |
| visibility | ✅ | ✅ | ❌* | ✅ |
| pressure | ✅ | ✅ | ❌* | ✅ |
| wind_speed | ✅ | ✅ | ❌* | ✅ |
| wind_gust | ✅ | ✅ | ❌* | ✅ |
| precip_rate | Limited | ✅ | ❌* | ✅ |
| uv_index | ❌ | ✅ | ❌* | ✅ |
| air_quality | ❌ | ✅ | ❌* | ❌ |

*Open-Meteo is forecast-only - no real-time observations

---

## Parser Compatibility

| Provider | Format | ArrayIteratorParser | Notes |
|----------|--------|---------------------|-------|
| WeatherAPI | Row-oriented | ✅ Compatible | `forecast.forecastday[].hour` |
| Weatherbit | Row-oriented | ✅ Compatible | `data[]` |
| NWS Hourly | Row-oriented | ✅ Compatible | `properties.periods[]` |
| NWS Observations | Single object | ⚠️ Use FlatJsonParser | `properties.*` |
| **NWS Raw Gridpoints** | Column-oriented | ❌ Needs new parser | Each metric has own `values[]` |
| **Open-Meteo** | Column-oriented | ❌ Needs new parser | Parallel arrays (time[], temp[]) |

### Column-Oriented Format (requires new parser)

Both NWS raw gridpoints and Open-Meteo use column-oriented data:

```json
// NWS Raw Gridpoints
{
  "temperature": {"values": [{"validTime": "2025-12-23T00:00/PT1H", "value": 15}]},
  "skyCover": {"values": [{"validTime": "2025-12-23T00:00/PT1H", "value": 85}]}
}

// Open-Meteo
{
  "hourly": {
    "time": ["2025-12-23T00:00", "2025-12-23T01:00"],
    "temperature_2m": [15.2, 14.8],
    "cloud_cover": [85, 90]
  }
}
```

**Recommendation:** Build a `ColumnOrientedParser` to support both NWS raw gridpoints and Open-Meteo with a single implementation.

---

## Pricing Summary

| Monthly Budget | Recommendation | Calls/Day |
|----------------|----------------|-----------|
| **$0** | Open-Meteo | 10,000 |
| **$0** | WeatherAPI.com | 33,333 |
| **$7** | WeatherAPI Starter | 100,000 |
| **$29** | Open-Meteo Commercial | ~33,333 |
| **$49** | Weatherbit Standard | 25,000 |
| **$56** | WeatherAPI + Weatherbit | 125,000 |

---

## Implementation Recommendation for NDP

### Option A: Maximum Free Data (NWS-Only)

**Best if:** You want 100% free with most complete forecast data

| Component | Endpoint | Parser | Poll Interval |
|-----------|----------|--------|---------------|
| Observations | `/stations/KSGJ/observations/latest` | FlatJsonParser | 15-20 min |
| Forecast | `/gridpoints/JAX/79,49` (raw) | ColumnOrientedParser (new) | 1 hour |

**Pros:** 40+ forecast fields, unlimited calls, official data
**Cons:** Requires new parser, no air quality, 20-min observation delay

### Option B: Easiest Integration (WeatherAPI)

**Best if:** You want working solution with current parsers

| Component | Endpoint | Parser | Poll Interval |
|-----------|----------|--------|---------------|
| Current + Forecast | `api.weatherapi.com/v1/forecast.json` | ArrayIteratorParser | 1 hour |
| Air Quality | Included in above | Same | Same |

**Pros:** Single endpoint, parser-ready, includes AQI
**Cons:** $7/mo for production, less complete than NWS raw

### Option C: Hybrid (Best of Both)

**Best if:** You want maximum data with some parser work

| Component | Source | Parser | Notes |
|-----------|--------|--------|-------|
| Observations | NWS Stations | FlatJsonParser | Free, official |
| Forecast | Open-Meteo | ColumnOrientedParser | Free, 3km HRRR |
| Air Quality | Open-Meteo | Same as above | Integrated |
| Historical | Open-Meteo | Same | 1940+ free |

**Pros:** All free, most complete combined dataset
**Cons:** Requires new parser (but reusable for both sources)

### Recommended Path

1. **Immediate:** Build `ColumnOrientedParser` (unlocks NWS raw + Open-Meteo)
2. **Then:** Add NWS station observations for current conditions
3. **Then:** Add Open-Meteo for forecasts + air quality + historical
4. **Optional:** Keep NWS hourly as US backup (no parser change needed)

---

## Data Completeness Summary

| Source | Current Obs | Forecast Fields | Air Quality | Parser Ready |
|--------|-------------|-----------------|-------------|--------------|
| NWS Raw + Stations | ✅ 15+ | ✅ 40+ | ❌ | ❌ (needs column parser) |
| Open-Meteo | ❌ | ✅ 50+ | ✅ | ❌ (needs column parser) |
| WeatherAPI | ✅ 15+ | ✅ 20+ | ✅ | ✅ |
| NWS Hourly | ❌ | ⚠️ 12 (limited) | ❌ | ✅ |

**See:** `NWS-COMPLETE-ANALYSIS.md` for detailed NWS endpoint documentation.

---

## Sources

- https://open-meteo.com/
- https://www.weatherapi.com/
- https://www.weatherbit.io/
- https://api.weather.gov/
- https://www.tomorrow.io/
- https://openweathermap.org/
