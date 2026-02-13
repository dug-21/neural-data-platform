# Product Vision Alignment Criteria

> **Owner**: User (edit this anytime — all agents align immediately)
> **Last updated**: 2026-02-13
> **Referenced by**: ndp-vision-guardian agent, /align skill

This document defines the checkable criteria that every specification, feature, and bug fix must satisfy. Agents read this document — not the full vision docs — when producing alignment reports.

For full context, see:
- `product/vision/EDGE-INTELLIGENCE-PLATFORM.md` — master product vision
- `product/vision/ROADMAP-TO-V2.md` — version roadmap
- `product/INTEGRATION_FIRST_MANDATE.md` — integration rules

---

## The Vision (1 sentence)

A Configuration driven, generic data platform that uses neural capabilities to self learns patterns, predicts & triggers actions that operates offline, at the edge.
---

## Future Flexibiity
Architecture for the future, build for now.  Want the flexibility in the future to:
- Enable full management interface through command line and MCP layer, and/or API
- 

## Version Roadmap (current state)

| Version | Focus | Status |
|---------|-------|--------|
| v1.0 | Bronze→Silver pipeline, declarative deployment | COMPLETE |
| v1.1 | Gold layer, ML-ready features, stream classification, objectives | IN PROGRESS |
| v1.2 | Discovery engine — automatic correlation detection | PLANNED |
| v1.3 | Prediction & actions — causal validation, model selection | PLANNED |
| v2.0 | Cross-domain intelligence — financial adapter, multi-domain | PLANNED |

**Rule**: Features must target the current or next version. Do not build v1.3 capabilities during v1.1 work unless explicitly scoped.

---

## Alignment Principles (7 checks)

### 1. Edge-Only
- All processing runs on-device (Raspberry Pi 5, 16GB RAM)
- No cloud dependencies for core functionality
- No data leaves the device unless user explicitly configures export
- Internet not required after initial setup

### 2. Config-Driven
- Users declare objectives, not implementations
- All behavior controlled via declarative JSON configuration (YAML limited to docker)
- No hardcoded values for: retention periods, thresholds, refresh intervals, API keys
- Stream configuration is the source of truth

### 3. Domain-Portable
- Domain adapters use Source/Sink traits (hexagonal architecture)
- New domains require only new adapters, not core changes
- Same learning engine across all domains
- Config structure is generic — domain-specific only in adapter layer

### 4. Resource-Constrained
- Memory budget: ~5.5GB typical of 16GB available
- Storage: ~1.2GB/year growth rate
- CPU: 4 cores, ~40% average utilization target
- ARM64 architecture (aarch64-unknown-linux-gnu)
- No dependencies that fail on ARM64 (e.g., jemalloc on Pi 5 kernel 6.14+)
- No dependencies with excessive memory footprint (e.g., Polars)

### 5. Integration-First
- Extend existing code, do not create parallel systems
- Read before you build — search for existing functionality first
- Add methods to existing traits, don't create new abstractions
- Every new feature must be called from existing code paths
- See `product/INTEGRATION_FIRST_MANDATE.md` for full rules

### 6. Privacy by Architecture
- Local-only is a technical reality, not a policy
- No telemetry, no phone-home, no cloud analytics
- User data never leaves the device by default
- Open source — auditable by anyone

### 7. Self-Learning
- System improves over time from its own observations
- Compounding intelligence — longer runtime = better for YOUR environment
- Drift detection and seasonal adaptation
- Every action outcome feeds back into the learning loop
- N/A for pure infrastructure features (ops, tooling) — note as such

---

## Technical Constraints (hard requirements)

| Constraint | Rule |
|------------|------|
| Architecture | ARM64 (Pi 5) — all dependencies must compile for aarch64 |
| Banned: DuckDB | Eliminated from architecture — use TimescaleDB |
| Banned: Polars | Eliminated — use arrow-rs for Parquet, TimescaleDB for aggregates |
| Database | TimescaleDB for Silver/Gold, Parquet+WAL for Bronze |
| Configuration | Config-driven via JSON in `config/base/streams/` |
| Deployment | Docker on Pi, git as transport, `deploy.sh` as orchestrator |
| Data flow | Bronze (Parquet+WAL) → Silver (TimescaleDB hypertables) → Gold (materialized views) |
| Versioning | v{major}.{minor} defined in `product/features/gold-001/FEATURE-ROADMAPv1.2.md` Use {sequence} for feature release iterations |

---

## Scope Alignment Rules

- **SCOPE.md is authoritative**: The user writes SCOPE.md. Specs must deliver what SCOPE.md asks for — no more, no less.
- **Additions beyond scope**: Flag as "out of scope addition" in alignment report. User decides whether to include.
- **Omissions from scope**: Flag as "scope gap" in alignment report. Must be addressed or explicitly deferred.
- **Simplifications**: Acceptable if documented as "corner cut" with rationale. User approves.

---

## Variance Classification

| Classification | Meaning | Action Required |
|----------------|---------|-----------------|
| **PASS** | Fully aligned with vision and constraints | None |
| **WARN** | Minor deviation, acceptable with documentation | Note in alignment report |
| **VARIANCE** | Significant deviation requiring user approval | Present to user before implementation |
| **FAIL** | Violates a hard constraint or core principle | Must be resolved before implementation |

---

## How to Update This Document

This is a living document. Update it when:
- The product vision evolves
- A version milestone is reached (update roadmap status)
- A new technical constraint is discovered
- A banned dependency is added or removed
- A new alignment principle emerges

All agents and skills that reference this document will immediately use the updated criteria.
