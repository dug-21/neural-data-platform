# DP-003: Implementation Plan - MQTT Multi-Subscription Support

## Overview

This document outlines the step-by-step TDD implementation plan for adding multi-subscription support to the MQTT source. Implementation follows the **London School TDD** approach: write failing tests first, implement minimal code to pass, then refactor.

**Total Estimated Effort**: ~3-4 days
**Test Coverage Target**: 95% for MqttConfig, 90% for TopicRouter

---

## Implementation Phases

### Phase 1: SubscriptionConfig Struct + Unit Tests

**Duration**: ~2-3 hours | **Complexity**: S (Small)

**Goal**: Define the `SubscriptionConfig` struct with serde deserialization and validation.

#### Tasks

1. **[S]** Create `core/src/sources/mqtt/subscription.rs` module
2. **[S]** Define `SubscriptionConfig` struct with fields:
   - `stream_id: String`
   - `topic_pattern: String`
   - `parser: Option<ParserConfig>`
   - `enabled: bool` (default: true)
3. **[S]** Implement `Default` trait
4. **[S]** Write unit tests for:
   - Struct creation with all fields
   - Default values (enabled = true)
   - Serde deserialization from YAML
   - Validation (empty stream_id, empty topic_pattern)

#### Acceptance Criteria

- [ ] `SubscriptionConfig` struct compiles with all fields
- [ ] `#[serde(default)]` works for `enabled` field
- [ ] 5+ unit tests passing
- [ ] `cargo clippy` clean

#### Dependencies

- None (new code)

---

### Phase 2: TopicRouter + Unit Tests

**Duration**: ~4-5 hours | **Complexity**: M (Medium)

**Goal**: Implement topic pattern matching with MQTT wildcard to regex conversion.

#### Tasks

1. **[S]** Create `core/src/sources/mqtt/router.rs` module
2. **[M]** Implement `mqtt_pattern_to_regex()` function:
   - Convert `+` to `[^/]+`
   - Convert `#` to `.*`
   - Escape special regex characters
   - Validate `#` is at end of pattern
3. **[S]** Define `RouteEntry` struct:
   - `pattern: String`
   - `regex: Regex`
   - `stream_id: String`
   - `parser_config: Option<ParserConfig>`
   - `enabled: bool`
4. **[M]** Implement `TopicRouter` struct:
   - `new(subscriptions: Vec<SubscriptionConfig>) -> Result<Self>`
   - `route(topic: &str) -> Option<&RouteEntry>`
   - `topic_patterns() -> Vec<&str>`
5. **[S]** Write comprehensive unit tests (see TDD_SEQUENCE.md)

#### Acceptance Criteria

