# Open-Meteo Deep Dive

*Research Date: 2025-12-23*

## Overview

Open-Meteo is an **open-source weather API** that aggregates data from 15+ national weather agencies worldwide. It offers enterprise-grade data quality with generous free tier limits.

**Website:** https://open-meteo.com/
**GitHub:** https://github.com/open-meteo
**License:** CC BY 4.0 (attribution required)

---

## Pricing

| Tier | Calls/Day | Calls/Month | Cost |
|------|-----------|-------------|------|
| **Free** | 10,000 | ~300,000 | $0 |
| **Commercial** | ~33,333 | 1,000,000 | $29/mo |
| **Custom** | Unlimited | Unlimited | Contact |

**Free tier includes:**
- No API key required
- All endpoints (forecast, historical, air quality)
- Non-commercial use
- 5,000 calls/hour, 600 calls/minute rate limits

---

## Data Sources (15+ Agencies)

| Agency | Models | Resolution | Coverage |
|--------|--------|------------|----------|
| **NOAA (US)** | GFS, HRRR | 3-25km | North America |
| **ECMWF (EU)** | IFS, AIFS (AI) | 9km | Global |
| **DWD (Germany)** | ICON, ICON-D2 | 2-13km | Europe |
| **MeteoFrance** | Arome, Arpege | 1.3-10km | France/Europe |
| **Environment Canada** | GEM, HRDPS | 2.5-10km | North America |
| **JMA (Japan)** | MSM, GSM | 5-20km | Asia-Pacific |
| **BOM (Australia)** | ACCESS-G | 12km | Australia |
| **CMA (China)** | GFS-CMA | 13km | China |
| + 7 more | Various | Various | Regional |

**Smart Blending:** Open-Meteo automatically selects the highest-resolution model available for each location.

---

## Resolution by Region

| Region | Best Resolution | Model |
|--------|-----------------|-------|
| **Central Europe** | 1.3 km | Arome (MeteoFrance) |
| **Germany/Alps** | 2 km | ICON-D2 (DWD) |
| **North America** | 3 km | HRRR (NOAA) |
| **Canada Dense** | 2.5 km | HRDPS |
| **Global** | 9 km | ECMWF IFS |
| **Global (backup)** | 25 km | GFS |

For US locations (like Jacksonville), expect **3km resolution** from HRRR with **hourly updates**.

---

## Available Endpoints

### 1. Weather Forecast API
```
GET https://api.open-meteo.com/v1/forecast
```

**Parameters:**
- `latitude`, `longitude` (required)
- `hourly` - comma-separated variables
- `daily` - daily aggregations
- `timezone` - local time conversion
- `forecast_days` - 1-16 days

**Hourly Variables (50+):**
- `temperature_2m`, `temperature_80m`, `temperature_120m`
- `relative_humidity_2m`
- `dew_point_2m`
- `apparent_temperature`
- `precipitation`, `precipitation_probability`
- `rain`, `showers`, `snowfall`
- `cloud_cover`, `cloud_cover_low`, `cloud_cover_mid`, `cloud_cover_high`
- `visibility`
- `wind_speed_10m`, `wind_speed_80m`, `wind_speed_120m`
- `wind_direction_10m`
- `wind_gusts_10m`
- `surface_pressure`, `pressure_msl`
- `shortwave_radiation`, `direct_radiation`, `diffuse_radiation`
- `uv_index`, `uv_index_clear_sky`
- `et0_fao_evapotranspiration`
- `soil_temperature_0cm` through `soil_temperature_54cm`
- `soil_moisture_0_1cm` through `soil_moisture_27_81cm`

### 2. Air Quality API
```
GET https://api.open-meteo.com/v1/air-quality
```

**Variables:**
- `pm10`, `pm2_5`
- `carbon_monoxide`, `nitrogen_dioxide`, `sulphur_dioxide`, `ozone`
- `ammonia`, `dust`
- `european_aqi`, `us_aqi`
- `us_aqi_pm2_5`, `us_aqi_pm10`, `us_aqi_o3`, `us_aqi_no2`, `us_aqi_co`, `us_aqi_so2`

### 3. Historical Weather API
```
GET https://api.open-meteo.com/v1/archive
```

