# MQTT Field Comparison: Specification vs. Actual Data

**Analysis Date:** 2025-12-13
**Sensor:** AirGradient ONE (Serial: d83bda1cd074)
**Firmware:** 3.4.1
**MQTT Topic:** `airgradient/readings/d83bda1cd074`

---

## Field-by-Field Comparison

| # | Field Name | Type | Spec Source | Actual MQTT | Status | Sample Value |
|---|------------|------|-------------|-------------|--------|--------------|
| 1 | `wifi` | Int | Both | YES | CORRECT | -29 dBm |
| 2 | `serialno` | String | Both | YES | CORRECT | "d83bda1cd074" |
| 3 | `rco2` | Int | Both | YES | CORRECT | 396 ppm |
| 4 | `pm01` | Int | Both | YES | CORRECT | 0 µg/m³ |
| 5 | `pm02` | Int | Both | YES | CORRECT | 2.17 µg/m³ |
| 6 | `pm10` | Int | Both | YES | CORRECT | 2.33 µg/m³ |
| 7 | `pm02Compensated` | Int | **Local API** | **YES** | **WRONG** | 1.27 µg/m³ |
| 8 | `pm01Standard` | Int | **Local API** | **YES** | **WRONG** | 0 µg/m³ |
| 9 | `pm02Standard` | Int | **Local API** | **YES** | **WRONG** | 2.17 µg/m³ |
| 10 | `pm10Standard` | Int | **Local API** | **YES** | **WRONG** | 2.33 µg/m³ |
| 11 | `pm003Count` | Int | **Local API** | **YES** | **WRONG** | 283.67 /dL |
| 12 | `pm005Count` | Int | **Local API** | **YES** | **WRONG** | 242 /dL |
| 13 | `pm01Count` | Int | **Local API** | **YES** | **WRONG** | 43.67 /dL |
| 14 | `pm02Count` | Int | **Local API** | **YES** | **WRONG** | 3.67 /dL |
| 15 | `pm50Count` | Int | **Local API** | **YES** | **WRONG** | 0.67 /dL |
| 16 | `pm10Count` | Int | **Local API** | **YES** | **WRONG** | 0 /dL |
| 17 | `atmp` | Float | Both | YES | CORRECT | 22.1 °C |
| 18 | `atmpCompensated` | Float | **Local API** | **YES** | **WRONG** | 22.1 °C |
| 19 | `rhum` | Float | Both | YES | CORRECT | 65.13% |
| 20 | `rhumCompensated` | Float | **Local API** | **YES** | **WRONG** | 65.13% |
| 21 | `tvocIndex` | Int | Both | YES | CORRECT | 42 |
| 22 | `tvocRaw` | Int | **Local API** | **YES** | **WRONG** | 31506.42 |
| 23 | `noxIndex` | Int | **Local API** | **YES** | **WRONG** | 2 |
| 24 | `noxRaw` | Int | **Local API** | **YES** | **WRONG** | 19013.92 |
| 25 | `boot` | Int | Both | YES | CORRECT | 1568 |
| 26 | `bootCount` | Int | **Local API** | **YES** | **WRONG** | 1568 |
| 27 | `ledMode` | String | **Local API** | **YES** | **WRONG** | "co2" |
| 28 | `firmware` | String | **Local API** | **YES** | **WRONG** | "3.4.1" |
| 29 | `model` | String | **Local API** | **YES** | **WRONG** | "I-9PSL" |

---

## Statistics

| Metric | Count | Percentage |
|--------|-------|------------|
| **Total Fields** | 29 | 100% |
| **Spec Claims "Both"** | 13 | 45% |
| **Spec Claims "Local API Only"** | 16 | 55% |
| **Actually in MQTT** | 29 | 100% |
| **Specification Errors** | 16 | 55% |
| **Specification Accuracy** | 13 | 45% |

---

## Category Breakdown

### Particulate Matter (8 fields)

| Field | Spec | Actual | Status |
|-------|------|--------|--------|
| `pm01` | Both | MQTT | CORRECT |
| `pm02` | Both | MQTT | CORRECT |
| `pm10` | Both | MQTT | CORRECT |
| `pm02Compensated` | **Local API** | **MQTT** | **WRONG** |
| `pm01Standard` | **Local API** | **MQTT** | **WRONG** |
| `pm02Standard` | **Local API** | **MQTT** | **WRONG** |
| `pm10Standard` | **Local API** | **MQTT** | **WRONG** |

Accuracy: 3/7 = 43%

### Particle Counts (6 fields)

| Field | Spec | Actual | Status |
|-------|------|--------|--------|
| `pm003Count` | **Local API** | **MQTT** | **WRONG** |
| `pm005Count` | **Local API** | **MQTT** | **WRONG** |
| `pm01Count` | **Local API** | **MQTT** | **WRONG** |
| `pm02Count` | **Local API** | **MQTT** | **WRONG** |
| `pm50Count` | **Local API** | **MQTT** | **WRONG** |
| `pm10Count` | **Local API** | **MQTT** | **WRONG** |

Accuracy: 0/6 = 0% (ALL WRONG)

### Gases (5 fields)

