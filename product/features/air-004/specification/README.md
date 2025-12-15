# AIR-004 Specification Documents

## Overview

This directory contains the complete specification for AIR-004: Generic Multi-Stream Data Platform.

**Key Insight**: This specification has been revised to align with the CURRENT WORKING air quality monitoring system, ensuring an additive implementation approach rather than a greenfield rewrite.

---

## Documents

### 1. SPECIFICATION.md (Primary Document)
**File**: `/workspaces/neural-data-platform/product/features/air-004/specification/SPECIFICATION.md`
**Size**: 1,201 lines (45KB)
**Version**: 1.1.0 (Revised)

**Purpose**: Complete SPARC specification for multi-stream platform extension

**Key Sections**:
- **Section 0 (NEW)**: Current Implementation Baseline
  - Documents working MQTT ingestion (MqttSource, 593 LOC)
  - Documents working Parquet storage (ParquetStore, WAL, batching)
  - Documents working config system (config-client, 260 LOC)
  - Defines critical constraints (no breaking changes)

- **Sections 1-9**: Functional/Non-Functional Requirements
  - FR-001: Stream Registry (EXTENDS existing etcd patterns)
  - FR-002: Multi-Source Ingestion (BUILDS ON existing MqttSource)
  - FR-003: Schema Validation (new capability)
  - FR-004: Bronze Layer Storage (ALREADY IMPLEMENTED)
  - FR-005: Silver/Gold Layer (TimescaleDB - deferred)
  - NFR-001: Performance (references CURRENT baselines)

- **Section 10**: Implementation Phases (REVISED)
  - Phase 0: Baseline Verification (NEW - regression testing)
  - Phases 1-5: Additive approach with backward compatibility

- **Section 15 (NEW)**: Alignment Summary
  - What changed from v1.0.0 to v1.1.0
  - Critical constraints honored
  - Implementation impact analysis

**Read this if**: You need complete technical requirements for AIR-004

---

### 2. REVISION_SUMMARY.md
**File**: `/workspaces/neural-data-platform/product/features/air-004/specification/REVISION_SUMMARY.md`
**Size**: 217 lines (6.3KB)

**Purpose**: Executive summary of changes from v1.0.0 to v1.1.0

**Contents**:
- Critical finding: Working system exists
- Major revisions (Sections 0, FR-001, FR-002, NFR-001, Phases)
- Before/after comparisons
- What stayed the same
- Implementation impact
- Verification checklist

**Read this if**: You need to understand why the spec was revised

---

### 3. IMPLEMENTATION_CONSTRAINTS.md
**File**: `/workspaces/neural-data-platform/product/features/air-004/specification/IMPLEMENTATION_CONSTRAINTS.md`
**Size**: 272 lines (7.6KB)

**Purpose**: Developer quick reference for implementation rules

**Contents**:
- 5 Non-Negotiable Rules
  1. Preserve existing functionality
  2. Backward compatible configuration
  3. No performance regression
  4. Data continuity
  5. Additive implementation

- Phase-by-phase constraints
- Code review checklist
- Rollback plan
- Red flags to watch for

**Read this if**: You're implementing AIR-004 and need to know what NOT to do

---

## Document Relationship

```
SPECIFICATION.md (1,201 lines)
│
├─ Comprehensive technical spec
├─ Defines ALL requirements
└─ Source of truth for implementation
    │
    ├─► REVISION_SUMMARY.md (217 lines)
    │   └─ Why spec was revised (context)
    │
    └─► IMPLEMENTATION_CONSTRAINTS.md (272 lines)
        └─ Developer guardrails (rules)
```

---

## Quick Start

### For Stakeholders
1. Read: REVISION_SUMMARY.md (10 min)
2. Review: SPECIFICATION.md Section 0 (Current Baseline)
3. Review: SPECIFICATION.md Section 15 (Alignment Summary)

