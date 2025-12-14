# AirGradient Config Endpoint - Specification Gap Analysis

**Document Version:** 1.0.0
**Date:** December 13, 2025
**Status:** Review - Critical Missing Functionality
**Author:** Code Review Agent

---

## Executive Summary

This document analyzes the **undocumented** AirGradient `/config` endpoint that provides critical device configuration data affecting data interpretation, sensor calibration, and platform operations. The current specification (v1.1.0) **completely omits** this endpoint, creating significant gaps in data quality assessment and multi-device support.

### Key Findings

**Critical Gaps:**
1. No support for config endpoint retrieval or parsing
2. Missing temperature unit conversion (°F vs °C)
3. Missing PM2.5 correction algorithm awareness (EPA 2021 vs raw)
4. Missing CO2 ABC calibration status
5. Missing VOC/NOx learning offset tracking
6. No device configuration validation

**Impact:**
- Data misinterpretation (temperature in Fahrenheit assumed as Celsius = 24.47°C vs 75°F actual)
- Incorrect PM2.5 values (compensated vs raw readings)
- Unreliable CO2 forecasts during ABC period
- Poor sensor health assessments

---

## 1. Config Endpoint Overview

### 1.1 Actual Endpoint Response

**URL:** `http://airgradient_{SERIAL}.local/config`
**Method:** GET
**Format:** JSON
**Frequency:** Static configuration (changes infrequently)

```json
{
  "country": "US",
  "pmStandard": "ugm3",
  "ledBarMode": "co2",
  "abcDays": 8,
  "tvocLearningOffset": 12,
  "noxLearningOffset": 12,
  "mqttBrokerUrl": "",
  "httpDomain": "",
  "temperatureUnit": "f",
  "disableCloudConnection": false,
  "configurationControl": "both",
  "postDataToAirGradient": true,
  "ledBarBrightness": 100,
  "displayBrightness": 100,
  "offlineMode": false,
  "monitorDisplayCompensatedValues": false,
  "model": "I-9PSL-DE",
  "corrections": {
    "pm02": {
      "correctionAlgorithm": "epa_2021",
      "slr": null
    }
  }
}
```

### 1.2 Specification Status

**Current Specification (v1.1.0):**
- FR-1.2 mentions `/measures/current` endpoint only
- No reference to `/config` endpoint anywhere
- No configuration awareness in data parsing
- No sensor calibration status tracking

**Status:** Complete gap - endpoint not mentioned or implemented

---

## 2. Complete Field Catalog

### 2.1 Core Configuration Fields

| Field | Type | Values | Description | Platform Impact |
|-------|------|--------|-------------|-----------------|
| `country` | String | "US", "EU", "JP", etc. | Device locale (affects defaults) | **Medium** - Affects threshold defaults |
| `pmStandard` | String | "ugm3", "usaqi" | PM display units | **High** - Data unit interpretation |
| `temperatureUnit` | String | "c", "f" | Temperature unit | **Critical** - Unit conversion required |
| `model` | String | "I-9PSL", "I-9PSL-DE" | Hardware model variant | **Low** - Metadata tracking |
| `offlineMode` | Boolean | true, false | Offline operation mode | **Medium** - Data source availability |

**Criticality:** HIGH - Affects core data interpretation

### 2.2 Sensor Calibration Fields

| Field | Type | Values | Description | Platform Impact |
|-------|------|--------|-------------|-----------------|
| `abcDays` | Integer | 0-255 | CO2 ABC calibration period (days) | **Critical** - CO2 quality scoring |
| `tvocLearningOffset` | Integer | 12 (typical) | VOC sensor learning hours | **High** - VOC quality scoring |
| `noxLearningOffset` | Integer | 12 (typical) | NOx sensor learning hours | **High** - NOx quality scoring |

**Criticality:** CRITICAL - Directly affects FR-1.3 quality assessment

#### 2.2.1 CO2 ABC Calibration Impact

**Problem:** CO2 sensors using Automatic Baseline Calibration (ABC) require periodic exposure to outdoor air (400ppm reference).

**Configuration Details:**
- `abcDays: 8` means sensor recalibrates every 8 days
- During first 3 weeks after deployment, readings are unreliable (warmup period)
- If device never exposed to outdoor air, ABC will drift

**Platform Requirements:**
1. Track device first-seen timestamp → flag CO2 readings as "warmup" for 21 days
2. Monitor time since last ABC event → warn if >30 days without outdoor exposure
3. Apply quality penalty during warmup: `quality_score *= 0.7` (per FR-1.3)

**Specification Gap:**
- FR-1.3 mentions warmup period but has no mechanism to track it
- No config-driven calibration status awareness

#### 2.2.2 VOC/NOx Learning Offset Impact

**Problem:** Sensirion SGP41 sensors require 12-hour learning period to establish baseline.

**Configuration Details:**
- `tvocLearningOffset: 12` = sensor needs 12 hours of continuous operation
- Index values are **relative to 24-hour baseline**, not absolute concentrations
- Readings during first 12 hours are unstable

**Platform Requirements:**
1. Track device first-seen timestamp → flag VOC/NOx as "learning" for 12 hours
2. Quality penalty during learning: `quality_score *= 0.8`
3. Do not trigger VOC alerts during learning period

**Specification Gap:**
- FR-1.3 does not mention VOC/NOx learning periods
- No tracking of sensor warmup state

### 2.3 PM2.5 Correction Fields

| Field | Type | Values | Description | Platform Impact |
|-------|------|--------|-------------|-----------------|
| `corrections.pm02.correctionAlgorithm` | String | "none", "epa_2021", "lrapa" | PM2.5 correction algorithm | **Critical** - Which PM field to use |
| `corrections.pm02.slr` | Float/null | null or 0.0-2.0 | Simple Linear Regression coefficient | **Medium** - Custom correction |
| `monitorDisplayCompensatedValues` | Boolean | true, false | Which values shown on display | **Low** - Documentation clarity |

