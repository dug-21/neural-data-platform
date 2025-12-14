# AirGradient API Discrepancy Report

**Date:** 2025-12-13
**Sensor Model:** I-9PSL
**Firmware Version:** 3.4.1 (Actual) vs 3.1.3 (Spec)
**Reviewer:** Code Review Agent
**Status:** CRITICAL - Multiple type mismatches requiring schema revision

---

## Executive Summary

Analysis of the actual AirGradient ONE sensor API response reveals **critical discrepancies** between the real-world data and the specification (FR-2.1). The primary issues are:

1. **FLOAT vs INTEGER types**: The actual API returns FLOAT values for fields specified as UInt16/UInt32
2. **Field naming conventions**: Actual API uses camelCase while Parquet schema uses snake_case
3. **Missing fields**: Spec is missing `ledMode` and `bootCount` fields
4. **Data precision**: Fractional values in PM counts and measurements contradict unsigned integer types

**Impact:** The current Parquet schema will cause data loss or ingestion failures when storing actual sensor data.

---

## Detailed Discrepancy Analysis

### 1. Data Type Mismatches

| Field (Actual API) | Actual Type | Actual Value Example | Spec Type | Issue Severity | Impact |
|-------------------|-------------|---------------------|-----------|----------------|--------|
| `pm01` | Float | 0 | UInt16 | MEDIUM | Works for integer values but schema doesn't support fractional |
| `pm02` | **Float** | **0.33** | UInt16 | **CRITICAL** | Fractional values will be truncated or rejected |
| `pm10` | **Float** | **0.5** | UInt16 | **CRITICAL** | Fractional values will be truncated or rejected |
| `pm01Standard` | Float | 0 | UInt16 | MEDIUM | Works for integer values |
| `pm02Standard` | **Float** | **0.33** | UInt16 | **CRITICAL** | Fractional values will be truncated |
| `pm10Standard` | **Float** | **0.5** | UInt16 | **CRITICAL** | Fractional values will be truncated |
| `pm02Compensated` | **Float** | **0.76** | UInt16 | **CRITICAL** | Fractional values will be truncated |
| `pm003Count` | **Float** | **271.33** | UInt32 | **CRITICAL** | Particle counts should be integers but API returns floats |
| `pm005Count` | **Float** | **221.67** | UInt32 | **CRITICAL** | Particle counts should be integers but API returns floats |
| `pm01Count` | Float | 31 | UInt32 | MEDIUM | Works for integer values |
| `pm02Count` | **Float** | **1.33** | UInt32 | **CRITICAL** | Fractional particle count is invalid |
| `pm50Count` | Float | 0 | UInt32 | MEDIUM | Works for integer values |
| `pm10Count` | Float | 0 | UInt32 | MEDIUM | Works for integer values |
| `atmp` | Float | 22.04 | Float32 | **OK** | Type matches |
| `atmpCompensated` | Float | 22.04 | Float32 | **OK** | Type matches |
| `rhum` | Float | 59.91 | Float32 | **OK** | Type matches |
| `rhumCompensated` | Float | 59.91 | Float32 | **OK** | Type matches |
| `rco2` | Integer | 391 | UInt16 | **OK** | Type matches |
| `tvocIndex` | Integer | 28 | UInt16 | **OK** | Type matches |
| `tvocRaw` | **Float** | **31619.5** | UInt32 | **CRITICAL** | Raw sensor values shouldn't have decimals |
| `noxIndex` | Integer | 2 | UInt16 | **OK** | Type matches |
| `noxRaw` | **Float** | **18924.08** | UInt32 | **CRITICAL** | Raw sensor values shouldn't have decimals |
| `boot` | Integer | 1485 | UInt32 | **OK** | Type matches |
| `bootCount` | Integer | 1485 | UInt32 | **OK** | Type matches |
| `wifi` | Integer | -28 | Int8 | **OK** | Type matches |

### 2. Field Naming Discrepancies

