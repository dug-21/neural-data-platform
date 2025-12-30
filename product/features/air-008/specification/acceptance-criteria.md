# AIR-008: Home Events - Acceptance Criteria

## Overview

This document defines the acceptance criteria for the AIR-008 feature. Each criterion must be verified before the feature can be considered complete.

## Phase 1: Research & Architecture Decision

### AC-1: Data Architecture Research Complete

**Given** the need for event-based data integration
**When** research is complete
**Then**:
- [ ] Research document exists at `product/research/dp-analysis/event-architecture.md`
- [ ] Home Assistant data approach is documented with pros/cons
- [ ] At least 3 alternative approaches are evaluated
- [ ] Clear recommendation is provided with rationale
- [ ] Future extensibility to log streams is addressed

### AC-2: Architecture Decision Recorded

**Given** research is complete
**When** architecture decision is made
**Then**:
- [ ] ADR created in `/docs/architecture/decisions/`
- [ ] Decision addresses event-based vs state-change trade-offs
- [ ] Decision considers existing Domain Adapter pattern
- [ ] Decision aligns with Bronze/Silver/Gold layer strategy

## Phase 2: Data Model Specification

### AC-3: Event Schema Defined

**Given** architecture decision is made
**When** schema is designed
**Then**:
- [ ] Event schema supports window state changes
- [ ] Schema is extensible for additional event types
- [ ] Schema includes required context fields (timestamp, source, type)
- [ ] Schema validation rules are documented

### AC-4: State Derivation Strategy

**Given** event schema is defined
**When** state derivation approach is specified
**Then**:
- [ ] Algorithm for deriving current state from events is documented
- [ ] Point-in-time state query approach is defined
- [ ] Performance characteristics are estimated
- [ ] Edge cases are addressed (missing events, out-of-order)

## Phase 3: Interface Specification

### AC-5: Data Write Interface Specified

**Given** data model is complete
**When** interface specification is written
**Then**:
- [ ] API contract for event submission is documented
- [ ] Request/response formats are defined (JSON schema)
- [ ] Error handling is specified
- [ ] Validation rules are documented
- [ ] Batch submission is supported

### AC-6: Home Assistant Integration Path Documented

**Given** interface specification is complete
**When** integration options are evaluated
**Then**:
- [ ] Recommended integration method is documented
- [ ] Data mapping from HA to NDP schema is defined
- [ ] Polling/subscription frequency is specified
- [ ] Error recovery approach is documented

## Phase 4: Implementation Readiness

### AC-7: Stream Configuration Ready

**Given** all specifications are complete
**When** implementation is ready to begin
**Then**:
- [ ] Stream configuration template exists
- [ ] Source adapter type is defined (new or existing)
- [ ] Storage adapter requirements are documented
- [ ] Test data scenarios are defined

### AC-8: Correlation with Air Quality Defined

**Given** event data model is complete
**When** correlation approach is specified
**Then**:
- [ ] Join strategy between events and time-series is documented
- [ ] Query patterns for "air quality when windows open" are defined
- [ ] Aggregation approach is specified (if needed)

## Verification Checklist

### Documentation
- [ ] All research artifacts in `product/research/dp-analysis/`
- [ ] ADR(s) in `/docs/architecture/decisions/`
- [ ] SPARC phases completed in feature directory
- [ ] STATUS.md kept current throughout

### Technical
- [ ] Schema is compatible with TimescaleDB (future Silver layer)
- [ ] Schema is compatible with Parquet (Bronze layer)
- [ ] No breaking changes to existing streams

### Process
- [ ] All acceptance criteria verified
- [ ] Stakeholder review completed
- [ ] Ready for implementation phase (separate feature)

## Out of Scope Verification

The following are explicitly NOT acceptance criteria for AIR-008:
- Working Home Assistant integration code
- UI for event entry
- Deployed and running event collection
- Neural prediction model

These will be addressed in subsequent features.
