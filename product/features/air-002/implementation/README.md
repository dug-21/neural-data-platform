# AIR-002 Implementation Documentation

**Feature:** MQTT to Parquet Data Ingestion Pipeline
**Status:** Planning Complete, Ready for Implementation
**Last Updated:** 2025-12-14

---

## Quick Navigation

### For Implementers
- **START HERE:** [Implementation Guide](./05-config-implementation-guide.md) - Step-by-step guide for T1
- **Roadmap:** [01-roadmap.md](./01-roadmap.md) - Full task breakdown (22-30 hours)
- **Timeline:** [04-timeline-comparison.md](./04-timeline-comparison.md) - Visual timeline analysis

### For Decision Makers
- **Executive Summary:** [03-scope-decision-summary.md](./03-scope-decision-summary.md) - Quick decision overview
- **Full Analysis:** [02-config-scope-analysis.md](./02-config-scope-analysis.md) - Detailed scope analysis

---

## Document Overview

### 01-roadmap.md
**Purpose:** Complete implementation roadmap for AIR-002
**Audience:** Developers, Project Managers
**Key Sections:**
- Task breakdown (T1-T6)
- Milestone definitions
- Dependency graph
- Resource requirements
- Risk assessment

**Key Facts:**
- Total Effort: 22-30 hours (REVISED)
- Critical Path: 23 hours
- Timeline: 2.75 days (single dev) or 1.5-2 days (two devs)
- Tasks: 6 main tasks (T1-T6)

### 02-config-scope-analysis.md
**Purpose:** In-depth analysis of config-store integration decision
**Audience:** Architects, Tech Leads
**Key Sections:**
- Current state assessment
- Option analysis (3 options)
- Technical debt analysis
- Dependency chain impact
- Migration path

**Key Facts:**
- Decision: Option 3 (Minimal Config)
- Savings: 2 hours on critical path
- Tech Debt: 3-5 hours (deferred to AIR-003)
- Confidence: 95%

### 03-scope-decision-summary.md
**Purpose:** Executive summary of config scope decision
**Audience:** Stakeholders, Decision Makers
**Key Sections:**
- Impact summary
- Timeline comparison
- Migration path
- Action items

**Key Facts:**
- Approved: Minimal YAML Config
- Time to E2E: 2.75 days (vs 4.5 days full standardization)
- ROI: 62% (5h net gain on 8h investment)

### 04-timeline-comparison.md
**Purpose:** Visual timeline and effort comparison
**Audience:** All stakeholders
**Key Sections:**
- Critical path diagrams
- Effort breakdown charts
- Risk heatmaps
- Decision scorecard

**Key Facts:**
- Option 3 wins: 82% weighted score
- 1.75 days faster than Option 1
- Lowest risk profile

### 05-config-implementation-guide.md
**Purpose:** Practical step-by-step implementation guide for T1
**Audience:** Developers implementing T1
**Key Sections:**
- Quick start checklist
- Code examples
- Testing procedures
- Troubleshooting

**Key Facts:**
- Estimated Time: 1-2 hours
- Steps: 5 main steps
- Tests: 6 comprehensive tests
- Files: 3 created/modified

---

## Implementation Quick Start

### For New Developers

1. **Read First:**
   - [03-scope-decision-summary.md](./03-scope-decision-summary.md) - Understand the decision
   - [01-roadmap.md](./01-roadmap.md) - See the big picture

2. **Implement T1:**
   - Follow [05-config-implementation-guide.md](./05-config-implementation-guide.md)
   - Estimated: 1-2 hours
   - Files: `config.rs`, `config.yaml`

3. **Move to T2:**
   - See [01-roadmap.md](./01-roadmap.md) Task 2
   - Use config from T1

### For Project Managers

1. **Timeline:**
   - Single Developer: 22-30 hours (2.75-3.75 days)
   - Two Developers: 14-18 hours (1.5-2.25 days)

2. **Milestones:**
   - M1: Config + MQTT (9-10h)
   - M2: Storage (5-6h)
   - M3: Integration (6-8h)
   - M4: Testing (4-5h)

3. **Critical Path:**
   - T1 (2h) → T2 (8h) → T4 (5h) → T5 (3h) → T6 (5h) = 23h

### For Architects

1. **Technical Decisions:**
   - Config: Simple YAML (deferred config-store to AIR-003)
   - MQTT: Use platform-core MqttSource
   - Storage: Use platform-core ParquetStore with WAL

2. **Tech Debt:**
   - Accepted: 3-5 hours refactoring in AIR-003
   - Mitigation: Clear migration path, isolated impact
   - Timeline: Not on critical path

3. **Future Work:**
   - AIR-003: Config standardization (3-5h)
   - AIR-004: Alerting system
   - AIR-005: Forecasting integration

---

## Key Decisions

### Decision 1: Config Approach
**Question:** Should AIR-002 include config-store integration?
**Answer:** No, defer to AIR-003
**Rationale:**
- Saves 2 hours on critical path
- Reduces complexity and risk
- Manageable tech debt (3-5h later)
- Fastest path to E2E testing

**Documents:** [02-config-scope-analysis.md](./02-config-scope-analysis.md), [03-scope-decision-summary.md](./03-scope-decision-summary.md)

### Decision 2: Task Scope Reduction
**Question:** Should T1 scope be reduced?
**Answer:** Yes, from 3-4h to 1-2h
**Rationale:**
- Remove config-store integration
- Remove advanced validation
- Keep only essentials: YAML loading, env vars, type conversion

