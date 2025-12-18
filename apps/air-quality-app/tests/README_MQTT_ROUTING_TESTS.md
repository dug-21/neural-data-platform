# MQTT Routing Tests - Regression Prevention

## Problem Statement

Previously, MQTT sources bypassed the IngestionRouter and wrote data directly to ParquetStore using the device MAC address as the partition key. This caused:
- Data stored in `/data/d83bda1cd074/` instead of `/data/air-quality/`
- Inconsistency with HTTP sources which correctly used stream names
- Difficulty in data discovery and querying

## Solution

MQTT sources now route through the IngestionRouter like HTTP sources:
1. MQTT source sends `(source_id, stream_id, point)` tuples to ingestion channel
2. Router enriches points with `stream_id` and `source_id` tags
3. ParquetStore uses `stream_id` tag (if present) for partition path

## Test Coverage

### Unit Tests

#### `source_manager.rs` Tests
- `test_spawn_mqtt_source_sends_to_ingestion_channel()` - Verifies MQTT uses ingestion channel
- `test_mqtt_config_parsing_from_source_params()` - Verifies MQTT config extraction
- `test_mqtt_source_uses_stream_id_not_device_id()` - Critical regression test for stream_id usage

#### `router.rs` Tests
- `test_router_adds_stream_id_tag_to_points()` - Verifies stream_id enrichment
- `test_router_adds_source_id_tag_to_points()` - Verifies source_id enrichment
- `test_router_preserves_existing_tags()` - Verifies tag preservation

#### `parquet.rs` Tests
- `test_partition_key_uses_stream_id_over_location_id()` - Critical regression test
- `test_partition_key_falls_back_to_location_id()` - Backward compatibility
- `test_mqtt_points_written_to_stream_directory()` - End-to-end directory test
- `test_get_partition_key_function()` - Unit test for partition key logic

### Integration Tests

#### `mqtt_routing_integration_test.rs`
- `test_mqtt_and_http_sources_both_route_through_ingestion_channel()` - Contract test
- `test_all_sources_get_stream_id_enrichment()` - Consistency test
- Helper functions for MQTT point simulation
- Partition key logic verification

## Running Tests

```bash
# Run all unit tests
cd /workspaces/neural-data-platform
cargo test --package air-quality-app

# Run specific test module
cargo test --package air-quality-app coordinator::source_manager::tests::test_mqtt

# Run integration tests (requires etcd)
cargo test --package air-quality-app --test mqtt_routing_integration_test

# Run with output
cargo test --package air-quality-app -- --nocapture

# Run specific regression test
cargo test test_partition_key_uses_stream_id_over_location_id -- --exact
```

## Critical Regression Tests

These tests MUST pass to prevent the bug from reoccurring:

1. **`test_mqtt_source_uses_stream_id_not_device_id`** - Ensures MQTT uses stream name
2. **`test_partition_key_uses_stream_id_over_location_id`** - Ensures correct directory
3. **`test_router_adds_stream_id_tag_to_points`** - Ensures enrichment happens

## Test Philosophy (London School TDD)

- **Behavior verification** over state inspection
- **Mock-driven** development for isolation
- **Contract testing** for component integration
- **Regression prevention** through critical path coverage

## Related Documentation

- `docs/testing/AIR-005-TEST-DESIGN.md` - Overall test strategy
- `docs/testing/AIR-005-TEST-SUMMARY.md` - Test summary
- `CLAUDE.md` - NDP testing patterns

## Future Enhancements

1. Add property-based tests for partition key logic
2. Add chaos tests for channel failures
3. Add benchmarks for routing performance
4. Mock StreamRegistry for faster unit tests
