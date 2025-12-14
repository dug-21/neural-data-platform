# Indoor Air Quality Monitoring and Analytics: Best Practices Research

**Research Date:** December 13, 2025
**Compiled for:** Neural Data Platform - Air Quality Analytics Module

---

## Executive Summary

This document provides comprehensive research on current best practices for indoor air quality (IAQ) monitoring and analytics, covering international standards, health-based thresholds, derived metrics, event detection patterns, time-series forecasting, available datasets, and sensor specifications for the AirGradient ONE platform.

---

## 1. Indoor AQI Calculation Standards (2024-2025)

### 1.1 EPA Standards and Updates

**2024 PM2.5 Standard Revision:**
- In February 2024, the U.S. EPA lowered the annual PM2.5 standard from 12 µg/m³ to **9 µg/m³**
- The 24-hour PM2.5 standard remains at **35 µg/m³**
- EPA announced plans to revise AQI breakpoints to match the new standard

**AQI Calculation Methodology:**
- AQI based on five criteria pollutants: ground-level ozone, particulate matter, carbon monoxide, sulfur dioxide, and nitrogen dioxide
- Uses linear interpolation between pollutant breakpoints
- The "dominant" (highest) AQI value is reported when multiple pollutants are measured
- **NowCast** surrogate concentration used for PM2.5, PM10, and ozone reporting - a weighted average giving more weight to recent data during changing conditions

**AQI Categories:**
1. Good (0-50)
2. Moderate (51-100)
3. Unhealthy for Sensitive Groups (101-150)
4. Unhealthy (151-200)
5. Very Unhealthy (201-300)
6. Hazardous (301+)

### 1.2 Indoor vs. Outdoor AQI

**Key Distinctions:**
- IAQI (Indoor Air Quality Index) is not a government standard
- Adapts EPA's ratio method for indoor conditions
- Outdoor AQI reports 1-24 hour averages; IAQI designed for minute-by-minute decisions
- IAQI includes indoor-relevant metrics like **CO2 and TVOC** not in EPA outdoor AQI
- Clean Air Act defines ambient air as "air external to buildings"

### 1.3 WHO and International Standards

- WHO released updated global air quality standards database in early 2025
- More than 50 organizations across 38 countries have established IAQ guidelines
- U.S. states (California, Illinois, Texas) have established some IAQ guidelines
- EPA has set indoor guideline for radon specifically

**Implementation Recommendation:**
Develop multi-tiered IAQI calculation using:
- PM2.5 (9 µg/m³ annual, 35 µg/m³ 24-hour targets)
- CO2 (< 1000 ppm standard)
- TVOC (< 0.5 mg/m³ acceptable)
- Temperature/humidity (mold risk indices)

---

## 2. Health-Based Thresholds

### 2.1 CO2 and Cognitive Impact

**Standard Thresholds:**
- **1000 ppm** - Most common global guideline limit (found in 35 of 43 guidelines surveyed)
- **800 ppm** - Recommended target for good IAQ
- **400 ppm** - Typical outdoor baseline
- **1100 ppm** - ASHRAE upper limit (outdoor + 700 ppm)

**Cognitive Impact Research:**
- Decline in decision-making performance starts at ~1000 ppm
- **165% decrease** in attention test performance at 1000-2000 ppm
- At 2,500 ppm: large, statistically significant reductions in seven decision-making scales
- 1000-2000 ppm: drowsiness, fatigue, stuffy air perception
- Reduced concentration and impaired decision-making abilities

**Guideline Basis (2024 Review):**
- 16 guidelines: no specific human effect mentioned
- 19 guidelines: specify odor dissatisfaction
- 5 guidelines: specify non-infectious health effects
- 3 guidelines: specify airborne infectious disease transmission

**Implementation Thresholds:**
- Green (< 800 ppm): Excellent
- Yellow (800-1000 ppm): Good
- Orange (1000-1500 ppm): Moderate - cognitive impact likely
- Red (> 1500 ppm): Poor - significant cognitive impairment

### 2.2 PM2.5 Cardiovascular Effects

**EPA Standards:**
- Primary annual standard: **9.0 µg/m³** (updated 2024)
- 24-hour standard: **35 µg/m³**

**Health Effects:**
- PM2.5 accounts for 80% of all premature deaths caused by air pollution
- Short-term exposure: acute cardiovascular deaths, heart attacks, strokes
- Long-term exposure: increased cardiovascular death risk, reduced longevity
- Promotes: hypertension, atherosclerosis, heart failure, arrhythmias

**Critical Finding - No Safe Threshold:**
- Increased cardiovascular risk **even below 12 µg/m³**
- Per 10 µg/m³ increase below 12 µg/m³ threshold:
  - CVD mortality: HR 2.31
  - Stroke: HR 1.41
  - Acute myocardial infarction: HR 1.51

**Indoor PM2.5 Sources:**
- Indoor-generated: smoking, cooking, cleaning
- Infiltrated outdoor PM2.5
- Canadian studies: < 15 µg/m³ (non-smoking homes), < 35 µg/m³ (smoking homes)
- Generally lower indoors than outdoors, except in homes with smokers

