# Config Enhancement Research for NDP

**Document**: 02-config-enhancements.md
**Date**: 2026-01-17
**Author**: ndp-architect
**Status**: Research Complete

---

## Executive Summary

This document analyzes the current config-driven capabilities of the Neural Data Platform and identifies opportunities to make more functionality configurable, reducing the need for code changes when adding new streams, transforms, or data quality rules.

### Current State

NDP already has a robust config-driven architecture:
- YAML stream configs in `config/base/streams/{stream-id}/config.yaml`
- etcd-backed configuration with GitOps sync via `StreamRegistry`
- Hot-reload capability via etcd watch
- Rich config types for Bronze ingestion, Silver ETL, DQ rules, and parsers

### Key Findings

| Area | Current State | Recommendation |
|------|--------------|----------------|
| Parser Types | 4 built-in, no plugin system | Add config-driven custom parser registration |
| Source Types | 2 implemented (MQTT, HTTP), 2 stubbed | Implement remaining + plugin system |
| Transform Formulas | Linear only, Custom stubbed | Implement expression evaluation |
| DQ Rules | 11 rule types config-driven | Add custom rule expressions |
| Defaults | Many hardcoded in source_manager.rs | Move to config hierarchy |
| Schema Validation | Manual validation | Add JSON Schema validation |

---

## 1. Current Configuration Architecture

### 1.1 Configuration Hierarchy

```
Priority 1: Stream Registry (/streams/{id}/config in etcd)
Priority 2: Legacy etcd (/config/{app}/*)
Priority 3: YAML files (config/base/streams/*)
Priority 4: Code defaults (hardcoded in Rust)
```

**Current Files**:
- `/workspaces/neural-data-platform/config/base/streams/` - Stream YAML configs
- `/workspaces/neural-data-platform/config-client/` - etcd client crate
- `/workspaces/neural-data-platform/core/src/types/stream_config.rs` - StreamConfig struct
- `/workspaces/neural-data-platform/core/src/config/silver_etl.rs` - Silver ETL config
- `/workspaces/neural-data-platform/core/src/parsers/config.rs` - Parser config

### 1.2 Stream Config Sections

Each stream config YAML contains:

```yaml
# Stream identification
stream_id: air-quality
description: "..."
version: "1.0.0"
enabled: true

# Retention
retention_days: 365
compression_after_days: 7
partitioning_strategy: daily

# Schema
fields: [...]

# Sources
sources:
  - type: mqtt|http_poll
    ndp_id: "..."
    context: {...}
    parser: {...}

# Storage
storage:
  batch_size: 100
  batch_timeout_secs: 5
  buffer_capacity: 1000

# Entity Schemas (Data Dictionary)
entity_schemas: [...]

# Silver ETL (new in DP-006)
silver_etl:
  enabled: true
  target_table: silver.xxx
  field_mappings: [...]
  dq_rules: [...]
```

### 1.3 Hot-Reload Capabilities

Current capabilities in `config-client/src/watch.rs`:
- Watch etcd prefix for changes
- Callback on PUT/DELETE events
- No automatic reconnection on watch failure

**Gap**: No automatic re-application of config changes at runtime.

---

## 2. Gaps Requiring Code Changes

### 2.1 Source Type Implementation

**Location**: `core/src/coordinator/source_manager.rs:65-80`

```rust
match config.source_type {
    SourceType::Mqtt => self.spawn_mqtt_source(source_id, config).await,
    SourceType::HttpPoll => self.spawn_http_poll_source(source_id, config).await,
    SourceType::Webhook => Err("Webhook sources not implemented"),
    SourceType::FileWatch => Err("FileWatch sources not implemented"),
}
```

**Issue**: Adding a new source type requires:
1. Adding enum variant to `SourceType` in `stream_config.rs`
2. Implementing spawn logic in `source_manager.rs`
3. Creating source implementation in `core/src/sources/`

**Recommendation**: Create a pluggable source registry where source adapters can be registered at startup via configuration.

### 2.2 Parser Type Registration

**Location**: `core/src/parsers/factory.rs:24-51`

```rust
match config.parser_type {
    ParserType::FlatJson => Ok(Box::new(FlatJsonParser::from_config(config)?)),
    ParserType::JsonPath => Ok(Box::new(JsonPathParser::from_config(config)?)),
    ParserType::ArrayIterator => Ok(Box::new(ArrayIteratorParser::from_config(config)?)),
    ParserType::ColumnOriented => Ok(Box::new(ColumnOrientedParser::from_config(config)?)),
    ParserType::Custom(name) => Err("Custom parser type not registered"),
}
```

