# DP-004: Bronze Raw JSON Schema - Acceptance Criteria

## Overview

This document defines the acceptance criteria for dp-004 in Given/When/Then (Gherkin) format. Each criterion maps to functional requirements and can be validated through automated tests.

---

## AC-1: RawDataPoint Struct Creation

### AC-1.1: Basic Structure

```gherkin
Feature: RawDataPoint data structure

  Scenario: Create RawDataPoint with all fields
    Given a timestamp of "2026-01-01T12:00:00Z"
    And a source_id of "air-quality-Mqtt"
    And an ndp_id of "airgradient-office-001"
    And a context of {"room": "office", "floor": 2}
    And a raw_payload of {"pm02": 12.5, "rco2": 450, "serialno": "abc123"}
    When I create a RawDataPoint
    Then the RawDataPoint should be successfully created
    And the timestamp should equal "2026-01-01T12:00:00Z"
    And the source_id should equal "air-quality-Mqtt"
    And the ndp_id should equal Some("airgradient-office-001")
    And the context should contain "room" = "office"
    And the raw_payload should contain "pm02" = 12.5

  Scenario: Create RawDataPoint with minimal fields
    Given a timestamp of "2026-01-01T12:00:00Z"
    And a source_id of "simple-sensor"
    And no ndp_id
    And no context
    And a raw_payload of {"value": 42}
    When I create a RawDataPoint
    Then the RawDataPoint should be successfully created
    And the ndp_id should be None
    And the context should be None
    And the raw_payload should contain "value" = 42
```

### AC-1.2: Serialization

```gherkin
Feature: RawDataPoint serialization

  Scenario: Serialize RawDataPoint to JSON
    Given a valid RawDataPoint with all fields populated
    When I serialize it to JSON using serde_json
    Then the serialization should succeed
    And the JSON should contain all five fields

  Scenario: Deserialize RawDataPoint from JSON
    Given a JSON string representing a valid RawDataPoint
    When I deserialize it using serde_json
    Then the deserialization should succeed
    And all fields should match the original values

  Scenario: Clone RawDataPoint
    Given a valid RawDataPoint
    When I clone it
    Then the cloned point should equal the original
    And modifying the clone should not affect the original
```

---

## AC-2: Parquet Storage Schema

### AC-2.1: Write Operations

```gherkin
Feature: Parquet storage for RawDataPoint

  Scenario: Write single RawDataPoint to Parquet
    Given a ParquetStore initialized with a valid path
    And a RawDataPoint with source_id "air-quality-Mqtt"
    When I call write_raw(point)
    Then the operation should succeed
    And a Parquet file should exist at the expected partition path
    And the file should contain exactly 5 columns

  Scenario: Write batch of RawDataPoints to Parquet
    Given a ParquetStore initialized with a valid path
    And 100 RawDataPoints with source_id "air-quality-Mqtt"
    When I call write_raw_batch(points)
    Then the operation should succeed
    And the Parquet file should contain 100 rows
    And all rows should have valid raw_payload JSON

  Scenario: Partition path uses source_id
    Given a RawDataPoint with source_id "weather-sensor"
    And timestamp "2026-01-15T10:30:00Z"
    When I write it to ParquetStore
    Then the file should be located at:
      | base_path/data/weather-sensor/year=2026/month=01/day=15/readings.parquet |
```

### AC-2.2: Schema Validation

```gherkin
Feature: Parquet schema compliance

  Scenario: Verify column types
    Given a Parquet file written by the new storage layer
    When I inspect the schema
    Then the "timestamp" column should be Int64 (microseconds)
    And the "source_id" column should be Utf8 (non-nullable)
    And the "ndp_id" column should be Utf8 (nullable)
    And the "context" column should be Utf8 (nullable)
    And the "raw_payload" column should be Utf8 (non-nullable)

  Scenario: Verify compression
    Given a Parquet file written by the new storage layer
    When I inspect the file metadata
    Then the compression codec should be Snappy
```

---

## AC-3: Source Implementation

### AC-3.1: HttpPollingSource

```gherkin
Feature: HTTP polling source emits RawDataPoint

  Scenario: Poll endpoint and emit RawDataPoint
    Given an HttpPollingSource configured for endpoint "http://sensor.local/data"
    And the endpoint returns {"pm02": 15.5, "temp": 22.0}
    And source_id is configured as "air-quality-Http"
    And ndp_id is configured as "sensor-office-001"
    And context is configured as {"room": "office"}
    When the source polls the endpoint
    Then the source should emit a RawDataPoint
    And the raw_payload should exactly match {"pm02": 15.5, "temp": 22.0}
    And the source_id should equal "air-quality-Http"
    And the ndp_id should equal "sensor-office-001"
    And the context should equal {"room": "office"}
    And the timestamp should be the ingestion time (not from payload)

  Scenario: Preserve nested JSON structures
    Given an HttpPollingSource configured for OpenWeatherMap
    And the API returns {"main": {"temp": 295.15}, "wind": {"speed": 3.5}}
    When the source polls the endpoint
    Then the raw_payload should preserve the nested structure exactly
    And raw_payload["main"]["temp"] should equal 295.15
    And raw_payload["wind"]["speed"] should equal 3.5
```

### AC-3.2: Source Resilience

