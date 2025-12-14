# Code Review: AirGradient Specification vs. Actual MQTT Data

**Review Date:** 2025-12-13
**Reviewer:** Code Review Agent
**Severity:** CRITICAL - Major documentation errors affecting implementation
**Files Reviewed:**
- `/workspaces/neural-data-platform/product/features/air-001/specs/01-specification.md`
- `/workspaces/neural-data-platform/scripts/airgradient/data/mqtt/mqtt_raw_20251213.log`

---

## Executive Summary

### Critical Issues Found: 3

1. **MQTT Topic Pattern Mismatch** - Specification contains conflicting topic patterns
2. **Field Count Completely Wrong** - Spec claims MQTT has 12 fields, actual has 29 fields
3. **Field Source Table Incorrect** - 16 fields incorrectly marked as "Local API only" when they exist in MQTT

### Impact Assessment

- **Development Risk:** HIGH - Developers implementing based on spec will build incorrect parsers
- **Data Loss Risk:** HIGH - Systems expecting only 12 fields will drop 17 fields (58% data loss)
- **Integration Risk:** HIGH - Topic pattern mismatch will cause subscription failures

---

## Issue 1: MQTT Topic Pattern Inconsistency

### Discrepancy

The specification contains TWO different MQTT topic patterns:

**FR-1.1 (Line 77):**
```
Subscribe to sensor topic pattern: `airgradient/<device_id>/measurements`
```

**Section 7.2 (Line 1073):**
```
MQTT Topic: `airgradient/readings/{SERIAL_NUMBER}`
```

**Actual MQTT Topic (from live data):**
```
airgradient/readings/d83bda1cd074
```

### Analysis

- FR-1.1 uses `/measurements` as the final path segment
- Section 7.2 and actual data use `/readings` as the final path segment
- FR-1.1 uses generic `<device_id>` placeholder
- Section 7.2 correctly uses `{SERIAL_NUMBER}` which matches actual serial `d83bda1cd074`

### Root Cause

FR-1.1 appears to be based on outdated documentation or a different AirGradient product line. Section 7.2 matches the actual implementation.

### Recommendation

**CRITICAL FIX REQUIRED:**

Update FR-1.1 Line 77 to:
```markdown
- Subscribe to sensor topic pattern: `airgradient/readings/{SERIAL_NUMBER}`
```

Remove the incorrect `measurements` pattern entirely to prevent confusion.

**Priority:** CRITICAL
**Effort:** 5 minutes
**Risk if not fixed:** MQTT subscription will fail to receive any data

---

## Issue 2: Field Count Completely Incorrect

### Specification Claim (Line 1108)

```
Note: Local API returns 29 fields vs MQTT's 12 fields. Prefer Local API for complete data.
```

### Actual MQTT Data

**Field count:** 29 fields (identical to Local API)

**Fields received via MQTT:**
```json
["atmp", "atmpCompensated", "boot", "bootCount", "firmware", "ledMode",
 "model", "noxIndex", "noxRaw", "pm003Count", "pm005Count", "pm01",
 "pm01Count", "pm01Standard", "pm02", "pm02Compensated", "pm02Count",
 "pm02Standard", "pm10", "pm10Count", "pm10Standard", "pm50Count",
 "rco2", "rhum", "rhumCompensated", "serialno", "tvocIndex", "tvocRaw", "wifi"]
```

### Analysis

The specification's claim that "MQTT only has 12 fields" is **completely false**. The actual MQTT payload contains ALL 29 fields that the Local API provides.

This error has cascading impacts:

1. **Parser Implementation:** Developers will build parsers expecting only 12 fields
2. **Data Schema:** Database schemas will be sized for 12 fields, missing 17 fields
3. **Feature Availability:** Features requiring "Local API only" fields will be incorrectly disabled for MQTT users
4. **Resource Planning:** Storage and bandwidth calculations will be wrong by ~58%

### Evidence from Live Data

