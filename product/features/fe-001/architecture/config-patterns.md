# NDP Configuration Architecture Patterns

**Document**: config-patterns.md
**Version**: 1.0
**Date**: 2026-02-03
**Author**: NDP Architect
**Feature**: FE-001 (Gold Layer Foundation / V1.1)

---

## Executive Summary

This document analyzes the existing NDP configuration architecture patterns to inform the design of Gold layer (gold_etl) configuration. The analysis reveals a mature, well-structured configuration system that follows consistent patterns, making Gold layer extension straightforward.

**Key Finding**: Gold ETL configuration should follow the established pattern of **embedding layer-specific config within stream configs** (like `silver_etl`), not creating separate config files.

---

## 1. Current Configuration Architecture

### 1.1 Configuration Hierarchy

NDP uses a layered configuration approach with clear priority ordering:

```
Priority 1: Stream Registry (/streams/{id}/config in etcd)
Priority 2: Legacy etcd (/config/{app}/*)
Priority 3: YAML files (config/*.yaml) and JSON files (config/base/streams/*/config.json)
Priority 4: Code defaults
```

**Source**: `/workspaces/neural-data-platform/docs/architecture/CONFIG_DRIVEN_SILVER_ETL_DESIGN.md`

### 1.2 Directory Structure

```
config/
├── base/
│   ├── platform.yaml                    # Platform-wide subscriber configuration
│   ├── streams/
│   │   ├── air-quality/
│   │   │   ├── config.json             # Primary stream config (V1.1 format)
│   │   │   └── config.yaml             # Legacy YAML (deprecated)
│   │   ├── outdoor-weather/
│   │   │   └── config.json
│   │   ├── home-assistant-state/
│   │   │   └── config.json
│   │   ├── nws-forecast-hourly/
│   │   │   └── config.json
│   │   └── ... (other streams)
│   ├── processors/
│   │   └── threshold-alerts.yaml       # Alert threshold rules
│   └── dimensions/
│       └── entity_context.yaml         # Dimensional data config
├── schemas/
│   └── homeassistant/
│       └── config.yaml                 # External schema definitions
├── overlays/
│   ├── development/
│   │   └── overrides.yaml
│   └── production/
│       └── overrides.yaml
└── grafana/
    └── dashboards/                     # Grafana dashboard configs
```

### 1.3 Format Standards

| Format | Status | Use Case |
|--------|--------|----------|
| **JSON** | Primary (V1.1+) | Stream configs, schema definitions |
| **YAML** | Legacy/Platform | Platform config, overlays, processors |

**ADR-016-001 (JSON Config Standard)**: JSON files are the authoritative source for stream configurations. YAML files exist for backward compatibility but are deprecated for stream configs.

---

## 2. Stream Configuration Structure

### 2.1 Core StreamConfig Fields

The `StreamConfig` struct (defined in `/workspaces/neural-data-platform/core/src/types/stream_config.rs`) contains:

```rust
pub struct StreamConfig {
    // Identity
    pub stream_id: String,              // Unique identifier (kebab-case)
    pub description: String,            // Human-readable description
    pub version: String,                // Semver

    // Lifecycle
    pub enabled: bool,
    pub retention_days: u32,
    pub compression_after_days: u32,
    pub partitioning_strategy: String,

    // Schema
    pub fields: Vec<SchemaField>,       // Field definitions with types, units, ranges

    // Data Sources
    pub sources: Vec<SourceConfig>,     // MQTT, HTTP polling, CSV sources

    // Storage
    pub storage: Option<StorageConfig>, // Batch size, timeout, buffer capacity

    // Layer-specific ETL (PATTERN FOR EXTENSION)
    pub silver_etl: Option<SilverEtlConfig>,  // DP-018: Silver layer transform config

    // Legacy (deprecated in V1.1)
    pub entity_schemas: Option<Vec<EntitySchema>>,
}
```

### 2.2 Silver ETL Configuration Pattern

The `silver_etl` field demonstrates the pattern for layer-specific configuration:

