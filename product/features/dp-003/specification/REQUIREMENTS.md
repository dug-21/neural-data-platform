# DP-003: MQTT Multi-Subscription Requirements

## 1. Introduction

### 1.1 Purpose

This document defines the functional and non-functional requirements for refactoring MqttSource to support multiple topic subscriptions per broker connection, enabling config-driven multi-stream ingestion.

### 1.2 Scope

- Refactor `MqttConfig` and `MqttSource` in `neural-core`
- Update config parsing in `air-quality-app`
- Maintain backward compatibility with existing air-quality configuration
- Enable HomeAssistant stream alongside air-quality stream

### 1.3 Definitions

| Term | Definition |
|------|------------|
| **Stream** | A logical data pipeline with unique stream_id (e.g., "air-quality", "homeassistant") |
| **Subscription** | An MQTT topic pattern subscription routed to a specific stream |
| **Topic Pattern** | MQTT wildcard pattern (e.g., "airgradient/readings/+") |
| **Bronze Layer** | Raw data storage in Parquet format with consistent schema |

### 1.4 References

- SCOPE.md: Feature scope definition
- `core/src/sources/mqtt.rs`: Current MqttSource implementation
- `config/base/streams/air-quality/config.yaml`: Current stream config format
- `config/base/streams/homeassistant/config.yaml`: HomeAssistant stream config
- `product/research/homeassistant/mqtt-patterns.md`: MQTT namespace research

---

## 2. Functional Requirements

### 2.1 Multi-Subscription Configuration

| ID | Requirement | Priority | Status |
|----|-------------|----------|--------|
| FR-2.1.1 | MqttConfig SHALL support a `subscriptions` array containing multiple subscription configurations | High | Proposed |
| FR-2.1.2 | Each subscription SHALL specify a `stream_id` for routing | High | Proposed |
| FR-2.1.3 | Each subscription SHALL specify a `topic_pattern` for MQTT subscription | High | Proposed |
| FR-2.1.4 | Each subscription MAY specify a custom parser configuration | Medium | Proposed |
| FR-2.1.5 | System SHALL support backward-compatible single `topic_pattern` configuration | High | Proposed |

#### FR-2.1 Configuration Schema

```yaml
# New multi-subscription format
sources:
  - type: mqtt
    enabled: true
    params:
      broker_url: "mosquitto"
      port: 1883
      client_id: "ndp-mqtt-ingestion"
      qos: 1
      subscriptions:
        - stream_id: "air-quality"
          topic_pattern: "airgradient/readings/+"
          parser:
            parser_type: flat_json
            location_id_field: serialno
            skip_fields: [serialno, firmware, model, ledMode]

        - stream_id: "homeassistant"
          topic_pattern: "homeassistant/+/+/state"
          parser:
            parser_type: flat_json
            location_id_field: entity_id
```

```yaml
# Backward-compatible single topic format (MUST continue to work)
sources:
  - type: mqtt
    enabled: true
    params:
      broker_url: "mosquitto"
      port: 1883
      topic_pattern: "airgradient/readings/+"  # Legacy single-topic
```

### 2.2 Topic Pattern Matching and Routing

| ID | Requirement | Priority | Status |
|----|-------------|----------|--------|
| FR-2.2.1 | System SHALL subscribe to all configured topic patterns on broker connection | High | Proposed |
| FR-2.2.2 | System SHALL match incoming messages against subscription topic patterns | High | Proposed |
| FR-2.2.3 | System SHALL route matched messages to the corresponding stream_id | High | Proposed |
| FR-2.2.4 | System SHALL use first-match routing when patterns overlap | Medium | Proposed |
| FR-2.2.5 | System SHALL log unmatched messages at DEBUG level | Low | Proposed |

#### FR-2.2 Routing Logic

```
Message arrives on topic: "airgradient/readings/abc123"

1. Iterate subscriptions in order:
   - Check "airgradient/readings/+" -> MATCH
   - Route to stream_id: "air-quality"
   - Stop iteration (first-match wins)

2. Send to ingestion channel: (source_id, "air-quality", TimeSeriesPoint)
```

### 2.3 Message Parsing

| ID | Requirement | Priority | Status |
|----|-------------|----------|--------|
| FR-2.3.1 | Each subscription SHALL use its configured parser | High | Proposed |
| FR-2.3.2 | Parser SHALL produce TimeSeriesPoint with consistent schema | High | Proposed |
| FR-2.3.3 | System SHALL add `stream_id` tag to all parsed points | High | Proposed |
| FR-2.3.4 | System SHALL preserve original field names (no field renaming at Bronze) | High | Proposed |
| FR-2.3.5 | Parser errors SHALL NOT crash the subscription | High | Proposed |

#### FR-2.3 Output Schema (Bronze Layer)

All MQTT subscriptions MUST produce data conforming to this Parquet schema:

| Column | Type | Description |
|--------|------|-------------|
| timestamp | TIMESTAMP | Message arrival time (UTC) |
| location_id | STRING | Device/entity identifier from payload |
| metric | STRING | Metric name from JSON key |
| value | FLOAT64 | Metric value |
| stream_id | STRING | Target stream identifier (tag) |
| source | STRING | Source type (tag, always "mqtt") |

### 2.4 Connection Management