**Issue**: Custom parsers require code changes. The `Custom(String)` variant exists but has no registration mechanism.

**Recommendation**: Add parser registry with configuration-based registration:

```yaml
# config/parsers/custom_parsers.yaml
parsers:
  - name: my_custom_parser
    base_type: json_path
    extensions:
      pre_process: "jq_expression"
      post_process: "transform_expression"
```

### 2.3 Transform Expression Evaluation

**Location**: `core/src/config/silver_etl.rs:294-306`

```rust
pub enum ConversionFormula {
    Linear { scale: f64, offset: f64 },
    Custom { code: String },  // Future: evaluate custom expression
}

impl ConversionFormula {
    pub fn apply(&self, value: f64) -> f64 {
        match self {
            ConversionFormula::Linear { scale, offset } => (value * scale) + offset,
            ConversionFormula::Custom { .. } => value, // NOT IMPLEMENTED
        }
    }
}
```

**Issue**: The `Custom` transform is defined but not implemented. Users cannot add new transform types without code changes.

**Recommendation**: Implement expression evaluation engine (options):
1. Embedded Rhai scripting
2. SQL expression evaluation via DuckDB
3. Simple math expression parser (e.g., `evalexpr` crate)

### 2.4 DQ Rule Custom Expressions

**Location**: `core/src/config/silver_etl.rs:421-428`

The `CrossFieldCheck` rule allows SQL expressions:

```yaml
dq_rules:
  - rule: cross_field_check
    name: pm10_gte_pm25
    expression: "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25"
```

**Gap**: No mechanism for users to define entirely custom rule types.

**Recommendation**: Add `CustomRule` variant with expression-based validation:

```yaml
- rule: custom
  name: heat_stress_check
  expression: "wet_bulb_globe_temperature > 32 AND wind_speed < 5"
  action: flag
```

---

## 3. Hardcoded Values to Make Configurable

### 3.1 Source Manager Defaults

**Location**: `core/src/coordinator/source_manager.rs:94-127`

```rust
let port = params.get("port").and_then(|v| v.as_u64()).unwrap_or(1883) as u16;
let qos = params.get("qos").and_then(|v| v.as_u64()).unwrap_or(1);
let reconnect_delay_secs = params.get("reconnect_delay_secs").and_then(|v| v.as_u64()).unwrap_or(5);
let max_reconnect_delay_secs = params.get("max_reconnect_delay_secs").and_then(|v| v.as_u64()).unwrap_or(300);
let buffer_capacity = params.get("buffer_capacity").and_then(|v| v.as_u64()).unwrap_or(1000);
let poll_interval_secs = params.get("poll_interval_secs").and_then(|v| v.as_u64()).unwrap_or(60);
let timeout_secs = params.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(10);
```

**Recommendation**: Create a global defaults config:

```yaml
# config/base/defaults.yaml
mqtt:
  port: 1883
  qos: 1
  reconnect_delay_secs: 5
  max_reconnect_delay_secs: 300
  buffer_capacity: 1000

http_poll:
  poll_interval_secs: 60
  timeout_secs: 10
  buffer_capacity: 1000
  max_concurrent_fetches: 10

storage:
  batch_size: 100
  batch_timeout_secs: 5
  buffer_capacity: 1000
```

### 3.2 HTTP Polling Constants

**Location**: `core/src/sources/http_poll.rs:664`

```rust
const MAX_CONCURRENT_FETCHES: usize = 10;
```

**Location**: `core/src/sources/http_poll.rs:62-70`

```rust
impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}
```

**Recommendation**: Make these configurable per-source or globally.

### 3.3 MCP Tool Constants

**Location**: `core/src/mcp/tools/sample_data.rs:41-44`

```rust
const DEFAULT_N: usize = 10;
const MAX_N: usize = 100;
```

**Recommendation**: Move to MCP server config:

```yaml
# config/mcp/server.yaml
tools:
  sample_data:
    default_n: 10
    max_n: 100
  list_streams:
    max_streams: 1000
```

### 3.4 Parquet Storage Constants

**Location**: `core/src/storage/parquet.rs:462`