**Coverage:** 1940 to present (ERA5 reanalysis)
**Resolution:** 9-25km
**Free:** Yes, same limits as forecast

### 4. Historical Forecast API
```
GET https://api.open-meteo.com/v1/forecast (with past_days)
```

**Coverage:** Past 2-5 years
**Resolution:** 1-3km (same as current forecasts)
**Use case:** Backfill high-resolution data

---

## API Response Format

```json
{
  "latitude": 30.2672,
  "longitude": -97.7431,
  "elevation": 149.0,
  "generationtime_ms": 0.5,
  "utc_offset_seconds": -21600,
  "timezone": "America/Chicago",
  "timezone_abbreviation": "CST",
  "hourly_units": {
    "time": "iso8601",
    "temperature_2m": "°C",
    "relative_humidity_2m": "%",
    "cloud_cover": "%",
    "precipitation_probability": "%",
    "visibility": "m"
  },
  "hourly": {
    "time": [
      "2025-12-23T00:00",
      "2025-12-23T01:00",
      "2025-12-23T02:00"
    ],
    "temperature_2m": [15.2, 14.8, 14.5],
    "relative_humidity_2m": [78, 80, 82],
    "cloud_cover": [85, 90, 95],
    "precipitation_probability": [20, 30, 40],
    "visibility": [16000, 15000, 14000]
  }
}
```

**Key difference from NWS/Tomorrow.io:**
- Uses **parallel arrays** instead of array of objects
- `time[0]` corresponds to `temperature_2m[0]`, `cloud_cover[0]`, etc.
- More compact but requires different parsing approach

---

## Parser Consideration for NDP

Open-Meteo's parallel array format differs from `ArrayIteratorParser` expectations:

**Current parser expects:**
```json
{"periods": [{"time": "...", "temp": 15}, {"time": "...", "temp": 14}]}
```

**Open-Meteo provides:**
```json
{"hourly": {"time": ["...", "..."], "temperature_2m": [15, 14]}}
```

**Options:**
1. **New parser type** (`ParallelArrayParser`) - cleanest
2. **Pre-transform middleware** - pivot columns to rows before parsing
3. **Custom parser registration** - one-off for Open-Meteo

---

## Update Frequency

| Model | Update Interval | Data Delay |
|-------|-----------------|------------|
| HRRR (US) | Every 1 hour | <20 minutes |
| ICON-D2 (Europe) | Every 1 hour | <20 minutes |
| ECMWF IFS | Every 6 hours | ~2 hours |
| GFS | Every 6 hours | Variable |

**15-minute data:** Available for US and Europe (interpolated from hourly models)

---

## Example Requests

### Basic Forecast (US Location)
```bash
curl "https://api.open-meteo.com/v1/forecast?latitude=30.27&longitude=-97.74&hourly=temperature_2m,relative_humidity_2m,cloud_cover,precipitation_probability,visibility,wind_speed_10m&forecast_days=7&timezone=America/Chicago"
```

### With Air Quality
```bash
curl "https://api.open-meteo.com/v1/air-quality?latitude=30.27&longitude=-97.74&hourly=pm2_5,pm10,us_aqi&timezone=America/Chicago"
```

### Historical Data
```bash
curl "https://api.open-meteo.com/v1/archive?latitude=30.27&longitude=-97.74&start_date=2024-01-01&end_date=2024-12-31&hourly=temperature_2m,precipitation"
```

---

## Accuracy Assessment

**No formal benchmarks published**, but:

- Uses same underlying models as national weather services
- Higher resolution (1-3km) captures local variations better than global models
- Community validated (50K+ R package downloads, production deployments)
- Multi-model blending reduces single-model bias

**For Jacksonville, FL:**
- Primary model: HRRR (3km, hourly updates)
- Backup: GFS (25km)
- Expected accuracy: Similar to NWS (same HRRR source)

---

## Recommendation

**Strongly recommended** as primary weather source for NDP:

1. **Free tier is generous** (10K/day vs ~50 calls needed)
2. **Integrated air quality** (no separate API)
3. **Historical data included** (1940+, free)
4. **High resolution** (3km HRRR for US)
5. **No API key hassle** for development
6. **Open source** - no vendor lock-in

**Main consideration:** Parser modification needed for parallel array format.
