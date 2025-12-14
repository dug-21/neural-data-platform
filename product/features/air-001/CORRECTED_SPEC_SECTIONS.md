# Corrected Specification Sections - Ready to Apply

**Date:** 2025-12-13
**Purpose:** Drop-in replacements for incorrect specification sections

---

## Section 1: FR-1.1 MQTT Client Connection (CORRECTED)

**File:** `01-specification.md`
**Lines:** 73-82
**Status:** CRITICAL - Wrong topic pattern

### CURRENT (INCORRECT):

```markdown
**FR-1.1: MQTT Client Connection**
- **Description:** Establish persistent MQTT connection to AirGradient ONE sensor
- **Acceptance Criteria:**
  - Connect to MQTT broker (mosquitto or airgradient cloud) with TLS support
  - Subscribe to sensor topic pattern: `airgradient/<device_id>/measurements`
  - Auto-reconnect on connection loss with exponential backoff (1s, 2s, 4s, 8s, max 30s)
  - Log connection events (connect, disconnect, reconnect)
- **Priority:** HIGH
- **Dependencies:** rumqttc crate (Rust MQTT client)
```

### CORRECTED:

```markdown
**FR-1.1: MQTT Client Connection**
- **Description:** Establish persistent MQTT connection to AirGradient ONE sensor
- **Acceptance Criteria:**
  - Connect to MQTT broker (mosquitto or airgradient cloud) with TLS support
  - Subscribe to sensor topic pattern: `airgradient/readings/{SERIAL_NUMBER}`
  - Example: `airgradient/readings/d83bda1cd074` (where d83bda1cd074 is device serial)
  - Auto-reconnect on connection loss with exponential backoff (1s, 2s, 4s, 8s, max 30s)
  - Log connection events (connect, disconnect, reconnect)
- **Priority:** HIGH
- **Dependencies:** rumqttc crate (Rust MQTT client)
```

---

## Section 2: Section 7.2 Note (CORRECTED)

**File:** `01-specification.md`
**Line:** 1108
**Status:** CRITICAL - Wrong field count

### CURRENT (INCORRECT):

```markdown
**Note:** Local API returns 29 fields vs MQTT's 12 fields. Prefer Local API for complete data.
```

### CORRECTED:

```markdown
**Note:** Both MQTT and Local API return identical 29-field payloads (firmware 3.4.1+). Either data source provides complete sensor data. MQTT is preferred for real-time updates (push-based), while Local API is useful for polling fallback or historical backfill.
```

---

## Section 3: Section 7.2 Field Source Table (CORRECTED)

**File:** `01-specification.md`
**Lines:** 1076-1107
**Status:** HIGH - 16 incorrect "Source" values

### CURRENT (INCORRECT):

```markdown
| Field | Type | Unit | Description | Source |
|-------|------|------|-------------|--------|
| `wifi` | Int | dBm | WiFi signal strength | Both |
| `serialno` | String | - | Device serial number | Both |
| `rco2` | Int | ppm | CO₂ concentration (Senseair S8) | Both |
| `pm01` | Int | µg/m³ | PM1.0 atmospheric | Both |
| `pm02` | Int | µg/m³ | PM2.5 atmospheric | Both |
| `pm10` | Int | µg/m³ | PM10 atmospheric | Both |
| `pm02Compensated` | Int | µg/m³ | PM2.5 with humidity correction | Local API |
| `pm01Standard` | Int | µg/m³ | PM1.0 standard particle | Local API |
| `pm02Standard` | Int | µg/m³ | PM2.5 standard particle | Local API |
| `pm10Standard` | Int | µg/m³ | PM10 standard particle | Local API |
| `pm003Count` | Int | /dL | Particles ≥0.3µm count | Local API |
| `pm005Count` | Int | /dL | Particles ≥0.5µm count | Local API |
| `pm01Count` | Int | /dL | Particles ≥1.0µm count | Local API |
| `pm02Count` | Int | /dL | Particles ≥2.5µm count | Local API |
| `pm50Count` | Int | /dL | Particles ≥5.0µm count | Local API |
| `pm10Count` | Int | /dL | Particles ≥10µm count | Local API |
| `atmp` | Float | °C | Temperature raw | Both |
| `atmpCompensated` | Float | °C | Temperature corrected | Local API |
| `rhum` | Float | % | Relative humidity raw | Both |
| `rhumCompensated` | Float | % | Relative humidity corrected | Local API |
| `tvocIndex` | Int | 1-500 | VOC index (Sensirion SGP41) | Both |
| `tvocRaw` | Int | - | VOC raw sensor signal | Local API |
| `noxIndex` | Int | 1-500 | NOx index (Sensirion SGP41) | Local API |
| `noxRaw` | Int | - | NOx raw sensor signal | Local API |
| `boot` | Int | - | Measurement cycle counter | Both |
| `bootCount` | Int | - | Same as boot (HA compat) | Local API |
| `ledMode` | String | - | Current LED display mode | Local API |
| `firmware` | String | - | Firmware version | Local API |
| `model` | String | - | Hardware model (I-9PSL) | Local API |
```