**Implementation Thresholds:**
- Green (< 9 µg/m³): Excellent
- Yellow (9-12 µg/m³): Good
- Orange (12-35 µg/m³): Moderate
- Red (35-55 µg/m³): Unhealthy for sensitive groups
- Purple (> 55 µg/m³): Unhealthy

### 2.3 VOC Exposure Limits

**General Guidelines:**
- **No federally enforceable standards** for VOCs in non-industrial U.S. settings
- Overall safe concentration: **< 0.5 mg/m³** total VOCs

**Specific VOC Threshold Limit Values (TLVs):**
- Formaldehyde: **0.1-0.3 ppm** (OSHA PEL: 0.75 ppm, action level: 0.5 ppm)
- Benzene: **0.1 ppm** (WHO: no safe level - carcinogen)
- Ethanol: **1,000 ppm**
- Acetone: **750-1,000 ppm**

**Health Concerns:**
- VOC concentrations consistently **up to 10x higher indoors** than outdoors
- Health issues: eye/nose/throat irritation, headaches, dizziness
- Long-term: respiratory and nervous system effects
- Exacerbates asthma symptoms

**International Variations:**
- China benzene threshold: ≤ 0.03 mg/m³ (1-hour exposure)
- Formaldehyde (>8 hour exposure): 7-40 ppb in various guidelines

**Implementation Note:**
The Sensirion SGP41 sensor provides **relative VOC index (1-500)** rather than absolute concentration:
- Index 100 = average indoor composition (24-hour baseline)
- Index > 100 = deteriorating air quality
- Cannot distinguish between VOC types

---

## 3. Derived Metrics and Analytics

### 3.1 Ventilation Adequacy (ACH from CO2 Decay)

**Air Changes Per Hour (ACH):**
- Definition: Number of times total air volume is completely replaced per hour
- **63.2%** of air actually changed after 1 hour at 1 ACH (in well-mixed scenario)

**Calculation Methods:**

**Basic ACH Formula:**
```
ACH = (CFM × 60) / (Area × Height)
ACH = airflow (m³/h) / room volume (m³)
```

**CO2 Decay Method:**
- Most common technique: Tracer Gas Decay (step-down)
- Exponential decay after occupants leave directly proportional to air change rate
- Requires: 2 CO2 readings, time between readings (ideally 3-8 hours), baseline CO2 estimate
- Single air change removes **63%** of airborne contaminants

**Practical Application:**
- 800 ppm at occupancy with 4 ACH target = adequate
- 1400 ppm consistently = below target ACH
- Can indicate ventilation inadequacy

**Important Considerations:**
- Steady-state CO2 can range 700-5000 ppmv
- Single CO2 concentration not justified as universal ventilation adequacy indicator
- Assumptions required: constant/uniform CO2 generation, uniform concentration at steady state, known constant outdoor CO2, constant outdoor air ventilation rate
- Transient mass balance method most flexible for low ACH and dynamic occupancy

**Implementation:**
Implement CO2 decay analysis during unoccupied periods to estimate:
- ACH rate for the space
- HVAC system effectiveness
- Ventilation adequacy alerts

### 3.2 Mold Risk Indices

**Temperature and Humidity Thresholds:**
- **> 60% RH**: Mold growth risk begins
- **> 80% RH**: Risk significantly increases
- Ideal range: **30-50% RH**

**Temperature-Dependent Risk:**
- 80°F (27°C): Risk begins at 65% RH
- 60°F (16°C): Risk begins at 72% RH
- Mold growth range: 32°F (0°C) to 95°F (35°C)
- Optimal growth: 70-80°F (21-27°C)
- Some molds grow without liquid water at chronic RH > 60%

**VTT Mold Growth Model:**
- Mathematical model based on surface RH and temperature readings on wood
- Provides mold index from environmental readings
- Developed in controlled laboratory conditions

**Key Factors:**
- Temperature and RH have **complementary relationship** in bioaerosol concentrations
- Independent, combined, and interactive effects on microorganism survival/growth
- Warmer room = lower relative humidity (all else constant)
- Cold surfaces + humid air = condensation

**Material-Specific Behavior:**
- Bamboo: fully covered at RH ≥ 85%
- Reducing RH from 95% to 75%: delayed mold germination by ~70 days

**Health Impacts:**
- Indoor mould exposure linked to allergic and non-allergic diseases
- Higher risk for children, elderly, immunocompromised
- Can aggravate respiratory and allergic conditions

**Implementation:**
Calculate mold risk index using:
- Temperature
- Humidity
- Surface temperatures (if available)
- Insulation/thermal resistance values
- Time-weighted exposure > 60% RH

**Alert Levels:**
- Green: < 60% RH
- Yellow: 60-65% RH
- Orange: 65-80% RH
- Red: > 80% RH (time-weighted over 24h)

### 3.3 Thermal Comfort Indices (PMV/PPD)

**Predicted Mean Vote (PMV):**
- Developed by Ole Fanger (1970s)
- Scale: -3 (cold) to +3 (hot)
- Based on heat exchange between body and environment
- PMV = 0 represents thermal neutrality
- Comfort zone: **-0.5 < PMV < +0.5**

**Predicted Percentage Dissatisfied (PPD):**
- Percentage of people dissatisfied with thermal conditions
- Range: 5-100%
- At PMV = 0 (ideal conditions): **~5% still dissatisfied**
- Target: PPD < 10%

