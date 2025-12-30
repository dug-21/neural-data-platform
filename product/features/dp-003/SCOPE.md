# DP-003: MQTT Multi-Subscription Support

## Overview

Refactor MqttSource to support multiple topic subscriptions per broker connection, enabling config-driven multi-stream ingestion through a single MQTT broker.

## Problem Statement

Currently, MqttSource supports only ONE topic pattern per source. This prevents:
- HomeAssistant stream from working alongside air-quality stream
- Config-driven addition of new MQTT-based streams
- Consistent architecture with HTTP polling (which supports multiple endpoints)

## Goals

1. **Single broker, multiple subscriptions** - One MQTT connection to Mosquitto with multiple topic patterns
2. **Config-driven** - Add new streams via YAML config, not code changes
3. **Consistent Bronze format** - All subscriptions produce identical Parquet schema
4. **Stream routing** - Route messages to correct stream based on topic pattern match

## Non-Goals

- Multiple broker connections (defer to future feature)
- Context enrichment at Bronze layer (handled in Silver)
- Complex topic-based parsing (use FlatJsonParser for all)

## Proposed Config Format

```yaml
sources:
  - type: mqtt
    broker_url: "mosquitto"
    port: 1883
    subscriptions:
      - stream_id: air-quality
        topic_pattern: "airgradient/readings/+"

      - stream_id: homeassistant
        topic_pattern: "homeassistant/+/+/state"
```

## Success Criteria

1. HomeAssistant stream loads without errors
2. Both air-quality and homeassistant data written to separate Parquet partitions
3. No changes to existing air-quality functionality
4. Config changes don't require code changes or rebuilds

## Technical Scope

### Must Change
- `MqttConfig` struct - add `subscriptions: Vec<SubscriptionConfig>`
- `MqttSource` - subscribe to multiple topics, route by pattern
- Config parsing in air-quality-app
- Stream config YAML format

### Must NOT Change
- Parquet schema (timestamp, location_id, metric, value)
- FlatJsonParser behavior
- ParquetStore implementation
- HTTP polling sources

## References

- DP-002: Data Dictionary (HomeAssistant stream config created)
- Investigation: MQTT vs HTTP polling architecture comparison
