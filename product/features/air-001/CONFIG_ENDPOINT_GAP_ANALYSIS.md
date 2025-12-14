# Config Endpoint Gap Analysis

**Date:** 2025-12-13
**Status:** SPECIFICATION GAP - Config endpoint not documented

---

## Executive Summary

The specification documents focus entirely on the `/measures/current` endpoint but **completely omit** the `/config` endpoint which provides critical sensor configuration data needed for proper data interpretation.

---

## Actual Config Endpoint Response

**Endpoint:** `http://airgradient_{SERIAL}.local/config`

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

---

## Critical Config Fields for Platform

| Field | Value | Impact | Priority |
|-------|-------|--------|----------|
| `pmStandard` | `"ugm3"` | **Defines PM2.5 units** - critical for interpretation | CRITICAL |
| `temperatureUnit` | `"f"` | **Temperature in Fahrenheit** - affects `atmp` conversion | CRITICAL |
| `corrections.pm02.correctionAlgorithm` | `"epa_2021"` | **PM2.5 compensation algorithm** in use | HIGH |
| `abcDays` | `8` | CO2 ABC calibration period - affects quality scoring | MEDIUM |
| `tvocLearningOffset` | `12` | VOC sensor learning period (hours) | MEDIUM |
| `noxLearningOffset` | `12` | NOx sensor learning period (hours) | MEDIUM |
| `offlineMode` | `false` | Sensor network state | LOW |

---

## Impact of Missing Spec

### 1. **pmStandard: "ugm3"**
- Confirms PM values are in µg/m³ (not AQI or other unit)
- Platform must know this to correctly interpret raw values
- Without this, platform might misconfigure health thresholds

### 2. **temperatureUnit: "f"**
- **CRITICAL**: The `atmp` field contains Fahrenheit, NOT Celsius!
- Spec assumes all temperatures are Celsius (FR-2.1 says "atmp: Float32 (°C)")
- If platform stores atmp=22.04 as Celsius, it's actually 22.04°F = -5.5°C!
- **Action Required:** Verify if `atmp` is raw value or converted based on config

### 3. **corrections.pm02.correctionAlgorithm: "epa_2021"**
- Sensor is already applying EPA 2021 PM2.5 compensation
- `pm02Compensated` uses this algorithm
- Platform should NOT re-apply compensation
- Need to document which corrections are pre-applied

### 4. **abcDays: 8**
- CO2 sensor ABC (Automatic Baseline Calibration) runs every 8 days
- FR-1.3 mentions "CO2 sensor warmup period (<3 weeks) = 0.7x penalty"
- Should align quality scoring with actual ABC period

### 5. **VOC/NOx Learning Offsets**
- `tvocLearningOffset: 12` and `noxLearningOffset: 12` (hours)
- Affects sensor accuracy during first 12 hours
- Should factor into quality scoring

---

## Recommended Specification Additions

### New FR-1.5: Config Endpoint Support

```markdown
**FR-1.5: Configuration Retrieval**
- **Description:** Retrieve sensor configuration to properly interpret readings
- **Acceptance Criteria:**
  - Fetch config from `http://airgradient_{SERIAL}.local/config` on startup
  - Cache config for 1 hour (config rarely changes)
  - Extract critical fields: pmStandard, temperatureUnit, corrections
  - Validate config version compatibility
  - Log warning if unexpected config values detected
- **Priority:** HIGH
- **Dependencies:** None
```

### New FR-8.7: Config-Based Data Interpretation

```markdown
**FR-8.7: Dynamic Unit Handling**
- **Description:** Interpret sensor readings based on sensor config
- **Acceptance Criteria:**
  - If `temperatureUnit: "f"`, convert atmp to Celsius before storage
  - If `pmStandard` != "ugm3", apply appropriate conversion
  - Record original unit in metadata for audit trail
  - Store all values in SI units (Celsius, µg/m³)
- **Priority:** HIGH
```

### Update FR-1.3: Quality Scoring

```markdown
**FR-1.3: Data Quality Assessment (REVISED)**
- Add to Calibration status section:
  - VOC sensor learning period: First `tvocLearningOffset` hours = 0.7x penalty
  - NOx sensor learning period: First `noxLearningOffset` hours = 0.7x penalty
  - CO2 ABC period: Quality increases after `abcDays` days