```rust
const SUFFIXES: &[&str] = &["-FileWatch", "-Webhook", "-HttpPoll", "-Http", "-Mqtt"];
```

These suffixes are used to clean stream IDs. Should be configurable.

### 3.5 WAL File Name

**Location**: `core/src/storage/parquet.rs:26`

```rust
let wal_path = base_path.join("wal.log");
```

**Recommendation**: Make WAL file name configurable.

---

## 4. Hot-Reload Improvements

### 4.1 Current State

The `WatchHandle` in `config-client/src/watch.rs` provides basic etcd watch:

```rust
pub struct WatchHandle {
    cancel_tx: mpsc::Sender<()>,
}

impl WatchHandle {
    pub async fn new<F>(client: Client, prefix: &str, callback: F) -> Result<Self, ConfigError>
    where
        F: Fn(String, Option<serde_json::Value>) + Send + Sync + 'static,
    { ... }
}
```

### 4.2 Gaps

1. **No automatic reconnection**: If etcd watch fails, no retry logic
2. **No change diffing**: Callback receives raw value, no diff with previous
3. **No validation before apply**: Invalid config could break running system
4. **No atomic updates**: Multi-key changes not transactional

### 4.3 Recommendations

#### 4.3.1 Add Watch Reconnection

```yaml
# config/etcd/client.yaml
watch:
  reconnect:
    enabled: true
    initial_delay_ms: 1000
    max_delay_ms: 30000
    backoff_multiplier: 2.0
```

#### 4.3.2 Add Config Validation on Hot-Reload

Before applying a config change:
1. Parse and validate new config
2. Check compatibility with running state
3. Apply atomically or reject with error

#### 4.3.3 Add Change Notification System

```rust
pub enum ConfigChangeType {
    Added { key: String, value: Value },
    Modified { key: String, old_value: Value, new_value: Value },
    Deleted { key: String, old_value: Value },
}

pub trait ConfigChangeHandler {
    fn on_change(&self, change: ConfigChangeType) -> Result<(), Error>;
}
```

#### 4.3.4 Add Live Source Reconfiguration

Currently, changing a source config requires restart. Enable:
- Poll interval changes without restart
- Parser config updates without restart
- DQ rule updates without restart

---

## 5. Schema Validation Enhancements

### 5.1 Current State

Validation is manual in Rust code:
- `StreamConfig::validate()` checks basic structure
- `SilverFieldMapping::validate()` checks column types
- `DqRule::validate()` checks rule parameters

### 5.2 Recommendations

#### 5.2.1 Add JSON Schema for Stream Configs

Create schema files:
- `config/schemas/stream_config.schema.json`
- `config/schemas/silver_etl.schema.json`
- `config/schemas/parser.schema.json`

Benefits:
- IDE autocompletion in YAML editors
- Pre-commit validation
- Documentation generation

#### 5.2.2 Add Runtime Schema Validation

```yaml
# config/base/validation.yaml
validation:
  strict_mode: true
  on_invalid_config: reject  # reject | warn | ignore
  schema_version: "2.0"
```

---

## 6. Additional Config Options to Add

### 6.1 Stream-Level Options

```yaml
# config/base/streams/{stream}/config.yaml

# Monitoring
monitoring:
  enabled: true
  metrics_prefix: ndp_stream
  health_check_interval_secs: 30

# Alerting
alerting:
  enabled: true
  rules:
    - name: data_staleness
      condition: "last_point_age > 5m"
      severity: warning
    - name: high_error_rate
      condition: "error_rate > 0.1"
      severity: critical

# Backpressure
backpressure:
  strategy: drop_oldest  # drop_oldest | block | drop_newest
  high_watermark: 0.8
  low_watermark: 0.2
```

### 6.2 Parser Extensions

```yaml
parser:
  parser_type: json_path
  # New: Pre/post processing
  pre_process:
    - type: jq_filter
      expression: ".data | select(.valid == true)"
  post_process:
    - type: add_computed_field
      name: aqi_category
      expression: "CASE WHEN aqi <= 50 THEN 'Good' ELSE 'Moderate' END"

  # New: Error handling
  error_handling:
    on_parse_error: skip  # skip | null | default
    default_values:
      temperature: 0.0
```

### 6.3 Silver ETL Extensions

