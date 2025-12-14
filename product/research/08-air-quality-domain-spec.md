# Air Quality Domain Specification

**Version:** 1.0.0
**Date:** December 13, 2025
**Platform:** Neural Data Platform - Air Quality Module
**Target Hardware:** AirGradient ONE on Raspberry Pi

---

## 1. Domain Overview

### 1.1 Purpose

This specification defines the air quality monitoring domain for the neural-data-platform, enabling real-time indoor air quality (IAQ) assessment, health-based alerting, ML-driven forecasting, and actionable recommendations for residential and commercial environments.

**Core Objectives:**
- Monitor indoor air quality with AirGradient ONE sensor suite
- Detect health-impacting pollution events (cooking, smoke infiltration, poor ventilation)
- Provide actionable alerts and recommendations to occupants
- Learn occupancy patterns and ventilation adequacy through ML
- Integrate with home automation (HomeKit, MQTT) and Claude (MCP)

### 1.2 Scope

**In Scope:**
- Indoor air quality monitoring and analytics
- Multi-pollutant health threshold tracking (CO2, PM2.5, PM10, VOC, NOx)
- Derived metrics (AQI, ventilation rate, mold risk, thermal comfort)
- Event detection (cooking, occupancy, HVAC cycles, smoke infiltration)
- Time-series forecasting (1-6 hour predictions)
- MCP tool integration for Claude-based interactions
- HomeKit/MQTT publishing for home automation

**Out of Scope:**
- Outdoor ambient air quality monitoring (except for I/O ratio calculations)
- Industrial/workplace air quality compliance (OSHA standards)
- Laboratory-grade precision measurements
- Chemical speciation beyond sensor capabilities

### 1.3 Key Differentiators

Unlike outdoor AQI systems:
- **1-minute granularity** vs. hourly/daily averages
- **Indoor-specific pollutants** (CO2, VOC index) not in EPA outdoor AQI
- **Event-driven alerts** (cooking spikes, occupancy changes) vs. threshold-only
- **Ventilation adequacy** as first-class metric
- **Privacy-preserving occupancy inference** from CO2 patterns

---

## 2. Sensor Specifications (AirGradient ONE)

### 2.1 Sensor Suite Overview

| Sensor | Measurement | Technology | Communication |
|--------|-------------|------------|---------------|
| SenseAir S8/S88 | CO2 | NDIR (Non-Dispersive Infrared) | UART |
| Plantower PMS5003 | PM1, PM2.5, PM10 | Laser scattering | UART |
| Sensirion SGP41 | VOC Index, NOx Index | Metal oxide | I2C |
| Sensirion SHT4x | Temperature, Humidity | Capacitive/resistive | I2C |

### 2.2 SenseAir S8/S88 CO2 Sensor

**Measurement Range:** 0-20,000 ppm

**Accuracy:**
- ±30 ppm ±3% of reading (industry-leading)
- ±75 ppm at 600, 1000, 2500 ppm @ sea level, 25°C (ASHRAE compliant)
- Accuracy achieved after **minimum 3 weeks** continuous operation with ABC

**Automatic Baseline Calibration (ABC):**
- Assumes periodic outdoor air exposure (400 ppm)
- Calibration interval: 7-14 days
- **Critical:** Unsuitable for 24/7 occupied spaces without manual calibration

**Lifetime:** >15 years (maintenance-free)

**S88 Enhancements:**
- Altitude compensation for high-elevation deployments
- Recommended upgrade by manufacturer

**Calibration Requirements:**

```rust
pub struct CO2CalibrationConfig {
    pub abc_enabled: bool,
    pub abc_interval_days: u8,          // 7-14 typical
    pub manual_reference_ppm: Option<u16>, // 400 for outdoor
    pub altitude_meters: Option<u16>,   // For S88
    pub warmup_period_seconds: u32,     // Allow stabilization
}

pub enum CO2CalibrationStatus {
    Warming,                  // < 3 weeks since installation
    AbcActive,                // ABC enabled and functioning
    ManualRequired,           // 24/7 occupancy detected
    Stale,                    // No outdoor exposure in 30+ days
    Failed,                   // Sensor malfunction
}
```

### 2.3 Plantower PMS5003 Particulate Matter Sensor

**Measurement Range:**
- PM1.0: 0-500 µg/m³
- PM2.5: 0-500 µg/m³
- PM10: 0-500 µg/m³

**Accuracy:**
- Factory spec: ±10-15 µg/m³
- Calibrated (with RH correction): ±3.1 µg/m³ (54% improvement)
- Counting efficiency: 98% for ≥0.5 µm, 50% for 0.3 µm

**Particle Size Limitations:**
- **PM1 and PM2.5:** Reliable (< 1 µm particles)
- **PM10:** Limited accuracy (peer-reviewed finding)
- Behaves like nephelometer for larger particles

**Environmental Corrections:**

```rust
pub struct PMCalibration {
    pub relative_humidity: f32,         // 0-100%
    pub temperature_celsius: f32,
    pub batch_correction_factor: f32,   // Batch-specific calibration
}

impl PMSensor {
    /// Apply RH-based calibration using multivariate linear regression
    pub fn calibrate_pm25(raw_pm25: f32, rh: f32) -> f32 {
        // MLR model: PM2.5_true = a + b*PM2.5_raw + c*RH + d*PM2.5_raw*RH
        let a = -2.65;
        let b = 0.96;
        let c = 0.08;
        let d = -0.0012;
        a + (b * raw_pm25) + (c * rh) + (d * raw_pm25 * rh)
    }
}
```

**Data Quality Flags:**

```rust
pub enum PMQualityFlag {
    Good,                     // Standard operation
    HighHumidity,             // RH > 80%, hygroscopic growth effects
    FanDegraded,              // Flow rate anomaly detected
    Saturated,                // > 500 µg/m³
    CalibrationRequired,      // Drift detected vs. reference
}
```

### 2.4 Sensirion SGP41 VOC/NOx Sensor

**Physical Specs:**
- Dual-pixel metal oxide sensor (VOC + NOx on single chip)
- Package: 2.44 × 2.44 × 0.85 mm³
- Lifetime: 10 years (indoor conditions)

**Measurement Process:**
- Command: `measure_raw_signals` (50 ms)
- Returns: SRAW_VOC, SRAW_NOx (16-bit + CRC)
- Humidity compensation when RH provided

**Output Format:**

```rust
pub struct SGP41Reading {
    pub voc_index: u16,        // 1-500 scale (NOT ppb/ppm)
    pub nox_index: u16,        // 1-500 scale
    pub raw_voc: u16,          // Raw resistance signal
    pub raw_nox: u16,          // Raw resistance signal
    pub timestamp: DateTime<Utc>,
}

pub struct VOCIndexInterpretation {
    pub index: u16,
    pub baseline: u16,         // 100 = 24-hour average
    pub deviation_percent: i16, // (index - baseline) / baseline * 100
    pub air_quality: VOCLevel,
}

pub enum VOCLevel {
    Excellent,      // < 100 (better than baseline)
    Good,           // 100-150 (up to 50% above baseline)
    Moderate,       // 150-200
    Poor,           // 200-300
    VeryPoor,       // > 300
}
```

**Sensitivity:** < 100 ppb detection for most VOCs (ethanol proxy)

**Critical Limitations:**
- **NO absolute TVOC concentration** - relative index only
- Cannot distinguish between VOC types (formaldehyde vs. cleaning products)
- Industry trend away from absolute values (too unreliable with low-cost sensors)

**Gas Index Algorithm Tuning:**

```rust
pub struct VOCAlgorithmParams {
    pub index_offset: u16,              // 1-250, default: 100
    pub learning_time_offset_hours: u16, // Baseline stabilization
    pub learning_time_gain_hours: u16,
    pub gating_max_duration_minutes: u16,
    pub std_initial: u16,
    pub gain_factor: u16,               // 1-1000, default: 230
}
```

**Startup Procedure:**

```rust
impl SGP41Sensor {
    pub async fn initialize(&mut self) -> Result<()> {
        // Execute conditioning command for exactly 10s after power-on
        self.execute_conditioning().await?;
        tokio::time::sleep(Duration::from_secs(10)).await;

        // Begin normal measurement loop
        self.start_measurement_loop(Duration::from_secs(1)).await
    }
}
```

### 2.5 Sensirion SHT4x Temperature/Humidity Sensor

**Accuracy:**
- Temperature: ±0.2°C
- Humidity: ±1.8% RH
- I2C digital interface

**Applications:**
- Thermal comfort (PMV/PPD calculations)
- Mold risk assessment
- PM sensor calibration (hygroscopic growth correction)
- Dew point/heat index calculations

---

## 3. Primary Measurements

### 3.1 Core Data Structure

```rust
use chrono::{DateTime, Utc};

/// Raw sensor reading - single timestamp snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirQualityReading {
    // Metadata
    pub location_id: String,
    pub sensor_id: String,
    pub timestamp: DateTime<Utc>,

    // CO2 (ppm)
    pub co2_ppm: Option<u16>,
    pub co2_quality: CO2QualityFlag,

    // Particulate Matter (µg/m³)
    pub pm1_0: Option<f32>,
    pub pm2_5: Option<f32>,
    pub pm2_5_calibrated: Option<f32>,  // RH-corrected
    pub pm10: Option<f32>,
    pub pm_quality: PMQualityFlag,

    // VOC/NOx (index 1-500)
    pub voc_index: Option<u16>,
    pub nox_index: Option<u16>,
    pub voc_baseline: u16,              // 24-hour moving baseline

    // Environmental
    pub temperature_c: Option<f32>,
    pub humidity_percent: Option<f32>,

    // Data Quality
    pub reading_quality: ReadingQuality,
    pub sensor_uptime_seconds: u64,
}

/// Data quality assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadingQuality {
    pub overall_score: f32,         // 0.0-1.0
    pub completeness: f32,          // % of expected fields present
    pub freshness_ms: u64,          // Time since sensor poll
    pub validity_flags: Vec<String>, // Human-readable issues
}

/// Valid ranges for each measurement
pub struct MeasurementRanges {
    pub co2_min: u16,           // 400 ppm (outdoor baseline)
    pub co2_max: u16,           // 10,000 ppm (sensor limit)
    pub pm25_max: f32,          // 500 µg/m³ (sensor saturation)
    pub voc_index_min: u16,     // 1
    pub voc_index_max: u16,     // 500
    pub temp_min_c: f32,        // -10°C
    pub temp_max_c: f32,        // 50°C
    pub humidity_min: f32,      // 0%
    pub humidity_max: f32,      // 100%
}

impl AirQualityReading {
    /// Validate all measurements within sensor specifications
    pub fn validate(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        if let Some(co2) = self.co2_ppm {
            if co2 < 350 || co2 > 10_000 {
                errors.push(ValidationError::OutOfRange {
                    field: "co2_ppm",
                    value: co2.into(),
                    expected: "350-10000 ppm",
                });
            }
        }

        if let Some(pm25) = self.pm2_5 {
            if pm25 < 0.0 || pm25 > 500.0 {
                errors.push(ValidationError::OutOfRange {
                    field: "pm2_5",
                    value: pm25.into(),
                    expected: "0-500 µg/m³",
                });
            }
        }

        errors
    }

    /// Calculate overall reading quality score
    pub fn calculate_quality(&mut self) {
        let mut score = 1.0;
        let mut flags = Vec::new();

        // Completeness (expected 7 fields)
        let present = [
            self.co2_ppm.is_some(),
            self.pm2_5.is_some(),
            self.voc_index.is_some(),
            self.temperature_c.is_some(),
            self.humidity_percent.is_some(),
        ].iter().filter(|&&x| x).count();

        let completeness = present as f32 / 5.0;
        score *= completeness;

        if completeness < 0.8 {
            flags.push("incomplete_reading".to_string());
        }

        // Sensor-specific quality flags
        if matches!(self.co2_quality, CO2QualityFlag::Warming) {
            score *= 0.7;
            flags.push("co2_warmup_period".to_string());
        }

        if matches!(self.pm_quality, PMQualityFlag::HighHumidity) {
            score *= 0.9;
            flags.push("pm_high_humidity".to_string());
        }

        self.reading_quality = ReadingQuality {
            overall_score: score,
            completeness,
            freshness_ms: 0, // Set by ingestion pipeline
            validity_flags: flags,
        };
    }
}
```