**Documents:** [01-roadmap.md](./01-roadmap.md)

### Decision 3: Migration Strategy
**Question:** How to migrate to config-store later?
**Answer:** Implement in AIR-003 as separate feature
**Rationale:**
- Not blocking E2E testing
- Clear migration path documented
- Can be done in 3-5 hours

**Documents:** [02-config-scope-analysis.md](./02-config-scope-analysis.md) Appendix

---

## Success Criteria

### For AIR-002 Completion

**Functional:**
- [ ] MQTT messages flow to Parquet storage
- [ ] REST API returns real sensor data
- [ ] Health endpoint shows accurate status
- [ ] Integration tests validate persistence
- [ ] WAL recovery works on restart

**Performance:**
- [ ] Handle 10 messages/second
- [ ] <100MB memory footprint
- [ ] <5% CPU idle, <50% CPU peak
- [ ] MQTT ingestion within 1 second
- [ ] Parquet writes within 5 seconds

**Quality:**
- [ ] All unit tests passing
- [ ] All integration tests passing
- [ ] Code reviewed and approved
- [ ] Documentation updated

---

## Timeline Summary

### Critical Milestones

```
Day 1: Configuration + MQTT Start
├─ T1: Config (2h) ✅ Simple YAML
└─ T2: MQTT (6h) - Connection + parsing

Day 2: MQTT Completion + Storage
├─ T2: MQTT finish (2h)
└─ T3: Storage (6h) - Parquet + WAL

Day 3: Integration + Health
├─ T4: Main Integration (5h)
└─ T5: Health Endpoint (3h)

Day 4: Testing
└─ T6: Integration Tests (5h) + Buffer (1h)
```

**Total:** 23 hours critical path (29 hours with parallel work)

---

## Risk Management

### Low Risk (Accepted)
- **Tech Debt:** 3-5h refactoring later (AIR-003)
- **YAML Parsing:** Well-tested technology

### Medium Risk (Mitigated)
- **Config Format Changes:** Unlikely in 3-4 days, easy to update
- **MQTT Broker Availability:** Documented setup, auto-reconnect

### High Risk (Avoided)
- **Config-Store Integration:** DEFERRED to AIR-003
- **Complex Abstractions:** Using simple, direct approaches

---

## References

### External Dependencies
- **platform-core:** `/workspaces/neural-data-platform/core/`
  - `sources::mqtt::MqttSource`
  - `storage::parquet::ParquetStore`
  - `storage::wal::WriteAheadLog`

- **config-store:** `/workspaces/neural-data-platform/config-store/`
  - Not used in AIR-002
  - Will integrate in AIR-003

- **air-quality domain:** `/workspaces/neural-data-platform/domains/air-quality/`
  - Parser, validator, adapter (already implemented)

### Related Features
- **AIR-001:** Air quality monitoring platform (COMPLETED)
- **AIR-003:** Configuration standardization (PLANNED)
- **AIR-004:** Alerting system (PLANNED)
- **AIR-005:** Forecasting integration (PLANNED)

---

## FAQ

### Q: Why not use config-store in AIR-002?
**A:** It adds 9-12 hours to the timeline and increases risk on the critical path. We can add it later in AIR-003 (3-5h) without blocking E2E testing.

### Q: What's the total effort?
**A:** 22-30 hours total (23h critical path). With two developers, can ship in 1.5-2 days.

### Q: What happens in AIR-003?
**A:** Config standardization - migrate to config-store, add TOML support, advanced validation. Estimated 3-5 hours.

### Q: Can we change the config format later?
**A:** Yes, the simple YAML config has a clear conversion method. Migration to config-store is well-documented.

### Q: What if requirements change?
**A:** Simple YAML config is flexible. Can add fields easily. Migration to config-store provides more features when needed.

### Q: Is this production-ready?
**A:** The ingestion pipeline is production-ready. Config standardization (AIR-003) adds production-grade config management.

---

## Next Steps

### Immediate (Now)
1. Review [05-config-implementation-guide.md](./05-config-implementation-guide.md)
2. Implement T1 (1-2 hours)
3. Run tests and verify
4. Move to T2

### Short-term (This Feature)
1. Complete T2-T6 per [01-roadmap.md](./01-roadmap.md)
2. Pass integration tests
3. Document any learnings

### Medium-term (Next Feature)
1. Plan AIR-003: Config standardization
2. Migrate to config-store
3. Add TOML support

### Long-term (Future)
1. AIR-004: Alerting system
2. AIR-005: Forecasting integration
3. AIR-006: MCP tools

---

## Document Change Log

| Date | Document | Change | Author |
|------|----------|--------|--------|
| 2025-12-14 | 01-roadmap.md | Created initial roadmap | Strategic Planning Agent |
| 2025-12-14 | 02-config-scope-analysis.md | Analyzed config options | Strategic Planning Agent |
| 2025-12-14 | 03-scope-decision-summary.md | Executive summary | Strategic Planning Agent |
| 2025-12-14 | 04-timeline-comparison.md | Visual comparison | Strategic Planning Agent |
| 2025-12-14 | 05-config-implementation-guide.md | Implementation guide | Strategic Planning Agent |
| 2025-12-14 | 01-roadmap.md | Updated with config decision | Strategic Planning Agent |
| 2025-12-14 | README.md | Created index | Strategic Planning Agent |

---

**End of Documentation Index**