| Actual API Field | Spec Parquet Field | Status | Recommendation |
|-----------------|-------------------|--------|----------------|
| `pm01` | `pm01` | **MATCH** | No change |
| `pm02` | `pm02` | **MATCH** | No change |
| `pm10` | `pm10` | **MATCH** | No change |
| `pm02Compensated` | `pm02_compensated` | **MISMATCH** | Transform camelCase → snake_case during ingestion |
| `pm01Standard` | `pm01_standard` | **MISMATCH** | Transform camelCase → snake_case during ingestion |
| `pm02Standard` | `pm02_standard` | **MISMATCH** | Transform camelCase → snake_case during ingestion |
| `pm10Standard` | `pm10_standard` | **MISMATCH** | Transform camelCase → snake_case during ingestion |
| `pm003Count` | `pm003_count` | **MISMATCH** | Transform camelCase → snake_case during ingestion |
| `pm005Count` | `pm005_count` | **MISMATCH** | Transform camelCase → snake_case during ingestion |
| `pm01Count` | `pm01_count` | **MISMATCH** | Transform camelCase → snake_case during ingestion |
| `pm02Count` | `pm02_count` | **MISMATCH** | Transform camelCase → snake_case during ingestion |
| `pm50Count` | `pm50_count` | **MISMATCH** | Transform camelCase → snake_case during ingestion |
| `pm10Count` | `pm10_count` | **MISMATCH** | Transform camelCase → snake_case during ingestion |
| `atmpCompensated` | `atmp_compensated` | **MISMATCH** | Transform camelCase → snake_case during ingestion |
| `rhumCompensated` | `rhum_compensated` | **MISMATCH** | Transform camelCase → snake_case during ingestion |
| `tvocIndex` | `tvoc_index` | **MISMATCH** | Transform camelCase → snake_case during ingestion |
| `tvocRaw` | `tvoc_raw` | **MISMATCH** | Transform camelCase → snake_case during ingestion |
| `noxIndex` | `nox_index` | **MISMATCH** | Transform camelCase → snake_case during ingestion |
| `noxRaw` | `nox_raw` | **MISMATCH** | Transform camelCase → snake_case during ingestion |

### 3. Missing/Extra Fields

| Field | In Actual API | In Spec | Status | Priority |
|-------|---------------|---------|--------|----------|
| `ledMode` | YES | **NO** | **MISSING FROM SPEC** | MEDIUM - Useful for device status |
| `bootCount` | YES | **NO** | **MISSING FROM SPEC** | LOW - Duplicate of `boot` |
| `serialno` | YES | YES (as `location_id`) | OK | N/A |
| `firmware` | YES | YES | OK | N/A |
| `model` | YES | YES | OK | N/A |

### 4. Value Range Analysis

| Field | Actual Value | Spec Range | Validation Status | Notes |
|-------|-------------|-----------|-------------------|-------|
| `rco2` | 391 ppm | 400-10,000 ppm | **BELOW MINIMUM** | Actual value below spec minimum (400 ppm is outdoor baseline) |
| `pm02` | 0.33 µg/m³ | 0-500 µg/m³ | OK | Within range |
| `tvocIndex` | 28 | 1-500 | OK | Within range |
| `noxIndex` | 2 | 1-500 | OK | Within range |
| `atmp` | 22.04°C | -10 to 50°C | OK | Within range |
| `rhum` | 59.91% | 0-100% | OK | Within range |
| `wifi` | -28 dBm | N/A | OK | Good signal strength |

### 5. Firmware Version Impact

**Spec Firmware:** 3.1.3
**Actual Firmware:** 3.4.1

**Analysis:** The firmware version difference (3.1.3 → 3.4.1) may explain some data type changes. The newer firmware appears to:
- Return averaged/smoothed values as floats for PM measurements
- Provide fractional particle counts (possibly time-averaged)
- Return raw sensor values with decimals (internal ADC precision)

---

## Critical Issues Requiring Action

### Issue 1: Float Values in PM Fields (CRITICAL)

**Problem:** Particulate matter fields return floats but spec defines UInt16.

**Example:**
```json
Actual: "pm02": 0.33
Spec: pm02: UInt16
```