**Criticality:** CRITICAL - Affects which PM2.5 field to trust

#### 2.3.1 PM2.5 Correction Algorithm Impact

**Problem:** AirGradient devices can apply different PM2.5 correction algorithms based on config.

**Configuration Options:**
1. **"none"**: Use raw `pm02` value from PMS5003
2. **"epa_2021"**: Use EPA-corrected `pm02Compensated` (humidity-adjusted)
3. **"lrapa"**: Use LRAPA correction (different humidity model)

**Platform Requirements:**
1. Parse config to determine which field is authoritative:
   - If `correctionAlgorithm == "epa_2021"` → use `pm02Compensated`
   - If `correctionAlgorithm == "none"` → use `pm02`
2. Store algorithm name in metadata for audit trail
3. Apply health thresholds to correct field (EPA AQI uses compensated values)

**Specification Gap:**
- FR-1.2 schema includes both `pm02` and `pm02Compensated` but no guidance on which to use
- FR-5.1 health thresholds assume single PM2.5 value without correction awareness
- No configuration retrieval or parsing

**Specification Impact:**
```yaml
# Current FR-5.1 (INCORRECT)
PM2.5 thresholds: >12 µg/m³ (USG), >35 µg/m³ (Unhealthy)

# Should be (CORRECT):
PM2.5 thresholds:
  source: "pm02Compensated" if correctionAlgorithm != "none" else "pm02"
  values: >12 µg/m³ (USG), >35 µg/m³ (Unhealthy)
```

### 2.4 MQTT Configuration Fields

| Field | Type | Values | Description | Platform Impact |
|-------|------|--------|-------------|-----------------|
| `mqttBrokerUrl` | String | "" or URL | Custom MQTT broker | **High** - Connection config |
| `postDataToAirGradient` | Boolean | true, false | Cloud upload enabled | **Low** - Privacy awareness |
| `disableCloudConnection` | Boolean | true, false | Cloud disabled entirely | **Low** - Privacy awareness |

**Criticality:** HIGH - Affects MQTT connection strategy

#### 2.4.1 MQTT Broker Configuration Impact

**Problem:** Devices can use custom MQTT brokers or built-in cloud broker.

**Configuration Details:**
- `mqttBrokerUrl: ""` (empty) → using AirGradient cloud broker
- `mqttBrokerUrl: "mqtt://local:1883"` → using custom broker
- `disableCloudConnection: true` → no external connectivity (local API only)

**Platform Requirements:**
1. If custom broker: Platform must connect to same broker
2. If empty broker: Platform can use AirGradient cloud (requires API key)
3. If cloud disabled: Platform MUST use Local API only (no MQTT available)

**Specification Gap:**
- FR-1.1 assumes MQTT always available
- FR-1.4 data source selection (`mqtt | local_api | both`) has no config awareness
- No handling of cloud-disabled devices

### 2.5 Display Configuration Fields

| Field | Type | Values | Description | Platform Impact |
|-------|------|--------|-------------|-----------------|
| `ledBarMode` | String | "co2", "pm", "off" | LED bar display mode | **Low** - Informational |
| `ledBarBrightness` | Integer | 0-100 | LED brightness % | **None** |
| `displayBrightness` | Integer | 0-100 | OLED brightness % | **None** |
| `configurationControl` | String | "both", "local", "cloud" | Config source priority | **Medium** - Config mutability |

**Criticality:** LOW - Mostly informational

### 2.6 Connectivity Fields

| Field | Type | Values | Description | Platform Impact |
|-------|------|--------|-------------|-----------------|
| `httpDomain` | String | "" or URL | Custom API domain | **Low** - Future use |

**Criticality:** LOW - Future extensibility

---

## 3. Specification Gap Analysis

### 3.1 Missing Functional Requirements

#### Gap 1: Config Endpoint Retrieval

**Missing Requirement:** FR-1.X: Device Configuration Retrieval

**Proposed:** **FR-1.5: Device Configuration Retrieval**

**Description:** Retrieve device configuration from `/config` endpoint to determine data source capabilities and sensor calibration status

**Acceptance Criteria:**
- Poll `/config` endpoint on platform startup and every 6 hours
- Parse complete config JSON schema (19 fields)
- Cache config in memory with last-updated timestamp
- Support multi-device configurations (hash map: `serial → config`)
- Retry config fetch on failure (exponential backoff: 5s, 10s, 30s)
- Fall back to defaults if config unavailable after 3 retries:
  ```rust
  default_config = {
    temperatureUnit: "c",
    pmStandard: "ugm3",
    corrections: { pm02: { correctionAlgorithm: "none" } },
    abcDays: 8,
    tvocLearningOffset: 12,
    noxLearningOffset: 12,
  }
  ```
- Log config changes on refresh (diff previous vs new)

**Priority:** HIGH

**Dependencies:**
- reqwest crate (HTTP client)
- serde_json (JSON parsing)

**Schema:**
```rust
#[derive(Debug, Deserialize)]
struct AirGradientConfig {
    country: String,
    pm_standard: String, // "ugm3" | "usaqi"
    temperature_unit: String, // "c" | "f"
    abc_days: u8,
    tvoc_learning_offset: u8,
    nox_learning_offset: u8,
    mqtt_broker_url: String,
    offline_mode: bool,
    corrections: PmCorrections,
    model: String,
}

#[derive(Debug, Deserialize)]
struct PmCorrections {
    pm02: PmCorrectionConfig,
}

#[derive(Debug, Deserialize)]
struct PmCorrectionConfig {
    correction_algorithm: String, // "none" | "epa_2021" | "lrapa"
    slr: Option<f64>,
}
```