**Standards:**
- **ASHRAE Standard 55**: PMV between -0.5 and +0.5 (PPD < 10%)
- **ISO 7730**:
  - Hard limit: -2 to +2
  - Existing buildings: -0.7 to +0.7
  - New buildings: -0.5 to +0.5

**Input Parameters:**
1. **Environmental:**
   - Air temperature
   - Mean radiant temperature
   - Relative humidity
   - Air speed

2. **Personal:**
   - Clothing insulation (Clo)
   - Metabolic rate (Met)

**Model Limitations:**
- Prediction accuracy only 34% using world's largest thermal comfort database
- Does not account for adaptation mechanisms
- Does not consider outdoor thermal conditions
- Global application but regional variation exists

**Benefits:**
- Improved occupant satisfaction
- Enhanced productivity
- Reduced absenteeism
- Energy-efficient HVAC operation

**Implementation Note:**
With AirGradient ONE sensors (temperature + humidity only), can calculate simplified comfort indices. Full PMV/PPD requires air velocity and radiant temperature sensors.

---

## 4. Event Detection Patterns

### 4.1 Cooking Detection (PM2.5 Spike Signatures)

**Typical Pattern:**
- **Start:** PM2.5 increases 2-4 min after range turned on (0-2 min after food added)
- **Peak:** 1-7 min after cooking ends
- **Peak levels:** 200-1400 µg/m³
- **Decay:** Gradual return to background (< 1 hour to > 6 hours)

**Cooking Method Variations (Median Peak PM2.5):**
1. Pan-frying: **92.9 µg/m³**
2. Stir-frying: **26.7 µg/m³**
3. Deep-frying: **7.7 µg/m³**
4. Boiling: **0.7 µg/m³**
5. Air-frying: **0.6 µg/m³**

**Factors Affecting Emissions:**
- Oil-based cooking produces higher PM2.5
- High-fat meats > fish
- Water-based methods produce minimal PM2.5

**Decay Characteristics:**
- Mean source strength: **36 mg/min** (median: 12 mg/min)
- Mean decay rate: **0.27 h⁻¹** (median: 0.17 h⁻¹)
- Mean cooking duration: **11 min** (median: 7 min)

**Machine Learning Detection:**
- Researchers quantify stove hood use via ML detection based on particle counts
- Low-cost sensors respond rapidly to heating oil in kitchen
- Can track both stove and hood usage

**Mitigation Effectiveness:**
- Range hood significantly reduces peak concentrations
- Opening windows/doors effective
- Portable air cleaners: 48-78% reduction

**Health Context:**
- Cooking emissions account for **up to 73%** of particle surface area in homes
- Highest in-home exposure during cooking activities
- WHO: 4.3 million deaths (2012) attributable to household air pollution

**Implementation:**
Develop cooking event detector:
1. Rapid PM2.5 rise (> 10 µg/m³ in 5 min)
2. Peak within 10-15 minutes
3. Exponential decay pattern
4. Correlation with time-of-day patterns (meal times)
5. Optional: VOC spike correlation

### 4.2 Wildfire Smoke Infiltration

**Indoor/Outdoor PM2.5 Ratio Analysis:**

**Normal vs. Wildfire Days:**
- Non-fire days: Geometric mean infiltration ratio = **0.4**
- Wildfire days: Geometric mean infiltration ratio = **0.2** (50% reduction)
- Interpretation: Buildings provide better protection during wildfires

**Infiltration Factors by Building Type:**
- Seattle residences: **0.33-0.76** (mean range across 7 homes)
- Healthcare facility (typical): **0.32** (range: 0.22-0.39)
- Healthcare facility (smoke episode): **0.37** (range: 0.31-0.47) - **19% increase**

**HVAC Filter Impact:**
- MERV13 filters: I/O ratio = **0.12 ± 0.07**
- MERV8 filters: I/O ratio = **0.28 ± 0.14**
- Two-stage particle filtration + HVAC: Indoor = 21 µg/m³, I/O = **0.27**
- Natural ventilation only: Indoor = 36 µg/m³, I/O = **0.67**

**HEPA Filtration:**
- With filtration: I/O = **0.19**
- Without filtration: I/O = **0.61**
- Reduction: **48-78%** with portable HEPA cleaners

**Building Type Differences:**
- Residential: Lower I/O and correlation vs. commercial/school buildings
- Better penetration control in residential

**Detection Using Low-Cost Sensors:**
- AirGradient/PurpleAir sensors can track wildfire smoke dispersion
- Require smoke-specific adjustment factors for quantitative assessment
- Demonstrated utility in tracking impacts on susceptible populations

**Key Finding:**
Staying indoors with closed windows provides **33-76%** of outdoor levels - **insufficient protection** without air filtration

**Implementation:**
Wildfire smoke detector:
1. Monitor outdoor PM2.5 from public API (PurpleAir, EPA AirNow)
2. Calculate real-time I/O ratio
3. Alert when:
   - Outdoor PM2.5 > 55 µg/m³ (unhealthy for sensitive groups)
   - I/O ratio > 0.5 (poor infiltration protection)