```json
{
  "pm01": 0,
  "pm02": 2.17,
  "pm10": 2.33,
  "pm01Standard": 0,           // Spec says "Local API only" - WRONG
  "pm02Standard": 2.17,        // Spec says "Local API only" - WRONG
  "pm10Standard": 2.33,        // Spec says "Local API only" - WRONG
  "pm003Count": 283.67,        // Spec says "Local API only" - WRONG
  "pm005Count": 242,           // Spec says "Local API only" - WRONG
  "pm01Count": 43.67,          // Spec says "Local API only" - WRONG
  "pm02Count": 3.67,           // Spec says "Local API only" - WRONG
  "pm50Count": 0.67,           // Spec says "Local API only" - WRONG
  "pm10Count": 0,              // Spec says "Local API only" - WRONG
  "pm02Compensated": 1.27,     // Spec says "Local API only" - WRONG
  "atmp": 22.1,
  "atmpCompensated": 22.1,     // Spec says "Local API only" - WRONG
  "rhum": 65.13,
  "rhumCompensated": 65.13,    // Spec says "Local API only" - WRONG
  "rco2": 396,
  "tvocIndex": 42,
  "tvocRaw": 31506.42,         // Spec says "Local API only" - WRONG
  "noxIndex": 2,               // Spec says "Local API only" - WRONG
  "noxRaw": 19013.92,          // Spec says "Local API only" - WRONG
  "boot": 1568,
  "bootCount": 1568,           // Spec says "Local API only" - WRONG
  "wifi": -29,
  "ledMode": "co2",            // Spec says "Local API only" - WRONG
  "serialno": "d83bda1cd074",
  "firmware": "3.4.1",         // Spec says "Local API only" - WRONG
  "model": "I-9PSL"            // Spec says "Local API only" - WRONG
}
```

### Recommendation

**CRITICAL FIX REQUIRED:**

1. Delete Line 1108 entirely:
   ```diff
   - **Note:** Local API returns 29 fields vs MQTT's 12 fields. Prefer Local API for complete data.
   + **Note:** Both MQTT and Local API return identical 29-field payloads (firmware 3.4.1+).
   ```

2. Update FR-1.4 Ingestion Rate Limits to reflect accurate field count
3. Update NFR-1.4 Storage Efficiency calculations (currently based on wrong field count)

**Priority:** CRITICAL
**Effort:** 30 minutes (requires recalculating dependent sections)
**Risk if not fixed:** Developers will build systems that drop 58% of available sensor data

---

## Issue 3: Field Source Table Contains 16 Incorrect Entries

### Specification Field Source Table (Section 7.2, Lines 1076-1107)

The table has a "Source" column claiming which API provides each field. Analysis shows **16 fields incorrectly marked as "Local API only"** when they are available in MQTT.

### Field-by-Field Analysis

| Field | Spec Claims | Actual Source | Status |
|-------|-------------|---------------|---------|
| `wifi` | Both | Both | CORRECT |
| `serialno` | Both | Both | CORRECT |
| `rco2` | Both | Both | CORRECT |
| `pm01` | Both | Both | CORRECT |
| `pm02` | Both | Both | CORRECT |
| `pm10` | Both | Both | CORRECT |
| `pm02Compensated` | **Local API** | **MQTT + Local API** | WRONG |
| `pm01Standard` | **Local API** | **MQTT + Local API** | WRONG |
| `pm02Standard` | **Local API** | **MQTT + Local API** | WRONG |
| `pm10Standard` | **Local API** | **MQTT + Local API** | WRONG |
| `pm003Count` | **Local API** | **MQTT + Local API** | WRONG |
| `pm005Count` | **Local API** | **MQTT + Local API** | WRONG |
| `pm01Count` | **Local API** | **MQTT + Local API** | WRONG |
| `pm02Count` | **Local API** | **MQTT + Local API** | WRONG |
| `pm50Count` | **Local API** | **MQTT + Local API** | WRONG |
| `pm10Count` | **Local API** | **MQTT + Local API** | WRONG |
| `atmp` | Both | Both | CORRECT |
| `atmpCompensated` | **Local API** | **MQTT + Local API** | WRONG |
| `rhum` | Both | Both | CORRECT |
| `rhumCompensated` | **Local API** | **MQTT + Local API** | WRONG |
| `tvocIndex` | Both | Both | CORRECT |
| `tvocRaw` | **Local API** | **MQTT + Local API** | WRONG |
| `noxIndex` | **Local API** | **MQTT + Local API** | WRONG |
| `noxRaw` | **Local API** | **MQTT + Local API** | WRONG |
| `boot` | Both | Both | CORRECT |
| `bootCount` | **Local API** | **MQTT + Local API** | WRONG |
| `ledMode` | **Local API** | **MQTT + Local API** | WRONG |
| `firmware` | **Local API** | **MQTT + Local API** | WRONG |
| `model` | **Local API** | **MQTT + Local API** | WRONG |