### CORRECTED:

```markdown
| Field | Type | Unit | Description | Source |
|-------|------|------|-------------|--------|
| `wifi` | Int | dBm | WiFi signal strength | Both |
| `serialno` | String | - | Device serial number | Both |
| `rco2` | Int | ppm | CO₂ concentration (Senseair S8) | Both |
| `pm01` | Int | µg/m³ | PM1.0 atmospheric | Both |
| `pm02` | Int | µg/m³ | PM2.5 atmospheric | Both |
| `pm10` | Int | µg/m³ | PM10 atmospheric | Both |
| `pm02Compensated` | Int | µg/m³ | PM2.5 with humidity correction | Both |
| `pm01Standard` | Int | µg/m³ | PM1.0 standard particle | Both |
| `pm02Standard` | Int | µg/m³ | PM2.5 standard particle | Both |
| `pm10Standard` | Int | µg/m³ | PM10 standard particle | Both |
| `pm003Count` | Int | /dL | Particles ≥0.3µm count | Both |
| `pm005Count` | Int | /dL | Particles ≥0.5µm count | Both |
| `pm01Count` | Int | /dL | Particles ≥1.0µm count | Both |
| `pm02Count` | Int | /dL | Particles ≥2.5µm count | Both |
| `pm50Count` | Int | /dL | Particles ≥5.0µm count | Both |
| `pm10Count` | Int | /dL | Particles ≥10µm count | Both |
| `atmp` | Float | °C | Temperature raw | Both |
| `atmpCompensated` | Float | °C | Temperature corrected | Both |
| `rhum` | Float | % | Relative humidity raw | Both |
| `rhumCompensated` | Float | % | Relative humidity corrected | Both |
| `tvocIndex` | Int | 1-500 | VOC index (Sensirion SGP41) | Both |
| `tvocRaw` | Int | - | VOC raw sensor signal | Both |
| `noxIndex` | Int | 1-500 | NOx index (Sensirion SGP41) | Both |
| `noxRaw` | Int | - | NOx raw sensor signal | Both |
| `boot` | Int | - | Measurement cycle counter | Both |
| `bootCount` | Int | - | Same as boot (HA compat) | Both |
| `ledMode` | String | - | Current LED display mode | Both |
| `firmware` | String | - | Firmware version | Both |
| `model` | String | - | Hardware model (I-9PSL) | Both |
```

---

## Section 4: FR-1.2 Example Firmware Version (CORRECTED)

**File:** `01-specification.md**
**Line:** 116
**Status:** LOW - Outdated firmware version in example

### CURRENT (INCORRECT):

```json
{
  ...
  "firmware": "3.1.3",
  ...
}
```

### CORRECTED:

```json
{
  ...
  "firmware": "3.4.1",
  ...
}
```

---

## Section 5: NEW - Firmware Version Note (ADD THIS)

**File:** `01-specification.md`
**Insert After:** Section 7.2 Field Source Table (after line 1107)
**Status:** RECOMMENDED - Documents version requirements

### ADD THIS NEW SECTION:

```markdown
### 7.2.1 Firmware Version Compatibility

**MQTT Payload Completeness by Firmware Version:**