4. Recommend air filtration when outdoor AQI > 150

### 4.3 Occupancy Inference from CO2 Rise Rates

**Sensing by Proxy Approach:**
- Infer latent factors (occupancy) via proxy measurements (CO2)
- Based on constitutive models exploiting spatial/physical features
- Uses coupled PDE-ODE system models

**Performance Metrics:**
- Mean squared error: **0.6044** fractional persons (CO2-based inference)
- Best alternative (Bayes net): **1.2061** fractional persons
- **50% improvement** over alternative methods

**CO2 Rise Rate Correlation:**
- Increase in CO2 over time correlates with CO2 generation rate per person
- Relatively constant for specific activities (examination, lecture)
- Rapid rise = re-entry or increased occupancy

**Machine Learning Methods:**
- Multi-sensor fusion: Temperature, humidity, CO2, sound, pressure, illumination
- Algorithms: MLP, GP with RBF, SVM, Random Forest, Naive Bayes
- CO2 + ventilation status + differential pressure data

**Advantages:**
- **Privacy-preserving** - no personal information required
- **Device-free** - no body-attached sensors
- Low cost, low intrusiveness
- Improves building energy efficiency and occupant comfort

**Challenges:**
- Non-uniform indoor CO2 concentration
- Measurement spikes from noise, irregular air movement, occupants near sensor
- Data smoothing necessary
- Naturally ventilated buildings: low accuracy due to complex ventilation behavior

**Typical CO2 Generation Rates:**
- Sedentary adult: ~0.005 L/s per person
- Light activity: ~0.010 L/s per person
- Moderate activity: ~0.015 L/s per person

**Implementation:**
Occupancy estimation algorithm:
1. Smooth CO2 data (moving average filter)
2. Calculate rise rate (ppm/minute during occupied periods)
3. Account for ventilation rate (ACH)
4. Estimate occupancy: `N = (dCO2/dt) × V / (G × (1 - η))`
   - N = number of occupants
   - dCO2/dt = rise rate
   - V = room volume
   - G = generation rate per person
   - η = ventilation efficiency
5. Train ML model on historical occupancy patterns
6. Validate with time-of-day priors

---

## 5. Time-Series Forecasting for Environmental Data

### 5.1 Seasonal Decomposition

**Time Series Components:**
- **Trend:** Long-term increase or decrease
- **Seasonal:** Regular patterns (daily, weekly, annual)
- **Cyclic:** Irregular fluctuations (not fixed frequency)
- **Residual:** Random variation

**Environmental Data Characteristics:**
- Clear **diurnal cycles** (24-hour periodicity)
- **Weekly seasonality** (weekday vs. weekend patterns)
- **Annual seasonality** (heating/cooling seasons)
- O3: very strong relational dependence every 24 hours

**Decomposition Methods:**
- Additive: Y(t) = T(t) + S(t) + R(t)
- Multiplicative: Y(t) = T(t) × S(t) × R(t)
- Group-by-day summarization removes diurnal fluctuations

### 5.2 HVAC Cycle Detection

**Machine Learning Approaches:**
- LSTM (Long Short-Term Memory) networks
- Random Forest
- Gradient Boosting Regressor
- Performance metrics: RMSE, MAE, MAPE

**Application Areas:**
- Energy consumption prediction
- Occupancy pattern analysis
- Environmental condition monitoring
- Demand response timing optimization

**Case Study - Carbon Emissions:**
- Linear diurnally seasonal AR model for demand response timing
- **20% improvement** in carbon emissions reduction
- Net reduction depends on carbon intensity at displaced operating time

**HVAC Signature Features:**
- Temperature setpoints and deadbands
- On/off cycling patterns
- Heating vs. cooling mode transitions
- Fresh air intake modulation
- Filter replacement cycles

### 5.3 Diurnal Pattern Recognition

**Characteristic Patterns:**
- **Temperature:** Lower at night, peaks mid-afternoon
- **CO2:** Rises during occupancy, decays when unoccupied
- **PM2.5:** Cooking spikes (breakfast, dinner), lower during sleep/work hours
- **Humidity:** Often inversely related to temperature
- **VOC:** Activity-related (cooking, cleaning, evening hours)

**Pattern Analysis Techniques:**
- Autocorrelation functions (ACF) for periodicity detection
- Spectral analysis for frequency domain patterns
- Wavelet decomposition for multi-scale patterns

### 5.4 Forecasting Methods

**Statistical Methods:**
- **ARIMA:** Trend and autocorrelation, lacks seasonal component
- **SARIMA:** Seasonal ARIMA for periodic patterns
- **Multi-seasonal state space models:** Multiple seasonal patterns
- **Linear diurnal seasonal AR:** For regular daily patterns

**Deep Learning Methods:**
- **RNN/LSTM:** Sequential dependencies, temporal patterns
- **CNN-LSTM hybrid:** Spatial features + temporal dependencies
- **GRU (Gated Recurrent Unit):** Lighter than LSTM
- **Transformer models:** Attention mechanisms for long sequences

**Ensemble Methods:**
- Combining multiple models for robust predictions
- Weighted averaging based on model performance
- Temporal cross-validation for model selection