---

## 4. Health Thresholds

### 4.1 CO2 Cognitive Impact Levels

Based on EPA, ASHRAE, and cognitive function research:

| Level | CO2 Range (ppm) | Color | Health Impact | Action Required |
|-------|----------------|-------|---------------|-----------------|
| Excellent | < 600 | Green | Outdoor-equivalent air quality | None |
| Good | 600-800 | Green | Optimal indoor air quality | None |
| Acceptable | 800-1000 | Yellow | ASHRAE standard, minimal impact | Monitor |
| Moderate | 1000-1500 | Orange | Cognitive decline begins, 165% attention decrease | Increase ventilation |
| Poor | 1500-2000 | Red | Drowsiness, fatigue, impaired decision-making | Urgent ventilation |
| Very Poor | > 2000 | Purple | Severe cognitive impairment | Immediate action |

**Implementation:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CO2Level {
    Excellent,   // < 600
    Good,        // 600-800
    Acceptable,  // 800-1000
    Moderate,    // 1000-1500
    Poor,        // 1500-2000
    VeryPoor,    // > 2000
}

impl CO2Level {
    pub fn from_ppm(ppm: u16) -> Self {
        match ppm {
            0..=599 => CO2Level::Excellent,
            600..=799 => CO2Level::Good,
            800..=999 => CO2Level::Acceptable,
            1000..=1499 => CO2Level::Moderate,
            1500..=1999 => CO2Level::Poor,
            _ => CO2Level::VeryPoor,
        }
    }

    pub fn health_message(&self) -> &'static str {
        match self {
            CO2Level::Excellent => "Outdoor-quality air. No action needed.",
            CO2Level::Good => "Optimal indoor air quality.",
            CO2Level::Acceptable => "Acceptable ventilation per ASHRAE standards.",
            CO2Level::Moderate => "Cognitive performance may decline. Consider opening windows or increasing HVAC.",
            CO2Level::Poor => "Drowsiness and fatigue likely. Ventilate immediately.",
            CO2Level::VeryPoor => "Severe air quality. Evacuate or provide emergency ventilation.",
        }
    }

    pub fn aqi_equivalent(&self) -> u16 {
        // Map to 0-500 AQI scale for consistency with EPA
        match self {
            CO2Level::Excellent => 25,
            CO2Level::Good => 75,
            CO2Level::Acceptable => 100,
            CO2Level::Moderate => 150,
            CO2Level::Poor => 200,
            CO2Level::VeryPoor => 300,
        }
    }
}
```

### 4.2 PM2.5 Cardiovascular Thresholds

Based on EPA 2024 updates and WHO guidelines:

| Level | PM2.5 Range (µg/m³) | Color | AQI Category | Health Impact | Alert |
|-------|-------------------|-------|--------------|---------------|-------|
| Excellent | 0-5 | Green | Good | Minimal risk | None |
| Good | 5-9 | Green | Good | EPA 2024 annual target | None |
| Moderate | 9-12 | Yellow | Moderate | Sensitive groups may react | Monitor |
| USG | 12-35 | Orange | Unhealthy for Sensitive Groups | Increased CVD risk | Alert sensitive groups |
| Unhealthy | 35-55 | Red | Unhealthy | General population effects | Alert all |
| Very Unhealthy | 55-150 | Purple | Very Unhealthy | Serious health impacts | Urgent alert |
| Hazardous | > 150 | Maroon | Hazardous | Emergency conditions | Emergency |

**Critical Finding:** Increased cardiovascular risk **even below 12 µg/m³** (no safe threshold)

**Implementation:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PM25Level {
    Excellent,       // 0-5
    Good,            // 5-9
    Moderate,        // 9-12
    USG,             // 12-35 (Unhealthy for Sensitive Groups)
    Unhealthy,       // 35-55
    VeryUnhealthy,   // 55-150
    Hazardous,       // > 150
}

impl PM25Level {
    pub fn from_ugm3(ugm3: f32) -> Self {
        match ugm3 {
            x if x < 5.0 => PM25Level::Excellent,
            x if x < 9.0 => PM25Level::Good,
            x if x < 12.0 => PM25Level::Moderate,
            x if x < 35.0 => PM25Level::USG,
            x if x < 55.0 => PM25Level::Unhealthy,
            x if x < 150.0 => PM25Level::VeryUnhealthy,
            _ => PM25Level::Hazardous,
        }
    }

    /// EPA AQI calculation using 2024 breakpoints
    pub fn to_aqi(ugm3: f32) -> u16 {
        // EPA linear interpolation formula: I = ((I_hi - I_lo) / (C_hi - C_lo)) * (C - C_lo) + I_lo
        match ugm3 {
            x if x < 9.0 => Self::interpolate(x, 0.0, 9.0, 0, 50),
            x if x < 35.5 => Self::interpolate(x, 9.0, 35.4, 51, 100),
            x if x < 55.5 => Self::interpolate(x, 35.5, 55.4, 101, 150),
            x if x < 125.5 => Self::interpolate(x, 55.5, 125.4, 151, 200),
            x if x < 225.5 => Self::interpolate(x, 125.5, 225.4, 201, 300),
            _ => Self::interpolate(ugm3, 225.5, 500.0, 301, 500),
        }
    }

    fn interpolate(conc: f32, c_lo: f32, c_hi: f32, i_lo: u16, i_hi: u16) -> u16 {
        (((i_hi - i_lo) as f32 / (c_hi - c_lo)) * (conc - c_lo) + i_lo as f32).round() as u16
    }
}
```

### 4.3 VOC Index Exposure Limits

**Important:** SGP41 provides **relative index** (1-500), not absolute TVOC concentration

| Level | VOC Index | Deviation from Baseline | Color | Action |
|-------|-----------|------------------------|-------|--------|
| Excellent | < 100 | Better than baseline | Green | None |
| Good | 100-150 | Up to 50% above baseline | Yellow | Monitor sources |
| Moderate | 150-200 | 50-100% above baseline | Orange | Identify sources |
| Poor | 200-300 | 100-200% above baseline | Red | Remove sources + ventilate |
| Very Poor | > 300 | > 200% above baseline | Purple | Emergency ventilation |

**Reference Absolute Limits** (if converting from other sensors):
- Total VOC: < 0.5 mg/m³ safe
- Formaldehyde: 0.1-0.3 ppm (OSHA PEL: 0.75 ppm)
- Benzene: No safe level (carcinogen)

```rust
pub struct VOCThresholds {
    pub baseline: u16,          // 100 = 24-hour average
    pub good_max: u16,          // 150
    pub moderate_max: u16,      // 200
    pub poor_max: u16,          // 300
}

impl VOCThresholds {
    pub fn assess(&self, current_index: u16) -> VOCLevel {
        match current_index {
            x if x < self.baseline => VOCLevel::Excellent,
            x if x <= self.good_max => VOCLevel::Good,
            x if x <= self.moderate_max => VOCLevel::Moderate,
            x if x <= self.poor_max => VOCLevel::Poor,
            _ => VOCLevel::VeryPoor,
        }
    }

    pub fn deviation_percent(&self, current_index: u16) -> i16 {
        ((current_index as i32 - self.baseline as i32) * 100 / self.baseline as i32) as i16
    }
}
```

### 4.4 Composite Thresholds Table

```rust
pub struct HealthThresholds {
    pub co2: CO2Thresholds,
    pub pm25: PM25Thresholds,
    pub voc: VOCThresholds,
    pub mold_risk: MoldRiskThresholds,
}

pub struct CO2Thresholds {
    pub excellent_max: u16,     // 600 ppm
    pub good_max: u16,          // 800 ppm
    pub acceptable_max: u16,    // 1000 ppm
    pub moderate_max: u16,      // 1500 ppm
    pub poor_max: u16,          // 2000 ppm
}

pub struct PM25Thresholds {
    pub excellent_max: f32,     // 5 µg/m³
    pub good_max: f32,          // 9 µg/m³ (EPA 2024)
    pub moderate_max: f32,      // 12 µg/m³
    pub usg_max: f32,           // 35 µg/m³
    pub unhealthy_max: f32,     // 55 µg/m³
    pub very_unhealthy_max: f32, // 150 µg/m³
}

pub struct MoldRiskThresholds {
    pub low_rh_max: f32,        // 50%
    pub moderate_rh_max: f32,   // 60%
    pub elevated_rh_max: f32,   // 65%
    pub high_rh_max: f32,       // 80%
}
```

---

## 5. Derived Metrics

### 5.1 Indoor AQI Calculation

**Multi-Pollutant Index** (adapted from EPA outdoor AQI):

