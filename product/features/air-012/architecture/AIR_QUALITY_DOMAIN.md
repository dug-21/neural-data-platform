# Air Quality Domain Analysis: Window State Optimization

**Feature:** air-012 (Home Assistant Integration)
**Domain Specialist:** ndp-air-quality-specialist
**Location:** Coastal Florida (29.96N, 81.31W - near Atlantic coast)
**Date:** 2026-01-19

---

## Executive Summary

This document provides domain expertise for using window state data to optimize indoor air quality in a coastal Florida home. It defines the scientific thresholds, decision logic, and alert recommendations for determining when opening or closing windows will improve indoor air quality.

**Key Insight:** Window management is a multi-variable optimization problem balancing:
- Air quality (PM2.5, CO2, VOCs)
- Thermal comfort (temperature, humidity)
- Energy efficiency (HVAC load)
- Regional factors (salt air, humidity, seasonal patterns)

---

## 1. Data Sources and Available Metrics

### 1.1 Indoor Sensors (AirGradient ONE)

| Metric | Sensor | Range | Update Frequency |
|--------|--------|-------|------------------|
| PM2.5 (compensated) | Plantower PMS5003 | 0-500 ug/m3 | ~2 min |
| PM10 | Plantower PMS5003 | 0-500 ug/m3 | ~2 min |
| CO2 | SenseAir S8 | 380-10,000 ppm | ~2 min |
| Temperature | Sensirion SHT40 | -10 to 50C | ~2 min |
| Humidity | Sensirion SHT40 | 0-100% | ~2 min |
| TVOC Index | Sensirion SGP41 | 1-500 | ~2 min |
| NOx Index | Sensirion SGP41 | 1-500 | ~2 min |

**Note:** AirGradient provides both raw and compensated values for PM2.5 and temperature. Use compensated values when available as they apply humidity correction.

### 1.2 Outdoor Data (OpenWeatherMap APIs)

**Current Weather (outdoor-weather stream):**
- Temperature (Celsius)
- Humidity (%)
- Wind speed/direction
- Precipitation
- Visibility
- Cloud cover

**Air Quality (outdoor-air-quality stream):**
- AQI (1-5 OWM scale, converted to EPA 0-500)
- PM2.5, PM10 (ug/m3)
- CO, NO, NO2, O3, SO2, NH3 (ug/m3)

### 1.3 Window State (Home Assistant via air-012)

| Entity | Type | States |
|--------|------|--------|
| Binary contact sensors | binary_sensor | on/off (open/closed) |

---

## 2. Air Quality Decision Factors

### 2.1 When Is Outdoor Air "Better" Than Indoor?

The primary decision hinges on comparing indoor vs outdoor pollutant levels while accounting for infiltration effects.

#### 2.1.1 PM2.5 Comparison Logic

```
Outdoor is BETTER when:
  outdoor_pm25 < (indoor_pm25 * 0.7)

Rationale:
  - Opening windows doesn't instantly equalize air
  - Outdoor air must be significantly better (30%+ lower)
    to achieve meaningful improvement
  - Accounts for infiltration losses at window boundary
```

**PM2.5 Threshold Table:**

| Indoor PM2.5 | Open Window If Outdoor < | Rationale |
|--------------|--------------------------|-----------|
| < 12 ug/m3 | Don't open for PM2.5 | Already in "Good" range |
| 12-35 ug/m3 | < 8 ug/m3 | Moderate indoor, need good outdoor |
| 35-55 ug/m3 | < 20 ug/m3 | Unhealthy sensitive, strong gradient |
| > 55 ug/m3 | < 35 ug/m3 | Unhealthy, any improvement helps |

#### 2.1.2 AQI-Based Decision

Using EPA AQI categories (converted from OWM 1-5 scale):

| EPA AQI | OWM AQI | Category | Window Recommendation |
|---------|---------|----------|----------------------|
| 0-50 | 1 | Good | FAVORABLE for opening |
| 51-100 | 2 | Moderate | CONDITIONAL (check humidity/temp) |
| 101-150 | 3 | Unhealthy for Sensitive | AVOID opening unless indoor worse |
| 151-200 | 4 | Unhealthy | DO NOT OPEN |
| 201-300 | 4-5 | Very Unhealthy | DO NOT OPEN |
| 301-500 | 5 | Hazardous | DO NOT OPEN - seal home |