**Impact:**
- Parquet writer will fail or truncate values
- Data loss for fractional measurements
- Inaccurate storage of low concentration readings

**Recommendation:**
```diff
- pm01: UInt16, pm02: UInt16, pm10: UInt16
+ pm01: Float32, pm02: Float32, pm10: Float32

- pm02_compensated: UInt16
+ pm02_compensated: Float32

- pm01_standard: UInt16, pm02_standard: UInt16, pm10_standard: UInt16
+ pm01_standard: Float32, pm02_standard: Float32, pm10_standard: Float32
```

### Issue 2: Float Values in Particle Counts (CRITICAL)

**Problem:** Particle count fields return floats but spec defines UInt32.

**Example:**
```json
Actual: "pm003Count": 271.33, "pm02Count": 1.33
Spec: pm003_count: UInt32, pm02_count: UInt32
```

**Impact:**
- Fractional particle counts are physically meaningless
- Suggests API is returning time-averaged values
- Schema prevents storing actual API data

**Recommendation:**
```diff
- pm003_count: UInt32, pm005_count: UInt32, pm01_count: UInt32
- pm02_count: UInt32, pm50_count: UInt32, pm10_count: UInt32
+ pm003_count: Float32, pm005_count: Float32, pm01_count: Float32
+ pm02_count: Float32, pm50_count: Float32, pm10_count: Float32
```

**Alternative:** Store as UInt32 after rounding, but add metadata flag indicating data was averaged.

### Issue 3: Float Values in Raw Sensor Readings (CRITICAL)

**Problem:** Raw sensor values have decimals but spec defines UInt32.

**Example:**
```json
Actual: "tvocRaw": 31619.5, "noxRaw": 18924.08
Spec: tvoc_raw: UInt32, nox_raw: UInt32
```

**Impact:**
- Loss of sensor precision
- Cannot store actual raw ADC values

**Recommendation:**
```diff
- tvoc_raw: UInt32, nox_raw: UInt32
+ tvoc_raw: Float32, nox_raw: Float32
```

### Issue 4: Missing ledMode Field (MEDIUM)

**Problem:** Actual API includes `ledMode` field not in spec.

**Example:**
```json
Actual: "ledMode": "co2"
Spec: (not present)
```

**Impact:**
- Useful diagnostic information lost
- Cannot correlate LED display mode with data quality

**Recommendation:**
```diff
  # Device Metadata
- wifi: Int8 (dBm), boot: UInt32, firmware: Utf8, model: Utf8
+ wifi: Int8 (dBm), boot: UInt32, firmware: Utf8, model: Utf8, led_mode: Utf8
```

### Issue 5: CO2 Below Minimum Range (LOW)

**Problem:** Actual CO2 reading (391 ppm) below spec minimum (400 ppm).

**Analysis:**
- 400 ppm is typical outdoor baseline
- 391 ppm is physically valid (outdoor air can be 380-420 ppm)
- Spec range should include outdoor baseline

**Recommendation:**
```diff
- CO2 (rco2): 400-10,000 ppm (Senseair S8)
+ CO2 (rco2): 380-10,000 ppm (Senseair S8, outdoor baseline ~400 ppm)
```

---

## Recommended Actions

### Immediate (Required for v1.0)

1. **Update Parquet Schema (FR-2.1):**
   - Change all PM fields to Float32
   - Change all particle count fields to Float32
   - Change raw sensor fields to Float32
   - Add `led_mode: Utf8` field

2. **Update Validation Ranges (FR-1.2):**
   - Lower CO2 minimum to 380 ppm

3. **Implement Field Name Transformation:**
   - Add camelCase → snake_case converter in ingestion pipeline
   - Document mapping in `air_quality_adapter.rs`

### Short-term (v1.1)

4. **Add Data Quality Flags:**
   - Flag `is_averaged: bool` when API returns fractional counts
   - Flag `firmware_version: Utf8` for version-specific parsing

5. **Update Documentation:**
   - Document firmware version differences (3.1.x vs 3.4.x)
   - Update FR-7.2 field reference table with actual types

### Long-term (v1.2+)