**Performance Considerations:**
- Large datasets required (representative sample size)
- Autocorrelation, trend, seasonal variation must be modeled
- Patterns discovered must not be outliers
- Regular retraining on recent data

**Implementation Recommendations:**
1. **Short-term forecasting (1-6 hours):**
   - LSTM for CO2, temperature, humidity
   - Account for HVAC schedules and occupancy patterns

2. **Medium-term forecasting (6-24 hours):**
   - SARIMA for strong diurnal patterns
   - Feature engineering: hour of day, day of week, season

3. **Event-specific forecasting:**
   - Cooking events: Pattern matching + time-of-day priors
   - Occupancy: Historical patterns + calendar data

---

## 6. Open Datasets and Benchmarks

### 6.1 AQ-Bench (Global Air Quality Benchmark)

**Overview:**
- Aggregated air quality data from **2010-2014**
- **>5500** air quality monitoring stations worldwide
- Provided by Tropospheric Ozone Assessment Report (TOAR)
- Focus on tropospheric ozone metrics

**Features:**
- Global scope with easy-access metadata
- Enables comparison of machine learning methods
- Effects on climate, human health, crop yields

**Limitations:**
- Primarily outdoor/ambient air quality
- Ozone-focused, limited PM2.5/indoor pollutant data

### 6.2 UCI Air Quality Dataset

**Specifications:**
- **9,358 instances** of hourly averaged data
- 5 metal oxide chemical sensor array
- Ground truth from certified reference analyzers
- Location: Road-level in Italian city (significantly polluted area)

**Data Period:**
- March 2004 - February 2005

**License:**
- Creative Commons Attribution 4.0 International (CC BY 4.0)

**Usage:**
- Benchmark dataset in recent literature
- Reproducible testbed for methodology evaluation
- Continues to serve research despite age

### 6.3 AirNet Dataset

**Characteristics:**
- Deep learning focused
- **0.25° resolution** grid map of mainland China
- **>2 years** continuous measurements
- Air quality + meteorological data combined

**Application:**
- Spatial-temporal forecasting
- Multi-modal data fusion research

### 6.4 Recent ML Research Datasets (2024)

**Multi-Modal Data Sources:**
- Meteorological variables
- Satellite imagery
- Fixed and mobile sensor networks
- Localized demographic information

**Common Pollutants:**
- CO, O3, NO2, PM2.5

**Model Approaches:**
- **Random Forest Regression:** Up to 90% accuracy for PM2.5
- **Hybrid models:** CNN (spatial) + Bi-LSTM (temporal) + GNN (relationships)
- **Neural-ODE:** Continuous-time dynamics
- **SHAP analysis:** Interpretability of influential variables

**Cloud-Based Platforms:**
- Real-time data flow
- Web dashboards
- Mobile alert systems

### 6.5 Dataset Gaps for Indoor Air Quality

**Current Limitations:**
- Most datasets focus on **outdoor/ambient** air quality
- Limited **indoor-specific** labeled datasets
- Few datasets with **multi-sensor fusion** (CO2, PM2.5, VOC, temp/humidity)
- Scarce **event-labeled** data (cooking, cleaning, occupancy)
- Limited **residential building** datasets (mostly commercial/school)

**Recommendation:**
Create proprietary indoor dataset from AirGradient ONE deployments:
- Multi-sensor time series (1-minute resolution)
- User-labeled events (cooking, cleaning, windows open/closed)
- Occupancy ground truth (for training)
- Geographic diversity (climate zones)
- Building characteristics (age, HVAC type, filtration)

---

## 7. AirGradient ONE Sensor Specifications

### 7.1 Sensor Suite Overview

| Parameter | Sensor Model | Technology |
|-----------|-------------|------------|
| PM1, PM2.5, PM10 | Plantower PMS5003 | Laser scattering |
| CO2 | SenseAir S8/S88 | NDIR |
| TVOC, NOx | Sensirion SGP41 | Metal oxide |
| Temperature & Humidity | Sensirion SHT4x | Digital |

### 7.2 SenseAir S8/S88 CO2 Sensor

**Measurement Range:**
- 0-20,000 ppm

**Accuracy:**
- **±30 ppm ±3%** of reading (industry-leading accuracy)
- ±75 ppm at 600, 1000, 2500 ppm @ sea level, 25°C (ANSI/ASHRAE compliance)
- Accuracy defined after **minimum 3 weeks** continuous operation with ABC

**Technology:**
- **NDIR (Non-Dispersive Infrared)** - gold standard for CO2 measurement
- Individual calibration at factory
- UART digital interface

**Automatic Baseline Calibration (ABC):**
- Looks for lowest CO2 value in last **7-14 days**
- Resets to this value assuming periodic exposure to outdoor air
- Customizable interval
- **Critical:** Requires periodic outdoor air exposure for accuracy

**Lifetime:**
- **>15 years** estimated lifetime
- Maintenance-free

**S88 Advantages (New Version):**
- Nearly identical performance to S8
- **Altitude compensation** for high-altitude locations
- Recommended upgrade by SenseAir

**Startup Behavior:**
- 7-day automatic baseline calibration requires **few days** to become accurate after installation
- Should allow stabilization period before trusting readings