### For Architects
1. Read: SPECIFICATION.md Sections 0, 1, 2 (baseline + requirements)
2. Review: SPECIFICATION.md Section 10 (implementation phases)
3. Check: IMPLEMENTATION_CONSTRAINTS.md (verify approach)

### For Developers
1. **MUST READ**: IMPLEMENTATION_CONSTRAINTS.md (all 5 rules)
2. **Reference**: SPECIFICATION.md for technical details
3. **Context**: REVISION_SUMMARY.md for "why"

### For Reviewers
1. Use: IMPLEMENTATION_CONSTRAINTS.md code review checklist
2. Verify: Phase 0 regression tests pass
3. Check: Performance benchmarks vs baseline

---

## Key Takeaways

### What AIR-004 IS
- Extension of working air quality monitoring system
- Additive implementation (new features alongside old)
- Multi-stream capability without breaking single-stream

### What AIR-004 IS NOT
- Greenfield rewrite
- Refactoring of existing MqttSource
- Migration of /air-quality/* config
- Performance downgrade

### Critical Success Factors
1. Air-quality MQTT ingestion continues working throughout
2. Existing etcd `/air-quality/*` keys remain valid
3. Current Parquet data remains queryable
4. Performance stays within ±10% of baseline
5. Rollback capability exists at every phase

---

## Verification Before Implementation

**BEFORE starting Phase 1**:

```bash
# 1. Verify air-quality system works NOW
cd /workspaces/neural-data-platform
docker-compose up -d air-quality-app
docker logs -f air-quality-app | grep "Connected to MQTT"

# 2. Check etcd configuration
etcdctl get --prefix /air-quality/ | head -20

# 3. Verify Parquet data exists
ls -lh data/*/year=*/month=*/day=*/readings.parquet | head -10

# 4. Benchmark current performance (Phase 0)
# (Create benchmark scripts as per Phase 0 requirements)
```

**If any verification fails**: FIX THE CURRENT SYSTEM FIRST before starting AIR-004

---

## Phase 0 Deliverables (Week 1)

Before any new code is written:

- [ ] Document all active `/air-quality/*` etcd keys
- [ ] Create integration test: MQTT → Parquet end-to-end
- [ ] Benchmark: Config read latency (<10ms target)
- [ ] Benchmark: MQTT ingestion throughput (1+ msg/sec sustained)
- [ ] Benchmark: Parquet write throughput (10k records/sec target)
- [ ] Verify: Query existing Parquet data successfully
- [ ] **GATE**: All tests pass before Phase 1 starts

---

## References

### Current Working Implementation
- `core/src/sources/mqtt.rs` - MqttSource (593 LOC, 11 tests)
- `core/src/storage/parquet.rs` - ParquetStore (639 LOC, 18 tests)
- `config-client/src/lib.rs` - ConfigClient (260 LOC total)

### Related Documentation
- AIR-001: Core platform foundation
- AIR-002: Configuration management
- AIR-003: etcd-based configuration with hot-reload

### etcd Namespace
```
/air-quality/          (PRESERVE - backward compatible)
  ├── server/*
  ├── mqtt/*
  ├── storage/*
  ├── alerts/*
  └── logging/*

/streams/*             (NEW - AIR-004 additions)
  └── {stream-id}/
      ├── config
      ├── schema
      └── sources
```

---

## Contact / Questions

If implementation approach is unclear:

1. Check: IMPLEMENTATION_CONSTRAINTS.md (rules)
2. Review: SPECIFICATION.md relevant section
3. Verify: Phase 0 regression tests still pass
4. Principle: When in doubt, preserve existing functionality

---

## Document History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2025-12-15 | Initial specification (greenfield approach) |
| 1.1.0 | 2025-12-15 | Revised for current implementation alignment |

---

*Last Updated*: 2025-12-15
*Status*: Specification Complete - Ready for Phase 0 Baseline Verification
*Next Phase*: Phase 0 (Regression Testing & Performance Baseline)