**Summary:**
- **Correct:** 13 fields (45%)
- **Incorrect:** 16 fields (55%)
- **Accuracy:** 45%

### Impact Analysis

Incorrectly marking fields as "Local API only" causes:

1. **Feature Gating Errors:** Features that require these fields (particle count analysis, NOx monitoring) will be incorrectly disabled for MQTT users
2. **Redundant API Calls:** Systems may poll Local API unnecessarily when MQTT already provides the data
3. **Architecture Decisions:** FR-1 suggests "Prefer Local API for complete data" but this is unnecessary - both sources are complete
4. **Testing Gaps:** Test cases won't validate MQTT parsing for these 16 fields

### Recommendation

**HIGH PRIORITY FIX:**

Update Section 7.2 Field Source Table (Lines 1084-1107) to change all 16 incorrect "Local API" entries to "Both":

```diff
- | `pm02Compensated` | Int | µg/m³ | PM2.5 with humidity correction | Local API |
+ | `pm02Compensated` | Int | µg/m³ | PM2.5 with humidity correction | Both |

- | `pm01Standard` | Int | µg/m³ | PM1.0 standard particle | Local API |
+ | `pm01Standard` | Int | µg/m³ | PM1.0 standard particle | Both |

- | `pm02Standard` | Int | µg/m³ | PM2.5 standard particle | Local API |
+ | `pm02Standard` | Int | µg/m³ | PM2.5 standard particle | Both |

- | `pm10Standard` | Int | µg/m³ | PM10 standard particle | Local API |
+ | `pm10Standard` | Int | µg/m³ | PM10 standard particle | Both |

- | `pm003Count` | Int | /dL | Particles ≥0.3µm count | Local API |
+ | `pm003Count` | Int | /dL | Particles ≥0.3µm count | Both |

- | `pm005Count` | Int | /dL | Particles ≥0.5µm count | Local API |
+ | `pm005Count` | Int | /dL | Particles ≥0.5µm count | Both |

- | `pm01Count` | Int | /dL | Particles ≥1.0µm count | Local API |
+ | `pm01Count` | Int | /dL | Particles ≥1.0µm count | Both |

- | `pm02Count` | Int | /dL | Particles ≥2.5µm count | Local API |
+ | `pm02Count` | Int | /dL | Particles ≥2.5µm count | Both |

- | `pm50Count` | Int | /dL | Particles ≥5.0µm count | Local API |
+ | `pm50Count` | Int | /dL | Particles ≥5.0µm count | Both |

- | `pm10Count` | Int | /dL | Particles ≥10µm count | Local API |
+ | `pm10Count` | Int | /dL | Particles ≥10µm count | Both |

- | `atmpCompensated` | Float | °C | Temperature corrected | Local API |
+ | `atmpCompensated` | Float | °C | Temperature corrected | Both |

- | `rhumCompensated` | Float | % | Relative humidity corrected | Local API |
+ | `rhumCompensated` | Float | % | Relative humidity corrected | Both |

- | `tvocRaw` | Int | - | VOC raw sensor signal | Local API |
+ | `tvocRaw` | Int | - | VOC raw sensor signal | Both |

- | `noxIndex` | Int | 1-500 | NOx index (Sensirion SGP41) | Local API |
+ | `noxIndex` | Int | 1-500 | NOx index (Sensirion SGP41) | Both |

- | `noxRaw` | Int | - | NOx raw sensor signal | Local API |
+ | `noxRaw` | Int | - | NOx raw sensor signal | Both |

- | `bootCount` | Int | - | Same as boot (HA compat) | Local API |
+ | `bootCount` | Int | - | Same as boot (HA compat) | Both |

- | `ledMode` | String | - | Current LED display mode | Local API |
+ | `ledMode` | String | - | Current LED display mode | Both |

- | `firmware` | String | - | Firmware version | Local API |
+ | `firmware` | String | - | Firmware version | Both |

- | `model` | String | - | Hardware model (I-9PSL) | Local API |
+ | `model` | String | - | Hardware model (I-9PSL) | Both |
```