### 2.2 CO2 Ventilation Requirements

CO2 is a key driver for ventilation need - it only increases indoors and requires fresh air to dilute.

#### 2.2.1 CO2 Thresholds and Actions

| CO2 Level | Category | Ventilation Need | Action |
|-----------|----------|------------------|--------|
| < 600 ppm | Excellent | None | Maintain current state |
| 600-800 ppm | Good | Low | No action required |
| 800-1000 ppm | Acceptable | Moderate | Consider ventilation |
| 1000-1200 ppm | Poor | High | **Open windows if conditions allow** |
| 1200-1500 ppm | Very Poor | Urgent | **Ventilate regardless of comfort** |
| > 1500 ppm | Inadequate | Critical | **Immediate ventilation required** |

**Scientific Basis:**
- ASHRAE Standard 62.1: 1000 ppm upper limit for acceptable IAQ
- Harvard T.H. Chan study: Cognitive scores decline 15% at 1000 ppm
- Lawrence Berkeley Lab: 50%+ cognitive decline above 2500 ppm
- Outdoor baseline: 410-420 ppm (2024 global average)

#### 2.2.2 CO2 Rate of Rise Prediction

```
CO2 accumulation rate (typical):
  - 1 person, sedentary: +15-20 ppm/hour
  - 1 person, active: +25-35 ppm/hour
  - 2 people, sedentary: +30-40 ppm/hour

Prediction logic:
  IF co2_trend > 30 ppm/hour AND co2_current > 700 ppm
  THEN alert "CO2 rising - ventilation needed in ~X minutes"
  WHERE X = (1000 - co2_current) / co2_trend * 60
```

### 2.3 Temperature/Humidity Comfort Trade-offs

#### 2.3.1 Comfort Zone Definition (ASHRAE Standard 55)

```
Comfort zone (summer, typical clothing):
  Temperature: 23-26C (73-79F)
  Humidity: 30-60% RH

Extended acceptable (with air movement):
  Temperature: 20-28C (68-82F)
  Humidity: 25-70% RH
```

#### 2.3.2 Temperature Decision Logic

```python
def temperature_favorable(indoor_temp, outdoor_temp, target_temp=24):
    """
    Returns True if opening windows moves indoor temp toward target.
    """
    if indoor_temp < target_temp:
        # Room is cool - outdoor should be warmer
        return outdoor_temp > indoor_temp and outdoor_temp <= target_temp + 2
    else:
        # Room is warm - outdoor should be cooler
        return outdoor_temp < indoor_temp and outdoor_temp >= target_temp - 2
```

#### 2.3.3 Humidity Impact Assessment

**Florida-Specific Challenge:** High outdoor humidity (often 70-95% RH) limits window ventilation opportunities.

| Outdoor RH | Recommendation | Florida Context |
|------------|----------------|-----------------|
| < 50% | FAVORABLE | Rare - typically winter mornings |
| 50-70% | ACCEPTABLE | Spring/fall, early morning |
| 70-80% | MARGINAL | Common year-round |
| > 80% | AVOID | Risk of condensation, mold |

**Humidity Trade-off Logic:**
```
IF outdoor_rh > 75% AND indoor_rh < 60%
THEN DO NOT OPEN (will increase indoor humidity)

IF outdoor_rh < indoor_rh AND indoor_rh > 65%
THEN OPEN (will reduce indoor humidity)
```

### 2.4 Florida-Specific Considerations

#### 2.4.1 Salt Air (Coastal Location)

- **Impact:** Salt-laden air can corrode electronics, HVAC systems
- **Mitigation:** Monitor wind direction - onshore winds (E, SE) carry more salt
- **Threshold:** Wind > 15 mph from ocean + visibility < 10km = high salt content
- **Recommendation:** Avoid prolonged window opening during strong onshore winds

#### 2.4.2 Pollen Seasons

