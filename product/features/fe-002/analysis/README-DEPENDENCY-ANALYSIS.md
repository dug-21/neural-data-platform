# Dependency Analysis: GAP-001 vs GAP-003

**Analysis Date:** 2026-02-05
**Status:** COMPLETE - READY FOR SPRINT PLANNING
**Document Index:** 3 Files | 58 KB | Comprehensive Analysis

---

## Quick Answer

**Can GAP-003 be done before GAP-001?**
- ❌ **NO** - Schema validation requires JSON format; YAML files will be deleted during GAP-001 migration

**Can GAP-001 be done before GAP-003?**
- ✅ **YES** - Strongly recommended. Format migration enables validation.

**Should they be combined or separate?**
- ✅ **COMBINE** - Single V1.2 feature with sequential phases (2h + 3h = 5h total)

---

## Document Guide

### 1. DEPENDENCY-ANALYSIS-GAP-001-GAP-003.md (18 KB)

**For:** Architecture reviewers, decision makers, stakeholders

**Contains:**
- Executive summary with key finding
- Detailed issue overview (GAP-001 and GAP-003)
- 4-question dependency analysis
  - Can GAP-003 be done before GAP-001? (Technical blockers + rework risk)
  - Can GAP-001 be done before GAP-003? (Benefits + testing clarity)
  - Shared changes in loader.rs
  - Cascade effects and dependencies
- Configuration schema status (current vs. after migration)
- Risk assessment and mitigation
- Combined feature specification
- Alternative analysis (Option A: Combined vs Option B: Separate)
- Implementation checklist (30+ items)
- Evidence and references

**Read this if you:** Make architectural decisions, approve features, or need full context

---

### 2. DEPENDENCY-QUICK-REFERENCE.md (9.7 KB)

**For:** Developers, sprint planners, quick reference

**Contains:**
- One-sentence summary
- Visual dependency flow (ASCII diagram)
- Can they run in parallel? (Risk matrix)
- Shared code visualization (before → after → complete)
- Impact matrix (5 components × 2 issues)
- Risk: Schema-struct mismatch (with resolution steps)
- Decision points and checkpoints
- Why NOT separate? (intermediate state problems)
- Execution estimate table (8 tasks, 4.5-5 hours)
- Verification commands
- Recommendation summary

**Read this if you:** Need a one-page summary or are implementing the feature

---

### 3. IMPLEMENTATION-GUIDE-GAP-001-GAP-003.md (30 KB)

**For:** Rust developers (hands-on implementation)

**Contains:**
- Phase 1: Domain Config Format Migration (GAP-001) - 2 hours
  - 1.1: Migrate domain.yaml to domain.json (with full example)
  - 1.2: Update loader.rs path (before/after code)
  - 1.3: Update loader.rs parser (before/after code)
  - 1.4: Update domain.rs tests (2 YAML→JSON conversions)
  - 1.5: Test & verify (6 verification commands)

- Phase 2: JSON Schema Validation (GAP-003) - 3 hours
  - 2.1: Create validator.rs module (full source code, 120 lines)
  - 2.2: Update Cargo.toml (add jsonschema crate)
  - 2.3: Update loader.rs integration (before/after code)
  - 2.4: Create test fixtures (5 JSON files with examples)
  - 2.5: Add integration tests (full test code, 6 tests)
  - 2.6: Test & verify (6 verification commands)

- Phase 3: Documentation (30 minutes)
  - 3.1: Update VALIDATION-PROCEDURE.md

- Checkpoints after each phase
- Full end-to-end verification procedure
- Timeline summary (8 tasks broken down)
- Rollback plan (3 scenarios)
- Success criteria checklist

**Read this if you:** Are implementing GAP-001 and GAP-003

---

## Key Findings Summary

### 1. Sequential Dependency (CONFIRMED)

```
GAP-001 (YAML→JSON)
    ↓ (enables)
GAP-003 (Schema Validation)
```

**Why:** Schema validates JSON format. After GAP-001 completes, domain.json exists and validator can run cleanly.

### 2. Shared Code: loader.rs

All changes touch this single file:

| Line | GAP-001 | GAP-003 |
|------|---------|---------|
| 46-47 | Path change (domain.yaml → domain.json) | Uses new path |
| 80 | Parser change (serde_yaml → serde_json) | Validator input source |
| 69-85 | Wrapper function | Integration point |

**Result:** Clean sequential integration possible, no conflicts

### 3. Combined Feature Recommended

| Aspect | Value |
|--------|-------|
| **Feature Name** | "V1.2: Domain Configuration Standardization" |
| **Total Time** | ~5 hours (2h Phase 1 + 3h Phase 2) |
| **Architecture** | Sequential phases with checkpoints |
| **Risk** | Low-Medium (schema-struct mismatch main risk) |
| **Benefit** | Avoids intermediate state, single narrative |

### 4. Risk Assessment

| Risk | Probability | Severity | Mitigation |
|------|-------------|----------|-----------|
| Schema-struct mismatch | Medium | Medium | Test immediately after Phase 1 |
| Phase 1 breaks tests | Low | Low | Git revert (1 command) |
| Phase 2 validation too strict | Low | Medium | Schema already comprehensive |

---

## Recommendation for Stakeholders

### For Sprint Planners
- Schedule as **single V1.2 epic** (not two separate features)
- Two sequential work phases with clear checkpoints
- Estimated 5 hours dev time (technical spike + 2-3 hour implementation)
- Flag: Checkpoint after Phase 1 before proceeding to Phase 2