```rust
pub struct IndoorAQI {
    pub overall_aqi: u16,           // 0-500
    pub dominant_pollutant: Pollutant,
    pub co2_aqi: u16,
    pub pm25_aqi: u16,
    pub voc_aqi: u16,
    pub category: AQICategory,
}

#[derive(Debug, Clone, Copy)]
pub enum Pollutant {
    CO2,
    PM25,
    PM10,
    VOC,
    NOx,
}

#[derive(Debug, Clone, Copy)]
pub enum AQICategory {
    Good,                // 0-50
    Moderate,            // 51-100
    USG,                 // 101-150
    Unhealthy,           // 151-200
    VeryUnhealthy,       // 201-300
    Hazardous,           // 301-500
}

impl IndoorAQI {
    pub fn calculate(reading: &AirQualityReading) -> Self {
        let co2_aqi = reading.co2_ppm
            .map(|ppm| CO2Level::from_ppm(ppm).aqi_equivalent())
            .unwrap_or(0);

        let pm25_aqi = reading.pm2_5_calibrated
            .or(reading.pm2_5)
            .map(|ugm3| PM25Level::to_aqi(ugm3))
            .unwrap_or(0);

        let voc_aqi = reading.voc_index
            .map(|idx| Self::voc_index_to_aqi(idx, reading.voc_baseline))
            .unwrap_or(0);

        // EPA "dominant pollutant" approach - highest AQI wins
        let overall_aqi = co2_aqi.max(pm25_aqi).max(voc_aqi);

        let dominant_pollutant = if co2_aqi >= pm25_aqi && co2_aqi >= voc_aqi {
            Pollutant::CO2
        } else if pm25_aqi >= voc_aqi {
            Pollutant::PM25
        } else {
            Pollutant::VOC
        };

        IndoorAQI {
            overall_aqi,
            dominant_pollutant,
            co2_aqi,
            pm25_aqi,
            voc_aqi,
            category: Self::aqi_to_category(overall_aqi),
        }
    }

    fn voc_index_to_aqi(voc_index: u16, baseline: u16) -> u16 {
        // Map VOC index to AQI scale
        match voc_index {
            x if x < baseline => 50,                // Better than baseline = Good
            x if x < baseline + 50 => 100,           // Up to 50% above = Moderate
            x if x < baseline + 100 => 150,          // 100% above = USG
            x if x < baseline + 200 => 200,          // 200% above = Unhealthy
            _ => 300,                                // > 200% = Very Unhealthy
        }
    }

    fn aqi_to_category(aqi: u16) -> AQICategory {
        match aqi {
            0..=50 => AQICategory::Good,
            51..=100 => AQICategory::Moderate,
            101..=150 => AQICategory::USG,
            151..=200 => AQICategory::Unhealthy,
            201..=300 => AQICategory::VeryUnhealthy,
            _ => AQICategory::Hazardous,
        }
    }
}
```

### 5.2 Ventilation Adequacy (ACH from CO2 Decay)

**Air Changes Per Hour (ACH)** measures how quickly fresh air replaces stale air.

**Formula from CO2 Decay:**
```
ACH = -ln((C2 - Co) / (C1 - Co)) / (t2 - t1)
```
Where:
- C1 = Initial CO2 (ppm) when space becomes unoccupied
- C2 = Final CO2 (ppm) after time interval
- Co = Outdoor CO2 baseline (400 ppm)
- t2 - t1 = Time interval (hours)

**Implementation:**

```rust
pub struct VentilationMetrics {
    pub ach_estimate: Option<f32>,      // Air changes per hour
    pub ach_adequacy: VentilationAdequacy,
    pub co2_decay_rate: Option<f32>,    // ppm/hour
    pub outdoor_co2_baseline: u16,      // Assumed 400 ppm
    pub confidence: f32,                // 0.0-1.0
}

#[derive(Debug, Clone, Copy)]
pub enum VentilationAdequacy {
    Excellent,      // >= 6 ACH (hospitals, labs)
    Good,           // 4-6 ACH (offices, schools)
    Adequate,       // 2-4 ACH (residential)
    Marginal,       // 1-2 ACH (poor ventilation)
    Poor,           // < 1 ACH (stagnant)
    Unknown,        // Insufficient data
}

impl VentilationMetrics {
    /// Calculate ACH from CO2 decay during unoccupied period
    pub fn calculate_ach(
        initial_co2: u16,
        final_co2: u16,
        time_interval_hours: f32,
        outdoor_co2: u16,
    ) -> Option<f32> {
        if time_interval_hours < 0.5 || initial_co2 <= final_co2 {
            return None; // Insufficient decay or invalid data
        }

        let c1 = initial_co2 as f32;
        let c2 = final_co2 as f32;
        let co = outdoor_co2 as f32;

        // Natural log of concentration ratio
        let ratio = (c2 - co) / (c1 - co);
        if ratio <= 0.0 {
            return None; // Mathematical impossibility
        }

        let ach = -ratio.ln() / time_interval_hours;

        // Sanity check: ACH typically 0.1-10 for buildings
        if ach < 0.1 || ach > 10.0 {
            None
        } else {
            Some(ach)
        }
    }

    pub fn assess_adequacy(ach: f32, space_type: SpaceType) -> VentilationAdequacy {
        match space_type {
            SpaceType::Residential => match ach {
                x if x >= 4.0 => VentilationAdequacy::Excellent,
                x if x >= 2.0 => VentilationAdequacy::Good,
                x if x >= 1.0 => VentilationAdequacy::Adequate,
                x if x >= 0.5 => VentilationAdequacy::Marginal,
                _ => VentilationAdequacy::Poor,
            },
            SpaceType::Office => match ach {
                x if x >= 6.0 => VentilationAdequacy::Excellent,
                x if x >= 4.0 => VentilationAdequacy::Good,
                x if x >= 2.0 => VentilationAdequacy::Adequate,
                _ => VentilationAdequacy::Poor,
            },
            SpaceType::Hospital => match ach {
                x if x >= 12.0 => VentilationAdequacy::Excellent,
                x if x >= 6.0 => VentilationAdequacy::Good,
                _ => VentilationAdequacy::Poor,
            },
        }
    }
}

pub enum SpaceType {
    Residential,
    Office,
    School,
    Hospital,
    Laboratory,
}
```

**Decay Detection Algorithm:**

```rust
pub struct CO2DecayDetector {
    window_size: Duration,          // 3-8 hours ideal
    min_decay_threshold: u16,       // Minimum 100 ppm drop
    occupancy_timeout: Duration,    // Time to assume unoccupied
}

impl CO2DecayDetector {
    pub async fn detect_decay_periods(
        &self,
        readings: &[AirQualityReading],
    ) -> Vec<DecayPeriod> {
        let mut periods = Vec::new();
        let mut current_period: Option<DecayPeriod> = None;

        for window in readings.windows(2) {
            let prev = &window[0];
            let curr = &window[1];

            let (Some(prev_co2), Some(curr_co2)) = (prev.co2_ppm, curr.co2_ppm) else {
                continue;
            };

            // Detect start of decay (unoccupied period)
            if prev_co2 > curr_co2 && prev_co2 > 800 {
                if current_period.is_none() {
                    current_period = Some(DecayPeriod {
                        start: prev.timestamp,
                        initial_co2: prev_co2,
                        ..Default::default()
                    });
                }
            }

            // Detect end of decay (re-occupancy)
            if curr_co2 > prev_co2 || curr.timestamp - current_period.as_ref().unwrap().start > self.window_size {
                if let Some(period) = current_period.take() {
                    periods.push(DecayPeriod {
                        end: curr.timestamp,
                        final_co2: curr_co2,
                        ..period
                    });
                }
            }
        }

        periods
    }
}

pub struct DecayPeriod {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub initial_co2: u16,
    pub final_co2: u16,
    pub ach_estimate: Option<f32>,
}
```

### 5.3 Mold Risk Index

**Temperature-Humidity Correlation:**

```rust
pub struct MoldRiskIndex {
    pub risk_score: f32,            // 0.0-1.0
    pub risk_level: MoldRiskLevel,
    pub hours_above_threshold: f32, // Time-weighted exposure
    pub contributing_factors: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum MoldRiskLevel {
    Minimal,        // < 0.2
    Low,            // 0.2-0.4
    Moderate,       // 0.4-0.6
    High,           // 0.6-0.8
    Critical,       // > 0.8
}

impl MoldRiskIndex {
    /// VTT Mold Growth Model (simplified)
    pub fn calculate(
        temperature_c: f32,
        relative_humidity: f32,
        surface_material: SurfaceMaterial,
        hours_exposed: f32,
    ) -> Self {
        let mut risk_score = 0.0;
        let mut factors = Vec::new();

        // Base RH risk (complementary relationship with temperature)
        let rh_risk = match relative_humidity {
            rh if rh < 50.0 => 0.0,
            rh if rh < 60.0 => (rh - 50.0) / 10.0 * 0.2,  // 0.0-0.2
            rh if rh < 65.0 => 0.2 + (rh - 60.0) / 5.0 * 0.2, // 0.2-0.4
            rh if rh < 80.0 => 0.4 + (rh - 65.0) / 15.0 * 0.4, // 0.4-0.8
            _ => 0.8 + (relative_humidity - 80.0) / 20.0 * 0.2, // 0.8-1.0
        };

        // Temperature amplification (70-80°F / 21-27°C optimal for mold)
        let temp_factor = if (21.0..=27.0).contains(&temperature_c) {
            1.2 // Amplify risk in optimal growth range
        } else if (16.0..=32.0).contains(&temperature_c) {
            1.0 // Normal risk
        } else {
            0.7 // Reduced risk outside growth range
        };

        risk_score = rh_risk * temp_factor;

        // Time-weighted exposure (chronic exposure increases risk)
        if hours_exposed > 24.0 && relative_humidity > 60.0 {
            risk_score *= 1.0 + (hours_exposed - 24.0) / 168.0; // Up to 2x over a week
            factors.push(format!("Chronic exposure: {:.1}h above 60% RH", hours_exposed));
        }

        // Material sensitivity
        let material_factor = match surface_material {
            SurfaceMaterial::Wood => 1.3,
            SurfaceMaterial::Drywall => 1.2,
            SurfaceMaterial::Fabric => 1.4,
            SurfaceMaterial::Tile => 0.8,
            SurfaceMaterial::Metal => 0.6,
        };

        risk_score *= material_factor;
        risk_score = risk_score.min(1.0); // Cap at 1.0

        if relative_humidity > 80.0 {
            factors.push("Critical humidity level".to_string());
        }

        if (21.0..=27.0).contains(&temperature_c) {
            factors.push("Optimal mold growth temperature".to_string());
        }

        MoldRiskIndex {
            risk_score,
            risk_level: Self::score_to_level(risk_score),
            hours_above_threshold: hours_exposed,
            contributing_factors: factors,
        }
    }

    fn score_to_level(score: f32) -> MoldRiskLevel {
        match score {
            x if x < 0.2 => MoldRiskLevel::Minimal,
            x if x < 0.4 => MoldRiskLevel::Low,
            x if x < 0.6 => MoldRiskLevel::Moderate,
            x if x < 0.8 => MoldRiskLevel::High,
            _ => MoldRiskLevel::Critical,
        }
    }
}

pub enum SurfaceMaterial {
    Wood,
    Drywall,
    Fabric,
    Tile,
    Metal,
}
```

### 5.4 Thermal Comfort Index (Simplified PMV/PPD)

**Predicted Mean Vote (PMV)** and **Predicted Percentage Dissatisfied (PPD)** (ASHRAE 55):