| Season | Primary Pollen | Severity | Typical Dates |
|--------|---------------|----------|---------------|
| Winter/Spring | Oak, Cedar, Pine | High | Feb-May |
| Late Spring | Grass | Moderate | May-Jun |
| Summer | Grass, Ragweed | Moderate | Jun-Sep |
| Fall | Ragweed, Mold | Moderate | Sep-Nov |

**Pollen Advisory Logic:**
```
IF pollen_forecast = HIGH AND occupant_allergies = true
THEN reduce_window_open_duration
     OR prefer_mechanical_ventilation
```

#### 2.4.3 Wildfire Smoke Events

Florida experiences occasional wildfire smoke, particularly in dry seasons.

```
IF aqi_owm >= 4 (>150 EPA AQI)
OR pm25_outdoor > 55 ug/m3
THEN CLOSE_ALL_WINDOWS
     SET alert = "Air quality advisory - keep windows closed"
     RECOMMEND run_air_purifier
```

---

## 3. Window Opening Impact Analysis

### 3.1 Expected Air Quality Response Curves

#### 3.1.1 PM2.5 Equilibration

When opening windows with significant indoor/outdoor gradient:

```
Time to 50% equilibration: 15-30 minutes
Time to 90% equilibration: 45-90 minutes

Factors affecting rate:
  - Window area / room volume ratio
  - Wind speed (faster = quicker mixing)
  - Temperature differential (drives convection)
  - Number of windows (cross-ventilation)
```

**Typical PM2.5 Response (opening window, outdoor = 5 ug/m3, indoor = 30 ug/m3):**

| Time (min) | Indoor PM2.5 | % Reduction |
|------------|--------------|-------------|
| 0 | 30.0 | 0% |
| 15 | 22.0 | 27% |
| 30 | 16.0 | 47% |
| 45 | 12.0 | 60% |
| 60 | 9.5 | 68% |
| 90 | 7.0 | 77% |

#### 3.1.2 CO2 Dilution Dynamics

CO2 responds faster due to higher diffusion rate:

```
Time to 50% reduction: 10-20 minutes
Time to 90% reduction: 30-60 minutes

Calculation:
  air_changes_per_hour (ACH) = f(window_area, wind_speed, temp_diff)
  co2_decay_rate = ACH * (indoor_co2 - outdoor_co2)
```

**Typical CO2 Response (opening window, indoor = 1200 ppm, outdoor = 420 ppm):**

| Time (min) | Indoor CO2 | ACH Equivalent |
|------------|------------|----------------|
| 0 | 1200 | - |
| 10 | 900 | ~3 ACH |
| 20 | 700 | ~3 ACH |
| 30 | 550 | ~3 ACH |
| 45 | 480 | ~3 ACH |

#### 3.1.3 Temperature Response

Slowest to change due to thermal mass:

```
Time to 50% equilibration: 30-60 minutes
Time to 90% equilibration: 2-4 hours

Thermal mass factors:
  - Furniture, walls, floors retain heat
  - HVAC system may counteract changes
  - Humidity affects perceived temperature
```

### 3.2 Diminishing Returns Analysis

**Key Finding:** Most benefit occurs in first 30-45 minutes.

| Duration | CO2 Benefit | PM2.5 Benefit | Recommendation |
|----------|-------------|---------------|----------------|
| 0-15 min | 25% | 25% | Minimum useful duration |
| 15-30 min | 50% | 45% | Good short ventilation |
| 30-60 min | 75% | 65% | Optimal duration |
| 60-120 min | 90% | 80% | Extended ventilation |
| > 120 min | 95%+ | 85%+ | Diminishing returns |

**Recommendation Logic:**
```
optimal_duration = CASE
  WHEN co2_initial > 1500 ppm THEN 60 minutes
  WHEN co2_initial > 1200 ppm THEN 45 minutes
  WHEN co2_initial > 1000 ppm THEN 30 minutes
  WHEN pm25_ratio > 3 THEN 45 minutes  -- large gradient
  ELSE 20 minutes
END
```

---

## 4. Multi-Window Strategy

### 4.1 Cross-Ventilation Benefits

Opening windows on opposite sides creates airflow:

```
Single window:  ACH ~ 1-3 (stack effect only)
Cross-ventilation: ACH ~ 5-15 (wind-driven)
```