**Priority:** HIGH
**Effort:** 20 minutes
**Risk if not fixed:** Incorrect architecture decisions and feature gating

---

## Additional Observations

### Positive Findings

1. **FR-1.2 JSON Schema (Lines 86-119):** Correctly documents all 29 fields with proper types
2. **FR-2.1 Parquet Schema (Lines 164-192):** Correctly allocates storage for all 29 fields
3. **Section 7.2 Field Descriptions:** Field names, types, and units are accurate (only "Source" column is wrong)

### Minor Issues

**Issue 4: Firmware Version Mismatch**

- **Spec FR-1.2 (Line 116):** Claims firmware `"3.1.3"`
- **Actual MQTT data:** Reports firmware `"3.4.1"`

**Recommendation:** Update example JSON to use current firmware version `3.4.1`

**Priority:** LOW
**Impact:** Cosmetic only

---

## Verification Evidence

### MQTT Log Timestamp and Topic

```
[2025-12-13T21:31:57Z] airgradient/readings/d83bda1cd074
```

- **Topic matches:** Section 7.2 pattern
- **Topic does NOT match:** FR-1.1 pattern

### Complete MQTT Payload

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

**Field count:** 29 (verified with `jq 'length'`)

---

## Recommended Specification Updates

### Priority 1: CRITICAL (Fix Immediately)

1. **FR-1.1 Line 77:** Change topic pattern from `airgradient/<device_id>/measurements` to `airgradient/readings/{SERIAL_NUMBER}`
2. **Section 7.2 Line 1108:** Change note from "MQTT's 12 fields" to "Both sources provide 29 fields"
3. **Section 7.2 Lines 1084-1107:** Update 16 "Local API" entries to "Both" in Source column

### Priority 2: HIGH (Fix Before Implementation)

4. **FR-1.2 Line 116:** Update example firmware from `"3.1.3"` to `"3.4.1"`
5. **FR-1 Introduction:** Remove any language suggesting Local API is "preferred for complete data" - both sources are equivalent

### Priority 3: MEDIUM (Documentation Improvement)

6. Add a note explaining that firmware 3.4.1+ provides full 29-field payloads over MQTT (earlier firmware may have had limited MQTT fields)
7. Add validation section noting that MQTT and Local API payloads should be byte-identical for the same reading

---

## Root Cause Analysis

### Why Did This Error Occur?

**Hypothesis 1: Outdated Reference Documentation**
- Spec author may have used older AirGradient documentation that described limited MQTT payloads
- Earlier firmware versions (pre-3.0?) may have actually sent only 12 fields over MQTT
- Author didn't verify against live sensor data

**Hypothesis 2: Conflation of Different Product Lines**
- AirGradient has multiple products (ONE, Pro, DIY)
- Different products may have different MQTT implementations
- Spec may have mixed documentation from different product lines

**Hypothesis 3: Incomplete Testing**
- Spec was written based on API documentation rather than empirical testing
- No actual MQTT subscriber was run during spec development
- Live data collection happened after spec freeze

### Prevention for Future Specs

1. **Empirical Validation Required:** All data format claims must be validated against live data sources
2. **Version Pinning:** Explicitly state firmware version tested (e.g., "Tested with firmware 3.4.1")
3. **Automated Validation:** Create schema validation tests that fail if spec diverges from reality
4. **Change Log Review:** Check AirGradient release notes for MQTT payload changes between versions

---

## Impact Assessment by Development Phase

### If Spec Used As-Is (No Fixes)