**Rationale:** Foundation for all config-aware features

---

#### Gap 2: Temperature Unit Conversion

**Missing Requirement:** FR-1.X: Temperature Data Unit Conversion

**Proposed:** **FR-1.6: Temperature Unit Conversion**

**Description:** Convert temperature readings to Celsius based on device `temperatureUnit` configuration

**Acceptance Criteria:**
- Parse `temperatureUnit` from config ("c" or "f")
- If `temperatureUnit == "f"`:
  - Convert `atmp` and `atmpCompensated` using: `celsius = (fahrenheit - 32) × 5/9`
  - Round to 2 decimal places
  - Log conversion: `"Temperature converted from 75.0°F to 23.89°C"`
- If `temperatureUnit == "c"`:
  - Use values as-is (no conversion)
- Store original unit in metadata: `temperature_unit_source: "f"`
- Normalize all stored temperatures to Celsius (canonical format)
- Support querying in original or normalized units

**Priority:** CRITICAL

**Example:**
```rust
// Config: temperatureUnit = "f"
// Raw: atmp = 75.0
// Stored: atmp = 23.89, metadata: { temp_unit_source: "f" }

// Config: temperatureUnit = "c"
// Raw: atmp = 24.47
// Stored: atmp = 24.47, metadata: { temp_unit_source: "c" }
```

**Impact:** Without this, temperature data is corrupted (75°F interpreted as 75°C = 167°F!)

**Rationale:** Essential for correct data interpretation and multi-device support (US devices use °F, EU devices use °C)

---

#### Gap 3: PM2.5 Field Selection

**Missing Requirement:** FR-1.X: PM2.5 Correction Algorithm Awareness

**Proposed:** **FR-1.7: PM2.5 Correction Algorithm Awareness**

**Description:** Select authoritative PM2.5 field based on device correction algorithm configuration

**Acceptance Criteria:**
- Parse `corrections.pm02.correctionAlgorithm` from config
- Field selection logic:
  ```rust
  let authoritative_pm25 = match config.corrections.pm02.correction_algorithm.as_str() {
      "epa_2021" => reading.pm02_compensated,
      "lrapa" => reading.pm02_compensated,
      "none" => reading.pm02,
      _ => reading.pm02, // fallback to raw
  };
  ```
- Store both values but flag authoritative:
  ```
  pm02_raw: 7
  pm02_compensated: 6
  pm02_authoritative: 6  # <-- used for alerts/forecasts
  correction_algorithm: "epa_2021"
  ```
- Use authoritative value for:
  - Health threshold alerts (FR-5.1)
  - Forecasting input (FR-4.1)
  - MCP tool responses (FR-6.1)
- Expose both values in queries for debugging

**Priority:** CRITICAL

**Example Impact:**
```yaml
# Scenario: High humidity (>80% RH)
# Raw PM2.5: 15 µg/m³ (hygroscopic growth artifacts)
# EPA 2021 compensated: 11 µg/m³ (corrected for humidity)

# WRONG (using raw):
Alert: "PM2.5 exceeds USG threshold (15 > 12 µg/m³)"

# CORRECT (using compensated):
No alert (11 < 12 µg/m³)
```

**Rationale:** EPA AQI health thresholds assume EPA 2021 correction; using raw values causes false alerts

---

#### Gap 4: Sensor Warmup Tracking

**Missing Requirement:** FR-1.3 mentions warmup but no implementation

**Proposed:** **FR-1.8: Sensor Calibration Status Tracking**

**Description:** Track sensor warmup/learning periods based on config and device uptime

**Acceptance Criteria:**
- Maintain sensor state machine per device:
  ```rust
  enum SensorCalibrationState {
      Warmup { started_at: DateTime, duration_hours: u32 },
      Learning { started_at: DateTime, duration_hours: u8 },
      Active,
      Stale { last_calibration: DateTime },
  }
  ```
- CO2 calibration state:
  - **Warmup:** First 3 weeks (504 hours) after first reading
  - **Active:** After warmup, ABC active (recalibrates every `abcDays`)
  - **Stale:** >30 days since last known outdoor exposure (warn user)
- VOC/NOx calibration state:
  - **Learning:** First `tvocLearningOffset` hours (default 12h)
  - **Active:** After learning period
- Quality score modifiers (enhance FR-1.3):
  ```rust
  let quality_modifier = match sensor_state {
      Warmup { .. } => 0.7,  // CO2 warmup
      Learning { .. } => 0.8, // VOC/NOx learning
      Stale { .. } => 0.9,    // CO2 needs recalibration
      Active => 1.0,
  };
  quality_score *= quality_modifier;
  ```
- Quality flags:
  - `["co2_warmup"]` - CO2 in first 3 weeks
  - `["voc_learning"]` - VOC/NOx in first 12 hours
  - `["co2_stale"]` - CO2 needs outdoor exposure
- Suppress alerts during warmup/learning (FR-5.1 enhancement)

**Priority:** HIGH

**Storage:**
```parquet
# New schema fields
calibration_state: Utf8  # "warmup" | "learning" | "active" | "stale"
device_age_hours: UInt32 # hours since first reading
last_abc_event: Timestamp # CO2 calibration timestamp
```

**Rationale:** Prevents false alerts and poor forecasts during sensor stabilization

---

#### Gap 5: Data Source Availability

**Missing Requirement:** FR-1.4 assumes dual sources always available

**Proposed:** **FR-1.9: Data Source Capability Detection**

