# AIR-002: Ingestion Pipeline - MQTT to Parquet Data Flow

**Version:** 1.0.0
**Date:** December 14, 2025
**Status:** SPARC Specification Phase
**Focus:** Complete the front-end data ingestion for AirGradient sensor data

---

## Objective

Complete the fully integrated, tested ingestion pipeline:
```
AirGradient Sensor → MQTT Broker → Parser → Validator → Adapter → Parquet Storage
```

This addresses the **critical blocker** identified in air-001 gap analysis:
- FR-1.1: MQTT Client Connection (0% → 100%)
- FR-1.2: Message Parsing integration (exists but not wired)
- FR-1.3: Data Quality Assessment (partial → complete)
- FR-2.1-2.3: Storage integration (exists but not wired)

---

## SPARC Methodology

| Phase | Agent | Status |
|-------|-------|--------|
| Specification | specification | IN PROGRESS |
| Pseudocode | pseudocode | PENDING |
| Architecture | system-architect | PENDING |
| Refinement | tester | PENDING |
| Completion | planner | PENDING |

---

## Directory Structure

```
air-002/
├── README.md              # This file
├── specs/
│   └── 01-specification.md  # Detailed requirements
├── architecture/
│   └── 01-system-design.md  # Component architecture
├── pseudocode/
│   └── 01-algorithms.md     # Algorithm design
├── tests/
│   └── 01-test-plan.md      # Test strategy
└── implementation/
    └── 01-roadmap.md        # Implementation plan
```

---

## Success Criteria

1. MQTT client connects and subscribes to `airgradient/readings/+`
2. Readings flow through parser → validator → adapter → storage
3. Data persists to Parquet with WAL durability
4. REST API returns real (not mocked) data
5. Health endpoint reflects actual MQTT connection status
6. All integration tests pass

---

## References

- [air-001 Specification](/product/features/air-001/specs/01-specification.md)
- [Gap Analysis](/product/features/air-001/current-state/gaps/critical-blockers.md)
- [Existing Components](/product/features/air-001/current-state/analysis/)