**Implementation Considerations:**
- Validate ABC is appropriate for deployment (24/7 occupied spaces may need manual calibration)
- Consider altitude compensation with S88
- Allow 3-week burn-in period for best accuracy

### 7.3 Plantower PMS5003 PM Sensor

**Measurement Range:**
- PM1.0, PM2.5, PM10 (µg/m³)

**Manufacturer Accuracy:**
- **±10-15 µg/m³**
- Counting efficiency: 98% for ≥0.5 µm particles, 50% for 0.3 µm

**Technology:**
- Laser scattering optical particle counter
- Inexpensive optical components and fans

**Accuracy Factors:**
- Particle composition affects readings
- Temperature and RH (especially high RH) impact particle size detection
- Ambient PM2.5 concentration
- Wind direction
- Sensor degradation over time

**Calibration Performance:**
- Uncalibrated median RMSE: **6.8 µg/m³**
- Calibrated median RMSE: **3.1 µg/m³** (54% improvement)
- Multivariate Linear Regression (MLR) with RH: MNB < ±10%, MNE < 30%

**Particle Size Limitations:**
- Effective for particles **< 1 µm**
- Behaves like a nephelometer (total scattered light)
- **Cannot accurately detect particles > 1 µm** (peer-reviewed finding)
- PM1 and PM2.5: Good accuracy
- PM10: **Limited accuracy** - not recommended for critical applications

**Batch-Specific Issues:**
- Newer batches may require calibration factors for low-concentration accuracy
- **DO NOT apply new calibration to old batches** - causes overestimation
- Errors consistent across batches - batch-wide corrections possible

**Implementation Considerations:**
- Apply RH-based calibration for improved accuracy
- Focus on PM1 and PM2.5 measurements
- Treat PM10 readings as informational only
- Consider periodic calibration against reference monitor
- Account for humidity in real-time readings

### 7.4 Sensirion SGP41 VOC/NOx Sensor

**Physical Specifications:**
- Package: DFN 2.44 × 2.44 × 0.85 mm³
- Two sensors on single chip (VOC + NOx pixels)

**Technology:**
- Metal oxide gas sensor
- Dual hotplate (separate VOC and NOx pixels)

**Sensitivity:**
- **< 100 ppb** detection for most VOCs in clean air
- Raw signals proportional to **logarithm of resistance**

**Measurement Process:**
- `measure_raw_signals` command: 50 ms measurement time
- Returns SRAW_VOC and SRAW_NOx (16-bit words + CRC)
- Humidity compensation when actual RH provided

**Output Format:**
- **VOC Index:** 1-500 scale (NOT absolute concentration)
- **NOx Index:** 1-500 scale
- Index 100 = average indoor composition (24-hour baseline)
- Index > 100 = deteriorating air quality

**Gas Index Algorithm:**
- Processes raw measurements on external microcontroller
- Available on Sensirion GitHub
- Maps raw signals to index scales
- Tunable parameters:
  - `index_offset`: 1-250 (default: 100) - typical conditions
  - `learning_time_offset_hours`: Offset estimation time constant
  - `gain_factor`: 1-1000 (default: 230) - amplify/attenuate output

**Proxy Gases (Specifications):**
- VOC events: Ethanol in clean air
- NOx events: NO2 in clean air

**Startup Procedure:**
- **Conditioning required:** Execute conditioning command for 10s after restart
- Heats NOx hotplate to different temperature for faster switch-on
- **DO NOT exceed 10s** to avoid sensing material damage

**Self-Test:**
- On-chip self-test available for production testing
- 320 ms execution time
- Returns fixed data pattern for validation

**Lifetime:**
- **10 years** in indoor field conditions (extensive qualification testing)
- Multi-pixel element robust against contaminating gases
- Low drift, long-term stability

**Limitations:**
- **NO absolute TVOC concentration** - relative index only
- Cannot distinguish between VOC types
- Industry trend (Sensirion, Bosch) away from absolute values
- Reason: VOCs too hard to accurately measure with low-cost components

**Implementation Considerations:**
- Use index values, not ppb conversions
- Implement 24-hour baseline learning
- Tune algorithm parameters for specific environment
- Alert on sustained index > 150 (50% above baseline)
- Combine with PM2.5 for event detection (cooking produces both VOC and PM spikes)
- Execute 10s conditioning on every startup/power cycle

### 7.5 Sensirion SHT4x Temperature/Humidity Sensor

**Technology:**
- Digital sensor with I2C interface

**Typical Specifications (SHT4x family):**
- Temperature accuracy: ±0.2°C
- Humidity accuracy: ±1.8% RH
- Fast response time
- Low power consumption

**Applications:**
- Thermal comfort calculations
- Mold risk assessment
- Dew point calculations
- Heat index / apparent temperature

### 7.6 Multi-Sensor Calibration and Data Quality

**Cross-Sensor Validation:**
- CO2 rise + occupancy patterns should correlate
- PM2.5 spikes should correlate with VOC index increases (cooking)
- Temperature/humidity should follow expected inverse relationship
- Sudden changes in multiple sensors = ventilation event (window opening)

**Quality Control Flags:**
- Sensor out-of-range values
- Unrealistic rate-of-change (sensor failure)
- Missing data (communication errors)
- Calibration staleness (ABC not executed)