```rust
// From core/src/config/silver_etl.rs
pub struct SilverEtlConfig {
    pub enabled: bool,
    pub target_table: String,                    // e.g., "silver.air_quality_observations"
    pub target_schema: Option<String>,           // Optional versioned schema name
    pub timestamp: TimestampMapping,             // Timestamp field handling
    pub valid_timestamp: Option<ValidTimestampMapping>,  // For forecasts
    pub pre_transform: Option<PreTransformConfig>,       // Array explosion, etc.
    pub identity_fields: Vec<IdentityField>,     // Passthrough fields
    pub field_mappings: Vec<SilverFieldMapping>, // Transform + DQ rules
    pub dq_rules: Vec<DqRule>,                   // Global DQ rules
    pub dq_output: DqOutputConfig,               // DQ transparency config
    pub deduplication: DeduplicationConfig,      // Upsert strategy
    pub incremental: IncrementalConfig,          // Watermark-based loading
}
```

**Key Pattern**: Layer-specific ETL configuration is embedded directly in the stream config, not in separate files.

### 2.3 Example: Complete Stream Config with Silver ETL

From `/workspaces/neural-data-platform/config/base/streams/air-quality/config.json`:

```json
{
  "stream_id": "air-quality",
  "description": "AirGradient sensor readings from MQTT",
  "version": "1.0.0",
  "enabled": true,
  "retention_days": 365,

  "fields": [
    {
      "name": "pm02",
      "type": "float",
      "unit": "ug/m3",
      "description": "PM2.5 raw mass concentration",
      "range": [0, 1000]
    }
  ],

  "sources": [
    {
      "type": "mqtt",
      "enabled": true,
      "ndp_id": "aq_airgradient_1",
      "context": { "device_type": "airgradient", "location": {...} }
    }
  ],

  "silver_etl": {
    "enabled": true,
    "target_table": "silver.air_quality_observations",
    "timestamp": {
      "source_field": "timestamp",
      "target_field": "observation_time",
      "transform": "microseconds_to_timestamp"
    },
    "field_mappings": [
      {
        "source_path": "raw_payload.pm02Compensated",
        "target_column": "pm25",
        "type": "double_precision",
        "dq_rules": [
          { "rule": "range_check", "min": 0, "max": 1000, "action": "flag" }
        ]
      }
    ],
    "dq_output": {
      "enabled": true,
      "target_column": "dq_flags",
      "transparency": {
        "enabled": true,
        "table": "silver.dq_transparency"
      }
    }
  },
  "config_version": 2
}
```

---

## 3. Configuration Loading and Validation

