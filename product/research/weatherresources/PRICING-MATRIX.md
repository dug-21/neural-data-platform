# Weather API Pricing Matrix

*Research Date: 2025-12-23*

## Free Tier Comparison

| Provider | Calls/Day | Calls/Month | Forecast Days | Historical | Air Quality | Commercial OK |
|----------|-----------|-------------|---------------|------------|-------------|---------------|
| **Open-Meteo** | 10,000 | ~300,000 | 16 | 1940+ | ✅ | ❌ (non-commercial) |
| **WeatherAPI.com** | 33,333 | 1,000,000 | 3 | 7 days | Limited | ❌ |
| **NWS** | Unlimited | Unlimited | 7 | ❌ | ❌ | ✅ |
| **OpenWeatherMap** | 1,000 | 30,000 | 5 | ❌ | ❌ | ❌ |
| **Weatherbit** | 50 | 1,500 | 7 (daily) | ❌ | ❌ | ❌ |
| **Tomorrow.io** | 500 | 15,000 | 5 | ❌ | ❌ | ❌ |
| **Visual Crossing** | 1,000 | 30,000 | 15 | 50 years | ❌ | ✅ (unique!) |

---

## Entry-Level Paid Tiers

| Provider | Plan | Price/Month | Calls/Day | Calls/Month | $/1K Calls |
|----------|------|-------------|-----------|-------------|------------|
| **WeatherAPI** | Starter | $7 | 100,000 | 3,000,000 | $0.002 |
| **Open-Meteo** | Commercial | $29 | ~33,333 | 1,000,000 | $0.029 |
| **Weatherstack** | Standard | $10 | 50,000 | 1,500,000 | $0.007 |
| **OpenWeatherMap** | Developer | $40 | 3,000 | 90,000 | $0.44 |
| **Weatherbit** | Standard | $49 | 25,000 | 750,000 | $0.065 |
| **Visual Crossing** | Professional | $35 | 10,000 | 300,000 | $0.12 |
| **Tomorrow.io** | N/A | B2B Only | - | - | - |

**Best Value:** WeatherAPI.com at $0.002 per 1K calls

---

## Mid-Tier Options ($25-100/month)

| Provider | Plan | Price | Calls/Day | Key Features |
|----------|------|-------|-----------|--------------|
| **WeatherAPI** | Business | $35 | 500,000 | 14-day forecast, marine data |
| **Weatherbit** | Professional | $99 | 50,000 | Minutely, 16-day forecast |
| **AerisWeather** | Developer | $23 | 10,000 | Severe alerts, lightning |
| **AccuWeather** | Standard | $25 | 5,000 | Strong brand, 45-day |
| **Visual Crossing** | Corporate | $75 | 100,000 | Full historical archive |

---

## Enterprise Tiers

| Provider | Starting Price | Notes |
|----------|----------------|-------|
| **Tomorrow.io** | ~$150-2000/mo | Contact sales only |
| **Meteomatics** | Custom | 90m resolution, expensive |
| **AccuWeather** | Custom | Largest station network |
| **Weatherbit** | $495/mo+ | 25-year historical |

---

## Cost Analysis for NDP

### Current Usage Estimate
- **Locations:** 1-5 sensors
- **Poll frequency:** Hourly
- **Calls needed:** 24 × 5 = 120 calls/day (weather)
- **With air quality:** 240 calls/day

### Recommended Budget Allocation

| Budget | Strategy |
|--------|----------|
| **$0/month** | Open-Meteo (10K/day) or WeatherAPI free (33K/day) |
| **$7/month** | WeatherAPI Starter (100K/day, AQI included) |
| **$29/month** | Open-Meteo Commercial (commercial license) |
| **$56/month** | WeatherAPI ($7) + Weatherbit ($49) for hyperlocal |

---

## Feature vs Price Positioning

```
                    FEATURES
                       ↑
    Weatherbit $49     │     Tomorrow.io $$$$
    (hyperlocal)       │     (minutely, rich)
                       │
    ───────────────────┼───────────────────→ PRICE
                       │
    Open-Meteo $0-29   │     OpenWeatherMap $40
    (best value)       │     (overpriced)
                       │
    WeatherAPI $7      │
    (budget king)      │
```

---

## Decision Tree

```
Need hyperlocal (30m) accuracy?
├── Yes → Weatherbit ($49/mo)
└── No
    └── Need integrated air quality?
        ├── Yes → WeatherAPI ($7/mo)
        └── No
            └── Need commercial license?
                ├── Yes → Open-Meteo ($29/mo) or Visual Crossing ($35/mo)
                └── No → Open-Meteo Free (10K/day)
```

---

## NDP Recommendation

**Phase 1 (Now - Free):**
- Open-Meteo free tier for testing
- Validate accuracy against sensors

**Phase 2 (Production - $7/mo):**
- WeatherAPI.com Starter
- Includes AQI data for correlation analysis

**Phase 3 (If needed - $56/mo):**
- Add Weatherbit for hyperlocal precipitation
- Dual-provider redundancy