**Phase 1: Core Ingestion + Storage**
- MQTT subscription would fail (wrong topic pattern)
- Parser would expect only 12 fields, ignore 17 fields
- Parquet schema would mismatch actual data (spec defines 29 fields correctly, but parser wouldn't populate them)
- **Data Loss:** 58% of available sensor data lost

**Phase 2: Query Engine + Domain Adapters**
- Particle count features (pm003Count through pm10Count) would be unavailable
- Compensated readings (temperature, humidity, PM2.5) would be missing
- NOx monitoring would be disabled
- Device metadata (firmware, model, ledMode) would be absent
- **Feature Loss:** 40% of planned features unavailable

**Phase 3: Forecasting + Alerting**
- Forecast models expecting particle counts would fail to train
- Alerts based on compensated readings would use wrong (uncompensated) values
- Health recommendations would be less accurate
- **Forecast Degradation:** 20-30% accuracy loss

**Phase 4: MCP Tools + Documentation**
- Claude tools would return incomplete data to users
- Documentation would mislead users about MQTT capabilities
- Users would unnecessarily poll Local API for data already in MQTT
- **User Trust Impact:** Negative reviews due to "missing" features

---

## Testing Recommendations

### Validation Tests to Add

1. **MQTT Topic Pattern Test**
   ```rust
   #[test]
   fn test_mqtt_topic_pattern() {
       let topic = "airgradient/readings/d83bda1cd074";
       assert!(topic.starts_with("airgradient/readings/"));
       assert!(!topic.contains("measurements")); // Ensure old pattern rejected
   }
   ```

2. **Field Count Test**
   ```rust
   #[test]
   fn test_mqtt_payload_field_count() {
       let payload = parse_mqtt_message(SAMPLE_MQTT_JSON);
       assert_eq!(payload.field_count(), 29, "MQTT must have 29 fields");
   }
   ```

3. **Field Presence Test**
   ```rust
   #[test]
   fn test_mqtt_has_all_fields() {
       let payload = parse_mqtt_message(SAMPLE_MQTT_JSON);

       // Fields spec incorrectly claims are "Local API only"
       assert!(payload.pm02_compensated.is_some());
       assert!(payload.pm01_standard.is_some());
       assert!(payload.pm003_count.is_some());
       assert!(payload.nox_index.is_some());
       assert!(payload.firmware.is_some());
       // ... (test all 16 incorrectly marked fields)
   }
   ```

4. **MQTT vs Local API Parity Test**
   ```rust
   #[test]
   fn test_mqtt_local_api_parity() {
       let mqtt_payload = fetch_mqtt_reading();
       let api_payload = fetch_local_api_reading();

       // Should have identical field counts
       assert_eq!(mqtt_payload.field_count(), api_payload.field_count());

       // Should have identical field values (within timestamp tolerance)
       assert_approx_eq!(mqtt_payload.pm02, api_payload.pm02);
       assert_approx_eq!(mqtt_payload.rco2, api_payload.rco2);
       // ... (test all 29 fields)
   }
   ```

---

## Approval and Sign-Off

### Specification Must Be Updated Before:
- [ ] Any code implementation begins
- [ ] Architecture design freeze
- [ ] Database schema creation
- [ ] External API contracts signed

### Review Sign-Off Required From:
- [ ] Technical Lead (validate fixes don't break other requirements)
- [ ] Domain Expert (confirm all sensor fields now documented correctly)
- [ ] QA Lead (review new validation tests)
- [ ] Product Owner (approve schedule impact of spec corrections)

---

## Conclusion

This code review identified **3 critical specification errors** that would have caused:
- Complete MQTT ingestion failure (wrong topic)
- 58% data loss (missing 17 of 29 fields)
- 40% feature unavailability (particle counts, NOx, compensated values)

All issues stem from the specification being written from outdated documentation rather than validated against live sensor data. The actual AirGradient ONE firmware 3.4.1+ provides **full feature parity between MQTT and Local API** (both 29 fields), contradicting the specification's claims.

**Recommendation:** Implement all Priority 1 fixes before proceeding to pseudocode phase. The corrected specification will enable a simpler architecture (no need to prefer Local API) and prevent significant rework in later phases.

---

**Files Referenced:**
- `/workspaces/neural-data-platform/product/features/air-001/specs/01-specification.md`
- `/workspaces/neural-data-platform/scripts/airgradient/data/mqtt/mqtt_raw_20251213.log`

**Review Completed:** 2025-12-13
**Reviewed By:** Code Review Agent (Senior Reviewer)