**Cross-Ventilation Conditions:**
- Minimum 2 windows on different walls
- Wind speed > 2 m/s (4.5 mph)
- Windows should create airflow path through room

### 4.2 Room-Specific Considerations

| Room Type | Priority Metric | Notes |
|-----------|-----------------|-------|
| Bedroom | CO2, temperature | Nighttime ventilation important |
| Kitchen | PM2.5, VOCs | Cooking creates pollutants |
| Bathroom | Humidity, VOCs | Exhaust preferred over window |
| Living room | CO2, PM2.5 | Occupancy-driven |
| Home office | CO2 | Cognitive performance critical |

### 4.3 Time-of-Day Patterns (Florida)

#### 4.3.1 Morning (6-10 AM)
- **Temperature:** Coolest of day, favorable
- **Humidity:** Often lower before evapotranspiration peaks
- **Air Quality:** Usually best before traffic/heat
- **Recommendation:** OPTIMAL window opening time

#### 4.3.2 Midday (10 AM - 4 PM)
- **Temperature:** Hot (summer: 30-35C)
- **Humidity:** Building
- **Air Quality:** Ozone may peak
- **Recommendation:** Typically AVOID opening

#### 4.3.3 Evening (4-8 PM)
- **Temperature:** Cooling but still warm
- **Humidity:** Often high
- **Air Quality:** Traffic pollution may peak
- **Recommendation:** CONDITIONAL - check all factors

#### 4.3.4 Night (8 PM - 6 AM)
- **Temperature:** Cooler
- **Humidity:** Often highest (land-sea breeze reversal)
- **Air Quality:** Generally good (low traffic/activity)
- **Recommendation:** Good for CO2 if humidity acceptable

---

## 5. Alert and Recommendation Thresholds

### 5.1 "Open Window Now" Conditions

Trigger immediate recommendation to open windows:

```sql
-- OPEN_WINDOW_NOW alert condition
SELECT CASE
  WHEN
    -- CO2 is critical
    (indoor_co2 > 1200
     AND outdoor_aqi_epa < 100
     AND outdoor_humidity_pct < 80
     AND NOT is_raining)
  OR
    -- Indoor PM2.5 is unhealthy, outdoor is good
    (indoor_pm25 > 55
     AND outdoor_pm25 < 20
     AND outdoor_aqi_epa < 75)
  OR
    -- VOC event (cooking, cleaning)
    (indoor_tvoc_index > 250
     AND outdoor_aqi_epa < 100
     AND outdoor_humidity_pct < 80)
  THEN 'OPEN_WINDOW_NOW'
  ELSE NULL
END AS alert_type
```

**Alert Message Template:**
```
OPEN WINDOWS RECOMMENDED

Reason: [CO2 at {value} ppm / PM2.5 at {value} ug/m3 / VOC event detected]
Outdoor conditions: AQI {value}, {temp}C, {humidity}% RH
Estimated improvement time: {duration} minutes
```

### 5.2 "Close Window Now" Conditions

Trigger immediate recommendation to close windows:

```sql
-- CLOSE_WINDOW_NOW alert condition
SELECT CASE
  WHEN
    -- Outdoor air quality deteriorating
    (outdoor_aqi_epa > 100
     OR outdoor_pm25 > indoor_pm25 * 1.2)
  OR
    -- Weather event
    (is_raining
     OR wind_speed_kmh > 40
     OR outdoor_humidity_pct > 85)
  OR
    -- Temperature extreme
    (outdoor_temp_c > 32
     OR outdoor_temp_c < 18)
  OR
    -- Smoke/wildfire event
    (outdoor_pm25 > 55)
  THEN 'CLOSE_WINDOW_NOW'
  ELSE NULL
END AS alert_type
```

**Alert Message Template:**
```
CLOSE WINDOWS RECOMMENDED

Reason: [Outdoor AQI {value} / Rain detected / Temperature {value}C]
Current indoor: CO2 {value} ppm, PM2.5 {value} ug/m3
Action: Close windows and {use_air_purifier | run_hvac}
```

### 5.3 "Conditions Favorable for Ventilation" Advisory