**Description:** Determine available data sources (MQTT, Local API, both) based on device configuration

**Acceptance Criteria:**
- Parse config to detect capabilities:
  ```rust
  enum DataSourceCapability {
      MqttOnly,      // mqttBrokerUrl set, offlineMode=false
      LocalApiOnly,  // disableCloudConnection=true OR mqttBrokerUrl empty
      Both,          // MQTT enabled + local API reachable
  }
  ```
- Detection logic:
  ```rust
  let capability = if config.offline_mode || config.disable_cloud_connection {
      DataSourceCapability::LocalApiOnly
  } else if !config.mqtt_broker_url.is_empty() {
      DataSourceCapability::Both  // verify with connectivity test
  } else {
      DataSourceCapability::Both  // default to AirGradient cloud
  };
  ```
- Override user-configured `data_source` if incompatible:
  ```yaml
  # config.yaml: data_source = "mqtt"
  # Device config: offlineMode = true
  # Result: Auto-fallback to "local_api" with warning log
  ```
- Health check (FR-NFR-5.3 enhancement):
  ```json
  {
    "status": "degraded",
    "mqtt": "unavailable",
    "local_api": "connected",
    "effective_source": "local_api"
  }
  ```

**Priority:** MEDIUM

**Rationale:** Prevents connection failures and clarifies monitoring

---

### 3.2 Missing Non-Functional Requirements

#### Gap 6: Config Refresh Rate

**Missing Requirement:** NFR-X: Configuration Refresh Performance

**Proposed:** **NFR-2.6: Configuration Refresh Performance**

**Requirement:** Periodic config refresh without impacting ingestion latency

**Measurement:**
- Config fetch latency: p95 <500ms
- Refresh interval: 6 hours (configurable)
- Zero ingestion delay during config refresh (async operation)

**Acceptance:**
- Config fetched in background thread
- Cache invalidation triggers revalidation (pm field selection, unit conversion)
- Metric: `config_refresh_duration_ms` histogram

**Priority:** MEDIUM

**Rationale:** Config changes are rare but important (e.g., user changes PM correction algorithm)

---

#### Gap 7: Config Validation

**Missing Requirement:** NFR-X: Configuration Validation

**Proposed:** **NFR-4.5: Configuration Validation and Compatibility**

**Requirement:** Validate device config compatibility with platform capabilities

**Validation Rules:**
1. **Temperature Unit:** Must be "c" or "f" (reject unknown)
2. **PM Standard:** Must be "ugm3" or "usaqi" (warn if unsupported)
3. **Correction Algorithm:** Must be "none", "epa_2021", or "lrapa" (fallback to "none")
4. **ABC Days:** Must be 0-255 (reject out of range)
5. **Learning Offset:** Must be 1-255 (reject 0)

**Error Handling:**
- Invalid config → use defaults + log warning
- Partial config → merge with defaults
- Config fetch timeout → use cached config (if available) or defaults

**Acceptance:**
- 100% of invalid configs handled gracefully (no crashes)
- Clear error messages: `"Invalid temperatureUnit 'k': expected 'c' or 'f', using default 'c'"`

**Priority:** MEDIUM

**Rationale:** Defensive programming against device firmware bugs or unsupported variants

---

### 3.3 Enhancements to Existing Requirements

#### Enhancement 1: FR-1.2 Schema Update

**Original FR-1.2:**
> Parse incoming MQTT/HTTP JSON payloads and validate against AirGradient schema

**Enhancement:**
Add config awareness to schema:

```diff
  - Parse complete AirGradient ONE JSON payload (29+ fields from firmware 3.1.4+)
+ - Retrieve device configuration from /config endpoint (FR-1.5)
+ - Apply unit conversions based on config (FR-1.6):
+   - Temperature: Convert °F → °C if temperatureUnit = "f"
+ - Select authoritative PM2.5 field based on config (FR-1.7):
+   - Use pm02Compensated if correctionAlgorithm = "epa_2021" or "lrapa"
+   - Use pm02 if correctionAlgorithm = "none"
+ - Validate ranges using config-appropriate units (post-conversion)
```

---

#### Enhancement 2: FR-1.3 Quality Assessment

**Original FR-1.3:**
> Quality score = completeness × calibration_status × freshness_factor

**Enhancement:**
Add config-driven calibration awareness:

```diff
  - Calibration status:
-   - CO2 sensor warmup period (<3 weeks) = 0.7x penalty
+   - CO2 sensor warmup period (first 3 weeks after deployment) = 0.7x penalty
+     - Track via device first-seen timestamp + config.abcDays
+   - VOC/NOx learning period (first config.tvocLearningOffset hours) = 0.8x penalty
+   - CO2 stale (>30 days without ABC event) = 0.9x penalty
    - PM high humidity (>80% RH) = 0.9x penalty
- - Attach quality flags: `["co2_warmup_period", "pm_high_humidity"]`
+ - Attach quality flags:
+   - `["co2_warmup"]` - First 3 weeks
+   - `["voc_learning"]` - First config.tvocLearningOffset hours
+   - `["nox_learning"]` - First config.noxLearningOffset hours
+   - `["co2_stale"]` - >30 days since ABC
+   - `["pm_high_humidity"]` - RH >80%
```

---

#### Enhancement 3: FR-5.1 Health Threshold Alerts

**Original FR-5.1:**
> PM2.5 thresholds: >12 µg/m³ (USG), >35 µg/m³ (Unhealthy)

**Enhancement:**
Use config-aware PM2.5 field:

```diff
  - PM2.5 thresholds: >12 µg/m³ (USG), >35 µg/m³ (Unhealthy), >55 µg/m³ (Very Unhealthy)
+   - Source field: Use pm02Compensated if config.corrections.pm02.correctionAlgorithm != "none"
+   - Note: EPA AQI thresholds assume EPA 2021 correction applied
+ - Suppress alerts during sensor warmup/learning:
+   - CO2 alerts: Suppress for first 3 weeks (warmup)
+   - VOC alerts: Suppress for first config.tvocLearningOffset hours (learning)
```

---

#### Enhancement 4: FR-6.1 Air Quality Query Tool

**Original FR-6.1:**
> Output: JSON with readings and health interpretations

**Enhancement:**
Include config metadata:

```diff
  - Example: `{
      co2_ppm: 850,
      co2_level: "Acceptable",
+     co2_calibration_status: "active",
      pm25_ugm3: 8.2,
      pm25_level: "Good",
+     pm25_correction_algorithm: "epa_2021",
+     temperature_c: 23.9,
+     temperature_source_unit: "f",
      ...
    }`
```

---

#### Enhancement 5: FR-6.4 Sensor Health Tool

**Original FR-6.4:**
> Output: `{status, last_reading_age_seconds, co2_calibration_status, pm_quality}`

**Enhancement:**
Add config-driven status details:

```diff
  - Output: `{
      status: "online" | "offline" | "degraded",
      last_reading_age_seconds: 120,
-     co2_calibration_status: "warming" | "active" | "stale",
+     co2_calibration_status: "warmup" | "active" | "stale",
+     co2_abc_days: 8,
+     co2_next_abc: "2025-12-21T00:00:00Z",
+     voc_calibration_status: "learning" | "active",
+     voc_learning_hours_remaining: 4,
      pm_quality: "good" | "high_humidity" | "saturated",
+     pm_correction_algorithm: "epa_2021",
+     temperature_unit: "f",
+     data_source_capability: "both" | "mqtt_only" | "local_api_only"
    }`
```

---

## 4. Proposed New Functional Requirements

### FR-8: Device Configuration Management

**FR-8.1: Configuration Retrieval and Caching**
- **Description:** Fetch and cache device configuration from `/config` endpoint
- **Priority:** HIGH
- **See:** Gap 1 (Section 3.1)

**FR-8.2: Temperature Unit Normalization**
- **Description:** Convert temperature readings to canonical Celsius
- **Priority:** CRITICAL
- **See:** Gap 2 (Section 3.1)

**FR-8.3: PM2.5 Correction Algorithm Awareness**
- **Description:** Select authoritative PM2.5 field based on device correction algorithm
- **Priority:** CRITICAL
- **See:** Gap 3 (Section 3.1)

**FR-8.4: Sensor Calibration Status Tracking**
- **Description:** Track CO2 ABC and VOC/NOx learning status per config
- **Priority:** HIGH
- **See:** Gap 4 (Section 3.1)

**FR-8.5: Data Source Capability Detection**
- **Description:** Auto-detect MQTT vs Local API availability from config
- **Priority:** MEDIUM
- **See:** Gap 5 (Section 3.1)

**FR-8.6: Configuration Change Detection**
- **Description:** Detect and log configuration changes on refresh
- **Acceptance Criteria:**
  - Compare fetched config with cached config (field-by-field diff)
  - Log changes: `"Config changed: temperatureUnit 'c' → 'f', requires data revalidation"`
  - Trigger dependent actions:
    - Temperature unit change → revalidate recent readings (last 24h)
    - PM correction change → recalculate authoritative PM values (last 24h)
    - ABC days change → recalculate CO2 calibration schedule
  - Metric: `config_changes_total` counter
- **Priority:** LOW

**FR-8.7: Multi-Device Configuration**
- **Description:** Support heterogeneous device configurations (different units, corrections per device)
- **Acceptance Criteria:**
  - Store config per device: `HashMap<SerialNumber, AirGradientConfig>`
  - Apply device-specific conversions during parsing
  - Support queries filtering by config: `"Show devices using EPA 2021 correction"`
  - MCP tool: `list_device_configs` → table of all device configurations
- **Priority:** MEDIUM

**FR-8.8: Configuration Validation and Defaults**
- **Description:** Validate device config and fall back to safe defaults on errors
- **Priority:** MEDIUM
- **See:** NFR-4.5 (Section 3.2)

---

## 5. Implementation Priority Matrix

### 5.1 Critical Path (Phase 1 - Week 1)

**Must-have for correct data interpretation:**

1. **FR-8.1: Config Retrieval** (2 days)
   - Fetch `/config` on startup
   - Parse JSON schema
   - Cache in memory

2. **FR-8.2: Temperature Unit Conversion** (1 day)
   - Parse `temperatureUnit`
   - Apply °F → °C conversion
   - Store normalized values

3. **FR-8.3: PM2.5 Field Selection** (1 day)
   - Parse `correctionAlgorithm`
   - Select authoritative field
   - Update FR-5.1 alerting

**Dependencies:** None (can parallelize with existing Phase 1 work)

---

### 5.2 High Priority (Phase 2 - Week 2-3)

**Important for quality assessment:**

4. **FR-8.4: Sensor Calibration Tracking** (3 days)
   - Implement state machine (warmup/learning/active/stale)
   - Track device first-seen timestamp
   - Apply quality penalties (FR-1.3 enhancement)
   - Suppress alerts during warmup (FR-5.1 enhancement)

5. **FR-8.6: Config Change Detection** (1 day)
   - Implement diff logic
   - Trigger revalidation
   - Add logging

**Dependencies:** FR-8.1 (config retrieval)

---

### 5.3 Medium Priority (Phase 3 - Week 4)

**Nice-to-have for robustness:**

6. **FR-8.5: Data Source Capability Detection** (2 days)
   - Parse MQTT/offline config
   - Auto-fallback logic
   - Health check enhancement