- [ ] MQTT patterns correctly converted to regex
- [ ] Single-level wildcard (+) matches correctly
- [ ] Multi-level wildcard (#) matches correctly
- [ ] First-match wins for overlapping patterns
- [ ] Invalid patterns rejected with clear errors
- [ ] 15+ unit tests passing
- [ ] `cargo clippy` clean

#### Dependencies

- Phase 1 complete (SubscriptionConfig struct)
- `regex` crate (already in Cargo.toml)

---

### Phase 3: MqttConfig Refactor + Backward Compatibility Tests

**Duration**: ~3-4 hours | **Complexity**: M (Medium)

**Goal**: Add `subscriptions` field to `MqttConfig` while maintaining backward compatibility with `topic_pattern`.

#### Tasks

1. **[S]** Add `subscriptions: Vec<SubscriptionConfig>` field to `MqttConfig`
2. **[S]** Make `topic_pattern` optional: `Option<String>`
3. **[M]** Implement `get_subscriptions()` method:
   - Return `subscriptions` if not empty
   - Convert legacy `topic_pattern` to subscription if present
   - Log deprecation warning for legacy format
   - Merge without duplicates
4. **[S]** Implement `validate()` method:
   - At least one subscription required
   - Stream IDs must be unique
   - Topic patterns must be valid
5. **[M]** Write backward compatibility tests:
   - New format with subscriptions array works
   - Legacy format with topic_pattern works
   - Mixed format handled correctly
   - Deprecation warning logged

#### Acceptance Criteria

- [ ] Existing configs with `topic_pattern` continue to work
- [ ] New configs with `subscriptions` array work
- [ ] Deprecation warning logged for legacy format
- [ ] `get_subscriptions()` returns correct list
- [ ] Validation catches config errors
- [ ] 12+ unit tests passing
- [ ] All existing MQTT tests still pass

#### Dependencies

- Phase 1 complete (SubscriptionConfig)
- Phase 2 complete (TopicRouter for validation)

---

### Phase 4: MqttSource Integration + Integration Tests

**Duration**: ~4-6 hours | **Complexity**: L (Large)

**Goal**: Integrate TopicRouter into MqttSource for multi-subscription message routing.

#### Tasks

1. **[M]** Add `TopicRouter` field to `MqttSource`
2. **[M]** Update `MqttSource::new()`:
   - Build TopicRouter from config subscriptions
   - Validate configuration at construction time
3. **[M]** Update subscription logic:
   - Subscribe to all patterns from `router.topic_patterns()`
   - Log subscription count
4. **[L]** Update message processing:
   - Route incoming topic through TopicRouter
   - Use matched parser for payload parsing
   - Tag points with stream_id
   - Handle unmatched topics (dead letter)
5. **[M]** Update reconnection logic:
   - Resubscribe to all patterns on reconnect
6. **[M]** Write integration tests:
   - Multi-subscription routing
   - Parser per subscription
   - Dead letter handling
   - Reconnection resubscribes all

#### Acceptance Criteria

- [ ] Multiple subscriptions processed correctly
- [ ] Each subscription uses correct parser
- [ ] Points tagged with correct stream_id
- [ ] Unmatched topics handled gracefully
- [ ] Reconnection restores all subscriptions
- [ ] 8-10 integration tests passing
- [ ] All existing MQTT tests still pass

#### Dependencies

- Phases 1-3 complete
- Mock MQTT broker for integration tests (optional, can use unit tests)

---

### Phase 5: Config Parsing in air-quality-app

**Duration**: ~2-3 hours | **Complexity**: S (Small)

**Goal**: Update application config loading to support new subscription format.

#### Tasks

1. **[S]** Update YAML config structure in `config/base/streams/air-quality/config.yaml`
2. **[S]** Verify config parsing in `apps/air-quality-app/src/main.rs`
3. **[S]** Add shared MQTT config file (optional): `config/base/mqtt-sources.yaml`
4. **[S]** Write config validation tests
5. **[S]** Test with both old and new config formats

#### Acceptance Criteria

- [ ] New YAML format parses correctly
- [ ] Old YAML format still works
- [ ] Config validation errors clear and actionable
- [ ] Application starts with new config
- [ ] `cargo test` passes

#### Dependencies

- Phases 1-4 complete

---

## Implementation Order Summary

```
Phase 1: SubscriptionConfig (Foundation)
    |
    v
Phase 2: TopicRouter (Pattern Matching)
    |
    v
Phase 3: MqttConfig Refactor (Config Layer)
    |
    v
Phase 4: MqttSource Integration (Runtime)
    |
    v
Phase 5: Config Parsing (Application)
```

---

## Risk Mitigation

### Risk 1: Breaking Existing MQTT Functionality

**Mitigation**:
- Run existing MQTT tests after each phase
- Maintain backward compatibility throughout
- Feature flag for gradual rollout (if needed)

### Risk 2: Complex Regex Edge Cases

**Mitigation**:
- Comprehensive test cases from pseudocode
- Use well-tested `regex` crate
- Validate patterns at config load time

### Risk 3: Parser Configuration Complexity

**Mitigation**:
- Optional parser in SubscriptionConfig
- Fall back to default parser if not specified
- Reuse existing ParserConfig infrastructure

---

## Testing Strategy Summary

| Phase | Unit Tests | Integration Tests | Total |
|-------|------------|-------------------|-------|
| 1     | 5-7        | 0                 | 5-7   |
| 2     | 15-20      | 0                 | 15-20 |
| 3     | 12-15      | 0                 | 12-15 |
| 4     | 5-10       | 8-10              | 13-20 |
| 5     | 3-5        | 2-3               | 5-8   |
| **Total** | **40-57** | **10-13**      | **50-70** |

---

## Definition of Done

For each phase:

1. [ ] All tests passing (`cargo test`)
2. [ ] No clippy warnings (`cargo clippy`)
3. [ ] Code formatted (`cargo fmt --check`)
4. [ ] Existing tests not broken
5. [ ] Documentation updated (rustdoc comments)

For feature completion:

1. [ ] All 5 phases complete
2. [ ] 50+ tests total
3. [ ] Test coverage >= 90%
4. [ ] STATUS.md updated
5. [ ] Patterns saved to AgentDB

---

## Related Documents

- FILE_CHANGES.md: Detailed file modifications
- TDD_SEQUENCE.md: Red-Green-Refactor sequence
- TOPIC_ROUTER.md: Algorithm pseudocode
- CONFIG_PARSER.md: Config parsing pseudocode
- ADR-002-CONFIG-FORMAT.md: Configuration decisions
