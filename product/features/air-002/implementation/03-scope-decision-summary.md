# AIR-002 Config Scope Decision - Executive Summary

**Decision Date:** 2025-12-14
**Status:** APPROVED
**Decision Owner:** Strategic Planning Agent

---

## The Question

Should AIR-002 (MQTT ingestion pipeline) include config-store standardization, or use minimal YAML config?

---

## The Decision

**APPROVED: Option 3 - Minimal YAML Config**

Defer config-store integration to AIR-003 (separate feature).

---

## Impact Summary

| Metric | Before | After | Change |
|--------|--------|-------|--------|
| **Total Effort** | 24-32h | 22-30h | -2h (8% faster) |
| **T1 Duration** | 3-4h | 1-2h | -2h (50% faster) |
| **Critical Path** | 25h | 23h | -2h |
| **Time to E2E** | 3-4 days | 2.5-3.5 days | -0.5 days |
| **Tech Debt** | None | 3-5h (AIR-003) | Manageable |

---

## What Changed in AIR-002

### T1: Configuration Management

**OLD Scope (3-4 hours):**
- Complex config system
- Full validation
- Versioning support
- Config-store integration

**NEW Scope (1-2 hours):**
- Simple YAML loading
- Basic env var overrides
- Convert to platform-core types
- Default config fallback

**Removed (Deferred to AIR-003):**
- config-store integration
- TOML format
- Schema validation
- Config versioning

---

## What's in AIR-003 (New Feature)

**Title:** Configuration Standardization
**Effort:** 3-5 hours
**Priority:** MEDIUM (not blocking E2E)

**Tasks:**
1. Add MqttConfig to config-store (1h)
2. Add StorageConfig to config-store (1h)
3. Migrate air-quality-app to use config-store (1-2h)
4. Testing and validation (1h)

---

## Why This Decision?

### Primary Goals Achieved
1. Unblock E2E testing FASTEST (22-30h vs 33-44h for full standardization)
2. Minimize risk on critical path
3. Enable parallel work (no config-store crate modifications needed)
4. Simplify debugging during development

### Technical Debt Accepted
- 3-5 hours of refactoring work in AIR-003
- Temporary duplication of config structs
- Two config formats in codebase (YAML and TOML)

### Risk Mitigation
- Tech debt is isolated to single app
- Clear migration path documented
- Refactoring is NOT on critical path
- Config structure matches platform-core exactly

---

## Timeline Comparison

### Option 1: Full Config-Store Standardization
```
T0.1→T0.4 (9h) → T1 (6h) → T2 (8h) → T4 (5h) → T5 (3h) → T6 (5h) = 36h
└─ REJECTED: 37% slower, blocks E2E for 1.5 extra weeks
```

### Option 2: Lightweight Config Client
```
T0 (3h) → T1 (4h) → T2 (8h) → T4 (5h) → T5 (3h) → T6 (5h) = 28h
└─ REJECTED: Still adds complexity, intermediate abstraction
```

### Option 3: Minimal Config (APPROVED)
```
T1 (2h) → T2 (8h) → T4 (5h) → T5 (3h) → T6 (5h) = 23h
└─ APPROVED: Fastest, simplest, lowest risk
```

---

## Developer Timeline Impact

### Single Developer
**Before:** 24-32 hours (3-4 days)
**After:** 22-30 hours (2.5-3.5 days)
**Savings:** 2 hours (half day faster to E2E)

### Two Developers (Recommended)
**Before:** 16-20 hours (2 days)
**After:** 14-18 hours (1.5-2 days)
**Savings:** 2 hours (can ship in 1.5 days!)

---

## Migration Path

### Phase 1: AIR-002 (NOW)
```rust
// apps/air-quality-app/src/config.rs
pub struct AppConfig {
    mqtt: MqttConfigYaml,
    storage: StorageConfigYaml,
}

impl MqttConfigYaml {
    pub fn to_mqtt_config(&self) -> platform_core::MqttConfig {
        // Direct conversion
    }
}
```

### Phase 2: AIR-003 (LATER)
```rust
// config-store/configs/sources.rs
pub struct MqttConfig { ... }

// apps/air-quality-app/src/main.rs
use config_store::{PlatformConfig, MqttConfig};

let config = PlatformConfig::load_from_file("config.toml")?;
let mqtt = config.mqtt.to_mqtt_config();
```

### Phase 3: Production (FUTURE)
- All apps use config-store
- Centralized config management
- Advanced features (secrets, validation, versioning)

---

## Action Items

### Immediate (AIR-002 Implementation)
- [x] Update roadmap T1 estimate: 3-4h → 1-2h
- [x] Update total effort: 24-32h → 22-30h
- [x] Document deferred work in "Next Steps"
- [ ] Implement simple YAML config (T1)
- [ ] Convert to platform-core types
- [ ] Add env var overrides

### Deferred (AIR-003 Planning)
- [ ] Create AIR-003 feature spec
- [ ] Plan config-store integration
- [ ] Design migration strategy
- [ ] Schedule for post-E2E

### Future (AIR-004+)
- [ ] Standardize all apps on config-store
- [ ] Add secrets management
- [ ] Implement config service

---

## Key Takeaways

1. **Speed over perfection:** E2E testing is the priority, not config elegance
2. **Defer non-critical work:** Config standardization doesn't block functionality
3. **Manage tech debt:** 3-5 hours is acceptable for 2-hour savings NOW
4. **Isolate risk:** Simple YAML is proven, config-store integration is not critical path

---

## References

- **Full Analysis:** `/workspaces/neural-data-platform/product/features/air-002/implementation/02-config-scope-analysis.md`
- **Updated Roadmap:** `/workspaces/neural-data-platform/product/features/air-002/implementation/01-roadmap.md`
- **Config Store:** `/workspaces/neural-data-platform/config-store/`
- **Platform Core:** `/workspaces/neural-data-platform/core/src/sources/mqtt.rs`

---

**End of Summary**
