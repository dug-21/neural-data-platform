# ADR-002: Stream Registry Design

**Status**: Approved
**Date**: 2025-12-15
**Decision Makers**: Architecture Team
**Context**: AIR-004 Multi-Stream Data Platform
**Related**: ADR-001 (Multi-Stream Foundation)

---

## Context and Problem Statement

The multi-stream platform requires a centralized registry to dynamically manage stream configurations, schemas, and source definitions. The registry must support:

1. **Dynamic Registration**: Add/remove streams without code changes
2. **Schema Management**: Define per-stream field types and validation rules
3. **Source Configuration**: Specify ingestion sources (MQTT topics, HTTP endpoints, etc.)
4. **Real-Time Updates**: Hot-reload when stream definitions change
5. **Version Control**: Configuration as code (GitOps workflow)

### Current State (AIR-003)

- etcd v3.5.11 running in production
- config-client (260 LOC) wrapping etcd with type-safe API
- GitOps sync pattern (YAML → etcd)
- Watch API proven for hot-reload (< 100ms latency)
- Hierarchical key structure: `/air-quality/mqtt/broker_url`

### Requirements

**Functional**:
- Store stream definitions (config, schema, sources)
- Support 3-5 streams initially, scalable to 10+
- Watch API for dynamic stream spawning
- Schema validation before ingestion
- Per-stream retention policies

**Non-Functional**:
- Registry read latency: < 10ms p95 (reuse AIR-003 performance)
- Update notification: < 100ms (watch API)
- High availability: Multi-node etcd cluster (production)
- GitOps integration: YAML source of truth

---

## Decision

**Use etcd as the stream registry backend with a thin extension to the existing config-client crate.**

### Architecture

```
┌────────────────────────────────────────────────────────────┐
│                Git Repository (Source of Truth)             │
│  config/streams/                                           │
│  ├── air-quality.yaml                                      │
│  ├── home-events.yaml                                      │
│  └── weather.yaml                                          │
└────────────────────┬───────────────────────────────────────┘
                     │ GitOps Sync (sync-streams-to-etcd.sh)
                     ▼
┌────────────────────────────────────────────────────────────┐
│                    etcd v3.5.11                            │
│  /streams/                                                 │
│  ├── air-quality/                                          │
│  │   ├── config     → StreamConfig JSON                   │
│  │   ├── schema     → SchemaDefinition JSON               │
│  │   └── sources    → SourceConfig[] JSON                 │
│  ├── home-events/                                          │
│  │   ├── config     → ...                                 │
│  │   ├── schema     → ...                                 │
│  │   └── sources    → ...                                 │
│  └── weather/                                              │
│      └── ...                                               │
└────────────────────┬───────────────────────────────────────┘
                     │ gRPC + Watch API
                     ▼
┌────────────────────────────────────────────────────────────┐
│         StreamRegistry (extends config-client)             │
│                                                            │
│  pub struct StreamRegistry {                              │
│      client: ConfigClient,  // Reuse existing client      │
│  }                                                         │
│                                                            │
│  impl StreamRegistry {                                    │
│      pub async fn load_stream(&self, stream_id: &str)     │
│          -> Result<StreamConfig>;                         │
│                                                            │
│      pub async fn list_streams(&self)                     │
│          -> Result<Vec<String>>;                          │
│                                                            │
│      pub async fn watch_streams(&self)                    │
│          -> Result<Receiver<StreamEvent>>;                │
│  }                                                         │
└────────────────────┬───────────────────────────────────────┘
                     │ Consume stream definitions
                     ▼
         ┌───────────────────────────┐
         │  Ingestion Coordinator    │
         │  - Spawn sources          │
         │  - Validate records       │
         │  - Route to storage       │
         └───────────────────────────┘
```

---

## Rationale

### Alternative Approaches Considered

#### Alternative 1: Hardcoded Stream Definitions

**Approach**: Define streams in Rust code as constants or config files