```gherkin
Feature: Source resilience to format changes

  Scenario: Handle new fields in source response
    Given an HttpPollingSource expecting {"pm02": number, "temp": number}
    And the source now returns {"pm02": 15.5, "temp": 22.0, "new_field": "value"}
    When the source polls the endpoint
    Then the ingestion should succeed
    And raw_payload should contain "new_field" = "value"
    And no parsing error should occur

  Scenario: Handle missing optional fields
    Given an HttpPollingSource
    And the source returns {"pm02": 15.5} (temp field missing)
    When the source polls the endpoint
    Then the ingestion should succeed
    And raw_payload should exactly match {"pm02": 15.5}
    And no default values should be injected

  Scenario: Handle non-numeric values
    Given an HttpPollingSource
    And the source returns {"state": "open", "battery": 85, "active": true}
    When the source polls the endpoint
    Then the ingestion should succeed
    And raw_payload["state"] should be the string "open"
    And raw_payload["battery"] should be the number 85
    And raw_payload["active"] should be the boolean true
```

---

## AC-4: Parser Simplification

### AC-4.1: No Metric Extraction

```gherkin
Feature: Parser preserves raw payload

  Scenario: Parser passes through JSON unchanged
    Given a JSON payload {"pm02": 12.5, "rco2": 450, "serialno": "abc123"}
    When the parser processes the payload
    Then no metric extraction should occur
    And the output raw_payload should exactly equal the input
    And no fields should be added, removed, or modified

  Scenario: Parser attaches metadata only
    Given a JSON payload {"value": 42}
    And a ParseContext with ndp_id "sensor-001" and context {"room": "lab"}
    When the parser processes the payload with context
    Then the output should include:
      | Field | Value |
      | raw_payload | {"value": 42} |
      | ndp_id | "sensor-001" |
      | context | {"room": "lab"} |
```

---

## AC-5: Pipeline Integration

### AC-5.1: End-to-End Flow

```gherkin
Feature: Complete ingestion pipeline

  Scenario: Ingest data from HTTP source to Parquet
    Given an HTTP source polling "http://sensor.local/data"
    And the source is configured with stream_id "air-quality"
    And the endpoint returns {"pm02": 10.5, "temp": 21.0}
    When the ingestion pipeline runs
    Then a RawDataPoint should be written to Parquet
    And the file should be at "data/air-quality/year=YYYY/month=MM/day=DD/readings.parquet"
    And querying the file should return the exact raw_payload

  Scenario: WAL recovery preserves raw payload
    Given a RawDataPoint was written to WAL but not committed
    When the system restarts
    And WAL replay occurs
    Then the RawDataPoint should be recovered
    And the raw_payload should exactly match the original
```

### AC-5.2: Backward Compatibility

```gherkin
Feature: Backward compatibility with existing data

  Scenario: Read old schema Parquet files
    Given a Parquet file with the old 7-column schema
    When I query the file using the storage layer
    Then the read should succeed
    And the data should be interpretable (though not as RawDataPoint)

  Scenario: Coexist with old schema files
    Given a directory containing both old and new schema Parquet files
    When I list partitions
    Then both file types should be discoverable
    And queries should handle schema differences gracefully
```

---

## AC-6: Performance Requirements

### AC-6.1: Ingestion Performance

```gherkin
Feature: Ingestion performance

  Scenario: Throughput within acceptable range
    Given 1000 RawDataPoints with average payload size 500 bytes
    When I write them as a batch
    Then the write should complete in under 5 seconds
    And the throughput should be at least 200 points/second

  Scenario: Memory usage within limits
    Given a continuous ingestion stream of 100 points/second
    When the pipeline runs for 1 minute
    Then peak memory usage should not exceed 256 MB
    And no memory leaks should be detected
```

### AC-6.2: Storage Efficiency

```gherkin
Feature: Storage efficiency

  Scenario: File size reasonable for JSON storage
    Given 10,000 RawDataPoints with average payload size 400 bytes
    When written to a single Parquet file with Snappy compression
    Then the file size should be under 2 MB
    And compression ratio should be at least 3:1
```

---

## AC-7: Observability

### AC-7.1: Logging

```gherkin
Feature: Operational logging

  Scenario: Log raw payload size
    Given a RawDataPoint with raw_payload size 512 bytes
    When the point is written to storage
    Then a debug log should include "raw_payload_size=512"

  Scenario: Log ingestion latency
    Given a successful write operation
    When the write completes
    Then a log entry should include the write duration in milliseconds
```

---

## Verification Matrix

| Acceptance Criteria | Requirement | Test Type | Automated |
|---------------------|-------------|-----------|-----------|
| AC-1.1 | FR-1.1 - FR-1.7 | Unit | Yes |
| AC-1.2 | FR-1.7 | Unit | Yes |
| AC-2.1 | FR-2.1 - FR-2.3, FR-3.1 - FR-3.3 | Integration | Yes |
| AC-2.2 | FR-2.2 - FR-2.7 | Unit | Yes |
| AC-3.1 | FR-4.1 - FR-4.6 | Integration | Yes |
| AC-3.2 | FR-5.2, NFR-2.1 | Integration | Yes |
| AC-4.1 | FR-5.1 - FR-5.4 | Unit | Yes |
| AC-5.1 | FR-6.1 - FR-6.3 | E2E | Yes |
| AC-5.2 | NFR-2.1, NFR-2.2 | Integration | Yes |
| AC-6.1 | NFR-1.1, NFR-1.3 | Benchmark | Semi |
| AC-6.2 | NFR-1.2 | Benchmark | Semi |
| AC-7.1 | NFR-4.1, NFR-4.2 | Integration | Yes |

---

## References

- [DP-004 Requirements](./REQUIREMENTS.md)
- [ADR-001: Bronze Raw JSON Schema](../architecture/ADR-001-bronze-raw-json-schema.md)