### For Architecture
- Update ADR-016-001 reference to include both issues
- Document as "JSON as authoritative configuration format"
- Consolidate YAML usage (eliminated after this feature)

### For Rust Developer
- Follow IMPLEMENTATION-GUIDE-GAP-001-GAP-003.md exactly
- Test thoroughly after Phase 1 (before Phase 2)
- Pay attention to schema-struct wrapper mismatch (section 2.1)

### For QA/Testing
- Prepare test matrix now (independent of code)
- Phase 1: Validate domain loading works
- Phase 2: Positive + negative schema validation cases
- All cases in DEPENDENCY-QUICK-REFERENCE.md commands

---

## Decision Timeline

```
NOW:  Review this analysis (15 minutes)
      ↓
PLAN: Sprint planning decision (5 minutes)
      - Combine? YES
      - When? V1.2 (next sprint or future)
      ↓
PREP: Developer reads IMPLEMENTATION-GUIDE (20 minutes)
      ↓
EXEC: Phase 1 (2 hours) → Checkpoint → Phase 2 (3 hours)
      ↓
DONE: Merge + Deploy (~30 min after implementation)
```

---

## File Locations (Absolute Paths)

Generated Analysis Documents:
- `/workspaces/neural-data-platform/product/features/fe-001/DEPENDENCY-ANALYSIS-GAP-001-GAP-003.md`
- `/workspaces/neural-data-platform/product/features/fe-001/DEPENDENCY-QUICK-REFERENCE.md`
- `/workspaces/neural-data-platform/product/features/fe-001/IMPLEMENTATION-GUIDE-GAP-001-GAP-003.md`

Related Project Files:
- **Schema:** `/workspaces/neural-data-platform/config/schemas/domain.schema.json`
- **Config:** `/workspaces/neural-data-platform/config/domains/indoor-air-quality/domain.yaml` (to be migrated)
- **Code:** `/workspaces/neural-data-platform/tools/ndp-gold-ddl/src/config/loader.rs`
- **Code:** `/workspaces/neural-data-platform/tools/ndp-gold-ddl/src/config/domain.rs`
- **Issues:** GitHub #11 (GAP-001), #13 (GAP-003)
- **Report:** `/workspaces/neural-data-platform/product/features/fe-001/phase-d/reports/FAST-FOLLOWER-REPORT.md`

---

## How to Use These Documents

### Scenario 1: Architecture Review Meeting
1. Mention: "Analysis complete, 3 documents generated"
2. Present: DEPENDENCY-QUICK-REFERENCE.md (one page)
3. Deep Dive: DEPENDENCY-ANALYSIS-GAP-001-GAP-003.md if questions
4. Decision: Combine for V1.2? (Yes / No / Defer)

### Scenario 2: Sprint Planning
1. Read: DEPENDENCY-QUICK-REFERENCE.md (5 min)
2. Decision: "Combine as single V1.2 epic, ~5 hours"
3. Assign: Developer reads IMPLEMENTATION-GUIDE
4. Schedule: 2h Phase 1 + checkpoint + 3h Phase 2

### Scenario 3: Implementation
1. Read: IMPLEMENTATION-GUIDE-GAP-001-GAP-003.md (20 min)
2. Phase 1: Follow sections 1.1-1.5 (2 hours)
3. Checkpoint: All tests pass? Yes → Phase 2
4. Phase 2: Follow sections 2.1-2.6 (3 hours)
5. Verify: Full end-to-end test passes
6. Merge!

### Scenario 4: Debugging Issues
1. Check: IMPLEMENTATION-GUIDE section "Rollback Plan"
2. Find: Specific risk in DEPENDENCY-ANALYSIS section "Risk & Mitigation"
3. Reference: Exact code locations and line numbers

---

## Dependencies and Context

### Blocking Issues
- **#11 GAP-001:** Domain Config Format Inconsistency
- **#13 GAP-003:** No JSON Schema Validation for Domains

### Related Architecture Decisions
- **ADR-016-001:** JSON Configuration Standard
- **dp-019:** Stream Config Validation (Layer 1 + Layer 2)

### Related Features
- **FE-001 Phase E:** Deployed and operational (event detection)
- **V1.2:** Next feature planning cycle

### Constraints
- Rust stack (serde_json, serde_yaml, jsonschema crates)
- Config-driven architecture (no code hard-dependencies on format)
- Pi deployment path (config located at `/opt/ndp/config` on device)

---

## Next Steps

1. **Review** this index (5 minutes)
2. **Read** DEPENDENCY-QUICK-REFERENCE.md (5 minutes)
3. **Present** to team / stake decision
4. **If Approved:** Assign IMPLEMENTATION-GUIDE to developer
5. **If Deferred:** Archive for V1.2 planning

---

## Questions Answered by This Analysis

| Question | Document | Section |
|----------|----------|---------|
| Can GAP-003 precede GAP-001? | Analysis | Q1 |
| Can GAP-001 precede GAP-003? | Analysis | Q2 |
| Shared code locations? | Analysis | Q3 |
| Combine or separate? | Quick Ref | "Why NOT Separate?" |
| How long will it take? | Quick Ref | Execution Estimate |
| What's the rollback plan? | Implementation | Rollback Plan |
| Exact code changes needed? | Implementation | Phase 1 & 2 sections |
| How to verify success? | Implementation | Success Criteria |

---

**Analysis Status:** ✅ COMPLETE
**Ready for:** Sprint Planning, Architecture Review, Implementation
**Last Updated:** 2026-02-05

---

*For questions or clarifications, refer to specific sections above or the detailed documents.*
