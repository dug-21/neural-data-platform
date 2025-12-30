# DP-003: MQTT Multi-Subscription Support

## Status: PLANNING COMPLETE - Ready for Implementation

**Last Updated**: 2025-12-30
**Phase**: Ready for Refinement (TDD Implementation)

---

## SPARC Progress

| Phase | Status | Deliverables |
|-------|--------|--------------|
| **Scope** | ✅ Complete | SCOPE.md |
| **Specification** | ✅ Complete | REQUIREMENTS.md, ACCEPTANCE_CRITERIA.md, USER_STORIES.md, TEST_STRATEGY.md, TEST_CASES.md |
| **Architecture** | ✅ Complete | ADR-001, ADR-002, ADR-003, SYSTEM_DESIGN.md |
| **Pseudocode** | ✅ Complete | TOPIC_ROUTER.md, CONFIG_PARSER.md, MESSAGE_PROCESSOR.md, CONNECTION_MANAGER.md |
| **Refinement** | ⏳ Pending | TDD Implementation |
| **Completion** | ⏳ Pending | Integration Testing, Deployment |

---

## Planning Summary

**Total Planning Documents**: 13 deliverables across 4 phases

### Specification Phase (5 docs)
| Document | Description |
|----------|-------------|
| `specification/REQUIREMENTS.md` | 20 functional + 16 non-functional requirements |
| `specification/ACCEPTANCE_CRITERIA.md` | 23 testable Gherkin scenarios |
| `specification/USER_STORIES.md` | 10 stories, 30 story points |
| `specification/TEST_STRATEGY.md` | Test pyramid, coverage targets, CI/CD |
| `specification/TEST_CASES.md` | 47 test cases mapped to acceptance criteria |

### Architecture Phase (4 docs)
| Document | Description |
|----------|-------------|
| `architecture/ADR-001-MQTT-SUBSCRIPTIONS.md` | Subscription array vs multiple sources |
| `architecture/ADR-002-CONFIG-FORMAT.md` | New MqttConfig structure |
| `architecture/ADR-003-TOPIC-ROUTING.md` | Topic→stream routing algorithm |
| `architecture/SYSTEM_DESIGN.md` | Component diagram and data flow |

### Pseudocode Phase (4 docs)
| Document | Description |
|----------|-------------|
| `pseudocode/TOPIC_ROUTER.md` | MQTT wildcard → regex conversion, routing |
| `pseudocode/CONFIG_PARSER.md` | Config parsing with backward compatibility |
| `pseudocode/MESSAGE_PROCESSOR.md` | Message pipeline: route → parse → store |
| `pseudocode/CONNECTION_MANAGER.md` | Connection, subscription, reconnection |

---

## Key Architecture Decisions

1. **Single Source with Subscription Array** (Option B selected)
   - Mirrors HTTP polling pattern (`GenericHttpPollingConfig.endpoints`)
   - Single broker connection for resource efficiency
   - Config-driven stream addition

2. **New Config Structure**
   ```rust
   pub struct MqttConfig {
       pub subscriptions: Vec<SubscriptionConfig>,  // NEW
       pub topic_pattern: Option<String>,           // DEPRECATED
   }

   pub struct SubscriptionConfig {
       pub stream_id: String,
       pub topic_pattern: String,
       pub parser: Option<ParserConfig>,
       pub enabled: bool,
   }
   ```

3. **Topic Routing**
   - MQTT patterns (+, #) converted to regex at config load
   - First-match wins for overlapping patterns
   - Dead letter queue for unmatched messages

4. **Backward Compatibility**
   - Legacy `topic_pattern` field still supported
   - Automatic conversion to subscription entry
   - Deprecation warning logged

---

## Patterns Saved to AgentDB

| Pattern | Description |
|---------|-------------|
| `specification:mqtt-multi-subscription-config` | Config format decisions |
| `architecture:mqtt-multi-subscription` | Architecture decision (Option B) |
| `mqtt-topic-routing-algorithm` | Topic matching algorithm |
| `mqtt-multi-subscription-test-strategy` | Test approach and coverage |

---

## Implementation Scope

### Files to Create
- `core/src/sources/mqtt/subscription.rs` - SubscriptionConfig struct
- `core/src/sources/mqtt/router.rs` - TopicRouter implementation

### Files to Modify
- `core/src/sources/mqtt.rs` - MqttConfig, MqttSource refactor
- `apps/air-quality-app/src/main.rs` - Config parsing updates
- `config/base/streams/*/config.yaml` - New subscription format

### Test Coverage Targets
- Unit tests: 40-50 tests (MqttConfig: 95%, TopicRouter: 90%)
- Integration tests: 8-10 tests (full message flow)
- E2E tests: 2-3 tests (HomeAssistant + air-quality)

---

## Next Steps

1. **Refinement Phase**: TDD implementation starting with TopicRouter
2. **Integration Testing**: Test with real Mosquitto broker
3. **Completion Phase**: Enable HomeAssistant stream, verify dashboards

---

## Notes

- Feature initiated from DP-002 homeassistant config loading failure
- Root cause: MqttSource only supports single topic pattern
- Solution: Refactor to support multiple subscriptions per broker
- Reference implementation: `core/src/sources/http_poll.rs` (GenericHttpPollingConfig)