```rust
const STREAMS: &[StreamDefinition] = &[
    StreamDefinition {
        id: "air-quality",
        schema: vec![Field { name: "pm25", type: FieldType::Float }],
        sources: vec![Source::Mqtt { topic: "airgradient/#" }],
    },
];
```

**Pros**:
- Simple implementation
- No external dependencies
- Compile-time validation

**Cons**:
- Requires code recompilation to add streams
- No hot-reload capability
- Couples configuration to deployment
- No GitOps workflow

**Verdict**: Rejected - Too rigid for operational needs

---

#### Alternative 2: PostgreSQL/SQLite Registry

**Approach**: Store stream definitions in relational database

**Pros**:
- Rich query capabilities (SQL)
- Transactional updates
- Familiar tooling (psql, DBeaver)

**Cons**:
- No built-in watch API (polling required)
- Additional infrastructure (PostgreSQL)
- Overkill for simple key-value needs
- Slower than etcd for reads (< 10ms requirement)

**Verdict**: Rejected - Not optimized for configuration use case

---

#### Alternative 3: Custom REST API

**Approach**: Build HTTP API for stream management

```
POST /api/streams/air-quality
GET /api/streams
PUT /api/streams/air-quality/schema
```

**Pros**:
- HTTP-based (familiar, easy to test)
- Custom business logic
- Versioning support (API v1, v2)

**Cons**:
- Custom code to build and maintain (500+ LOC)
- No watch API (would need WebSocket or SSE)
- Requires persistence backend (etcd/PostgreSQL anyway)
- Slower than direct etcd access

**Verdict**: Rejected - Unnecessary abstraction layer

---

### Chosen Approach: etcd-Based Registry

**Why etcd**:

1. **Already Deployed** (AIR-003):
   - etcd v3.5.11 running in production
   - config-client (260 LOC) proven and tested
   - Performance meets requirements (< 10ms reads, < 100ms watch)

2. **Built-in Watch API**:
   - Real-time notifications on stream changes
   - No polling overhead
   - Proven in AIR-003 for config hot-reload

3. **Hierarchical Key Structure**:
   - Natural fit for nested stream definitions
   - Prefix queries for listing streams
   - Atomic updates per stream

4. **GitOps Ready**:
   - Reuse sync-config-to-etcd.sh pattern
   - YAML source of truth in git
   - Audit trail via git commits

5. **Minimal New Code**:
   - Extend config-client with StreamRegistry (~150 LOC)
   - Reuse existing error handling, serialization, watch patterns

---

## Design Details

### 1. etcd Key Structure

```
/streams/{stream-id}/config     → StreamConfig
/streams/{stream-id}/schema     → SchemaDefinition
/streams/{stream-id}/sources    → SourceConfig[]
```

**Example: Air Quality Stream**

```
Key: /streams/air-quality/config
Value: {
  "stream_id": "air-quality",
  "description": "Indoor air quality measurements from AirGradient sensors",
  "enabled": true,
  "retention_days": 365,
  "compression_after_days": 7,
  "tags": ["iot", "air-quality", "health"]
}

Key: /streams/air-quality/schema
Value: {
  "version": "1.0",
  "fields": [
    {
      "name": "pm25",
      "type": "float",
      "unit": "µg/m³",
      "nullable": false,
      "validation": {
        "min": 0.0,
        "max": 500.0
      }
    },
    {
      "name": "pm10",
      "type": "float",
      "unit": "µg/m³",
      "nullable": true,
      "validation": {
        "min": 0.0,
        "max": 500.0
      }
    },
    {
      "name": "co2",
      "type": "int",
      "unit": "ppm",
      "nullable": false,
      "validation": {
        "min": 380,
        "max": 10000
      }
    },
    {
      "name": "temperature",
      "type": "float",
      "unit": "celsius",
      "nullable": true,
      "validation": {
        "min": -10.0,
        "max": 50.0
      }
    },
    {
      "name": "humidity",
      "type": "float",
      "unit": "percent",
      "nullable": true,
      "validation": {
        "min": 0.0,
        "max": 100.0
      }
    }
  ]
}

Key: /streams/air-quality/sources
Value: [
  {
    "id": "airgradient-mqtt",
    "type": "mqtt",
    "enabled": true,
    "config": {
      "topic": "airgradient/readings/#",
      "qos": 1
    }
  },
  {
    "id": "airgradient-http-fallback",
    "type": "http_poll",
    "enabled": false,
    "config": {
      "url": "http://192.168.1.100/measures/current",
      "interval_secs": 60
    }
  }
]
```