7. **FR-8.7: Multi-Device Configuration** (2 days)
   - Per-device config storage
   - Heterogeneous config support
   - MCP list tool

8. **FR-8.8: Config Validation** (1 day)
   - Validation rules
   - Default fallbacks
   - Error handling

**Dependencies:** FR-8.1

---

### 5.4 Low Priority (Phase 4+ - Future)

**Future enhancements:**

9. **Config-based model selection** (future)
   - Train separate models per correction algorithm
   - Model registry: `nhits_pm25_epa2021`, `nhits_pm25_raw`

10. **Config synchronization** (future)
    - Push config changes to device via POST `/config`
    - Audit trail of config changes

---

## 6. Testing Strategy

### 6.1 Unit Tests

**Test Coverage Requirements:**

```rust
#[cfg(test)]
mod tests {
    // FR-8.1: Config Retrieval
    #[test]
    fn test_parse_config_json() { }

    #[test]
    fn test_config_fetch_timeout_fallback() { }

    // FR-8.2: Temperature Conversion
    #[test]
    fn test_fahrenheit_to_celsius_conversion() {
        // Input: 75.0°F
        // Expected: 23.89°C
    }

    #[test]
    fn test_celsius_passthrough() {
        // Input: 24.47°C
        // Expected: 24.47°C (no conversion)
    }

    // FR-8.3: PM2.5 Field Selection
    #[test]
    fn test_pm25_epa2021_uses_compensated() {
        // correctionAlgorithm = "epa_2021"
        // Expected: authoritative = pm02Compensated
    }

    #[test]
    fn test_pm25_none_uses_raw() {
        // correctionAlgorithm = "none"
        // Expected: authoritative = pm02
    }

    // FR-8.4: Calibration Tracking
    #[test]
    fn test_co2_warmup_first_3_weeks() {
        // device_age = 10 days
        // Expected: state = Warmup, quality_modifier = 0.7
    }

    #[test]
    fn test_voc_learning_first_12h() {
        // device_age = 6 hours
        // Expected: state = Learning, quality_modifier = 0.8
    }
}
```

---

### 6.2 Integration Tests

**Scenario-Based Testing:**

```rust
#[tokio::test]
async fn test_e2e_fahrenheit_device() {
    // 1. Mock /config endpoint: temperatureUnit = "f"
    // 2. Mock /measures/current: atmp = 75.0
    // 3. Ingest reading
    // 4. Query stored value
    // Expected: atmp = 23.89, metadata.temp_unit_source = "f"
}

#[tokio::test]
async fn test_e2e_epa2021_correction() {
    // 1. Mock /config: correctionAlgorithm = "epa_2021"
    // 2. Mock /measures: pm02 = 15, pm02Compensated = 11
    // 3. Trigger health check
    // Expected: No alert (11 < 12 µg/m³ USG threshold)
}

#[tokio::test]
async fn test_e2e_co2_warmup_suppression() {
    // 1. Mock device first-seen = now() - 10 days
    // 2. Mock /config: abcDays = 8
    // 3. Mock reading: rco2 = 1600
    // Expected: No alert (warmup period)
}
```

---

### 6.3 Manual Testing Checklist

**Real Device Validation:**

- [ ] Deploy to real AirGradient ONE device (US config: °F, EPA 2021)
- [ ] Verify temperature conversion: Display shows 75°F, platform stores 23.89°C
- [ ] Verify PM2.5 selection: High humidity → compensated used for alerts
- [ ] Deploy to second device (EU config: °C, no correction)
- [ ] Verify heterogeneous config: Both devices work simultaneously
- [ ] Change device config (via AirGradient dashboard): temperatureUnit °F → °C
- [ ] Verify config refresh detects change within 6 hours
- [ ] Verify recent data revalidated with new config

---

## 7. Risks and Mitigations

### 7.1 Technical Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Config endpoint unavailable on older firmware | High | Medium | Fall back to defaults, log warning, continue operation |
| Config format changes in future firmware | Medium | Low | Version detection via firmware field, schema migration |
| Temperature conversion errors (precision loss) | Low | Low | Use f64 for intermediate calculations, round only on storage |
| PM field selection wrong (algorithm mismatch) | High | Low | Extensive unit tests, integration tests with real data |
| Config refresh too frequent (network overhead) | Low | Low | 6-hour default interval, configurable |

---

### 7.2 Data Quality Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Historical data pre-config-awareness is wrong | High | Document cutover date, provide reprocessing tool |
| Users assume raw PM values when corrected used | Medium | Clear metadata in MCP responses, documentation |
| Mixed unit temperatures in same query | Medium | Always normalize to Celsius, expose source unit in metadata |
| Warmup/learning periods not detected | High | Conservative defaults (assume warmup if device age unknown) |

---

### 7.3 Operational Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Config fetch timeout delays startup | Medium | Async fetch, use cached/default config, continue startup |
| Config change mid-day causes alert flapping | Medium | Graceful transition: revalidate only recent data (24h window) |
| Multiple devices with different configs | Low | Per-device config storage, clear metadata in queries |

---

## 8. Documentation Requirements

### 8.1 User Documentation

**New Documentation Sections:**

1. **Device Configuration Guide** (`docs/device-configuration.md`)
   - Explanation of `/config` endpoint
   - Impact of temperature unit, PM correction, ABC settings
   - Recommended settings for different use cases

2. **Data Interpretation Guide** (`docs/data-interpretation.md`)
   - How to tell if temperature is °F or °C
   - Which PM2.5 value to trust (raw vs compensated)
   - Sensor warmup/learning periods

