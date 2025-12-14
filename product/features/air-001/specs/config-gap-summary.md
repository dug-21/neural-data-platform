# Config Endpoint Gap Analysis - Executive Summary

**Document:** config-endpoint-gap-analysis.md
**Review Date:** December 13, 2025
**Severity:** CRITICAL

---

## Critical Issues Found

### 1. Temperature Unit Confusion (CRITICAL)
**Problem:** Devices can report in °F or °C - specification assumes always Celsius

**Impact:**
- US device reports 75°F
- Platform interprets as 75°C (167°F!)
- All temperature-based features broken

**Fix:** FR-8.2 Temperature Unit Conversion (1 day)

---

### 2. Wrong PM2.5 Value Used (CRITICAL)
**Problem:** Specification doesn't know which PM2.5 field to trust

**Impact:**
- Device applies EPA 2021 correction: raw=15, compensated=11 µg/m³
- Platform uses raw → false "Unhealthy" alert (15 > 12)
- Should use compensated → no alert (11 < 12)

**Fix:** FR-8.3 PM2.5 Field Selection (1 day)

---

### 3. Missing Sensor Warmup Tracking (HIGH)
**Problem:** CO2 needs 3 weeks warmup, VOC needs 12 hours - not tracked

**Impact:**
- New device shows CO2=1600ppm during warmup (unreliable)
- Platform triggers alert immediately
- Should suppress alerts for 3 weeks

**Fix:** FR-8.4 Calibration Status Tracking (3 days)

---

## Implementation Priority

### Phase 1 Blockers (Must Fix Before Production)
1. **FR-8.1: Config Retrieval** - 2 days
2. **FR-8.2: Temperature Conversion** - 1 day
3. **FR-8.3: PM Field Selection** - 1 day

**Total:** 4 days to fix data corruption issues

### Phase 2 (Quality Improvements)
4. **FR-8.4: Calibration Tracking** - 3 days
5. **FR-8.6: Config Change Detection** - 1 day

**Total:** 4 days for quality features

---

## Config Fields Reference

### Critical Fields (Affect Data Interpretation)
- `temperatureUnit`: "c" or "f" → requires conversion
- `corrections.pm02.correctionAlgorithm`: "epa_2021" | "none" → field selection
- `abcDays`: CO2 calibration period → warmup tracking
- `tvocLearningOffset`: VOC learning hours → suppress early alerts

### Important Fields (Affect Operations)
- `mqttBrokerUrl`: Empty means cloud broker
- `offlineMode`: true means local API only
- `pmStandard`: "ugm3" vs "usaqi" display

---

## Proposed New Requirements

**FR-8: Device Configuration Management**
- FR-8.1: Configuration retrieval and caching
- FR-8.2: Temperature unit normalization (°F → °C)
- FR-8.3: PM2.5 correction algorithm awareness
- FR-8.4: Sensor calibration status tracking
- FR-8.5: Data source capability detection
- FR-8.6: Configuration change detection
- FR-8.7: Multi-device configuration support
- FR-8.8: Configuration validation and defaults

**Enhancements to Existing FRs:**
- FR-1.2: Add config-aware parsing
- FR-1.3: Add calibration-driven quality scoring
- FR-5.1: Use correct PM2.5 field for alerts
- FR-6.1: Include config metadata in responses
- FR-6.4: Add config-driven sensor health details

---

## Risk Assessment

### Data Quality Risks
- **HIGH:** Existing data may have wrong units (needs reprocessing)
- **HIGH:** PM2.5 alerts currently unreliable
- **MEDIUM:** Temperature forecasts trained on wrong data

### Operational Risks
- **MEDIUM:** Config fetch timeout could delay startup (mitigation: cache)
- **LOW:** Config changes mid-day (mitigation: graceful transition)

---

## Testing Requirements

### Unit Tests (30+ tests)
- Temperature conversion (°F ↔ °C)
- PM field selection (epa_2021 vs none)
- Calibration state machine (warmup/learning/active)
- Config parsing and validation

### Integration Tests
- End-to-end with real device configs
- Multi-device heterogeneous configs
- Config change detection and revalidation

### Manual Testing
- Deploy to US device (°F, EPA 2021)
- Deploy to EU device (°C, no correction)
- Change config via AirGradient dashboard
- Verify platform detects change

---

## Recommendations

### Immediate (This Week)
✅ Review full gap analysis document
✅ Approve FR-8.1, FR-8.2, FR-8.3 for Phase 1
✅ Update specification to v1.2.0 with config support

### Short Term (Next Sprint)
- Implement FR-8.1, 8.2, 8.3 (4 days)
- Add unit tests for config-aware parsing
- Update existing FRs (1.2, 1.3, 5.1)

### Medium Term (Phase 2)
- Implement FR-8.4 calibration tracking (3 days)
- Integration testing with real devices
- Write device configuration guide

### Long Term (Post-v1.0)
- Historical data reprocessing tool
- Config change auditing
- Advanced calibration features

---

## Document Location

Full analysis: `/workspaces/neural-data-platform/product/features/air-001/specs/config-endpoint-gap-analysis.md`

**Sections:**
1. Executive Summary
2. Complete Field Catalog (19 config fields documented)
3. Specification Gap Analysis (8 gaps identified)
4. Proposed New FRs (FR-8.1 through FR-8.8)
5. Implementation Priority Matrix
6. Testing Strategy
7. Risks and Mitigations
8. Documentation Requirements
9. Backward Compatibility
10. Success Metrics
11. Recommendations
12. Appendices (schemas, examples)

**Pages:** 40+ pages of detailed analysis
**Code Examples:** 15+ Rust code snippets
**Test Cases:** 30+ unit/integration tests
**Config Examples:** 3 real-world device configs

---

## Next Steps

1. **Review Meeting:** Schedule with technical lead + domain expert
2. **Approval:** Get sign-off on FR-8.x requirements
3. **Specification Update:** Merge into 01-specification.md v1.2.0
4. **Implementation:** Start Phase 1 blockers (4 days)
5. **Validation:** Test with real AirGradient devices

**Timeline Impact:** +4 days to Phase 1 (critical path), +4 days to Phase 2 (quality)

**Total Effort:** 13 days across phases (2.6 weeks)

---

**Status:** Ready for Review
**Priority:** CRITICAL - Blocks production deployment