```rust
pub struct ThermalComfort {
    pub pmv: f32,                   // -3 (cold) to +3 (hot)
    pub ppd: f32,                   // 0-100%
    pub comfort_level: ComfortLevel,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum ComfortLevel {
    Comfortable,        // -0.5 < PMV < 0.5, PPD < 10%
    SlightlyWarm,       // 0.5 <= PMV < 1.0
    SlightlyCool,       // -1.0 < PMV <= -0.5
    Warm,               // 1.0 <= PMV < 2.0
    Cool,               // -2.0 < PMV <= -1.0
    Uncomfortable,      // |PMV| >= 2.0
}

impl ThermalComfort {
    /// Simplified PMV calculation (limited sensors - no air velocity/radiant temp)
    pub fn calculate_simplified(
        temperature_c: f32,
        relative_humidity: f32,
        metabolic_rate_met: f32,    // 1.0 = seated, 1.2 = standing, 1.6 = walking
        clothing_clo: f32,          // 0.5 = summer, 1.0 = winter
    ) -> Self {
        // Simplified PMV estimation (assumes still air, radiant temp = air temp)
        // Full ISO 7730 calculation requires iterative solver

        let air_velocity_ms = 0.1; // Assume still air

        // Simplified heat balance equation
        let metabolic_heat = metabolic_rate_met * 58.15; // W/m²
        let clothing_insulation = clothing_clo * 0.155;   // m²·K/W

        // Operative temperature (simplified: same as air temp without radiant sensor)
        let operative_temp = temperature_c;

        // PMV approximation (linear for |PMV| < 2)
        let pmv = 0.303 * (-0.036 * metabolic_heat).exp() + 0.028;
        let thermal_sensation = (operative_temp - 22.0) / 4.0; // Normalized around 22°C comfort
        let pmv = thermal_sensation * (1.0 + clothing_insulation) / (1.0 + air_velocity_ms);

        // PPD from Fanger's equation
        let ppd = 100.0 - 95.0 * (-0.03353 * pmv.powi(4) - 0.2179 * pmv.powi(2)).exp();

        let comfort_level = match pmv {
            x if x.abs() < 0.5 => ComfortLevel::Comfortable,
            x if x >= 0.5 && x < 1.0 => ComfortLevel::SlightlyWarm,
            x if x <= -0.5 && x > -1.0 => ComfortLevel::SlightlyCool,
            x if x >= 1.0 && x < 2.0 => ComfortLevel::Warm,
            x if x <= -1.0 && x > -2.0 => ComfortLevel::Cool,
            _ => ComfortLevel::Uncomfortable,
        };

        let mut recommendations = Vec::new();

        if pmv > 0.5 {
            recommendations.push("Consider lowering temperature or increasing air circulation".to_string());
        } else if pmv < -0.5 {
            recommendations.push("Consider raising temperature or adding clothing insulation".to_string());
        }

        if relative_humidity > 60.0 {
            recommendations.push("High humidity may increase thermal discomfort".to_string());
        }

        ThermalComfort {
            pmv,
            ppd,
            comfort_level,
            recommendations,
        }
    }
}
```

### 5.5 CO2 Trend Classification

```rust
#[derive(Debug, Clone, Copy)]
pub enum CO2Trend {
    Stable,         // < 50 ppm/hour change
    Rising,         // 50-150 ppm/hour
    RapidRise,      // > 150 ppm/hour (occupancy event)
    Falling,        // -50 to -150 ppm/hour
    RapidFall,      // < -150 ppm/hour (ventilation event)
}

impl CO2Trend {
    pub fn classify(current_ppm: u16, previous_ppm: u16, time_delta_minutes: f32) -> Self {
        let delta_ppm = current_ppm as i32 - previous_ppm as i32;
        let rate_ppm_per_hour = (delta_ppm as f32 / time_delta_minutes) * 60.0;

        match rate_ppm_per_hour {
            x if x > 150.0 => CO2Trend::RapidRise,
            x if x > 50.0 => CO2Trend::Rising,
            x if x < -150.0 => CO2Trend::RapidFall,
            x if x < -50.0 => CO2Trend::Falling,
            _ => CO2Trend::Stable,
        }
    }
}
```

---

## 6. Event Patterns

### 6.1 Event Detection Framework

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedEvent {
    pub event_type: EventType,
    pub timestamp: DateTime<Utc>,
    pub confidence: f32,            // 0.0-1.0
    pub detection_method: DetectionMethod,
    pub evidence: EventEvidence,
    pub recommended_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    Cooking(CookingEvent),
    WildfireSmoke(WildfireSmokeEvent),
    Occupancy(OccupancyEvent),
    HVACCycle(HVACEvent),
    WindowOpen(VentilationEvent),
    AnomalyDetected(AnomalyEvent),
}

#[derive(Debug, Clone)]
pub enum DetectionMethod {
    RuleBased,
    StatisticalAnomaly,
    MachineLearning(String), // Model name
    HybridEnsemble,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEvidence {
    pub pm25_spike: Option<f32>,        // µg/m³ increase
    pub voc_spike: Option<u16>,         // Index increase
    pub co2_rate: Option<f32>,          // ppm/hour
    pub io_ratio: Option<f32>,          // Indoor/outdoor PM2.5
    pub duration_minutes: Option<f32>,
    pub pattern_match_score: Option<f32>, // 0.0-1.0
}
```

### 6.2 Cooking Detection (PM2.5 Spike Signature)

| Pattern | Detection Signature | Confidence Method |
|---------|-------------------|-------------------|
| Start | PM2.5 rise > 10 µg/m³ in 5 min | > 0.8 if VOC correlates |
| Peak | 1-7 min after cooking ends (200-1400 µg/m³) | > 0.9 if time-of-day matches meal |
| Decay | Exponential decay (0.17-0.27 h⁻¹) | Pattern matching |
| Method signature | Pan-frying (92.9 µg/m³), stir-frying (26.7 µg/m³), boiling (0.7 µg/m³) | Clustering |

**Implementation:**

```rust
pub struct CookingEvent {
    pub cooking_method: Option<CookingMethod>,
    pub peak_pm25: f32,
    pub peak_voc_index: Option<u16>,
    pub duration_minutes: f32,
    pub ventilation_used: bool,         // Hood/window detected
}

#[derive(Debug, Clone, Copy)]
pub enum CookingMethod {
    PanFrying,      // Peak 50-150 µg/m³
    StirFrying,     // Peak 15-40 µg/m³
    DeepFrying,     // Peak 5-15 µg/m³
    Boiling,        // Peak < 2 µg/m³
    Baking,         // Peak < 5 µg/m³
    Unknown,
}

pub struct CookingDetector {
    baseline_pm25: f32,
    spike_threshold: f32,       // 10 µg/m³
    min_duration_minutes: f32,  // 2 minutes
    meal_time_priors: Vec<(u8, u8)>, // (hour_start, hour_end) for breakfast/lunch/dinner
}

impl CookingDetector {
    pub fn detect(&self, readings: &[AirQualityReading]) -> Vec<DetectedEvent> {
        let mut events = Vec::new();
        let mut in_cooking_event = false;
        let mut event_start_idx = 0;

        for (i, window) in readings.windows(5).enumerate() {
            let current = &window[4];
            let five_min_ago = &window[0];

            let Some(curr_pm25) = current.pm2_5_calibrated.or(current.pm2_5) else {
                continue;
            };
            let Some(prev_pm25) = five_min_ago.pm2_5_calibrated.or(five_min_ago.pm2_5) else {
                continue;
            };

            let delta = curr_pm25 - prev_pm25;

            // Detect spike start
            if !in_cooking_event && delta > self.spike_threshold {
                in_cooking_event = true;
                event_start_idx = i;
            }

            // Detect spike end (decay below baseline + threshold)
            if in_cooking_event && curr_pm25 < self.baseline_pm25 + 5.0 {
                let event_readings = &readings[event_start_idx..=i + 4];

                if let Some(event) = self.classify_cooking_event(event_readings) {
                    events.push(event);
                }

                in_cooking_event = false;
            }
        }

        events
    }