3. **Troubleshooting Guide** (`docs/troubleshooting.md`)
   - "Why are my temperatures wrong?" → Check temperatureUnit
   - "Why are PM2.5 alerts incorrect?" → Check correctionAlgorithm
   - "Why are CO2 readings unstable?" → Check device age (warmup period)

---

### 8.2 Developer Documentation

**Code Documentation:**

```rust
/// Retrieves device configuration from /config endpoint
///
/// # Configuration Fields
/// - `temperatureUnit`: "c" or "f" (affects FR-8.2 conversion)
/// - `corrections.pm02.correctionAlgorithm`: "none" | "epa_2021" | "lrapa" (affects FR-8.3 field selection)
/// - `abcDays`: CO2 ABC period (affects FR-8.4 calibration tracking)
///
/// # Returns
/// - `Ok(AirGradientConfig)`: Parsed configuration
/// - `Err(ConfigError::Timeout)`: Fetch timeout (uses defaults)
/// - `Err(ConfigError::ParseError)`: Invalid JSON (uses defaults)
///
/// # Example
/// ```rust
/// let config = fetch_config("http://airgradient_abc123.local").await?;
/// assert_eq!(config.temperature_unit, "f");
/// ```
async fn fetch_config(base_url: &str) -> Result<AirGradientConfig, ConfigError> {
    // Implementation
}
```

---

### 8.3 API Documentation

**MCP Tool Updates:**

```yaml
# air_quality_sensor_health (FR-6.4 enhancement)
output:
  temperature_unit: "f"  # NEW: From config
  pm_correction_algorithm: "epa_2021"  # NEW: From config
  co2_abc_days: 8  # NEW: From config
  co2_calibration_status: "active"  # ENHANCED: Config-driven
  voc_calibration_status: "learning"  # NEW: Config-driven
  voc_learning_hours_remaining: 4  # NEW: Based on tvocLearningOffset
```

---

## 9. Backward Compatibility

### 9.1 Data Migration

**Historical Data Issues:**

**Problem:** Existing data may have wrong temperature units or PM2.5 field selection

**Solution 1: Reprocess Historical Data** (recommended)
1. Fetch current config for each device
2. Assume config unchanged since deployment (document assumption)
3. Reprocess Parquet files:
   - Apply temperature conversion if `temperatureUnit = "f"`
   - Recalculate `pm02_authoritative` based on `correctionAlgorithm`
4. Write reprocessed Parquet files with metadata: `reprocessed_at`, `config_version`

**Solution 2: Mark Historical Data as Unvalidated** (fallback)
1. Add quality flag: `["pre_config_awareness"]`
2. Reduce quality score: `quality_score *= 0.9`
3. Document limitations in user guide

**Timeline:** Phase 5 (Week 8) - optional migration tool

---

### 9.2 Configuration Schema Versioning

**Future-Proofing:**

```rust
#[derive(Debug, Deserialize)]
struct AirGradientConfig {
    #[serde(default = "default_schema_version")]
    schema_version: String,  // "1.0", "1.1", etc.

    // Existing fields...
}

fn default_schema_version() -> String {
    "1.0".to_string()
}