| ID | Requirement | Priority | Status |
|----|-------------|----------|--------|
| FR-2.4.1 | System SHALL maintain a SINGLE connection per broker | High | Proposed |
| FR-2.4.2 | System SHALL re-subscribe to all topics after reconnection | High | Proposed |
| FR-2.4.3 | System SHALL use exponential backoff for reconnection | High | Existing |
| FR-2.4.4 | System SHALL support QoS levels 0, 1, and 2 | Medium | Existing |
| FR-2.4.5 | Connection failures SHALL NOT affect other sources | High | Existing |

### 2.5 Configuration Hot-Reload

| ID | Requirement | Priority | Status |
|----|-------------|----------|--------|
| FR-2.5.1 | System SHALL detect subscription configuration changes via etcd watch | Medium | Proposed |
| FR-2.5.2 | System SHALL add new subscriptions without reconnecting | Medium | Proposed |
| FR-2.5.3 | System SHALL remove deleted subscriptions without reconnecting | Medium | Proposed |
| FR-2.5.4 | System SHALL log subscription changes at INFO level | Low | Proposed |

---

## 3. Non-Functional Requirements

### 3.1 Performance

| ID | Requirement | Metric | Target |
|----|-------------|--------|--------|
| NFR-3.1.1 | Message processing latency | p95 end-to-end | < 100ms |
| NFR-3.1.2 | Message throughput | Messages/second | >= 1000 |
| NFR-3.1.3 | Memory overhead per subscription | MB | < 10 |
| NFR-3.1.4 | CPU overhead per subscription | % of core | < 5% |

### 3.2 Reliability

| ID | Requirement | Description |
|----|-------------|-------------|
| NFR-3.2.1 | No message loss during normal operation | QoS 1 guarantees delivery |
| NFR-3.2.2 | Graceful degradation on parse errors | Skip malformed messages, continue processing |
| NFR-3.2.3 | Automatic recovery from broker disconnection | Reconnect with backoff, re-subscribe |
| NFR-3.2.4 | No data corruption on concurrent writes | Channel-based serialization |

### 3.3 Maintainability

| ID | Requirement | Description |
|----|-------------|-------------|
| NFR-3.3.1 | Configuration via YAML | No code changes for new subscriptions |
| NFR-3.3.2 | Clear error messages | Include stream_id, topic, and error details |
| NFR-3.3.3 | Structured logging | Use tracing with span context |
| NFR-3.3.4 | Test coverage | >= 80% for new code |

### 3.4 Compatibility

| ID | Requirement | Description |
|----|-------------|-------------|
| NFR-3.4.1 | Backward compatible config | Single topic_pattern format continues to work |
| NFR-3.4.2 | Existing tests pass | No regression in air-quality functionality |
| NFR-3.4.3 | API stability | Source trait implementation unchanged |
| NFR-3.4.4 | Wire protocol compatibility | Standard MQTT 3.1.1 |

### 3.5 Observability

| ID | Requirement | Description |
|----|-------------|-------------|
| NFR-3.5.1 | Health check per subscription | Report individual subscription status |
| NFR-3.5.2 | Metrics per subscription | message_count, error_count, latency_ms |
| NFR-3.5.3 | Connection state logging | Connect, disconnect, subscribe events |
| NFR-3.5.4 | Structured logging with stream_id | All logs include stream context |

---

## 4. Constraints

### 4.1 Technical Constraints

| ID | Constraint | Rationale |
|----|------------|-----------|
| TC-4.1.1 | Single broker per MqttSource instance | Simplifies connection management |
| TC-4.1.2 | rumqttc async client library | Already in use, well-tested |
| TC-4.1.3 | Parquet output schema fixed | Bronze layer consistency requirement |
| TC-4.1.4 | Channel-based routing | Must flow through IngestionCoordinator |

### 4.2 Business Constraints

| ID | Constraint | Rationale |
|----|------------|-----------|
| BC-4.2.1 | No service downtime for deployment | Use rolling updates |
| BC-4.2.2 | Config changes without code deployment | GitOps workflow via etcd |
| BC-4.2.3 | Existing air-quality dashboards unchanged | Data format compatibility |

---

## 5. Dependencies

### 5.1 Internal Dependencies

| Component | Version | Usage |
|-----------|---------|-------|
| core | Current | MqttSource, MqttConfig, Source trait |
| config-client | Current | etcd configuration loading |
| air-quality-app | Current | SourceManager, IngestionCoordinator |

### 5.2 External Dependencies

| Component | Version | Usage |
|-----------|---------|-------|
| rumqttc | 0.24.x | Async MQTT client |
| tokio | 1.x | Async runtime |
| serde_json | 1.x | JSON parsing |
| Mosquitto | 2.x | MQTT broker |

---

## 6. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Topic pattern collision | Medium | High | First-match routing, configuration validation |
| Memory pressure with many subscriptions | Low | Medium | Shared connection, lazy buffer allocation |
| Parser incompatibility between streams | Medium | Medium | Per-subscription parser configuration |
| Hot-reload complexity | Medium | Low | Defer to Phase 2, restart-based for MVP |

---

## 7. Open Questions

| ID | Question | Status |
|----|----------|--------|
| OQ-1 | Should we support regex topic patterns? | Decided: No, MQTT wildcards only |
| OQ-2 | How to handle overlapping topic patterns? | Decided: First-match wins |
| OQ-3 | Should hot-reload be MVP or Phase 2? | Pending |
| OQ-4 | Per-subscription vs shared parser config? | Decided: Per-subscription |

---

## 8. Approval

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Product Owner | | | |
| Technical Lead | | | |
| Architect | | | |