**Recommended Maintenance:**
- PM sensor: Check for dust accumulation every 6-12 months
- CO2 sensor: Verify ABC execution, manual calibration if 24/7 occupied space
- VOC sensor: No maintenance, but 10-year replacement recommended
- System: Power cycle monthly to execute sensor self-tests

---

## 8. Implementation Roadmap

### Phase 1: Core Data Pipeline
1. Ingest 1-minute resolution data from all AirGradient ONE sensors
2. Implement data quality checks and sensor validation
3. Calculate basic derived metrics:
   - AQI (indoor adapted)
   - Thermal comfort indices (simplified without air velocity)
   - Mold risk score

### Phase 2: Event Detection
1. Cooking detection (PM2.5 + VOC spike patterns)
2. Occupancy inference (CO2 rise rates)
3. Ventilation events (multi-sensor sudden changes)
4. Wildfire smoke infiltration (I/O ratio with external API)

### Phase 3: Time-Series Analytics
1. Seasonal decomposition (diurnal, weekly patterns)
2. HVAC cycle detection
3. Short-term forecasting (1-6 hour LSTM models)
4. Anomaly detection (deviations from learned patterns)

### Phase 4: Health and Recommendations
1. Health-based alerting (threshold exceedances)
2. Ventilation adequacy (ACH estimation)
3. Air quality improvement recommendations
4. Energy efficiency insights (over-ventilation detection)

### Phase 5: Machine Learning Enhancement
1. Create labeled dataset from deployments
2. Train event classifiers (cooking, cleaning, occupancy)
3. Personalized baseline learning
4. Multi-building pattern recognition

---

## 9. Key Research Sources