**Example: Home Events Stream**

```
Key: /streams/home-events/config
Value: {
  "stream_id": "home-events",
  "description": "Discrete home automation events",
  "enabled": true,
  "retention_days": 730,
  "compression_after_days": 30,
  "tags": ["home-automation", "events"]
}

Key: /streams/home-events/schema
Value: {
  "version": "1.0",
  "fields": [
    {
      "name": "event_type",
      "type": "string",
      "nullable": false,
      "validation": {
        "enum": ["window_state", "door_state", "cooking_activity", "occupancy"]
      }
    },
    {
      "name": "target",
      "type": "string",
      "nullable": false,
      "description": "Window ID, door ID, room name, etc."
    },
    {
      "name": "state",
      "type": "string",
      "nullable": true,
      "validation": {
        "enum": ["open", "closed", "active", "inactive"]
      }
    },
    {
      "name": "metadata",
      "type": "json",
      "nullable": true,
      "description": "Additional context (duration, trigger source, etc.)"
    }
  ]
}

Key: /streams/home-events/sources
Value: [
  {
    "id": "homebridge-mqtt",
    "type": "mqtt",
    "enabled": true,
    "config": {
      "topic": "home/events/#",
      "qos": 1
    }
  },
  {
    "id": "manual-webhook",
    "type": "webhook",
    "enabled": true,
    "config": {
      "path": "/api/events",
      "auth": {
        "type": "bearer",
        "token_env": "WEBHOOK_AUTH_TOKEN"
      }
    }
  }
]
```

---

### 2. Rust Type Definitions

```rust
/// Stream configuration metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    pub stream_id: String,
    pub description: String,
    pub enabled: bool,
    pub retention_days: u32,
    pub compression_after_days: u32,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Field schema definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDefinition {
    pub version: String,
    pub fields: Vec<SchemaField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaField {
    pub name: String,
    pub field_type: FieldType,
    #[serde(default)]
    pub unit: Option<String>,
    pub nullable: bool,
    #[serde(default)]
    pub validation: Option<FieldValidation>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    Int,
    Float,
    String,
    Bool,
    Json,
    Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldValidation {
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub r#enum: Option<Vec<String>>,
    #[serde(default)]
    pub pattern: Option<String>,  // Regex
}

/// Source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    pub id: String,
    pub source_type: SourceType,
    pub enabled: bool,
    pub config: serde_json::Value,  // Type-specific config
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Mqtt,
    HttpPoll,
    Webhook,
    Websocket,
    FileWatch,
}
```

---

### 3. StreamRegistry Implementation