- **Firmware 3.4.1+** (CURRENT): Full 29-field payload over MQTT
- **Firmware 3.0.0 - 3.4.0**: Partial MQTT payload (12-20 fields, varies by version)
- **Firmware <3.0.0**: Limited MQTT payload (6-12 core fields only)

**Recommendation:** Ensure AirGradient ONE sensors are updated to firmware 3.4.1 or later for full feature parity between MQTT and Local API. The platform will gracefully handle partial payloads from older firmware by:
- Storing available fields only
- Flagging readings with `firmware_outdated` quality flag
- Disabling features that require missing fields (particle counts, NOx monitoring)

**Field Availability by Firmware:**

| Field Category | Firmware 3.4.1+ | Firmware 3.0-3.4 | Firmware <3.0 |
|----------------|-----------------|------------------|---------------|
| Core PM (pm01, pm02, pm10) | MQTT + API | MQTT + API | MQTT + API |
| Standard PM | MQTT + API | API only | API only |
| Particle Counts | MQTT + API | API only | API only |
| Compensated Values | MQTT + API | API only | API only |
| NOx (Index + Raw) | MQTT + API | API only | API only |
| Device Metadata | MQTT + API | API only | API only |

**Verification:** Check `firmware` field in MQTT payload. If absent or <3.4.1, the platform should recommend Local API polling as fallback for complete data.
```

---

## Section 6: Update Data Source Guidance (CORRECTED)

**File:** `01-specification.md`
**Line:** 23-24 (Dual data ingestion section)
**Status:** RECOMMENDED - Remove bias toward Local API

### CURRENT:

```markdown
- **Dual data ingestion** from AirGradient ONE sensors:
  - MQTT subscription: `airgradient/readings/{SERIAL_NUMBER}` topic
  - Local HTTP API polling: `http://airgradient_{SERIAL}.local/measures/current`
```

### CORRECTED:

```markdown
- **Dual data ingestion** from AirGradient ONE sensors (firmware 3.4.1+):
  - **Primary:** MQTT subscription: `airgradient/readings/{SERIAL_NUMBER}` (push-based, real-time)
  - **Fallback:** Local HTTP API polling: `http://airgradient_{SERIAL}.local/measures/current` (pull-based, historical backfill)
  - **Note:** Both sources provide identical 29-field payloads. MQTT preferred for lower latency and reduced network overhead.
```

---

## Quick Apply Instructions

### Option 1: Manual Edit

1. Open `/workspaces/neural-data-platform/product/features/air-001/specs/01-specification.md`
2. Apply each section's corrections above
3. Save and commit

### Option 2: Use Edit Tool (Recommended)

See `SPEC_CORRECTIONS_EDIT_COMMANDS.sh` (generated below)

---

## Validation After Apply

Run these checks to verify corrections:

```bash
# 1. Verify topic pattern corrected
grep -n "airgradient/readings" /workspaces/neural-data-platform/product/features/air-001/specs/01-specification.md
grep -n "measurements" /workspaces/neural-data-platform/product/features/air-001/specs/01-specification.md

# 2. Verify field count note removed
grep -n "12 fields" /workspaces/neural-data-platform/product/features/air-001/specs/01-specification.md

# 3. Count "Both" entries in field table (should be 29, not 13)
grep -c "| Both |" /workspaces/neural-data-platform/product/features/air-001/specs/01-specification.md

# 4. Verify no "Local API only" entries remain in table
grep "| Local API |" /workspaces/neural-data-platform/product/features/air-001/specs/01-specification.md
```

Expected results:
- Line 77 should contain `airgradient/readings/{SERIAL_NUMBER}`
- Line 77 should NOT contain `measurements`
- Line 1108 should NOT contain "12 fields"
- Field table should have 29 "Both" entries
- Field table should have 0 "Local API" entries

---

## Sign-Off

- [ ] Technical Lead reviewed corrections
- [ ] Domain expert verified field table accuracy
- [ ] Product owner approved scope impact
- [ ] QA lead updated test plan for 29 MQTT fields
- [ ] Ready to proceed to implementation

---

**Created:** 2025-12-13
**Review Reference:** `/workspaces/neural-data-platform/product/features/air-001/CODE_REVIEW_SPEC_VS_ACTUAL_MQTT.md`