// Schema migration
impl AirGradientConfig {
    fn migrate(mut self) -> Result<Self, ConfigError> {
        match self.schema_version.as_str() {
            "1.0" => Ok(self),
            "1.1" => {
                // Future: handle new fields
                Ok(self)
            },
            _ => Err(ConfigError::UnsupportedVersion(self.schema_version)),
        }
    }
}
```

---

## 10. Success Metrics

### 10.1 Functional Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Config fetch success rate | >99% | `config_fetch_success_total / config_fetch_attempts_total` |
| Temperature conversion accuracy | 100% | Unit tests: ±0.01°C tolerance |
| PM field selection accuracy | 100% | Unit tests: correct field for each algorithm |
| Calibration state accuracy | >95% | Integration tests: warmup/learning detection |
| Config change detection latency | <6 hours | Time between config change and platform awareness |

---

### 10.2 Data Quality Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Temperature data validity | 100% | No values >50°C or <-10°C after conversion |
| PM2.5 field consistency | 100% | Authoritative field matches algorithm |
| Warmup period false alert rate | 0% | No CO2/VOC alerts during known warmup |
| Config-driven quality scoring | >90% | Percentage of readings with calibration-aware quality score |

---

### 10.3 Operational Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Config fetch latency (p95) | <500ms | `histogram(config_fetch_duration_ms)` |
| Config cache hit rate | >99% | `config_cache_hits / config_requests` |
| Config refresh overhead | <1% | CPU/network impact during refresh |
| Multi-device config support | 100% | Heterogeneous configs work correctly |

---

## 11. Recommendations

### 11.1 Immediate Actions (This Sprint)

1. **Implement FR-8.1, FR-8.2, FR-8.3** (Critical Path)
   - 4 days of development effort
   - Zero data quality without these features
   - Required before any production deployment

2. **Update FR-1.2, FR-1.3, FR-5.1** (Enhancements)
   - Integrate config-aware parsing
   - Add calibration status tracking
   - Fix alerting to use correct PM field

3. **Add Unit Tests** (FR-8.x Coverage)
   - Temperature conversion tests
   - PM field selection tests
   - Config parsing tests

---

### 11.2 Next Sprint Actions

4. **Implement FR-8.4** (Calibration Tracking)
   - 3 days of development effort
   - Improves quality scoring significantly
   - Prevents false alerts

5. **Integration Testing** (Real Device Validation)
   - Test with US device (°F, EPA 2021)
   - Test with EU device (°C, no correction)
   - Verify multi-device support

6. **Documentation** (User + Developer Guides)
   - Device configuration guide
   - Data interpretation guide
   - API documentation updates

---

### 11.3 Future Enhancements (Post-v1.0)

7. **Historical Data Reprocessing Tool** (v1.1)
   - Reprocess existing Parquet files with config awareness
   - Add `reprocessed_at` metadata

8. **Config Change Auditing** (v1.1)
   - Log all config changes with timestamps
   - Expose config history via MCP tool

9. **Device Config Push** (v2.0)
   - Allow platform to update device config via POST `/config`
   - Sync config across multiple devices

10. **Advanced Calibration Features** (v2.0)
    - Detect ABC events (CO2 drops to ~400ppm)
    - Recommend outdoor exposure schedule
    - Track sensor drift over time

---

## 12. Conclusion

The **AirGradient `/config` endpoint** provides critical metadata that is **completely missing** from the current specification (v1.1.0). Without config-aware data parsing:

- **Temperature data is corrupted** (°F values interpreted as °C)
- **PM2.5 alerts are incorrect** (wrong field used for health thresholds)
- **CO2 forecasts are unreliable** (warmup period not tracked)
- **Multi-device support is broken** (heterogeneous configs not supported)

**Estimated Implementation Effort:**
- Critical features (FR-8.1, 8.2, 8.3): **4 days**
- High priority features (FR-8.4, 8.6): **4 days**
- Medium priority features (FR-8.5, 8.7, 8.8): **5 days**
- **Total:** 13 days (2.6 weeks) - fits within Phase 2 timeline

**Recommendation:** Prioritize **FR-8.1 (config retrieval), FR-8.2 (temperature conversion), and FR-8.3 (PM field selection)** as **blockers for Phase 1 completion**. Without these, the platform produces incorrect data.

---

## Appendix A: Full Config Schema

```rust
/// Complete AirGradient device configuration schema
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AirGradientConfig {
    /// Device country/region ("US", "EU", etc.)
    pub country: String,

    /// PM display standard ("ugm3" or "usaqi")
    pub pm_standard: String,

    /// LED bar display mode ("co2", "pm", "off")
    pub led_bar_mode: String,

    /// CO2 ABC calibration period (days)
    pub abc_days: u8,

    /// VOC sensor learning offset (hours)
    pub tvoc_learning_offset: u8,

    /// NOx sensor learning offset (hours)
    pub nox_learning_offset: u8,

    /// Custom MQTT broker URL (empty = AirGradient cloud)
    #[serde(default)]
    pub mqtt_broker_url: String,

    /// Custom HTTP API domain
    #[serde(default)]
    pub http_domain: String,

    /// Temperature unit ("c" or "f")
    pub temperature_unit: String,

    /// Disable cloud connection (local-only mode)
    #[serde(default)]
    pub disable_cloud_connection: bool,

    /// Configuration control ("both", "local", "cloud")
    pub configuration_control: String,

    /// Upload data to AirGradient cloud
    pub post_data_to_air_gradient: bool,

    /// LED bar brightness (0-100%)
    pub led_bar_brightness: u8,

    /// Display brightness (0-100%)
    pub display_brightness: u8,

    /// Offline mode (no external connectivity)
    pub offline_mode: bool,

    /// Display shows compensated values
    pub monitor_display_compensated_values: bool,

    /// Hardware model ("I-9PSL", "I-9PSL-DE", etc.)
    pub model: String,

    /// PM correction algorithms
    pub corrections: PmCorrections,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PmCorrections {
    pub pm02: PmCorrectionConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PmCorrectionConfig {
    /// Correction algorithm ("none", "epa_2021", "lrapa")
    pub correction_algorithm: String,

    /// Simple Linear Regression coefficient (optional)
    pub slr: Option<f64>,
}
```

---

## Appendix B: Reference Config Examples

### B.1 US Device (Fahrenheit, EPA 2021)

```json
{
  "country": "US",
  "pmStandard": "ugm3",
  "temperatureUnit": "f",
  "abcDays": 8,
  "tvocLearningOffset": 12,
  "noxLearningOffset": 12,
  "corrections": {
    "pm02": {
      "correctionAlgorithm": "epa_2021",
      "slr": null
    }
  },
  "offlineMode": false,
  "model": "I-9PSL"
}
```

**Impact:**
- Temperature: 75°F → 23.89°C conversion required
- PM2.5: Use `pm02Compensated` for alerts (EPA corrected)
- CO2: ABC every 8 days, warmup period tracking needed

---

### B.2 EU Device (Celsius, No Correction)

```json
{
  "country": "DE",
  "pmStandard": "ugm3",
  "temperatureUnit": "c",
  "abcDays": 8,
  "tvocLearningOffset": 12,
  "noxLearningOffset": 12,
  "corrections": {
    "pm02": {
      "correctionAlgorithm": "none",
      "slr": null
    }
  },
  "offlineMode": false,
  "model": "I-9PSL-DE"
}
```

**Impact:**
- Temperature: No conversion (already Celsius)
- PM2.5: Use `pm02` raw for alerts (no correction)
- CO2: Same ABC tracking as US device

---

### B.3 Offline Device (Local API Only)

```json
{
  "country": "US",
  "pmStandard": "ugm3",
  "temperatureUnit": "f",
  "abcDays": 0,
  "tvocLearningOffset": 12,
  "noxLearningOffset": 12,
  "corrections": {
    "pm02": {
      "correctionAlgorithm": "epa_2021",
      "slr": null
    }
  },
  "offlineMode": true,
  "disableCloudConnection": true,
  "mqttBrokerUrl": "",
  "model": "I-9PSL"
}
```

**Impact:**
- Data source: Local API only (MQTT unavailable)
- CO2: ABC disabled (`abcDays: 0`) - manual calibration required
- Privacy: No cloud uploads

---

**End of Gap Analysis**