Proactive notification when conditions are ideal:

```sql
-- FAVORABLE_FOR_VENTILATION advisory
SELECT CASE
  WHEN
    outdoor_aqi_epa <= 50
    AND outdoor_pm25 < 12
    AND outdoor_temp_c BETWEEN 20 AND 27
    AND outdoor_humidity_pct BETWEEN 40 AND 70
    AND wind_speed_kmh BETWEEN 5 AND 25
    AND NOT is_raining
    AND indoor_co2 > 700  -- some benefit available
  THEN 'FAVORABLE_FOR_VENTILATION'
  ELSE NULL
END AS advisory_type
```

**Advisory Message Template:**
```
VENTILATION CONDITIONS FAVORABLE

Outdoor: AQI {value} (Good), {temp}C, {humidity}% RH
Benefit: CO2 reduction from {indoor_co2} to ~{target_co2} ppm
Recommended duration: {duration} minutes
Best windows: {room_list} (cross-ventilation available)
```

### 5.4 Health-Based Alert Hierarchy

| Priority | Condition | Alert Level | Action |
|----------|-----------|-------------|--------|
| 1 | Outdoor AQI > 200 | CRITICAL | Seal home, run purifiers |
| 2 | Indoor CO2 > 1500 ppm | HIGH | Ventilate immediately |
| 3 | Indoor PM2.5 > 55 ug/m3 | HIGH | Ventilate if outdoor better |
| 4 | Outdoor AQI > 150 | MODERATE | Close windows |
| 5 | Indoor CO2 > 1200 ppm | MODERATE | Plan ventilation |
| 6 | Indoor PM2.5 > 35 ug/m3 | LOW | Monitor, consider ventilation |
| 7 | Favorable conditions | INFO | Suggest window opening |

---

## 6. Seasonal Patterns (Florida)

### 6.1 Summer (June - September)

**Characteristics:**
- Temperature: 28-35C daily highs
- Humidity: 70-95% (often >80%)
- Afternoon thunderstorms (2-5 PM typical)
- Hurricane season peak

**Ventilation Strategy:**
```
Primary window: Early morning only (6-9 AM)
Secondary option: After evening thunderstorm (7-9 PM)
Avoid: Midday heat, pre-storm conditions

Recommendation threshold adjustments:
  - Raise humidity tolerance to 85% for brief ventilation
  - Shorter duration (20-30 min max due to humidity)
  - Prioritize CO2 over thermal comfort
```

### 6.2 Winter (December - February)

**Characteristics:**
- Temperature: 15-25C (mild, occasional cold fronts)
- Humidity: Lower (50-70% typical)
- Rare freeze events (1-3 per winter)
- Dry season - fire risk

**Ventilation Strategy:**
```
Optimal season for natural ventilation
Windows can remain open for extended periods
Watch for:
  - Wildfire smoke events (check AQI)
  - Pollen starting late February

Recommendation threshold adjustments:
  - Lower temperature threshold to 15C (Florida-adapted)
  - Extended duration OK (60-120 min)
  - Cross-ventilation highly effective
```

### 6.3 Spring (March - May)

**Characteristics:**
- Temperature: 20-30C (warming)
- Humidity: Variable (50-80%)
- Heavy pollen season (oak, pine)
- Increasing afternoon storms

**Ventilation Strategy:**
```
Morning ventilation preferred
Pollen check required before opening
Monitor oak pollen forecasts

Recommendation threshold adjustments:
  - Include pollen advisory in decision
  - Reduce duration during high pollen (15-20 min)
  - Consider evening over morning if pollen high
```

### 6.4 Fall (October - November)

**Characteristics:**
- Temperature: 22-28C (cooling)
- Humidity: Decreasing (60-75%)
- Hurricane season winding down
- Ragweed pollen peaks

**Ventilation Strategy:**
```
Excellent ventilation conditions
Similar to winter strategy
Watch for late-season tropical systems

Recommendation threshold adjustments:
  - Standard thresholds apply
  - Extended ventilation opportunities
  - Monitor ragweed for allergy sufferers
```

---

## 7. Implementation Recommendations

### 7.1 Data Requirements