```yaml
silver_etl:
  # New: Batch processing
  batch:
    max_rows: 10000
    max_age_secs: 60

  # New: Partitioning
  partitioning:
    type: time  # time | hash | range
    column: observation_time
    interval: 1 day

  # New: Materialized views
  continuous_aggregates:
    - name: hourly_summary
      interval: 1 hour
      aggregations:
        - column: temperature_c
          functions: [avg, min, max]
```

### 6.4 Global Platform Config

```yaml
# config/platform.yaml
platform:
  name: neural-data-platform
  environment: production

  # Resource limits
  resources:
    max_memory_mb: 512
    max_concurrent_sources: 10
    max_concurrent_etl_jobs: 3

  # Observability
  observability:
    tracing:
      enabled: true
      exporter: otlp
      endpoint: "http://jaeger:4317"
    metrics:
      enabled: true
      exporter: prometheus
      port: 9090

  # Security
  security:
    tls:
      enabled: false
      cert_path: /etc/ssl/certs/ndp.crt
      key_path: /etc/ssl/private/ndp.key
```

---

## 7. Implementation Priority

### Phase 1: Quick Wins (Low Effort, High Value)

1. **Move hardcoded defaults to config** - 1-2 days
   - Create `config/base/defaults.yaml`
   - Update `source_manager.rs` to read defaults
   - Backward compatible

2. **Add JSON Schema for IDE support** - 1 day
   - Generate from existing Rust types
   - Add to `config/schemas/`

3. **Add watch reconnection** - 1 day
   - Update `WatchHandle` with retry logic

### Phase 2: Medium Effort Features

4. **Implement Custom transform expressions** - 3-5 days
   - Add `evalexpr` or similar crate
   - Implement `ConversionFormula::Custom`

5. **Add validation before hot-reload** - 2-3 days
   - Parse and validate before applying
   - Rollback on failure

6. **Add parser pre/post processing** - 3-5 days
   - Implement jq-like filtering
   - Add computed field support

### Phase 3: Larger Initiatives

7. **Plugin system for sources** - 1-2 weeks
   - Design plugin interface
   - Implement dynamic loading

8. **Plugin system for parsers** - 1 week
   - Extend `ParserRegistry` with config-based registration

9. **Live source reconfiguration** - 1-2 weeks
   - Design graceful reconfiguration
   - Implement per-component hot-reload

---

## 8. Conclusion

NDP already has a strong config-driven foundation. The key opportunities are:

1. **Reduce hardcoded defaults** - Move 15+ hardcoded values to config hierarchy
2. **Enable custom transforms** - Implement the stubbed `Custom` formula variant
3. **Improve hot-reload** - Add validation, reconnection, and live reconfiguration
4. **Add schema validation** - JSON Schema for IDE support and validation
5. **Create plugin systems** - Allow new sources and parsers without code changes

These enhancements would enable operators to add new data streams, modify transforms, and update DQ rules entirely through configuration changes, achieving the goal of minimal code changes for common operations.

---

## Appendix A: Current Config File Inventory

| File | Purpose |
|------|---------|
| `config/base/streams/air-quality/config.yaml` | Indoor AirGradient sensor stream |
| `config/base/streams/outdoor-weather/config.yaml` | OpenWeatherMap current weather |
| `config/base/streams/outdoor-air-quality/config.yaml` | OpenWeatherMap air pollution |
| `config/base/streams/nws-observations/config.yaml` | NWS station observations |
| `config/base/streams/nws-forecast-hourly/config.yaml` | NWS hourly forecast |
| `config/base/streams/nws-gridpoints-forecast/config.yaml` | NWS gridpoint forecast (40+ metrics) |
| `config/samples/mqtt_stream.yaml` | MQTT stream template |
| `config/samples/http_stream.yaml` | HTTP polling stream template |

## Appendix B: Key Rust Config Types

| Type | Location | Purpose |
|------|----------|---------|
| `StreamConfig` | `core/src/types/stream_config.rs` | Top-level stream configuration |
| `SourceConfig` | `core/src/types/stream_config.rs` | Individual source within stream |
| `ParserConfig` | `core/src/parsers/config.rs` | Parser configuration |
| `SilverEtlConfig` | `core/src/config/silver_etl.rs` | Bronze-to-Silver ETL config |
| `DqRule` | `core/src/config/silver_etl.rs` | Data quality rule definitions |
| `ConfigClient` | `config-client/src/client.rs` | etcd client wrapper |
| `StreamRegistry` | `config-client/src/stream/registry.rs` | Stream config management |
