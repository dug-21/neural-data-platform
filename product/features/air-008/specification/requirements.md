# AIR-008: Home Events - Requirements

## Overview

This specification defines the requirements for integrating home event data (particularly window state) into the Neural Data Platform, with a focus on data architecture decisions that support both immediate needs and future extensibility.

## Functional Requirements

### FR-1: Home Event Data Integration

**FR-1.1**: The system SHALL support ingestion of window open/close events from Home Assistant.

**FR-1.2**: The system SHALL capture event context including:
- Timestamp of state change
- Window identifier (which window)
- New state (open/closed)
- Previous state (if available)
- Duration in previous state (derived)

**FR-1.3**: The system SHALL support manual event entry for cases where automatic detection is unavailable.

### FR-2: Data Architecture

**FR-2.1**: The data model SHALL support both:
- Event-based storage (discrete state changes)
- State derivation (current state at any point in time)

**FR-2.2**: The architecture SHALL be extensible to support:
- Additional home automation events (doors, HVAC, occupancy)
- System log streams (non-sensor data)
- Generic event streams with varying schemas

**FR-2.3**: The data model SHALL maintain relationship to existing air quality streams for correlation analysis.

### FR-3: Data Access Interface

**FR-3.1**: The system SHALL define a specification for how external interfaces write event data.

**FR-3.2**: The interface specification SHALL support:
- Single event submission
- Batch event submission
- Event correction/amendment

**FR-3.3**: The interface SHALL validate incoming events against schema.

## Non-Functional Requirements

### NFR-1: Performance

**NFR-1.1**: Event ingestion latency SHALL be less than 1 second from source to storage.

**NFR-1.2**: State derivation queries SHALL complete within 100ms for single-window lookups.

### NFR-2: Data Integrity

**NFR-2.1**: Events SHALL be immutable once stored (corrections via amendment events).

**NFR-2.2**: The system SHALL maintain event ordering guarantees within a single source.

### NFR-3: Extensibility

**NFR-3.1**: Adding a new event type SHALL NOT require schema migrations to core tables.

**NFR-3.2**: The architecture SHALL support future neural prediction workloads (window open/close optimization).

## Research Requirements

### RR-1: Platform Analysis

**RR-1.1**: Research SHALL evaluate Home Assistant's data architecture approach.

**RR-1.2**: Research SHALL include broader time-series and event-sourcing patterns:
- Apache Kafka event streaming patterns
- TimescaleDB event storage patterns
- InfluxDB vs TimescaleDB for mixed workloads
- Event sourcing vs state storage trade-offs

**RR-1.3**: Research SHALL document recommendations in `product/research/dp-analysis/`.

### RR-2: Integration Options

**RR-2.1**: Evaluate Home Assistant integration methods:
- REST API polling
- WebSocket subscription
- MQTT broker integration
- Direct database access (if applicable)

## Constraints

### C-1: Platform Constraints

- Must integrate with existing NDP Domain Adapter architecture
- Must use Rust for core implementation
- Storage must align with Bronze/Silver/Gold layer strategy

### C-2: Scope Exclusions

- UI/Interface design (specification of data format only)
- Home Assistant configuration/setup
- Real-time alerting based on events (future feature)

## Definitions

| Term | Definition |
|------|------------|
| Event | A discrete occurrence with timestamp and context |
| State | The current value of an attribute at a point in time |
| State Change | Transition from one state to another (generates event) |
| Event Sourcing | Pattern where state is derived from sequence of events |
| Home Assistant | Open-source home automation platform |

## References

- Home Assistant Data: https://data.home-assistant.io
- Existing air quality streams: AIR-001 through AIR-007
- NDP Architecture: `/docs/architecture/`