    fn classify_cooking_event(&self, readings: &[AirQualityReading]) -> Option<DetectedEvent> {
        let peak_pm25 = readings.iter()
            .filter_map(|r| r.pm2_5_calibrated.or(r.pm2_5))
            .max_by(|a, b| a.partial_cmp(b).unwrap())?;

        let peak_voc = readings.iter()
            .filter_map(|r| r.voc_index)
            .max();

        let duration_minutes = (readings.last()?.timestamp - readings.first()?.timestamp)
            .num_minutes() as f32;

        let cooking_method = match peak_pm25 {
            x if x > 50.0 => CookingMethod::PanFrying,
            x if x > 15.0 => CookingMethod::StirFrying,
            x if x > 5.0 => CookingMethod::DeepFrying,
            x if x > 2.0 => CookingMethod::Baking,
            _ => CookingMethod::Boiling,
        };

        // Time-of-day prior boost
        let hour = readings.first()?.timestamp.hour() as u8;
        let meal_time_match = self.meal_time_priors.iter()
            .any(|(start, end)| hour >= *start && hour < *end);

        let confidence = if meal_time_match && peak_voc.is_some() {
            0.9
        } else if meal_time_match || peak_voc.is_some() {
            0.7
        } else {
            0.5
        };

        Some(DetectedEvent {
            event_type: EventType::Cooking(CookingEvent {
                cooking_method: Some(cooking_method),
                peak_pm25,
                peak_voc_index: peak_voc,
                duration_minutes,
                ventilation_used: false, // TODO: Detect from decay rate
            }),
            timestamp: readings.first()?.timestamp,
            confidence,
            detection_method: DetectionMethod::RuleBased,
            evidence: EventEvidence {
                pm25_spike: Some(peak_pm25 - self.baseline_pm25),
                voc_spike: peak_voc.map(|v| v.saturating_sub(100)),
                duration_minutes: Some(duration_minutes),
                ..Default::default()
            },
            recommended_actions: vec![
                "Turn on range hood".to_string(),
                "Open windows for ventilation".to_string(),
            ],
        })
    }
}
```

### 6.3 Wildfire Smoke Infiltration

| Detection Pattern | Threshold | Confidence Method | Response |
|------------------|-----------|-------------------|----------|
| Outdoor PM2.5 spike | > 55 µg/m³ (USG) | PurpleAir/EPA API | Monitor I/O ratio |
| I/O ratio > 0.5 | Poor infiltration protection | Real-time sensor vs. API | Alert: Close windows, enable filtration |
| I/O ratio 0.2-0.5 | Moderate protection | MERV filter effectiveness | Recommend HEPA filter |
| I/O ratio < 0.2 | Good protection | High-efficiency filtration active | Continue monitoring |

**Implementation:**

```rust
pub struct WildfireSmokeEvent {
    pub outdoor_pm25: f32,
    pub indoor_pm25: f32,
    pub io_ratio: f32,
    pub filter_effectiveness: FilterEffectiveness,
    pub recommended_actions: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum FilterEffectiveness {
    Excellent,      // I/O < 0.2 (MERV13+/HEPA)
    Good,           // I/O 0.2-0.3
    Moderate,       // I/O 0.3-0.5 (MERV8-13)
    Poor,           // I/O 0.5-0.7 (minimal filtration)
    None,           // I/O > 0.7 (natural ventilation only)
}

pub struct WildfireSmokeDetector {
    outdoor_api_client: OutdoorAQClient,
    outdoor_threshold_ugm3: f32,    // 55 µg/m³
    io_ratio_alert_threshold: f32,  // 0.5
}

impl WildfireSmokeDetector {
    pub async fn detect(
        &self,
        indoor_reading: &AirQualityReading,
    ) -> Result<Option<DetectedEvent>> {
        let outdoor_pm25 = self.outdoor_api_client
            .get_nearest_pm25(indoor_reading.location_id.clone())
            .await?;

        if outdoor_pm25 < self.outdoor_threshold_ugm3 {
            return Ok(None); // No wildfire event
        }

        let indoor_pm25 = indoor_reading.pm2_5_calibrated
            .or(indoor_reading.pm2_5)
            .ok_or_else(|| anyhow::anyhow!("No indoor PM2.5 reading"))?;

        let io_ratio = indoor_pm25 / outdoor_pm25;

        if io_ratio > self.io_ratio_alert_threshold {
            let filter_effectiveness = match io_ratio {
                x if x > 0.7 => FilterEffectiveness::None,
                x if x > 0.5 => FilterEffectiveness::Poor,
                x if x > 0.3 => FilterEffectiveness::Moderate,
                x if x > 0.2 => FilterEffectiveness::Good,
                _ => FilterEffectiveness::Excellent,
            };

            let mut actions = vec![
                "Close all windows and doors immediately".to_string(),
                "Enable HVAC recirculation mode".to_string(),
            ];

            if matches!(filter_effectiveness, FilterEffectiveness::Poor | FilterEffectiveness::None) {
                actions.push("Deploy portable HEPA filter for 48-78% reduction".to_string());
                actions.push("Consider upgrading to MERV13+ filter (I/O ratio 0.12)".to_string());
            }

            Ok(Some(DetectedEvent {
                event_type: EventType::WildfireSmoke(WildfireSmokeEvent {
                    outdoor_pm25,
                    indoor_pm25,
                    io_ratio,
                    filter_effectiveness,
                    recommended_actions: actions.clone(),
                }),
                timestamp: indoor_reading.timestamp,
                confidence: 0.95, // High confidence with outdoor API correlation
                detection_method: DetectionMethod::RuleBased,
                evidence: EventEvidence {
                    pm25_spike: Some(indoor_pm25 - 10.0), // Baseline assumption
                    io_ratio: Some(io_ratio),
                    ..Default::default()
                },
                recommended_actions: actions,
            }))
        } else {
            Ok(None) // Adequate filtration protection
        }
    }
}
```

### 6.4 Occupancy Inference (CO2 Rise Rate)

| Pattern | Detection Signature | Confidence Method | Response |
|---------|-------------------|-------------------|----------|
| Entry | CO2 rise > 50 ppm/hour | > 0.7 if time-of-day matches | Log occupancy |
| Steady state | CO2 plateau 800-1500 ppm | Calculate occupant count | Estimate N people |
| Exit | CO2 decay begins | > 0.8 if ACH estimate consistent | Trigger ACH calculation |
| Count estimation | N = (dCO2/dt) × V / (G × (1 - η)) | ML-trained on historical data | Update occupancy model |

**Implementation:**

```rust
pub struct OccupancyEvent {
    pub occupancy_estimate: Option<u8>,    // Number of people
    pub occupancy_change: OccupancyChange,
    pub confidence: f32,
    pub co2_rise_rate_ppm_per_hour: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum OccupancyChange {
    Entry,          // CO2 rising
    Exit,           // CO2 falling
    Stable,         // CO2 plateau
}

pub struct OccupancyDetector {
    room_volume_m3: f32,
    co2_generation_rate_l_per_s_per_person: f32, // 0.005 (sedentary)
    ventilation_efficiency: f32,                  // 0.7-1.0
    baseline_outdoor_co2: u16,                    // 400 ppm
}

impl OccupancyDetector {
    pub fn estimate_occupancy(
        &self,
        current_co2: u16,
        previous_co2: u16,
        time_delta_minutes: f32,
    ) -> OccupancyEvent {
        let delta_co2 = current_co2 as i32 - previous_co2 as i32;
        let rise_rate_ppm_per_hour = (delta_co2 as f32 / time_delta_minutes) * 60.0;

        let occupancy_change = if rise_rate_ppm_per_hour > 50.0 {
            OccupancyChange::Entry
        } else if rise_rate_ppm_per_hour < -50.0 {
            OccupancyChange::Exit
        } else {
            OccupancyChange::Stable
        };

        // Estimate number of occupants (simplified mass balance)
        let occupancy_estimate = if matches!(occupancy_change, OccupancyChange::Stable) {
            let steady_state_co2 = current_co2 as f32;
            let excess_co2 = steady_state_co2 - self.baseline_outdoor_co2 as f32;

            // N = (excess_co2 * room_volume * ach) / (generation_rate * 3600 * 1000)
            // Simplified: N ≈ excess_co2 / 600 (rule of thumb for typical rooms)
            Some((excess_co2 / 600.0).round() as u8)
        } else {
            None
        };

        let confidence = match occupancy_change {
            OccupancyChange::Entry | OccupancyChange::Exit => 0.75,
            OccupancyChange::Stable if occupancy_estimate.is_some() => 0.6,
            _ => 0.4,
        };

        OccupancyEvent {
            occupancy_estimate,
            occupancy_change,
            confidence,
            co2_rise_rate_ppm_per_hour: rise_rate_ppm_per_hour,
        }
    }
}
```

### 6.5 HVAC Cycle Detection

```rust
pub struct HVACEvent {
    pub cycle_type: HVACCycleType,
    pub duration_minutes: f32,
    pub temperature_delta: f32,
    pub co2_impact: Option<f32>,        // Fresh air intake effect
}

#[derive(Debug, Clone, Copy)]
pub enum HVACCycleType {
    HeatingOn,
    HeatingOff,
    CoolingOn,
    CoolingOff,
    FanOnly,
    FreshAirIntake,
}

pub struct HVACDetector {
    temp_deadband: f32,         // ±0.5°C typical
    min_cycle_duration_min: f32, // 5 minutes
}

impl HVACDetector {
    pub fn detect(&self, readings: &[AirQualityReading]) -> Vec<DetectedEvent> {
        let mut events = Vec::new();

        for window in readings.windows(10) {
            let start = &window[0];
            let end = &window[9];

            let (Some(start_temp), Some(end_temp)) = (start.temperature_c, end.temperature_c) else {
                continue;
            };

            let temp_delta = end_temp - start_temp;

            if temp_delta.abs() > self.temp_deadband {
                let cycle_type = if temp_delta > 0.0 {
                    HVACCycleType::HeatingOn
                } else {
                    HVACCycleType::CoolingOn
                };

                // Check for fresh air intake (CO2 decrease during cycle)
                let co2_impact = if let (Some(start_co2), Some(end_co2)) = (start.co2_ppm, end.co2_ppm) {
                    Some((end_co2 as f32 - start_co2 as f32) / start_co2 as f32 * 100.0)
                } else {
                    None
                };

                events.push(DetectedEvent {
                    event_type: EventType::HVACCycle(HVACEvent {
                        cycle_type,
                        duration_minutes: (end.timestamp - start.timestamp).num_minutes() as f32,
                        temperature_delta: temp_delta,
                        co2_impact,
                    }),
                    timestamp: start.timestamp,
                    confidence: 0.8,
                    detection_method: DetectionMethod::StatisticalAnomaly,
                    evidence: EventEvidence {
                        ..Default::default()
                    },
                    recommended_actions: vec![],
                });
            }
        }

        events
    }
}
```

### 6.6 Window Open Detection

```rust
pub struct VentilationEvent {
    pub ventilation_type: VentilationType,
    pub effectiveness_score: f32,       // 0.0-1.0
    pub multi_sensor_correlation: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum VentilationType {
    WindowOpened,
    WindowClosed,
    DoorOpened,
    MechanicalVentilation,
}

pub struct VentilationDetector;

impl VentilationDetector {
    pub fn detect(&self, readings: &[AirQualityReading]) -> Option<DetectedEvent> {
        if readings.len() < 5 {
            return None;
        }

        let start = &readings[0];
        let end = &readings[readings.len() - 1];

        // Multi-sensor simultaneous change = window event
        let temp_drop = start.temperature_c? - end.temperature_c?;
        let humidity_drop = start.humidity_percent? - end.humidity_percent?;
        let co2_drop = start.co2_ppm? as f32 - end.co2_ppm? as f32;

        let multi_sensor_correlation = temp_drop > 1.0 && humidity_drop > 5.0 && co2_drop > 100.0;

        if multi_sensor_correlation {
            Some(DetectedEvent {
                event_type: EventType::WindowOpen(VentilationEvent {
                    ventilation_type: VentilationType::WindowOpened,
                    effectiveness_score: (co2_drop / 500.0).min(1.0),
                    multi_sensor_correlation: true,
                }),
                timestamp: start.timestamp,
                confidence: 0.85,
                detection_method: DetectionMethod::RuleBased,
                evidence: EventEvidence {
                    co2_rate: Some(co2_drop),
                    ..Default::default()
                },
                recommended_actions: vec![],
            })
        } else {
            None
        }
    }
}
```

---

## 7. Actions & Alerts

### 7.1 Action Framework

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AirQualityAction {
    Alert(AlertAction),
    Recommendation(RecommendationAction),
    Automation(AutomationAction),
    Log(LogAction),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertAction {
    pub severity: AlertSeverity,
    pub title: String,
    pub message: String,
    pub pollutant: Option<Pollutant>,
    pub current_value: f32,
    pub threshold_value: f32,
    pub delivery_channels: Vec<DeliveryChannel>,
    pub rate_limit_key: String,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertSeverity {
    Info,       // AQI 0-50, informational
    Notice,     // AQI 51-100, moderate
    Warning,    // AQI 101-150, USG
    Alert,      // AQI 151-200, unhealthy
    Critical,   // AQI 201-300, very unhealthy
    Emergency,  // AQI 301+, hazardous
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeliveryChannel {
    PushNotification(PushConfig),
    HomeKit,
    MQTT(String), // Topic
    Webhook(String), // URL
    Email(String),
    SMS(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushConfig {
    pub service: PushService,
    pub user_key: String,
    pub device: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PushService {
    Pushover,
    Ntfy,
    AppleHomeNotification,
}
```

### 7.2 Alert Severity Matrix

| AQI Range | Severity | Pollutant Triggers | Delivery | Rate Limit |
|-----------|----------|-------------------|----------|------------|
| 0-50 | Info | None | Log only | N/A |
| 51-100 | Notice | CO2 > 1000, PM2.5 > 9 | Log, optional push | 1/hour |
| 101-150 | Warning | CO2 > 1500, PM2.5 > 12, VOC > 150 | Push, HomeKit | 1/30min |
| 151-200 | Alert | PM2.5 > 35, VOC > 200 | Push, MQTT, HomeKit | 1/15min |
| 201-300 | Critical | PM2.5 > 55, VOC > 300 | All channels | 1/5min |
| 301+ | Emergency | PM2.5 > 150 | Immediate, all channels | None |

**Implementation:**

```rust
pub struct AlertManager {
    rate_limiters: HashMap<String, RateLimiter>,
    delivery_clients: DeliveryClients,
}

impl AlertManager {
    pub async fn trigger_alert(
        &mut self,
        reading: &AirQualityReading,
        aqi: &IndoorAQI,
    ) -> Result<Vec<AirQualityAction>> {
        let mut actions = Vec::new();

        // CO2 alert
        if let Some(co2) = reading.co2_ppm {
            let co2_level = CO2Level::from_ppm(co2);

            if matches!(co2_level, CO2Level::Moderate | CO2Level::Poor | CO2Level::VeryPoor) {
                let severity = match co2_level {
                    CO2Level::Moderate => AlertSeverity::Warning,
                    CO2Level::Poor => AlertSeverity::Alert,
                    CO2Level::VeryPoor => AlertSeverity::Critical,
                    _ => unreachable!(),
                };

                if self.should_alert("co2_high", severity) {
                    actions.push(AirQualityAction::Alert(AlertAction {
                        severity,
                        title: "High CO2 Level".to_string(),
                        message: format!(
                            "CO2 is {} ppm. {}",
                            co2,
                            co2_level.health_message()
                        ),
                        pollutant: Some(Pollutant::CO2),
                        current_value: co2 as f32,
                        threshold_value: 1000.0,
                        delivery_channels: self.channels_for_severity(severity),
                        rate_limit_key: "co2_high".to_string(),
                        expires_at: Some(Utc::now() + Duration::hours(1)),
                    }));
                }
            }
        }

        // PM2.5 alert
        if let Some(pm25) = reading.pm2_5_calibrated.or(reading.pm2_5) {
            let pm_level = PM25Level::from_ugm3(pm25);

            if matches!(pm_level, PM25Level::USG | PM25Level::Unhealthy | PM25Level::VeryUnhealthy | PM25Level::Hazardous) {
                let severity = match pm_level {
                    PM25Level::USG => AlertSeverity::Warning,
                    PM25Level::Unhealthy => AlertSeverity::Alert,
                    PM25Level::VeryUnhealthy => AlertSeverity::Critical,
                    PM25Level::Hazardous => AlertSeverity::Emergency,
                    _ => unreachable!(),
                };

                if self.should_alert("pm25_high", severity) {
                    actions.push(AirQualityAction::Alert(AlertAction {
                        severity,
                        title: "Poor Air Quality - PM2.5".to_string(),
                        message: format!(
                            "PM2.5 is {:.1} µg/m³. AQI: {}. Sensitive groups should limit outdoor exposure.",
                            pm25,
                            PM25Level::to_aqi(pm25),
                        ),
                        pollutant: Some(Pollutant::PM25),
                        current_value: pm25,
                        threshold_value: 35.0,
                        delivery_channels: self.channels_for_severity(severity),
                        rate_limit_key: "pm25_high".to_string(),
                        expires_at: Some(Utc::now() + Duration::minutes(30)),
                    }));
                }
            }
        }

        Ok(actions)
    }

    fn should_alert(&mut self, key: &str, severity: AlertSeverity) -> bool {
        let rate_limit = match severity {
            AlertSeverity::Info => Duration::hours(24),
            AlertSeverity::Notice => Duration::hours(1),
            AlertSeverity::Warning => Duration::minutes(30),
            AlertSeverity::Alert => Duration::minutes(15),
            AlertSeverity::Critical => Duration::minutes(5),
            AlertSeverity::Emergency => Duration::seconds(0), // No rate limit
        };

        self.rate_limiters
            .entry(key.to_string())
            .or_insert_with(|| RateLimiter::new(rate_limit))
            .check_and_update()
    }

    fn channels_for_severity(&self, severity: AlertSeverity) -> Vec<DeliveryChannel> {
        match severity {
            AlertSeverity::Info | AlertSeverity::Notice => vec![],
            AlertSeverity::Warning => vec![
                DeliveryChannel::HomeKit,
            ],
            AlertSeverity::Alert => vec![
                DeliveryChannel::PushNotification(PushConfig {
                    service: PushService::Pushover,
                    user_key: "...".to_string(),
                    device: None,
                }),
                DeliveryChannel::HomeKit,
            ],
            AlertSeverity::Critical | AlertSeverity::Emergency => vec![
                DeliveryChannel::PushNotification(PushConfig {
                    service: PushService::Pushover,
                    user_key: "...".to_string(),
                    device: None,
                }),
                DeliveryChannel::HomeKit,
                DeliveryChannel::MQTT("homeassistant/alerts".to_string()),
            ],
        }
    }
}

struct RateLimiter {
    last_triggered: Option<DateTime<Utc>>,
    min_interval: Duration,
}

impl RateLimiter {
    fn new(min_interval: Duration) -> Self {
        Self {
            last_triggered: None,
            min_interval,
        }
    }

    fn check_and_update(&mut self) -> bool {
        let now = Utc::now();

        if let Some(last) = self.last_triggered {
            if now - last < self.min_interval {
                return false; // Rate limited
            }
        }

        self.last_triggered = Some(now);
        true
    }
}
```

### 7.3 Recommendation Actions

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationAction {
    pub category: RecommendationCategory,
    pub priority: RecommendationPriority,
    pub title: String,
    pub description: String,
    pub expected_impact: String,
    pub difficulty: Difficulty,
}

#[derive(Debug, Clone, Copy)]
pub enum RecommendationCategory {
    Ventilation,
    Filtration,
    SourceControl,
    BehaviorChange,
    Maintenance,
}

#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
pub enum RecommendationPriority {
    Low,
    Medium,
    High,
    Urgent,
}

#[derive(Debug, Clone, Copy)]
pub enum Difficulty {
    Easy,       // < 1 minute
    Moderate,   // 1-10 minutes
    Hard,       // > 10 minutes or requires purchase
}

impl RecommendationAction {
    pub fn ventilate_immediately() -> Self {
        Self {
            category: RecommendationCategory::Ventilation,
            priority: RecommendationPriority::Urgent,
            title: "Increase Ventilation Immediately".to_string(),
            description: "Open windows and doors, or increase HVAC fresh air intake.".to_string(),
            expected_impact: "CO2 reduction: 300-500 ppm within 15 minutes".to_string(),
            difficulty: Difficulty::Easy,
        }
    }