### Standards and Guidelines
- [Indoor Air Quality Index (IAQI): Standards and Metrics](https://atmotube.com/blog/indoor-air-quality-index-iaqi)
- [EPA Final Updates to the Air Quality Index (AQI) for Particulate Matter](https://www.epa.gov/system/files/documents/2024-02/pm-naaqs-air-quality-index-fact-sheet.pdf)
- [IAQ Standards and Guidelines (EPA and ASHRAE Standard)](https://foobot.io/guides/iaq-standards-and-guidelines.php)
- [WHO guidelines for indoor air quality: dampness and mould](https://www.who.int/publications/i/item/9789289041683)

### CO2 and Cognitive Function
- [Carbon Dioxide Levels Chart – CO2 Meter](https://www.co2meter.com/blogs/news/carbon-dioxide-indoor-levels-chart)
- [Carbon dioxide guidelines for indoor air quality: a review - Nature](https://www.nature.com/articles/s41370-024-00694-7)
- [Is CO2 an Indoor Pollutant? Direct Effects on Human Decision-Making - PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC3548274/)

### PM2.5 Health Effects
- [The Impact of Fine Particulate Matter 2.5 on the Cardiovascular System - PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC9370264/)
- [Guidance for fine particulate matter (PM2.5) in residential indoor air - Canada](https://www.canada.ca/en/health-canada/services/publications/healthy-living/guidance-fine-particulate-matter-pm2-5-residential-indoor-air.html)
- [National Ambient Air Quality Standards (NAAQS) for PM - EPA](https://www.epa.gov/pm-pollution/national-ambient-air-quality-standards-naaqs-pm)

### VOCs
- [Volatile Organic Compounds' Impact on Indoor Air Quality - EPA](https://www.epa.gov/indoor-air-quality-iaq/volatile-organic-compounds-impact-indoor-air-quality)
- [Threshold Limit Values of Volatile Organic Compounds](https://foobot.io/guides/threshold-limit-values-volatile-organic-compounds.php)
- [A Comprehensive Guide on Volatile Organic Compounds (VOCs) - TSI](https://tsi.com/indoor-environments/learn/volatile-organic-compounds-guide)

### Ventilation and ACH
- [Review and Extension of CO2-Based Methods to Determine Ventilation Rates - PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC5334699/)
- [Harvard Healthy Buildings - 5-step guide to checking ventilation](https://schools.forhealth.org/wp-content/uploads/sites/19/2020/08/Harvard-Healthy-Buildings-program-How-to-assess-classroom-ventilation-08-28-2020.pdf)
- [Measuring air change rates with CO2 sensors - OpenEnergyMonitor](https://docs.openenergymonitor.org/heatpumps/measuring_ach_with_co2.html)

### Mold Risk
- [Mold Chart for Temperature and Humidity Monitors](https://energyhandyman.com/knowledge-library/mold-chart-for-temperature-and-humidity-monitors/)
- [Temperature versus Relative Humidity: Which Is More Important for Indoor Mold Prevention? - PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC9319059/)
- [Modelling mould growth in domestic environments - ScienceDirect](https://www.sciencedirect.com/science/article/pii/S0360132321009756)

### Thermal Comfort
- [What Is PMV? What Is PPD? The Basics of Thermal Comfort - SimScale](https://www.simscale.com/blog/what-is-pmv-ppd/)
- [Thermal comfort - Wikipedia](https://en.wikipedia.org/wiki/Thermal_comfort)
- [Thermal Comfort ref guide IDC - USGBC](https://www.usgbc.org/node/2758273)

### Cooking Detection
- [Indoor Household Particulate Matter Measurements Using Low-cost Sensors - Aerosol and Air Quality Research](https://aaqr.org/articles/aaqr-19-01-lcs-0046)
- [Residential cooking-related PM2.5: Spatial-temporal variations - PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC8224830/)
- [Impact of Cooking Methods on Indoor Air Quality - Indoor Air](https://onlinelibrary.wiley.com/doi/10.1155/2024/6355613)

### Wildfire Smoke
- [Using Low-Cost Sensors to Assess PM2.5 Infiltration during Wildfire Smoke - PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC8468682/)
- [Wildfire smoke impacts on indoor air quality assessed using crowdsourced data - PNAS](https://www.pnas.org/doi/10.1073/pnas.2106478118)
- [Field measurements of PM2.5 infiltration factor and portable air cleaner effectiveness - PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC8026580/)

### Occupancy Detection
- [Sensing by Proxy: Occupancy Detection Based on Indoor CO2 Concentration - Berkeley](https://bayen.berkeley.edu/sites/default/files/sensing_by_proxy.pdf)
- [Indoor Human Occupancy Counting using Carbon Dioxide - arXiv](https://arxiv.org/pdf/1706.05286)
- [Estimation of Occupancy Using IoT Sensors and CO2-Based ML - PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC9860618/)

### Time-Series Forecasting
- [Time Series Visualization and Analysis - Environmental Data Science](https://bookdown.org/igisc/EnvDataSci/ts.html)
- [Forecasting air quality time series using deep learning](https://www.tandfonline.com/doi/full/10.1080/10962247.2018.1459956)
- [Time series and regression methods for univariate environmental forecasting - ScienceDirect](https://www.sciencedirect.com/science/article/pii/S0048969723011968)

### Datasets and Benchmarks
- [AQ-Bench: a benchmark dataset for machine learning on global air quality metrics](https://essd.copernicus.org/articles/13/3013/2021/)
- [AirNet: predictive machine learning model for air quality forecasting - Environmental Systems Research](https://environmentalsystemsresearch.springeropen.com/articles/10.1186/s40068-024-00378-z)
- [UCI Machine Learning Repository - Air Quality Dataset](http://archive.ics.uci.edu/dataset/360/air+quality)

### AirGradient ONE Sensors
- [AirGradient ONE Documentation](https://www.airgradient.com/documentation/one-v9/)
- [SenseAir S8 Datasheet](http://www.co2meters.com/Documentation/Datasheets/DS-S8-3.2.pdf)
- [Why We've Decided to Change Our CO2 Sensor - AirGradient Blog](https://www.airgradient.com/blog/changed-co2-sensor-to-s88/)
- [The Plantower PMS5003 and PMS7003 Air Quality Sensor experiment](https://aqicn.org/sensor/pms5003-7003/)
- [Enhance the Accuracy of the PMS Sensors - AirGradient](https://www.airgradient.com/documentation/calibrate-low-pms-sensors/)
- [Sensirion SGP41 - VOC and NOx sensor](https://sensirion.com/products/catalog/SGP41)
- [Datasheet SGP41 - Sensirion](https://sensirion.com/media/documents/5FE8673C/61E96F50/Sensirion_Gas_Sensors_Datasheet_SGP41.pdf)
- [How Accurate is the Sensirion SGP41 TVOC Sensor? - AirGradient Blog](https://www.airgradient.com/blog/accuracy-sensirion-sgp41/)

---

## Appendix A: Recommended Alert Thresholds

### CO2 Levels
| Level | Range (ppm) | Color | Action |
|-------|-------------|-------|---------|
| Excellent | < 600 | Green | None |
| Good | 600-800 | Green | None |
| Moderate | 800-1000 | Yellow | Monitor |
| Poor | 1000-1500 | Orange | Increase ventilation |
| Very Poor | > 1500 | Red | Immediate ventilation required |

### PM2.5 (µg/m³)
| Level | Range | Color | AQI Category |
|-------|-------|-------|--------------|
| Excellent | < 5 | Green | Good |
| Good | 5-9 | Green | Good |
| Moderate | 9-12 | Yellow | Moderate |
| Unhealthy (Sensitive) | 12-35 | Orange | USG |
| Unhealthy | 35-55 | Red | Unhealthy |
| Very Unhealthy | > 55 | Purple | Very Unhealthy |

### VOC Index
| Level | Index | Color | Action |
|-------|-------|-------|---------|
| Good | < 100 | Green | None |
| Moderate | 100-150 | Yellow | Monitor sources |
| Poor | 150-250 | Orange | Identify and reduce sources |
| Very Poor | > 250 | Red | Immediate source removal + ventilation |

### Mold Risk (RH %)
| Level | RH Range | Color | Risk |
|-------|----------|-------|------|
| Low | < 50 | Green | Minimal |
| Moderate | 50-60 | Yellow | Low |
| Elevated | 60-65 | Orange | Moderate (monitor) |
| High | 65-80 | Red | High (reduce immediately) |
| Critical | > 80 | Purple | Very High (urgent action) |

---

**Document Version:** 1.0
**Last Updated:** December 13, 2025
**Next Review:** Quarterly (March 2026)