```rust
use config_client::{ConfigClient, ConfigError};

pub struct StreamRegistry {
    client: ConfigClient,
}

impl StreamRegistry {
    /// Create new registry client
    pub async fn new(endpoints: &[&str]) -> Result<Self, ConfigError> {
        let client = ConfigClient::with_prefix(endpoints, "/streams").await?;
        Ok(Self { client })
    }

    /// Load complete stream definition
    pub async fn load_stream(&self, stream_id: &str) -> Result<StreamDefinition, ConfigError> {
        let config_key = format!("{}/config", stream_id);
        let schema_key = format!("{}/schema", stream_id);
        let sources_key = format!("{}/sources", stream_id);

        let config: StreamConfig = self.client.get(&config_key).await?;
        let schema: SchemaDefinition = self.client.get(&schema_key).await?;
        let sources: Vec<SourceConfig> = self.client.get(&sources_key).await?;

        Ok(StreamDefinition {
            config,
            schema,
            sources,
        })
    }

    /// List all stream IDs
    pub async fn list_streams(&self) -> Result<Vec<String>, ConfigError> {
        let keys = self.client.list("").await?;

        // Extract stream IDs from keys like "air-quality/config"
        let streams: HashSet<String> = keys
            .iter()
            .filter_map(|k| k.split('/').next())
            .map(|s| s.to_string())
            .collect();

        Ok(streams.into_iter().collect())
    }

    /// Watch for stream changes (add, update, delete)
    pub async fn watch_streams(&self) -> Result<Receiver<StreamEvent>, ConfigError> {
        let (tx, rx) = mpsc::channel(100);

        self.client.watch("", move |key, value| {
            let event = match value {
                Some(v) => {
                    // Parse key to determine event type
                    if key.ends_with("/config") {
                        let stream_id = key.split('/').next().unwrap().to_string();
                        StreamEvent::Updated { stream_id }
                    } else {
                        return;  // Ignore schema/sources updates for now
                    }
                }
                None => {
                    // Key deleted
                    let stream_id = key.split('/').next().unwrap().to_string();
                    StreamEvent::Deleted { stream_id }
                }
            };

            let _ = tx.try_send(event);
        }).await?;

        Ok(rx)
    }
}

#[derive(Debug, Clone)]
pub enum StreamEvent {
    Updated { stream_id: String },
    Deleted { stream_id: String },
}

#[derive(Debug, Clone)]
pub struct StreamDefinition {
    pub config: StreamConfig,
    pub schema: SchemaDefinition,
    pub sources: Vec<SourceConfig>,
}
```

**Key Design Decisions**:

1. **Wrap ConfigClient**: Reuse existing 260 LOC implementation
2. **Prefix `/streams`**: Isolate stream registry from other etcd keys
3. **Structured Keys**: `{stream-id}/{config|schema|sources}` for granularity
4. **Watch Granularity**: Watch entire `/streams` prefix, filter in callback
5. **Event Types**: Updated vs Deleted (no "Created" - updated implies creation)

---

### 4. GitOps Sync Pattern

**YAML Source Files** (git repository):

```yaml
# config/streams/air-quality.yaml
stream_id: air-quality
description: Indoor air quality measurements from AirGradient sensors
enabled: true
retention_days: 365
compression_after_days: 7
tags:
  - iot
  - air-quality
  - health

schema:
  version: "1.0"
  fields:
    - name: pm25
      type: float
      unit: µg/m³
      nullable: false
      validation:
        min: 0.0
        max: 500.0
    - name: co2
      type: int
      unit: ppm
      nullable: false
      validation:
        min: 380
        max: 10000
    # ... more fields

sources:
  - id: airgradient-mqtt
    type: mqtt
    enabled: true
    config:
      topic: "airgradient/readings/#"
      qos: 1
```

**Sync Script** (reuse AIR-003 pattern):

```bash
#!/bin/bash
# scripts/sync-streams-to-etcd.sh

set -e

ETCD_ENDPOINTS=${ETCD_ENDPOINTS:-http://localhost:2379}
STREAM_DIR=${STREAM_DIR:-./config/streams}

echo "Syncing stream definitions to etcd..."

for file in "$STREAM_DIR"/*.yaml; do
    stream_id=$(basename "$file" .yaml)
    echo "  Processing stream: $stream_id"

    # Use yq to extract sections and push to etcd
    yq eval '.stream_id, .description, .enabled, .retention_days, .compression_after_days, .tags' "$file" | \
        etcdctl --endpoints=$ETCD_ENDPOINTS \
                put /streams/$stream_id/config -

    yq eval '.schema' "$file" | \
        etcdctl --endpoints=$ETCD_ENDPOINTS \
                put /streams/$stream_id/schema -

    yq eval '.sources' "$file" | \
        etcdctl --endpoints=$ETCD_ENDPOINTS \
                put /streams/$stream_id/sources -
done

echo "Sync complete!"
```

**Deployment Integration**:

```yaml
# docker-compose.yml
services:
  ingestion-coordinator:
    image: neural-data-platform/ingestion-coordinator
    depends_on:
      - etcd
    entrypoint:
      - /bin/sh
      - -c
      - |
        # Sync stream definitions on startup
        /app/scripts/sync-streams-to-etcd.sh
        # Start application
        /app/ingestion-coordinator
    volumes:
      - ./config/streams:/config/streams:ro
```

---

### 5. Schema Validation

**Validation Engine**:

```rust
pub struct SchemaValidator {
    schema: SchemaDefinition,
}

impl SchemaValidator {
    pub fn validate(&self, record: &StreamRecord) -> Result<(), ValidationError> {
        for field in &self.schema.fields {
            // Extract value from record.point.tags or record.point.value
            let value = self.extract_field(&record, &field.name)?;

            // Check nullability
            if value.is_none() && !field.nullable {
                return Err(ValidationError::RequiredFieldMissing {
                    field: field.name.clone(),
                });
            }

            // Type validation
            if let Some(v) = value {
                self.validate_type(v, &field.field_type)?;

                // Range/enum validation
                if let Some(validation) = &field.validation {
                    self.validate_constraints(v, validation)?;
                }
            }
        }

        Ok(())
    }

    fn validate_type(&self, value: &serde_json::Value, expected: &FieldType) -> Result<()> {
        match (expected, value) {
            (FieldType::Int, serde_json::Value::Number(n)) if n.is_i64() => Ok(()),
            (FieldType::Float, serde_json::Value::Number(_)) => Ok(()),
            (FieldType::String, serde_json::Value::String(_)) => Ok(()),
            (FieldType::Bool, serde_json::Value::Bool(_)) => Ok(()),
            (FieldType::Json, _) => Ok(()),  // Any JSON is valid
            _ => Err(ValidationError::TypeMismatch {
                expected: format!("{:?}", expected),
                actual: format!("{:?}", value),
            }),
        }
    }

    fn validate_constraints(&self, value: &serde_json::Value, validation: &FieldValidation) -> Result<()> {
        // Min/max for numbers
        if let Some(min) = validation.min {
            if value.as_f64().unwrap() < min {
                return Err(ValidationError::BelowMinimum { min, actual: value.as_f64().unwrap() });
            }
        }

        // Enum validation for strings
        if let Some(enum_values) = &validation.r#enum {
            if let Some(s) = value.as_str() {
                if !enum_values.contains(&s.to_string()) {
                    return Err(ValidationError::NotInEnum {
                        value: s.to_string(),
                        allowed: enum_values.clone(),
                    });
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Required field '{field}' is missing")]
    RequiredFieldMissing { field: String },

    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },

    #[error("Value {actual} is below minimum {min}")]
    BelowMinimum { min: f64, actual: f64 },

    #[error("Value '{value}' not in allowed enum: {allowed:?}")]
    NotInEnum { value: String, allowed: Vec<String> },
}
```

---

## Consequences

### Positive Consequences

1. **Zero New Infrastructure**:
   - Reuses existing etcd deployment
   - Extends config-client with ~150 LOC
   - Same performance characteristics (< 10ms reads)

2. **Dynamic Stream Management**:
   - Add streams via YAML commit + sync
   - Watch API enables hot-reload (< 100ms)
   - No application restart required

3. **GitOps Workflow**:
   - Stream definitions in version control
   - Code review for stream changes
   - Audit trail via git history

4. **Schema Validation**:
   - Enforce types and constraints before storage
   - Prevent bad data in Bronze/Silver layers
   - Errors visible immediately (not at query time)

5. **Operational Simplicity**:
   - Single source of truth (etcd)
   - Familiar tooling (etcdctl, YAML)
   - Proven patterns from AIR-003

### Negative Consequences (Trade-offs Accepted)

1. **etcd Dependency**:
   - Single point of failure (development)
   - **Mitigation**: Multi-node cluster (production)
   - **Accepted**: Already required for AIR-003

2. **No Schema Evolution Support (v1.0)**:
   - Adding fields requires manual coordination
   - **Mitigation**: Automated DDL generation (future)
   - **Accepted**: Rare operation, acceptable manual process