| Field | Spec | Actual | Status |
|-------|------|--------|--------|
| `rco2` | Both | MQTT | CORRECT |
| `tvocIndex` | Both | MQTT | CORRECT |
| `tvocRaw` | **Local API** | **MQTT** | **WRONG** |
| `noxIndex` | **Local API** | **MQTT** | **WRONG** |
| `noxRaw` | **Local API** | **MQTT** | **WRONG** |

Accuracy: 2/5 = 40%

### Environmental (4 fields)

| Field | Spec | Actual | Status |
|-------|------|--------|--------|
| `atmp` | Both | MQTT | CORRECT |
| `atmpCompensated` | **Local API** | **MQTT** | **WRONG** |
| `rhum` | Both | MQTT | CORRECT |
| `rhumCompensated` | **Local API** | **MQTT** | **WRONG** |

Accuracy: 2/4 = 50%

### Device Metadata (6 fields)

| Field | Spec | Actual | Status |
|-------|------|--------|--------|
| `wifi` | Both | MQTT | CORRECT |
| `serialno` | Both | MQTT | CORRECT |
| `boot` | Both | MQTT | CORRECT |
| `bootCount` | **Local API** | **MQTT** | **WRONG** |
| `ledMode` | **Local API** | **MQTT** | **WRONG** |
| `firmware` | **Local API** | **MQTT** | **WRONG** |
| `model` | **Local API** | **MQTT** | **WRONG** |

Accuracy: 3/7 = 43%

---

## Impact by Feature

### Features That Would Break with Current Spec

1. **Particle Size Distribution Analysis**
   - Requires: `pm003Count` through `pm10Count` (6 fields)
   - Spec says: Local API only
   - Reality: Available in MQTT
   - Impact: Feature would be unnecessarily disabled for MQTT users

2. **Advanced Air Quality Index Calculation**
   - Requires: `pm02Compensated` (humidity-corrected PM2.5)
   - Spec says: Local API only
   - Reality: Available in MQTT
   - Impact: Less accurate AQI for MQTT users

3. **NOx Monitoring**
   - Requires: `noxIndex`, `noxRaw`
   - Spec says: Local API only
   - Reality: Available in MQTT
   - Impact: NO₂ pollution monitoring disabled for MQTT users

4. **Sensor Diagnostics**
   - Requires: `firmware`, `model`, `ledMode`
   - Spec says: Local API only
   - Reality: Available in MQTT
   - Impact: Cannot diagnose sensor issues via MQTT

5. **Climate Comfort Calculations**
   - Requires: `atmpCompensated`, `rhumCompensated`
   - Spec says: Local API only (temperature/humidity)
   - Reality: Available in MQTT
   - Impact: Inaccurate comfort metrics for MQTT users

---

## Actual MQTT Payload Example

```json
{
  "pm01": 0,
  "pm02": 2.17,
  "pm10": 2.33,
  "pm01Standard": 0,
  "pm02Standard": 2.17,
  "pm10Standard": 2.33,
  "pm003Count": 283.67,
  "pm005Count": 242,
  "pm01Count": 43.67,
  "pm02Count": 3.67,
  "pm50Count": 0.67,
  "pm10Count": 0,
  "pm02Compensated": 1.27,
  "atmp": 22.1,
  "atmpCompensated": 22.1,
  "rhum": 65.13,
  "rhumCompensated": 65.13,
  "rco2": 396,
  "tvocIndex": 42,
  "tvocRaw": 31506.42,
  "noxIndex": 2,
  "noxRaw": 19013.92,
  "boot": 1568,
  "bootCount": 1568,
  "wifi": -29,
  "ledMode": "co2",
  "serialno": "d83bda1cd074",
  "firmware": "3.4.1",
  "model": "I-9PSL"
}
```

**Timestamp:** 2025-12-13T21:31:57Z
**Topic:** `airgradient/readings/d83bda1cd074`
**Field Count:** 29 (NOT 12 as spec claims)

---

## Recommendations

1. **Update Specification Table:** Change all 16 "Local API" entries to "Both"
2. **Remove Field Count Note:** Delete claim that "MQTT has 12 fields"
3. **Update Architecture:** Remove preference for Local API (both sources equivalent)
4. **Add Firmware Note:** Document that full MQTT payload requires firmware 3.4.1+
5. **Create Validation Tests:** Ensure parsers handle all 29 MQTT fields

---

## Root Cause

The specification appears to be based on:
- Outdated AirGradient documentation (pre-firmware 3.4.1)
- OR: Different AirGradient product line (not ONE v9)
- OR: Incomplete testing (no actual MQTT subscription during spec writing)

**Evidence:** Current firmware 3.4.1 provides full feature parity between MQTT and Local API.

---

**Files:**
- Specification: `/workspaces/neural-data-platform/product/features/air-001/specs/01-specification.md`
- MQTT Data: `/workspaces/neural-data-platform/scripts/airgradient/data/mqtt/mqtt_raw_20251213.log`
- Full Review: `/workspaces/neural-data-platform/product/features/air-001/CODE_REVIEW_SPEC_VS_ACTUAL_MQTT.md`