```

---

## Config Field Schema

| Field | Type | Description | Storage Impact |
|-------|------|-------------|----------------|
| `country` | String | ISO country code | Regulatory threshold selection |
| `pmStandard` | String | PM unit standard ("ugm3") | Unit conversion |
| `ledBarMode` | String | LED display mode | Informational |
| `abcDays` | Integer | CO2 ABC calibration period | Quality scoring |
| `tvocLearningOffset` | Integer | VOC learning hours | Quality scoring |
| `noxLearningOffset` | Integer | NOx learning hours | Quality scoring |
| `mqttBrokerUrl` | String | Custom MQTT broker | MQTT configuration |
| `httpDomain` | String | Custom HTTP endpoint | Not needed |
| `temperatureUnit` | String | "c" or "f" | **Unit conversion** |
| `disableCloudConnection` | Boolean | Cloud connectivity | Informational |
| `configurationControl` | String | Config source | Not needed |
| `postDataToAirGradient` | Boolean | Cloud reporting | Not needed |
| `ledBarBrightness` | Integer | LED brightness | Not needed |
| `displayBrightness` | Integer | Display brightness | Not needed |
| `offlineMode` | Boolean | Offline operation | Connectivity status |
| `monitorDisplayCompensatedValues` | Boolean | Display type | Informational |
| `model` | String | Device model variant | Device identification |
| `corrections.pm02.correctionAlgorithm` | String | PM2.5 correction | **Data interpretation** |
| `corrections.pm02.slr` | Float/null | SLR correction factor | Data interpretation |

---

## Verification Questions

Before finalizing spec, verify with actual sensor:

1. **Temperature Unit**: Is `atmp: 22.04` in the measures response already converted based on config, or is it always raw Celsius?
   - Test by changing `temperatureUnit` and observing `atmp` behavior

2. **PM Standard**: Is `pm02: 0.33` always in µg/m³ regardless of config?
   - Likely yes, but should verify

3. **Compensated Values**: Does `pm02Compensated` use the algorithm specified in config?
   - Should document which corrections are pre-applied

---

## Impact Assessment

### Without Config Support

| Issue | Impact | Risk |
|-------|--------|------|
| Unknown temperature unit | ±30°C interpretation error if F→C mismatch | HIGH |
| Unknown correction algorithm | Double-compensation or no-compensation | MEDIUM |
| Missing quality scoring data | Inaccurate quality flags | LOW |

### With Config Support

| Benefit | Value |
|---------|-------|
| Accurate unit conversion | 100% data interpretation accuracy |
| Correct compensation handling | No double-correction errors |
| Better quality scoring | More meaningful quality flags |
| Device identification | Better multi-sensor support |

---

## Recommended Actions

### Immediate (v1.0)

1. Add FR-1.5: Config endpoint support
2. Add FR-8.7: Dynamic unit handling
3. Update FR-1.3: Quality scoring with VOC/NOx learning periods
4. Add config schema to Section 7.2 reference table

### Short-term (v1.1)

5. Add config caching mechanism
6. Add config change detection
7. Add config validation with warnings

### Long-term (v1.2+)

8. Support config push/updates
9. Multi-sensor config management
10. Config history tracking

---

## Conclusion

The specification **must** be updated to include config endpoint documentation before implementation. Failure to do so risks:

- Incorrect temperature storage (if sensor is in Fahrenheit mode)
- Missing quality scoring factors (VOC/NOx learning periods)
- Unknown compensation algorithm status

**Recommendation:** Add FR-1.5 and FR-8.7 to specification. Verify temperature unit behavior with live sensor testing.

---

**Report Generated:** 2025-12-13
**Files Referenced:**
- `/workspaces/neural-data-platform/scripts/airgradient/data/api/local_d83bda1cd074_config_20251213_150850.json`
- `/workspaces/neural-data-platform/product/features/air-001/specs/01-specification.md`