6. **Schema Versioning:**
   - Implement schema version field for future compatibility
   - Support migration from v1 (UInt16) to v2 (Float32) schemas

7. **API Version Detection:**
   - Auto-detect firmware version from `firmware` field
   - Apply version-specific parsing logic

---

## Revised Parquet Schema (Proposed)

```
timestamp: Timestamp(Microsecond, UTC)
location_id: Utf8 (serialno)

# Particulate Matter (µg/m³) - CHANGED TO FLOAT32
pm01: Float32, pm02: Float32, pm10: Float32
pm02_compensated: Float32
pm01_standard: Float32, pm02_standard: Float32, pm10_standard: Float32

# Particle Counts (per dL) - CHANGED TO FLOAT32
pm003_count: Float32, pm005_count: Float32, pm01_count: Float32
pm02_count: Float32, pm50_count: Float32, pm10_count: Float32

# Gases
rco2: UInt16 (CO2 ppm)
tvoc_index: UInt16, tvoc_raw: Float32  # CHANGED tvoc_raw
nox_index: UInt16, nox_raw: Float32    # CHANGED nox_raw

# Environmental
atmp: Float32, atmp_compensated: Float32 (°C)
rhum: Float32, rhum_compensated: Float32 (%)

# Device Metadata
wifi: Int8 (dBm), boot: UInt32, firmware: Utf8, model: Utf8
led_mode: Utf8  # ADDED

# Quality
quality_score: Float32, quality_flags: List<Utf8>
```

---

## Testing Recommendations

### Unit Tests

1. **Type Conversion Tests:**
   - Verify Float → Float32 storage (no truncation)
   - Verify negative CO2 values rejected
   - Verify fractional PM values stored correctly

2. **Field Mapping Tests:**
   - Verify camelCase → snake_case transformation
   - Verify missing `ledMode` handled gracefully
   - Verify unknown fields logged and ignored

### Integration Tests

3. **Real Sensor Data Tests:**
   - Ingest 1000 real API responses from firmware 3.4.x
   - Verify no data loss (compare JSON → Parquet → JSON round-trip)
   - Verify query results match original values

4. **Firmware Version Tests:**
   - Test with firmware 3.1.3 responses (integer values)
   - Test with firmware 3.4.1 responses (float values)
   - Verify both versions store correctly

---

## Security & Data Integrity Notes

### Concerns

1. **Fractional Particle Counts:** Physically impossible - suggests firmware is averaging over time. Need to verify with AirGradient if this is intended behavior.

2. **Raw Sensor Decimals:** May indicate internal calibration or temperature compensation. Should validate against AirGradient documentation.

3. **Type Coercion Risks:** Automatic float → int conversion could hide data quality issues. Prefer explicit type checking and quality flags.

### Recommendations

- **Strict Validation:** Reject messages with impossible values (negative PM, particle counts > 10^6)
- **Quality Scoring:** Lower quality score for fractional counts (indicate averaged data)
- **Alerting:** Log warnings when API returns unexpected types
- **Documentation:** Contact AirGradient support to confirm float values are intended behavior in firmware 3.4.x

---

## Conclusion

The specification (FR-2.1) requires **major revision** to support actual AirGradient API data. The current schema assumes integer values for PM and particle counts, but firmware 3.4.1 returns floats. This will cause ingestion failures or data loss.

**Recommended Next Steps:**
1. Update specification FR-2.1 with revised schema (all PM/count fields → Float32)
2. Implement field name transformation (camelCase → snake_case)
3. Add `led_mode` field to schema
4. Update validation ranges (CO2 minimum 380 ppm)
5. Add firmware version to quality metadata
6. Test with 1000+ real sensor readings before production deployment

**Risk Assessment:**
- **Current Risk:** HIGH - Production deployment will fail to ingest fractional values
- **With Changes:** LOW - Schema will correctly store all actual API data

---

**Report Generated:** 2025-12-13
**Next Review:** After schema updates implemented
**Reviewers Required:**
- Technical Lead (schema changes)
- Domain Expert (validate PM float values)
- AirGradient Support (confirm API behavior)