### 3.1 Config Loading Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        CONFIG LOADING ARCHITECTURE                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  FILE LAYER                                                                  │
│  ──────────                                                                  │
│  config/base/streams/*/config.json                                           │
│           │                                                                  │
│           ▼                                                                  │
│  VALIDATION LAYER (JSON Schema)                                              │
│  ─────────────────────────────────                                           │
│  config/schemas/*.schema.json                                                │
│           │                                                                  │
│           ▼                                                                  │
│  SYNC LAYER (GitOps)                                                         │
│  ─────────────────────                                                       │
│  deploy/pi/scripts → etcd (/streams/{id}/config)                            │
│           │                                                                  │
│           ▼                                                                  │
│  RUNTIME LAYER                                                               │
│  ─────────────                                                               │
│  ┌──────────────────────┐     ┌──────────────────────┐                      │
│  │ ConfigLoader trait   │────▶│ StreamConfig struct  │                      │
│  │ (MockConfigLoader    │     │ (neural_core)        │                      │
│  │  or etcd adapter)    │     └──────────────────────┘                      │
│  └──────────────────────┘                                                   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 ConfigLoader Trait (Port/Adapter Pattern)

From `/workspaces/neural-data-platform/core/src/config/mock_loader.rs`:

```rust
#[async_trait]
pub trait ConfigLoader: Send + Sync {
    async fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig, ConfigLoaderError>;
    async fn load_silver_etl_config(&self, stream_id: &str) -> Result<SilverEtlConfig, ConfigLoaderError>;
    async fn list_streams(&self) -> Result<Vec<String>, ConfigLoaderError>;
    async fn stream_exists(&self, stream_id: &str) -> Result<bool, ConfigLoaderError>;
    fn source_name(&self) -> &'static str;
}
```

**Implementations**:
- `MockConfigLoader` - For testing without etcd
- `StreamRegistryAdapter` - Production adapter wrapping etcd StreamRegistry

### 3.3 Two-Layer Validation Architecture (ADR-019-001)

```
LAYER 1 - JSON SCHEMA (Declarative)
├── Validates structure before runtime
├── Validates enum values, required fields
└── config/schemas/*.schema.json

LAYER 2 - RUST VALIDATION (Semantic)
├── Validates business rules
├── Cross-field constraints
└── core/src/types/stream_config.rs::StreamConfig::validate()
```

---

## 4. Platform Configuration

### 4.1 Platform-Wide Config

From `/workspaces/neural-data-platform/config/base/platform.yaml`:

```yaml
# Platform-wide subscriber configuration
# GitOps managed - synced to etcd at /platform/*

timescale:
  connection_string: "${TIMESCALE_URL}"
  max_connections: 5
  table_mapping:
    air-quality: "air_quality_observations"
    outdoor-weather: "weather_observations"

subscribers:
  bronze:
    enabled: true
    batch_size: 100
    flush_interval_secs: 5

  silver:
    enabled: true
    catch_up:
      enabled: true
      window_secs: 3600
    batch_size: 50

  event_notifier:
    enabled: false
    mqtt:
      topic_prefix: "ndp/events"

  threshold_processor:
    enabled: false
    config_path: "config/base/processors/threshold-alerts.yaml"
```

**Pattern**: Platform-level configuration is separate from stream-level configuration.

---

## 5. etcd Configuration Store

### 5.1 etcd Key Structure

```
/streams/{stream_id}/config         # StreamConfig JSON blob
/platform/*                         # Platform-wide settings
/config/{app}/*                     # Legacy app-specific config
```

### 5.2 Config-Client Crate

From `/workspaces/neural-data-platform/config-client/`:

```rust
// Key methods
pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T, ConfigError>
pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<(), ConfigError>
pub async fn watch<F>(&self, prefix: &str, callback: F) -> Result<WatchHandle, ConfigError>
pub async fn get_with_env<T>(&self, key: &str, env_prefix: &str) -> Result<T, ConfigError>
```

**Features**:
- Type-safe serialization/deserialization
- Environment variable override mechanism
- Watch for configuration changes (hot reload)
- Prefix support for namespacing

---

## 6. Analysis: Gold ETL Config Location

### 6.1 Options Considered

| Option | Description | Pros | Cons |
|--------|-------------|------|------|
| **A: Embed in stream config** | Add `gold_etl` field to `StreamConfig` | Consistent with `silver_etl` pattern; single source of truth | Larger config files |
| **B: Separate gold config files** | `config/base/gold/*.yaml` | Clear separation; smaller stream configs | Duplication of stream identity; harder to keep in sync |
| **C: Hybrid approach** | Stream config references external gold config | Flexibility | Complex resolution; multiple files to manage |

### 6.2 Recommendation: Option A - Embed in Stream Config

**Decision**: Add `gold_etl: Option<GoldEtlConfig>` to `StreamConfig`, following the established `silver_etl` pattern.

**Rationale**:

1. **Consistency**: Matches the proven `silver_etl` pattern
2. **Single Source of Truth**: All stream configuration in one place
3. **Validation**: Existing JSON Schema and Rust validation infrastructure applies
4. **GitOps**: Same sync workflow to etcd
5. **Testing**: MockConfigLoader pattern works unchanged
6. **Discoverability**: All layer configs visible when loading a stream

### 6.3 Proposed StreamConfig Extension

```rust
pub struct StreamConfig {
    // ... existing fields ...

    /// Silver ETL configuration (DP-018)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub silver_etl: Option<SilverEtlConfig>,

    /// Gold ETL configuration (FE-001/V1.1)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gold_etl: Option<GoldEtlConfig>,
}
```

### 6.4 Proposed GoldEtlConfig Structure

Based on FE-001 scope, the `gold_etl` section should support:

```rust
pub struct GoldEtlConfig {
    pub enabled: bool,
    pub description: Option<String>,

    /// Continuous aggregate configuration
    pub aggregates: AggregatesConfig,

    /// Feature computation configuration
    pub features: FeaturesConfig,

    /// State transition configuration (for state_event streams)
    pub transitions: Option<TransitionsConfig>,

    /// Threshold crossing configuration (derives events from objectives)
    pub threshold_crossings: Option<ThresholdCrossingsConfig>,
}

pub struct AggregatesConfig {
    /// Time bucket granularities (e.g., ["1 hour", "1 day"])
    pub granularities: Vec<String>,

    /// Fields to aggregate with their metrics
    pub fields: HashMap<String, FieldAggregateConfig>,
}

pub struct FieldAggregateConfig {
    /// Metrics to compute (mean, std, min, max, count, p95, p99)
    pub metrics: Vec<String>,
}

pub struct FeaturesConfig {
    /// Lag feature configuration
    pub lag: Option<LagFeatureConfig>,

    /// Rolling statistics configuration
    pub rolling: Option<RollingFeatureConfig>,

    /// Trend computation configuration
    pub trend: Option<TrendFeatureConfig>,
}

pub struct LagFeatureConfig {
    pub enabled: bool,
    pub lags_hours: Vec<i32>,  // e.g., [1, 6, 24]
    pub fields: Vec<String>,   // Fields to compute lags for
}

pub struct RollingFeatureConfig {
    pub enabled: bool,
    pub windows: Vec<String>,  // e.g., ["4 hours", "24 hours"]
    pub stats: Vec<String>,    // e.g., ["mean", "std"]
    pub fields: Vec<String>,
}

pub struct TrendFeatureConfig {
    pub enabled: bool,
    pub window: String,        // e.g., "4 hours"
    pub fields: Vec<String>,
}

pub struct TransitionsConfig {
    pub enabled: bool,
    pub state_field: String,         // Field containing state value
    pub entity_field: String,        // Field for entity partitioning
    pub track_duration: bool,        // Compute duration in previous state
    pub include_in_alignment: bool,  // Include in gold.aligned_hourly
}

pub struct ThresholdCrossingsConfig {
    pub enabled: bool,
    pub source: String,              // "objectives" - derive from objectives config
    pub include_in_unified: bool,    // Include in gold.events_unified
}
```

---

## 7. Config Flow Summary

### 7.1 Bronze -> Silver -> Gold Config Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     STREAM CONFIG: SINGLE SOURCE OF TRUTH                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  config/base/streams/{stream_id}/config.json                                 │
│  ────────────────────────────────────────────                                │
│                                                                              │
│  {                                                                           │
│    "stream_id": "air-quality",                                              │
│    "fields": [...],           ──▶ BRONZE: Field schema                      │
│    "sources": [...],          ──▶ BRONZE: Data ingestion                    │
│    "storage": {...},          ──▶ BRONZE: Parquet writer                    │
│                                                                              │
│    "silver_etl": {            ──▶ SILVER: Bronze→Silver ETL                 │
│      "target_table": "silver.air_quality_observations",                     │
│      "field_mappings": [...],                                               │
│      "dq_rules": [...]                                                      │
│    },                                                                        │
│                                                                              │
│    "gold_etl": {              ──▶ GOLD: Silver→Gold aggregates/features     │
│      "aggregates": {...},                                                   │
│      "features": {...},                                                     │
│      "transitions": {...}                                                   │
│    }                                                                         │
│  }                                                                           │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 7.2 Cross-Stream Alignment Configuration

Following the FE-001 scope, alignment configuration should be **separate** from stream configs because it references multiple streams:

```
config/base/alignment.json
─────────────────────────
{
  "enabled": true,
  "view_name": "aligned_hourly",
  "granularity": "1 hour",
  "streams": [
    { "stream_id": "air-quality", "alias": "indoor" },
    { "stream_id": "outdoor-weather", "alias": "outdoor" },
    { "stream_id": "home-assistant-state", "alias": "state" }
  ],
  "join_strategy": "full_outer",
  "null_handling": "preserve"
}
```

### 7.3 Objectives Configuration

Objectives are also cross-stream and should be separate:

```
config/base/objectives.json
───────────────────────────
{
  "objectives": [
    {
      "id": "indoor_air_quality",
      "description": "Maintain healthy indoor air",
      "targets": [
        {
          "stream": "air-quality",
          "metric": "co2",
          "condition": "<",
          "threshold": 800,
          "unit": "ppm",
          "priority": "high"
        }
      ]
    }
  ]
}
```

---

## 8. Implementation Recommendations

### 8.1 For FE-001 Implementation

| Task | Location | Pattern |
|------|----------|---------|
| Define `GoldEtlConfig` struct | `core/src/config/gold_etl.rs` (new) | Match `silver_etl.rs` structure |
| Add to `StreamConfig` | `core/src/types/stream_config.rs` | Add optional `gold_etl` field |
| Create JSON Schema | `config/schemas/gold-etl.schema.json` | Match existing schema patterns |
| Update ConfigLoader | `core/src/config/mock_loader.rs` | Add `load_gold_etl_config()` |
| Create alignment config | `config/base/alignment.json` | New cross-stream config |
| Create objectives config | `config/base/objectives.json` | New objectives config |

### 8.2 File Naming Conventions

| Config Type | Location | Example |
|-------------|----------|---------|
| Stream config | `config/base/streams/{stream_id}/config.json` | `air-quality/config.json` |
| Platform config | `config/base/platform.yaml` | Single file |
| Alignment config | `config/base/alignment.json` | Single file (cross-stream) |
| Objectives config | `config/base/objectives.json` | Single file (cross-stream) |
| JSON Schemas | `config/schemas/*.schema.json` | `gold-etl.schema.json` |

### 8.3 Backward Compatibility

- `gold_etl` field is optional (`#[serde(default)]`)
- Streams without `gold_etl` continue normal Bronze/Silver operation
- Existing stream configs unchanged until explicitly extended
- `config_version` field tracks major config format changes

---

## 9. Patterns to Follow

### 9.1 Configuration Patterns (From Existing Codebase)

| Pattern | Description | Example |
|---------|-------------|---------|
| **Embedded Layer Config** | Layer-specific ETL in stream config | `silver_etl` section |
| **Optional with Default** | Use `Option<T>` with `#[serde(default)]` | All ETL config fields |
| **Skip Serializing None** | Don't output null fields | `#[serde(skip_serializing_if = "Option::is_none")]` |
| **Enum for Types** | Use enums for constrained values | `DqAction`, `SourceType` |
| **Validation in Struct** | Add `validate()` method to config structs | `SilverEtlConfig::validate()` |

### 9.2 Naming Conventions

| Entity | Convention | Example |
|--------|------------|---------|
| Stream ID | kebab-case | `air-quality`, `outdoor-weather` |
| Field name | snake_case | `pm25`, `temperature_c` |
| Config file | `config.json` | `/streams/air-quality/config.json` |
| SQL table | snake_case with layer prefix | `silver.air_quality_observations` |
| Rust struct | PascalCase | `GoldEtlConfig`, `AggregatesConfig` |

---

## 10. References

### 10.1 Key Files

| File | Purpose |
|------|---------|
| `/workspaces/neural-data-platform/core/src/types/stream_config.rs` | StreamConfig struct definition |
| `/workspaces/neural-data-platform/core/src/config/silver_etl.rs` | SilverEtlConfig reference implementation |
| `/workspaces/neural-data-platform/core/src/config/mock_loader.rs` | ConfigLoader trait definition |
| `/workspaces/neural-data-platform/config/base/streams/air-quality/config.json` | Example stream config with silver_etl |
| `/workspaces/neural-data-platform/config/base/platform.yaml` | Platform-wide configuration |
| `/workspaces/neural-data-platform/docs/architecture/CONFIG_DRIVEN_SILVER_ETL_DESIGN.md` | Silver ETL design document |
| `/workspaces/neural-data-platform/product/features/gold-001/FEATURE-ROADMAP.md` | Gold layer roadmap |

### 10.2 Related ADRs

| ADR | Topic |
|-----|-------|
| ADR-016-001 | JSON Configuration Standard |
| ADR-019-001 | Two-Layer Validation Architecture |
| ADR-018-001 | Pass-through Silver ETL Architecture |

### 10.3 AgentDB Patterns Found

| Pattern ID | Description | Success Rate |
|------------|-------------|--------------|
| architecture:json-config-standard | JSON files as authoritative source | 95% |
| config:stream-config-struct | StreamConfig structure pattern | 95% |
| architecture:two-layer-validation | JSON Schema + Rust validation | 90% |
| config:silver-etl-config-struct | SilverEtlConfig structure | 95% |
| config:config-loader-trait | ConfigLoader trait pattern | 95% |

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-02-03 | NDP Architect | Initial analysis |

---

## Conclusion

The NDP configuration architecture is mature and follows consistent patterns. The `gold_etl` configuration should:

1. **Embed in StreamConfig** - Following the `silver_etl` pattern
2. **Use JSON format** - Per ADR-016-001
3. **Support optional fields** - For backward compatibility
4. **Include validation** - Both JSON Schema and Rust semantic validation
5. **Integrate with ConfigLoader** - Using the existing trait pattern

Cross-stream configurations (alignment, objectives) should be separate files since they reference multiple streams.

This approach ensures consistency with existing patterns while enabling the declarative Gold layer architecture defined in FE-001.