3. **Limited Validation Complexity**:
   - No cross-field validation (e.g., pm25 > pm10)
   - **Mitigation**: Application-level validation (future)
   - **Accepted**: Sufficient for initial use cases

---

## Performance Expectations

Based on AIR-003 benchmarks (etcd v3.5.11, local network):

| Operation | Latency (p95) | Throughput |
|-----------|---------------|------------|
| Load stream definition | 15-25ms | 200+ ops/sec |
| List streams | 10-20ms | 500+ ops/sec |
| Watch notification | < 100ms | Real-time |
| Sync YAML to etcd | 2-5 sec (3 streams) | Startup only |

**Scalability**:
- Current: 3-5 streams (< 50 keys total)
- Target: 10-20 streams (< 200 keys)
- etcd limit: 1000s of keys (not a concern)

---

## Security Considerations

1. **Stream Definition Access**:
   - Development: No auth (acceptable)
   - Production: etcd RBAC with read-only role for applications

2. **Sensitive Source Config**:
   - API keys in source config stored as env var references
   - Example: `{ "api_key_env": "WEATHER_API_KEY" }`
   - Application resolves from environment

3. **Schema Integrity**:
   - Git-based review process prevents malicious schemas
   - Validation engine enforces constraints

---

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_load_stream_definition() {
    let registry = StreamRegistry::new(&["http://localhost:2379"]).await?;
    let stream = registry.load_stream("air-quality").await?;
    assert_eq!(stream.config.stream_id, "air-quality");
    assert_eq!(stream.schema.fields.len(), 5);
}

#[test]
fn test_list_streams() {
    let registry = StreamRegistry::new(&["http://localhost:2379"]).await?;
    let streams = registry.list_streams().await?;
    assert!(streams.contains(&"air-quality".to_string()));
}

#[test]
fn test_watch_stream_updates() {
    let registry = StreamRegistry::new(&["http://localhost:2379"]).await?;
    let mut rx = registry.watch_streams().await?;

    // Trigger update in etcd
    etcdctl_put("/streams/new-stream/config", "{}");

    // Verify event received
    let event = rx.recv().await.unwrap();
    assert!(matches!(event, StreamEvent::Updated { stream_id } if stream_id == "new-stream"));
}

#[test]
fn test_schema_validation() {
    let schema = SchemaDefinition { /* ... */ };
    let validator = SchemaValidator::new(schema);

    // Valid record
    let record = StreamRecord { /* pm25: 25.0, co2: 800 */ };
    assert!(validator.validate(&record).is_ok());

    // Invalid: co2 out of range
    let record = StreamRecord { /* pm25: 25.0, co2: 15000 */ };
    assert!(matches!(
        validator.validate(&record),
        Err(ValidationError::AboveMaximum { .. })
    ));
}
```

### Integration Tests

- Sync YAML → etcd → load stream
- Watch API with actual etcd instance
- Schema validation with real records

---

## Migration Path

### Phase 1: Initial Implementation (Week 1-2)

1. Implement StreamRegistry (~150 LOC)
2. Define air-quality stream YAML
3. Sync script for YAML → etcd
4. Unit tests

### Phase 2: Validation (Week 2-3)

1. SchemaValidator implementation
2. Integration with Ingestion Router
3. Test with air-quality stream

### Phase 3: Multi-Stream (Week 3-4)

1. Add home-events and weather streams
2. Test cross-stream scenarios
3. Watch API hot-reload

---

## Related Decisions

- **ADR-001**: Multi-Stream Foundation (overall architecture)
- **ADR-003**: Storage Layer Strategy (Bronze + Silver)
- **ADR-004**: Source Abstraction Pattern (source spawning)
- **ADR-005**: Dual-Write Coordination (storage)

---

## References

- AIR-003 Architecture Summary (etcd patterns)
- config-client source code (260 LOC)
- etcd documentation: https://etcd.io/docs/v3.5/
- GitOps principles: https://opengitops.dev/

---

**Last Updated**: 2025-12-15
**Next Review**: After StreamRegistry implementation