    pub fn enable_hepa_filter() -> Self {
        Self {
            category: RecommendationCategory::Filtration,
            priority: RecommendationPriority::High,
            title: "Deploy HEPA Air Purifier".to_string(),
            description: "Portable HEPA filter can reduce PM2.5 by 48-78%.".to_string(),
            expected_impact: "PM2.5 reduction: 50-70% within 30 minutes".to_string(),
            difficulty: Difficulty::Moderate,
        }
    }

    pub fn calibrate_co2_sensor() -> Self {
        Self {
            category: RecommendationCategory::Maintenance,
            priority: RecommendationPriority::Medium,
            title: "Calibrate CO2 Sensor".to_string(),
            description: "Sensor has not been exposed to outdoor air in 30+ days. Manual calibration recommended.".to_string(),
            expected_impact: "Accuracy improvement: ±30 ppm".to_string(),
            difficulty: Difficulty::Moderate,
        }
    }
}
```

### 7.4 Automation Actions (HomeKit/MQTT)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationAction {
    pub automation_type: AutomationType,
    pub trigger_condition: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AutomationType {
    HomeKitUpdate,
    MQTTPublish(String), // Topic
    WebhookPost(String), // URL
    IFTTTTrigger(String), // Event name
}

impl AutomationAction {
    pub fn update_homekit_air_quality(aqi: &IndoorAQI) -> Self {
        let homekit_value = match aqi.category {
            AQICategory::Good => 1,
            AQICategory::Moderate => 2,
            AQICategory::USG => 3,
            AQICategory::Unhealthy => 4,
            AQICategory::VeryUnhealthy | AQICategory::Hazardous => 5,
        };

        Self {
            automation_type: AutomationType::HomeKitUpdate,
            trigger_condition: "AQI updated".to_string(),
            payload: serde_json::json!({
                "service": "AirQualitySensor",
                "characteristic": "AirQuality",
                "value": homekit_value,
            }),
        }
    }

    pub fn publish_mqtt_readings(reading: &AirQualityReading) -> Self {
        Self {
            automation_type: AutomationType::MQTTPublish("home/airquality".to_string()),
            trigger_condition: "New reading available".to_string(),
            payload: serde_json::to_value(reading).unwrap(),
        }
    }
}
```

---

## 8. Proto Schema

### 8.1 Core Event Schema

```protobuf
syntax = "proto3";

package air_quality.v1;

import "google/protobuf/timestamp.proto";

// Primary sensor reading event
message AirQualityEvent {
  // Metadata
  string location_id = 1;
  string sensor_id = 2;
  google.protobuf.Timestamp timestamp = 3;

  // CO2 measurement
  optional uint32 co2_ppm = 4;
  CO2QualityFlag co2_quality = 5;

  // Particulate matter (µg/m³)
  optional float pm1_0 = 6;
  optional float pm2_5 = 7;
  optional float pm2_5_calibrated = 8;
  optional float pm10 = 9;
  PMQualityFlag pm_quality = 10;

  // VOC/NOx index (1-500)
  optional uint32 voc_index = 11;
  optional uint32 nox_index = 12;
  uint32 voc_baseline = 13;

  // Environmental
  optional float temperature_c = 14;
  optional float humidity_percent = 15;

  // Data quality
  ReadingQuality quality = 16;
  uint64 sensor_uptime_seconds = 17;
}

enum CO2QualityFlag {
  CO2_QUALITY_UNKNOWN = 0;
  CO2_QUALITY_GOOD = 1;
  CO2_QUALITY_WARMING = 2;
  CO2_QUALITY_ABC_STALE = 3;
  CO2_QUALITY_MANUAL_REQUIRED = 4;
  CO2_QUALITY_FAILED = 5;
}

enum PMQualityFlag {
  PM_QUALITY_UNKNOWN = 0;
  PM_QUALITY_GOOD = 1;
  PM_QUALITY_HIGH_HUMIDITY = 2;
  PM_QUALITY_FAN_DEGRADED = 3;
  PM_QUALITY_SATURATED = 4;
  PM_QUALITY_CALIBRATION_REQUIRED = 5;
}

message ReadingQuality {
  float overall_score = 1;          // 0.0-1.0
  float completeness = 2;           // 0.0-1.0
  uint64 freshness_ms = 3;
  repeated string validity_flags = 4;
}
```

### 8.2 Derived Metrics Schema

```protobuf
// Indoor AQI calculation result
message IndoorAQI {
  uint32 overall_aqi = 1;           // 0-500
  Pollutant dominant_pollutant = 2;
  uint32 co2_aqi = 3;
  uint32 pm25_aqi = 4;
  uint32 voc_aqi = 5;
  AQICategory category = 6;
  google.protobuf.Timestamp calculated_at = 7;
}

enum Pollutant {
  POLLUTANT_UNKNOWN = 0;
  POLLUTANT_CO2 = 1;
  POLLUTANT_PM25 = 2;
  POLLUTANT_PM10 = 3;
  POLLUTANT_VOC = 4;
  POLLUTANT_NOX = 5;
}

enum AQICategory {
  AQI_CATEGORY_UNKNOWN = 0;
  AQI_CATEGORY_GOOD = 1;
  AQI_CATEGORY_MODERATE = 2;
  AQI_CATEGORY_USG = 3;
  AQI_CATEGORY_UNHEALTHY = 4;
  AQI_CATEGORY_VERY_UNHEALTHY = 5;
  AQI_CATEGORY_HAZARDOUS = 6;
}

// Ventilation adequacy metrics
message VentilationMetrics {
  optional float ach_estimate = 1;      // Air changes per hour
  VentilationAdequacy adequacy = 2;
  optional float co2_decay_rate = 3;    // ppm/hour
  uint32 outdoor_co2_baseline = 4;
  float confidence = 5;
  google.protobuf.Timestamp calculated_at = 6;
}

enum VentilationAdequacy {
  VENTILATION_UNKNOWN = 0;
  VENTILATION_EXCELLENT = 1;
  VENTILATION_GOOD = 2;
  VENTILATION_ADEQUATE = 3;
  VENTILATION_MARGINAL = 4;
  VENTILATION_POOR = 5;
}

// Mold risk assessment
message MoldRiskMetrics {
  float risk_score = 1;                 // 0.0-1.0
  MoldRiskLevel risk_level = 2;
  float hours_above_threshold = 3;
  repeated string contributing_factors = 4;
  google.protobuf.Timestamp calculated_at = 5;
}

enum MoldRiskLevel {
  MOLD_RISK_UNKNOWN = 0;
  MOLD_RISK_MINIMAL = 1;
  MOLD_RISK_LOW = 2;
  MOLD_RISK_MODERATE = 3;
  MOLD_RISK_HIGH = 4;
  MOLD_RISK_CRITICAL = 5;
}

// Thermal comfort metrics
message ThermalComfort {
  float pmv = 1;                        // -3 to +3
  float ppd = 2;                        // 0-100%
  ComfortLevel comfort_level = 3;
  repeated string recommendations = 4;
  google.protobuf.Timestamp calculated_at = 5;
}

enum ComfortLevel {
  COMFORT_UNKNOWN = 0;
  COMFORT_COMFORTABLE = 1;
  COMFORT_SLIGHTLY_WARM = 2;
  COMFORT_SLIGHTLY_COOL = 3;
  COMFORT_WARM = 4;
  COMFORT_COOL = 5;
  COMFORT_UNCOMFORTABLE = 6;
}
```

### 8.3 Event Detection Schema

```protobuf
// Detected event envelope
message DetectedEvent {
  EventType event_type = 1;
  google.protobuf.Timestamp timestamp = 2;
  float confidence = 3;                 // 0.0-1.0
  DetectionMethod detection_method = 4;
  EventEvidence evidence = 5;
  repeated string recommended_actions = 6;
}

message EventType {
  oneof type {
    CookingEvent cooking = 1;
    WildfireSmokeEvent wildfire_smoke = 2;
    OccupancyEvent occupancy = 3;
    HVACEvent hvac = 4;
    VentilationEvent ventilation = 5;
    AnomalyEvent anomaly = 6;
  }
}

enum DetectionMethod {
  DETECTION_UNKNOWN = 0;
  DETECTION_RULE_BASED = 1;
  DETECTION_STATISTICAL = 2;
  DETECTION_ML = 3;
  DETECTION_HYBRID = 4;
}

message EventEvidence {
  optional float pm25_spike = 1;
  optional uint32 voc_spike = 2;
  optional float co2_rate = 3;
  optional float io_ratio = 4;
  optional float duration_minutes = 5;
  optional float pattern_match_score = 6;
}

message CookingEvent {
  optional CookingMethod cooking_method = 1;
  float peak_pm25 = 2;
  optional uint32 peak_voc_index = 3;
  float duration_minutes = 4;
  �� ventilation_used = 5;
}

enum CookingMethod {
  COOKING_METHOD_UNKNOWN = 0;
  COOKING_METHOD_PAN_FRYING = 1;
  COOKING_METHOD_STIR_FRYING = 2;
  COOKING_METHOD_DEEP_FRYING = 3;
  COOKING_METHOD_BOILING = 4;
  COOKING_METHOD_BAKING = 5;
}

message WildfireSmokeEvent {
  float outdoor_pm25 = 1;
  float indoor_pm25 = 2;
  float io_ratio = 3;
  FilterEffectiveness filter_effectiveness = 4;
}

enum FilterEffectiveness {
  FILTER_EFFECTIVENESS_UNKNOWN = 0;
  FILTER_EFFECTIVENESS_EXCELLENT = 1;
  FILTER_EFFECTIVENESS_GOOD = 2;
  FILTER_EFFECTIVENESS_MODERATE = 3;
  FILTER_EFFECTIVENESS_POOR = 4;
  FILTER_EFFECTIVENESS_NONE = 5;
}

message OccupancyEvent {
  optional uint32 occupancy_estimate = 1;
  OccupancyChange occupancy_change = 2;
  float confidence = 3;
  float co2_rise_rate_ppm_per_hour = 4;
}

enum OccupancyChange {
  OCCUPANCY_CHANGE_UNKNOWN = 0;
  OCCUPANCY_CHANGE_ENTRY = 1;
  OCCUPANCY_CHANGE_EXIT = 2;
  OCCUPANCY_CHANGE_STABLE = 3;
}

message HVACEvent {
  HVACCycleType cycle_type = 1;
  float duration_minutes = 2;
  float temperature_delta = 3;
  optional float co2_impact = 4;
}

enum HVACCycleType {
  HVAC_CYCLE_UNKNOWN = 0;
  HVAC_CYCLE_HEATING_ON = 1;
  HVAC_CYCLE_HEATING_OFF = 2;
  HVAC_CYCLE_COOLING_ON = 3;
  HVAC_CYCLE_COOLING_OFF = 4;
  HVAC_CYCLE_FAN_ONLY = 5;
  HVAC_CYCLE_FRESH_AIR = 6;
}

message VentilationEvent {
  VentilationType ventilation_type = 1;
  float effectiveness_score = 2;
  bool multi_sensor_correlation = 3;
}

enum VentilationType {
  VENTILATION_TYPE_UNKNOWN = 0;
  VENTILATION_TYPE_WINDOW_OPENED = 1;
  VENTILATION_TYPE_WINDOW_CLOSED = 2;
  VENTILATION_TYPE_DOOR_OPENED = 3;
  VENTILATION_TYPE_MECHANICAL = 4;
}

message AnomalyEvent {
  string anomaly_type = 1;
  float anomaly_score = 2;
  string description = 3;
}
```

### 8.4 Alert Schema

```protobuf
message AirQualityAlert {
  AlertSeverity severity = 1;
  string title = 2;
  string message = 3;
  optional Pollutant pollutant = 4;
  float current_value = 5;
  float threshold_value = 6;
  repeated DeliveryChannel delivery_channels = 7;
  string rate_limit_key = 8;
  optional google.protobuf.Timestamp expires_at = 9;
  google.protobuf.Timestamp created_at = 10;
}

enum AlertSeverity {
  ALERT_SEVERITY_UNKNOWN = 0;
  ALERT_SEVERITY_INFO = 1;
  ALERT_SEVERITY_NOTICE = 2;
  ALERT_SEVERITY_WARNING = 3;
  ALERT_SEVERITY_ALERT = 4;
  ALERT_SEVERITY_CRITICAL = 5;
  ALERT_SEVERITY_EMERGENCY = 6;
}

message DeliveryChannel {
  oneof channel {
    PushConfig push_notification = 1;
    bool homekit = 2;
    string mqtt_topic = 3;
    string webhook_url = 4;
    string email = 5;
    string sms = 6;
  }
}

message PushConfig {
  PushService service = 1;
  string user_key = 2;
  optional string device = 3;
}

enum PushService {
  PUSH_SERVICE_UNKNOWN = 0;
  PUSH_SERVICE_PUSHOVER = 1;
  PUSH_SERVICE_NTFY = 2;
  PUSH_SERVICE_APPLE_HOME = 3;
}
```

---

## 9. MCP Tools

### 9.1 Tool Definitions

```rust
use mcp_server::Tool;

pub struct AirQualityMCPTools;

impl AirQualityMCPTools {
    pub fn get_current_readings() -> Tool {
        Tool {
            name: "get_current_readings".to_string(),
            description: "Get current air quality readings from all sensors".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "location_id": {
                        "type": "string",
                        "description": "Location identifier (defaults to primary sensor)"
                    },
                    "include_derived": {
                        "type": "boolean",
                        "description": "Include derived metrics (AQI, ventilation, mold risk)"
                    }
                },
                "required": []
            }),
        }
    }

    pub fn get_air_quality_forecast() -> Tool {
        Tool {
            name: "get_air_quality_forecast".to_string(),
            description: "Get ML-based air quality forecast for next 1-6 hours".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "location_id": {
                        "type": "string"
                    },
                    "forecast_hours": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 6,
                        "description": "Forecast horizon in hours"
                    },
                    "pollutants": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": ["co2", "pm2_5", "voc", "all"]
                        },
                        "description": "Which pollutants to forecast"
                    }
                },
                "required": ["forecast_hours"]
            }),
        }
    }

    pub fn analyze_ventilation() -> Tool {
        Tool {
            name: "analyze_ventilation".to_string(),
            description: "Analyze ventilation adequacy and calculate ACH from recent CO2 patterns".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "location_id": {
                        "type": "string"
                    },
                    "lookback_hours": {
                        "type": "integer",
                        "minimum": 3,
                        "maximum": 24,
                        "description": "Hours of historical data to analyze"
                    },
                    "space_type": {
                        "type": "string",
                        "enum": ["residential", "office", "school", "hospital"],
                        "description": "Building type for adequacy assessment"
                    }
                },
                "required": []
            }),
        }
    }

    pub fn get_health_recommendations() -> Tool {
        Tool {
            name: "get_health_recommendations".to_string(),
            description: "Get personalized health recommendations based on current air quality and detected events".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "location_id": {
                        "type": "string"
                    },
                    "occupant_profile": {
                        "type": "object",
                        "properties": {
                            "age": { "type": "integer" },
                            "health_conditions": {
                                "type": "array",
                                "items": {
                                    "type": "string",
                                    "enum": ["asthma", "copd", "cardiovascular", "pregnancy", "none"]
                                }
                            },
                            "activity_level": {
                                "type": "string",
                                "enum": ["sedentary", "light", "moderate", "vigorous"]
                            }
                        }
                    }
                },
                "required": []
            }),
        }
    }

    pub fn explain_reading() -> Tool {
        Tool {
            name: "explain_reading".to_string(),
            description: "Get a human-friendly explanation of current air quality reading and what it means for health".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "location_id": {
                        "type": "string"
                    },
                    "detail_level": {
                        "type": "string",
                        "enum": ["brief", "detailed", "technical"],
                        "description": "Level of detail in explanation"
                    }
                },
                "required": []
            }),
        }
    }

    pub fn detect_events() -> Tool {
        Tool {
            name: "detect_events".to_string(),
            description: "Detect air quality events (cooking, occupancy changes, HVAC cycles, etc.) from recent data".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "location_id": {
                        "type": "string"
                    },
                    "lookback_hours": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 24
                    },
                    "event_types": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": ["cooking", "occupancy", "hvac", "ventilation", "wildfire_smoke", "all"]
                        }
                    }
                },
                "required": []
            }),
        }
    }

    pub fn configure_alerts() -> Tool {
        Tool {
            name: "configure_alerts".to_string(),
            description: "Configure alert thresholds and delivery preferences".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "location_id": {
                        "type": "string"
                    },
                    "thresholds": {
                        "type": "object",
                        "properties": {
                            "co2_warning_ppm": { "type": "integer" },
                            "pm25_warning_ugm3": { "type": "number" },
                            "voc_warning_index": { "type": "integer" }
                        }
                    },
                    "delivery_channels": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": ["push", "homekit", "mqtt", "webhook"]
                        }
                    }
                },
                "required": []
            }),
        }
    }
}
```

---

## 10. Data Quality Rules

### 10.1 Sensor Warm-Up Handling

```rust
pub struct SensorWarmupManager {
    installation_timestamp: DateTime<Utc>,
    warmup_periods: HashMap<SensorType, Duration>,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum SensorType {
    CO2,
    PM,
    VOC,
    TempHumidity,
}

impl SensorWarmupManager {
    pub fn new(installation_timestamp: DateTime<Utc>) -> Self {
        let mut warmup_periods = HashMap::new();
        warmup_periods.insert(SensorType::CO2, Duration::weeks(3)); // ABC stabilization
        warmup_periods.insert(SensorType::PM, Duration::hours(24)); // Fan stabilization
        warmup_periods.insert(SensorType::VOC, Duration::hours(24)); // Baseline learning
        warmup_periods.insert(SensorType::TempHumidity, Duration::minutes(5)); // Minimal

        Self {
            installation_timestamp,
            warmup_periods,
        }
    }

    pub fn is_warmed_up(&self, sensor_type: SensorType) -> bool {
        let elapsed = Utc::now() - self.installation_timestamp;
        elapsed >= *self.warmup_periods.get(&sensor_type).unwrap()
    }

    pub fn warmup_progress(&self, sensor_type: SensorType) -> f32 {
        let elapsed = Utc::now() - self.installation_timestamp;
        let required = *self.warmup_periods.get(&sensor_type).unwrap();
        (elapsed.num_seconds() as f32 / required.num_seconds() as f32).min(1.0)
    }
}
```

### 10.2 Outlier Detection

```rust
pub struct OutlierDetector {
    zscore_threshold: f32,      // 3.0 typical
    iqr_multiplier: f32,        // 1.5 typical
}

impl OutlierDetector {
    /// Z-score method for normally distributed data
    pub fn detect_zscore(&self, value: f32, historical_values: &[f32]) -> bool {
        if historical_values.len() < 10 {
            return false; // Insufficient data
        }

        let mean = historical_values.iter().sum::<f32>() / historical_values.len() as f32;
        let variance = historical_values.iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f32>() / historical_values.len() as f32;
        let std_dev = variance.sqrt();

        let zscore = (value - mean) / std_dev;
        zscore.abs() > self.zscore_threshold
    }

    /// IQR method for skewed distributions (PM2.5, VOC)
    pub fn detect_iqr(&self, value: f32, historical_values: &[f32]) -> bool {
        if historical_values.len() < 10 {
            return false;
        }

        let mut sorted = historical_values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let q1_idx = sorted.len() / 4;
        let q3_idx = 3 * sorted.len() / 4;
        let q1 = sorted[q1_idx];
        let q3 = sorted[q3_idx];
        let iqr = q3 - q1;

        let lower_bound = q1 - self.iqr_multiplier * iqr;
        let upper_bound = q3 + self.iqr_multiplier * iqr;

        value < lower_bound || value > upper_bound
    }

    /// Rate-of-change outlier (sudden spikes)
    pub fn detect_rate_outlier(&self, current: f32, previous: f32, max_rate_per_minute: f32) -> bool {
        let rate = (current - previous).abs();
        rate > max_rate_per_minute
    }
}
```

### 10.3 Missing Data Interpolation

```rust
pub struct DataInterpolator;

impl DataInterpolator {
    /// Linear interpolation for short gaps (< 5 minutes)
    pub fn linear_interpolate(
        prev: &AirQualityReading,
        next: &AirQualityReading,
        target_timestamp: DateTime<Utc>,
    ) -> AirQualityReading {
        let total_duration = (next.timestamp - prev.timestamp).num_seconds() as f32;
        let elapsed = (target_timestamp - prev.timestamp).num_seconds() as f32;
        let ratio = elapsed / total_duration;

        let co2_ppm = match (prev.co2_ppm, next.co2_ppm) {
            (Some(p), Some(n)) => Some(Self::lerp(p as f32, n as f32, ratio) as u16),
            _ => None,
        };

        let pm2_5 = match (prev.pm2_5, next.pm2_5) {
            (Some(p), Some(n)) => Some(Self::lerp(p, n, ratio)),
            _ => None,
        };

        AirQualityReading {
            location_id: prev.location_id.clone(),
            sensor_id: prev.sensor_id.clone(),
            timestamp: target_timestamp,
            co2_ppm,
            pm2_5,
            ..Default::default()
        }
    }

    fn lerp(a: f32, b: f32, t: f32) -> f32 {
        a + (b - a) * t
    }

    /// Forward-fill for medium gaps (5-15 minutes) - last valid value
    pub fn forward_fill(last_valid: &AirQualityReading) -> AirQualityReading {
        AirQualityReading {
            timestamp: Utc::now(),
            ..last_valid.clone()
        }
    }

    /// Mark as missing for long gaps (> 15 minutes) - do not interpolate
    pub fn mark_missing() -> AirQualityReading {
        AirQualityReading {
            timestamp: Utc::now(),
            co2_ppm: None,
            pm2_5: None,
            voc_index: None,
            ..Default::default()
        }
    }
}
```

### 10.4 Calibration Drift Detection

```rust
pub struct CalibrationDriftDetector {
    reference_readings: HashMap<SensorType, Vec<(DateTime<Utc>, f32)>>,
    drift_threshold_percent: f32,  // 10% typical
}

impl CalibrationDriftDetector {
    /// Compare against known reference conditions
    pub fn detect_co2_drift(&self, current_outdoor_reading: u16) -> bool {
        // Outdoor air should be ~400 ppm
        let expected = 400.0;
        let actual = current_outdoor_reading as f32;
        let drift_percent = (actual - expected).abs() / expected * 100.0;

        drift_percent > self.drift_threshold_percent
    }

    /// PM sensor drift via inter-sensor comparison
    pub fn detect_pm_drift(&self, sensor_a_pm25: f32, sensor_b_pm25: f32) -> bool {
        // Collocated sensors should read within 20%
        let mean = (sensor_a_pm25 + sensor_b_pm25) / 2.0;
        let diff_percent = (sensor_a_pm25 - sensor_b_pm25).abs() / mean * 100.0;

        diff_percent > 20.0
    }

    /// VOC baseline drift (should stabilize around 100 after 24h)
    pub fn detect_voc_baseline_drift(&self, baseline_history: &[(DateTime<Utc>, u16)]) -> bool {
        if baseline_history.len() < 24 {
            return false; // Insufficient data
        }

        // Check if baseline is consistently drifting from 100
        let recent_baselines: Vec<u16> = baseline_history.iter()
            .rev()
            .take(24)
            .map(|(_, baseline)| *baseline)
            .collect();

        let mean_baseline = recent_baselines.iter().sum::<u16>() as f32 / recent_baselines.len() as f32;
        let drift_from_nominal = (mean_baseline - 100.0).abs();

        drift_from_nominal > 20.0 // > 20% drift from nominal
    }
}
```

---

## 11. Summary

This Air Quality Domain Specification provides a comprehensive foundation for building an indoor air quality monitoring and analytics platform using the AirGradient ONE sensor suite.

**Key Components:**
1. **Sensor Suite**: SenseAir S8/S88 (CO2), Plantower PMS5003 (PM), Sensirion SGP41 (VOC/NOx), SHT4x (temp/humidity)
2. **Health Thresholds**: EPA 2024 standards, WHO guidelines, cognitive impact research
3. **Derived Metrics**: Indoor AQI, ACH ventilation, mold risk, thermal comfort, CO2 trends
4. **Event Detection**: Cooking (PM2.5 spikes), wildfire smoke (I/O ratio), occupancy (CO2 rise), HVAC cycles, window opening
5. **Actions**: Multi-level alerts (Info→Emergency), recommendations, HomeKit/MQTT automation
6. **Proto Schema**: Complete event, metric, and alert definitions for event-driven architecture
7. **MCP Tools**: 7 Claude-integrated tools for querying, forecasting, and configuring
8. **Data Quality**: Warm-up handling, outlier detection, interpolation, calibration drift detection

**Next Steps:**
1. Implement Rust types and Proto generation
2. Build data ingestion pipeline (poll AirGradient ONE every 60s)
3. Develop event detection algorithms
4. Train ML forecasting models (LSTM, NHITS)
5. Create MCP server integration
6. Build HomeKit/MQTT adapters
7. Deploy Grafana dashboards

This specification is ready for implementation in the neural-data-platform architecture.