| Source | Metric | Poll Interval | Silver Table |
|--------|--------|---------------|--------------|
| AirGradient | pm25, co2, temp, humidity, tvoc | 2 min | silver.air_quality_readings |
| OWM Weather | temp, humidity, wind, rain | 10 min | silver.weather_observations |
| OWM AQ | aqi, pm25, pm10, o3 | 10 min | silver.outdoor_air_quality |
| Home Assistant | window_state | Event-driven | silver.window_events |

### 7.2 Derived Metrics (Silver/Gold Layer)

```sql
-- Ventilation favorability score (0-100)
ventilation_score =
  (100 - outdoor_aqi_epa) * 0.3 +          -- Air quality weight
  LEAST(100, 100 - ABS(outdoor_temp - 24) * 10) * 0.25 +  -- Temperature weight
  LEAST(100, 100 - (outdoor_humidity - 50) * 2) * 0.25 +  -- Humidity weight
  CASE WHEN indoor_co2 > 1000 THEN 20 ELSE (indoor_co2 - 600) / 20 END * 0.20  -- Need weight

-- Indoor/Outdoor differential
iod_pm25 = indoor_pm25 - outdoor_pm25
iod_favorable = CASE WHEN iod_pm25 > 10 THEN true ELSE false END
```

### 7.3 Alert Implementation

Recommended alert channels:
1. **Home Assistant notification** - Push to mobile
2. **Grafana alert** - Dashboard visibility
3. **MQTT publish** - Automation trigger

```yaml
# Home Assistant automation trigger example
trigger:
  - platform: numeric_state
    entity_id: sensor.indoor_co2
    above: 1200
    for: "00:05:00"
condition:
  - condition: numeric_state
    entity_id: sensor.outdoor_aqi_epa
    below: 100
action:
  - service: notify.mobile
    data:
      message: "CO2 high ({{ states('sensor.indoor_co2') }} ppm) - open windows"
```

### 7.4 Machine Learning Opportunities

Future enhancements for air-012 and beyond:

1. **Equilibration time prediction** - Learn actual response curves per room
2. **Occupancy-aware CO2 prediction** - Forecast CO2 based on presence
3. **Optimal window schedule** - Learn best times based on historical success
4. **Personalized comfort zones** - Adapt thresholds to household preferences

---

## 8. References

### Scientific Literature
- ASHRAE Standard 62.1-2022: Ventilation for Acceptable Indoor Air Quality
- ASHRAE Standard 55-2020: Thermal Environmental Conditions for Human Occupancy
- EPA: Air Quality Index (AQI) Basics
- Harvard T.H. Chan School: COGfx Study (Allen et al., 2016)
- Lawrence Berkeley National Laboratory: Ventilation and Cognitive Function

### Regulatory Standards
- EPA NAAQS (2024): PM2.5 annual standard = 9.0 ug/m3
- OSHA: 5000 ppm CO2 8-hour TWA limit (occupational)
- WHO: PM2.5 annual guideline = 5 ug/m3 (2021)

### NDP Internal References
- `config/base/streams/outdoor-air-quality/config.yaml` - OWM air quality config
- `config/base/streams/outdoor-weather/config.yaml` - OWM weather config
- `product/features/air-001/` - AirGradient sensor implementation
- `domains/air-quality/src/validation.rs` - Sensor validation ranges

---

## Appendix A: Quick Reference Card

### Open Windows When:
- Indoor CO2 > 1000 ppm AND outdoor AQI < 100
- Indoor PM2.5 > outdoor PM2.5 * 1.5 AND outdoor AQI < 75
- Indoor TVOC > 200 AND outdoor humidity < 80%
- Morning (6-10 AM) with outdoor temp 20-27C

### Close Windows When:
- Outdoor AQI > 100 (EPA scale)
- Outdoor PM2.5 > 35 ug/m3
- Outdoor humidity > 85%
- Rain, thunderstorm, or wind > 40 km/h
- Outdoor temp > 32C or < 18C

### Florida-Specific Rules:
- Summer: Morning only (6-9 AM)
- Hurricane watch: Close all windows
- Spring: Check oak pollen before opening
- Onshore wind + low visibility = high salt content
